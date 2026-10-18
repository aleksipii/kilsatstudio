# Live authored clip layers

Inspector → **Live authored clips** creates references to the project's named
bone clips. Create the library clip first using the per-bone editor and
[clip library](animation-clip-library.md). Select the target actor, choose
**New live clip instance**, enter a scene-local instance ID, clip, start, duration
and weight, then explicitly Apply. Existing instance IDs remain fixed while editing.

The ordered rows show the entire scene, including other actors. Select an actor
to edit or move its rows. Up/Down changes the row's position in the complete
scene list. Removing an instance needs confirmation and leaves its definition
and independently copied tracks intact. Reset discards an unapplied draft.
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
its end excluded. Clip time is `(scene_time - start) / duration * clip.duration`.
Within that window, tracks hold endpoint values if their keys do not span the
clip duration. Outside it the layer contributes nothing. Hard activation edges
can pop; no fade, looping or automatic smoothing is added.

Start with the actor's ordinary authored local bone offsets, using identity for
absent bones. Visit active rows in saved order. On each bone present in a clip,
interpolate the current offset toward the sampled clip offset by its constant
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

## Live references versus copies

Library replacement affects every live reference and displays affected scene
and instance counts. Instance start/duration stay fixed; a changed definition
duration changes its sample mapping. Previously copied or retimed tracks remain
independent. A referenced definition cannot be deleted: remove its live instances
first. Removing a character removes its instances atomically. Duplicating a
scene keeps scene-local IDs and shares the clip definitions.

## Validation and limits

IDs use 1–64 ASCII letters/digits/underscores/hyphens and are unique within the
scene. Clip and actor references must exist. Start must be finite and nonnegative;
duration finite and positive; the finite end must exceed start and fit the scene.
Weights must be finite in `[0,1]`. Zero-weight instances still require valid
references and count toward limits.

Limits: 128 instances per scene, 32 per actor per scene and 1024 per project.
Existing clip, key, transform and source-size limits remain. Prepared snapshots
share referenced library tracks; instances and frames do not duplicate their
keys. Unused definitions are not prepared. Export preflight conservatively
counts inactive and zero-weight layers, scans and track searches under the
existing one-million-unit per-scene work budget. Preview also checks bounded
per-frame sample work. These are protective estimates, not measured performance.

## Validation scope and remaining work

Regressions cover independent expected offsets, overlap order, missing bones,
rotation and scale, endpoint windows, seek order, source references and deletion,
saved projects and source history. Actual SVG/PNG fixtures compare independently
authored expectations and sample-clock preview including speech/root stages.
Physical Mac motion quality, production performance, animated weights/fades,
loops, masks, custom/textured rigs and cross-project exchange remain open.
This change does not complete GPU scene parity or recognizer-result memoization.
