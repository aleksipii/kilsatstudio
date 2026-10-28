# Kilsat Studio

Kilsat Studio is a macOS-first, screenplay-driven 2D animation application built
in Rust. Its StoryMotion Engine evaluates YAML MotionScript with Finnish or
English dialogue. Local speech uses configured Finnish Piper, English Kokoro-82M
or explicitly imported WAV recordings.

The desktop includes editable scenes, original custom rigs, measured-audio
direction, continuous head-tilt recording and bounded Full HD MP4 export.
Physical-device, artistic, speech-quality and release acceptance are tracked in
[master-progress](docs/master-progress.md). All Kilsat Studio source is independently authored; no Storytold
source code or dependencies are used.

## License

Original Kilsat Studio source and documentation are licensed under the
[MIT License](LICENSE). Third-party libraries, model weights, voices, language
data and external tools retain their own licenses. This source license does
not complete their redistribution review; see the
[dependency audit](docs/dependency-audit.md).

## Current capabilities

- Follow the twelve-topic [Finnish desktop guide](apps/studio-desktop/KAYTTOOHJE.md)
  for new scenes/episodes, mouse editing, speech, camera recording and export.
- Direct actual Finnish speech or imported WAVs into editable acting, reaction
  and camera shots through a cancellable background worker and one-step Undo.
- Create a local Piper profile from installed files; prepare Finnish speech and
  explicitly labelled estimated mouth timing. See [Finnish speech](docs/finnish-speech.md).
- Review continuous local camera head-tilt recordings before applying keys.
- Stream up to 1920x1080/24fps/60s to MP4 and measure the whole speech-to-video
  pipeline. See [Full HD](docs/fullhd-performance.md) and [integration evidence](docs/production-integration.md).
- Open the [31 original Finnish episode projects](examples/kilsat-season/KAUSI.md)
  or use the [checked season/compilation runner](docs/kilsat-season.md).
- Open the [three English Aino/Rane episodes](docs/kilsat-english.md), with English
  Kokoro voice bindings, captions and an explicit Full HD rendering workflow.

- Reference authored clips as ordered live bone layers with activation windows, fades and optional local weight curves; see [live clip blending](docs/live-clip-blending.md).

- Capture, inspect and reuse project-local authored bone clips with explicit replacement and Undo/Redo; see the [clip library guide](docs/animation-clip-library.md).

- Edit, open and save MotionScript; validate actor references, parameters and timing.
- Add, duplicate, edit and remove events in the Inspector; drag start times on the
  project frame grid, and undo/redo screenplay changes.
- Draw pen strokes, rectangles and ellipses on persisted foreground layers with rectangular clips.
- Display imported PNG/JPEG layers in preview and SVG/PNG/MP4 export.
- Edit persistent actor transform keyframes (offset, rotation, scale and easing).
- Edit validated camera pan/zoom keys, actor layer order/visibility, background colors and scene-edge fades.
- Import, relink and remove explicit asset references on a bounded background worker.
- Render frames with bounded fixed workers and deterministic ordered output.
- Navigate a resizable studio workspace with a project/asset browser, YAML editor,
  central preview, editable event Inspector and scrubbable actor/event timeline.
- Preview Emma/Alex placeholders with deterministic movement, expressions and wave
  gestures. Play, pause, restart and scrub a silent scene.
- Configure per-character local speech presets, language and speed in the Inspector, with validation and Undo/Redo.
- Prepare selected dialogue or a complete scene on bounded background workers,
  using local synthesis or explicitly imported WAV audio.
- Reuse verified Kokoro speech by content/model/runtime identity; detect corrupt
  audio and time out sidecars. Piper and system voices run fresh because their
  complete dependency closure is unverified; generated WAVs are content-addressed.
- Mix dialogue at sample-frame positions, preserve silence and overlapping actors,
  and reject overlapping speech from the same actor or scene overruns.
- Preview waveform/duration and timed mouth shapes. Full-scene macOS playback uses
  one mixed source clock for the waveform, scene movement and active actor mouths.
- Import explicit timed IPA/ARPAbet alignment with provenance and audio checks.
- Export mixed scene WAV and sample-timed dialogue/viseme JSON; sample actor poses
  through the same scene evaluator used by the preview.
- Save versioned `.kilsat` projects with exact screenplay source and portable asset metadata.
- Add, duplicate and delete scenes; add or delete unreferenced characters with source Undo/Redo.
- Evaluate original hierarchical rigs, procedural walking/waving and reusable keyframe curves.
- Export PNG sequences or H.264/AAC MP4 via an explicitly selected local FFmpeg.
- Render deterministic procedural SVG frame sequences, including a scene-bundle
  path with matching WAV and explicit timed mouth tracks on a shared sample clock.

Manual, forced and Rhubarb tracks take precedence over explicitly labelled text
estimates. An estimate is not measured acoustic alignment. Kokoro phoneme strings
are **not** treated as accurate timestamps. Facial emotion affects visual acting;
acoustic emotion conditioning is not implemented. Custom rigs support textures,
two-joint skinning, outlined/rounded parts, expressions and viseme mouths.

## Practical macOS test build

The **Tikku-ukkostudio** button opens an editable, silent 30-second starter with
six original articulated stick figures, four sets and 20 props. Connected round
limbs preserve the stick-figure style in bent poses. **Esine käteen** attaches
nine hand-prop choices, including front, back and oblique phone views, to a
character across all scenes. Framing buttons write native 2D camera keys for
wide, two-person, medium, close-up and held-prop shots. The
[asset pack guide](examples/stick-studio/README.md) describes the cast, native
projects and 60 PNG/SVG exports. The [art direction plan](docs/stick-studio-art-direction.md)
sets the joint, grip, background and framing review criteria. The
[professional studio roadmap](docs/professional-studio-next.md)
prioritizes an asset browser/recovery, visual animation/contact tools and
integrated sound editing with measured speech quality.

**Kilsat Studio 1.0** (Apple Silicon, macOS 13+) is built by
`scripts/build_release_app.sh` and the
[release app workflow](https://github.com/aleksipii/kilsatstudio/actions/workflows/macos-release-app.yml);
see [Kilsat Studio 1.0](docs/release-1.0.md). The package is ad-hoc signed;
Developer ID signing and notarization are a separate explicit step.

Preview builds for Apple Silicon, macOS 13+, are available from the
[macOS practical preview workflow](https://github.com/aleksipii/kilsatstudio/actions/workflows/macos-preview.yml).
See the [practical test guide](docs/practical-testing.md) for download and launch
instructions. The app includes an **Ääniesimerkki (synthetic tones)**
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

## Kokoro inside the app

The **Kokoro — setup and test** panel can install an isolated local environment
and hash-verified model/English voice assets after an explicit download choice,
connect an existing managed profile, and run/play a CPU speech test without a
terminal. A compatible locally installed Python3.10–3.12 is still required;
known Mac paths are detected on a worker. Models/environments are not bundled.
See [managed setup](docs/managed-kokoro.md) and [acceptance](docs/kokoro-in-app-acceptance.md).

## Advanced manual Kokoro setup

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

- Broader offline/device acceptance beyond the user-reported successful M1 Kokoro CPU tests for `af_heart` and `am_michael`; see [recorded evidence](docs/kokoro-in-app-acceptance.md#recorded-mac-evidence).
- GUI/audio interaction beyond confirmed launch, hardware latency and perceptual lip-sync quality.
- Practical macOS cancellation validation and robust runtime device-loss recovery.
- Textured character rigs, full-scene GPU rendering/parity, physical Metal evidence, parallel rendering and production release packaging.
- Real-Mac validation of the new local MP4 path and media synchronization with actual speech.
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

- [Master plan implementation checklist](docs/master-progress.md)
- [Artwork and project v2](docs/art-format.md)
- [Artwork rendering](docs/art-rendering.md)
- [Actor keyframes](docs/actor-keyframes.md)
- [Keyframe editor](docs/keyframe-editor.md)
- [Artwork acceptance](docs/art-acceptance.md)
- [Selected-scene CPU compositor](docs/native-compositor.md)
- [Native estimated mouth timing](docs/native-alignment.md)
- [Packaged Metal device check](docs/gpu-device-check.md)
- [Retained license provenance](docs/license-provenance.md)
- [Composition and camera](docs/composition.md)
- [Asset inventory management](docs/asset-management.md)
- [Bounded jobs and benchmark](docs/jobs-performance.md)
- [Alignment cache](docs/alignment-cache.md)
- [Open-feature acceptance review](docs/open-feature-review.md)
- [PNG/MP4 export](docs/media-export.md)
- [Project format](docs/project-format.md)
- [Rig and animation primitives](docs/character-animation.md)
- [Speech evidence](docs/speech-acceptance.md)
- [macOS distribution](docs/macos-distribution.md)


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


### Project and scene settings

In the Project tab, expand **Project / scene settings** to edit the project title,
frame rate and dimensions, or the selected scene ID and duration. **Apply settings**
validates the whole screenplay and records one Undo action; **Reset settings draft**
discards pending settings. Event times remain in seconds and coordinates are not
rescaled. Dimensions apply to every scene. Shortening rejects events or known
prepared dialogue extending past the new end; speech not yet generated is checked
on preparation. Accepted changes invalidate prepared audio and pending editor
drafts while preserving pasted timing maps. Undo restores exact source text
and clears timing maps under the existing history policy. YAML is normalized and
comments removed on Apply. Script dimensions permit 16384 per side, while SVG
export supports 8192; audio export supports one hour.


### Current development increment

PR #30 is merged and synchronized to `kilsatstudio1.0`. The next unmerged
increment adds selected-scene shared CPU frames for advanced masks/blending,
explicit prepared-line Rhubarb estimation with separate Apply, a packaged Metal
fixture check and retained managed-install license evidence. See the guides above
and [master progress](docs/master-progress.md) for actual validation. These tools
do not establish real recognizer quality, physical Mac device/GPU acceptance,
full-scene GPU parity or distribution clearance.

Suomenkielinen työpöytäsovelluksen käyttöohje: [Mistä tehdään mitäkin](apps/studio-desktop/KAYTTOOHJE.md). Sama opas löytyy sovelluksen yläpalkin Ohjeet-painikkeesta.
