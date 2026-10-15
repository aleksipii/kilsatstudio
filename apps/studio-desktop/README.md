# Native dialogue workbench

Run from the repository root:

```sh
cargo run -p studio-desktop
```

This native egui/eframe app edits and validates MotionScript YAML, selects dialogue,
submits bounded background generation/import jobs, previews duration and waveform,
and scrubs a sample-based symbolic mouth display. macOS device playback is available as the next stage below; it also renders original procedural characters with deterministic scene movement, expressions and waves. It does not export video. macOS GUI execution and real Kokoro
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
hardware latency calibration or perceptual alignment validation is claimed.

## Playback stage

On macOS, prepare a dialogue WAV and use Play from cursor, Pause, Resume, Stop,
or the scene playhead slider. A bounded worker opens the current default output
device and retains the stream. PCM stays shared; seek replaces the source at an
exact clip-local frame. Mouth preview reads Rodio's consumed source position,
including the seek offset, rather than advancing a separate wall-clock timer.
This does not measure hardware latency; perceptual synchronization requires
listening and visual validation on a real Mac. The UI continues polling while a
device opens or reports an error. Linux offers waveform scrubbing and an explicit
unsupported-device message; audio-device output is currently macOS-only.

Device-open errors appear in the UI and Play can retry. Rodio runtime device
faults currently go to stderr; an unplugged/stalled device may require restarting
the app. No hardware playback or GUI runtime test has been performed here.


## Scene preview stage

The default screenplay is `examples/hello-emma.yaml`. Validate it, select a scene,
and use Play scene (silent), Pause scene, Restart scene or the scene slider.
Movement, expressions and wave gestures are evaluated from the same scene time
on every frame. Scene selection, screenplay edits and validation failures discard
stale state. The preview uses simple original procedural characters rather than
loading external character assets.

Silent playback advances a quantized 24 kHz scene clock using elapsed time. While
a prepared dialogue source is loaded for this scene, its source sample position
drives the scene and matching actor mouth instead. Scene controls stop that source
before switching to silent transport; silent playback does not automatically mix
or play all dialogue events. The waveform and standalone mouth detail remain
available. Alignment quality and device latency still require real Mac testing.
