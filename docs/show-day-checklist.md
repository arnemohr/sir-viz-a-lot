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
