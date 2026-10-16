# Scene transitions: inspected implementation contract

Status: design contract only. **No scene transitions are implemented by this report.**

## Existing behavior verified in source

- `motion_script::Script.scenes` preserves ordered scenes; `Scene` has duration, events and composition, but no transition field.
- `motion_scene::evaluate(script, scene_index, time_seconds)` evaluates one scene on its local clock.
- `EvaluatedScene` carries camera, whole-canvas opacity, actors and local time. Existing fade-in/out fades that scene over black; it is not a crossfade between scenes.
- Camera keys, actor tracks, background color keys and artwork active intervals already evaluate deterministically on scene-local time.
- `motion_render::render_svg_with_art` composes background/foreground artwork and actors for one scene, with scene fade enclosing the composition.
- `motion_voice::export::render_scene` produces 24 kHz mono audio and sample-based mouth intervals for one scene; it rejects speech beyond that scene.
- `motion_export::encode_frame` uses integer `frame * 24000 / fps` samples before converting to scene time. Sequence/MP4 exports and desktop scene selection remain scene-specific.

## Next scoped implementation: cuts and linear crossfades

1. Extend `Scene` with an optional outgoing transition, defaulting to a cut for old YAML. Proposed variants: `Cut` and `Crossfade { duration }`; these names are proposed interfaces, not present APIs.
2. Validate finite positive crossfade duration, a following scene, and integer-sample overlap strictly shorter than either participating scene. Reject a scene whose incoming plus outgoing overlap exceeds its duration, preventing triple-scene overlap. Reject durations rounding to zero samples and checked-arithmetic overflow.
3. Add a compiled sequence timeline in `motion-scene`: scene sample lengths `round(duration * 24000)`, start offsets, overlap lengths and total sample length. Require positive rounded scene lengths. Start of scene B equals end of scene A minus their overlap; cuts have zero overlap. Total length equals summed lengths minus overlaps.
4. Proposed `sample_sequence(script, timeline, global_sample)` returns one or two `(scene_index, local_sample, weight)` entries. Public playback interval is `[0, total_samples)`. At a cut boundary only the new scene contributes. In overlap starting at S of length N, incoming weight is `(sample-S)/N`, outgoing weight `1-incoming`; at S+N only incoming remains. Reject out-of-range samples; do not depend on previous playback state.
5. Video frame scheduling remains `floor(frame * 24000 / fps)` globally; determine frame count from the quantized total duration and exclude samples at/after EOF. Global seek and playback use this same clock, never separately accumulated scene times.

## Visual semantics

- Evaluate each contributing scene with its own local sample time, camera and visemes, then render complete scene images before blending. Do not interpolate actors or cameras across unrelated scenes.
- Apply each scene's existing fade over black before transition blending. Blend complete opaque scene images by the shared weights; simply stacking two opacity-reduced images with source-over is not an equivalent linear crossfade.
- Existing scene-local artwork intervals, camera projection, actor attachments, z order and background steps remain intact. Both background and foreground participate in transitions and scene fades. Render clipping and image resources remain scene-specific.
- New SVG composition must namespace internal clip/image IDs per scene or use isolated nested image documents; repeated `canvas` and `artN` IDs must not collide.
- Preview and SVG/PNG/MP4 export must consume the same evaluated sequence contributions and blend definition. A preview-only visual effect is not completion.

## Audio and duration semantics

- Offset existing rendered scene audio onto the compiled global timeline. During overlaps apply the same linear weights to samples; outside overlap copy the single contributing scene. Do not perform an independent FFmpeg audio crossfade with separately rounded durations.
- Preserve within-scene mixing/gain and document the existing clipping policy; explicitly bound/reject nonfinite output. Do not silently stretch, truncate or regenerate dialogue to fit an overlap.
- Mouth samples remain tied to each scene's local audio sample. Equal actor IDs in two scenes must retain separate contribution identities during overlap.
- Add a bounded sequence-audio allocation budget and checked sample/byte arithmetic before starting jobs. Cancellation and partial-output cleanup must match existing exports.

## Exact next implementation surfaces

| File | Change |
| --- | --- |
| `crates/motion-script/src/lib.rs` | Transition serde/defaults, validation and source-preserving transition edit API. |
| `crates/motion-scene/src/lib.rs` | Compiled global sample timeline and stateless sequence sampling; retain scene evaluator. |
| `crates/motion-voice/src/export.rs` | Bounded sequence audio/manifest assembly from existing rendered scenes. |
| `crates/motion-render/src/lib.rs` | Complete-scene composition with collision-free IDs and defined linear blend. |
| `crates/motion-export/src/lib.rs` | Shared global frame scheduler, sequence validation and sequence export APIs. |
| `crates/motion-export/src/media.rs` | Sequence MP4 wrapper using the shared visual/audio pipeline. |
| `apps/studio-desktop/src/main.rs` | Sequence playback/seek, outgoing transition editor, validation feedback and transactional document edit integration. |
| `apps/studio-desktop/src/scene_settings.rs` | Transition draft and coherent duration/transition validation. |
| `docs/master-progress.md` | Keep transitions open until integrated preview/export and tests pass; record exact supported variants. |

## Acceptance tests required before marking complete

- Legacy YAML roundtrip defaults to cuts; invalid final transition, NaN/infinity, zero/subsample duration, overlong overlap, triple overlap and overflow fail without changing the document.
- Two/three scenes with distinct colors: exact cut, overlap start/midpoint/end, first/final sample and random backward/forward seeks yield identical contributions and pixels.
- Fractional durations and fps not dividing 24000: frame samples are monotonic, no duplicate boundary frame or sample at EOF, and manifest/audio/video durations agree within the documented frame/sample granularity.
- Distinct camera keys, timed image planes and actor-mouth tracks in both scenes verify local clocks, complete-scene fades and collision-free SVG resource IDs.
- Known constant/impulse audio verifies exact offsets, weights, overlap length, silence gaps and output length; simultaneous speech keeps per-scene mouth identity.
- Preview composite and exported raster frame agree at the same global samples; existing single-scene golden behavior remains unchanged.
- Real FFmpeg sequence export verifies duration, dimensions and expected color samples; cancellation/resource limits leave no published partial destination.
- Editing durations/transitions and Undo/Redo must rebuild one coherent timeline and invalidate stale sequence audio/export previews.
