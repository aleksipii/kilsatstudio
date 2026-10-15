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
   identify it as an unnotarized development app; only open a build from a commit
   you trust, using the normal macOS Privacy & Security/Finder controls.
4. Read the included `Contents/Resources/BUILD-INFO.txt` for the exact source commit and architecture.
   The ZIP includes test instructions, examples and the Python sidecar source,
   but no inference dependencies or model assets.

The build job checks bundle layout, architecture, dynamic-library references and
ad-hoc signature. It does not launch the GUI or listen to the output device.

## Test 1: screenplay and movement, no model

The default Hello Emma screenplay includes movement, expressions and a wave.
Click **Validate screenplay**, then **Play scene (silent)**. Try Pause, Restart and
the scene time slider. Change a movement coordinate in YAML, revalidate and verify
that the procedural actor responds. Try an invalid actor name and verify that an
actionable validation error appears instead of a stale playable scene.

The characters are original procedural placeholders. Textured puppets, production
frame rendering and video export are still unfinished.

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

## Test 3: real Kokoro CPU inference (separate setup)

Use the [runtime guide](../services/kokoro-runtime/README.md) to prepare a compatible
Python environment and approved model/voice/G2P assets outside the repository.
Review their exact licenses before downloading or distributing assets. Configure
absolute local paths and a writable cache directory in the desktop settings.
The bundled sidecar source is available in the app's Resources; Python and the
model remain external. Run `smoke_macos.py --help` from the source checkout for an
explicit local CPU test before testing full dialogue in the app.

Generate the Hello Emma scene with `af_heart` and `am_michael`. Record actual WAV
output, durations, voice quality and failures. Real speech has no validated mouth
timestamps automatically; import or author timing separately. Do not judge
phoneme alignment from synthetic tones or Kokoro token strings.

## Report the result

Include the source commit from BUILD-INFO, macOS version, Mac model/chip, output
device, whether the app opened, steps taken, expected versus observed behavior
and any visible error. For Kokoro tests, include Python/runtime/model revisions
and whether all assets were already local. Do not include model weights in reports.

| Check | What to record |
| --- | --- |
| Launch | App opens, editor and preview visible, no crash |
| Editing | Valid edits update actors; invalid YAML/references show clear errors |
| Synthetic audio | Tone heard, no model/Python/download required |
| Seeking/pause | Cursor, sound and mouth cues remain consistent |
| Real speech | Actual CPU WAV generation and voice behavior on your Mac |
| Real alignment | Annotated boundaries and perceptual timing, separately measured |

Remaining development includes cancellation, runtime output-device recovery,
measured speech alignment, textured rigs, rendered frames/video, release signing,
notarization, and the complete redistribution/dependency audit.

### Cancel speech generation

While dialogue or scene preparation is running, choose **Cancel generation**. The interface stays responsive and waits for the worker to stop before allowing a retry. Kokoro terminates and reaps its active sidecar and removes partial staging files; completed cache entries remain available. Imported audio and mixing stop at processing boundaries. Custom speech providers using the default cancellation implementation stop before or after their synthesis call.

To check cancellation, start a long uncached dialogue, cancel it, wait for the cancellation message, and generate again. No cancelled result should replace the preview. This interaction still requires a practical macOS GUI test.
