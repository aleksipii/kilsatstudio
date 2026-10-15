# Artwork and keyframe acceptance

This stage adds a bounded foreground artwork editor and actor root animation. It
is a partial implementation of the master plan, not a complete painting,
compositing, skeletal animation or GPU rendering system.

## Scope and contract

- Artwork is stored in version 2 `.kilsat` documents. Version 1 documents migrate
  explicitly to an empty artwork document; unknown versions and unknown fields
  fail rather than silently dropping data.
- Layers have stable identifiers, scene references, visibility, opacity and
  document order. Content is a PNG/JPEG image, freehand stroke, rectangle or
  ellipse. A mask is a rectangular clip, not an arbitrary painted mask.
- Image files are local, explicitly resolved assets. The decoder accepts PNG and
  JPEG only, checks header dimensions before decoding, and bounds encoded bytes,
  dimensions and decoded pixels. Decoder allocation limits are best effort.
- Actor attachment follows actor translation only. It does not constitute a
  textured joint rig or inherit actor root rotation and scale.
- Root keyframes add translation to the existing move result and supply rotation
  and positive scale. Interpolation uses the shared scene clock; times must be
  increasing and within the scene. This is not a per-joint keyframe editor.
- Artwork is foreground content, painted after procedural actors. Its ordering
  does not interleave individual artwork layers with individual actors.

## Review and automated evidence

The code review checked atomic artwork mutations, document reference validation,
strict version migration, bounded image decoding, and shared root-transform
sampling. Tests and execution evidence are reported in the pull request after
integration; this document alone does not certify a successful build or run.

The final read-only integration audit confirmed guards for structured source
changes and source Undo/Redo, rollback of invalid artwork UI changes, inventory
worker comparison against the previous artwork and assets, context-based artwork
history invalidation, scene-filtered selection, and an explicit image reload
button. This is code inspection, not a native UI execution result.

The integration acceptance checks are:

- Invalid layer geometry, duplicate IDs and invalid references preserve the
  previous document. Referenced image assets cannot be removed accidentally.
- Loading, saving and undoing preserve artwork; plain screenplay saving must
  refuse silent artwork loss. Scene/character deletion and scene rename must
  preserve or explicitly reject references.
- A worker result prepared for an older document cannot overwrite newer edits.
  Missing/corrupt images produce actionable errors rather than blank success.
- Preview and export use the same scene camera, fade, rectangular clipping,
  attachment rule and keyframe clock. Unsupported artwork semantics stay explicit.
- Export validates resources before creating output and respects existing frame,
  image and concurrency budgets; failure removes only its own partial output.

## Manual checks still required

On a real Mac, open both a version 1 and version 2 project. Import a transparent
PNG and a JPEG, add strokes and shapes, reorder layers, change opacity and clip,
attach a layer to an actor, then scrub and play. Compare the native preview with
SVG, PNG and MP4 exports at the same timestamps. Test Undo/Redo, save/reopen,
missing images, invalid relinks, scene rename/deletion and character deletion.
Delete an image layer, remove its asset reference, and try artwork Undo; the
operation must not resurrect an invalid reference. Switch scenes while a layer
is selected and verify its controls cannot modify a layer in the previous scene.
Replace image bytes at the same path and exercise the explicit reload path.

Verify that long image loads and exports keep the UI responsive, cancellation
works, and stale results never replace newer edits. Inspect high-DPI rendering,
image alpha edges, nonuniform actor scaling and rotated facial features.

Native GPU/offscreen acceleration, arbitrary masks, blend modes, textured joint
rigging, vector node editing and production-scale painting remain open. Linux
unit tests and SVG assertions do not establish native Mac visual parity,
Apple Silicon performance, actual audio-device behavior or Kokoro speech quality.
