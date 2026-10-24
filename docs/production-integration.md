# Production animation integration

Work branch: feat/production-animation-pipeline, based on main
2a0168a5d3065157033e30a8abc8947a5cc9482c.

PR #89 head 64d7293b09602c84a12254500554b2a00f2b9d7f is integrated in this
work branch: rounded custom-rig shapes and outlines. Its original CI passed;
combined-source CI and artistic review are still required.

## Implemented workflow (2026-10-24, PR #95)

- The Finnish desktop quickstart offers measured-audio direction. A bounded
  background worker prepares actual PCM speech or reads explicitly imported WAVs,
  measures sample counts and creates editable acting, reaction and camera shots.
  Applying the result is one Undo operation; changed source, project, dialogue,
  voice profile and cancelled jobs cannot publish stale edits.
- Finnish Piper speech is supported in schema, preparation, selected dialogue,
  scene and sequence playback/export. A native profile editor binds separately
  installed local files. Installed Mac system voices are private preview only.
  Finnish mouth estimates use a separate, explicitly unmeasured estimator.
  [Real-audio quality measurement](finnish-speech.md) requires a named, manually
  annotated reference; synthetic test tones cannot pass speech-quality acceptance.
- Camera streaming and recording are explicit local operations. The operator
  reviews and applies head-tilt keys through Undo. Live preview leaves the source
  untouched; Stop alone does not apply motion. Actual camera access requires a
  physical Mac and user permission.
- The streaming MP4 profile supports up to 1920x1080/24fps/60seconds with bounded
  working memory, output bytes, cancellation and deadlines, without PNG sequences.
  The separate benchmark includes actual speech, preparation and media validation.
- GPU commands cover rig outlines, native caption glyph contours, animated image
  props and scene lights. CPU/GPU regression fixtures are retained. This is not
  blanket production GPU or physical Metal acceptance.
- [The original Finnish season](kilsat-season.md) has 31 validated `.kilsat`
  projects, 248 authored dialogue turns and seven compilation groups (six groups
  of five plus the finale). The runner creates real speech/video and verifies
  decoded frames, track start/duration, codecs, samples and file hashes. Compiled
  video is rebased to the same 24kHz clock, retaining delayed H.264 frames.
- [Developer ID tooling](mac-production-release.md) signs nested helpers, checks
  the exact team, submits only with explicit consent, requires Accepted, staples,
  assesses and archives final bytes. The workflow is manual and needs an operator's
  Apple credentials; no actual Developer ID/notarization is claimed.

The offline Finnish [desktop guide](../apps/studio-desktop/KAYTTOOHJE.md) now has
twelve topics including real speech, profile creation, Full HD and camera recording.

## Actual integration evidence

The final-source CI links and results are recorded in [master-progress.md](master-progress.md).
Local checks have passed rustfmt, a strict all-target/all-feature Clippy-backed
workspace check, 249 desktop tests (one pre-existing environment-dependent test
excluded locally), speech/voice tests and 121 repository Python tests. The broad
local Rust run exposed a Python fixture's incorrect `Path.parent()` call; it was
fixed to the property `Path.parent`, then the speech/voice/desktop suites passed.
Three pre-existing tests need unavailable `/proc` host information in this sandbox;
normal CI runs them unfiltered. None are changed to bypass their assertions.

An actual local Finnish finale produced 1,050 frames / 43.75seconds at 960x540/24fps.
Its complete speech/preparation/export run took 46.23seconds; it is explicitly a
review render, not a physical-Mac or Full HD performance claim. The corrected
compilation retained all 1,050 frames. A separate real-FFmpeg regression joins
two H.264/AAC clips with delayed frames and verifies all 1,440 decoded output
frames and simultaneous zero starts for audio and video.

On source `88d39ac80133f511c60d1b8e3994701e9ada0b21`,
[the Apple Silicon CI measurement](https://github.com/aleksipii/kilsatstudio/actions/runs/40126710338/artifacts/12572953265)
completed the actual Finnish episode-one pipeline in **77.197seconds**, including
external validation. The H.264/AAC video is **1920x1080, 24fps, 1,025frames,
42.708333seconds**. Host observations are Darwin24.6.0/arm64, VirtualMac2,1,
three logical CPUs and 7GiB RAM. CI and virtualization are both observed: the
numeric 300-second gate passes; the physical-Mac gate remains false. Artifact ZIP
SHA-256: `d09c2e47a863d62282ec0f9f1f701d45d674ff1e744ee53e916daad6aadf73fe`.

The same [season CI run](https://github.com/aleksipii/kilsatstudio/actions/runs/40126710338)
completed all seven groups: **31 actual Finnish episode MP4s and seven actual
compilation MP4s at 960x540/24fps**, each checked against decoded frames, audio
sample counts, track starts/durations and input/media hashes. These are review
renders. Request revision three explicitly selects full_hd with no review-size
override for the subsequent complete season run. Publication remains unapproved.

## Implementation ownership

- direction_orchestrator: measured-audio direction worker, acting, reactions and camera.
- finnish_speech: offline Finnish providers and annotated lip-sync measurement.
- continuous_camera: explicit local tracking, recording and reviewed head keys.
- fullhd_performance: bounded streaming Full HD export and end-to-end benchmark.
- season_visuals: 31 original Finnish episodes, assets and resumable production.
- gpu_production: outlines/lights/animated props and measured parity fixtures.
- Parent integrator: schemas, desktop integration, release/Mac tooling, tests,
  GitHub checkpoints and truthful acceptance evidence.

## Acceptance boundaries

Product acceptance is still open. No subsystem is accepted merely because an
agent or interface exists. Physical camera/GUI/audio behaviour and a physical
Mac <=300-second whole-pipeline measurement require real evidence. Apple system
voices are for private preview only; final season audio needs separately
permitted local speech or cleared imported recordings. The original missing
season is not being represented as recovered; new stories are authored here.
The Kilsat prop is illustrative until a real approved capture is supplied.

Production Developer ID signing/notarization requires an authorized Apple
identity and signing environment. Final distribution, asset and dependency
rights are not granted by CI, generated inventories or ad-hoc signing.
