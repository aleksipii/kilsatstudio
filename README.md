# Kilsat Studio

Kilsat Studio is a macOS-first, screenplay-driven 2D animation application built
in Rust. Its StoryMotion Engine evaluates English YAML MotionScript, while an
offline voice pipeline integrates the approved Hugging Face Kokoro-82M model.

The current application provides an original procedural character preview and a
dialogue workbench. It is an actively developed foundation, not a production-ready
animation suite. All Kilsat Studio source is independently authored; no Storytold
source code or dependencies are used.

## Current capabilities

- Edit, open and save MotionScript; validate actor references, parameters and timing.
- Add, duplicate, edit and remove events in the Inspector; drag start times on the
  project frame grid, and undo/redo screenplay changes.
- Navigate a resizable studio workspace with a project/asset browser, YAML editor,
  central preview, editable event Inspector and scrubbable actor/event timeline.
- Preview Emma/Alex placeholders with deterministic movement, expressions and wave
  gestures. Play, pause, restart and scrub a silent scene.
- Configure per-character Kokoro presets, English language and speech speed in the Inspector, with validation and Undo/Redo.
- Prepare selected dialogue or a complete scene on bounded background workers,
  using local synthesis or explicitly imported WAV audio.
- Cache generated speech by content and model/runtime identity; detect corrupted
  cached audio, time out sidecars and report recoverable errors.
- Mix dialogue at sample-frame positions, preserve silence and overlapping actors,
  and reject overlapping speech from the same actor or scene overruns.
- Preview waveform/duration and timed mouth shapes. Full-scene macOS playback uses
  one mixed source clock for the waveform, scene movement and active actor mouths.
- Import explicit timed IPA/ARPAbet alignment with provenance and audio checks.
- Export mixed scene WAV and sample-timed dialogue/viseme JSON; sample actor poses
  through the same scene evaluator used by the preview.
- Render deterministic procedural SVG frame sequences, including a scene-bundle
  path with matching WAV and explicit timed mouth tracks on a shared sample clock.

Missing alignment keeps mouths at rest. Kokoro phoneme strings are **not** treated
as accurate timestamps. Facial emotion affects the visual preview; acoustic emotion
conditioning is not implemented. The current artwork and backdrop are procedural
placeholders, not loaded texture rigs.

## Practical macOS test build

Preview builds for Apple Silicon, macOS 13+, are available from the
[macOS practical preview workflow](https://github.com/aleksipii/kilsatstudio/actions/workflows/macos-preview.yml).
See the [practical test guide](docs/practical-testing.md) for download and launch
instructions. The app includes a **Load sound-check demo (synthetic tones)**
button to test scene movement, mixed audio and illustrative mouth cues without
Python, Kokoro assets or a network download. These tones are not speech or Kokoro
output. Preview builds are ad-hoc signed and not notarized. A user has confirmed
that the preview opens on their Mac; chip, macOS version, audio playback and
Kokoro inference have not yet been recorded. The guide describes the scoped
macOS **Open Anyway** launch procedure.

## Quick start

Install stable Rust and the macOS developer command-line tools, then:

```sh
git clone https://github.com/aleksipii/kilsatstudio.git
cd kilsatstudio
cargo run -p studio-desktop
```

The app starts with [Hello Emma](examples/hello-emma.yaml), including movement,
expressions, a wave and Kokoro voice settings for Emma and Alex. Validate it and
use the silent scene controls to explore animation without installing speech
assets. The [desktop guide](apps/studio-desktop/README.md) describes runtime paths,
imported audio, single-line and full-scene preparation, alignment and playback.

Audio-device output currently targets macOS. Linux supports the editor, visual
scrubbing, tests and headless tools, with an explicit unsupported-device message.
The application never automatically downloads models or requires a paid cloud API.

## Local Kokoro setup

Use a separate compatible Python 3.10–3.12 environment. Follow the
[Kokoro CPU runtime guide](services/kokoro-runtime/README.md) to install the
inference and English G2P dependencies explicitly. Review the exact licenses and
obtain appropriate consent before downloading model, voice or language assets.
Keep model files outside this repository:

```text
/absolute/kokoro-assets/config.json
/absolute/kokoro-assets/kokoro-v1_0.pth
/absolute/kokoro-assets/voices/af_heart.pt
/absolute/kokoro-assets/voices/am_michael.pt
```

In the desktop settings, provide the Python executable, sidecar script, model
config/weights, voice directory, cache directory and immutable model/dependency
revision. Use **Prepare full scene audio** for the selected scene, then the mixed
scene playback controls. Imported `speak.audio` paths resolve from the configured
project root; imported audio must be nonempty 24 kHz mono PCM16 WAV.

Choose **Check local Kokoro setup** to inspect configured assets and dependency
metadata on a background worker. This hashes local files without loading a model,
generating speech or downloading anything. The equivalent `doctor.py` command
emits JSON prerequisite evidence; see the runtime guide.

For explicit local-only macOS inference verification, use
`services/kokoro-runtime/smoke_macos.py --help`. The runner requires preinstalled
assets and does not download them. Add `--report /absolute/reports/kokoro.json`
to save evidence. Model weights, voice assets and a fully audited
runtime distribution are not included.

## Headless workflows

Generate/import all dialogue, mix each scene and write `scene-N.wav` plus
`scene-N.json`:

```sh
cargo run -p motion-voice --example render_dialogue -- \
  examples/hello-emma.yaml /absolute/kokoro-assets /absolute/venv/bin/python \
  services/kokoro-runtime/runtime.py /absolute/dialogue-output \
  'UPSTREAM_REVISION:ENVIRONMENT_LOCK_DIGEST'
```

The desktop and this command share scene preparation and sample-clock mixing.
Explicit imported audio bypasses synthesis. Missing dependencies, invalid timing
or unavailable model assets produce errors; they do not trigger cloud inference.

Sample deterministic actor poses at the project's frame rate:

```sh
cargo run -p motion-scene --example evaluate_scene -- \
  examples/hello-emma.yaml 0 /absolute/new-poses.json
```

Pose JSON and audio/viseme manifests are interchange data. Pose export refuses an
existing output path and caps frame/actor work. For rendered SVG frames, see the
[frame export guide](docs/frame-export.md). SVG sequences are individual vector
images; they are **not encoded video** and contain no embedded audio.

## Components

| Component | Responsibility |
| --- | --- |
| `motion-script` | MotionScript schema, syntax/semantic checks and unsupported-command diagnostics |
| `motion-scene` | Stateless movement/expression/gesture evaluation shared by preview and pose export |
| `motion-render` | Deterministic procedural SVG rendering and bounded movement-only frame example |
| `motion-export` | Bounded, cancellable SVG/audio scene bundles with sample-timed mouth evaluation |
| `motion-audio` | WAV I/O, waveform peaks, frame transport and deterministic mixing |
| `motion-tts` | Replaceable speech provider, Kokoro process boundary, cache and timeout handling |
| `motion-lipsync` | Explicit alignment provenance, IPA/ARPAbet conversion and timed visemes |
| `motion-voice` | Dialogue jobs, background orchestration, complete-scene preparation and audio/metadata export |
| `services/kokoro-runtime` | Local CPU inference sidecar, protocol tests and opt-in macOS smoke runner |
| `studio-desktop` | Native egui/eframe editor, procedural preview, waveform and macOS playback controls |

## Validation and remaining work

Validation results are recorded per commit in GitHub Actions. Automated checks
cover protocol boundaries, cache integrity, cancellation, mixing and packaging;
stub inference tests do not establish real speech quality.

GitHub Actions runs formatting, compilation, strict Clippy, Rust tests and Python
tests on Linux and macOS. CI compilation/test harnesses do not establish that a
GUI was launched, a physical output device was heard or Kokoro generated real
speech. See the repository's [Actions](https://github.com/aleksipii/kilsatstudio/actions)
for the result associated with each exact commit.

Still unverified or unfinished:

- Real Kokoro CPU synthesis on macOS/Apple Silicon with the approved local assets.
- GUI/audio interaction beyond confirmed launch, hardware latency and perceptual lip-sync quality.
- Practical macOS cancellation validation and robust runtime device-loss recovery.
- Textured character rigs, production rendering, encoded video/audio muxing and release packaging.
- Complete model/voice/dependency redistribution audit and locked Python environment.

No native Rust TTS, Metal, MPS or Core ML acceleration is claimed. Rodio startup
errors reach the UI; runtime device faults currently go to stderr and may require
an application restart.

Run checks from the workspace root. Use Python 3.12 for the pinned NumPy test
dependency below, matching CI:

```sh
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 -m pip install numpy==2.3.5
python3 -m unittest discover -s services/kokoro-runtime/tests -v
python3 -m unittest discover -s tests -p 'test_package_macos.py' -v
```

The Python synthesis-path tests use NumPy and stub inference modules. They do not
load a real Kokoro model. In restricted environments lacking `/proc/self/exe`,
Cargo formatter/Clippy wrappers may need direct-tool equivalents; standard checks
remain enabled in CI.

## Project guides

- [Supported MotionScript subset](docs/motionscript.md)
- [Audio architecture and acceptance gates](docs/audio.md)
- [Staged development](docs/development-stages.md)
- [SVG frame export](docs/frame-export.md)
- [Studio workspace](docs/ui-workspace.md)
- [Complete master build prompt with Kokoro](docs/master-build-prompt.md)
- [Agent engineering instructions](AGENTS.md)

Approved stages are integrated into `main` and synchronized to `kilsatstudio1.0`
while preserving branch history. Keep subsequent changes reviewable and distinguish
implemented behavior from platform acceptance that still needs real evidence.

### Cancel speech generation

While dialogue or scene preparation is running, choose **Cancel generation**. The interface stays responsive and waits for the worker to stop before allowing a retry. Kokoro terminates and reaps its active sidecar and removes partial staging files; completed cache entries remain available. Imported audio and mixing stop at processing boundaries. Custom speech providers using the default cancellation implementation stop before or after their synthesis call.

See the [practical test guide](docs/practical-testing.md) for cancellation and retry.
