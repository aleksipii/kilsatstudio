# GPU production workstream handoff after PR #43

Owner: GPU production agent; integration owner: parent. Inspected 2026-10-18 on
`feat/studio-open-workstreams`. This is a source-based assignment and implementation
specification, not production implementation or device acceptance. Scope owned by
this handoff is rendering, GPU resource ownership and renderer integration. Shared
scene/art schema changes require their owners; no second evaluator or clock.

## Inspected current state

PR #43 already supplies genuine evaluated geometry rendering. Do not reopen that
implementation as fixture-only work. `scene_commands::build` validates evaluated
scenes, camera/root transforms, skeleton, expressions, visemes, actor order,
letterboxing, complete-scene opacity and bounded command/fragment work.
`gpu_scene::GpuSceneRenderer` reuses device, queue and pipeline; each render owns
and releases target/readback, with one mutable in-flight render. WGSL evaluates
analytical shapes rather than uploading a CPU final frame. Shader validation,
command tests and `compare_decoded_rgba` metric tests already exist.

The experiment is default-disabled under `motion-render`'s `gpu` feature, locked
to wgpu 24.0.5. `Labels::Require` rejects actor labels; only explicit
`OmitAcknowledged` permits omission. `reject_artwork_or_sequence` reports unsupported
art/textures/masks/blends/transitions. The probe emits raw RGBA and SVG reference,
not a decoded PNG comparison. Its geometry AA and smile joins differ from SVG;
metrics establish no acceptance tolerance. The current full-screen shader loops
all commands per pixel, an important production scaling constraint.

Existing limits are 64–2048 per output dimension, 4096 commands, 256 mouth entries,
64 Mi command-fragment visits, 32 MiB padded readback and a 10-second cooperative
initialization/readback deadline. The owned-resource target is 128 MiB, excluding
driver overhead. Device/pipeline reuse is implemented; persistent texture, atlas,
and scene-target reuse is not. Cancellation cannot interrupt a blocked driver.

CPU `motion-render/src/lib.rs` already emits ordered background/foreground art,
active half-open timing, actor attachment translation, images, rectangle/ellipse/
polygon clips, animated feathered soft ellipses and normal/multiply/screen styles.
These are completed CPU features, with GPU parity remaining. Image masks, polygon
feathering and groups remain broader artist-feature work, not existing CPU parity.
`ImageResource::rgba()` in `motion-art/src/resource.rs` exposes decoded RGBA.

CPU `motion-export/src/media.rs::rasterize_scaled` parses SVG using default usvg
options and rasterizes with resvg/tiny-skia. SVG actor labels request generic
sans-serif; this function does not explicitly load a font database. Native egui
text and emitted SVG text therefore do not establish a shared reproducible font
baseline. Font presence and shaping must be made explicit before label acceptance.
`ScenePreview::render_with_mouths` evaluates prepared state at the shared sample
clock, applies speech and uses that CPU path. `sequence.rs::compose` samples
`SequenceTimeline`, renders one or two complete opaque scene images, then rounds
weighted byte-channel sums. Existing PNG/SVG sequence and advanced compositing
regressions are reference behavior. Production desktop/export do not select GPU.

## Ordered remaining implementation

1. Establish reproducible labels and image conventions. Select a locally supplied,
   redistribution-reviewed font with explicit hash/face/fallback policy; use the
   same font input and shaping/layout in CPU references and GPU glyph coverage.
   Specify baseline, centering, actor-local transform, Unicode/missing-glyph errors,
   bounded glyph/atlas allocations and eviction. CPU rasterized individual glyph
   coverage uploaded to an atlas is permitted; uploading a CPU final scene is not
   GPU scene rendering. Keep missing-font failure explicit. Document straight
   source image RGBA, premultiplication, target/texture formats, sampling and
   transfer functions, including whether current CPU byte-space composition is
   preserved. Do not silently replace CPU crossfade/color behavior with linear
   lighting and call it parity.
2. Add bounded textured artwork and normal composition: original decoded image
   resources, stroke/shape content, plane/document order, visibility, half-open
   timing, attachment translation, shared camera/canvas clip and scene fade.
   Validate the complete request before adapter initialization or output creation.
   Reuse decoded resources by content identity with explicit residency/eviction
   limits; no per-frame asset decode on the desktop event loop.
3. Add isolated layer targets for hard rectangle/ellipse/simple-polygon masks,
   then existing animated soft ellipse coverage/feathering. Sample mask keys with
   the existing art semantics, including zero feather and scene-local boundaries.
   Implement normal/multiply/screen against the correct composed backdrop, with
   opacity and premultiplied alpha applied once. Budget intermediate targets,
   polygon work, image occurrences and texture bytes before allocation. Future
   image masks/groups/polygon feathering require coordinated CPU/schema support
   and explicit capability errors until available.
4. Add complete-frame cut/crossfade composition. Consume existing sequence sample
   contributions in `motion-export`, never infer new transition times in render.
   Compose each fully rendered scene including art/masks/blends/fade before
   applying contribution weights. Preserve existing opaque byte-channel rounding
   unless an explicit cross-backend behavior change is separately reviewed.
5. Introduce explicit shared production backend selection only after coverage.
   A renderer session owned by a bounded worker must serve selected-scene and
   project preview plus PNG/MP4 frame production with identical prepared scenes,
   mouths, coordinates and samples. CPU SVG remains vector output. Reuse GPU
   sessions across frames, propagate init/unsupported/device-loss/cancel errors,
   and expose the selected backend. No silent fallback reported as GPU success.
   Agree desktop worker cancellation/stale-result handling with its owner.
6. Optimize only against retained full-scene measurements. The current fragment
   loop/work ceiling can reject ordinary larger scenes; consider bounded geometry
   draws or tiles after correctness evidence. Separately review supporting CPU's
   broader dimensions; do not remove bounds merely to accept 8192 output.

## First actionable coherent increment

Implement explicit label resources and GPU glyph composition while leaving the
existing primitive experiment API and packaged fixture schema compatible.

Proposed interfaces (design targets, not existing declarations):

- In `motion-render/src/scene_commands.rs`, add `build_with_labels` accepting an
  immutable validated label resource/layout input alongside current scene,
  dimensions and mouths. Preserve existing `build(..., Labels)` behavior.
  Result commands include ordered glyph quads and checked atlas references with
  the same actor/camera/root transform as geometry. Validate text, font identity,
  glyph count and atlas dimensions before GPU work.
- Add `motion-render/src/text.rs` only for actual shared font/layout preparation,
  not empty scaffolding. Return bounded glyph coverage plus placements and font
  identity, with typed missing-font/missing-glyph/budget errors. Select a shaping
  dependency only after license/API review; do not assume ASCII-only actor IDs.
- In `gpu_scene.rs`, add an explicit render entry point consuming validated glyph
  resources; upload bounded glyph atlas data and composite glyph coverage in
  command order. Account atlas and temporary bytes with existing 128 MiB target.
- Extend `gpu_scene_probe.rs` with an explicit local font argument and recorded
  font hash; preserve omission acknowledgement for historical geometry-only runs.
  Add a reproducible decoded CPU-reference/PNG comparison harness at the
  `motion-export` test boundary, where resvg already belongs. Do not introduce a
  dependency from `motion-render` to `motion-export`.
- Coordinate `motion-export/src/media.rs` font options with its owner so the CPU
  reference uses that same font. Preserve existing public defaults until callers
  opt into deterministic resources; full production eligibility requires this
  mode. No desktop backend option in this first increment.

Deliverable: actor labels actually compose through GPU texture sampling with
bounded resources and deterministic reference setup. Geometry omission remains
explicit in the old experimental mode; fonts do not imply art or scene parity.
A local approved font is a prerequisite for real pixel evidence, not grounds to
silently omit labels. Font selection and dependency/license approval must be
resolved before this increment can claim label acceptance.

## Tests and acceptance, kept separate

| Gate | Required evidence | Meaning |
| --- | --- | --- |
| Adapter-free | Existing default regressions; new font identity/layout/Unicode/missing glyph/budget tests; WGSL Naga validation; command ordering, alpha/color conversion and checked allocation tests; malformed/unsupported/cancel requests leave output absent | Implementation contracts only; no GPU execution |
| Explicit real GPU | Decoded CPU/GPU PNG+RGBA at fixed samples/resolutions, odd widths, resize/reuse, transparent glyph/image edges, transformed strokes, art timing, animated feather masks and backdrop blends; exact backend/device/commit/font/resource hashes and max/RMS/changed fraction/edge/interior metrics | Measured raster behavior on identified hardware; freeze justified tolerance before acceptance |
| Shared preview/export | Same explicit GPU backend and prepared sample/mouth state gives identical decoded preview/export pixels at equal dimensions; cuts/crossfade boundary, pre-roll, random/repeated seek; bounded cancellation/error cleanup; PNG/MP4 encoder checks separately | Shared production pipeline behavior, after full feature coverage |
| Physical Metal | Packaged helper fixture plus full scene corpus on supported physical Mac; package/build hash, native architecture, OS/chip/adapter, raw images/reports; actual device loss, cancel/readback/repeat and native preview/export observation | Physical acceptance; macOS CI compile/signing or fixture success alone is insufficient |
| Performance | Identical corpus/samples/resolutions; cold/warm CPU and GPU wall time, GPU timestamps when supported, peak memory, upload/readback costs separated from PNG/FFmpeg; two-scene overlap and cancellation responsiveness | Retained measurements, not presumed acceleration |

For code changes run repository-required fmt/check/Clippy/tests, plus optional
`motion-render --features gpu` check, strict Clippy and adapter-free tests. Keep
adapter requests out of default CI. Reuse reference cases
`advanced_mask_blending_and_visibility_match_export`,
`advanced_clips_and_backdrop_blends_are_real_raster_pixels`,
`timed_art_is_composited_before_complete_frame_crossfade`,
`vector_sequence_raster_matches_png_cuts_crossfades_and_mask_namespaces` and
`preview_matches_export_pixels_and_scales_original_coordinates`; add PR #43
soft-ellipse boundary/feather cases rather than treating old hard-mask tests as
coverage of feathering.

## Dependencies and exact blockers

- Renderer owner owns `motion-render` changes; export owner coordinates font
  reference settings and backend session APIs; desktop owner coordinates bounded
  worker/session lifetime. Artist owner retains `motion-art` schema, future image
  masks/groups and mask semantics. Sequence owner retains `SequenceTimeline`.
- No deterministic approved font input or shared explicit font setup is established
  by the inspected CPU raster function. Font data/provenance and shaping choices
  block reproducible label parity; this inspection acquired no font/dependency.
- GPU commands currently cannot carry textures/layer targets/masks/blends or
  sequence contributions. These are code gaps, not merely missing hardware runs.
- No measured tolerance, complete-scene GPU comparison, production selector,
  full-scene physical Metal evidence or performance evidence was produced here.
- Physical Mac execution, real GPU device failure and hardware benchmarks are
  unavailable under this planning-only assignment. They must remain open with
  actual retained evidence; CI/source inspection cannot substitute.
- Existing work/resource limits need extension accounting for atlas residency,
  image uploads, isolated layers and two complete scene targets before enabling
  those features. CPU larger-output support does not imply GPU support.

This handoff performed source/document inspection only, no downloads, adapter or
physical device execution, production edits, benchmarks or acceptance test runs.
