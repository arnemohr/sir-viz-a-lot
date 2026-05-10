# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`rmap` is a single-machine, single-projector projection-mapping tool (Rust + wgpu + egui) targeted at small live shows. v1 is **macOS-only** by design (objc2 family for display-sleep prevention, App Nap suppression, monitor names). The product direction in `specs/roadmap.md` deliberately constrains scope: photo-driven scene composition over generic media-server breadth.

A spec-driven workflow is in active use. Numbered task IDs in commit messages (`T-MN-YY`, `003-T1.16`) refer to entries in `specs/00X-tasks*.md`. When picking up work, read the relevant `specs/` doc first — it usually documents the *why* and the acceptance criteria, not just the diff.

## Subdirectory guides — load-bearing

Nested `CLAUDE.md` files live next to the code they govern; Claude Code auto-loads them when working in those areas. **Read them before editing — they cover silent-corruption traps that the type system can't catch.**

- **`src/project/CLAUDE.md`** — scene snapshot invariants (`restore_scene` ≠ `restore`, `snapshots_share_layer_topology` gating); v3 `Mutation` Reverse-storage rules (whole-enum, effects-vec, snapshot); the `Command` vs `Mutation` separation.
- **`src/render/CLAUDE.md`** — GPU bring-up split (device before surface), per-frame render-graph order, surface-acquisition outcome mapping, the `panic_restore` frame wrapper, build-time WGSL validation.

## Commands

Use the Makefile (it also documents itself via `make help`):

- `make setup` — `mise install` + wires `core.hooksPath` to `.githooks/`. Run once per checkout.
- `make build` / `make build-release` / `make build-show` — debug, release, and the `release-show` profile (LTO=fat, codegen-units=1, panic=abort, stripped) for live use.
- `make test` — runs **`cargo nextest run`** (parallel, terser output than `cargo test`). Doctests are not run by nextest; `make ci` covers them.
- `make test-cargo` — vanilla `cargo test` fallback (use if nextest is unavailable, or for ad-hoc doctest runs).
- `make test-gpu` — `cargo nextest run --features gpu-tests`; needs a working wgpu adapter (golden images live in `tests/golden/`).
- `make lint` — `cargo clippy --all-targets --all-features -- -D warnings`.
- `make ci` — `fmt-check lint test` plus `cargo test --doc`.
- `make bundle` — macOS `.app` via `cargo bundle --profile release-show`.

Single-test invocations (nextest filter syntax differs slightly from `cargo test`):

```bash
cargo nextest run -E 'test(/recall_preserves_other_slots/)'           # one test by name
cargo nextest run --test cli_smoke list_monitors_exits_zero            # one integration test
cargo nextest run --features gpu-tests --test headless_gpu             # GPU golden tests
UPDATE_GOLDEN=1 cargo nextest run --features gpu-tests                 # rewrite goldens instead of asserting
```

Toolchain + cargo subcommands (cargo-watch, cargo-bundle, cargo-nextest) are pinned in `mise.toml` (`mise install`). There is no `rust-toolchain.toml` — intentionally gitignored to prevent drift. v1 work assumed Rust 1.85 (per `Cargo.toml`); mise pins 1.92 for the developer environment. nextest is reachable via the mise shim at `~/.local/share/mise/shims/cargo-nextest` (auto-on-PATH when `mise activate` is in your shell rc).

`build.rs` runs naga WGSL parse + validation over every shader in `src/render/shaders/` at compile time, so a broken shader fails `cargo build` instead of crashing at startup. Editing a `.wgsl` triggers a rebuild via `cargo:rerun-if-changed`.

### Pre-commit hook

`.githooks/pre-commit` runs on every `git commit` once `make setup` has wired the hook path. It does the bare minimum to keep the inner loop fast:

- **rustfmt** scoped to staged `.rs` files only (pre-existing drift in unrelated files won't block your commit).
- **`cargo check --workspace --all-targets`** (not clippy) — catches type errors and missing imports without the full lint set.

Heavier checks (clippy `--all-features`, the full test suite, doctests) live in `make ci` and run on push / before merge. Bypass once with `git commit --no-verify` if needed; if you find yourself bypassing routinely, the hook is wrong — fix it.

### Dev profile

`Cargo.toml` sets `[profile.dev.package."*"] opt-level = 1` so dependencies (wgpu, egui, resvg, …) compile with optimization in dev builds while workspace code stays at `-O0` for fast incremental compiles + debuggable symbols. The first cold build is slower; every subsequent build is the same speed but the runtime is dramatically faster (wgpu in unoptimized debug runs at single-digit fps; with this profile it hits vsync). This is the standard wgpu/Bevy-community workaround.

## Cargo features

- `gpu-tests` — opts in the headless-wgpu golden-image harness (`tests/headless_gpu.rs`). Off by default so `cargo test` stays CPU-only.
- `audio` (`cpal` + `rustfft`), `midi` (`midir`), `osc` (`rosc`) — M7 input sources. Each is gated so the show-day binary stays lean. Do not promote to default.
- `v3` — Spec 003 UI/UX overhaul (state machine, command/mutation pattern, undo, launcher, project audit). Currently behind the flag while v3 ships incrementally; planned to flip to default at M3.

## Architecture (big picture)

`src/main.rs` is intentionally thin (CLI + `tracing` setup → `App::run`). The interesting wiring lives in **`src/app.rs`** (~2k lines) — read it before making structural changes.

### State machine (`AppState` in `src/app.rs`)

`Booting → Editing` is the only path that exists today; the other variants are scaffolded for Spec 003:

- `Booting` — pre-`resumed`; CLI parsed, monitors not yet known.
- `Launcher(LauncherState)` — first-run window flow (T-003-T2.\*; not yet populated).
- `Editing(EditingState)` — control + output windows live; the bulk of runtime state hangs off `EditingState`.
- `GoLive(EditingState)` — same payload as `Editing`, fullscreen on the projector (T-003-T4.16/17).
- `Failed(FailureKind)` — project-load / audit-critical / render-init failures (T-003-T1.44).

Per-state `ControlFlow`: `Editing`/`GoLive` use `Poll` (vsync-driven redraws); `Launcher`/`Failed` use `Wait` (idle, battery-friendly). macOS may fire `resumed` more than once; `AppState::is_running` guards the re-init path.

**For GPU lifecycle and the per-frame render graph, see `src/render/CLAUDE.md`.**

**For scene snapshot invariants and v3 Mutation Reverse-storage rules, see `src/project/CLAUDE.md`.** Both are auto-loaded when you work in those directories.

### Live input

`controls::Source` trait is polled per frame from `InputState`. Today: keyboard (always on); MIDI/OSC behind their cargo features. `apply_command` (in `src/app.rs`) returns a `SideEffect` because `EditingState` is mutably borrowed during dispatch — render-graph mutations (e.g. `RebuildLayers` after a scene snap) must happen *outside* the borrow chain.

### Show-day reliability (`src/show_day/`)

Everything in this module exists because of bad live experiences, not feature parity:

- `panic_restore::run_frame_assert_unwind_safe` — wraps every render frame; converts panics into `RenderError::RenderPanic` rather than unwinding the event loop.
- `sleep_assertion::SleepAssertion` — IOPMAssertion preventing display sleep on macOS; verify with `pmset -g assertions` per `docs/show-day-checklist.md`.
- `release-show` profile (`panic=abort`, LTO, strip) for live use; `catch_unwind` still gives in-frame recovery before any abort path.

Logs land in `~/Library/Logs/rmap/rmap.log` (daily rolling); `RUST_LOG` overrides the default filter (`mise.toml` sets a developer-friendly `rmap=debug,wgpu=warn,naga=warn,winit=info`).

## Dependency constraints

- `egui` / `egui-wgpu` / `egui-winit` / `wgpu` / `winit` are a **tightly coupled version set**. Bump them in lockstep and verify against `https://github.com/emilk/egui#integrations` before pinning new majors.
- `objc2` / `objc2-foundation` / `objc2-app-kit` / `objc2-io-kit` likewise move together. Older `cocoa` / `objc` crates are deprecated — do not reintroduce.
- No tokio. Async wgpu calls are driven through `pollster::block_on`; this is intentional.

## Gitignore footguns

- `*.rmap.json` is ignored — user shows do not belong in the repo. If you need a fixture for tests, put it under `tests/` or `assets/` with a non-`.rmap.json` extension.
- A literal `~/` directory is also ignored: the save-path field doesn't shell-expand, so an operator typing `~/foo` creates `./~/foo`.

## Spec-driven workflow

When implementing a numbered task:

1. Read the matching spec section (e.g. `specs/003-tasks-phase-1.md` for `T-003-T1.*`).
2. Match the commit-message style: `003-T1.17: proptest harness for Mutation Reverse-storage round-trip`.
3. Acceptance criteria in the spec are part of the task — verify them, don't just "write the code that fits the title".
