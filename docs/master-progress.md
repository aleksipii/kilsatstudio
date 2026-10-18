# Master prompt implementation and acceptance tracker

Source of requirements: [master build prompt](master-build-prompt.md). Updated
2026-10-18. A checked item identifies integrated, inspected implementation, not a
claim that every hardware or product acceptance test has passed. An unchecked
item includes work in progress until integration and relevant checks succeed.
The parent integrator owns completion marking and evidence. Never replace a
hardware, real-model or perceptual gate with mocked success.

## Latest merge status

Documentation-only PR #44 merged as `443e3f7b1881c02a7b0fdd55fbd647aeeba583cb`, assigning all eight
remaining areas to named agents and recording inspected next-package handoffs.
Local Markdown targets, whitespace checks and independent cross-workstream
review passed. No production code changed, no Rust/Python execution was newly
claimed, and no product/device/quality/license gate was closed. PR #43 below
remains the latest implementation increment. The release branch is synchronized
with this assignment and progress record.

PR #43 merged as `ebd7287825c10c8dcfdbf7805ac920c03545eedc` after final Linux/macOS CI passed.
Live clips now support original-speed loops and explicit seven-joint selection;
soft ellipse masks support feathering and scene-local key animation. Native scene
reordering uses a versioned editable timing document and shared sequence clock.
The evaluated-geometry GPU experiment remains opt-in and incomplete; the engine
index foundation still has no eligible real profile. CI now retains final-ZIP-bound
build/native observations without granting license or device approval.
The release branch is synchronized with this progress record.

Earlier PR #42 merged as `ed149b4d363c490b2025733c8e9644e268dcd4bf` after final Linux/macOS CI passed.
Final signed preview ZIPs now receive a bounded offline inventory sidecar with
exact member hashes and current resource-boundary checks. CI attaches both ZIP
and report. Collection explicitly leaves distribution approval false; target
build graphs, native provenance, notices and legal review remain open.
The release branch is synchronized with this progress record.

Earlier PR #41 merged as `19667de847753bd1e0c7628e596449da3b061aa7` after final Linux/macOS CI passed.
Live original-rig layers now support per-instance animated weight curves and
linear entrance/exit fades, detached native editing, source Undo/Redo and project
persistence. Exact defaults retain the prior constant-weight behavior. The
release branch is synchronized with this progress record.

Earlier PR #40 merged as `aeca29cef42efa1b9a9393dfd95ef140ac1aa266` after final Linux/macOS CI passed.
The first original-rig live clip package now supports constant-weight ordered
override layers, half-open windows, persistent references, native editing and
prepared shared preview/export evaluation. Broader blending and hardware gates
remain open. The release branch is synchronized with this progress record.

Documentation-only PR #39 merged as `cf03e847674a00162766bceb81ba7e5ea44348f8`, assigning the
five open workstreams and recording inspected implementation/evidence plans.
Local Markdown link targets and diff whitespace checks passed; the license owner
also passed 16 existing inventory tests while the strict release gate stayed
blocked. This planning merge implements no production feature or acceptance.

Earlier implementation PR #38 merged as `797e7d4fb3c97b9577bef56a8570f25c49f2c34c` after its final
Linux/macOS CI passed. PR #37 merged earlier as
`0aa890bb6fa6ce597ba5f2a21fd942db720b7389`; PR #36 as
`e49f7add105409ed4d98859d1be4b55ae1c803b1`; PR #35 as
`6bd988ab162adb64cdcea621bb01600099abbeb2`; PR #34 as
`f89c59b8113b3b578c82fe648850615e704fdaf0`; PR #33 as
`504166d85daa15e88f937a777a17123162df5e8a`; PR #32 as
`f2e7b79f23968ac3b32637680ff2c48ac7f43453`; PR #31 as
`82f1b3116f6d8647e20076b0e2803f2fd2adedba`; PR #30 as
`de41228c0b68daece521cef1406361b21df0c3a6`. The release branch
`kilsatstudio1.0` is synchronized with this merge and progress record.
The authored bone editor, chronological history, geometric masks/CPU blending,
estimated-cue import, exact-audio mouth-shape quality evaluator, optional GPU
fixture and license inventory are integrated.
PR #27's user-reported first M1 Kokoro speech evidence is preserved.

The native acceptance tooling below is merged. Its implementation and CI checks
are complete; physical hardware, real recognizer quality, full-scene GPU rendering
and final redistribution clearance remain open.

| Completed | Still open |
| --- | --- |
| Timed solid colors/images, native editors, shared preview/export and SVG project sequence compositing | Physical Mac transition and SVG-viewer acceptance |
| Persisted per-bone tracks/editor, shared preview/export and per-actor static/animated procedural layer strengths | Custom rig authoring, textures/deformation and arbitrary custom-clip blending |
| Rule clips, authored offsets, project clip capture/reuse/retiming and ordered live original-rig layers with weight curves/fades, loops and joint selection; soft animated ellipse/polygon masks and multiply/screen CPU composition | Image masks, polygon feathering, groups and physical Mac compositing acceptance |
| Chronological source/artwork/asset-inventory Undo/Redo | Physical Mac shortcuts/accessibility and advanced retouching |
| Managed Kokoro, bound cache, reported M1 speech, estimated Rhubarb cues and selected-result archive | Offline/device acceptance, automatic engine memoization and measured recognizer quality |
| CPU SVG/PNG/MP4, bounded frames, optional evaluated-geometry GPU experiment, engine index foundation and final-ZIP/build observations | Full GPU scene parity, physical Metal/performance and final license clearance |

Maintain this summary and the full checklist after each authorized merge. A new
implementation remains pending until its relevant checks pass; hardware gates
require actual evidence.

## Ownership and current increment

Previous parallel implementation increment (2026-10-18, integrated in PR #43):
`gpu_scene_implementation`, `animation_loop_masks`, `artist_mask_tools`,
`speech_reuse_implementation`, `mac_release_evidence` and
`planned_timeline_features`; parent owns cross-stream integration and release.
The follow-up table below assigns the remaining product responsibilities. Agent activity
does not constitute hardware, quality, license or full-milestone acceptance.

Eight-area follow-up assignment (2026-10-18). Each owner inspects the current
implementation and defines a concrete next package, interfaces, file boundaries,
checks and remaining acceptance evidence. This assignment changes no production
feature and grants no milestone, hardware, quality or license acceptance.

| Workstream | Assigned agent | Handoff and remaining scope |
| --- | --- | --- |
| GPU | `gpu_production_owner` | [GPU production handoff](workstreams/next-gpu-production.md): text, textures, masks, blends, transitions, shared production preview/export and Metal measurements |
| Characters and animation | `character_animation_owner` | [Character/animation handoff](workstreams/next-character-animation.md): custom rigs, textured parts, deformation, idle, general mixing and cross-project libraries |
| Artist tools | `artist_tools_owner` | [Artist tools handoff](workstreams/next-artist-tools.md): groups, image masks, polygon feathering, painting, pressure and retouching |
| Speech and lip sync | `speech_quality_owner` | [Speech/quality handoff](workstreams/next-speech-quality.md): verified real environment, automatic reuse, timing editing, forced alignment and real-audio quality |
| Screenplay and timeline | `screenplay_timeline_owner` | [Screenplay/timeline handoff](workstreams/next-screenplay-timeline.md): audio cues, reusable/conditional rules, camera presets and general compiled-track editing |
| Performance and architecture | `performance_architecture_owner` | [Performance/architecture handoff](workstreams/next-performance-architecture.md): benchmarks, UI latency, process memory and dependency scheduling |
| Mac acceptance | `mac_device_owner` | [Mac acceptance handoff](workstreams/next-mac-device.md): offline speech, devices, synchronization, Finder, shortcuts, accessibility and full workflow evidence |
| Release and licenses | `release_license_owner` | [Release/license handoff](workstreams/next-release-license.md): exact components/assets, obligations/notices, production package, signing and notarization |
| Integration and QA | Parent agent | Shared schema/render/desktop contracts, non-overlapping edits, actual checks, review, progress and release synchronization |

- [x] Complete all eight owner inspections and publish linked handoffs.
- [ ] Implement and validate the next packages; keep feature checkboxes open until
  integration and relevant checks succeed.
- [ ] Collect external physical-device, real-audio and final upstream license
  evidence; source inspection and synthetic tests cannot replace it.

Owners describe responsibilities, not completed acceptance. Agent assignment is
maintained by the parent; shared files are changed only by their assigned owner.

## 1. Architecture and technical lead

- [x] Actual Rust workspace, lockfile and narrow working crates.
- [x] Repository engineering/independence rules in `AGENTS.md`.
- [x] Shared typed MotionScript, scene evaluation and audio provider boundaries.
- [x] Deterministic scene evaluation shared by desktop and SVG export.
- [x] Local core workflow without mandatory cloud service.
- [ ] Full document/evaluation/UI separation across all planned tools.
- [ ] Architecture decision records covering the final renderer/job architecture.
- [ ] Final dependency/license audit and macOS-first acceptance.

## 2. MotionScript compiler

- [x] Typed English YAML, syntax parsing and semantic diagnostics.
- [x] Project, characters, voices and multiple scene definitions.
- [x] Actor references, finite times, movement bounds and duration rules.
- [x] Move, emotion, gesture and speak commands with deterministic evaluation.
- [x] Explicit unsupported-command errors and documented examples.
- [x] Atomic event/voice/settings source edits and tests.
- [x] Scene add/duplicate/delete and character add/delete with reference/final-scene protection.
- [x] Validated scene camera keyframes and deterministic pan/zoom evaluation.
- [ ] Camera event commands or cinematic rule presets beyond scene camera tracks.
- [x] Timed solid background-color tracks shared by preview and export.
- [x] Timed background image placement shared by native preview and export.
- [x] Validated outgoing cuts/crossfades, compiled sample timeline and headless PNG/WAV sequence export.
- [x] Native whole-project sequence preview/transport and PNG/MP4 sequence export.
- [x] SVG sequence compositing (PR #33).
- [ ] Audio cue commands distinct from dialogue import.
- [ ] Reusable/conditional rules and extensible animation presets.
- [x] Versioned editable scene-order/duration document compiled through the shared sequence clock (PR #43).
- [ ] General versioned editable compiled event/animation tracks.

## 3. Character rigging and animation

- [x] Original procedural Emma/Alex preview characters.
- [x] Deterministic position interpolation and frame evaluation.
- [x] Basic expressions and wave gesture.
- [ ] Sprite parts and editable character definitions.
- [x] Hierarchical transforms, bones and joints (`motion-rig`).
- [x] Pose evaluation and original seven-joint procedural rig shared by preview/export.
- [x] Deterministic procedural walk/wave presets and static rest pose.
- [ ] Expressive idle cycles and extensible artist-authored motion presets.
- [x] Validated transform keyframe interpolation API: linear/smoothstep/hold and shortest rotation.
- [x] Persistent per-scene actor root transform tracks and detached keyframe/easing editor.
- [x] Headless per-joint PoseTrack sampler and validated weighted pose-blending API.
- [x] Rule-based keyframe clip library in shared preview/export: emotion posture blending, walk, wave/point/nod/shrug gestures and viseme-driven speech head motion.
- [x] Persisted parent-local per-bone tracks and detached editor in shared preview/export.
- [ ] Arbitrary rig definition editing, textured parts and deformation.
- [x] Authored bone offsets layer over rule clips in shared evaluation.
- [x] Artist-controlled per-scene/actor posture, walk and gesture strengths with shared evaluation (PR #35).
- [x] Scene-local animated procedural strength tracks with outgoing easing and native editor (PR #36).
- [x] Named project-local authored bone clip library with capture/explicit overwrite, inspection, copied-track application and deletion (PR #37).
- [x] Explicit copied-clip start/duration retiming with native controls, atomic validation and shared tracks (PR #38).
- [x] Persistent original-rig live clip instances, half-open windows and constant-weight ordered local-offset override blending (PR #40).
- [x] Per-instance animated weight curves, linear fades, shared sampling and native authoring (PR #41).
- [x] Live clip original-speed looping and explicit original-rig joint selection, including native controls and shared evaluation (PR #43).
- [ ] General normalized/additive mixing and cross-project libraries.
- [x] Integrated rig/animation transform, seek and interpolation tests.

## 4. Graphics and rendering

- [x] Native animated scene preview with scene-coordinate scaling.
- [x] Shared evaluated scene and deterministic SVG frame renderer.
- [x] Basic background and original procedural character composition.
- [x] Bounded PNG/JPEG foreground image layers shared by native preview and SVG/PNG/MP4.
- [ ] Textured per-bone character sprites and deformation.
- [x] Actor z-order/visibility composition metadata and editor.
- [x] Persisted ordered/visible/opacity image and basic drawing layers with rectangular clip masks.
- [x] Validated ellipse/simple-polygon masks and normal/multiply/screen shared CPU blending.
- [x] Persisted soft animated ellipse masks in shared SVG/PNG/native preview and sequence evaluation (PR #43).
- [ ] Groups, image masks, polygon feathering and physical Mac compositing acceptance.
- [x] Selected-scene shared CPU-composited advanced frames and final integration checks (PR #31).
- [x] Camera positioning/zoom shared by native preview and SVG/PNG/MP4.
- [x] Solid background color, timed step-color tracks and scene-edge fades to black.
- [x] Timed background image planes and validated half-open visibility intervals.
- [x] Whole opaque-frame cuts/crossfades in headless PNG sequence export.
- [x] Shared scaled raster transition preview and sequence MP4 compositing.
- [x] SVG project sequence compositing (PR #33).
- [x] Default-disabled evaluated-scene geometry GPU experiment with explicit label omissions, work bounds and decoded RGBA diagnostics (PR #43).
- [ ] Verified production GPU preview.
- [x] Optional genuine offscreen GPU fixture, shader validation, bounded readback and explicit backend/device metadata.
- [ ] Full-scene GPU renderer/parity and actual macOS Metal hardware evidence.
- [x] Composition SVG golden fixture plus shared geometry/clock regression tests.
- [ ] Wider image/texture golden coverage as rendering grows.

## 5. Desktop UI/UX

- [x] Screenplay editor, preview, timeline and Inspector panels.
- [x] Play/pause/restart, scrubbing and time/frame display.
- [x] Validation diagnostics, file open/save and dark interface.
- [x] Selected event and character voice editing.
- [x] Timeline event retiming, add/duplicate/delete and source Undo/Redo.
- [x] Project title/fps/dimensions and scene ID/duration drafts.
- [x] Scene/character structure controls, confirmed deletion and source Undo.
- [x] `.kilsat` exact-source persistence and portable asset inventory preservation.
- [x] Explicit v1→v2 migration preserving source/assets and persisting typed artwork.
- [x] Bounded unified source/artwork/asset-inventory history and source/reference guards.
- [x] SVG/PNG/MP4 background export selection and explicit FFmpeg path.
- [x] Audio waveform display and prepared-dialogue preview controls.
- [x] Explicit local asset import/relink/remove-reference UI and bounded worker.
- [x] `.kilsat` inventory dirty tracking and chronological metadata-only Undo/Redo.
- [x] Background PNG/JPEG decoding, texture loading and foreground image rendering.
- [x] Native cached image thumbnails with aspect-fit and virtualized asset rows.
- [x] Asset inventory operations participate in the global document history; physical files are retained.
- [x] Actor layer order/visibility and composition camera-key editor.
- [x] Character root-transform keyframe editor plus pen/rectangle/ellipse canvas tools and rectangular masks.
- [x] Native detached per-bone editor and validated ellipse/polygon mask/blend controls.
- [x] Native procedural layer strength editor with Apply, unified Undo/Redo and project persistence (PR #35).
- [x] Animated procedural strength key editor with timing/easing, explicit Apply and safe track removal (PR #36).
- [x] Native project clip library with detached names/selection, explicit replacement/deletion confirmations and unified source history (PR #37).
- [x] Opt-in clip start/duration controls, explicit reset and replacement confirmation after timing edits (PR #38).
- [x] Native live-instance editor with saved layer order, source history, reference warnings and cached shared preview (PR #40).
- [x] Detached fade/key/easing editor with invalid draft preservation, Apply, reset and Undo/Redo (PR #41).
- [x] Native detached soft ellipse mask/key authoring with atomic Apply and shared history (PR #43).
- [x] Native stable-ID scene reorder actions with validated transitions and source Undo/Redo (PR #43).
- [ ] Pressure brushes, advanced mask painting and image retouching.
- [x] Camera controls, timed background-color editor and scene-edge fades.
- [x] Outgoing Cut/Crossfade editor with atomic settings Undo and invalid-overlap rejection.
- [x] Whole-project sequence preview/playback, global sample seek, waveform and PNG/MP4 export controls.
- [ ] Real Mac sequence playback/synchronization and end-to-end acceptance.
- [x] Chronological document history across source, artwork and inventory.
- [ ] Comprehensive Mac shortcut/accessibility acceptance.
- [ ] Comprehensive interactive macOS workflow acceptance.

## 6. Audio, Kokoro and lip sync

- [x] Replaceable Rust SpeechProvider with strict requests/results/errors.
- [x] Per-character Kokoro preset/language/speed configuration.
- [x] Local CPU Python sidecar protocol and actionable missing-runtime errors.
- [x] Content-addressed synthesis cache and integrity checks.
- [x] Imported WAV fallback and explicit alignment provenance.
- [x] Dialogue jobs, sample-clock placement, mixing and overlap checks.
- [x] Device playback integration and waveform UI implementation.
- [x] Separate imported/manual alignment and timed viseme evaluation.
- [x] Clear distinction between facial emotion and acoustic conditioning.
- [x] WAV-bound local acceptance reports and manual/candidate alignment error metrics.
- [x] Opt-in bounded Rhubarb CLI backend and exact-PCM estimated-cue import into line/scene/sequence preparation.
- [ ] Actual Rhubarb engine/device/corpus quality acceptance; stub tests do not establish these.
- [x] Consented managed Kokoro provisioning with pinned asset revisions, sizes and SHA256.
- [x] Reproducible Cargo/Python/model/voice/binary evidence inventory with fail-closed unresolved gate.
- [x] Final signed preview ZIP/member inventory, exact package resources and CI sidecar (PR #42).
- [ ] Complete artifact-specific dependency/asset license and redistribution audit.
- [x] Real local English Kokoro CPU WAV on Apple Silicon (user report 2026-10-16: M1, macOS 27.1, both voices; see [acceptance](kokoro-in-app-acceptance.md#recorded-mac-evidence)).
- [ ] Offline synthesis after provisioning with networking disabled.
- [x] Exact-PCM/transcript/provider/config/manual-edit alignment cache with atomic writes and headless import workflow.
- [x] Exact-PCM/current-text bound alignment envelopes and shared line/scene desktop cache integration.
- [x] Native explicit estimated-aligner execution controls (PR #31).
- [x] Explicit saved estimated-cue cache and native exact-PCM/text reload (PR #34).
- [x] Immutable exact-request engine-index foundation with private eligibility, corruption/conflict/cancellation checks and explicit result origin (PR #43).
- [ ] Automatic engine-result cache lookup with verified resources/environment.
- [x] Estimated cues preserve separate provenance and integer-sample trim/offset evaluation in preview/export.
- [ ] Validated forced-aligner integration and editable timing tools.
- [ ] Annotated real-audio corpus and measured timing error.
- [ ] Perceptual mouth-quality assessment across voices/speeds/pauses.
- [ ] Physical-device seek/pause/offset/trim/overlap synchronization checks.
- [ ] Synchronized encoded-video acceptance with real speech.
- [ ] Evaluated ONNX/Core ML alternatives with exact tested versions.

## 7. Export and media pipeline

- [x] Frame-accurate deterministic SVG sequence export and CLI examples.
- [x] WAV mix plus synchronized SVG/audio manifest bundle.
- [x] Export progress, cancellation and recoverable error reporting.
- [x] PNG sequence export, including decoded 1080p dimension validation.
- [x] Explicit local FFmpeg H.264/AAC encoding from the prepared preview mix.
- [x] Explicit encoder availability, invocation and external distribution/license boundary documented.
- [ ] Complete redistribution audit before bundling any encoder or dependency notices.
- [x] Real encoder/ffprobe frame count and duration checks for integer/fractional scenes.
- [ ] Long-run audiovisual drift and perceptual sync with real speech.
- [ ] Export acceptance with generated Kokoro and imported dialogue.

## 8. Concurrency and performance

- [x] Background speech preparation/export work with UI status.
- [x] Existing bounded speech worker and cancellation controls.
- [x] `motion-jobs` fixed scoped pool, bounded queues/batches and ordered consumption used by export.
- [ ] Explicit dependency-DAG scheduling beyond frame batches.
- [x] Background asset copying/relink checks and bounded parallel frame rendering.
- [x] Background-thread image decoding and native texture upload.
- [ ] Background scene-image replacement and texture-rig loading.
- [x] Estimated raster frame working-set budget limits worker count; byte/work caps remain.
- [ ] Process-wide measured memory/resource budgeting across all workers.
- [x] Ordered 10,000-job stress, worker cap, cancellation/failure/panic and byte-identical serial/parallel tests.
- [x] Runnable JSON export benchmark with debug Linux smoke measurement.
- [ ] Production release-profile and Apple Silicon benchmark series.
- [ ] UI responsiveness measurements under real workloads.

## 9. QA and release

- [x] Rust tests and Linux/macOS CI workflows present.
- [x] Sidecar protocol/error/diagnostic unit tests present.
- [x] Sample projects, build/run and practical-test documentation.
- [x] Native preview `.app` staging, ad-hoc signing and ZIP workflow.
- [x] Original-source MIT LICENSE, inherited Cargo metadata and byte-exact license in app resources/ZIP root (PR #36).
- [x] Integrated direct rustfmt, cargo check, strict Clippy and all-target Rust tests run.
- [x] Python runtime and package tests run; actual FFmpeg test run separately.
- [x] Original generated ICNS and Editor/Alternate file-type metadata structurally tested.
- [ ] Verify icon and Launch Services behavior on an actual Mac.
- [ ] Finder document event handling and open/save round-trip acceptance.
- [ ] Real Apple Silicon synthesis and device playback evidence.
- [ ] Production release packaging and release checklist acceptance.
- [ ] Optional Developer ID signing/notarization decision and validation.

## 10. Research and documentation

- [x] MotionScript, audio, UI, export and practical testing guides.
- [x] Kokoro dependency review with explicit outstanding obligations.
- [x] Development stages distinguish implementation from hardware acceptance.
- [x] Rigging, versioned project format and CPU media-export implementation guides.
- [x] Shared scheduler, resource estimates and benchmark design documented.
- [ ] Final GPU renderer decision and actual implementation.
- [ ] Forced-alignment quality research and documented corpus results.
- [ ] Exact pinned asset/dependency redistribution review and notices.
- [x] Local CPU raster/FFmpeg design, limits and integer/fractional test results documented.
- [ ] Production rendering performance measurements.
- [ ] Complete artist-facing documentation for remaining product tools.

## Whole-product milestone gates

These gates remain open until all constituent work and acceptance are evidenced.

- [ ] **M1 — First application:** Rust/parser/sample/deterministic preview/editor/
  transport/save/load/tests implemented and integrated checks passed; comprehensive
  interactive Mac acceptance still needs attachment. Prior user report
  established app launch only.
- [ ] **M2 — Character animation:** hierarchy, rigging, keyframes/easing, presets,
  expressions/gestures and blending integrated and tested; procedural rig and
  interpolation, root/per-bone editors and additive authored layering are implemented;
  textured custom rigs, general mixing and physical motion acceptance remain open.
- [ ] **M3 — Dialogue/lip sync:** import/tracks/mouth/timing foundations implemented;
  reported real M1 Kokoro speech and estimated-cue import are recorded; complete
  Speech Stages 2–4 and quality/hardware gates remain open.
- [ ] **M4 — Professional editing:** timeline and source Undo/Redo implemented;
  actor order/visibility, camera tracks and inventory tools implemented; transform
  root/per-bone keys, geometric masks, CPU blend modes, unified document history
  and transitions are implemented; advanced assets/retouching and full native
  compositor parity remain open.
- [ ] **M5 — Production rendering:** CPU SVG/PNG/MP4 and bounded parallel frames
  implemented/tested; optional GPU fixture compiles and validates shaders without
  a device. Full GPU scene parity, production performance and hardware acceptance
  remain open.
- [ ] **M6 — macOS distribution:** preview bundle, generated icon and metadata implemented; actual Finder,
  release packaging and Apple Silicon release acceptance remain open.

## Five speech acceptance gates

- [x] **Speech 1 — Foundation:** provider contract, voice schema and strict tests
  present in inspected baseline; this check does not imply real inference.
- [ ] **Speech 2 — Local synthesis:** consented/pinned assets, real macOS CPU WAV,
  Apple Silicon evidence and offline repeatability.
- [ ] **Speech 3 — Timeline:** implemented jobs/sample placement/playback integration;
  physical-device timing acceptance still pending.
- [ ] **Speech 4 — Mouth:** implemented alignment import/timed mouths; measured and
  perceptual quality acceptance still pending.
- [ ] **Speech 5 — Production:** implemented cache/voice settings/waveform/mix;
  complete timing editor, preset preview and synchronized-video acceptance pending.

## Completion log

| Date | Item | Evidence and scope |
| --- | --- | --- |
| 2026-10-15 | Baseline inspection | Read master prompt, AGENTS, workspace, existing source and development-stage docs. Existing features checked above; concurrent new modules deliberately remain unchecked. |
| 2026-10-15 | Packaging implementation | Original generated ICNS and alternate Editor document declarations implemented; `python3 -m unittest discover -s tests -p test_package_macos.py -v`: 8 tests passed. Integration/macOS validation still pending. |

When closing an item, append commit/PR, exact executed check and remaining platform
limitations here. Do not mark whole milestones complete from module presence.


| Date | Item | Evidence and scope |
| --- | --- | --- |
| 2026-10-16 | PR #18 integrated implementation | `.kilsat` persistence, source lifecycle, shared procedural rig/keyframe API, desktop controls, PNG/MP4 and speech-evidence tooling implemented. Six agents: media, rig, project, desktop, speech, tracker_release. |
| 2026-10-16 | Local validation | 150 Rust tests passed, one real-encoder test separately passed (1.0s and0.9s cases), 29 runtime +8 packaging Python tests passed; strict Clippy and direct rustfmt passed. Linux evidence only; GitHub CI result belongs to its exact head commit. |

Master prompt completion remains **open**. Per-bone sprite rendering, advanced drawing/masks, true cross-scene transitions,
custom rig/per-bone editor, blending, automatic forced alignment,
GPU rendering, production performance evidence, final distribution and
real Mac/model/device/perceptual acceptance are still unchecked above. No agent
continues running after the reported task completes; future increments must be
explicitly resumed and validated.


## Composition increment — completed implementation checks

Six agents inspected and implemented this pass: composition, assets,
desktop_compose, jobs, alignment_cache and acceptance_review. The attached
tracker was the input; this repository copy is the maintained implementation
record. PR #18 Linux/macOS CI passed at aa2f137; it and #19 were subsequently
merged after approval (main/release0866e31).

Library/headless alignment caching is complete; automatic forced alignment and
desktop cache integration remain separate. Imported images are inventory files,
not rendered character textures. Camera fades do not implement scene transitions.


| Date | Completed implementation | Actual evidence |
| --- | --- | --- |
| 2026-10-16 | Camera/layer/fade composition and native controls | Shared evaluator, clipped SVG golden, atomic invalid-edit rollback and source Undo tests. |
| 2026-10-16 | Asset import/relink/reference removal and UI | Bounded background copy, canonical-root and stale-result guards, inventory dirty/one-step Undo; source files preserved. |
| 2026-10-16 | Bounded parallel frame jobs and benchmark | Serial/parallel PNG/SVG byte equivalence, ordered stress/cancel/panic tests, documented debug Linux measurement. |
| 2026-10-16 | Exact-audio alignment cache | PCM/transcript/config/edit invalidation, integrity, provenance, concurrent writes and offline example; no automatic aligner or desktop preparation integration. |
| 2026-10-16 | Integrated local validation | 169 Rust tests passed; separate real FFmpeg/ffprobe test passed; strict workspace Clippy, cargo check, all46-file direct rustfmt and diff check passed. |

Remaining items stay unchecked. That composition increment did not supply artwork or actor-keyframe tools;
the subsequent artwork increment below implements the described basic subset.
Per-bone rig parts, advanced masks, full unified project history, true transitions,
automatic alignment, GPU rendering and hardware/model acceptance remain open.


## Artwork and actor keyframes increment

PRs #18 and #19 were merged in order after explicit approval; both had passing
Linux/macOS CI. `main` and `kilsatstudio1.0` were synchronized to0866e31.

Six agents implement this increment: art_model, keyframe_core, art_render,
desktop_art, keyframe_ui and art_qa. Component tests cover v1/v2 migration, image
limits, typed edits, root interpolation, raster pixels/masks, and referenced
source/artwork Undo safety. Final integrated results are recorded below.

Scope: artwork is foreground document order; rectangular masks and translations
for actor attachments are supported. Artwork attachments do not inherit root
rotation/scale; procedural actors do. This is not textured bone rigging, a full
image editor, unified project history, GPU export or real Mac acceptance.


| Date | Artwork increment completion | Evidence |
| --- | --- | --- |
| 2026-10-16 | Persisted root keyframes/editor and artwork v2 | 191 workspace/all-target Rust tests passed, including gesture/history/refguards, migration, PNG/JPEG, interpolation, raster and clip tests. |
| 2026-10-16 | Actual artwork media pipeline | Two explicit FFmpeg tests passed, including embedded artwork→PNG→H.264/AAC MP4 and integer/fractional media timing. |
| 2026-10-16 | Code checks | Workspace check, strict installed-driver Clippy, direct rustfmt for51 Rust files and diff check passed. Linux local evidence; new GitHub CI applies to its exact head. |

The current release acceptance gates remain open: no actual Mac GUI/device test,
real Kokoro inference, GPU rendering or perceptual mouth-sync acceptance was
performed for this artwork increment.

## Managed Kokoro desktop setup increment

- [x] PR #20 merged with approval; main and kilsatstudio1.0 synchronized to
  6634e68190b3588e58719c659bca99ff11e346bf.
- [x] Desktop consent, isolated setup, progress/cancel and existing-profile connection.
- [x] Verified model/voice/language asset manifest and packaged setup helpers.
- [x] Separate English CPU speech-test and playback controls without document edits.
- [x] Runtime Python tests: 38 passed; macOS packaging unit tests: 8 passed.
- [ ] Actual Apple Silicon installation, synthesis, listening and device acceptance.
- [ ] Complete transitive dependency lock and redistribution/license audit.

Compatible host Python 3.10–3.12 is required. Model assets and Python are not
bundled. No model installation or real inference was performed in development.
See managed-kokoro.md and kokoro-in-app-acceptance.md for the acceptance procedure.

## Timed background colors increment

PR #21 was merged with approval as 3448fb031a6bb3953e09f893874952fa8c77427d;
`main` and `kilsatstudio1.0` were synchronized. Its Linux/macOS CI and Apple
Silicon preview packaging passed. Real Kokoro synthesis/listening remains open.

This increment implements bounded, validated scene-local background color keys,
inclusive step evaluation, native draft editing and shared preview/SVG/PNG/MP4
behavior. It covers solid colors; background images and cross-scene transitions
remain open. Core, desktop and independent review agents contributed; the parent
owns export regressions, integration and completion marking. Validation evidence
is recorded in the pull request after final checks.

| Date | Completed increment | Evidence and scope |
| --- | --- | --- |
| 2026-10-16 | Timed solid background colors, PR #22 | 205 Rust workspace/all-target tests passed locally; decoded PNG/SVG color boundary coverage; strict installed-driver Clippy/workspace check, direct rustfmt and diff check passed. |
| 2026-10-16 | Atomic settings Apply | Shrink/remove, shrink/retime, extend/add and exact source Undo regressions passed; independent review blocker resolved. Real Mac GUI and background-image/transition work remain open. |

## Timed background images increment

PR #22 review completed without findings. Its final head572d779 passed all CI;
205 Rust tests passed locally. The merge status above records completed and
remaining work. New work adds persisted background/foreground artwork planes and
optional scene-local visibility intervals; final evidence follows in the PR.

| Date | Completed implementation | Actual evidence and limits |
| --- | --- | --- |
| 2026-10-16 | Background/foreground artwork planes and timed images | 217 Rust workspace/all-target tests passed locally, including legacy v2 defaults, interval bounds, invalid atomic edits, Undo, shared SVG ordering and decoded PNG frame pixels. |
| 2026-10-16 | Composition fade parity and future-resource preflight | Independent review found the native fade mismatch; one final canvas fade now follows both planes/actors. Shape-order tests and future-image/duration preflight tests passed. No physical Mac GUI test claimed. |

Cross-scene transitions, textured per-bone rigs, advanced masks/blending, unified
history, forced alignment/quality, GPU rendering, production benchmarks, license
audits and actual Mac/Kokoro/device acceptance remain open.

Final local checks: direct rustfmt and diff checks passed; strict installed-driver
Clippy/workspace all-target check passed. The full 20-test motion-export suite
also passed with actual /usr/bin/ffmpeg and /usr/bin/ffprobe, including both
normally ignored real-media tests. GitHub CI applies only to its exact head.

## Open workstream assignment and results

The user requested six agents for the remaining areas on 2026-10-16. Each
completed the bounded task below; these are not claims that the whole feature or
acceptance gate is finished. Reports are implementation contracts, not completed
product features.

| Agent | Delivered scope | Still open |
| --- | --- | --- |
| bone_animation | Local per-joint PoseTrack sampling, weighted pose blend API, tests and runnable example; [details](workstreams/animation.md) | Persistence, per-bone editor, textured rigs and preview/export integration |
| transition_workstream | [Cross-scene clock, composition, audio and test contract](workstreams/transitions.md) | Scene transition implementation and acceptance |
| mask_workstream | [Masks and blend-mode implementation contract](workstreams/masks-blending.md) | Ellipse/arbitrary masks, isolated compositing and blend modes |
| history_workstream | [Whole-project transaction/history migration](workstreams/unified-history.md) | Shared bounded history across source, art and inventory |
| alignment_workstream | [Exact-audio import/cache integration plan](workstreams/alignment.md) | Desktop identity-bound import, automatic backend and measured quality |
| gpu_release_workstream | [GPU proof, Mac/device and license evidence gaps](workstreams/gpu-release-gates.md) | Offscreen Metal path, physical-device/offline Kokoro acceptance and full license audit |

Current code increment is a headless animation foundation. Do not mark full
per-bone rigging, animation blending, masks, transitions, alignment, GPU or Mac
acceptance complete based on these APIs or reports. The exact final validation
is recorded below and in its pull request.

| Date | Completion evidence | Remaining scope |
| --- | --- | --- |
| 2026-10-16 | Six agent workstream audits delivered in docs/workstreams; 220 Rust workspace/all-target tests passed after the new animation APIs. Strict installed-driver Clippy/workspace check, direct rustfmt and diff checks passed; blend_pose example executed with weights 0/0.5/1. | Library foundation only; full per-bone editing and integrated animation blending remain open. |

PR #23 head7bdf5f1 passed Linux/macOS CI, media encoding and Apple Silicon
packaging; GitHub code review completed without findings. PRs #23 and #24
were subsequently merged in order, with the latter retargeted to main.

## Exact-audio alignment and desktop cache increment

Merge status is refreshed above and in each merged PR description. PR #23
completed timed background images and composition fade parity; PR #24 completed
headless joint-pose APIs and six scoped workstream reports. Neither merge closes
physical Mac/model acceptance, full per-bone editing, transition, mask, unified
history, automatic alignment or GPU milestones.

This bounded implementation binds supplied timings to the decoded PCM and
current transcript, adds explicit bound/cache/legacy status, and reuses the
existing local cache from line/scene background preparation. Core, voice and
desktop agents owned separate files; the parent integrated and verified them.
No automatic timing generation, model installation or measured quality is implied.

| Date | Completed increment | Executed evidence and limits |
| --- | --- | --- |
| 2026-10-16 | Bound alignment envelope, line/scene cache resolution and visible statuses | 235 Rust workspace/all-target tests passed locally; strict installed-driver Clippy/workspace check, direct rustfmt and diff checks passed. |
| 2026-10-16 | Exact audio/text rejection and immutable concurrent cache publication | Same-length different PCM/current-text mismatch, cache-only hit/miss, corruption, legacy one-sample tolerance, competing conflicting writers and single/scene parity regressions passed. |
| 2026-10-16 | Preview invalidation and review fixes | Selected timing replacement/clear invalidates prior preview; cache no-replace publication reviewed. Independent review confirmed both blockers resolved. |
| 2026-10-16 | Bound import example | Executed on a synthetic local silent WAV and synthetic phone fixture; creates a strict envelope without inference/downloads. This is not measured real speech. |

Automatic alignment, phone-boundary editing, real-audio corpus quality, physical
Mac synchronization and actual Kokoro/offline synthesis remain open alongside
transitions, full per-bone tooling, advanced compositing, unified history, GPU
rendering, production performance and final license/distribution audit.

## Rule-based character animation increment

PR #26 replaces the hand-written humanoid pose math with an additive keyframe
clip library in `motion-animation`. Emotion postures blend over 0.25 s with
`blend_pose`, a keyframed walk cycle plays during moves, `wave`, `point`, `nod`
and `shrug` gestures play under a sine in/out envelope, and
`motion_scene::apply_speech` dips the head by mouth openness from the same
per-actor visemes the mouth renders, in desktop preview and frame export.
Unknown gestures remain explicit unsupported errors. The master build prompt
now also describes Rhubarb Lip Sync as a candidate offline mouth-timing backend;
it is documentation only, not an implemented or evaluated integration.

| Date | Completed increment | Executed evidence and limits |
| --- | --- | --- |
| 2026-10-16 | Clip library, posture/gesture/walk layers and speech head motion | Local Linux: `cargo fmt --all --check`, `cargo check --workspace`, `cargo clippy --workspace --all-targets` and `cargo test --workspace` passed on the merged head with PR #25 (238 tests passed, 2 ignored, including the interrupted posture-blend regression). No Mac GUI run, visual motion review or perceptual speech-motion check. |

Persisted or artist-authored clips, per-bone editing, textured parts, a Rhubarb
or other automatic timing backend and measured mouth-timing quality remain open.

## First real Kokoro speech on a Mac

| Date | Evidence | Remaining |
| --- | --- | --- |
| 2026-10-16 | The user installed Kokoro through the in-app managed setup on an Apple M1 running macOS 27.1, using the preview built from `main` at 98d265b with Homebrew Python 3.12. They ran the English CPU speech test for `af_heart` and `am_michael` and reported that it works. | Offline synthesis with networking disabled, the remaining in-app acceptance checklist, real scene dialogue playback/export and the Speech 2 gate. |

## Scene sequence implementation increment (PR #28)

PR #28 added validated outgoing cuts/crossfades, a shared global 24 kHz
timeline, matching audio mixing and bounded headless PNG/WAV sequence export.
Inspector authoring is included; native preview/transport and scene-export buttons
remain scene-local. Full native sequence preview and MP4/SVG sequence export stay
open. See [guide](scene-sequences.md) and [sample](../examples/scene-sequence.yaml).

The initial uncommitted sequence work was lost when the transient workspace reset.
Four agents restored their implementations additively on main0ab1ba1, preserving
the new rule-based animation and user Mac evidence. The restored source was
checkpointed immediately in PR #28 before revalidation. Old incomplete test logs
are not reused as proof; final results for this head are recorded below.

| Date | Completed implementation | Actual validation and remaining scope |
| --- | --- | --- |
| 2026-10-16 | Shared sequence timeline, outgoing transition editor, linear audio and whole-frame PNG crossfades | Restored source head f6c62bdd passed GitHub Linux/macOS fmt/check/strict Clippy/all-target tests, runtime38 + packaging8 Python tests, real media encoding and Apple Silicon preview packaging. Linux job logs contain261 passed Rust tests, two encoder tests separately covered by media CI. |
| 2026-10-16 | Recovery review and local reproduction | Independent read-only re-review found no blocker; current apply_speech path and PR26/27 changes preserved. Locally4 sequence export tests passed. Recovered CLI fixture produced6 PNG frames,36000 audio samples and correct overlap midpoint PCM; synthetic tones only, no real speech inference. |

The checked scope at PR #28 was authoring plus headless PNG/WAV sequence export.
Native whole-project preview/transport, MP4 sequence export and CLI `.kilsat`
resource loading were open at that merge and are addressed by PR #29 below. SVG
sequence compositing remains open. Per-bone editing/textured rigs, advanced
compositing, unified
history, automatic alignment/quality, offline/device acceptance, GPU/performance
and final licensing/distribution gates also remain open. User-reported first
real M1 Kokoro success (PR27) remains distinct from those comprehensive gates.

## Whole-project desktop increment (PR #29)

- [x] Merge PR #28 and synchronize the release branch; retain all earlier Mac evidence.
- [x] Integrate and validate native whole-project preview, global playback/seek and waveform.
- [x] Integrate and validate shared scaled raster preview and PNG/MP4 sequence export.
- [x] Validate .kilsat sequence CLI resource loading with canonical image bounds.
- [x] Integrate and validate asset thumbnails and virtualized image inventory.

Four agents implemented these scopes. Parent validation passed all 274 Rust tests
(three explicit encoder tests ignored by default), direct rustfmt, all-target check
and strict Clippy. Seven sequence tests including the explicit real FFmpeg test
passed. All three opt-in real-encoder tests also passed; sequence encoding is now
covered by the media CI job. Preparation snapshots are created only on explicit
Prepare clicks; the final desktop 98 tests and strict Clippy passed after that
UI optimization. [Mac instructions](practical-testing.md) include the new pending
whole-project device/offline/transition acceptance checklist. A synthetic .kilsat
fixture exported 6 PNGs and a 1.5 s MP4 with 36000 audio samples; decoded PNG artwork matched the expected green pixel. Traversal input
was rejected. Independent review found EOF retry, paused-seek repaint and competing
playback bugs; fixes and regression tests are included. Implementation head
`6fb9696798bb31c222d5264dee3898b80526dcda` passed GitHub Linux/macOS
fmt/check/strict Clippy/all-target Rust tests, runtime and packaging Python tests,
and all three real media tests (Audio foundation run 38918205145). Apple Silicon
preview build/package/signature/helper checks and artifact upload also passed
(run 38918205156). This final tracker edit changes documentation only. These are
implementation checks.
At the PR #29 checkpoint, real Mac sequence/offline/perceptual acceptance,
unified history, per-bone editing, advanced composition, GPU and final licensing
were still open. The implementation slices completed next are recorded below;
physical acceptance and full product gates are not implied.

## Artist tools and offline pipeline increment (PR #30)

- [x] Merge PR #29, synchronize release branch and record actual checks.
- [x] Integrate persisted per-bone editor and shared evaluation, preserving rule clips.
- [x] Integrate chronological source/artwork/asset inventory Undo/Redo.
- [x] Integrate validated ellipse/polygon masks and verified multiply/screen blending.
- [x] Integrate bounded offline Rhubarb estimated cues and exact-PCM import.
- [x] Integrate optional offscreen GPU fixture; full scene GPU parity stays open.
- [x] Integrate reproducible dependency evidence inventory and strict unknown gate.
- [ ] Real Mac Metal, sequence/audio/offline and perceptual mouth acceptance.
- [ ] Full dependency/voice redistribution audit and reviewed third-party notices.

Parent integrated 244 focused Rust tests (108 desktop), workspace/all-target
check and strict Clippy with local Rust 1.90. The optional GPU library passed
14 adapter-free tests (one actual GPU test deliberately ignored) and feature
Clippy. Eighteen Python packaging/audit tests passed again after restoring the
checkpoint. Two independent reviews found no blocker in unified history/masks
and estimated-cue/GPU bounds. Final implementation/documentation head
`f986a736c0166c2b9b652fed6ce61b0c77296955` passed all Linux/macOS CI:
Audio foundation run 38955230988 includes fmt/check/strict Clippy, 303 default
Rust tests, the optional GPU feature tests/Clippy, runtime/packaging/audit Python
tests and all three actual media-encoder tests. GPU feature testing passes
14 adapter-free tests and intentionally skips actual device execution.
Apple Silicon preview run 38955231046 passed build/package/signature checks and
uploaded the arm64 artifact (SHA256
`3a41892e8da0afea8bd7cf9b5435aa76e09a16b43cc24d340aa9b5be11c7be6a`).
This subsequent edit records evidence only; it changes no implementation.

No actual Rhubarb engine, Kokoro inference or GPU device was executed here.
Estimated mouth shapes are not phonemes or forced alignment. GPU fixture
compilation is not physical hardware acceptance or full-scene GPU rendering.
Audit collection is not license clearance. See [bones](per-bone-editor.md),
[history](unified-history.md), [composition](advanced-compositing.md),
[estimated cues](rhubarb-estimated-cues.md), [GPU fixture](gpu-offscreen.md)
and [dependency evidence](dependency-audit.md).


## Native acceptance tooling and retained provenance (PR #31, merged)

Implemented source passed local validation, independent review and final CI:

- [x] Selected advanced-layer canvas routes immutable snapshots through a bounded
  shared CPU compositor with generation/sample guards and speech-mouth overrides.
- [x] Explicit native Rhubarb action runs against an already prepared line on a
  bounded worker; separate Apply reuses exact-PCM/current-text validation.
- [x] Packaged independently signed Metal fixture helper, explicit device-check
  UI, bounded evidence readback and cancellation/watchdog path.
- [x] New consented managed installs retain sanitized freeze bytes, installed
  distribution metadata and bounded license texts with manifest/hash binding.
  Missing or budget-omitted texts stay unresolved; distribution approval is false.
- [x] Local workspace/all-target tests: 329 passed, three explicit encoder tests ignored.
- [x] Independent integration reviews; EOF/error retry and Mac path-alias regressions fixed.
- [x] Final Linux/macOS CI and merge for this increment.
- [ ] Actual Mac compositor/device timing, real Rhubarb quality, physical Metal
  fixture results, full-scene GPU parity and final artifact-specific license clearance.

Current validation: workspace/all-target check and strict Clippy passed; 329 Rust
tests, 44 runtime, 16 inventory and 10 packaging Python tests passed. The first
CI checkpoint found a macOS canonical path alias regression, fixed with an alias
and escape regression test. Final head `5e0237143799d4d2bde61d91c5b8831cb11374fc`
passed Audio foundation run 38959215597 (Linux/macOS fmt/check/strict Clippy,
329 default tests, optional GPU tests/Clippy, Python checks and three actual
encoder tests) and Apple Silicon preview run 38959215483. Local optional GPU
validation passed 14 adapter-free tests with actual device execution ignored;
all three actual encoder tests passed locally too. The arm64 package includes
the independently signed Metal helper; bundle/helper architecture, signatures
and packaged executable permissions were checked. Uploaded artifact SHA256:
`d1ee4aea05b1dc2791cbdb6871debd9fed7b4db8a6e1813f5e9b8e14c127d651`.
Package: https://github.com/aleksipii/kilsatstudio/actions/runs/38959215483/artifacts/12071129527
This post-merge edit records evidence only and changes no implementation.
No real engine, speech inference or GPU hardware execution is claimed here.
See [native compositor](native-compositor.md), [native alignment](native-alignment.md),
[Metal device check](gpu-device-check.md) and [retained provenance](license-provenance.md).

## Estimated mouth-shape quality tooling (PR #32, merged 2026-10-18)

- [x] Integrate offline Rhubarb mouth-shape comparison against manual sample-based
  references, with whole-recording and active-union agreement, exact confusion
  counts and boundary errors only for matching collapsed shape sequences.
- [x] Integrate exact WAV/decoded-PCM/current-text evidence binding, bounded input
  snapshots, exclusive report publication and shared Rust/Python PCM golden vector.
- [x] Complete final CI, independent review and merge for this increment.
- [ ] Run representative real Rhubarb corpus and review audible/visible timing;
  establish reviewed targets and physical Mac synchronization evidence.

Local validation: 55 runtime tests (44 existing, 11 new) and 26 inventory/packaging
tests passed. Independent review reran all 55 runtime tests and cross-checked
signal measurements against the smoke reader at six lengths around chunk boundaries.
Final head `6fefd300fd57725b183b317455a73d0d1ab959c2` passed Audio foundation
39081856002 and Apple Silicon preview 39081856023: Linux/macOS fmt, workspace
check, strict Clippy, 330 default Rust tests, 14 adapter-free GPU tests/feature
Clippy, Python checks and three actual encoder tests. The shared Rust/Python
PCM golden vector passed. Rust checks ran in CI because no local toolchain was
available. The actual GPU test remains ignored.
The arm64 package passed architecture/signature/packaging verification. Artifact:
https://github.com/aleksipii/kilsatstudio/actions/runs/39081856023/artifacts/12118704932
SHA256: `0670ee100b0019713b25fee8049cb4957e84564fa0e9a4f6e1616925beab6a33`.
These quality-tool tests use synthetic audio; no recognizer, model or GPU was
executed. This post-merge edit records evidence only. The evaluator
always leaves production acceptance `not_granted`. Full GPU scene parity,
physical Mac acceptance and final artifact-specific license clearance remain open.
See [estimated mouth-shape evidence](viseme-quality.md).

## Whole-project SVG sequence export (PR #33, merged 2026-10-18)

- [x] Integrate vector SVG/WAV sequence export with cuts/crossfades, namespaced
  generated definitions, shared audio/sample manifest and bounded cleanup.
- [x] Integrate SVG format into the existing native export worker and headless
  YAML/`.kilsat` CLI; require FFmpeg only for MP4.
- [x] Validate SVG-vs-PNG raster fixtures with camera/fades, masks/blending,
  embedded images and prepared timed mouths, plus cancellation/output ownership.
- [x] Final Linux/macOS CI, independent review and merge.
- [ ] Physical Mac SVG-viewer comparison and real speech/device synchronization.

Final head `de29b3732b3ed491ad5db82cb19db9a5fdf8478c` passed Audio foundation
39082961887 and Apple Silicon preview 39082961885: Linux/macOS fmt/check/strict
Clippy, 337 default Rust tests, 14 adapter-free GPU tests/feature Clippy, 55 runtime
and 26 inventory/packaging Python tests, three actual encoder tests and signed
arm64 package verification. Rust checks ran in CI; no local toolchain was
available. SVG raster comparisons cover cuts/crossfades, cameras, scene fades,
mask IDs, multiply blending, an embedded PNG and prepared mouth samples, with
at most two channel values of rounding tolerance. Cancellation, existing outputs,
future missing images and repeated-image preflight have regressions.
Independent review found repeated references could evade the unique-image budget;
preflight now counts each occurrence before SVG/raster generation, even when
hidden or active later. Per-scene base64 image occurrences are capped at 32 MiB,
serialized scene/frame SVG at 64 MiB, total output at 256 MiB. Temporary copies
are included in the conservative working-set estimate, which is not an allocator cap.
Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39082961885/artifacts/12120160079
SHA256: `6f4e56e221ffd93ec3d8e34dfb35da278a17b2afe6fe6d3958b291f0bda10455`.
This post-merge record changes no implementation. No real model/recognizer,
GPU device or physical Mac viewer acceptance was executed. The tracker also
corrects two stale unchecked items already completed in PR #31:
selected advanced CPU frames and explicit native estimated-aligner controls.
Estimated-cue caching, full-scene GPU parity, real corpus quality, physical Mac
acceptance and final artifact-specific license clearance remain open.

## Selected estimated-cue cache (PR #34, merged 2026-10-18)

- [x] Integrate immutable saved estimated results selected by a full-content key,
  exact PCM/text validation and preserved original tool/recognizer provenance.
- [x] Integrate best-effort save after explicit Estimate, explicit Load cached
  timing and separate Apply on the existing bounded native worker.
- [x] Validate corruption/bounds/schema, concurrent publication, owned cleanup,
  no-engine load and stale/mismatched prepared audio/text.
- [x] Final Linux/macOS CI, independent review and merge.
- [ ] Automatic fresh-engine memoization with verified external resource identity.
- [ ] Project timing persistence, real corpus quality and physical Mac synchronization.

This increment is explicit selected-result reuse. It does not infer the current
recognizer environment from a binary hash or automatically substitute past cues
for a new execution. Final head `f5fc7d7334332fdc02218e7817bd170101a7ed75`
passed Audio foundation 39083898741 and Apple Silicon preview 39083898740:
Linux/macOS formatting, workspace check, strict Clippy, 345 default Rust tests,
14 adapter-free GPU tests/feature Clippy, 55 runtime plus 26 inventory/packaging
Python tests, three actual encoder tests and arm64 package/signature checks.
Rust checks ran in CI because no local toolchain was available. Independent
review found no blocker; same-frame settings/action handling was corrected.
Synthetic workflow tests save a result, remove the stub executable and WAV,
reload the selected entry and reject changed PCM/text. Save failures retain
usable timing with an explicit warning. Corrupt existing entries are preserved;
concurrent identical publication and owned staging cleanup have regressions.
The original result provenance is retained and no engine executes on Load.
Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39083898740/artifacts/12119862185
SHA256: `5423e84b9cabc704d5bdf6e91271483f294c235ace22b0d03cc4f7cf219bb3d1`.
This post-merge record changes no implementation. No real recognizer/model,
GPU-device or physical Mac acceptance was performed. Full GPU parity, advanced
artist tools, automatic engine memoization, physical acceptance and final license
clearance remain open.
See [saved estimated timing](estimated-cue-cache.md).

## Artist procedural layer strengths (PR #35, merged 2026-10-18)

- [x] Integrate per-scene/actor posture, walk and gesture strengths with strict
  defaults, finite range validation, atomic edits and character cleanup.
- [x] Integrate shared weighted evaluation while retaining authored bone offsets,
  facial/root/speech stages and existing default geometry.
- [x] Integrate detached native Inspector controls, explicit Apply, unified
  Undo/Redo, project persistence and stale/busy draft guards.
- [x] Validate actual SVG/PNG frames and shared preview on the speech sample clock.
- [x] Final Linux/macOS CI, independent review and merge.
- [ ] Editable custom clips, animated blend tracks, arbitrary/textured rigs,
  advanced masks and physical Mac motion/artist acceptance.

Final source head `fa03a4fecc0404b80d1b6c44d354ccaaca8ade98` passed Audio
foundation 39085331814 and Apple Silicon preview 39085331803. CI validated
formatting, workspace check, strict Clippy, 359 default Rust tests on Linux and
358 on macOS, 14 adapter-free GPU tests/feature Clippy per platform, 55 runtime
plus 26 inventory/packaging Python tests and three actual encoder tests.
Arm64 packaging/signature checks passed. Rust checks ran in CI because no
local toolchain was available. Independent review found no blocker. A headless
regression caught Slider normalizing a nonfinite draft during paint; invalid
values now remain detached and block Apply until explicitly reset.
Actual SVG/PNG regressions cover absent versus explicit default weights,
gesture suppression, preserved authored offsets and sample-clock preview/export.
Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39085331803/artifacts/12120487413
SHA256: `e88a51f79cc8842e6ee205afa59496efd95d149bb34c8a7043d81b2ebde317a4`.
This post-merge record changes no implementation. No real recognizer/model,
GPU device or physical Mac acceptance was executed. These are controls for the
three existing procedural layers; advanced artist tools remain incomplete.
Full GPU parity, automatic fresh-engine memoization, physical acceptance and
final licenses remain open.
See [artist layer strengths](artist-clip-weights.md).


## Source license and animated strengths (PR #36, merged 2026-10-18)

- [x] Integrate root MIT LICENSE, consistent Cargo package metadata and README scope.
- [x] Include source license in the signed app resources and ZIP root, with missing
  license rejection and byte-exact package verification.
- [x] Integrate validated scene-local procedural strength keys, outgoing
  linear/smoothstep/hold sampling, static fallback and character cleanup.
- [x] Integrate detached animated strength editor, Apply/Undo/Redo, project
  persistence and stale/busy guards.
- [x] Validate actual SVG/PNG exports, sample-clock preview and deterministic seeks.
- [x] Final Linux/macOS CI, independent review and merge.
- [ ] Arbitrary authored clip libraries/blending, custom/textured rigs and advanced masks.
- [ ] Full GPU/native parity, automatic fresh-engine estimated result reuse,
  physical Mac acceptance and final artifact-specific third-party license clearance.

Final source head `7a55ab5c506efec1ffedf2fd8fe71f4d86fd5978` passed Audio
foundation 39086245253 and Apple Silicon preview 39086245328. CI validated
formatting, workspace check, strict Clippy, 372 default Rust tests on Linux and
371 on macOS, 14 adapter-free GPU tests/feature Clippy per platform, 55 runtime
plus 29 inventory/packaging Python tests and three actual encoder tests.
Arm64 packaging/signature checks and byte-exact LICENSE checks inside the app
and at ZIP root passed. Rust checks ran in CI because no local toolchain was
available. The same 55 runtime and 29 packaging/inventory tests passed locally.
Independent review found an invalid empty-track export fixture; the corrected
test removes the track through the public API before comparing SVG/PNG output.
An existing worker format-routing test hit its five-second limit while rasterizing
default full-HD frames; its fixture now uses 64x64 frames and retains real exports.
Animated track tests cover strict schema/bounds, independent scalar interpolation,
outgoing easing, endpoint clamps, repeated/backwards seeks, actual SVG/PNG versus
sample-clock preview, static fallback, saved projects and one-edit Undo/Redo.
Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39086245328/artifacts/12120942405
SHA256: `91c80638a12768b504fc9d0e1db61fd45b551893672b6b7592b7bc3616996172`.
This post-merge record changes no implementation. No real recognizer/model,
GPU-device or physical Mac acceptance was performed. Declaring an original-source
license does not complete model/voice/tool/dependency review. Animated strengths
control the existing three procedural layers; custom clip libraries, arbitrary
rigs, full GPU parity, automatic fresh-engine reuse and physical acceptance remain
open. See [artist strengths](artist-clip-weights.md) and the
[dependency audit](dependency-audit.md).

## Project-local authored bone clip library (PR #37, merged 2026-10-18)

- [x] Integrate bounded, strict named animation clips in project source with
  original-rig bone tracks, duration and backward-compatible empty defaults.
- [x] Integrate atomic capture/explicit overwrite, full target-track replacement
  and removal; applied copies remain independent of library/source deletion.
- [x] Integrate native library capture/selection/inspection, explicit replacement
  confirmations, unified Undo/Redo, saved projects and stale/busy context guards.
- [x] Validate independent expected SVG/PNG frames, sample-clock preview,
  repeated/backwards seeking and deletion preserving already applied animation.
- [x] Final Linux/macOS CI, independent review and merge.
- [ ] Live custom clip instances/blending, retiming, arbitrary/textured rigs and
  cross-project library exchange.
- [ ] Full GPU/native parity, automatic fresh-engine estimated-result reuse,
  physical Mac acceptance and final third-party release license clearance.

Final source head `85c5cb5948c17ac1073810744ac22963ee383912` passed Audio
foundation 39089591769 and Apple Silicon preview 39089591760. CI validated
formatting, workspace check, strict Clippy, 381 default Rust tests on Linux and
380 on macOS, 14 adapter-free GPU tests/feature Clippy per platform, 55 runtime
plus 29 inventory/packaging Python tests and three actual encoder tests.
Arm64 packaging/signature and license-byte checks passed. The 29 packaging/
inventory tests also passed locally. Rust checks ran in CI because no local
toolchain was available. Independent review found no remaining blocker.
An integration regression caught the general source edit helper clearing the
target actor selection; accepted library edits now retain that validated actor,
while stale/busy guards and rejected/noop action behavior remain unchanged.
Tests cover strict schema/resource bounds, full target replacement, independent
actor/source-library lifetimes, saved projects, source Undo/Redo and real SVG/PNG
frames versus independently authored expectations and sample-clock preview.
Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39089591760/artifacts/12122271546
SHA256: `781e30c3d8a36021db9e5d8e28182719f7725c8372a369685235ff18588021fc`.
This post-merge record changes no implementation. No real recognizer/model,
GPU-device or physical Mac acceptance was performed. These clips copy authored
offsets into the existing shared evaluator; live instances/blending/retiming,
custom/textured rigs, full GPU parity, automatic fresh-engine estimated reuse,
physical acceptance and final third-party licenses remain open.
See [clip library](animation-clip-library.md).


## Explicit authored clip retiming (PR #38, merged 2026-10-18)

- [x] Integrate atomic start/duration mapping of copied authored bone keys,
  finite scene bounds, preserved transforms/easing and precision-collapse rejection.
- [x] Integrate opt-in native retiming controls, explicit target replacement,
  detached invalid values and confirmation resets after option changes.
- [x] Validate saved source, one-edit Undo/Redo, unchanged library data and
  actual SVG/PNG/sample-clock preview against independently retimed tracks.
- [x] Final Linux/macOS CI, independent review and merge.
- [ ] Live clip instances, arbitrary clip blending, cross-project exchange,
  custom/textured rigs and physical motion acceptance.
- [ ] Full GPU parity, automatic fresh-engine estimated-result reuse and final
  third-party artifact/license clearance.

Final source head `fc18aac32c5bc9068885601b23db9d21e323df85` passed Audio
foundation 39090819205 and Apple Silicon preview 39090819133. CI validated
formatting, workspace check, strict Clippy, 388 default Rust tests on Linux and
387 on macOS, 14 adapter-free GPU tests/feature Clippy per platform, 55 runtime
plus 29 inventory/packaging Python tests and three actual encoder tests.
Arm64 packaging/signature and license-byte checks passed. Rust checks ran in CI
because no local toolchain was available. Independent review found no blocker.
Numeric tests cover all seven bones, preserved transform/easing data, compression,
shifted placement, invalid intervals/references, overflow and precision collapse.
Native tests cover detached invalid drafts, action routing, original Apply,
source Undo/Redo and .kilsat persistence. Actual SVG/PNG regressions use
independently retimed tracks and compare sample-clock preview before/after keys
and during repeated/backwards seeks. A difference check originally chose a time
where both mappings correctly cross the same pose; the corrected quarter-second
check verifies independent geometry and visible pixel differences. Explicit
Reset copied key timing restores valid draft values and clears approval.
Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39090819133/artifacts/12122542757
SHA256: `81a68cc17af367e3c1a06237c4aa3f8b09b4ac1c833b273f37baa91247b9e01c`.
This post-merge record changes no implementation. Retiming copies keys into
existing tracks; endpoint holding remains normal bone-track behavior rather than
introducing a live activation window. No real recognizer/model, GPU device,
physical Mac or perceptual acceptance was performed. Full GPU parity, automatic
fresh-engine estimated-result reuse, live custom-clip blending, arbitrary rigs
and final third-party artifact/license clearance remain open.
See [clip library](animation-clip-library.md).


## Five open workstreams assigned (PR #39, merged 2026-10-18)

- [x] Assign a separate agent to each of the five requested open workstreams.
- [x] Inspect current implementations and record bounded next changes, interfaces,
  acceptance tests and evidence blockers in the linked workstream documents.
- [ ] Implement and verify full GPU scene/native parity.
- [ ] Implement automatic estimated-result lookup with verified resources/environment.
- [x] Implement the first original-rig persistent live-instance and ordered override package (PR #40).
- [x] Extend live layers with animated scalar weights and linear fades (PR #41).
- [x] Extend live layers with looping and explicit joint masks (PR #43).
- [ ] Extend live layers with more general mixing.
- [ ] Collect and review physical Mac/device/offline/synchronization acceptance evidence.
- [ ] Complete the final artifact-specific third-party license/redistribution review.

This assignment increment changes documentation only. No renderer, inference,
live blend evaluator or release approval is implemented by these plans. The
parent owns shared schema/scene/desktop files and coordinates implementation
before agents may edit overlapping production code. The existing user-reported
M1 Kokoro speech evidence is retained; it does not close the remaining device
gates. Plans and agent ownership are not completed product acceptance.


## Ordered live authored clip layers (PR #40, merged 2026-10-18)

- [x] Integrate strict persistent scene instances, reference/numeric/order/budget
  validation, atomic upsert/remove/reorder, deletion guards and actor cleanup.
- [x] Integrate prepared shared referenced tracks, checked local-offset override
  blending, half-open windows, unchanged default behavior and typed failures.
- [x] Integrate native detached instance editing, global layer order, library
  reference warnings, source Undo/Redo, persistence and stale/busy/actor guards.
- [x] Reuse prepared snapshots in scene/project export and cached native preview;
  preflight inactive/zero layers and enforce bounded work before output creation.
- [x] Validate independent actual SVG/PNG frames, sample-clock preview, seek order,
  overlap order, root/speech stages, library replacement and copied-track independence.
- [x] Final Linux/macOS CI, independent review and merge.
- [x] Animated weights/fades followed in PR #41; see the integration record below.
- [x] Looping and original-rig joint selection followed in PR #43.
- [ ] Arbitrary/textured rigs, cross-project exchange and physical/perceptual
  motion acceptance.
- [ ] Full GPU parity, verified-environment automatic estimated reuse and final
  third-party artifact/license approval.

Final source head `b6700bdf47f16cdfa900cd960fbf6877f7e29784` passed Audio
foundation 39125173890 and Apple Silicon preview 39125173903. CI validated
formatting, workspace check, strict Clippy, 409 default Rust tests on Linux and
408 on macOS, 14 adapter-free GPU tests/feature Clippy per platform, 55 runtime
plus 29 inventory/packaging Python tests and three actual encoder tests.
Arm64 packaging/signature and license-byte checks passed. Rust checks ran in CI
because no local toolchain was available. Independent review found no remaining
blocker after correcting unused-library preparation and total live-work bounds.
Prepared snapshots store only referenced definitions, share their keys across
instances/clones and are retained across production preview/export frames.
Existing bounded preflight may prepare separately; it retains its early errors.
The work estimate counts inactive/zero layers and binary-search/scanning work;
a regression rejects excessive valid live work before any output directory.
Tests cover strict schema/references/count limits, independent weight/order and
rotation/scale expectations, half-open/adjacent/compressed windows, repeated
seeks, copied/live definition independence, guarded deletion, source history,
project round trips, cross-actor/stale/busy rejection and prepared-storage sharing.
Actual SVG/PNG frames match independent authored expectations and sample-clock
preview with procedural, root and speech stages. Native live projects use the
existing bounded compositor worker rather than reconstructing tracks per draw.
Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39125173903/artifacts/12135299806
SHA256: `de5eb79159a1556c95f6389ca09c833a3247d68c80f169210d559beef71c0231`.
This post-merge record changes no implementation. First support is constant-weight
ordered override of original-rig local offsets, with explicit half-open activation
and no implicit fades. No real recognizer/model, GPU device or physical Mac
acceptance was performed. Animated weights/fades, loops, arbitrary rigs, full
GPU parity, verified-environment estimated reuse and final licenses remain open.
See [live clip blending](live-clip-blending.md).


## Live clip weight curves and fades (PR #41, merged 2026-10-18)

- [x] Integrate backward-compatible per-instance linear fades and local scalar
  weight keys with outgoing easing and exact static-envelope defaults.
- [x] Validate finite durations/multipliers, non-overlapping fades, ordered local
  key bounds and 256/8192/65536 instance/scene/project key budgets.
- [x] Integrate detached native fade/key authoring, explicit reset/Apply, invalid
  draft preservation, source Undo/Redo and project persistence.
- [x] Account for scalar searches/fades in preview/export work estimates even for
  inactive/zero layers; reject over-budget output before creating directories.
- [x] Validate independently expected actual SVG/PNG/sample-clock preview frames,
  faded base-pose boundaries, overlap order, backwards seeks and old defaults.
- [x] Final Linux/macOS CI, independent review and merge.
- [x] Looping and original-rig joint masks followed in PR #43.
- [ ] Custom/textured rigs, cross-project exchange and physical/perceptual
  motion acceptance.
- [ ] Full GPU parity, verified-environment automatic estimated-result reuse and
  final third-party artifact/license clearance.

Final source `9247f6a00c9167e2bc1ec9ffae9caf7518bb2a11` passed Audio foundation 39126660947 and Apple Silicon
preview 39126660943. CI verified formatting, workspace check, strict Clippy,
420 default Rust tests on Linux and 419 on macOS, 14 adapter-free GPU
tests/feature Clippy per platform, 55 runtime and 29 packaging Python tests per
platform, and three actual encoder tests. Independent review found no remaining
blocker. The actual-frame fixture binds PCM and manifest duration to the scene;
frame 8 tests the instance end inside a longer scene. The preflight regression
rejects dormant zero-weight envelope work before any output directory is created.
Local Rust tools are unavailable; these execution results come from CI.

Mac package: https://github.com/aleksipii/kilsatstudio/actions/runs/39126660943/artifacts/12135693647
SHA256: `5738e7406bcb8c36b0b08a381b847801b177c0bb1c2a270551567658ac3bc0f0`.
This post-merge record changes documentation only. This is an envelope for
ordered override weights, not a normalized or additive mixer. No real recognizer,
model, GPU device or physical Mac acceptance is performed by fixture tests. See
[live clip blending](live-clip-blending.md).

## Final package inventory (PR #42, merged 2026-10-18)

- [x] Integrate bounded offline final-ZIP/member inventory and exact current
  resource boundary, licenses, generated metadata and build-info checks.
- [x] Integrate explicit signature/AppleDouble observations, optional helper
  component identity, atomic external sidecar and distinct collection/strict exits.
- [x] Integrate post-signing macOS CI report attachment, independent review and
  package/security regression tests.
- [x] Collect separate resolved target-filtered Cargo closures, compiler identity
  and final-ZIP/member-bound native observations in CI (PR #43).
- [ ] Prove exact compiled/embedded dependency closure and review upstream artifact
  notices, license alternatives and redistribution obligations.
- [ ] Final third-party license clearance, GPU parity, automatic estimated-result
  reuse and physical Mac/hardware/model quality acceptance.

Final source `6bf46d2f2edebcfbd217e5d2927467b88ffb886f` passed Audio foundation 39128020733 and Apple Silicon
preview 39128020717. CI verified fmt/check/strict Clippy, 420/419 default Rust tests
on Linux/macOS, 14 adapter-free GPU tests/feature Clippy per platform, 55 runtime
and 49 repository Python tests per platform, and three actual media encoder
tests. The 20 new offline audit tests cover exact resources/licenses/generated
metadata, optional helper, unsafe ZIPs, budgets, malformed XML/DEFLATE, strict
exits and atomic output/alias protection. Local 49-test Python suite passed;
independent review found no remaining blockers. No Rust code/dependency changed.

Final signed ZIP collection in Mac CI passed with 21 file records,
`package_valid: true` and `distribution_approved: false`. Observed final ZIP
SHA256: `c869d8ebd96a1821a400bb9efa8f79f01cfc29d639969dbd8a148c5f38edfc89`.
ZIP and external sidecar artifact: https://github.com/aleksipii/kilsatstudio/actions/runs/39128020717/artifacts/12137460298
Artifact container SHA256 (distinct from contained preview ZIP): `c44972df2bb305ad4dc8eccd184d3fdeaaa7a65bfdc97163c53ef80c2ce4cb8b`.

See [final package audit](final-package-audit.md). This post-merge record changes
documentation only;
package-boundary success never grants distribution approval. No external assets,
recognizer, GPU device or physical Mac acceptance is exercised by fixture tests.

## Parallel studio workstreams (PR #43, merged 2026-10-18)

- [x] Integrate bounded evaluated-geometry GPU experiment, explicit font-label
  omission/unsupported-feature boundaries, adapter-free command/shader tests and
  full decoded-RGBA comparison metrics. Production full-scene parity stays open.
- [x] Integrate live-clip original-speed looping and explicit seven-joint masks,
  native controls, persisted defaults and independent SVG/PNG/preview tests.
- [x] Integrate persisted soft animated ellipse masks, native Apply/history,
  shared SVG/PNG/preview/sequence sampling and strict geometry/key/work bounds.
- [x] Integrate exact-request immutable engine-index foundation, synthetic
  conflict/corruption/cancellation tests and fresh versus explicit archive origin.
  Production automatic reuse remains ineligible without a verified real package.
- [x] Integrate final-ZIP-bound separate resolved Cargo closures, compiler and
  final-member native observations, retained CI inputs and never-approved report.
- [x] Integrate versioned editable scene-order/duration snapshot using the shared
  sequence clock, native scene-order actions, source history and validation.
- [x] Final Linux/macOS CI, independent cross-stream review and authorized merge.
- [ ] Full GPU/native parity and actual Metal performance; custom rigs, textured
  parts/deformation; artist groups, painting, pressure and image retouching.
- [ ] Verified real recognizer environment/automatic hits, editable mouth timings
  and measured real-audio quality; physical Mac/offline/device/GUI acceptance.
- [ ] Final licenses/redistribution review, production measurements/release,
  separate audio cues, conditional animation rules and general compiled-track editing.

No downloaded model, real recognizer, physical Mac or GPU execution is claimed.
See [GPU experiment](gpu-evaluated-scene.md), [live clips](live-clip-blending.md),
[soft masks](soft-animated-masks.md), [engine reuse](workstreams/estimated-auto-reuse.md),
[build evidence](build-evidence.md) and [editable timeline](editable-timeline.md).

Final source `de1dbc446510e2a895b7138612d80a292814c015` passed Audio foundation 39156985428 and Apple Silicon
preview 39156985436. CI passed workspace fmt/check/strict Clippy, 446/445
default Rust tests, 23 adapter-free optional GPU tests/feature Clippy,
55 runtime and 59 repository Python tests per platform and three media encoder
tests. No local Rust toolchain was available. Cross-stream and final integration
reviews found no remaining blockers. The mask pixel regression uses an excluded
head translation, since leaf-joint rotation alone does not move its plotted point.

Mac package, package audit, build report and retained inputs: https://github.com/aleksipii/kilsatstudio/actions/runs/39156985436/artifacts/12147780773
Artifact container SHA256: `42877ed0a8258ac459c7d59640aaec0448d50dc32817f303825993aa4b493be9`.
Final signed preview ZIP SHA256: `331db8d6e9c873c17c3e63b640e930ff4cae1569fb9d947ba149bcc4de1a5be6` (distinct from artifact container).
The package audit accepted 21 file records; build collection completed with
`distribution_approved: false`. These observations do not prove exact compile
units, upstream rights, physical device behavior, real recognizer quality or
GPU/Metal execution. No model/voice assets were downloaded or inference run.
This post-merge record changes documentation only.
