# Next Mac device acceptance package

## PR #46 delivery and remaining scope

PR #46 adds an offline artifact-bound operator observation collector; see
[report guide](../release-license-report.md) and [master progress](../master-progress.md).
Its synthetic tests do not run a physical Mac procedure. Offline Kokoro, device
switch/recovery, timing, Finder, shortcuts, accessibility and complete interactive
acceptance below remain open.

Owner: `mac_device_owner`. Planning only, 2026-10-18. No physical device,
model, recognizer, Finder or graphical acceptance was executed for this handoff.
Use [the existing device procedure](mac-device-acceptance.md) and
[practical testing](../practical-testing.md) as execution references.

## Evidence boundary

Preserve the original [2026-10-16 report](../kokoro-in-app-acceptance.md#recorded-mac-evidence):
Apple M1, reported macOS 27.1, Homebrew Python 3.12, `main` build `98d265b`,
practical preview run #61. The user reported managed profile installation and
audible English CPU speech for `af_heart` and `am_michael`. No WAV, smoke JSON or
BUILD-INFO was attached. It establishes that reported result on that build only;
it does not approve offline use, another build, multiple speeds, device changes,
scene synchronization or video. Keep its fields verbatim in the historical record.

The current `.github/workflows/macos-preview.yml` builds arm64 for macOS 13+,
packages and verifies an ad-hoc signature, and retains final-ZIP-bound audit and
build observations. `scripts/build_evidence.py` deliberately emits
`distribution_approved=false` and `physical_device_acceptance=not_granted`.
Cargo graphs, `lipo`, `otool`, package hashes and CI success establish build
observations; they do not establish a GUI launch, playback, offline inference,
Metal scene parity, notarization or license approval. Do not reuse a historical
artifact identity as the tested current build.

## Required operator inputs and retained evidence

A human operator needs a physical Apple Silicon Mac running macOS 13+, a trusted
exact preview artifact, keyboard, working built-in audio, and a reconnectable
USB/headphone/Bluetooth output to exercise device switching. Record the connection
kind; Bluetooth adds latency and must be reviewed separately. VoiceOver and macOS
keyboard navigation are needed for accessibility observations. A longer fixture
and screen/audio recording capability help investigate drift; recording does not
itself measure hardware latency.

For speech, supply an already installed authorized local Kokoro CPU profile,
Python 3.10–3.12, config/model and both voice files, plus local language resources.
The operator must be able to disable external networking. Installation/download
consent is a separate explicit action; offline testing never grants it. Supply an
owned two-scene project, imported WAVs/manual timing when used, an installed
absolute FFmpeg path, and a named Mac video player/SVG viewer. Do not upload model
weights, voice tensors or the Python environment.

Create a new evidence directory outside the checkout for every run. Retain:

- Original downloaded Actions archive and contained preview ZIP, each filename
  and SHA256; run URL/ID, source commit, extracted BUILD-INFO, audit JSON,
  build-evidence JSON and available `build-inputs` observations.
- Exact `.kilsat`/YAML fixture and owned media/alignment files; settings, text,
  voices, speeds, scene/sample boundaries and export dimensions/frame rate.
- New WAV/smoke pairs, human review/reference/candidate and acceptance JSON when
  used; setup/prerequisite diagnostics and failures/cancel logs.
- PNG/SVG frames, `audio.wav`, `sequence.json`, `sequence.mp4`, reviewed screenshots
  or recordings; Metal `report.json` and `fixture.rgba` if tested.
- An operator report with UTC date, reviewer, Mac/chip/architecture, macOS, build,
  Python and dependency/model/profile identities, network state, output device,
  player/viewer versions, test ID, exact steps, expected/actual outcome, errors,
  evidence filenames/hashes and `pass`/`fail`/`not_tested` per observation.

Leave unavailable observations `not_tested` with a reason. Review private paths
before sharing. A failed test remains failed until a new build/run is identified;
do not overwrite previous evidence. Agree and record timing tolerance and the
frame rate before judging perceptual drift; this document invents no quality limit.

## Ordered execution and pass criteria

1. **Baseline package and model-free controls.** Follow practical testing's launch
   instructions; retain current BUILD-INFO. Verify editor, Dock/Finder icon,
   invalid-YAML diagnostics and in-app Open/Save `.kilsat` round trip. Load
   **Load sound-check demo (synthetic tones)** and use **Play scene audio**,
   pause/resume, seek, stop and EOF restart. Pass requires audible tones and
   responsive controls with no stale audio; silent visual fallback is not a
   device pass. This fixture is explicitly synthetic, not speech evidence.
2. **Offline Kokoro.** Reload an installed profile after app restart. Disable
   external networking, record that method and run prerequisites. Generate new
   uncached text for each voice at speeds 0.75 and 1.25; retain four WAV/smoke
   pairs and listen on the named output. Pass requires real non-silent speech,
   intelligible complete words, no observed clipping/truncation, responsive UI,
   and no setup/download attempt. Repeat identical requests for cache reuse;
   change text/speed for distinct output. Record synthesis success separately
   from listening/device results. Cancel an uncached preparation, wait for
   completion, retry and confirm no cancelled result becomes playable.
3. **Devices and recovery.** Repeat tone and real-speech playback on built-in and
   external output. Switch the macOS default while paused and playing, disconnect
   and reconnect the external device, then retry Play. Record where sound actually
   emerges, delay, errors and recovery/restart needed. Pass requires the agreed
   device behavior, actionable failure and successful recovery without a hung UI;
   do not assume automatic hot switching. `playback.rs` retains an opened output
   stream and retries default-device opening only when none exists.
4. **Seek, pause, offset, trim and overlap.** First use
   `examples/scene-sequence.yaml`: expect duration 1.500 s, samples 12000/18000/24000
   at 24 kHz red/equal red-blue/blue. Seek both directions paused and playing;
   check EOF restart and that single-scene/project transports do not overlap.
   Then use the owned real-dialogue fixture with distinguishable lines overlapping
   across the transition, nonzero dialogue `at`, and exact-audio manual/imported
   timing. Check before/on/after onset, overlap start/middle/end, final speech and
   EOF; repeat pause/resume and reverse seeks. Compare audio, cursor, movement and
   mouths at the same sample. Pass requires no stale onset, duplicate playback,
   unexpected gap, old prepared result or observed drift beyond the predeclared
   tolerance. Source edits must stop/invalidate preparation. Native Rodio position
   is a consumed-source clock sampled about every 10 ms, not measured output latency.
   Source trimming is a lower-level contract: the inspected MotionScript/native
   workbench has no general persisted audio-trim editor. Do not invent a trim
   control or count a pre-trimmed WAV as exercising `trim_start_frame`; retain the
   automated contract result and leave physical nonzero-source-trim acceptance
   blocked until a reviewable fixture/editor path exists.
5. **Real video and cancellation.** Export that exact prepared fixture to three
   fresh directories: **PNG + WAV**, **SVG + WAV**, **MP4 + WAV** (explicit installed
   FFmpeg absolute path, even dimensions). Retain manifests and media. Compare
   identical transition samples with preview; inspect SVG in the named viewer.
   Play MP4 on the same physical output; inspect onset, overlap, mouths, final
   line and a longer run within export limits. Pass requires matching structural
   duration/frame counts and a separate passing human synchronization review.
   Invalid encoder/existing output must fail safely. Cancel frame export and MP4
   encoding where observable: only the attempt's partial output may be removed,
   previous exports must remain unchanged, and retry must succeed.
6. **Finder events and document safety.** Test cold-launch and already-running
   `.kilsat`, `.yaml`, `.yml` through double-click, Open With and Dock drop;
   include a path with spaces and Unicode, invalid input and an unsaved document.
   Capture which file opens and preservation/prompt behavior. Pass requires actual
   file delivery exactly once without silent loss of edits. Current plist type
   declarations alone do not deliver native open events; this gate is open.
7. **Shortcuts and accessibility.** Exercise Command-Z/Shift-Command-Z across
   source, artwork and asset edits in chronological order, then in screenplay,
   path, search/settings text fields; field undo must remain local where intended.
   Test busy/cancel/error states and keyboard focus visibility/reachability for
   Open, Save, Validate, transport, setup, export and dialogs. With VoiceOver,
   record names, roles, values, focus order and error/progress announcements;
   inspect contrast and enlarged system text. Pass only the observed controls;
   missing labels or inaccessible primary operations are defects, not a blanket
   accessibility pass. Only document Undo/Redo shortcuts are explicitly wired in
   the inspected workspace handler; do not claim Command-O/Command-S support.
8. **Full workflow regression.** From a fresh launch, open/edit/validate, author
   movement/artwork/timing, Undo/Redo, save/reopen, prepare offline speech, play,
   seek/pause, export and replay real video. Verify owned asset references survive
   restart, source/history stay intact after standalone speech tests, and failures
   can be corrected. Retain the resulting project and exports. Overall acceptance
   remains open while any required preceding gate fails or is `not_tested`.

## Exact existing commands

Run from a checkout matching BUILD-INFO, with the installed profile's Python
active. Replace `/local` paths; parents must exist and output files must be new.
These are operator instructions, not commands executed by this handoff.

```sh
sw_vers
uname -m
python3 --version
shasum -a 256 /local/downloaded-artifact.zip /local/Kilsat-Studio-Preview-arm64.zip
python3 services/kokoro-runtime/smoke_macos.py \
  --allow-local-inference --config /local/config.json \
  --model /local/kokoro.pth --voice /local/af_heart.pt \
  --text 'Offline device check, one new sentence.' --language en-us --speed 0.75 \
  --corpus-id mac-af-075 --output /local/mac-af-075.wav \
  --report /local/mac-af-075.smoke.json
python3 services/kokoro-runtime/acceptance.py \
  --wav /local/mac-af-075.wav --smoke /local/mac-af-075.smoke.json \
  --reference /local/reference.json --candidate /local/candidate.json \
  --review /local/review.json --output /local/mac-af-075.acceptance.json
shasum -a 256 /local/mac-af-075.wav /local/mac-af-075.smoke.json
```

Repeat the smoke command with `am_michael.pt`, the second speed and unique IDs,
text/output paths. Expected smoke fields include WAV SHA256, text/language/speed,
model/config/voice hashes, package/platform identities,
`backend=Python/PyTorch CPU`, `provenance.kind=recorded_local_model`,
`synthesis=structural_checks_passed`, `alignment=not_aligned` and
`perceptual_validation=requires listening`. Exit zero establishes structural WAV
checks only. Acceptance requires exact-audio manually annotated phoneme reference,
matching candidate labels and the existing human-review schema; it always emits
`production_acceptance=not_granted`. Use [viseme quality](../viseme-quality.md)
for Rhubarb shapes; do not relabel shapes as phonemes or run a recognizer merely
to complete this device package.

## Small next code package and ownership boundaries

After observations, keep repairs bounded and separately reviewed:

- Finder delivery: `apps/studio-desktop/src/main.rs` plus
  `apps/studio-desktop/Cargo.toml` only if a native integration dependency is
  required. Route startup/runtime file events into existing validated open logic,
  queue safely while busy, and define unsaved-document behavior. Packaging changes
  in `scripts/package_macos.py` belong to the release owner; declarations already
  exist and are not the missing delivery mechanism.
- Device recovery: `apps/studio-desktop/src/playback.rs`; first reproduce stream
  loss/default-switch behavior on hardware, then bound reopen/retry and state
  publication without inventing a second clock. Keep shared trim/mix rules in
  `crates/motion-audio/src/lib.rs` and timing in
  `crates/motion-voice/src/scene_audio.rs`; a physical trim fixture must use the
  actual nonzero trim path. Coordinate any persisted trim UI/schema with timeline
  and speech owners before editing shared types.
- Accessibility: `apps/studio-desktop/Cargo.toml` currently disables eframe defaults
  and does not enable `accesskit`; audit backend support before promising VoiceOver.
  Add the smallest verified integration and labels/focus behavior in the relevant
  desktop panels; keep document shortcut focus rules in `main.rs`.

No production changes are authorized by this document. The device owner retains
observations and proposes minimal reproductions; release/license, GPU parity and
speech-quality owners retain their independent approval gates. Future Rust fixes
run the applicable AGENTS.md checks, plus targeted regression tests; those checks
cannot substitute for repeating the affected physical procedure.
