# Explicit streaming Full HD export

`StreamingProfile::full_hd_24()` explicitly permits up to 1920×1080, 24 fps,
five minutes (300 seconds, 7200 frames) and 600 seconds of retained source audio.
Before 2026-11-03 the limit was 60 seconds, so a five-minute production could not
be streamed in one export. The legacy PNG/SVG
interchange exporters retain their cumulative work bounds. Streaming composes one
frame at a time with the same scene/art/caption/crossfade renderer and pipes raw
RGBA to a caller-selected FFmpeg. It retains audio.wav,sequence.mp4,sequence.json;
it does not retain a PNG sequence. Per-frame geometry/asset validation, a256MiB
working-set estimate, cooperative cancellation, encoder watchdog, deadline and
output/log-size caps remain in force. These estimates are not a hard OS heap cap.

The default export-only deadline is 3600 seconds (the profile maximum; it was 300
seconds) and the output cap 512 MiB; explicit configuration is bounded to
1..3600 seconds and 32..1024 MiB. The measurements below show why 300 seconds
was too short for five minutes of video. This export deadline is
not an end-to-end five-minute result. Whole-pipeline timing must also include
project loading, voice/model work, cache verification, audio preparation and
final media validation. Physical Mac performance and artistic quality are not
established by accepting a profile or a synthetic fixture.

Explicit review size is raster scaling that preserves authored world coordinates,
art/cameras, aspect ratio and frame rate. It is marked review=true; a960×540
review cannot establish1920×1080 performance or quality.

The process watchdog can interrupt a blocked raw-frame pipe by killing/reaping
FFmpeg. Scene rasterization is cooperative between bounded frames, not forcibly
preemptible mid-frame. Output is prepared in a sibling staging directory and
published to an atomically reserved new destination only after encoder confirms
all expected frames. Existing destinations are never intentionally replaced.

## Measured streaming export (2026-11-03)

`crates/motion-export/examples/benchmark_streaming.rs` streams a real `.kilsat`
project through `export_mp4_sequence_streaming` with synthetic silent audio, so it
measures scene evaluation, artwork composition and H.264/AAC encoding only (no
speech synthesis, alignment or project preparation):

```sh
cargo run --release -p motion-export --example benchmark_streaming -- \
    examples/kilsat-season/ep01.kilsat NEW_OUTPUT_DIRECTORY /opt/homebrew/bin/ffmpeg
```

Machine: Apple M1, 8 cores, 8 GB, macOS 27.0.1, release build, FFmpeg from Homebrew.

| Project | Length | Frames | Export time | Frames/s |
| --- | --- | --- | --- | --- |
| `kilsat-season/ep01` (2 characters, 10 scenes, 64 art layers) | 37.9 s | 910 | 37.5–37.9 s | 24.0–24.2 |
| `albus` (1 character, 2 scenes) | 42.0 s | 1008 | 44.0 s | 22.9 |
| ep01 scenes repeated to five minutes (78 scenes, 498 art layers) | 296.9 s | 7126 | 297.5 s | 24.0 |

The five-minute export produced a 1920×1080 H.264 file of 7126 frames (checked
with `ffprobe`), 17.9 MB, with AAC audio of the same duration. Peak resident
memory of the exporting process was 409 MB (`/usr/bin/time -l`, FFmpeg not
included), above the 256 MiB working-set estimate, which is not a heap cap.

One earlier ep01 run measured 7.7 frames/s (118 s) right after a full workspace
test build, with the disk nearly full; three later runs measured 24 frames/s. Treat
that run as a loaded-machine outlier, and expect slower exports while the Mac is
busy. A `sample` profile shows the exporter using one core at about 90 %, almost
all of it in resvg/tiny-skia rasterization; one frame is rendered at a time, so
multi-frame parallel rendering is the obvious next speed-up.

Not measured: speech synthesis and other whole-pipeline stages, the desktop app's
export path by hand, other Macs (Intel, Pro/Max chips) and visual quality.

## Re-measured on main (2026-11-04)

Re-run after the 1.0 integration (main `b56e883` plus this change, which does
not touch export code) on the same kind of machine: Apple M1, 8 GB, macOS
27.0.1, release build, `benchmark_streaming`, synthetic silent audio, Homebrew
FFmpeg. ep01 and Albus were run three times each; the five-minute project was
rebuilt the same way as above (ep01 scenes repeated, the last two scenes of the
eighth copy dropped: 78 scenes, 7126 frames) and run once.

| Project | Length | Frames | Export time | Frames/s | Peak RSS |
| --- | --- | --- | --- | --- | --- |
| `kilsat-season/ep01` | 37.9 s | 910 | 33.8–34.0 s | 26.8–26.9 | 528–615 MB |
| `albus` (now YAML source) | 42.0 s | 1008 | 31.2 s | 32.3 | 480–481 MB |
| ep01 repeated to five minutes | 296.9 s | 7126 | 270.9 s | 26.3 | 728 MB |

All runs were faster than real time and the three runs per project agree within
1 %. `ffprobe` on the five-minute file: H.264 1920×1080, 7126 frames, AAC,
296.92 s, 17.9 MB. Wall time 270.9 s against 357.4 s of user CPU, so FFmpeg
encodes alongside the single-threaded renderer.

Peak RSS (`/usr/bin/time -l`, the same tool as above) is higher than the earlier
409 MB and well above the 256 MiB working-set estimate. The figure may include
the FFmpeg child; it was not separated. Find out where the memory goes before
relying on long exports on memory-limited devices.
