# rmap

Minimal projection-mapping tool: single machine, SVG layers, compositor, warp, and master gamma — geared toward small-show operation.

## Run

```bash
cargo run --release -- --help
```

- **`*.rmap.json`** — full project (layers, warp, scenes, gamma, `output_monitor_index`, optional `output_windowed`).
- **`*.svg`** — bootstrap one layer; warp defaults are added automatically.
- **`--monitor INDEX`** — output monitor (overrides the value saved in the project file). Use `--list-monitors` to print indices.
- **`--windowed`** / **`--fullscreen`** — windowed draws a 1280×720 decorated window on the chosen monitor; fullscreen is the default and can be forced to override a saved `output_windowed` flag. The two flags are mutually exclusive.
- **`--autostart`** — with a `.rmap.json` argument, logs startup intent and uses the loaded project’s monitor index when `--monitor` is omitted (no extra click gate in this build).

Save from the control window under **Project file** (collapsible panel).

## Docs

- [Show-day operator checklist](docs/show-day-checklist.md) — macOS-focused pre-show steps, cables, and verifying display-sleep prevention.

## Tests

```bash
cargo test
```

GPU golden tests (optional feature):

```bash
cargo test --features gpu-tests
```
