# Native dialogue workbench

Run from the repository root:

```sh
cargo run -p studio-desktop
```

This native egui/eframe app edits and validates MotionScript YAML, selects dialogue,
submits bounded background generation/import jobs, previews duration and waveform,
and scrubs a sample-based symbolic mouth display. It does not play device audio,
render character rigs or export video. macOS GUI execution and real Kokoro
synthesis require local platform testing.

Install the Kokoro CPU runtime separately following `services/kokoro-runtime`.
Set the Python executable, runtime script, model config, weights, voice directory,
cache directory, and immutable model/dependency revision in the app. No files are
downloaded. Voice preset, language, speed and imported `audio` path come from YAML.
Imported WAV must be nonempty 24 kHz mono PCM16. Paths resolve from the configured
project root. Screenplay edits invalidate event selections until revalidation.

Paste optional alignment JSON before generation/import:

```json
{"source":{"kind":"manual"},"duration_seconds":1.0,"phonemes":[{"phoneme":"m","start_seconds":0.1,"end_seconds":0.3}]}
```

Set duration to the actual audio duration. The alignment parser checks interval
structure and supported phonemes; it does not certify perceptual lip-sync quality.
Without alignment, the mouth remains at rest. The preview offset is session-only;
edit `at` in YAML to retain a timing change. Use the explicit screenplay path and
Open/Save buttons for UTF-8 YAML (100 KB limit); Save replaces that path. Runtime
settings and preview offsets remain session-only.

The UI deliberately labels these boundaries. No native Rust TTS, Metal acceleration,
physical audio synchronization or perceptual alignment validation is claimed.
