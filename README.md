# Kilsat Studio — Agent 6 audio foundation

Independently authored offline voice infrastructure for the repository's Agent 6
workstream. This includes a native dialogue workbench and headless audio pipeline. It is
not yet a complete animation workstation. The combined scene foundation adds
original procedural Emma/Alex previews with deterministic movement, expressions
and a wave gesture. No Storytold source or dependencies are used.

## Components

- `motion-script`: strict dialogue-only YAML schema, character voice settings,
  scene/actor/time validation and imported WAV references.
- `motion-tts`: replaceable `SpeechProvider`, local Kokoro Python provider,
  content-addressed cache, corrupt-cache regeneration and subprocess timeout.
- `motion-audio`: WAV I/O, waveform peak data and deterministic sample-frame mixing.
- `motion-lipsync`: timed IPA alignment validation, mouth shapes and seek evaluation.
- `motion-voice`: bounded background worker, dialogue jobs and shared audio/mouth
  timeline placement, manual offset/trim/gain through prepared clips.
- `services/kokoro-runtime`: local CPU inference service with explicit asset paths and
  an opt-in macOS smoke runner.
- `studio-desktop`: native dialogue workbench with screenplay Open/Save, validation,
  background generation/import, waveform scrubbing and symbolic mouth-state preview.

See [runtime setup](services/kokoro-runtime/README.md) and
[architecture and verification](docs/audio.md).

See the [staged development gates](docs/development-stages.md) for delivery order
and platform acceptance requirements.

The complete [master build prompt with Kokoro](docs/master-build-prompt.md) and
[current MotionScript subset](docs/motionscript.md) describe the target and actual
supported behavior separately.

## Desktop workbench

Run `cargo run -p studio-desktop` from the repository root. See
[workbench instructions](apps/studio-desktop/README.md). The workbench previews
waveforms and timed mouth states. macOS audio-device playback is implemented
behind a bounded worker, but real hardware playback remains unverified. Procedural character preview is available; textured rigs, frame rendering and
video export remain unfinished. No model assets download automatically.

## Run headless synthesis on macOS

Install stable Rust. In a separately managed Python 3.10–3.12 virtual environment,
install the runtime requirements and English G2P dependencies described in the
runtime README. Review licenses and obtain user consent before downloading model,
voice, language assets or packages. Place approved model files outside this repo:

```
/absolute/kokoro-assets/config.json
/absolute/kokoro-assets/kokoro-v1_0.pth
/absolute/kokoro-assets/voices/af_heart.pt
/absolute/kokoro-assets/voices/am_michael.pt
```

Then, from this directory:

```sh
cargo run -p motion-voice --example render_dialogue -- \
  examples/dialogue.yaml /absolute/kokoro-assets /absolute/venv/bin/python \
  services/kokoro-runtime/runtime.py /absolute/dialogue-output \
  'UPSTREAM_REVISION:ENVIRONMENT_LOCK_DIGEST'
```

This example synthesizes dialogue, caches individual WAV assets, and writes each
scene's mixed WAV and sample-timed JSON manifest with sample-accurate event placement. It rejects scene overrun
rather than silently truncating dialogue. It runs on the command-line; desktop
code must use `VoiceWorker::submit` and `poll`, never invoke the blocking provider
on its event loop. `audio:` on a speak event selects an imported 24kHz mono PCM16
WAV, resolved relative to the script, without requiring Kokoro inference.

`emotion` is preserved as style metadata; Kokoro acoustic emotion conditioning is
not implemented. Speed, voice preset and language affect synthesis. No weights
or audio are included. Missing dependencies/assets yield structured errors.

## Checks

```sh
cargo fmt --all
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 -m unittest discover -s services/kokoro-runtime/tests -v
```

Python runtime tests: **11 passed**, including synthetic inference/WAV tests with
stub Kokoro/PyTorch modules. No actual model was loaded. After repairing a truncated local LLVM
library, `cargo check --workspace` and `cargo test --workspace` passed on Linux
(**36 Rust tests**, plus doc tests across the expanded workspace). The expanded workspace also includes regression tests for external alignment import,
worker failure recovery and cache corruption. Source was formatted with the toolchain's
`rustfmt` directly; Cargo's formatter wrapper requires `/proc/self/exe`, which is
unavailable in this sandbox. Cargo.lock was generated against crates.io.
The foundation passed standard checks in Linux and macOS GitHub CI. The expanded
workspace passed local formatting, compilation, strict Clippy and tests; its PR
has separate CI results. GUI execution itself has not been tested here.

Actual Kokoro synthesis and macOS/Apple Silicon execution have not been tested.
No native Rust inference, Metal, MPS or Core ML acceleration is claimed. See the
acceptance checklist for what remains before declaring the integration functional.

## Delivery

Source is maintained in https://github.com/aleksipii/kilsatstudio .
The initial source bundle was created while GitHub branch creation returned HTTP
403. Access was subsequently restored and the source was submitted on a dedicated
review branch. See the pull request for current validation results and limitations.
