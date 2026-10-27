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

## Movement-only exporter boundary

The existing `render_dialogue` command in the [README](../README.md) writes a
scene WAV and sample-timed dialogue/viseme JSON. This SVG command does not yet
consume that JSON or copy the WAV. Although the SVG renderer can draw supplied
viseme states, this command supplies no timed mouth track. A waveform, Kokoro
phoneme string or frame index is not a validated alignment model.

Use the scene-bundle exporter below to join timed visemes to visual frames.
Encoding frames and muxing scene audio into a video remains future work. Encoded-video duration,
frame/audio synchronization, seek behavior and perceptual lip-sync quality need
their own acceptance checks. Real Kokoro synthesis and physical audio playback
on Apple Silicon remain separately unverified until machine, WAV and listening
evidence is recorded in the [practical test guide](practical-testing.md).

## Export a scene with audio and timed mouths

The `motion-export` path adds an existing scene WAV and its dialogue/viseme
manifest to the visual sequence. It does not synthesize speech or invent timings.
After preparing matching audio and metadata, run:

```sh
cargo run -p motion-export --example render_scene -- \
  examples/hello-emma.yaml 0 /absolute/dialogue-output/scene-0.wav \
  /absolute/dialogue-output/scene-0.json /absolute/new-scene-export
```

The destination must be new. Output contains `frame_000000.svg` and subsequent
frames, `scene.wav`, `scene.json`, and a new `manifest.json` indexing the sequence.
This index records the serialized source project, fps, dimensions, audio file
names and each visual frame's sample position. These files remain a frame/audio
interchange bundle, **not an encoded video file**.

For visual frame `i`, this path uses sample frame
`floor(i × 24000 / fps)` and evaluates both the actor pose and mouth state at that
sample's time. Ordered half-open viseme intervals select mouth shapes; silence,
gaps and clip ends return the mouth to rest. The sample-clock quantization can
place a visual frame slightly before its nominal `i / fps` time; when fps divides
24 000 (12, 24, 25, 30, 60, …) the time is exactly `i / fps`. The sample comes
from `motion_audio::clock::frame_to_sample` (ORK-G1-04), and the desktop preview
(`ScenePreview::render`) evaluates the same sample with the same evaluator and
renderer. The frame count is `motion_scene::frame_count(duration, fps)`; the audio
track is `round(duration × 24000)` samples and differs from the video length by
less than one frame. Tests and the G2 reproduction command:
[evidence/g2-export](evidence/g2-export/README.md).

Input checks include the scene ID, schema version, duration, 24 kHz mono audio,
known actors, clip bounds, finite gains, non-overlapping dialogue for each actor,
ordered visemes and alignment provenance. The interchange format has no audio
content hash: these checks cannot establish that an externally supplied WAV is
the original recording for its timings, or that alignment is perceptually good.
Keep each WAV with its original manifest and record listening evidence separately.

The library requires a validated project no larger than 1 MiB when serialized,
at most 10,000 frames, 10,000 events across all scenes, 256 characters, 100,000
actor-frame evaluations and 1,000,000 event/frame/actor work units as above.
It also limits input to 10,000 dialogue clips and 100,000 viseme intervals,
caps frame × (dialogue clips + viseme intervals) work at 1,000,000, and
limits each output JSON manifest to 8 MiB. The complete output is capped at
256 MiB. Scene audio must match the rounded scene
sample count and the existing one-hour scene limit. Renderer dimension limits
still apply. The CLI additionally bounds source input to 1 MiB, scene-manifest
JSON to 8 MiB and WAV input to 128 MiB before decoding.

The blocking `motion_export::export_scene` API accepts a cancellation token and
reports completed/total frames. Callers must run it off the UI thread. Cancellation
is checked around validation and between frames and output phases; it does not
interrupt a single filesystem write. Errors or cancellation attempt to remove
only the new directory created by that invocation. A missing or relative FFmpeg
path is `Error::EncoderUnavailable` before any output exists. If removing the
partial directory fails, the result is `Error::PartialOutput { path, cause,
cleanup }`: the original error is kept and the leftover directory is named.
A completed directory is published with `manifest.json`; no resume function or
encoded media is provided.

## Export from the desktop

1. Open and validate the screenplay, then select its scene. For a model-free
   trial, use **Load sound-check demo**; its tones and mouth cues are illustrative.
2. Choose **Prepare full scene audio** and wait for preparation to finish.
3. In the Inspector, expand **Scene export · SVG + WAV** and enter an absolute
   **New output directory** whose parent already exists. The destination itself
   must not exist.
4. Choose **Export scene bundle**. The background job reports a percentage and
   completed/total frames. Use **Cancel export** in the toolbar to stop it.

The export button requires prepared full-scene audio and a destination path. It
does not export a selected line as a complete scene or silently generate missing
audio. Cancellation removes partial output where possible while keeping the
prepared scene audio available for another attempt. These steps describe the
implemented controls; real Mac GUI, playback and perceptual sync acceptance still
need their separately recorded checks.
