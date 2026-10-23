# Remaining studio work and active agents

Updated 2026-10-19. The first packages below merged in PR #55 after PR #54.
The authoritative requirements, completion marks and acceptance evidence remain
in [master-progress.md](master-progress.md), based on the
[master build prompt](master-build-prompt.md). Historical unchecked rows are not
automatically current missing features. Integration is owned by the parent agent.

`[x]` means integrated and checked; `[ ]` means still open. The table's concrete
packages are integrated; its final column remains open. Code tests cannot grant
physical-device or rights approval. Delivery evidence is below and in the master.

| Area / agent | Current concrete package | Further work left open |
| --- | --- | --- |
| Characters — `remaining_character` | Versioned bounded original-rig clip clipboard exchange between projects, explicit replacement and shared source history | Live (non-copy) custom-rig clip instancing, mirroring, multi-clip mixing and rest-orientation correction, a visual (canvas) weight/UV editor (numeric part texture/UV/two-joint weight controls: branch feat/rig-weight-uv-editor), idle and physical motion review (acting layer with part variants, viseme/emotion sprite swap, gaze and seeded blink: branch feat/rig-acting-layer, see custom-rigid-characters.md; CPU-rendered texture binding and two-joint skinning for custom rigs: branch feat/rig-textures-deformation; copy-based custom-rig clip library with joint mapping, translation scale and retiming: PR #57) |
| Artist tools — `remaining_artist` | Scene-local animated image-mask rectangles, native key editing, shared preview/export and seek tests | Animated polygon vertices/painted strokes, non-uniform mask scale, retouch/image processing and physical stylus acceptance (rigid polygon/painted mask motion: PR #56) |
| Speech — `remaining_speech` | Exact-audio sample-boundary mouth-shape editing, explicit manual provenance and Apply through existing history | Real forced-alignment integration, audited recognizer environment, annotated corpus and measured/perceptual quality |
| Screenplay — `remaining_screenplay` | Validated cinematic push/pull/pan camera presets using existing camera tracks, detached native Apply; [reusable/conditional animation rules](animation-rules.md) and [audio cues](audio-cues.md) with detached native cue/rule editors (headless-tested only) | Cue waveform display/file picker, hand-exercised cue/rule editor UI and general editable compiled tracks (YAML-authored audio cues on the shared sample clock: see [audio cues](audio-cues.md)) |
| Captions — professional-animation workstream 1 | Speaker-coloured captions from `speak` events on the CPU raster path ([captions](captions.md)): bundled OFL font in NOTICE, resvg text with a bundled-font-only database, project caption style, deterministic wrapping, preview/SVG/PNG/video agreement, missing-glyph errors, desktop Captions Follow/Show/Hide override for previews and exports persisted in `.kilsat` (headless tests only) | Caption style editor UI in the desktop app, per-event caption overrides, kerning-aware wrapping, non-Latin scripts, GPU captions, and human review of typography |
| GPU — `remaining_gpu` | Image alpha/luminance masks in the existing bounded GPU experiment, animation sampling, relocation and shader/command tests | GPU text (the CPU path now draws captions with a bundled font, see [captions](captions.md); the GPU path still omits all text, see docs/gpu-parity.md), tightening parity beyond the recorded regression-guard tolerances, production GPU preview/export and Metal performance (straight-edge feathered polygons: PR #58, corners approximate; measured 25-fixture CPU/GPU harness with per-fixture tolerances, including animated polygon/painted masks and flat/textured/skinned rig parts on the GPU: docs/gpu-parity.md) |
| Performance — parent | Opt-in bounded application-update timing window, active/idle background-work summaries and copyable report; process memory budget (images, export, decoded cue PCM, prepared/mixed sequence PCM, preview frames); dependency-DAG scheduler driving sequence preparation (scene audio, mix, peaks; one worker by default); opt-in application-side input-to-next-frame recorder; hand-run release benchmarks on one Apple M1 (see [jobs-performance](jobs-performance.md)) | Budgeting synthesized dialogue PCM and GPU/egui memory, parallel speech inference (needs a safe-concurrency review of the sidecar), hardware input-latency runs, display/photon latency and benchmarks on other machines/other workloads |
| Cinematography — workstream 5 | [Shot library](shot-library.md): framing presets, dialogue cut rules and bounded reaction holds expanding to ordinary scenes/camera keys; original night-room reference sequence integrating captions (white father, yellow boy), custom-rig acting layer (text-estimated mouth variants, gaze, seeded blinks, brows), lights (violet multiply tint, vignette, flickering lamp, phone screen glow, green hallway light) and a generated animated phone-screen prop in ONE project, with timing report, contact-sheet dump, PNG-sequence export and golden-property tests (CPU path; placeholder tones by default); an ignored test and `scripts/night_room_real_speech.py` rebuild it around real offline Kokoro WAVs (one hand run recorded, structural only) | MP4 of the sequence (no FFmpeg on the build machine), editor UI for shot lists, human review of art, framing, comic timing, voice, mouth/lip-sync quality and the licences (Anton font and CMUdict are not approved), mouths from forced alignment instead of text estimates, light drawn behind (not over) actors, GPU path (no captions/lights/animated props) |
| Mac — `remaining_release_mac` | Build-bound per-scenario evidence completeness in the existing operator report, with truthful untested/fail states | Physical offline Kokoro, devices/sync, Finder, shortcuts, accessibility, pressure and end-to-end workflow |
| Lighting and props — workstream 4 | Bounded deterministic scene lights (ambient multiply, vignette, radial screen/multiply glows with tracks, actor attachment and sine/noise flicker) and frame-sequenced animated image props with a screen-glow helper, SVG/preview/PNG/export shared, validated by project, history and export preflight ([lighting-props](lighting-props.md)); GPU path reports both as `UnsupportedFeature` | Native inspector/editor controls (authoring is project-artwork JSON only), GPU rendering of lights and animated props, spot cones and shadows, perceptual/hand review of the look |
| Real-speech reference CI — integration increment 3 | Manual, consent-gated `real-speech-reference.yml` ([doc](real-speech-reference-ci.md)): macOS arm64 runner installs Kokoro via `setup_kokoro.py` at the pinned revision with SHA-256-verified model and selected voices (all 28 English voices pinned), content-keyed cache with re-verification, existing synthesis script and night-room export, artifact with MP4, WAVs, contact sheet, timing report and provenance (hashes, licence record, versions, runner, timings, RTF); helper unit-tested, YAML actionlint-clean | First dispatched run on GitHub and its evidence, listening review, a reviewed boy voice, audio-aligned mouths, licence audit of transitive packages and generated media |
| Release/licences — `remaining_release_mac` | Preserve exact artifact binding and unresolved review gates while improving evidence coverage | Final upstream component/asset rights review, NOTICE corpus, production packaging and Developer ID/notarization decision |

## Acceptance for this increment

- [x] Integrate the concrete packages above without duplicating clocks, renderers
  or document histories; preserve old project defaults and unrelated work.
- [x] Run relevant format/check/strict Clippy/Rust, Python, GPU shader/contracts
  and media CI; record real counts and distinguish ignored hardware tests.
- [x] Review changed public types, enum consumers, duration/resource preflight,
  history/persistence, cancellation and source-context protection.
- [x] Update this file and [master-progress.md](master-progress.md) after merge
  with exact source/merge identities, actual checks and still-open scope.
- [ ] Collect external Mac, real-speech, GPU-quality/performance and licensing
  evidence. These gates remain open until supplied and reviewed.

## Existing completed baseline

PR #55: merge `c03bfe21779e9ed357c700f930f6d10086a5da86`, final source
`c98ac9cbb1cc909114ebd3707ebe3a0f53194f3a`. Linux/macOS CI passed fmt, check,
strict Clippy, **543/542 default Rust tests**, **35 GPU library + 2 probe tests**
per platform (3 hardware tests ignored), **3 real FFmpeg tests**, and **55 runtime
plus 82 repository Python tests** per platform. Three encoder tests ignored by the
default suite were explicitly run by the media job. Apple Silicon packaging passed.
See the [CI run](https://github.com/aleksipii/kilsatstudio/actions/runs/39248869347)
and [Mac package](https://github.com/aleksipii/kilsatstudio/actions/runs/39248869339/artifacts/12185025968).
Source review corrected timing loss on YAML save, runtime/history invalidation,
stale mask drafts and measurement wording. Manual timing currently starts from a
fresh/reused/loaded estimate; directly reopening applied manual edits and waveform
dragging remain open. Update timings are wall-clock observations, not measured
input/display latency or process-wide memory. Final rights approval is unchanged.

- [x] Custom rigid definitions and custom-rig joint tracks/editor (PRs #46/#50).
- [x] Original-rig override/additive/normalized clips, weights/fades, loops and
  joint masks (PRs #40–#46).
- [x] Groups, image masks, polygon feathering and vector mask painting/erasing
  (PRs #46/#48/#49/#53).
- [x] Soft mask strokes, actual-event pressure handling and experimental painted
  GPU mask commands (PR #54); physical Mac stylus and measured parity stay open.
- [x] Automatic estimated-cue reuse for explicitly enrolled supported local
  packages (PR #46); environment/quality approval stays open.

M1–M6 remain unaccepted as complete products. See the individual gates in
[master-progress.md](master-progress.md#whole-product-milestone-gates).

## Increment after PR #55 (2026-10-19)

Merged with green Linux/macOS CI and local fmt, all-features Clippy and
`cargo test --workspace` on an Apple M1:

- [x] PR #56 animated polygon and painted-mask motion (CPU preview/export; GPU reports unsupported).
- [x] PR #57 custom-rig clip library: capture, retargeting by joint map and translation scale, retiming.
- [x] PR #58 experimental GPU feathered polygon masks; one ignored hardware test was run by hand on Metal (M1) against an analytic edge profile.

Not attempted in this increment because they need devices, external models or
decisions rather than only code: physical Mac/offline/audio/stylus acceptance, real
forced alignment and an annotated corpus, perceptual quality, final licence and
distribution review. Software items still open and unassigned to any increment:
reusable/conditional animation rules, process-wide memory
budget, dependency-DAG scheduling, input-to-display latency measurement, GPU fonts,
measured full CPU/GPU parity and textured/deformed custom rigs.

## Increment after PR #59 (2026-10-20)

- [x] PR #60 CPU/GPU parity harness with recorded numbers (`docs/gpu-parity.md`).
- [x] PR #61 custom-rig textures and two-joint skinning.
- [x] PR #62 process memory budget, dependency job graph, input-latency recorder.
- [x] PR #63 audio cues. [x] PR #64 animation rules.

Still open in software: native cue and rule editors, GPU text (needs a decision on
labels and a licensed font), GPU support for textured/skinned parts and animated
masks, wiring the job graph into a real pipeline, budgeting audio/render caches,
weight/UV editors, tighter parity tolerances. Device, speech-quality and rights gates
are unchanged and need external evidence.

## Professional dialogue animation, workstream 2 (English lip-sync data)

- [x] Text normaliser, CMUdict lexicon plus rule-based fallback, validated
  `pronunciations:` overrides, pre-render report and an estimated viseme track
  (`docs/english-lipsync.md`). Unit tests cover structure and determinism only.
- [x] Text-estimated visemes reach scene preparation, preview and export with
  precedence manual > forced > Rhubarb > text, pronunciation-aware identity and typed
  diagnostics (`docs/estimated-visemes-pipeline.md`). Structural and pixel tests only.
- [ ] Open: perceptual lip-sync quality is unmeasured (no annotated audio), a real
  forced aligner would replace these estimates, and the sequence panel does not list
  the new diagnostics.
