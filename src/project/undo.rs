//! 003-T1.15 — Undo / Redo stack for `Mutation`s.
//!
//! `UndoStack` is the in-process history that backs Cmd-Z /
//! Cmd-Shift-Z. Mutations enter via `push`, which applies them to
//! the project and stores the resulting Reverse on the undo
//! deque. `undo()` pops a Reverse, applies it, and pushes the
//! resulting Reverse onto the redo deque. `redo()` is symmetric.
//!
//! Pushing a fresh Mutation (i.e. anything other than an undo /
//! redo) clears the redo deque — standard editor semantics.
//!
//! `non_undoable` Mutations (today only the crossfade-tick flavour
//! of `ApplyProjectSnapshot`) skip the undo deque entirely; they
//! still apply their effect on the project but fire ~60×/s and
//! must not pollute the user-facing history.
//!
//! Soft cap: 200 entries on the undo deque (FIFO trim). The
//! redo deque is bounded by the undo deque's history.

#![deny(missing_docs)]
#![allow(dead_code)] // T-003-T1.16+ wires call sites; the stack is
// foundational scaffolding that lands in T1.15.

use std::collections::VecDeque;

use crate::project::command::Mutation;
use crate::project::schema::Project;

/// Soft cap on the undo history. Once exceeded, the oldest entry
/// is dropped (FIFO). Tuned for wedding-scale operator sessions —
/// 200 ≈ 5 minutes of moderately-active editing without
/// truncation.
pub const UNDO_HISTORY_CAP: usize = 200;

/// Undo / Redo stack. See module docs for semantics.
pub struct UndoStack {
    undo: VecDeque<Mutation>,
    redo: VecDeque<Mutation>,
}

impl UndoStack {
    /// Construct an empty stack.
    pub fn new() -> Self {
        Self {
            undo: VecDeque::new(),
            redo: VecDeque::new(),
        }
    }

    /// Apply `mutation` to `project`. If `mutation.is_non_undoable()`
    /// is false, push the resulting Reverse onto the undo deque
    /// (and clear redo, per editor convention). Otherwise apply
    /// without recording history.
    pub fn push(&mut self, mutation: Mutation, project: &mut Project) {
        let undoable = !mutation.is_non_undoable();
        let reverse = mutation.apply(project);
        if undoable {
            self.undo.push_back(reverse);
            // Trim oldest if we exceeded the cap. FIFO so the
            // most recent edits are always recoverable.
            while self.undo.len() > UNDO_HISTORY_CAP {
                self.undo.pop_front();
            }
            self.redo.clear();
        }
    }

    /// Pop one entry off the undo deque, apply it (which restores
    /// the prior state), push the resulting Reverse onto the
    /// redo deque. Returns `Some(needs_rebuild)` if anything was
    /// undone, where the bool indicates whether the renderer's
    /// per-layer GPU state must be rebuilt. Returns `None` if the
    /// stack was empty.
    pub fn undo(&mut self, project: &mut Project) -> Option<bool> {
        let reverse = self.undo.pop_back()?;
        let redo_entry = reverse.apply(project);
        let needs_rebuild = redo_entry.needs_layer_rebuild();
        self.redo.push_back(redo_entry);
        Some(needs_rebuild)
    }

    /// Symmetric to `undo`: pop one entry off the redo deque,
    /// apply it, push the result onto the undo deque. Returns
    /// `Some(needs_rebuild)` if anything was redone, `None` if
    /// the redo stack was empty.
    pub fn redo(&mut self, project: &mut Project) -> Option<bool> {
        let redo_entry = self.redo.pop_back()?;
        let undo_entry = redo_entry.apply(project);
        let needs_rebuild = undo_entry.needs_layer_rebuild();
        self.undo.push_back(undo_entry);
        Some(needs_rebuild)
    }

    /// Number of mutations available to undo.
    pub fn len(&self) -> usize {
        self.undo.len()
    }

    /// Number of mutations available to redo.
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// `true` iff there is nothing to undo.
    pub fn is_empty(&self) -> bool {
        self.undo.is_empty()
    }

    /// `true` iff the undo deque is non-empty, i.e. Cmd-Z would do something.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// `true` iff the redo deque is non-empty, i.e. Cmd-Shift-Z would do something.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_project() -> Project {
        Project::default()
    }

    /// Pushing N mutations grows undo by N; redo stays empty.
    #[test]
    fn push_grows_undo_stack() {
        let mut stack = UndoStack::new();
        let mut p = fresh_project();
        for v in [1.5_f32, 2.0, 0.8] {
            stack.push(p.set_gamma_mutation(v), &mut p);
        }
        assert_eq!(stack.len(), 3);
        assert_eq!(stack.redo_len(), 0);
        assert!((p.gamma - 0.8).abs() < 1e-6);
    }

    /// Calling undo once shrinks undo by 1 and grows redo by 1.
    /// State is restored to the moment before the most recent push.
    #[test]
    fn undo_moves_top_of_stack_to_redo() {
        let mut stack = UndoStack::new();
        let mut p = fresh_project();
        stack.push(p.set_gamma_mutation(1.5), &mut p);
        stack.push(p.set_gamma_mutation(2.0), &mut p);
        let undid = stack.undo(&mut p);
        assert!(undid.is_some());
        assert_eq!(stack.len(), 1);
        assert_eq!(stack.redo_len(), 1);
        assert!((p.gamma - 1.5).abs() < 1e-6);
    }

    /// Redo restores what undo just removed.
    #[test]
    fn redo_restores_undo_target() {
        let mut stack = UndoStack::new();
        let mut p = fresh_project();
        stack.push(p.set_gamma_mutation(2.5), &mut p);
        stack.undo(&mut p);
        let redid = stack.redo(&mut p);
        assert!(redid.is_some());
        assert_eq!(stack.len(), 1);
        assert_eq!(stack.redo_len(), 0);
        assert!((p.gamma - 2.5).abs() < 1e-6);
    }

    /// Pushing a fresh mutation after an undo clears the redo
    /// deque (standard editor semantics — branching history is
    /// out of scope for v3).
    #[test]
    fn push_after_undo_clears_redo() {
        let mut stack = UndoStack::new();
        let mut p = fresh_project();
        stack.push(p.set_gamma_mutation(1.5), &mut p);
        stack.undo(&mut p);
        assert_eq!(stack.redo_len(), 1);
        stack.push(p.set_brightness_mutation(0.3), &mut p);
        assert_eq!(stack.redo_len(), 0);
    }

    /// Cap behaviour: pushing > UNDO_HISTORY_CAP entries trims
    /// the oldest. The most recent edits remain recoverable.
    #[test]
    fn cap_trims_oldest_first() {
        let mut stack = UndoStack::new();
        let mut p = fresh_project();
        let total = UNDO_HISTORY_CAP + 5;
        for i in 0..total {
            // Alternate between gamma edits so the project stays
            // mutating each push.
            let v = 1.0 + (i % 2) as f32 * 0.1;
            stack.push(p.set_gamma_mutation(v), &mut p);
        }
        assert_eq!(stack.len(), UNDO_HISTORY_CAP);
    }

    /// `undo` on an empty stack returns `None` and is harmless.
    #[test]
    fn undo_on_empty_returns_none() {
        let mut stack = UndoStack::new();
        let mut p = fresh_project();
        let did = stack.undo(&mut p);
        assert!(did.is_none());
    }

    /// Apply N mutations + undo all of them returns the project
    /// to byte-equal serde_json. (Property test in miniature;
    /// T-003-T1.17 generalises across mutation kinds.)
    #[test]
    fn apply_then_undo_all_round_trips() {
        let mut stack = UndoStack::new();
        let mut p = fresh_project();
        let before = serde_json::to_value(&p).unwrap();

        stack.push(p.set_gamma_mutation(2.0), &mut p);
        stack.push(p.set_brightness_mutation(0.4), &mut p);
        stack.push(p.set_contrast_mutation(1.2), &mut p);

        while stack.undo(&mut p).is_some() {}
        let after = serde_json::to_value(&p).unwrap();
        assert_eq!(before, after);
    }
}
