# Custom rigid characters

A character may optionally include a version 1 inline `rig` definition. Existing
characters without one retain the original procedural humanoid. The desktop
custom-rig editor provides a detached YAML draft with Apply, Reset and Use original
humanoid actions. Apply uses the existing source history and full script validation;
invalid definitions do not replace the project. The definition lives in screenplay
source and therefore saves and reopens in `.kilsat` projects.

```yaml
characters:
  PUPPET:
    asset: custom
    rig:
      version: 1
      id: puppet
      joints:
        - {id: root, parent: null, x: 0, y: -40}
        - {id: head, parent: root, x: 0, y: -40}
      parts:
        - id: body
          joint: root
          points: [[-20, -20], [20, -20], [20, 40], [-20, 40]]
          color: [70, 160, 220, 255]
        - id: head_shape
          joint: head
          points: [[-18, -18], [18, -18], [18, 18], [-18, 18]]
          color: [245, 200, 160, 255]
scenes:
  - {id: intro, duration: 2, events: []}
```

Stable definition, joint and part IDs contain 1–128 ASCII letters, digits,
underscores or hyphens. Joint IDs and part IDs are unique within their respective
collections. Joints appear before children; unknown parents and cyclic hierarchies
fail validation. Joint `x`, `y` and radian `rotation` default to zero; `scale_x` and
`scale_y` default to one. Parents compose translation, rotation and scale into child
coordinates. A part names its attachment joint and supplies 3–64 joint-local polygon
vertices and RGBA bytes. Parts draw in source order. Definitions contain 1–256
joints and 1–256 parts. Both authored and evaluated coordinates must remain finite
and within the rig's existing bounds. Collapsed polygons fail validation.

The validated scene snapshot prepares rest hierarchy and rigid part geometry once.
Preview and export use those same evaluated local polygons and the actor's existing
root translation/rotation/scale and camera transform. Custom rigid artwork replaces
the procedural body and face; the character label remains. Existing humanoid
procedural clips, seven-joint masks and visemes do not deform these custom parts.

This increment supports editable rigid colored polygons, per-joint animation
tracks (below), image textures and two-joint skinning (see
[Textures and skinning](#textures-and-skinning)). Mesh editing and a visual
joint/vertex/weight canvas remain separate work. Physical Mac editing and rendering acceptance require actual
device evidence; source fixtures and CI do not establish that acceptance.

## Joint animation tracks

A scene's `composition.joint_tracks` animates custom-rig joints by actor and joint
id. Keys use the same fields and easing as actor and bone tracks (`at`, `x`, `y`,
`rotation` in radians, `scale_x`, `scale_y`, `easing`) but are offsets from the
joint's rest pose: translation and rotation add, scale multiplies. Children follow
their parents, and parts draw in source order.

```yaml
scenes:
  - id: intro
    duration: 2
    events: []
    composition:
      joint_tracks:
        PUPPET:
          head: [{at: 0}, {at: 1, rotation: 0.4}, {at: 2}]
```

Validation requires an actor with a custom rig, existing joint ids, 1–4096
strictly increasing times within the scene, at most 65,536 keys per scene, offsets
and rotation within ±1000 and scale between 0.1 and 4. Unknown actors/joints,
actors without a rig, and keys beyond the scene are rejected with actionable
errors. Changing a rig so a tracked joint disappears fails the same validation, so
the edit is rejected atomically; remove the track first. The rig is validated once
per project; each sample only composes the hierarchy, so preview, SVG/PNG and video
export share one deterministic evaluation. Rotation interpolates by the shortest
direction, so a half-turn between two keys is ambiguous; use intermediate keys.
The inspector's **Custom rig joint keyframes** section (shown when the selected
actor has a rig) edits one joint's keys per scene with the same detached draft as
bone keys: Apply is an atomic, undoable source transaction; changing scene, actor or
joint discards unapplied edits; an empty list removes the track. Original-rig
bone tracks and clips stay unsupported for custom actors, and nothing here
establishes physical Mac rendering acceptance.

### Texture and weight controls

The custom-rig editor's **Part texture and skin weights** section edits the YAML
draft without hand-writing it: pick a part; pick an Image asset from the project
inventory (disabled with an explanation when there is none); edit `u0 v0 u1 v1`;
add/remove skin weights; per vertex choose one or two influence joints and weights,
with **Normalize vertex/all** (rejected, leaving the draft untouched, for negative,
non-finite or all-zero weights). Controls parse the draft, change it and re-serialise
it, so comments and formatting are normalised, a rig without texture/skin
re-serialises without those keys, and the controls are disabled while the YAML does
not parse. Issues (unknown asset, weights not summing to one, and the real rig
validation error) are listed before Apply. Apply is unchanged: the same atomic,
validated, undoable source transaction, which also rejects texture assets that are
not Image assets in the inventory and stale source/actor drafts. This is a numeric
form, not a visual canvas; tests are headless egui and logic tests only, with no
hand-run desktop validation.

## Try the integrated example

Open `examples/studio-production-demo.kilsat` with the desktop Open project action.
It contains only original colored shapes and inline source; it has no asset paths
to resolve, no speech events and no model or network requirement. Play or seek the
four-second scene. The polygon puppet moves to the right through a root movement;
the original humanoid moves left and loops two hand clips in the same normalized
group. The warm and cool background shapes belong to a nested isolated artwork
group, with separate group opacity and screen blending.

Select PUPPET and open Custom rigid character. Change the head polygon color or
its joint's `rotation`, Apply, then Undo/Redo. Save a copy and reopen it to check
persistence. Select EMMA and inspect the adjacent `high` and `out` live rows: both
use group `reach` and weight `0.6`. Changing a row weight changes the normalized
contribution; changing row order inside this group preserves the weighted result.
Seek repeatedly across the two-second loop boundary. The puppet intentionally has
no humanoid bone tracks, live clips, gesture events or speech deformation.

This example makes the source changes reviewable. Its JSON structure was checked
when created; repository CI must establish full Rust project decoding, evaluation
and rendering. Manual desktop actions above remain a physical Mac acceptance task.

PR #46 passed final Linux/macOS CI, including saved-project/history, context-guard,
simple-polygon and shared-render/preview regressions. Exact evidence is in
[master progress](master-progress.md). Custom actors reject original-rig gestures,
bone/live tracks, animated procedural strength tracks and nondefault static
procedural strengths with actionable errors. Remove unsupported original-rig
tracks before assigning a custom definition; default strengths remain compatible.
No physical Mac editing or motion quality acceptance is implied by these checks.

## Rig clip library and retargeting

`rig_clips` in the screenplay stores reusable joint-track clips (at most 64 clips,
65,536 keys, 600 s each). In the **Custom rig joint keyframes** section, name a clip
and **Capture joint tracks** to copy the selected actor's tracks for the current
scene (replacement needs the explicit checkbox). **Apply to selected actor** copies
the clip onto the actor's rig, replacing its joint tracks in that scene:

- *Joint mapping*: `clip_joint=target_joint` lines. Unlisted joints keep their ids;
  every target joint must exist, and two clip joints may not map to one target. The
  error names the first unmapped joint.
- *Translation scale* (0.01–100) multiplies x/y offsets for larger or smaller rigs.
- *Place in scene interval* maps clip times proportionally into a start/duration;
  without it the clip must fit the scene unchanged.

Applied copies and the library entry are independent; deleting a clip leaves copies.
Everything is one validated, undoable source edit. This copies and retargets by
joint id and scale only: it is not live instancing, mirroring, blending of several
clips, IK or automatic bone matching, and it carries no rotation/scale correction
between rigs with different rest orientations. No physical Mac review was done.

## Textures and skinning

Both fields are optional per part and skipped on save when absent, so legacy
rigs serialize and render byte-for-byte as before.

```yaml
parts:
  - id: arm
    joint: shoulder
    points: [[0, -4], [10, -4], [20, -4], [20, 4], [10, 4], [0, 4]]
    color: [200, 120, 80, 255]          # drawn first, under any texture
    texture: {asset: arm_skin, uv: [0, 0, 1, 1]}
    skin:                               # one entry per point
      - [{joint: shoulder, weight: 1}]
      - [{joint: shoulder, weight: 0.5}, {joint: elbow, weight: 0.5}]
      - [{joint: elbow, weight: 1}]
      # ... one list per remaining point
```

*Texture*: `asset` is the id of an **image** asset in the project inventory.
`uv` is `[u0, v0, u1, v1]` (image fractions, v downwards, default whole image;
each in 0..1 with at least 0.001 extent) stretched over the part's rest-pose
bounding box and clipped by the part polygon. The decoded PNG is embedded as a
data URI by the shared CPU/SVG renderer, so preview, SVG, PNG and video export
agree. At most 16 textured parts per rig; embedded PNG bytes per frame share the
artwork `MAX_IMAGE_BYTES` limit. A missing resource fails with an error naming
the asset (render, preview and export preflight); a project, and every history
snapshot, must reference an image asset present in the inventory. The command
line examples that load only layer-referenced images do not yet load rig
textures.

*Skinning*: `skin` is empty (rigid) or has exactly one entry per point, each with
one or two `{joint, weight}` influences: finite weights in 0..1 summing to one
(1e-6 tolerance), distinct existing joints. Points remain in the attachment
joint's rest-local frame, which is the bind pose: each vertex is the weighted sum
of `world(joint) * inverse(rest_world(joint)) * rest_world(part joint) * point`.
At rest this equals the unskinned polygon for any weights; the rest outline must
also stay a simple polygon. Posed outlines may self-intersect, as in any linear
blend skin. Evaluation happens once per sample and is deterministic.

A textured, skinned part moves its texture with the best least-squares affine fit
of the rest vertices onto the deformed ones (exact for rigid motion, and for
deformations that are affine); the clip uses the exact deformed polygon. The
texture is not warped per triangle.

The experimental GPU scene path draws flat, skinned (already deformed) and
textured parts through `build_with_art`, using the same affine placement and the
decoded images shared with artwork; `build` (no images) still reports textured
parts as `UnsupportedFeature`. Measured CPU/GPU differences are in
[gpu-parity.md](gpu-parity.md). Tests establish geometry, pixel registration at
90 degree rotations and preview/PNG agreement on Linux/macOS CI; no hand-run
desktop or perceptual review was done.

## Acting layer: variants, visemes, expressions, eyes

All of this is optional and skipped on save when absent, so legacy rigs and
scenes serialize and render as before. The complete original example is
[examples/acting-rig.yaml](../examples/acting-rig.yaml); the tests load that file.

```yaml
parts:
  - id: mouth
    joint: head
    points: [[-10, 14], [10, 14], [10, 17], [-10, 17]]
    color: [120, 40, 50, 255]
    variants:                       # up to 16 per part
      open: {points: [[-10, 10], [10, 10], [10, 26], [-10, 26]]}
      smile: {color: [150, 60, 70, 255]}
visemes:                            # sprite swap of one mouth part
  part: mouth
  map: {a: open, e: wide, o: round}
expressions:                        # emotion -> variants (neutral keeps base)
  happy: [{part: mouth, variant: smile}, {part: brow_l, variant: raised}]
eyes:
  pupils: [pupil_l, pupil_r]        # follow the gaze track
  travel: 4                         # max pupil offset, actor units (0..500)
  blink: {parts: [eye_l, eye_r, pupil_l, pupil_r], variant: blink, interval: 3, duration: 0.15}
scenes:
  - composition:
      gaze_tracks:
        PUPPET:
          - {at: 0, x: 0, y: 128}   # screen point, scene pixels
          - {at: 4, target: FRIEND} # another actor's eyes (origin if it has none)
```

*Variants* override any of `points`, `color` and `texture` of the base part;
alpha 0 hides it. A skinned part's variant polygon must keep its point count.
Texture variants count toward the 16 textured parts and are included in the
rig's image assets.

*Visemes*: map keys are the `motion_lipsync::Viseme` names (`rest closed teeth_lip
tongue_teeth tongue_up sibilant sh r a e i o u`); unmapped visemes, and actors with
no viseme, keep the base mouth. `apply_speech` (already used by preview, SVG/PNG and
video export with the same sample-clock viseme) swaps the part, so every path agrees.
The viseme replaces any expression variant on the mouth part.

*Expressions* use the five screenplay emotions; variants are chosen by the actor's
current emotion at the sample time. Precedence per part: expression, then blink,
then viseme. Brows may equally be animated by joint tracks.

*Eyes*: `gaze_tracks` keys are strictly increasing scene times with either `x`/`y`
or `target` (another actor). The look direction (full travel at 100 units distance)
is measured in the actor's own space from the mean pupil vertex and blends linearly
between keys; pupils translate by at most `travel`. Blinks swap `variant` onto the
listed parts for `duration` seconds, at most once per `interval`-second window
(1..30 s); the start inside the window comes from an FNV-1a hash of the actor id
and window index, so it depends only on actor id and scene time (no clock or random
state) and is identical across repeated and out-of-order seeks. Every scene
restarts the same pattern.

Validation rejects unknown variants, parts, viseme or emotion names, duplicate
parts per emotion, more than 16 variants or entries, out-of-range travel or blink
timing, gaze keys that are unordered, beyond the scene, mixed or half-specified,
unknown or self targets, and gaze tracks for actors without eyes. Removing a
character drops gaze keys that target it.

The experimental GPU path receives variants already resolved to ordinary parts, so
flat variants draw like any part; textured parts still need `build_with_art` and
otherwise report `UnsupportedFeature`, as before. Tests cover exact-sample viseme
and emotion selection, gaze bounds, blink determinism and rate, and
SVG/PNG/preview pixel agreement. They do not establish perceived lip-sync or acting
quality, and no hand-run desktop review was done.
