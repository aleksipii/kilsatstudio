# Artist controls for procedural animation layers

Each scene can store separate posture, walk and gesture strengths for each actor.
Values are finite numbers from 0 to 1. Missing actors/fields default to full
strength (1), preserving existing projects. The three strengths are independent
additive controls; they do not need to sum to 1.

```yaml
composition:
  clip_weights:
    EMMA: {posture: 0.4, walk: 0.8, gesture: 0.5}
```

`EMMA` must be a character defined by the screenplay. Unknown actors/fields,
booleans, nonfinite values and out-of-range strengths are rejected.

## What the controls change

- **Posture** scales the displacement from rest toward the current blended
  emotion posture. Closely spaced emotion changes keep their continuous blend.
  Facial expression continues to use the actor's emotion state.
- **Walk** scales the skeletal walk-cycle offsets while the Move event controls
  root position and timing. Setting Walk to zero therefore allows a sliding
  pose along the same authored path.
- **Gesture** scales the active wave/point/nod/shrug envelope. The strongest
  active gesture is selected using the existing deterministic rule.

After these layers, authored parent-local bone offsets are applied and the rig
hierarchy is evaluated. The existing viseme stage adds speech head motion on the
same audio sample clock. Root transforms and camera tracks retain their own
controls. These strengths use the shared evaluator for native preview and
SVG/PNG/MP4 scene and project export.

## Native authoring and project persistence

Select an actor in the timeline, then open Inspector → **Animation layer
strengths**. Adjust the detached draft and click **Apply clip intensities** to
validate and commit one document edit. **Reset draft** reloads its original
values; **Restore full strength** sets all three to 1 in the draft. Apply the
restored defaults to remove that actor's override from the scene.

Apply validates the whole screenplay and normalizes YAML formatting, including
removing comments, as with the existing source editing tools. Undo/Redo restore
the exact source transactions through the unified history. Changing source,
scene or selected actor reloads saved strengths and discards an unapplied draft.
Invalid drafts remain visible for correction and cannot be applied.

Save the project after Apply. Strengths live in MotionScript source inside
`.kilsat`; no project format migration or external animation asset is needed.
Deleting a character also removes its scene-local strength overrides.

## Animated strengths

Inspector → **Animated layer strengths** edits a separate per-actor track. Add
keys, set their scene-local times, strengths and outgoing easing, then click
**Apply animated strengths**. Times must strictly increase within the scene;
at most 256 keys are accepted. Linear interpolates each strength independently,
smoothstep eases both ends, and hold keeps the earlier value until the next key.
Before the first key and after the last, the endpoint value is held. Seeking
samples directly from scene time, without accumulating frame history.

```yaml
composition:
  clip_weights:
    EMMA: {posture: 0.4, walk: 0.8, gesture: 0.5}
  clip_weight_tracks:
    EMMA:
      - time: 0
        weights: {posture: 0.4, walk: 0.8, gesture: 0}
        easing: smoothstep
      - time: 2
        weights: {posture: 1, walk: 0.2, gesture: 1}
        easing: hold
```

A track overrides all three static strengths for that actor. Missing fields
inside `weights` default to 1. Remove all keys in the draft and Apply to remove
the track and resume the saved static strengths. A serialized empty track is
rejected. Reset restores the loaded draft; removing a draft does not write the
source until Apply. Source, actor or scene changes discard unapplied keys.
Track changes share the same Undo/Redo, project storage and preview/export
pipeline as static strengths. Shortening a scene is rejected if it would leave
keys outside its duration; edit the track first.

## Validation and remaining scope

Tests cover missing/default strengths, invalid data, independent zero/partial
layers, seek determinism, close emotion transitions, authored offsets, per-actor
isolation, project source round trips, stale drafts and source Undo/Redo. Export
fixtures compare default frames and actual weighted SVG/PNG frames with shared
preview at the same speech sample positions.

This implements controls for three existing procedural layers. The separate
[project-local bone clip library](animation-clip-library.md) copies authored
tracks between actors/scenes. Live blending of arbitrary authored clips, arbitrary rigs, textured/deformed
parts, inverse kinematics, advanced mask painting and physical Mac visual/motion
acceptance remain open. No perceptual motion-quality or GPU-parity gate is
completed by synthetic fixtures.
