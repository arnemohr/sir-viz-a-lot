# T-M1.5-01 — Venue dry-run report

**Date**: 2026-05-07
**Verdict**: **partial pass — proceed to M2 with venue-environment checks deferred**
**Hardware**: dev laptop (macOS, Apple Silicon, built-in Retina 3024×1964 @ scale 2.0) + one external display reachable as monitor index 1 (1920×1080 @ (-760, -1080) scale 1.0).

## Spec checklist

| Check | Status | Notes |
|---|---|---|
| Borderless fullscreen lands on chosen projector | **pass** | `cargo run -- --monitor 1` launched the M1 hello-rectangle on the external display; gradient visible; cursor hidden. |
| Esc closes cleanly | **pass** | Confirmed by operator. Desktop restored without artefacts; binary exits 0. |
| No display sleep within ½ hour | **deferred** | Will be re-verified at the actual venue; M2's sleep-prevention work (`T-M2-04`, `IOPMAssertion` via `objc2-io-kit`) hardens this path. |
| No Mission Control glitches | **deferred** | Same — re-verified at venue; pre-show checklist (`T-M6-07`) instructs operator to disable Hot Corners + Mission Control gestures. |
| No flicker on extended-display reconfiguration | **deferred** | Will be exercised by `T-M1-05`'s surface lost/outdated/suboptimal recovery (already shipped) when the projector is unplugged/replugged at the venue. |

## What this gate established

The macOS "displays have separate Spaces" risk noted in `specs/001-initial-setup.md` (risk notes) does **not** materialize against winit 0.30 + wgpu 29 — the borderless fullscreen window correctly lands on the addressed monitor on the first try. This was the single biggest M1 schedule risk; it is closed.

## What this gate did NOT establish

The 30-minute display-sleep, Mission Control, and unplug/replug checks belong to a real venue setting. They will be re-run as part of pre-show preparation. M2 work specifically hardens the codebase against the failure modes those checks would catch:

- `T-M2-04` — `IOPMAssertion` so the projector display does not sleep mid-show.
- `T-M2-02 + T-M2-03` — `panic_restore` around the per-frame render so a malformed surface event cannot take the show down.
- `T-M6-07` — `docs/show-day-checklist.md` operator-facing pre-show ritual covering the macOS booby traps (DnD, Hot Corners, Energy Saver).

## Decision

**M2 unblocked.** The environmental checks above will be re-verified at the venue when M2 + M6 deliverables are available.

## Reference

- `specs/001-initial-setup-plan.md` §3.4 M1.5
- `specs/001-tasks.md` T-M1.5-01
- M1 commits: `957c3f7`..`047a8d8`.
