# Live authored clip layers

Inspector → **Live authored clips** creates references to the project's named
bone clips. Create the library clip first using the per-bone editor and
[clip library](animation-clip-library.md). Select the target actor, choose
**New live clip instance**, enter a scene-local instance ID, clip, start, duration
and weight, then explicitly Apply. Existing instance IDs remain fixed while editing.

The ordered rows show the entire scene, including other actors. Select an actor
to edit or move its rows. Up/Down changes the row's position in the complete
scene list. Removing an instance needs confirmation and leaves its definition
and independently copied tracks intact. Reset restores draft timing and static
weight, clears fade durations and weight keys, and preserves the ID, selected clip,
looping mode, joint selection and blend mode.
Source/scene/actor changes discard drafts. Every source action supports unified
Undo/Redo and `.kilsat` persistence; YAML formatting and comments are normalized
as in the existing editors.

```yaml
animation_clips:
  tilt:
    duration: 1
    bone_tracks:
      head: [{at: 0, rotation: 0}, {at: 1, rotation: 0.4}]
scenes:
  - id: shot
    duration: 3
    events: []
    composition:
      clip_instances:
        - {id: tilt_a, clip: tilt, actor: EMMA, at: 0.5, duration: 2, weight: 0.5}
```

This snippet assumes `EMMA` is defined in the screenplay's `characters` map.
The weight defaults to 1 when omitted. Missing `clip_instances` means no live
layers, preserving existing projects.

## Timing and blending

An instance contributes on `[start, start + duration)`: its start is included,
its end excluded. By default, clip time is
`(scene_time - start) / duration * clip.duration`: the clip stretches once to fit.
Within that window, tracks hold endpoint values if their keys do not span the
clip duration. Outside it the layer contributes nothing. Hard activation edges
can pop when no envelope is authored. Optional fades and looping are described
below. Authored hold-key jumps are not automatically smoothed.

In the default override mode, start with the actor's ordinary authored local bone offsets, using identity for
absent bones. Visit active rows in saved order. On each selected bone present in a clip,
interpolate the current offset toward the sampled clip offset by its effective
weight. Translation and scale interpolate linearly; rotation uses the existing
shortest-arc rule. Weight 0 leaves the base exactly unchanged and 1 replaces it.
Bones missing from the clip preserve their previous value. Two half-weight
layers can give different results when reordered; weights are not normalized.

The combined offsets are applied after procedural posture/walk/gesture and
before the existing root and speech stages. Root paths, facial expression,
mouth timing and artwork retain their own stages. Native live-clip preview uses
the existing bounded compositor worker and the shared scene/export pipeline.
This is ordered override blending for the original seven-bone rig, not general
additive pose mixing.

## Fades and animated weights

The editor exposes **Fade in**, **Fade out** and local weight keys. Key times
are seconds since this instance starts, independent of the library clip's own
key times. Add/remove keys and choose outgoing linear, smoothstep or hold easing,
then Apply the detached draft. Before the first key and after the last, the
endpoint multiplier is held inside the activation window. With no keys the
multiplier is 1. Fades default to zero, preserving existing projects exactly.

The effective override weight is the static weight multiplied by the keyed
multiplier, the linear fade-in factor and the linear fade-out factor. A nonzero
fade-in rises from zero to one over its duration. A nonzero fade-out falls from
one to zero at the instance end. A zero fade duration contributes a factor of
one. Fades do not normalize competing layers or smooth internal hold jumps.

```yaml
clip_instances:
  - id: tilt_a
    clip: tilt
    actor: EMMA
    at: 0.5
    duration: 2
    weight: 0.8
    fade_in: 0.25
    fade_out: 0.5
    weight_keys:
      - {time: 0, weight: 0.4, easing: smoothstep}
      - {time: 1, weight: 1, easing: hold}
      - {time: 2, weight: 0.5}
```

Use this fragment under `scenes[].composition` with the earlier definition.
Both fade durations must be finite and nonnegative; their sum cannot exceed the
instance duration. Local keys must strictly increase within `[0, duration]` and
have finite multipliers in `[0,1]`. Changing instance duration does not silently
rescale keys or fades: inconsistent drafts remain visible and cannot Apply.
Use the explicit reset or edit/remove keys to recover. The library definition,
other instances and speech/event timing are not changed by an envelope edit.

## Looping and joint selection

Enable **Loop at original clip speed** to repeat a library clip at one clip second
per scene second. With `looping: true`, clip time is
`(scene_time - start) modulo clip.duration`. An exact cycle boundary samples the
start of the clip; the instance end remains excluded. A shorter instance cuts the
cycle short, while a longer one repeats for as many complete or partial cycles as
fit. With `looping: false` or the field omitted, the existing stretch-once mapping
is preserved. Loop boundaries can jump if authored endpoint poses differ.

Fades and weight keys keep their instance-local clock across every cycle. They do
not restart on loop boundaries. Changing the library duration changes the loop
period; changing instance duration changes the activation window without retiming
the library keys, fades or weight keys.

Enable **Select individual joints** to choose original-rig joints. The initial
selection includes all seven joints; the UI keeps at least one selected. Disable
the selection to use all authored clip joints. This setting remains a detached
draft until Apply. A mask affects only this instance's local authored offsets,
without descendant expansion. Unselected joints retain accumulated base or earlier
layer offsets. Selected joints absent from a clip also retain their prior offsets.

```yaml
clip_instances:
  - id: hand_loop
    clip: tilt
    actor: EMMA
    at: 0.5
    duration: 2
    weight: 0.5
    looping: true
    joint_mask: [head]
```

This fragment uses the earlier `tilt` definition. An empty or omitted `joint_mask`
selects all authored clip joints. Valid names are `hip`, `chest`, `head`,
`left_hand`, `right_hand`, `left_foot` and `right_foot`. Masking does not introduce
arbitrary rigs, per-joint blend weights, texture deformation or general additive
pose mixing. Reset timing/weight/envelope controls preserve loop mode and mask;
edit these controls explicitly to change them.

## Additive local offsets

PR #45 adds `blend_mode: additive`, integrated after final Linux/macOS CI.
Omitted `blend_mode` means `override`, preserving existing
documents and output. Select **Additive** in the detached instance editor, then
Apply. Reset timing/weight/envelope controls preserve the chosen blend mode.

Additive clips contain signed local deltas for the original seven-joint rig.
For accumulated offset `base`, sampled clip `delta` and effective weight `w`:

| Component | Additive result |
| --- | --- |
| Translation | `base + w * delta` |
| Rotation | `base + w * delta`, using signed authored radians |
| Scale | `base * (1 + w * (delta - 1))`, independently per axis |

For example, base X 4 with clip delta X 20 and weight 0.5 produces X 14;
override at that weight produces X 12. Base scale 1.125 and delta scale 1.5
produce scale 1.40625. Identity delta is neutral. Rotation is added as a signed
delta; it is not wrapped to the shortest arc during combination.

```yaml
clip_instances:
  - id: extra_tilt
    clip: tilt
    actor: EMMA
    at: 0
    duration: 2
    weight: 0.5
    blend_mode: additive
    joint_mask: [head]
```

Saved source order is retained. Override after additive can differ from additive
after override. Loops, joint selection, fades and weight curves use the same
sampling rules in both modes; effective weight is applied once. Missing or
unselected joints preserve their earlier offsets. Zero influence preserves the
base exactly. Inputs and accumulated outputs must remain finite and within the
existing authored-transform bounds; reduce accumulated translations, rotations
or scales when an evaluation error reports excessive poses. A later layer does
not hide an invalid intermediate result.

This is local-offset composition for the original rig. General normalized groups,
arbitrary-rig pose blending, textures and deformation remain separate work.

## Live references versus copies

Library replacement affects every live reference and displays affected scene
and instance counts. Instance start/duration stay fixed; a changed definition
duration changes its sample mapping or loop period. Previously copied or retimed tracks remain
independent. A referenced definition cannot be deleted: remove its live instances
first. Removing a character removes its instances atomically. Duplicating a
scene keeps scene-local IDs and shares the clip definitions.

## Validation and limits

IDs use 1–64 ASCII letters/digits/underscores/hyphens and are unique within the
scene. Clip and actor references must exist. Start must be finite and nonnegative;
duration finite and positive; the finite end must exceed start and fit the scene.
Weights must be finite in `[0,1]`. A joint mask must contain at most seven unique
original-rig joint names; unknown and repeated names are rejected. Zero-weight instances still require valid
references and count toward limits.

Limits: 128 instances per scene, 32 per actor per scene and 1024 per project.
Envelope limits are 256 keys per instance, 8,192 per scene and 65,536 per project,
counting inactive and zero-weight instances. Existing clip, transform and
source-size limits remain. Prepared snapshots
share referenced library tracks; instances and frames do not duplicate their
keys. Unused definitions are not prepared. Export preflight conservatively
counts inactive and zero-weight layers, scans, scalar/bone searches, fade
arithmetic, loop mapping, joint-selection checks and additive combination under the
existing one-million-unit per-scene work budget. Preview also checks bounded
per-frame sample work. These are protective estimates, not measured performance.

## Validation scope and remaining work

Regressions cover independent expected offsets, overlap order, missing bones,
rotation and scale, endpoint windows, seek order, source references and deletion,
saved projects and source history. Actual SVG/PNG fixtures compare independently
authored expectations and sample-clock preview including speech/root stages.
Loop/mask regressions additionally compare two cycle resets, repeated/backward
seeks, the excluded instance endpoint, preserved unselected joints, source-order
blending, project round trips and default compatibility. Looping and joint masks
passed PR #43 CI; additive numeric/schema/editor/evaluation and independent rendered
regressions passed PR #45 CI. Exact evidence is recorded in master progress.
Physical Mac motion quality, production performance, custom/textured rigs,
deformation, normalized/arbitrary-rig mixing and cross-project exchange remain open.
This change does not complete GPU scene parity or recognizer-result memoization.


## Normalized original-rig override groups

Set `normalized_group: reach` on consecutive override instances for one actor.
The group ID uses 1–64 ASCII letters, digits, `_` or `-`. The whole group must
occupy one contiguous block in the saved scene stack, and its ID cannot be reused
later in that scene or for another actor. An additive row cannot join a group.
Omitting the field preserves the existing sequential override/additive behavior.
The native live clip editor exposes a group checkbox and ID; Apply validates the
whole candidate block atomically. Reset timing preserves the selected group.

Each group replaces its block at that exact stack position. For each joint,
contributors are only active instances with positive effective envelope weight,
a selected joint and an authored track for that joint. Absent tracks and excluded
joints contribute no weight. Let `S` be their total effective weight:

| Weight total | Result |
| --- | --- |
| No positive contributors | Preserve the preceding accumulated offset exactly. |
| `0 < S < 1` | Sum the weighted clip offsets and retain `(1 − S)` of the preceding offset. |
| `S >= 1` | Sum clip offsets with each weight divided by `S`; no preceding residual remains. |

Translation and scale use the component-wise weighted sum. Rotation unwraps each
sample by the shortest signed arc around the preceding accumulated authored
rotation, for every total weight. The anchor never switches when the total
crosses one, so a continuous weight fade cannot introduce a reference-change
snap. An exact half turn chooses the positive arc. Contributors are summed in
stable instance ID order, so reordering rows within a group preserves the result,
including floating-point accumulation. This is authored-base anchored local
rotation blending, not a directional mean: clips at `+3` and `−3` radians around
base zero cancel, including immediately below and above total weight one. The
combined offset still must satisfy the existing bounds; invalid composition
returns a typed error.

For example, base head `x = 8`, two clips at `x = 40` and `x = 100`, each with
weight `0.25`, produce `8 × 0.5 + 40 × 0.25 + 100 × 0.25 = 39`. At weight `1`
each they produce `70`. If the second clip masks out the head, the head uses only
the first clip's weight. Fades, keyed weights, half-open activity and original-speed
loops are sampled once using the existing scene clock.

This is mixing for the original seven-joint offset rig. Custom rigid rest shapes
are a separate opt-in character format; original-rig bone tracks, live clips,
gesture events, animated procedural strengths and nondefault static strengths
are rejected for those actors rather than silently applying invisible animation.
Root movement and root transform tracks remain supported.
