# PNG and MP4 export

The shared scene evaluator renders the same poses and half-open mouth intervals
as SVG export. Diagnostic actor/emotion SVG text labels are omitted by the
font-free rasterizer; procedural character shapes remain. PNG uses local Rust resvg rasterization (no external process).
Every frame uses `floor(frame * 24000 / fps)` as its sample clock. Export preserves
PNG frames, `scene.wav`, `scene.json` and `manifest.json`; MP4 adds `scene.mp4`.
The WAV/alignment pairing supplied by the caller is not quality-certified.

The command-line example accepts:

```sh
cargo run -p motion-export --example render_scene -- SCRIPT SCENE_INDEX WAV SCENE_JSON NEW_DIRECTORY png
cargo run -p motion-export --example render_scene -- SCRIPT SCENE_INDEX WAV SCENE_JSON NEW_DIRECTORY mp4 /absolute/path/to/ffmpeg
```

Omitting the format preserves SVG export. MP4 requires a separately installed,
explicitly selected local FFmpeg executable with libx264 and AAC support. Nothing
is downloaded, PATH is not searched, and no shell is invoked. MP4 requires even
project dimensions for yuv420p; dimensions are never silently changed. The video
frame sequence covers the scene with ceiling(duration × fps) frames, so its final
frame can extend by less than one frame beyond WAV duration. Audio is not padded
or time-stretched by the exporter.

Raster allocation is limited to 16 million pixels per frame and one billion
rasterized pixels per sequence. Existing 10,000-frame,
scene work and 256 MiB output budgets still apply. The encoder has a five-minute
timeout, null stdin/stdout/stderr and a bounded output-size request; cancellation
kills and reaps it. Output exceeding the byte cap is rejected. Existing output
directories are never overwritten; a failed/cancelled job removes only the new
directory it created. Encoding runs after frame progress reaches its total; callers
must wait for the function's successful return before reporting completion.

`real_ffmpeg_encodes_h264_aac` is ignored by default. Run it explicitly with
`KILSAT_TEST_FFMPEG=/absolute/path/to/ffmpeg` and optionally
`KILSAT_TEST_FFPROBE=/absolute/path/to/ffprobe` to inspect video/audio codecs and
raster dimensions. Ordinary tests decode generated PNGs and check cancellation,
clock metadata and resource rejection without an encoder installation.

This implements CPU frame rasterization and local video encoding. It does not
implement offscreen GPU rendering, parallel rendering, a bundled FFmpeg distribution,
perceptual lip-sync measurement, or physical Mac audio-device acceptance testing.

Verified in this development environment (Linux, FFmpeg 6.1.1): both a one-second
scene and a fractional 0.9-second scene encoded successfully. ffprobe verified
H.264/AAC, 640×360 dimensions, four video frames at 4 fps, a one-second video
stream and the scene-length audio stream (50 ms tolerance). The ordinary export
suite passed 11 tests; the explicit real-encoder test also passed. This does not
establish macOS encoder or device compatibility.


## Dependency and encoder distribution boundary

The pinned rasterizer is resvg 0.45.1 (its downloaded package metadata declares
Apache-2.0 OR MIT;
[revision-specific source](https://github.com/linebender/resvg/tree/v0.45.1)).
The release must retain the notices for the exact locked dependency tree.
The preview does not package FFmpeg, libx264 or model assets. The user selects
a separately installed binary; its build configuration and applicable notices
must be reviewed before any future redistribution. FFmpeg documents that optional
GPL components such as libx264 affect the selected encoder build's license:
[official license guidance](https://www.ffmpeg.org/legal.html). This is a
distribution boundary, not a completed dependency or release-license audit.
