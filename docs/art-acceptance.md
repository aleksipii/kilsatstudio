# Artwork and keyframe acceptance

This stage adds a bounded artwork editor and actor root animation. It
is a partial implementation of the master plan, not a complete painting,
compositing, skeletal animation or GPU rendering system.

## Scope and contract

- Artwork is stored in version 2 `.kilsat` documents. Version 1 documents migrate
  explicitly to an empty artwork document; unknown versions and unknown fields
  fail rather than silently dropping data.
- Layers have stable identifiers, scene references, visibility, opacity and
  document order. Content is a PNG/JPEG image, freehand stroke, rectangle or
  ellipse. Clips support legacy rectangles, ellipses and validated simple polygons;
  Normal/Multiply/Screen blend modes use the shared SVG/resvg CPU path.
  Feathered/painted masks and mask images remain open.
- Image files are local, explicitly resolved assets. The decoder accepts PNG and
  JPEG only, checks header dimensions before decoding, and bounds encoded bytes,
  dimensions and decoded pixels. Decoder allocation limits are best effort.
- Actor attachment follows actor translation only. It does not constitute a
  textured joint rig or inherit actor root rotation and scale.
- Root keyframes add translation to the existing move result and supply rotation
  and positive scale. Interpolation uses the shared scene clock; times must be
  increasing and within the scene. The [per-bone editor](per-bone-editor.md) also
  authors stable procedural-bone transform tracks; textured joint deformation
  and arbitrary rig authoring remain open.
- Artwork has background and foreground planes, painted before/after procedural
  actors. Ordering within each plane follows document order. Optional visibility
  intervals use `[start, end)` on the shared scene clock; there is no per-actor
  artwork interleaving.

## Review and automated evidence

The code review checked atomic artwork mutations, document reference validation,
strict version migration, bounded image decoding, and shared root-transform
sampling. Tests and execution evidence are reported in the pull request after
integration; this document alone does not certify a successful build or run.

The final read-only integration audit confirmed guards for structured source
changes and chronological source/art/inventory Undo/Redo, rollback of invalid
artwork UI changes, stale inventory-worker baseline checks, scene-filtered
selection, and an explicit image reload button. See [unified history](unified-history.md)
for current snapshot bounds and metadata-only restoration. This is code inspection, not a native UI execution result.

The integration acceptance checks are:

- Invalid layer geometry, duplicate IDs and invalid references preserve the
  previous document. Referenced image assets cannot be removed accidentally.
- Loading, saving and undoing preserve artwork; plain screenplay saving must
  refuse silent artwork loss. Scene/character deletion and scene rename must
  preserve or explicitly reject references.
- A worker result prepared for an older document cannot overwrite newer edits.
  Missing/corrupt images produce actionable errors rather than blank success.
- Shared CPU preview and export use the same scene camera, fade, geometric clipping,
  attachment rule and keyframe clock. Unsupported artwork semantics stay explicit.
- Export validates resources before creating output and respects existing frame,
  image and concurrency budgets; failure removes only its own partial output.

## Manual checks still required

On a real Mac, open both a version 1 and version 2 project. Import a transparent
PNG and a JPEG, add strokes and shapes, reorder layers, change opacity and clip,
attach a layer to an actor, then scrub and play. Compare the native preview with
SVG, PNG and MP4 exports at the same timestamps. Test Undo/Redo, save/reopen,
missing images, invalid relinks, scene rename/deletion and character deletion.
Delete an image layer, remove its asset reference, then use global Undo twice:
the reference must return before the layer, without rewriting physical files.
Redo must remove the layer before the reference. Switch scenes while a layer
is selected and verify its controls cannot modify a layer in the previous scene.
Replace image bytes at the same path and exercise the explicit reload path.

Verify that long image loads and exports keep the UI responsive, cancellation
works, and stale results never replace newer edits. Inspect high-DPI rendering,
image alpha edges, nonuniform actor scaling and rotated facial features.

An [optional GPU fixture](gpu-offscreen.md) exists, but production GPU rendering,
actual GPU execution, feathered/painted masks, textured joint rigging, vector node
editing and production-scale painting remain open. [Advanced compositing](advanced-compositing.md)
describes implemented geometric masks and CPU blend modes. The selected-scene
direct canvas is suppressed for active advanced layers; inspect those frames
through Whole project preview and compare exports. Linux
unit tests and SVG assertions do not establish native Mac visual parity,
Apple Silicon performance, actual audio-device behavior or Kokoro speech quality.

## Timed background-image workflow

1. Import a local PNG/JPEG into a saved `.kilsat` project, then choose
   **Place background image** in **Artwork / layers**. It initially fills the
   project canvas; adjust its rectangle to change framing.
2. Select the layer, choose **Background** and enable **Visible during interval**.
   Enter `0 <= start < end <= scene duration`. A normal placed image remains in
   the foreground unless changed.
3. Scrub before start, exactly at start, just before end and exactly at end. Verify
   the image is absent, present, present and absent respectively. Verify actors
   remain above it and foreground artwork remains above actors.
4. Add camera motion, clip/opacity changes and scene-edge fades. Check that a fade
   darkens the completed composition, without the base color leaking through an
   opaque image. Save, reload, Undo and Redo the plane and interval.
5. Export SVG/PNG/MP4 and compare the same frame times. Missing future image
   resources and intervals beyond scene duration must fail before creating output.

These are interactive Mac acceptance steps; automated shape/pixel tests do not
claim they were performed on a physical Mac. Scene crossfades are implemented
in the whole-project CPU preview/export path; real Mac visual acceptance remains
separate. The user-reported M1 Kokoro speech success establishes a first speech
check, not artwork, device-clock, offline-repeatability or lip-sync acceptance.
