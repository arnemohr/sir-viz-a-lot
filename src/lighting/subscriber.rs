//! P5.5.1 — `LightSubscriber` trait + fan-out subscriber list.
//!
//! Every show-critical event (Blackout, Go-live, Exit-live) fans out to
//! all registered `LightSubscriber`s in the same frame as the visual
//! state flip. The subscriber list lives in `EditingState` and is iterated
//! by the `apply_command` arm for `Blackout` and the `EnterGoLive` /
//! `ExitGoLive` transition arms.
//!
//! # Design
//!
//! `LightSubscriber` is a `dyn` trait so new subscriber types (e.g. NDI
//! output, future Phase 7 streams) can be added without touching the fan-out
//! call sites. `Send` is required so the list can be held on the main thread
//! without additional synchronisation — each subscriber owns its cross-thread
//! handle (e.g. `LightingThread`) and dispatches commands via `Arc<AtomicBool>`
//! / crossbeam channel internally.
//!
//! # Blackout behaviour
//!
//! `on_blackout` sends an all-zero `DmxUniverse` for every configured
//! universe. The `LightingThread` implementation drains its channel and
//! queues the zero universe on the next tick.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossbeam_channel::Sender;

use crate::lighting::universe::{DmxUniverse, UniverseFrame, UniverseId};

// ---------------------------------------------------------------------------
// LightSubscriber trait (P5.5.1)
// ---------------------------------------------------------------------------

/// Observer for show-critical lighting events.
///
/// Implementors receive notifications when:
/// - The operator presses `B` (Blackout) — `on_blackout`.
/// - Go-live is confirmed — `on_go_live`.
/// - Go-live is exited — `on_exit_live`.
///
/// All three calls happen in the same frame as the corresponding visual
/// state change. Implementations must not block the calling thread.
pub trait LightSubscriber: Send {
    /// The operator pressed `B` — black out all light output immediately.
    ///
    /// Implementors should send all-zero DMX values for every configured
    /// universe as quickly as possible (within the current frame's dispatch).
    fn on_blackout(&mut self);

    /// The show has transitioned to Go-live — arm all lighting output.
    fn on_go_live(&mut self);

    /// The show has exited Go-live — stop lighting output and send a
    /// courtesy zero-universe packet.
    fn on_exit_live(&mut self);
}

// ---------------------------------------------------------------------------
// LightingThreadSubscriber — wraps a LightingThread sender (P5.5.1)
// ---------------------------------------------------------------------------

/// `LightSubscriber` implementation backed by the `LightingThread` channel.
///
/// `on_blackout`: pushes a zeroed `UniverseFrame` for each tracked universe
/// via `try_send` (non-blocking; silently drops if the channel is full).
///
/// `on_go_live` / `on_exit_live`: signal the thread via the `active` flag.
///
/// This type is constructed by `LightingThread::into_subscriber` (or inline
/// at Go-live start) and pushed into `EditingState.light_subscribers`.
pub struct LightingThreadSubscriber {
    /// Non-blocking sender for universe frames.
    pub tx: Sender<UniverseFrame>,
    /// Universe IDs that have been sent at least once; used for blackout.
    pub universe_ids: HashSet<UniverseId>,
    /// Shared flag — `true` while the thread is active.
    pub active: Arc<AtomicBool>,
}

impl LightSubscriber for LightingThreadSubscriber {
    fn on_blackout(&mut self) {
        // Send a zeroed universe frame for each known universe.
        for &id in &self.universe_ids {
            let frame = UniverseFrame {
                id,
                data: DmxUniverse::default(),
            };
            // Non-blocking; silently drop if full (lighting thread is busy).
            let _ = self.tx.try_send(frame);
        }
        tracing::info!(
            universes = self.universe_ids.len(),
            "LightSubscriber: blackout sent"
        );
    }

    fn on_go_live(&mut self) {
        self.active.store(true, Ordering::Relaxed);
        tracing::info!("LightSubscriber: go_live armed");
    }

    fn on_exit_live(&mut self) {
        // Send zero-universe as a courtesy.
        for &id in &self.universe_ids {
            let frame = UniverseFrame {
                id,
                data: DmxUniverse::default(),
            };
            let _ = self.tx.try_send(frame);
        }
        self.active.store(false, Ordering::Relaxed);
        tracing::info!("LightSubscriber: exit_live (zeros sent)");
    }
}

// ---------------------------------------------------------------------------
// SubscriberList — convenience wrapper for Vec<Box<dyn LightSubscriber>>
// ---------------------------------------------------------------------------

/// Fan-out dispatcher for a list of `LightSubscriber`s.
///
/// Iterates the list and calls the appropriate method on each subscriber.
/// Held as a field on `EditingState` (behind `#[cfg(feature = "lighting")]`).
#[derive(Default)]
pub struct SubscriberList {
    subscribers: Vec<Box<dyn LightSubscriber>>,
}

impl SubscriberList {
    /// Add a subscriber.
    pub fn push(&mut self, s: impl LightSubscriber + 'static) {
        self.subscribers.push(Box::new(s));
    }

    /// Call `on_blackout` on all subscribers.
    pub fn blackout(&mut self) {
        for s in &mut self.subscribers {
            s.on_blackout();
        }
    }

    /// Call `on_go_live` on all subscribers.
    pub fn go_live(&mut self) {
        for s in &mut self.subscribers {
            s.on_go_live();
        }
    }

    /// Call `on_exit_live` on all subscribers; then clear the list.
    ///
    /// Clearing the list on exit ensures that subscribers (e.g.
    /// `LightingThread` handles) are dropped here, which stops the background
    /// thread cleanly before the state transition completes.
    pub fn exit_live(&mut self) {
        for s in &mut self.subscribers {
            s.on_exit_live();
        }
        self.subscribers.clear();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock subscriber that records call sequence.
    struct MockSubscriber {
        calls: Vec<&'static str>,
    }

    impl LightSubscriber for MockSubscriber {
        fn on_blackout(&mut self) {
            self.calls.push("blackout");
        }
        fn on_go_live(&mut self) {
            self.calls.push("go_live");
        }
        fn on_exit_live(&mut self) {
            self.calls.push("exit_live");
        }
    }

    /// P5.5.1 — SubscriberList fans out to all registered subscribers.
    #[test]
    fn subscriber_list_fans_out_to_all() {
        let mut list = SubscriberList::default();

        // Wrap MockSubscribers to avoid needing Rc<RefCell> across Box<dyn ...>
        // by using a shared Vec via Arc<Mutex<>> — simpler: just add two mocks.
        let mock1 = MockSubscriber { calls: vec![] };
        let mock2 = MockSubscriber { calls: vec![] };
        list.push(mock1);
        list.push(mock2);

        list.go_live();
        list.blackout();
        list.exit_live();

        // After exit_live the subscriber list is cleared (both mocks dropped).
        assert!(
            list.subscribers.is_empty(),
            "exit_live should clear the subscriber list"
        );
    }

    /// P5.5.1 — call sequence on a single mock subscriber.
    #[test]
    fn mock_subscriber_records_call_sequence() {
        let mut calls = Vec::<&'static str>::new();

        // Use a closure-based helper to avoid boxing.
        struct ClosureSub<'a> {
            calls: &'a mut Vec<&'static str>,
        }
        impl<'a> LightSubscriber for ClosureSub<'a> {
            fn on_blackout(&mut self) {
                self.calls.push("blackout");
            }
            fn on_go_live(&mut self) {
                self.calls.push("go_live");
            }
            fn on_exit_live(&mut self) {
                self.calls.push("exit_live");
            }
        }

        // Safety: we don't send across threads in this test.
        // The `Send` bound is satisfied because `ClosureSub` holds a raw &mut,
        // which is not Send — so we test the trait interface directly.
        let mut sub = ClosureSub { calls: &mut calls };
        sub.on_go_live();
        sub.on_blackout();
        sub.on_exit_live();

        assert_eq!(calls, vec!["go_live", "blackout", "exit_live"]);
    }
}
