# Project-local authored bone clips

Use Inspector → **Authored clip library** to reuse authored offsets for the
original seven-bone rig. First create keys with the per-bone editor, then give
the clip a name and click **Capture bone tracks**. Capturing copies all authored
bone tracks for the selected actor and the source scene's duration into the
project library. Names use 1–64 ASCII letters, digits, `_` or `-`.

Select a library clip to see its duration, bones and key count. To use it,
select the target actor and scene, confirm **Replace all bone tracks**, then
click **Apply selected bone clip**. This replaces every authored bone track on
the target, including removing target tracks not present in the clip. Other
actors, root movement, procedural strengths, facial animation, speech, camera
and artwork remain unchanged. By default, key times and easing are copied
unchanged and the target scene must be at least as long as the source scene.
Explicit retiming is described below.

## Explicit start time and duration

Enable **Scale and offset copied key times**, then enter the start time and
positive duration in scene seconds. **Reset copied key timing** restores start
zero and the original clip duration in the draft, clearing replacement approval.
Confirm target replacement after editing
those values and Apply. All keys are mapped with
`new_time = start + (old_time / clip_duration) * duration`; transform values and
outgoing easing remain unchanged. For example, a two-second clip applied at
three seconds with duration one maps keys at 0, 1 and 2 to 3, 3.5 and 4 seconds.
The stored library clip remains unchanged. Compression can fit a longer source
clip into a shorter target scene.

Start must be finite and nonnegative, duration finite and positive, and their
sum must fit inside the scene. If floating-point precision would collapse two
keys onto one time, the edit fails atomically. There is no silent clamping,
trimming or sorting. The operation still replaces all target bone tracks.
These are copied keyframes, not a live clip window: ordinary bone-track
sampling holds the first/last key outside its range. Retiming does not activate
or deactivate a pose at the chosen bounds, and does not shift speech or events.

The clip is copied into editable scene tracks, not linked. Further per-bone
editing, replacing the library entry or deleting it will not change previous
copies. Deleting the source character/scene leaves captured clips usable.
Replacing an existing library name and deleting a clip each require explicit
confirmation. Source/scene/actor changes discard pending names and confirmations.
Every operation is a validated source edit with unified Undo/Redo. YAML
formatting is normalized and comments are removed, as with the existing editors.

Save the `.kilsat` project after applying changes. The library lives in its
MotionScript source; no external asset or new project format is needed:

```yaml
animation_clips:
  head_tilt:
    duration: 2
    bone_tracks:
      head:
        - {at: 0, rotation: 0}
        - {at: 1, rotation: 0.2, easing: smoothstep}
        - {at: 2, rotation: 0}
```

Library definitions alone do not play animation. Apply a clip to copy its tracks
into `scenes[].composition.bone_tracks`, or add a [live clip layer](live-clip-blending.md)
to retain an active reference with ordered blending. Unused clips have no rendering effect.

Limits are 64 clips per project, a positive duration up to 600 seconds, 1–4096
strictly increasing keys per bone and 65,536 keys across the library. Existing
finite transform and original-rig offset bounds also apply. Source-edit input
and output remain limited to one MiB; the native desktop source-edit limit is
100 KB. Exceeding either applicable limit preserves the document.
Empty clips, unknown bones/fields, invalid names and out-of-duration keys fail
validation. Existing projects without `animation_clips` use an empty library.

Tests compare copied tracks with independent expected SVG/PNG output, shared
sample-clock preview, repeated/backwards seeking and project round trips.
Arbitrary rigs, textured parts, animated layer weights/fades, looping and
cross-project library exchange remain open. No GPU,
physical Mac or perceptual motion-quality acceptance is established by fixtures.
