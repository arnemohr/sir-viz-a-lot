//! 003-T1.6 — CLI smoke tests covering paths that bypass the main
//! AppState boot sequence.
//!
//! Scope: `--list-monitors` is a non-GUI short-circuit; it brings up
//! a winit `EventLoop` long enough to enumerate monitors, prints to
//! stdout, and exits 0. That makes it cheap to validate in CI and
//! useful as a regression guard for `App::print_monitors` after
//! T-003-T1.* refactors.
//!
//! `--autostart` paths (success and failure) are *not* covered by
//! shell-harness tests because they require a display server +
//! wgpu adapter; those paths are exercised by the App-level unit
//! tests in `src/app.rs::tests` (e.g. `project_load_failure_preserves_reason`)
//! and by manual smoke runs documented in T-003-T1.6 acceptance.

use std::process::Command;
use std::time::Duration;

const BIN: &str = env!("CARGO_BIN_EXE_rmap");

/// 003-T1.6 acceptance #1: `--list-monitors` exits 0 and prints
/// the expected format.
///
/// The exact monitor list depends on the host, so we assert only:
/// - exit status 0,
/// - stdout starts with the documented header,
/// - stdout is non-empty.
#[test]
fn list_monitors_exits_zero_with_header() {
    let output = Command::new(BIN)
        .arg("--list-monitors")
        .output()
        .expect("spawning rmap binary failed");

    assert!(
        output.status.success(),
        "--list-monitors should exit 0; got {:?} (stderr: {})",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );

    // Tracing's default fmt layer writes to the same stream as
    // stdout in some tracing-subscriber versions, so the captured
    // stdout may include a `logging initialized` line before the
    // documented header. Use `contains` rather than `starts_with`
    // and verify the monitor enumeration header is present.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Monitors detected by winit"),
        "stdout should contain the documented header; got: {stdout}",
    );
    assert!(!stdout.is_empty(), "stdout must be non-empty");
}

/// 003-T1.6 acceptance #1b: `--list-monitors` returns within a
/// reasonable wall-clock budget. winit's enumeration is fast; if
/// the future T-003-T1.* refactor ever blocks on something
/// expensive, this catches it.
#[test]
fn list_monitors_returns_promptly() {
    let start = std::time::Instant::now();
    let output = Command::new(BIN)
        .arg("--list-monitors")
        .output()
        .expect("spawning rmap binary failed");
    let elapsed = start.elapsed();

    assert!(output.status.success());
    assert!(
        elapsed < Duration::from_secs(10),
        "--list-monitors took {elapsed:?}; expected < 10s",
    );
}
