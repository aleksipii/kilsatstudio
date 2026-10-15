# PNG and MP4 export

The shared scene evaluator renders the same poses and half-open mouth intervals
as SVG export. PNG uses local Rust resvg rasterization (no external process).
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

Raster allocation is limited to 16 million pixels per frame. Existing 10,000-frame,
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
