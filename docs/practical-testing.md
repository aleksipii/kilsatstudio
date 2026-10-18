# Practical macOS preview test

This preview targets Apple Silicon (arm64), macOS 13 or newer. It is a development
build with an ad-hoc signature, not an Apple Developer ID signed/notarized release.
Its purpose is to collect real GUI/audio evidence that compilation alone cannot
provide. No Kokoro model, voice pack, Python environment or paid API is required
for the model-free checks below.

## Download and launch

1. Open the successful **macOS practical preview** workflow run linked from the test-build PR.
2. Download its `Kilsat-Studio-Preview-arm64` artifact while signed into
   GitHub. Extract the artifact, then extract the contained preview ZIP.
3. Open `Kilsat Studio Preview.app`. Keep its Contents directory together. macOS may
   identify it as an unnotarized development app. For the specific warning that
   Apple cannot verify the developer, and only for a build you trust, dismiss the
   warning and open **System Settings → Privacy & Security → Open Anyway** for
   this app, then authenticate and confirm Open. This does not disable Gatekeeper
   globally. A damaged-app or malware warning requires separate investigation.
4. Read the included `Contents/Resources/BUILD-INFO.txt` for the exact source commit and architecture.
   The ZIP includes test instructions, examples and the Python sidecar source,
   but no inference dependencies or model assets.

A user confirmed the preview opens after normal macOS approval. On 2026-10-16,
they also reported successful in-app Kokoro setup and English CPU speech tests
for `af_heart` and `am_michael` on an Apple M1, macOS 27.1, Homebrew Python 3.12,
using build `98d265b`. Preserve that evidence; the new sequence/device/offline
checks below have not thereby passed. See the
[recorded Mac evidence](kokoro-in-app-acceptance.md#recorded-mac-evidence).

The build job checks bundle layout, architecture, dynamic-library references and
ad-hoc signature. It does not launch the GUI or listen to the output device.

## Test 1: screenplay and movement, no model

The default Hello Emma screenplay includes movement, expressions and a wave.
Click **Validate screenplay**, then **Play scene (silent)**. Try Pause, Restart and
the scene time slider. Change a movement coordinate in YAML, revalidate and verify
that the procedural actor responds. Try an invalid actor name and verify that an
actionable validation error appears instead of a stale playable scene.

The default characters use original procedural rigs. Local artwork and bounded
PNG/MP4 export are implemented; production GPU rendering and comprehensive
physical-device acceptance remain unfinished.

## Test 2: full scene sound and mouth cues, no model

1. Click **Load sound-check demo (synthetic tones)**.
2. The app creates short owned test WAVs in a temporary directory and loads a
   small screenplay with imported audio plus illustrative manual mouth cues.
3. Wait for automatic scene preparation to finish. It should complete without
   Python, Kokoro weights or a network download. Use **Prepare full scene audio**
   again after editing the scene.
4. Click **Play scene audio**. Expect synthetic tones from two characters, with
   movement and illustrative mouth cues following the scene cursor.
5. Try Pause/Resume, Stop, seeking before/during/after a tone and playing again.
   Watch the waveform cursor and character mouths at the boundaries.

These tones are **not speech and not Kokoro output**. Their manual mouth cues test
plumbing, not phoneme alignment quality. Temporary demo assets are removed when
their owner is dropped; use a separate path if saving a persistent project.

## Test 3: Kokoro setup and CPU test inside the app

1. Open **Kokoro — setup and test**. Provide a compatible local Python3.10–3.12
   only if autodetection cannot find one. The installer does not install system Python.
2. Choose a new managed install folder, review source/download information and
   explicitly enable the download choice. Press Install; the model is about328MB
   plus voices, language data and Python packages. Keep the app open and follow
   progress or Cancel. Existing directories are preserved rather than overwritten.
3. Successful setup connects a profile and fills advanced paths automatically.
   It proves prerequisites, not speech inference. You can also Connect existing
   to a previous managed installation.
4. Select af_heart or am_michael and run the local CPU speech test. The new WAV
   and JSON evidence paths are shown; use Play test audio. The test preserves your
   screenplay. Listen, record errors and repeat for both voices.
5. Once connected, use normal screenplay dialogue preparation/playback. Accurate
   lip timing still requires separate alignment.

The user has reported successful Mac installation and both voice tests as recorded
above. Complete offline repeatability and scene/device acceptance separately. See
[managed setup](managed-kokoro.md) and [acceptance](kokoro-in-app-acceptance.md).

## Advanced manual Kokoro setup and CPU evidence

Use the [runtime guide](../services/kokoro-runtime/README.md) to prepare a compatible
Python environment and approved model/voice/G2P assets outside the repository.
Review their exact licenses before downloading or distributing assets. Configure
absolute local paths and a writable cache directory in the desktop settings.
The bundled sidecar source is available in the app's Resources; Python and the
model remain external. Run `smoke_macos.py --help` from the source checkout for an
explicit local CPU test before testing full dialogue in the app.

1. In the app, choose **Check local Kokoro setup**. Confirm the editor remains
   responsive during asset hashing. Resolve missing-file/dependency errors and
   retry. This prerequisite check does not synthesize speech or certify model/G2P
   validity. For shareable JSON use `doctor.py` as described in the runtime guide.
2. Run the opt-in smoke test with `af_heart.pt`, a new WAV path, and
   `--report /absolute/reports/kokoro-af-heart.json`. Repeat with `am_michael.pt`
   and separate WAV/report paths. Keep both WAVs locally and listen: record whether
   each line is understandable, complete and free of obvious clipping/artifacts.
   RMS/peak and nonzero samples prove signal presence, not natural speech quality.
3. Generate the Hello Emma scene with `af_heart` and `am_michael`. Play it and
   exercise pause/resume and seeking. Record durations, voice quality and errors.
4. Prepare unchanged dialogue again. Observe whether it reuses a cache entry
   without a new synthesis process or WAV modification. Change text, preset or
   speed and verify that it generates distinct output; preserve the model revision.
5. Start a long uncached line and choose **Cancel generation**. Wait for completion
   of cancellation, then retry. A cancelled result must not replace the preview.
   Completed cache entries should remain reusable. Cancellation keeps the UI busy
   until workers stop safely: Kokoro kills/reaps its child and removes staging;
   imported audio/mixing stop at processing boundaries. Default custom providers
   may only stop before/after their synthesis call.

Real speech has no automatically validated mouth timestamps. Import or author
alignment separately; do not judge phoneme timing from tones or Kokoro strings.

## Test 4: whole-project sequence, new Mac acceptance

All boxes here start **pending**. Record `pass`, `fail` or `not_tested` per check;
passing Rust tests or the earlier M1 voice test cannot close these observations.
Use the current build's commit from BUILD-INFO, not the earlier build identifier.
Open **Whole project · sequence** in the native scene workspace. Its transport is
separate from single-scene playback.

### Model-free transition and transport

1. Open `examples/scene-sequence.yaml` and validate. It contains two one-second
   scenes, red then blue, with a half-second crossfade. Click **Prepare whole
   project**. No download or speech synthesis is needed for this empty-dialogue
   fixture; expect a 1.500-second prepared sequence.
2. Use **Play**, **Pause**, **Restart** and the **global sample / 24 kHz** slider.
   At sample 12000 (0.500 s), expect red; at 18000 (0.750 s), expect approximately
   equal red/blue; at 24000 (1.000 s), expect blue. This is a blend in encoded
   color space, not a linear-light reference. Scrub backward and forward while
   paused and playing; the picture must settle on the requested position without
   an older worker result replacing it.
3. Let playback reach 1.500 s. It must stop without looping or hanging; the last
   picture remains visible. Press **Play** at the end to restart from zero. Test
   **Restart** while playing and paused. Check that single-scene playback does
   not continue sounding underneath project playback.
4. Change a scene setting and apply it, or edit and validate the source. The old
   prepared project must stop and require preparation again. Restore the fixture
   before continuing. Inspect missing-device errors: **Silent visual transport**
   means the fallback picture clock ran, not that device audio passed.

### Real dialogue, physical output and offline repeatability

Create a separate persistent two-scene project using already installed voices or
owned imported WAV files. Put distinguishable dialogue near the end of scene A
and the beginning of B so both are audible during the crossfade; each line must
still fit its own scene. Add explicit imported/manual alignment if assessing
mouth cues. Keep this fixture and its assets outside temporary sound-check demo
directories. Use [sequence timing](scene-sequences.md) for transition limits.

- [ ] Prepare both scenes, listen on the named physical output device, and record
  whether the overlap fades continuously with no gap, repeated onset or obvious
  clipping. Crossfade weights are shared with video; that alone does not prove
  perceptual synchronization or acceptable loudness.
- [ ] Pause/resume inside the overlap; seek before, inside and after it in both
  directions. Check audio, global cursor, both scenes' movement/mouth cues and
  picture against each other. Repeat at the final speech boundary and EOF.
- [ ] Test missing/unavailable output and recovery using the actual device setup.
  Record the error and whether replay can recover. Do not count silent fallback
  as successful audio playback.
- [ ] With assets already provisioned, disconnect networking and prepare changed,
  uncached text using the configured local Kokoro runtime. Confirm real new WAV
  generation, not just cached replay; preserve smoke/diagnostic evidence. Then
  prepare unchanged text to check reuse. Neither **Prepare whole project** nor
  playback should start the installer or download assets. Do not enable setup
  downloads for this offline test.
- [ ] Cancel a sufficiently long uncached **Prepare whole project** with **Cancel
  sequence task**. Wait for worker completion; verify no cancelled/stale project
  becomes playable and retry works. Keep notes on responsiveness and errors.

### Export, cancellation and comparison

1. Prepare the dialogue project and supply a **new absolute output directory**.
   Select **PNG + WAV** and then **Export whole
   project**. Verify numbered PNGs, `audio.wav` and `sequence.json`; compare
   crossfade start/middle/end pictures and audio to the native preview.
2. Repeat into another new directory with **MP4 + WAV** selected and an explicit
   absolute path to an already installed local FFmpeg executable. Use even
   project dimensions for H.264. The app must not download or locate an encoder
   through PATH. An invalid encoder path or existing destination must produce an
   actionable failure and preserve existing files.
3. Also export **SVG + WAV** into a new directory without configuring FFmpeg.
   Check the `.svg` frame extensions and the sample/contribution manifest. Open
   crossfade start/middle/end frames in the chosen SVG viewer and compare camera,
   fades, image layers and masks with PNG output. Report the viewer and visual
   differences; automated raster fixtures are not browser/device acceptance.
4. Play `sequence.mp4` in a Mac video player through the same output device.
   Compare speech onset, overlap, mouth cues and final line with the native
   preview. Check frame count and duration against `sequence.json`; record
   perceptual drift separately, especially for a longer fixture within export
   limits. A structural duration match does not prove lip-sync quality.
5. Cancel an export long enough to observe progress. Wait for completion and
   check that its newly created partial directory is removed, earlier successful
   exports remain unchanged and another export succeeds. Repeat cancellation
   during MP4 encoding when feasible; report `not_tested` if it finishes too fast.

No new Mac/device/offline/video acceptance is claimed by this checklist. Retain
the fixture, exact build, transition samples, device, observed errors and reviewed
reports so another person can reproduce each result.

## Report the result

Include the source commit from BUILD-INFO, macOS version, Mac model/chip, output
device, whether the app opened, steps taken, expected versus observed behavior
and any visible error. For Kokoro tests, include Python/runtime/model revisions
and whether all assets were already local. Attach reviewed JSON diagnostics/smoke
reports, listening notes and errors; reports contain paths/environment details.
Do not upload model weights or voice tensors. A successful report does not certify
perceptual alignment, audio-device latency or video export.

| Check | What to record |
| --- | --- |
| Launch | App opens, editor and preview visible, no crash |
| Editing | Valid edits update actors; invalid YAML/references show clear errors |
| Synthetic audio | Tone heard, no model/Python/download required |
| Seeking/pause | Cursor, sound and mouth cues remain consistent |
| Real speech | Actual CPU WAV generation and voice behavior on your Mac |
| Real alignment | Annotated boundaries and perceptual timing, separately measured |

Remaining acceptance includes practical cancellation, runtime output-device
recovery, offline synthesis and measured/perceptual speech alignment. Production
GPU rendering, release signing/notarization and the complete
redistribution/dependency audit remain open. PNG/MP4 implementation is distinct
from the new Mac preview/export observations requested above.



## Next development increment: compositor, estimated cues and Metal evidence

These controls belong to the next unmerged increment; select its exact successful
CI artifact and retain BUILD-INFO.txt before recording results. All physical checks
below remain pending; the earlier successful M1 Kokoro speech report is preserved.

1. Add an ellipse/polygon mask or Multiply/Screen artwork layer. At an active
   frame, wait for the selected-scene shared CPU image, scrub and seek while it
   renders, and verify stale frames never replace the new scene/source. Compare
   equal timestamps with PNG export and Whole project preview. Test speech-mouth
   boundaries as well as camera/opacity/fade edges. This is CPU raster composition,
   not evidence of a production GPU renderer. See [compositor](native-compositor.md).
2. With a separately installed licensed Rhubarb executable, prepare one dialogue
   line first. Enter the absolute executable path, select a recognizer and click
   **Estimate prepared line**. Inspect the estimated result, then explicitly
   **Apply estimated timing**. Change line/text or cancel during execution and
   confirm stale results cannot apply. Record real audio/video quality separately;
   estimated shapes are not forced-aligned phonemes. No tool/model downloads are
   performed by the panel. See [native timing](native-alignment.md).
3. Expand **Metal device check**, choose a new absolute evidence directory with
   an existing parent, and explicitly run the packaged helper. Preserve its
   report, RGBA output and hashes with Mac/chip/macOS/build identity. Software or
   unknown adapters must fail; cancel/timeout must not claim success. A fixture
   pass establishes only that fixture's device gate, not complete scene parity.
   See [device evidence](gpu-device-check.md).
4. After a newly consented managed install, use the explicit profile with the
   offline inventory command in [license provenance](license-provenance.md).
   Verify evidence hashes, retain budget-omission markers and review texts before
   sharing local details. Older profiles do not manufacture missing evidence.
   `--strict` remains failing while review gaps remain; collection is not clearance.

Current development checks cover workspace/all-target compilation and 42 runtime,
14 inventory and 10 packaging Python tests. Full Rust/CI validation was pending
when these instructions were updated; no physical checks above are claimed.
