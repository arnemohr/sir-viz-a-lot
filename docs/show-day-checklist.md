# Show-day operator checklist

Use this top-to-bottom before doors; aim for under five minutes once you know your machine.

## macOS — fewer surprises during the set

1. **Do Not Disturb / Focus** — Turn on so notifications do not flash over the output or steal focus.
2. **Hot Corners & gestures** — Disable Hot Corners; reduce or disable Mission Control / App Expose gestures so accidental trackpad swipes do not rearrange spaces mid-show.
3. **Energy / sleep** — On the machine driving the projector, disable or lengthen **display sleep** for the session. `rmap` requests a display-sleep assertion on macOS; still verify below.
4. **Lock screen** — Decide policy for long dinners/ceremonies (disable auto-lock vs. accepting a brief blackout if the operator steps away).

## Hardware path

5. **Projector** — Correct input selected; firmware/menu familiarity (blanking, eco modes).
6. **Video chain** — Prefer a known-good USB-C / HDMI adapter and cable length; plug order: laptop awake → then projector input if fussy.

## Verify display-sleep prevention (macOS)

7. With `rmap` running and the output window up, in Terminal:

   ```bash
   pmset -g assertions
   ```

   Look for an assertion that indicates **display sleep is prevented** (wording varies by OS version). If nothing holds the display, treat sleep policy as **not** guaranteed by software alone—fall back to System Settings.

## Application

8. **Monitor matching** — rmap matches the saved projector display by UUID
   automatically; on the same physical projector, no `--monitor` flag is
   needed. Use `rmap --list-monitors` to inspect available displays, and
   `rmap --monitor N` to override — useful for first-time setup or when
   migrating a `.rmap.json` between machines where the UUID doesn't match.
9. **Output thumbnail** — top-right of the control window shows a live
   thumbnail of the projector output. Click it to bring the preview window
   forward.
10. **Autostart** — `rmap path/to/show.rmap.json --autostart` loads the file and uses saved monitor selection unless `--monitor` overrides.
11. **Project file** — Keep a backup `.rmap.json`; save again from the UI **Project file** panel after edits.
12. **Test-grid alignment check** — open the test-grid demo from the launcher to verify warp before your content loads. The grid lines make keystone and corner errors obvious.

## During show

13. **Blackout / freeze** — Know your keys (e.g. blackout / freeze / test pattern) and rehearse once on the actual rig.
14. **Hot reload** — SVG saves and JPG/PNG saves on disk refresh layers that watch those paths; avoid editing paths mid-cue without a fallback cue.
15. **Layer mute / solo** — Use the **M** button on any layer row to drop that
    layer from the output silently mid-cue. The **S** button solos a single
    layer (one at a time, project-scoped). Both are useful for subbing content
    in or out before committing to a scene save.
16. **BPM HUD** — If your show uses tempo-driven cues, verify the BPM HUD (top
    of the control window) shows the expected tempo. Tap Space a few times; the
    readout should converge within ~3 taps. If your tap source is MIDI Note 60
    (C4) or OSC `/rmap/tap`, confirm the source label updates to "MIDI" or
    "OSC" accordingly. Set the quantize selector to lock cue firing to a bar
    boundary.

## Live editor (v3)

17. **Drop targets** — Drag SVG / PNG / JPG files onto the **control window's left rail** to add a layer. Dropping on the projector window does nothing (intentional: the drop target is unambiguous).
18. **Layer manipulation** — Click a layer thumbnail in the left rail to select it; drag its warp corners directly on the canvas to translate/scale. Solo (S) or mute (M) from the layer row as needed.
19. **Mask zones** — Pick a starter shape from the **inspector panel's mask section** (window-rectangle, arch-portal, circle-spotlight, void-block); drag each vertex of the painted overlay onto the real venue feature. Double-click an edge to insert a vertex; Shift-click a vertex to delete (won't drop below 3).

## Two-projector setup (v0.4)

20. **Monitor identification** — In the launcher's output picker, click
    **Identify** next to each output; a full-screen flash confirms which
    physical display is which before you assign it.
21. **Display-sleep assertions** — With both output windows active, run:

    ```bash
    pmset -g assertions
    ```

    Confirm there is one sleep-prevention assertion per display (two total).
    If either is missing, verify the output window for that display is fully
    initialised before go-live.
22. **Edge-blend calibration** — Enable the **edge-blend gradient test
    pattern** from the show-day strip. Physically inspect the overlap on the
    wall: the gradient should fall off smoothly and the summed brightness
    across the seam should look even (approximately 1.0 combined). Adjust
    the overlap width in the OutputPanel until the seam is invisible at show
    distance.

## MIDI controller (v0.4)

23. **Controller visibility** — Open the binding picker (right-click any
    parameter row) and confirm your MIDI controller appears in the source
    list. If it doesn't, check that the controller is plugged in and
    recognised by macOS before launching rmap.
24. **MIDI-learn smoke** — Right-click a parameter, choose **Learn next MIDI
    CC**, send a CC from the controller; confirm the binding appears and the
    parameter responds. This should be done before the show — not during it.

## OSC sender (v0.4)

25. **Binding summary** — Open the Advanced panel's OSC section. Confirm
    every expected OSC address appears in the read-only bindings summary.
    Send a test value from your patch; verify the bound parameter moves.
    Check this before going to go-live.

## FX layer presets (v0.4)

26. **Preset render check** — If the show includes an FX layer using the
    `mask_edge_ripple_wash` preset, confirm it renders against the correct
    mask polygon before doors. A misconfigured or empty mask polygon produces
    a blank layer; fix the mask in the inspector and save before go-live.

## Treatment presets + video grammar (v0.5)

27. **Identity-at-defaults sanity** — for any layer carrying a treatment,
    confirm the preset's default-parameter pass is visually transparent
    (drop the slider to its leftmost / default value and verify no change
    against the source). All six v0.5 presets are bit-exact identity at
    default; if you see a shift, the project file may carry hand-edited
    params from an older draft — reset to default before doors.

28. **External assets exist** — for `texture_overlay` and `collage` layers,
    confirm each `overlay_path` / `collage_paths` entry points at a file
    that's actually on disk in the show machine's filesystem. Missing
    assets log a `treatment_overlay_load_failed` / `treatment_collage_load_failed`
    warn and fall back to source for the affected slot — operator gets
    no visible toast, so check the trace before doors.

29. **Video trim sanity** — if any Video layer uses `clip_in` / `clip_out`,
    scrub through the trim range in Advanced and confirm playback honours
    it. The decoder's `timeRange` is set on each reader rebuild
    (next-EOF or pause+play); a stale trim set mid-set won't take effect
    until then.

30. **BPM-lock test** — if any Video layer has BPM-lock on, tap a tempo
    in the top chrome and confirm the layer's playback rate scales with
    it. 120 BPM = identity; halving the BPM should halve the rate. If
    the rate doesn't change, the `pending_video_controls` channel may
    be dropping `SetSpeed` messages — restart the layer.

31. **Loop-mode glyph audit** — the left-rail Video row shows a glyph
    (∞ / → / ⇆) overlaid on the thumbnail. Confirm each video layer's
    glyph matches the operator's intent — `→` (Once) is a common
    surprise on show night because the playhead pauses on the last
    frame after the clip.

## FX preset library + particles (v0.6)

32. **Particle budget** — If using particle presets (`mask_constrained_drift`,
    `mask_edge_emission`, `mask_field_flow`, `mask_collision_reflection`),
    verify each FxLayer's particle count is within budget. No inline warning
    should be visible in the control panel; a warning means the mutation was
    refused and the slider snapped back — reset the value before going live.

33. **Preset library audit** — Confirm all FX presets load without
    `UnknownFxPreset` audit warnings in the diagnostics strip (Advanced →
    Diagnostics). An unknown-preset finding means the project file references
    a `preset_id` not in the registry; re-link or remove the affected layer
    before doors.

34. **Effect-chain order** — Confirm the effect-chain order on each layer
    matches your saved scene (Advanced → Selected layer → Effect chain). Check
    that the undo stack is clear before going live — a non-empty stack means
    unsaved edits are in flight.

## Frame-budget perf gate (P0.9.5)

### How to run

```bash
# Full gpu-tests suite (includes golden-image tests):
make test-gpu

# Just the perf gate:
cargo nextest run --features gpu-tests -E 'test(/perf_frame_budget/)'
```

The test renders 600 frames (10 seconds at 60 Hz) through the production render
graph against headless wgpu targets. No window or projector is required.

### What the printed output means

```
=== P0.9.5 Frame-Budget Gate Results ===
  Frames rendered:  600          — total frames in the run
  Min frame time:   X.XX ms     — fastest single frame
  p50 frame time:   X.XX ms     — median frame time (typical case)
  p99 frame time:   X.XX ms     — 99th-percentile frame time (worst 1%)
  Max frame time:   X.XX ms     — single worst frame (often cold-start)
  Texture drops:    0            — upload-queue overflows (must be 0)
  Panic count:      0            — frame panics caught by catch_unwind (must be 0)
```

### CI assertion vs. show-day target

| Threshold | Value | Purpose |
|-----------|-------|---------|
| **CI gate** | p99 < 100 ms | Regression guard; passes on any CI runner with a GPU adapter |
| **Show-day acceptance** | p99 ≤ 16.6 ms | Full 60 Hz budget; verify on the actual projector machine |

### Recording your show-day baseline

On the first run on your actual show hardware, note the p99 value here:

> **Show-day baseline (your machine):** ___ ms  _(record date + hardware)_

If p99 on show hardware exceeds 16.6 ms, investigate before go-live:
- Check `RUST_LOG=rmap=debug` for any per-frame warnings.
- Verify the GPU is not throttled (power settings, thermal state).
- Try `make build-show` (release-show profile with LTO + strip) for the
  live binary — debug builds are deliberately slower.

### Fixture note (v0.4)

The fixture uses 4 FxLayer (ripple wash) layers + edge-blend across 2 simulated
outputs. It substitutes for the spec's "4 video layers + 1 NDI input" because:
- No fixture mp4 was available at P0.4.2 time (no `ffmpeg` to encode one).
- NDI is deferred to v0.5.

When a fixture mp4 and NDI land, the fixture in `tests/perf_frame_budget.rs` can
be grown to match the original spec. The threshold strategy and harness shape stay
the same.

---

## External dependencies (v0.4)

No Homebrew packages or external system libraries are required beyond the
toolchain managed by `mise install`. Specifically:

- **Video** — the decoder will use AVFoundation (ships with macOS); no
  Homebrew install is needed. Video integration tracking in P0.4.2.
- **NDI** — deferred to v0.5; the NewTek/Vizrt NDI SDK will be required
  when that integration lands.

Run `mise install` once per checkout (`make setup`) to ensure the pinned
Rust toolchain and cargo subcommands (nextest, cargo-bundle) are available.
