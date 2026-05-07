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

8. **Monitor index** — `rmap --list-monitors`, then `rmap --monitor N …` or rely on `output_monitor_index` inside your `.rmap.json`.
9. **Autostart** — `rmap path/to/show.rmap.json --autostart` loads the file and uses saved monitor selection unless `--monitor` overrides.
10. **Project file** — Keep a backup `.rmap.json`; save again from the UI **Project file** panel after edits.

## During show

11. **Blackout / freeze** — Know your keys (e.g. blackout / freeze / test pattern) and rehearse once on the actual rig.
12. **Hot reload** — SVG saves and JPG/PNG saves on disk refresh layers that watch those paths; avoid editing paths mid-cue without a fallback cue.

## v2 scene editor

13. **Drop targets** — Drag SVG / PNG / JPG files onto the **control window's Scene tab** to add a layer. Dropping on the projector window does nothing (intentional: the drop target is unambiguous).
14. **Layer manipulation** — Click a layer in the live preview to select it; drag to translate, Shift-drag to scale, Alt-drag to rotate. Esc deselects.
15. **Mask zones** — Pick a starter shape from the **Mapping** tab's zone palette (window-rectangle, arch-portal, circle-spotlight, void-block); drag each vertex of the painted overlay onto the real venue feature. Double-click an edge to insert a vertex; Shift-click a vertex to delete (won't drop below 3).
