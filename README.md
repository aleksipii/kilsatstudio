# Kilsat Studio — Agent 6 audio foundation

Independently authored offline voice infrastructure for the repository's Agent 6
workstream. This is a headless integration foundation, not a complete desktop
application. No Storytold source or dependencies are used.

## Components

- `motion-script`: strict dialogue-only YAML schema, character voice settings,
  scene/actor/time validation and imported WAV references.
- `motion-tts`: replaceable `SpeechProvider`, local Kokoro Python provider,
  content-addressed cache, corrupt-cache regeneration and subprocess timeout.
- `motion-audio`: WAV I/O, waveform peak data and deterministic sample-frame mixing.
- `motion-lipsync`: timed IPA alignment validation, mouth shapes and seek evaluation.
- `motion-voice`: bounded background worker, dialogue jobs and shared audio/mouth
  timeline placement, manual offset/trim/gain through prepared clips.
- `services/kokoro-runtime`: local CPU inference service with explicit asset paths.

See [runtime setup](services/kokoro-runtime/README.md) and
[architecture and verification](docs/audio.md).

## Run on macOS

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
scene's mixed WAV with sample-accurate event placement. It rejects scene overrun
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

Python protocol tests: **4 passed** in the supplied Linux environment. Rust
checks were attempted but could not run: the downloaded toolchain requires
`/proc/self/exe`, unavailable here, and its compiler could not load LLVM. Rust
source has not been compiler-verified or rustfmt-verified. Run formatting and
checks before committing. Cargo.lock was generated successfully against crates.io;
this confirms dependency resolution, not compilation.
The included CI runs on Linux and macOS once the source is pushed; no CI result
is claimed.

Actual Kokoro synthesis and macOS/Apple Silicon execution have not been tested.
No native Rust inference, Metal, MPS or Core ML acceleration is claimed. See the
acceptance checklist for what remains before declaring the integration functional.

## Delivery

Source is maintained in https://github.com/aleksipii/kilsatstudio .
The initial source bundle was created while GitHub branch creation returned HTTP
403. Access was subsequently restored and the source was submitted on a dedicated
review branch. See the pull request for current validation results and limitations.
