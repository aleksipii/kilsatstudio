# Render contract (A08, ORK-G1-06)

Owner: `motion-render` (A08). Consumers: native preview (A09), export (A10).
Code: `crates/motion-render/src/contract.rs`, `framing.rs`; background style in
`crates/motion-art/src/style.rs`. Test scene and reference images:
[`examples/g2-minimal-scene/`](../examples/g2-minimal-scene/README.md),
[`docs/evidence/g2-render/`](evidence/g2-render/README.md).

## Input and output

- Input: `&motion_scene::EvaluatedScene` (type owned by A04), optional artwork
  (`scene_id`, `&motion_art::Document`, decoded images) and viseme mouths
  evaluated by `motion-scene`.
- Output: a self-contained SVG string (`render_svg`, `render_svg_with_mouths`,
  `render_svg_with_art`, `render_svg_with_art_transparent`) or GPU
  `scene_commands::SceneCommands` (feature `gpu`).
- No clock. No render entry point takes a time parameter or keeps state between
  calls. Everything time-dependent (artwork `active` intervals, animated images,
  mask motion, lights) reads `EvaluatedScene::time`, which the shared evaluator
  produced from the one sample clock. Seeking backwards and re-rendering gives
  identical bytes (tested by `reference_svgs_are_deterministic_and_match_recorded_hashes`).
- `motion-export` calls the same evaluator and renderer; it does not compute time
  differently. Field changes to `EvaluatedScene` are requested from A04 (KS-003d).

## Depth order

Back to front, as reported by `contract::paint_order` and painted by both the SVG
renderer and `SceneCommands`:

| Band | Contents | Order inside the band |
| --- | --- | --- |
| `Background` | Background-plane artwork (attached or not) | Document sibling order, groups flattened in order |
| (overlay) | Lights with `behind_actors` | Document order; canvas-space tint, not a band item |
| `Characters` | Evaluated actors | `EvaluatedScene::actors` order: composition `layers.z` ascending, ties by character ID |
| `Props` | Foreground-plane artwork attached to an actor | Document sibling order |
| `Foreground` | Unattached foreground-plane artwork | Document sibling order |
| (overlay) | Other scene lights | Document order, over everything |

Tie rules are total and position-independent: equal `z` falls back to character
ID (the evaluator's stable sort over the ID-ordered character map); artwork ties
are impossible because each item has one place in the sibling order. Layers
inactive at `scene.time`, hidden groups and layers attached to an actor that is
not in the scene are omitted, exactly as in rendering.

The renderer never re-sorts artwork. Inside the foreground plane, props and
unattached foreground share document order, so a prop listed after foreground
artwork draws over it. `contract::depth_violations` reports this (and any other
band inversion) as an authoring issue instead of silently reordering, which would
change existing projects' output.

Tests: `contract::tests` (band classification, z versus ID ties at identical
positions, seek-order determinism, omission rules, and that `paint_order` matches
the actual SVG painter order) and `depth_order_is_banded_and_deterministic` on the
G2 scene.

## Framing (wide and close shots)

`framing::check_framing(scene, subjects, rule)` projects subject bounds through
the shared `CameraPose::project` rule and returns typed violations. It checks;
choosing camera keys remains with `motion_script::expand_shots` and the desktop
shot-size helper (`apps/studio-desktop/src/camera_framing.rs`, A09).

Subjects: `Actor` (all visible parts), `Head` (custom rig part `face`, else `head`;
procedural head circle r = 25), `Hand` (`hand_l`/`hand_r`; procedural hand joint),
`World` (any pre-camera rectangle, e.g. a bound prop). Margins are fractions of the
frame's shorter side.

| Rule | Margin | Primary subject minimum height | Use |
| --- | --- | --- | --- |
| `FramingRule::WIDE` | 4 % | none | both figures, whole body and feet, raised hand |
| `FramingRule::CLOSE_UP` | 8 % | 20 % of frame height (first subject) | face plus the hand that holds the prop |

Violations: `MissingActor`, `MissingPart` (never silently skipped),
`OutsideMargin` (with projected bounds and allowed frame) and `TooSmall`.
The phone (KS-003a `grip`) will be added as a `World` subject from A05's bound
prop geometry, or as a dedicated subject once that interface exists.

## CPU and GPU

The CPU path (SVG rasterized by resvg in `motion-export`) is the reference for
preview and export. The GPU path is experimental. Its measured per-fixture
tolerances (max channel error, RMS, changed-pixel fraction) are in
[gpu-parity.md](gpu-parity.md); they are regression guards, not a parity claim
(ADR-A01-04). The G2 reference images were produced only with the CPU path; no
GPU render of the G2 scene was run, so no CPU/GPU comparison is claimed for it.
A future GPU comparison of the G2 scene uses the same `Tolerance` form and must
record machine, backend and the measured values.

## Background style

`motion_art::style::check_background` turns the art direction's background rules
([stick-studio-art-direction.md § 3](stick-studio-art-direction.md)) into checks
over one scene's background-plane layers:

- Large opaque surfaces (rectangles/ellipses covering at least 20 % of the canvas,
  clipped to it) have HSV saturation at most 0.35 ("muted").
- Their WCAG contrast against the limb ink `#24282d` is at least 4.5:1, so the
  dark limbs read.
- Background strokes are at most twice the on-screen limb width
  (9 px rig width × actor scale). Stick-studio scenes use actor scale 1.6 → 28.8 px.

All four stick-studio backgrounds and the G2 scene pass at actor scale 1.6
(test `stick_studio_backgrounds_and_g2_scene_follow_background_style`). At actor
scale 1 the cemetery tree trunk (28 px) exceeds the 18 px limit; that case is
asserted so the scale dependence stays visible. These are measurable rules, not
a perceptual art review.
