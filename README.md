# rmap

rmap is a single-machine projection-mapping tool for small live shows. Load a
still image or SVG, drag the warp corners onto your wall or screen, dial in a
mask polygon to hide the edges, and save the whole setup as a project file you
can reload at the next event. The launcher opens a bundled demo so you can
explore the canvas immediately — no command-line flags required.

<!-- TODO: screenshot of launcher -->

<!-- TODO: screenshot of canvas with photo layer and warp handles -->

<!-- TODO: screenshot of show-day strip -->

## Quick start

1. Build and run:

   ```bash
   cargo run --release
   ```

2. The launcher window opens. Click **Try a demo** to load the bundled sample
   project and explore the canvas.

3. To use your own content, click **Open a recent show** (if you have one) or
   drag a JPG, PNG, or SVG onto the canvas once a project is open.

4. Adjust warp corners directly on the canvas, mask out surfaces you don't
   want lit, and save with **Save** in the toolbar.

## Docs

- [Show-day operator checklist](docs/show-day-checklist.md) — macOS-focused
  pre-show steps, cables, and verifying display-sleep prevention.
- [Keyboard accelerators](specs/keyboard-accelerators.md) — every key binding
  with its source location.
- [Capability scope](specs/v3-capability-scope.md) — what v3 ships, what v3.1
  catches, and what v0.4 will own.

## Tests

```bash
make test
```

GPU golden tests (optional feature, requires a working wgpu adapter):

```bash
make test-gpu
```

Full CI (fmt + clippy + tests + doctests):

```bash
make ci
```

---

## Power users

### CLI flags

```bash
cargo run --release -- --help
```

- **`*.rmap.json`** — full project (layers, warp, scenes, gamma,
  `output_monitor_index`, optional `output_windowed`).
- **`*.svg`** — bootstrap one layer; warp defaults are added automatically.
- **`--monitor INDEX`** — output monitor (overrides the value saved in the
  project file). Use `--list-monitors` to print indices.
- **`--windowed`** / **`--fullscreen`** — windowed draws a 1280×720 decorated
  window on the chosen monitor; fullscreen is the default and can be forced to
  override a saved `output_windowed` flag. The two flags are mutually
  exclusive.
- **`--autostart`** — with a `.rmap.json` argument, logs startup intent and
  uses the loaded project's monitor index when `--monitor` is omitted (no
  extra click gate in this build).

### Cargo features

- `v3` — Spec 003 UI/UX overhaul (state machine, command/mutation pattern,
  undo, launcher, project audit). Currently behind the flag while v3 ships
  incrementally; planned to flip to default at M3.
- `gpu-tests` — headless wgpu golden-image harness. Off by default.
- `audio`, `midi`, `osc` — live input sources. Do not promote to default.

### Build profiles

```bash
make build          # debug
make build-release  # release
make build-show     # release-show (LTO, panic=abort, stripped) — for live use
make bundle         # macOS .app via cargo-bundle
```

Logs land in `~/Library/Logs/rmap/rmap.log` (daily rolling); override with
`RUST_LOG`.
