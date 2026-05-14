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
22a. **Per-projector gamma trims (v1.1)** — If the two projectors are not
    perfectly matched (one slightly brighter / cooler than the other), use
    the **Per-projector trims** sliders in each Output sub-card (Gamma /
    Brightness / Contrast) to pull them into alignment. The cascade is
    `output override > project override > project master`, so per-output
    trims override the project-level tuning for that one projector only.
    Set during setup; verify with a flat-grey test image before doors.

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
25a. **OSC parameter modulators (v1.1)** — `Modulator::OscBound` now reads
    live values from incoming OSC traffic. Send a known value to a bound
    address and confirm the slider moves before the show. If the slider
    stays at zero, the OSC datagram isn't reaching rmap (firewall? wrong
    port?) — debug with the macOS `nc -u -l <port>` listener.

## Audio feature (v1.1)

25b. **Audio feature compiled in?** — If your show binds any parameter to
    `Modulator::Audio { band, .. }`, ensure rmap was built with
    `cargo build --features audio` (CPAL is opt-in). The launcher emits a
    one-shot warn-toast at project load if audio modulators exist but the
    feature is compiled out; if you see that toast, rebuild with the
    feature before doors.

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

---

## Phase 2 acceptance smoke test

Run once against the v0.6 release-candidate build; record pass/fail per step
in a commit comment when the script lands. Target: under five minutes against
a debug build with the demo project.

1. **Three-click acceptance**
   - [ ] Open a fresh project (no layers).
   - [ ] Draw a polygon mask on the canvas.
   - [ ] Open Advanced → Selected layer → FX Preset browser. (click 1)
   - [ ] Select the `mask_edge_wave_wash` preset ("Mask-edge ripple wash"). (click 2)
   - [ ] Confirm the preset is running on the canvas — the animated edge wash
         is visible. (click 3)
   - **Pass:** preset running after at most three clicks from fresh project.
   - _Traces to `specs/004-phase-2.md` §Acceptance criteria, criterion 1:_
     _"An operator can drop a polygon mask, pick 'mask-edge ripple wash' from_
     _the preset library, and see it run within three clicks."_

2. **Particle budget enforcement**
   - [ ] Add a particle FxLayer and pick `mask_constrained_drift`.
   - [ ] In Advanced → FX Params, drag the `particle_count` slider past its
         declared `max_particle_count` limit.
   - [ ] Confirm: the mutation is refused, an inline warning appears in the
         control panel, the slider snaps back to the previous value, and the
         project state is unchanged (verify via undo stack depth).
   - **Pass:** mutation refuses and slider snaps back; no project-state change.
   - _Traces to `specs/004-phase-2.md` §Acceptance criteria, criterion 2:_
     _"Particle counts per layer are enforced to keep the show-day frame_
     _budget; over-budget configurations refuse to commit with an inline warning."_

3. **Scene recall preserves FxLayer state**
   - [ ] With a particle FxLayer active (e.g. `mask_constrained_drift`,
         any seed), save the current state as a scene slot (Cue strip → Save).
   - [ ] Modify the layer (change a parameter) so the state visually differs.
   - [ ] Recall the saved scene. Confirm the particles render identically to
         the saved state — same visual output, same seed.
   - **Pass:** recalled scene renders bit-identically to the saved state.
   - _Traces to `specs/004-phase-2.md` §Acceptance criteria, criterion 3:_
     _"FX layer state survives scene recall and undo (proptest harness in_
     _`src/project/` extended to cover FX layer mutations)."_

4. **Effect-chain reorder + undo**
   - [ ] Select an Image layer that has both Blur and Color effects in its
         chain.
   - [ ] In Advanced → Selected layer → Effect chain, drag the Blur effect
         above the Color effect.
   - [ ] Confirm the render order changes (Blur applies before Color).
   - [ ] Press Cmd-Z (undo). Confirm the effect order is restored to the
         pre-drag state.
   - **Pass:** drag changes order; undo restores; no crash.
   - _Traces to `specs/004-phase-2.md` §Effect-chain reordering ("resolves_
     _UX item M7")._

5. **Preset export / import**
   - [ ] Open a project with a tuned particle preset (custom parameter values).
   - [ ] Use Advanced → FX Preset → Export preset. A `.rmap-preset.json` file
         is written to disk.
   - [ ] Open a fresh project. Use Advanced → FX Preset → Import preset and
         select the exported file.
   - [ ] Confirm: the preset appears in the browser with the same `preset_id`
         and parameter values. Verify the exported file contains no media paths
         or warp data (open the JSON and inspect).
   - **Pass:** `preset_id` + params round-trip identically; file contains no
     media or warp fields.
   - _Traces to `specs/004-phase-2.md` §Acceptance criteria, criterion 4:_
     _"The preset library exports a single `.rmap-preset.json` per preset that_
     _can be shared across projects without media or warp data."_

> **Note:** run this script once against the v0.6 release-candidate build and
> record pass/fail per step in a commit comment when the script lands.

---

## Phase 3 acceptance smoke test (v0.7) — manual

Run against the v0.7 release-candidate build (`make build-release`).
Traces to `specs/004-phase-3.md` acceptance criteria.

6. **Zone palette in Mask mode**
   - [ ] Create a new FxLayer and enter Mask mode (edit mode pill → Mask).
   - [ ] Below the mask-feather slider, confirm the Zone Tag combobox is
         visible with "None" selected.
   - [ ] Draw a polygon (≥ 3 vertices). Open the Zone Tag picker; select
         "Window". Confirm the combobox updates to "Window".
   - [ ] Close Mask mode. Confirm the layer row shows a "[window]" badge in
         muted text below the layer ID.
   - [ ] Press Cmd-Z (undo). Confirm the zone role reverts to None and the
         badge disappears.
   - [ ] Press Cmd-Shift-Z (redo). Confirm it comes back.
   - [ ] Hover a role name inside the combobox; confirm the Glossary popover
         appears describing the role.
   - **Pass:** palette visible in Mask mode; selection changes role; undo/redo
     works; badge appears; glossary tooltip shown.

7. **Zone-consuming FX preset — light spill**
   - [ ] Apply "Light spill from window zones" preset to the window-tagged
         layer. Confirm the effect renders a warm glow inside the mask.
   - [ ] Apply the same preset to an untagged layer. Confirm transparent output
         (no visible effect, no crash).
   - [ ] In the preset browser, confirm "Light spill from window zones" shows
         a "— requires zone tag" supplemental label.
   - **Pass:** glow renders for window tag; transparent for None; browser label
     present.

8. **Old project backward compatibility**
   - [ ] Load a v0.6 project file (pre-zone-tags). Confirm it opens without
         errors, all layers render identically, and no zone-related audit
         findings appear (UnknownZoneRole / MissingZoneTag should NOT fire for
         non-zone-consuming presets).
   - **Pass:** project opens and renders identically; no spurious audit toasts.

9. **Glossary coverage**
   - [ ] Open the Glossary window (Help → Glossary, or from the control panel).
   - [ ] Search "window" — confirm a "Zone Role: Window" entry appears.
   - [ ] Search "zone" — confirm "Zone Tag" and "Zone-Aware Shader" entries
         appear.
   - **Pass:** all zone-role glossary terms are reachable and have meaningful
     definitions.

10. **GPU zone-tag dispatch tests** (developer-only)
    - [ ] Run `make test-gpu` (requires a Metal GPU adapter).
    - [ ] Confirm `zone_light_spill_window_tag_golden`,
          `zone_edge_ripple_edge_tag_golden`, and
          `zone_portal_drift_portal_tag_golden` all pass.
    - [ ] Confirm `zone_light_spill_window_tag_golden` verifies ZONE_NONE →
          transparent black (bit-exact).
    - **Pass:** all three GPU golden tests pass; no tolerance violations.

---

## Phase 4 — Scene template validation (v0.8)

11. **Scene template audit: no TemplateZonesMissing warnings**
    - [ ] Run the project audit (File → Audit, or load the project file and watch
          for orange toast banners).
    - [ ] If any `TemplateZonesMissing` Warn finding appears, either:
          (a) add the required zone tags to the relevant masks in Mask mode, or
          (b) confirm the template will render without zones (the operator
          intentionally skipped zone binding).
    - **Pass:** no `TemplateZonesMissing` warnings, or each one is acknowledged
      and the operator accepts degraded (non-zoned) output.

12. **Scene template media slots: all slots assigned**
    - [ ] For each layer produced by a scene template wizard, confirm its media
          path is set and the file exists on disk.
    - [ ] If a layer has an empty path (no media assigned in the wizard), assign
          it now via the Selected-Layer card → FX params → file picker, or return
          to Editing and add a media layer manually.
    - **Pass:** no layers with empty `path` fields that the operator intended
      to fill.

13. **BPM-synced templates: clock running before go-live**
    - [ ] If any template was created with "Sync to project BPM" enabled, confirm
          the BPM is set (tap the BPM strip or enter a value) and the clock is
          running before pressing Go live.
    - [ ] A BPM of 0 or an uninitialised clock will produce no animation; test
          the animation looks correct at show pace before doors open.
    - **Pass:** animation speed matches the musical tempo; no stalled presets.

> **Note:** record pass/fail per step as a commit comment when this script is
> run against the v0.8 release-candidate build.

---

## Lighting output — Art-Net / DMX pre-show checks (v0.9)

Phase 5 adds DMX light output via Art-Net. If your show does NOT use DMX fixtures, skip this section.

14. **Art-Net destination reachable**
    - [ ] Open the Output panel → Lighting section. Confirm the Art-Net destination
          IP:port is set correctly (default `255.255.255.255:6454` for subnet broadcast).
    - [ ] Using a network packet capture (e.g. Wireshark on UDP port 6454) or your
          fixture controller's "DMX monitor", confirm Art-Net packets arrive when
          rmap is in Go-live mode.
    - [ ] Cap: Phase 5 supports up to **16 universes**. If you have more, verify
          the first 16 are correct and the remainder are out of scope.
    - **Pass:** Art-Net node / controller shows incoming DMX traffic.

15. **DMX activity LED — green during Go-live**
    - [ ] In the rmap control window, open the **Diagnostics** section (Advanced
          disclosure panel).
    - [ ] Press Go live. Confirm the DMX activity LED (small circle) is **green**.
    - [ ] Press Exit GoLive. Within ~2 seconds, confirm the LED goes **grey**.
    - **Pass:** LED tracks output activity correctly.

16. **Blackout kills both projector and fixtures**
    - [ ] While in Go-live with DMX active, press **B** (Blackout).
    - [ ] Confirm the projector output goes black AND fixtures go dark in the
          same frame (visible simultaneously on the network monitor).
    - [ ] Press **B** again to release blackout; both surfaces should recover.
    - **Pass:** visual blackout and DMX zeros arrive within 23 ms of each other
      (one Art-Net tick; verified with Wireshark if precise).

17. **Fixture colour follows canvas region**
    - [ ] Open the Output panel → Lighting section. Select a fixture group with
          a CanvasRegion source.
    - [ ] Change the background colour or load a brightly coloured image. Observe
          that fixture output colour tracks the canvas region.
    - [ ] Operator target: from zero configuration to watching a fixture follow
          the canvas within **5 minutes** on a fresh setup.
    - **Pass:** fixture colour is visually correlated with the sampled canvas region.

18. **Show-day frame budget — 16 universes active**
    - [ ] With 16 DMX universes configured and active (or as many as your show
          uses), open the Diagnostics section and check the fps badge.
    - [ ] The diagnostics "DMX: N pkt/s" rate badge should show approximately
          44 pkt/s per universe (44 Hz × number of universes).
    - [ ] The render fps badge should remain at 60 Hz (vsync) with no frame drops.
    - **Pass:** frame budget unchanged; fps steady at show pace.

> **Note:** record pass/fail per step as a commit comment when this script is
> run against the v0.9 release-candidate build.

---

## Phase 6 show-control verification (cuelist + transport)

Run these checks after the Phase 5 steps above. Requires at least one cue saved.

19. **MIDI controller binding — before going live**
    - [ ] Right-click a parameter row (e.g. Blur radius) → "Learn next MIDI CC".
    - [ ] Twist a CC knob on the controller.
    - [ ] Verify the binding tag appears on the row (e.g. "MIDI CC 21 ch 1").
    - [ ] Save the project. Reload it. Verify the binding tag is still present.
    - [ ] Cmd-Z the binding. Verify it reverts to unbound. Cmd-Shift-Z: verify it returns.
    - **Pass:** MIDI CC binding survives save/reload/undo.

20. **Audio bands strip — active audio source**
    - [ ] Start an audio input source (e.g. system microphone or loopback).
    - [ ] Verify the audio bands strip appears above the show-day strip.
    - [ ] Click the chevron (▸/▾) to toggle collapsed (36 px) / expanded (80 px).
    - [ ] Verify 8 labelled bars (Sub through Air) animate with the audio input.
    - **Pass:** strip visible, bands animate, collapse/expand works.

21. **Cue strip dry-run (arm + fire each cue once)**
    - [ ] Build a project with at least 3 cues.
    - [ ] Press → to arm cue 2 (amber ring visible on tile 2).
    - [ ] Press Space to fire cue 2 (LIVE badge appears on tile 2).
    - [ ] Press → to arm cue 3. Press Space. Repeat through all cues.
    - [ ] Press Backspace to back-step one cue. Verify it fires the previous cue.
    - **Pass:** all cues fire on Space, back-step works, LIVE badge tracks correctly.

22. **Cue timing and fire mode**
    - [ ] Click a cue tile to open the detail panel.
    - [ ] Set in-time = 2 s, hold = 3 s, fire mode = Follow.
    - [ ] Fire the cue. Verify the LIVE tile shows a 2-second crossfade progress ring.
    - [ ] After 3 seconds in hold, verify the next cue auto-fires (follow chain).
    - **Pass:** fade animates for in-time duration; follow chain advances automatically.

23. **BPM quantize**
    - [ ] Set global quantize to 4 bars in the transport HUD.
    - [ ] Press → to arm a cue. Press Space.
    - [ ] Verify the cue does NOT fire immediately — the armed tile stays amber.
    - [ ] After the 4-bar boundary, verify the cue fires automatically.
    - **Pass:** cue defers to bar boundary; fires on the downbeat.

24. **Timecode trigger — synthetic MTC**
    - [ ] Connect a DAW (or MIDI test tool) sending MTC to a MIDI port rmap sees.
    - [ ] Click a cue tile. Enable timecode trigger. Set a position 10 seconds ahead.
    - [ ] Verify the cue fires when the DAW reaches that position.
    - [ ] (Alternative) use a software MTC generator; any source of 0xF1 messages works.
    - **Pass:** timecode trigger fires the cue at the specified position (±1 frame).

25. **MIDI Clock BPM sync**
    - [ ] Connect a DAW sending MIDI clock (0xF8) to a port rmap sees.
    - [ ] Verify the BPM HUD updates to match the DAW's tempo within ~1 second.
    - [ ] Change the DAW BPM. Verify the HUD tracks the change.
    - **Pass:** BPM HUD follows MIDI Clock within ±1 BPM.

### Phase 7 capability checks

26. **Syphon output** *(requires OBS + Syphon plugin)*
    - [ ] Enable Syphon out in the Output panel; confirm OBS sees the source.

27. **Venue calibration** *(requires a saved `.rmap-calibration.json`)*
    - [ ] Load venue calibration; verify alignment cross on surface.

28. **Bezier warp** *(if using Bezier mesh layers)*
    - [ ] Confirm Bezier handles are not accidentally engaged (warp mode = Anchor).

29. **RGBW fixtures** *(if using RGBW fixture groups)*
    - [ ] Fixture group CCT setting matches physical fixture spec.

### Recovery steps

- **MIDI learn times out (30 s):** The binding is not applied. Right-click the row
  again and choose "Learn next MIDI CC" to re-arm. Check that the controller is
  connected and sending CC messages on the correct channel.
- **LTC signal lost mid-show:** Timecode-triggered cues will stop firing automatically.
  The operator must advance manually with Space until the LTC source is restored.
  Add a back-up timecode trigger at the next cue's position in case of brief dropouts.
- **Cue order confusion:** Press Backspace to step back one cue, or click the tile
  directly to select it; then press Space to fire it manually.

> **Note:** record pass/fail per step as a commit comment when this script is
> run against the v1.0 release-candidate build.
