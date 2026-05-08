# Roadmap for a Photo-Driven Projection and Lighting Engine

## Purpose

This document outlines a product roadmap for a usability-first immersive visual system that combines projected visuals, photos, and physical lighting outputs into one coherent show engine. The current first draft already has a strong renderer-oriented foundation with milestones for fullscreen output, `wgpu` rendering, SVG loading and rasterization, shader effects, compositing, warping, masking, scenes, and project persistence.[cite:217]

The roadmap below focuses on two gaps at once: first, the gap between the current renderer-heavy draft and the desired creative workflow of integrating photos aesthetically into scenes and surfaces; second, the gap between the current draft and mature media-server or projection-mapping tools that offer broader protocol support, live control, and production-ready output workflows.[cite:217][cite:84][cite:251]

## Harsh assessment

The current draft is better engineered than it is positioned as a product.[cite:217] It contains many strong low-level decisions such as off-thread SVG rasterization, ping-pong shader passes, SDF-based masking, projective warping, test patterns, panic recovery, and operator-safety features like blackout, freeze, and display-sleep prevention, all of which are credible foundations for live visuals.[cite:217]

However, the draft currently reads more like a rendering engine roadmap than a workflow for making beautiful immersive shows quickly.[cite:217] Established tools compete less on internal architecture and more on how quickly an operator can map, cue, preview, adjust, and synchronize visuals and outputs across projection and lighting systems.[cite:238][cite:244][cite:251][cite:254]

The biggest product risk is becoming a custom media server that is technically elegant but too broad and too raw in its UX. Mature tools already cover warping, layers, effects, mapping, scenes, and protocol integration, so competing by matching feature breadth would likely create a more complex but still less complete alternative.[cite:238][cite:244][cite:252][cite:254]

## Product direction

The strongest direction is not to become a general-purpose media server, but a constrained immersive show builder centered on three strengths:

- Photo-driven visual composition.
- Authored interaction with surfaces and spatial zones such as windows, edges, voids, and glow areas.
- Unified output across projection and lighting.[cite:217][cite:249][cite:250][cite:251]

That positioning is more differentiated than a generic projection mapper because the system can turn photos into scene material, not just display assets, and can treat projector output and lighting output as parallel expressions of the same scene state.[cite:249][cite:251]

## Design principles

The roadmap should preserve usability over complexity by enforcing a small set of core objects and avoiding a control surface that exposes every internal rendering primitive.[cite:217] The product model should revolve around:

- **Media**: photos, SVGs, selected videos, texture layers.
- **Surfaces**: projector targets and mapped output regions.
- **Zones**: predefined masks and interaction areas such as windows, portals, edges, spill, and no-project regions.
- **Scenes**: authored visual grammars combining media, zones, effects, and timing.
- **Light outputs**: fixture groups, LED pixels, or networked lighting universes.
- **Cues**: scene recall, transitions, and tempo or control events.[cite:217][cite:244][cite:248][cite:251]

The UI should always favor scene templates and semantic controls over deep generic parameter exposure. That is especially important because tools like HeavyM, LightAct, ArKaos, and Modulo succeed in part by offering clearer show-building workflows, not just rendering power.[cite:238][cite:244][cite:251][cite:253]

## Capability gap analysis

| Area | Current draft | Gap to desired product | Gap to established tools |
|---|---|---|---|
| Rendering core | Strong `wgpu`-based core with warping, masks, effects, layers, scenes, and hot reload.[cite:217] | Needs image-first scene treatment, not only SVG/effect pipelines.[cite:217] | Broadly aligned, though less mature and less proven.[cite:84][cite:251] |
| Creative workflow | Mostly effect/layer oriented.[cite:217] | Needs scene templates, zone semantics, and photo-aware composition. | Commercial tools typically package faster authoring UX.[cite:238][cite:252] |
| Surface interaction | Warp and SDF masking are planned.[cite:217] **v3 schema 4 (M3) makes warp + mask per-layer so each layer maps onto its own surface independently — see `specs/003-ui-ux-overhaul-plan.md` §11.6a and `specs/003-tasks-phase-3.md` T3.0a–T3.0d.** | Needs authored spatial behaviors for windows, cutouts, spill, flow, and reveal logic. | Pro tools often include stronger calibration and mapping ecosystems.[cite:233][cite:237] |
| Lighting outputs | Reserved features for audio, MIDI, OSC later.[cite:217] | Needs first-class lighting outputs in the scene model. | Mature systems support DMX, Art-Net, sACN, pixel mapping, and hybrid show control.[cite:244][cite:248][cite:251][cite:254] |
| Operator usability | Some operator-safe features already planned.[cite:217] | Needs a simpler mental model and faster scene setup. | Established tools are more workflow-optimized for operators.[cite:238][cite:249][cite:252] |

## Roadmap overview

### Phase 0 — Keep the foundation, tighten the scope

**Goal:** Preserve the current architecture, but explicitly narrow the product to a single-surface, single-show immersive composition engine for photos, projection, and light.[cite:217]

**Key decisions:**
- Stay focused on one projector output first.
- Keep mappings authored, not auto-detected.
- Treat windows and other architectural features as predefined zones.
- Delay broad protocol sprawl until the core scene model is excellent.

**Outcomes:**
- A cleaner product statement.
- Lower implementation risk.
- Better UX decisions because the target use case is narrower.

### Phase 1 — Make media sources match the artistic goal

**Goal:** Expand beyond SVG-centric rendering into a true media pipeline where photos and raster images are first-class inputs.[cite:217]

**Additions:**
- Native photo/image layer type alongside SVG layers.
- Safe image preprocessing, for example crop modes, fit/fill, focal-point selection, tone mapping, and cache-friendly texture upload.
- Basic treatment pipeline for photos: blur masks, luminance-driven reveals, palette extraction, collage placement, texture overlays.

**Why this matters:**
Right now the draft is technically capable but does not yet express the desired aesthetic idea that photos should be embedded into filters and shaders rather than merely shown as assets.[cite:217] This phase gives the system a creative identity.

**Usability rule:**
Do not expose dozens of image controls at first. Ship a small number of tasteful image behaviors as presets.

### Phase 2 — Introduce spatial zones as first-class authored objects

**Goal:** Make surfaces meaningful without requiring AI or live facade detection.[cite:217]

**Additions:**
- Named zones with semantic roles, for example `window`, `portal`, `void`, `spill`, `edge`, `highlight`, `light-source`.
- Region-aware shaders that can read zone masks and adjust behavior by area.
- A lightweight zone authoring UI on top of the existing mask and warp system.[cite:217]

**Why this matters:**
This directly addresses the wish that windows or facade areas should influence the scene. The effect can be visually convincing even when the geometry is set up manually, and manual authoring is often more predictable than unreliable automation in real venues.[cite:217]

**Usability rule:**
Every zone should be selectable from a small semantic palette rather than built from arbitrary low-level shader graphs.

### Phase 3 — Replace “effect stack” thinking with scene grammars

**Goal:** Move from a renderer-centric experience to a scene-centric product.[cite:217]

**Additions:**
- Scene templates such as `window reveal`, `pixel drift`, `collage bloom`, `glow behind openings`, `fragmented portrait`, and `architectural wash`.
- Scene behaviors that combine media placement, timing, masking, and output dynamics.
- A scene editor that asks for media, zones, palette, mood, and tempo before offering deeper controls.

**Why this matters:**
This is the main step that improves usability over complexity. Instead of asking the operator to compose layers and effects manually every time, the system should start with strong scene grammars and allow tuning only where needed.

**Usability rule:**
A first-time operator should be able to create something impressive by selecting a scene template, assigning a few media assets, and mapping a handful of zones.

### Phase 4 — Add unified lighting outputs

**Goal:** Extend the visual pipeline so one scene can drive both projection and physical lights.[cite:244][cite:248][cite:250][cite:251]

**Additions:**
- Lighting output graph as a first-class part of the engine.
- Art-Net and/or sACN output as the primary lighting transport, because they are common for networked DMX and pixel-mapped event systems.[cite:244][cite:248][cite:250][cite:251][cite:254]
- Fixture groups and pixel maps that sample colors or intensities from scene outputs.
- A small number of output strategies:
  - Scene-wide color wash.
  - Zone-derived accent output.
  - Pixel-mapped LED strip or fixture groups.
  - Trigger/cue outputs for external lighting systems.

**Why this matters:**
This phase closes one of the largest gaps between the draft and the immersive vision. It also moves the system closer to established tools that treat video and lighting as coordinated parts of one show system.[cite:244][cite:249][cite:251]

**Usability rule:**
Do not start with full moving-light personality editing. Start with simple fixture groups, RGB/RGBW output, and pixel-mapped LED workflows.

### Phase 5 — Show control and cueing

**Goal:** Make the system practical for events and repeatable operation.[cite:217][cite:252]

**Additions:**
- Cue list and timeline-lite workflow.
- Scene transitions and crossfades.
- External control via OSC/MIDI after the core UX is stable.[cite:217]
- Optional audio-reactive modulation only when it supports scene design rather than adding chaos.

**Why this matters:**
The current draft already includes scene save/recall, blackout, freeze, and some modulator work, so this phase builds on that foundation in a product-oriented way.[cite:217]

**Usability rule:**
A live operator should be able to trigger scenes, fade between them, and recover from mistakes quickly without navigating deep control panes.

### Phase 6 — Professionalization and interoperability

**Goal:** Close the most important gaps to mature tools without turning the product into a bloated clone.[cite:238][cite:251][cite:254]

**Additions:**
- Better fixture abstraction and more robust network protocol support.
- Logging, diagnostics, and show-day utilities refined from the current reliability work.[cite:217]
- Export/import of scene packs and reusable surface templates.
- Optional multi-output growth only if the single-surface plus lighting workflow is already excellent.

**Why this matters:**
At this point the product should compete by being clearer and more beautiful, not by blindly matching every capability of established media servers.

## Proposed deliverables by product layer

| Layer | Near-term deliverable | Mid-term deliverable | Long-term deliverable |
|---|---|---|---|
| Media | Photo/image layer support | Photo-aware scene templates | Smarter media placement and stylization |
| Spatial authoring | Polygon masks and warps.[cite:217] | Named semantic zones | Reusable venue and facade templates |
| Rendering | Current effects, compositing, warp, masking.[cite:217] | Region-aware effects | Richer scene grammars and output routing |
| Lighting | None yet in product form | Art-Net/sACN output graph | Pixel mapping, fixture groups, console interop |
| Show control | Scene recall and blackout/freeze.[cite:217] | Cue list and transitions | External control ecosystem |
| UX | Control panel and mapping UI.[cite:217] | Template-driven scene builder | Productized operator workflow |

## Suggested implementation order

A practical implementation order that preserves usability would be:

1. Finish the current rendering and mapping foundation through the core v1 milestones.[cite:217]
2. Add image/photo layers.
3. Add zone semantics on top of the existing mask system.[cite:217]
4. Build scene templates that combine media and zones.
5. Add lighting outputs through Art-Net/sACN and simple fixture groups.[cite:244][cite:250][cite:251]
6. Add cueing, transitions, and external live control.
7. Only then expand into broader interoperability or higher-end automation.

This order ensures the product becomes useful and differentiated before it becomes broad.

## What to postpone deliberately

To preserve usability, the following should stay out of the critical path:

- Full AI-based facade detection.
- Deep generic shader graph authoring.
- Complex multi-projector workflows.
- Moving-light personality complexity.
- Huge protocol surface area early on.

These are tempting but would likely pull the product toward complexity before the core creative workflow is truly satisfying.[cite:217][cite:252][cite:254]

## Success criteria

The roadmap is working if the product reaches these outcomes:

- A user can create a beautiful photo-driven mapped scene in minutes, not hours.
- Surface interaction feels intentional through authored zones and masks.
- Projection and lighting outputs feel like one system, not two loosely connected tools.[cite:249][cite:251]
- The operator UI remains understandable even as output capabilities expand.
- The system competes by clarity and aesthetic coherence rather than by feature count.

## Strategic summary

The first draft already contains a solid renderer and show-safety foundation.[cite:217] The right next move is not to broaden it into a generic media server, but to turn it into a usability-first immersive composition system where photos, projected visuals, spatial masks, and lighting outputs are all different expressions of one scene model.[cite:217][cite:244][cite:251]

That strategy addresses both sets of gaps: it moves the product closer to the established tool landscape where integrated output and show control matter, while also sharpening the product around a more distinctive and emotionally compelling use case than generic projection mapping software.[cite:238][cite:249][cite:254]
