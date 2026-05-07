//! `Param<T>` — a parameter that can be a static value, a modulator-
//! driven value, or (v1.5+) bound to an external source like MIDI
//! or OSC. Spec §3.2 input/control extension point.
//!
//! For v1 only `Static` and `Modulated` resolve to a value;
//! `Bound` is reserved as an enum slot and returns `0.0` so the v1
//! codebase compiles ahead of the v1.5 source-binding work.

use crate::clock::Clock;
use crate::controls::InputState;
use crate::modulators::Modulator;

/// Opaque handle into [`InputState`]'s source registry. v1.5 work
/// (T-M7-05 / T-M7-06) will populate the registry with MIDI / OSC
/// sources; for v1 the type exists so `Param::Bound` can carry a
/// stable reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceRef(pub u64);

/// A numeric parameter. T = f32 today; the type parameter is kept
/// so future categorical params (e.g. blend mode) can use the same
/// shape.
#[derive(Debug, Clone)]
pub enum Param<T: Copy> {
    Static(T),
    Modulated(Modulator),
    Bound(SourceRef),
}

impl<T: Copy + Default> Default for Param<T> {
    fn default() -> Self {
        Self::Static(T::default())
    }
}

impl Param<f32> {
    /// Resolve to a concrete `f32`. `inputs` is needed by the `Bound`
    /// arm in v1.5; v1 ignores it.
    pub fn value(&self, clock: &Clock, inputs: &InputState) -> f32 {
        match self {
            Self::Static(v) => *v,
            Self::Modulated(m) => m.value(clock),
            Self::Bound(_source_ref) => {
                // v1.5: look up the source in `inputs` and read its
                // current value. v1: stub returns 0.0.
                let _ = inputs;
                0.0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::Clock;
    use crate::controls::InputState;

    #[test]
    fn static_passthrough() {
        let clock = Clock::new();
        let inputs = InputState::default();
        let p = Param::Static(5.0_f32);
        assert!((p.value(&clock, &inputs) - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn modulated_dispatches_to_modulator() {
        let clock = Clock::new();
        let inputs = InputState::default();
        let p = Param::Modulated(Modulator::Static(3.5));
        assert!((p.value(&clock, &inputs) - 3.5).abs() < f32::EPSILON);
    }

    #[test]
    fn bound_returns_zero_v1() {
        let clock = Clock::new();
        let inputs = InputState::default();
        let p: Param<f32> = Param::Bound(SourceRef(42));
        assert_eq!(p.value(&clock, &inputs), 0.0);
    }
}
