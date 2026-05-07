//! Show-day reliability: blackout/freeze state, panic restore, display-sleep
//! prevention. Everything in this module exists because of bad live
//! experiences, not feature parity with anything.

pub mod panic_restore;
pub mod sleep_assertion;
