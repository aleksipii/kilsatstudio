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
and artwork remain unchanged. Key times and easing are copied unchanged;
there is no automatic retiming. The target scene must be at least as long as
the captured source scene.

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
into `scenes[].composition.bone_tracks`. Unused clips have no rendering effect.

Limits are 64 clips per project, a positive duration up to 600 seconds, 1–4096
strictly increasing keys per bone and 65,536 keys across the library. Existing
finite transform and original-rig offset bounds also apply. Source-edit input
and output remain limited to one MiB; exceeding a limit preserves the document.
Empty clips, unknown bones/fields, invalid names and out-of-duration keys fail
validation. Existing projects without `animation_clips` use an empty library.

Tests compare copied tracks with independent expected SVG/PNG output, shared
sample-clock preview, repeated/backwards seeking and project round trips.
Arbitrary rigs, textured parts, live clip instances, clip retiming, layering of
multiple custom clips and cross-project library exchange remain open. No GPU,
physical Mac or perceptual motion-quality acceptance is established by fixtures.
