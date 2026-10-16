# Master prompt implementation and acceptance tracker

Source of requirements: [master build prompt](master-build-prompt.md). Updated
2026-10-16. A checked item identifies integrated, inspected implementation, not a
claim that every hardware or product acceptance test has passed. An unchecked
item includes work in progress until integration and relevant checks succeed.
The parent integrator owns completion marking and evidence. Never replace a
hardware, real-model or perceptual gate with mocked success.

## Latest merge status

PR #28 merged as d9370611fcb19ab0f5bdf75ee28342064161e66e after final
Linux/macOS CI and independent review. `kilsatstudio1.0` was fast-forwarded to
that merge. This preserves PR #26 animation clips and PR #27 user-reported Mac
Kokoro success. All PRs open at this task's initial inspection are now merged.

| Completed | Still open |
| --- | --- |
| Timed solid colors/images, native editors and shared preview/export | Native whole-project preview/transport and MP4 sequence workflow |
| Bounded joint PoseTrack sampler and weighted pose-blend API | Persisted per-bone editor, textured rigs and preview/export integration |
| Rule-based posture, walk, gesture and speech-motion clips shared by preview/export (PR #26) | Artist-authored/persisted clips, per-bone editor and Mac visual review |
| Atomic duration/composition settings and source Undo | Full unified project/asset history and advanced editor tools |
| Managed Kokoro setup, bound alignment/cache and user-reported M1 speech tests | Offline repeatability, scene/device acceptance, automatic alignment and quality |
| CPU SVG/PNG/MP4 and bounded parallel frames | GPU rendering, production benchmarks and final distribution/license audit |

Maintain this summary and the full checklist after each authorized merge. A new
implementation remains pending until its relevant checks pass; hardware gates
require actual evidence.

## Ownership and current increment

| Workstream | Owner in this increment | Integration boundary |
| --- | --- | --- |
| Architecture/integration | Parent agent | Workspace, shared interfaces, final checks |
| Compiler / project | Existing modules + parent | Validated script/schema integration |
| Rigging/animation | Existing modules | Original rig and animation modules |
| Rendering | Existing render modules | Shared deterministic frame path |
| Desktop UX | `project_sequence_ui` + `asset_thumbnails` | Global transport and native image inventory |
| Speech | `project_sequence_worker` | Bounded project preparation and shared alignment cache |
| Export | `project_sequence_export` | Shared raster preview, MP4 and .kilsat CLI |
| Scheduling | Existing bounded frame/decode workers | Bounded execution and cancellation |
| QA/release | Independent review + parent | Packaging tests, integrated checks |
| Documentation | All owners + parent | This tracker and feature documentation |

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
- [ ] Native whole-project sequence preview/transport and MP4/SVG sequence export.
- [ ] Audio cue commands distinct from dialogue import.
- [ ] Reusable/conditional rules and extensible animation presets.
- [ ] Stable versioned editable compiled timeline representation.

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
- [ ] Persisted per-bone custom tracks, rig editing and preview/export integration.
- [ ] Animation blending integrated with persisted tracks, desktop preview and export.
- [x] Integrated rig/animation transform, seek and interpolation tests.

## 4. Graphics and rendering

- [x] Native animated scene preview with scene-coordinate scaling.
- [x] Shared evaluated scene and deterministic SVG frame renderer.
- [x] Basic background and original procedural character composition.
- [x] Bounded PNG/JPEG foreground image layers shared by native preview and SVG/PNG/MP4.
- [ ] Textured per-bone character sprites and deformation.
- [x] Actor z-order/visibility composition metadata and editor.
- [x] Persisted ordered/visible/opacity image and basic drawing layers with rectangular clip masks.
- [ ] Layer groups, arbitrary masks and blend modes.
- [x] Camera positioning/zoom shared by native preview and SVG/PNG/MP4.
- [x] Solid background color, timed step-color tracks and scene-edge fades to black.
- [x] Timed background image planes and validated half-open visibility intervals.
- [x] Whole opaque-frame cuts/crossfades in headless PNG sequence export.
- [ ] Native transition preview and sequence MP4/SVG compositing.
- [ ] Offscreen GPU renderer using a verified macOS Metal path.
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
- [x] Bounded artwork history and source/reference guards (separate histories; not full unified Undo).
- [x] SVG/PNG/MP4 background export selection and explicit FFmpeg path.
- [x] Audio waveform display and prepared-dialogue preview controls.
- [x] Explicit local asset import/relink/remove-reference UI and bounded worker.
- [x] `.kilsat` inventory dirty tracking and one-step inventory Undo.
- [x] Background PNG/JPEG decoding, texture loading and foreground image rendering.
- [ ] Asset thumbnails and full unified asset history.
- [x] Actor layer order/visibility and composition camera-key editor.
- [x] Character root-transform keyframe editor plus pen/rectangle/ellipse canvas tools and rectangular masks.
- [ ] Per-bone animation editing, pressure brushes, arbitrary masks and image retouching.
- [x] Camera controls, timed background-color editor and scene-edge fades.
- [x] Outgoing Cut/Crossfade editor with atomic settings Undo and invalid-overlap rejection.
- [ ] Whole-project sequence preview/playback and desktop sequence-export controls.
- [ ] Full project/asset history and shortcut/accessibility coverage.
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
- [ ] Automatic alignment backend and actual quality acceptance; report tooling does not establish these.
- [x] Consented managed Kokoro provisioning with pinned asset revisions, sizes and SHA256.
- [ ] Complete dependency/asset license and redistribution audit.
- [x] Real local English Kokoro CPU WAV on Apple Silicon (user report 2026-10-16: M1, macOS 27.1, both voices; see [acceptance](kokoro-in-app-acceptance.md#recorded-mac-evidence)).
- [ ] Offline synthesis after provisioning with networking disabled.
- [x] Exact-PCM/transcript/provider/config/manual-edit alignment cache with atomic writes and headless import workflow.
- [x] Exact-PCM/current-text bound alignment envelopes and shared line/scene desktop cache integration.
- [ ] Automatic aligner execution and validated provider integration.
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
  interpolation and root-transform editor are implemented; textured custom rigs,
  per-bone controls and animation blending remain open.
- [ ] **M3 — Dialogue/lip sync:** import/tracks/mouth/timing foundations implemented;
  real local Kokoro, Speech Stages 2–4 and quality/hardware gates remain open.
- [ ] **M4 — Professional editing:** timeline and source Undo/Redo implemented;
  actor order/visibility, camera tracks and inventory tools implemented; transform
  root-keyframe/basic drawing/image layers now implemented; advanced assets,
  arbitrary masks/retouching, unified history and transitions remain open.
- [ ] **M5 — Production rendering:** SVG/audio baseline implemented; GPU offscreen,
  CPU PNG/MP4 and bounded parallel frame jobs implemented/tested; GPU rendering
  and production performance/hardware acceptance remain open.
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

## Scene sequence implementation increment

Current work adds validated outgoing cuts/crossfades, a shared global 24 kHz
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

The checked scope is authoring plus headless PNG/WAV sequence export. Native
whole-project preview/transport, MP4/SVG sequence export and CLI `.kilsat` resource
loading remain open. Per-bone editing/textured rigs, advanced compositing, unified
history, automatic alignment/quality, offline/device acceptance, GPU/performance
and final licensing/distribution gates also remain open. User-reported first
real M1 Kokoro success (PR27) remains distinct from those comprehensive gates.

## Whole-project desktop increment (in validation)

- [x] Merge PR #28 and synchronize the release branch; retain all earlier Mac evidence.
- [ ] Integrate and validate native whole-project preview, global playback/seek and waveform.
- [ ] Integrate and validate shared scaled raster preview and PNG/MP4 sequence export.
- [ ] Validate .kilsat sequence CLI resource loading with canonical image bounds.
- [ ] Integrate and validate asset thumbnails and virtualized image inventory.

Four agents implemented these scopes; independent review and parent integration
checks are running. Source in the current development branch is not yet final
acceptance. Real Mac sequence device synchronization, offline Kokoro repeatability,
perceptual timing, full unified history, per-bone textured editing, advanced masks
and blending, GPU/performance and final license audit remain open.
