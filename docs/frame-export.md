# SVG frame export

The headless frame exporter writes individual SVG vector images of the original
procedural characters. It uses the existing MotionScript scene evaluator for
movement, expression and wave gestures. It does not encode a movie, embed audio,
run Kokoro, import texture rigs or generate alignment timestamps.

## Run without speech assets

From the workspace root, with Rust installed:

```sh
cargo run -p motion-render --example render_frames -- \
  examples/hello-emma.yaml 0 /absolute/new-emma-frames
```

The scene index is zero-based. The output directory must not exist; its parent
must already exist. This command can render movement without Python or Kokoro
assets even when the screenplay contains dialogue. Dialogue is not synthesized
by the frame exporter, and its exported characters have neutral mouths.

Output contains:

| File | Contents |
| --- | --- |
| `frame_000000.svg`, `frame_000001.svg`, … | One self-contained vector image per frame |
| `manifest.json` | Schema version, scene identity, dimensions, fps, duration, frame count, original MotionScript text and each frame's time/file |

Open an SVG in a compatible viewer to inspect it. Retain `manifest.json` with the
sequence: it records the intended frame ordering and clock. The manifest includes
the complete source screenplay, including any dialogue and asset path strings.

## Timing and limits

Frame `i` is evaluated at `i / fps` seconds while that time is strictly less
than the scene duration. The exact scene-end instant is excluded; the exporter
checks the final candidate time to avoid an extra frame from floating-point
rounding of a decimal duration. The pose exporter uses the same shared frame
count rule.
For example, a one-second scene at 24 fps produces 24 images at times 0 through
23/24 seconds. For a duration that is not an exact frame multiple, the final
frame is still evaluated before scene end. The renderer does not consult a wall
clock or an audio device.

The command refuses existing output directories and validates the scene and first
render before creating output. Input is limited to 1 MiB of UTF-8 source and
10,000 events across all scenes. A sequence is limited to 10,000 frames, 100,000
actor-frame evaluations and 256 MiB including the JSON index. An additional work
budget requires `frames × total_events × (character_count + 1) ≤ 1,000,000`;
events in other scenes count because evaluation validates the whole script.
If writing fails,
it attempts to remove only the output directory created by that invocation and
reports a cleanup failure if it cannot remove partial output. It does not provide
a resumable export or an interactive cancellation control.

The SVG renderer accepts output dimensions from 1 through 8192 pixels per axis
and up to 256 actors, with bounded labels and finite pose values. The command uses
the project's width and height. SVG is a separate rendering backend using the
same evaluated poses; graphical parity with the desktop is not certified.
System sans-serif fonts can render labels differently across machines.

## Audio and mouth animation remain separate

The existing `render_dialogue` command in the [README](../README.md) writes a
scene WAV and sample-timed dialogue/viseme JSON. This SVG command does not yet
consume that JSON or copy the WAV. Although the SVG renderer can draw supplied
viseme states, this command supplies no timed mouth track. A waveform, Kokoro
phoneme string or frame index is not a validated alignment model.

The next synchronization step must join those timed visemes to visual frame
sampling, then encode the frames and mux the scene WAV. Encoded-video duration,
frame/audio synchronization, seek behavior and perceptual lip-sync quality need
their own acceptance checks. Real Kokoro synthesis and physical audio playback
on Apple Silicon remain separately unverified until machine, WAV and listening
evidence is recorded in the [practical test guide](practical-testing.md).
