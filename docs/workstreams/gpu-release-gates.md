# GPU, Mac/Kokoro acceptance and redistribution workstream

Status: open gates, inspected 2026-10-16. This document records implementation
boundaries and next evidence; it does not certify hardware or legal compliance.
The current development environment is Linux and cannot physically certify a
Mac, Metal adapter, Mac audio device, or real Kokoro installation/playback.
No model installation, asset downloads or inference were performed for this review.

## GPU ownership and inspected status (2026-10-18)

GPU workstream owner: `gpu_parity`. This section is a source inspection and
implementation contract, not a completed renderer or hardware result. Existing
Mac/Kokoro and license gates below retain their independent acceptance criteria.

| Boundary | Actual implementation | Still open |
| --- | --- | --- |
| Opt-in GPU fixture | `crates/motion-render/src/gpu.rs`: `Backend`, `render_fixture`, `row_layout`, `compare_fixture`; locked wgpu 24.0.5, explicit Metal/Vulkan/GL, genuine shader geometry/texture composition and readback | Decoded PNG parity, transparent-edge comparisons, device execution/evidence |
| Complete evaluated scene | `crates/motion-render/src/lib.rs`: `render_svg_with_mouths`, `render_svg_with_art` consume `motion_scene::EvaluatedScene`; `motion-export/src/media.rs::rasterize_scaled` uses CPU resvg/tiny-skia | GPU equivalent of actor skeleton/face/visemes, camera, scene fades, timed artwork, image textures, masks and blend modes |
| Preview/export | `motion-export/src/preview_scene.rs::ScenePreview` and `sequence.rs::SequencePreview`, `render_sequence_frame` use shared evaluated scenes and CPU rasterization; desktop `main.rs::native_canvas_ui` and `sequence_worker.rs` use that path for advanced composition; simpler `draw_scene`/`art_editor::paint` use egui primitives | One explicit production GPU backend shared by preview and export; native painter equivalence; full-scene parity and performance |
| Packaged Metal check | `apps/studio-desktop/src/gpu_check_panel.rs`, `gpu_check_worker.rs`, `motion-render/examples/gpu_probe.rs`; explicit bundled helper, report validation, watchdog/cancel | Retained physical Metal run tied to package/commit, device loss, in-flight cancellation, performance and real scene evidence |

A GPU-backed eframe/glow window is not GPU scene export. The helper renders only
an original fixture, not MotionScript. Uploading a CPU-resvg final frame to a GPU
texture must never count as GPU scene rendering. Existing `compare_fixture`
checks a few interior pixels with channel tolerance 2, not full raster equivalence.
The real-adapter test remains explicitly opt-in/ignored in default tests.

## Next bounded implementation: evaluated primitive scene backend

Implement a default-disabled, offscreen **experimental subset**, separate from
production backend selection. Its purpose is to replace fixture-only geometry
with the actual evaluated-scene input without hiding unsupported features.
PR #43 integrated a narrower evaluated-geometry experiment with explicit label
omission, genuine shader composition, bounded readback, shader/command tests and
decoded RGBA metrics. See [implemented experiment](../gpu-evaluated-scene.md).
The broader target in this subsection is not fully implemented or accepted:
font labels, artwork, transitions, PNG raster comparisons and real Metal evidence
remain open. Existing checkbox gates below retain those broader requirements.

Files and interfaces:

- Preserve `gpu::render_fixture` and packaged fixture report schema unchanged.
  Add `crates/motion-render/src/gpu_scene.rs` behind `gpu`, with a reusable
  `GpuSceneRenderer` owning device, target and bounded buffers. Proposed
  `render(&EvaluatedScene, width, height, mouths, cancel) -> Result<gpu::Frame, Error>`
  accepts already evaluated state and per-actor visemes. Do not introduce another
  animation evaluator, document parser or sample clock.
- Add an adapter-free validated primitive command builder in
  `crates/motion-render/src/scene_commands.rs`, shared with the existing SVG
  emission where practical. Preserve SVG output defaults and public render APIs.
  It must handle letterboxing/canvas clipping, camera, ordered actors, root
  transforms, authored skeleton points, face/visemes and whole-scene opacity.
  Refactor in small reviewed changes; shader paths must not approximate silently
  by dropping labels, facial details, strokes or unsupported art.
- Add an explicit experimental `gpu_scene_probe` example accepting a local
  script/scene/sample and selected backend. Use shared scene evaluation; emit PNG,
  report and CPU reference only after successful bounded render/readback.
  This first increment accepts no artwork and no sequence transitions; requests
  with these features return a typed `UnsupportedFeature` before adapter/output
  creation. Keep it out of production export/desktop selection until its own
  coverage passes. CPU remains an explicit independently selected path.
- Keep image/mask/blend semantics in `motion-art`, scene evaluation in
  `motion-scene`, speech/sample placement in `motion-export`/`motion-voice`.
  No dependency from `motion-render` back to `motion-export`.

Acceptance for this increment:

- [ ] Adapter-free command tests cover empty/background scene, letterboxing,
  camera pan/zoom, root rotation/nonuniform scale, each viseme, skeleton offsets,
  actor order and scene opacity. Validate finite inputs and all geometry budgets
  before GPU initialization; malformed/unsupported requests leave output absent.
- [ ] Preserve existing SVG regression behavior and fixtures in
  `motion-render/src/lib.rs`; add shared-command tests proving exactly the same
  evaluated scene/sample reaches both renderers. Include animated procedural
  strengths and copied/retimed bone tracks through existing shared evaluation.
- [ ] On an explicitly selected adapter, compare decoded GPU/CPU full images at
  fixed samples, including transparent/stroke edges. First establish and document
  color-space, alpha and antialiasing differences; report max/RMS channel error,
  changed-pixel fraction and edge/interior results separately. Freeze justified
  tolerance before declaring acceptance; do not raise it to hide missing geometry.
- [ ] Output tightly packed top-left RGBA and decoded PNG with matching dimensions;
  account for 256-byte readback row padding. Test odd widths, repeated renders,
  changed dimensions, cancellation and adapter/init errors without CPU fallback.
- [ ] Default CI never requests an adapter; optional feature fmt/check/strict Clippy
  and adapter-free tests pass. Opt-in real-adapter tests retain backend/device,
  exact commit, dimensions, samples, image hashes and validation errors.

Resource contract for the first increment: retain explicit 64–2048 output bounds,
32 MiB readback ceiling and 10-second cooperative initialization/readback deadline
from the fixture. Preserve at most 256 actors and existing finite/coordinate
validation; use checked arithmetic and reject geometry expansion above an explicit
preallocated vertex/index budget. Proposed total host+GPU owned frame/geometry
budget is 128 MiB (driver overhead excluded), one in-flight render/readback, no
per-frame thread creation and no unbounded cache. Account for CPU reference/PNG
buffers separately in the probe; no hidden full-resolution duplicates. Existing
fixture target+readback+returned RGBA is about 48 MiB at 2048 square. Native driver
calls cannot be forcibly interrupted; retain a child-process watchdog for device
probes rather than claiming the cooperative deadline can kill a blocked driver.

## Remaining full-scene parity gates

These follow the primitive-scene increment; they are separate from its acceptance.

- [ ] Add decoded original `motion_art::ImageResource` textures, background and
  foreground `Plane` order, document order, visibility and half-open time ranges.
  Keep camera/canvas clipping and complete-scene opacity identical to CPU output.
- [ ] Add rectangle/ellipse/simple-polygon `ClipShape`, polygon limits and isolated
  normal/multiply/screen `BlendMode` composition. Define linear versus sRGB color
  conversion, premultiplied alpha at every boundary and offscreen layer ownership.
  Scissor-only rectangular fixture clipping does not implement these masks.
- [ ] Cover actual alpha-image edges, overlapping translucent strokes/images,
  mask boundaries and backdrop-dependent multiply/screen pixels. Reuse
  `preview_scene.rs::advanced_mask_blending_and_visibility_match_export`,
  `sequence.rs::advanced_clips_and_backdrop_blends_are_real_raster_pixels`, and
  `timed_art_is_composited_before_complete_frame_crossfade` as behavioral cases.
- [ ] GPU sequence composition uses `SequenceTimeline` sample contributions from
  `sequence.rs`, including exact cuts and crossfade overlap. Compose complete
  incoming/outgoing scene targets before weighting; test pre-roll, boundaries,
  random seek, repeated seek and prepared speech mouth samples. Preserve
  `vector_sequence_raster_matches_png_cuts_crossfades_and_mask_namespaces` and
  `preview_matches_export_pixels_and_scales_original_coordinates` CPU references.
- [ ] Extend `ScenePreview`/`SequencePreview` and PNG/MP4 export through one explicit
  backend option only after coverage. Keep CPU SVG vector export a distinct format.
  Preview/export image comparison must use the same selected GPU backend and
  original coordinates/sample clock. Surface unsupported/backend/device errors;
  never call silent CPU fallback GPU success.
- [ ] Preserve resource limits: `motion-art` has 1024 layers, 65,536 points/layer,
  262,144 total points and 256 mask vertices; CPU renderer dimensions allow up to
  8192, which the initial GPU subset intentionally rejects above 2048. Export
  permits at most 10,000 frames and 256 MiB output, with 64 MiB sequence SVG-frame
  ceiling. Retain existing preflight image-occurrence/vertex-work budgets and add
  texture residency, isolated-layer targets and two-scene overlap to checked GPU
  allocation totals. Supporting all 8192 dimensions needs a separate budget review.

## Physical Metal and performance evidence (still open)

- [ ] Run packaged explicit Metal fixture check on a physical supported Mac; retain
  BUILD-INFO, package digest, commit, native architecture, macOS/chip, adapter,
  report/raw RGBA hashes and validation evidence. Reject CPU/unknown/mismatched
  adapters. See `docs/gpu-device-check.md`; CI builds/signatures are separate.
- [ ] Run the experimental evaluated-scene comparisons on that same explicit Metal
  path, retain CPU/GPU decoded images and metrics, then repeat the eventual full
  masks/images/blending/transition corpus. Fixture success alone passes none of
  the complete-scene/native-preview/export gates.
- [ ] Exercise initialization failure, actual device loss and cancellation during
  readback; verify repeated execution and resource cleanup. Mocked failure checks
  supplement actual evidence and must be identified separately.
- [ ] Measure cold/warm CPU and GPU wall time, GPU timestamps where supported,
  peak memory and readback separately from PNG/FFmpeg encoding, using identical
  scenes/samples/resolutions. Retain observations without presuming acceleration.

This inspection executed no adapter, Metal device, visual quality or benchmark
run. No new test execution is asserted by this planning document.

## Mac/Kokoro device acceptance

Use `docs/kokoro-in-app-acceptance.md` for detailed scenarios. Keep these gates
unchecked until evidence from the actual packaged build and machine is retained.

- [ ] Record commit, artifact digest, native arm64 architecture, macOS, exact Python
  3.10–3.12 interpreter, manifest revision, dependency freeze and output device.
- [ ] Launch the downloaded app, inspect setup wording, decline consent and verify
  no installation starts. Consent and install into a new directory; test cancel,
  retry and responsive UI. Inspect descendant processes after cancellation.
- [ ] Restart the app and connect the final profile without relocation or automatic
  downloads. Run prerequisite diagnostics; keep this distinct from synthesis.
- [ ] Deny external networking at the OS/network boundary after provisioning.
  Run real CPU synthesis with both `af_heart` and `am_michael`, two speeds, and
  fresh cache keys. Retain request/settings, WAV hashes and machine-readable report.
- [ ] Verify finite, non-silent audio and personally listen through the selected Mac
  output. Record intelligibility, clipping/truncation and playback-device errors.
- [ ] Exercise stop, cancellation, seek, pause/restart, device reconnect and cache
  invalidation on changed voice/settings. Confirm retries and no stale test audio.
- [ ] Prepare real project dialogue, play scene boundaries and export audio/video;
  verify standalone tests preserve screenplay/history. Measure mouth alignment
  against actual speech separately: phoneme tokens are not measured timestamps.
- [ ] Record offline failure cases for missing models/resources and an unavailable
  output device. Python network guards alone are not OS-level offline evidence.
- [ ] Retain packaging/signing/notarization/Gatekeeper results separately from local
  development launch and from the Mac CI build. Hardware listening is still manual.

## Exact pins and license evidence currently recorded

Evidence below is local repository documentation and manifests, not a newly
completed artifact-level license audit. See `docs/kokoro-dependency-review.md` for
upstream links and limitations; `docs/kokoro-setup-sources.md` records exact sources.

| Item | Current pin/evidence | Remaining gate |
| --- | --- | --- |
| Kokoro Python | Managed manifest 0.9.4; prior upstream review records Apache-2.0 | Verify selected wheel hash, LICENSE/NOTICE and modifications |
| Misaki English | Managed manifest 0.9.4; prior review records Apache-2.0 | Inspect English extras, dictionaries and actual resolved artifacts |
| Torch | Managed manifest 2.6.0; prior review records BSD-style upstream terms | Select exact arm64/ABI wheel; inventory native libraries/notices |
| NumPy | Managed manifest 1.26.4; prior review records BSD-style upstream terms | Inspect wheel and bundled numerical-library licenses |
| spaCy | Managed manifest 3.7.5; prior review records MIT | Verify selected wheel and dependencies/notices |
| English pipeline | 3.7.1 at immutable revision `8fb6f8155286360bf5486fe399e2738d1df7ded2`; metadata records MIT | Preserve model/source acknowledgements, including WordNet; inspect actual wheel |
| Kokoro config/weights | Revision `f3ff3571791e39611d31c381e3a41a3af07b4987`; model repository reviewed as Apache-2.0 | Preserve revision-specific card/notices and determine their applicability |
| English voice tensors | Same revision; both files have exact SHA256/size in manifest | Separate voice provenance/redistribution review remains unresolved |
| Python interpreter | Host-supplied 3.10–3.12, not bundled; exact distribution/license unknown | Record selected interpreter and terms if distribution boundary changes |
| resvg | Rust direct dependency 0.45.1; license evidence not established here | Inspect crate and tiny-skia/usvg dependency closure and notices |
| eframe/glow, rodio | eframe 0.31.1 declaration, rodio 0.20.1 macOS; license evidence not established here | Inventory exact Cargo.lock artifacts, fonts, platform/native dependencies |
| serde, serde_json, thiserror | Workspace dependencies; exact resolution in Cargo.lock; license evidence not established here | Collect resolved versions, licenses and required notices |
| FFmpeg | Explicit user-selected executable, unbundled; exact build/license unknown | Capture build configuration, codecs and applicable licenses before bundling |
| eSpeak/phonemizer path | Transitive English extras; prior review identifies eSpeak NG GPLv3 | Identify actual loader, libraries/data and obligations before redistribution |

The manifest includes full SHA256/size for config, weights, both voices and the
English wheel. Its five direct Python pins are not a full transitive hash lock;
`pip freeze` records an installed resolution but does not authenticate all wheels.
The older `requirements.txt` contains broader development constraints and must not
be treated as the managed manifest or as a release lock.

## Dependency and notices completion

- [ ] Resolve one supported native arm64/Python environment without installing it
  here; retain exact wheel filenames, hashes, platform tags and full dependency graph.
- [ ] Inventory actual distributions, embedded libraries, fonts, dictionaries and
  model data; record license texts, copyright, NOTICE, source obligations and unknowns.
- [ ] Inspect voice provenance and phonemizer/eSpeak loading/distribution details;
  escalate unresolved rights/obligations to qualified review before bundling.
- [ ] Produce reviewed third-party notices/SBOM tied to the shipped artifact digest,
  retain source offers where applicable, and verify contents against the package.
- [ ] Keep distribution boundaries explicit: preview packages small scripts/manifest,
  not model weights, voices, English wheel, Python environment or FFmpeg. User-triggered
  provisioning does not by itself settle applicable redistribution obligations.

Update `docs/master-progress.md` only when each gate has corresponding evidence;
implementation, automated fixture coverage and actual device acceptance are separate.
