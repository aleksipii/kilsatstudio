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

## Professional delivery: OTIO, ProRes and colour tags (P07)

**OpenTimelineIO.** `sequence_otio` / `write_sequence_otio` write `timeline.otio`
next to a sequence export, with plain serde and no Python dependency. `parse_otio`
reads it back and rejects unknown schema versions. Structure: `Timeline` →
`Stack` → one video track (one clip per scene), a `Mix` audio track over the
whole sequence WAV, and disabled per-actor `Dialogue:` tracks (a second lane
is added when one actor's lines overlap). Video times are whole frames from
`floor(frame * 24000 / fps)`, the same rule as PNG/MP4/ProRes export. Audio
times are whole 24 kHz samples. No floating-point sums are involved. Scene
crossfades are already rendered into the media, so they appear as a
`Crossfade (baked)` marker, not as an OTIO transition that would dissolve a
second time. Media paths are relative (`frame-%06d.png` as an
`ImageSequenceReference`, or a movie file name). Dialogue clips are slices of
the mixed WAV, not isolated stems, and say so in their metadata. The golden file
is `crates/motion-export/tests/golden/two_scene_crossfade.otio`. After reviewing
a deliberate change, regenerate it with `KILSAT_UPDATE_GOLDEN=1`.

**ProRes.** `export_prores_sequence` and `encode_prores_frames` encode through the
caller-selected FFmpeg (`prores_ks`): profile `Hq` (422 HQ, `yuv422p10le`)
or `P4444` (4444 with alpha). ProRes 4444 is natively 12-bit, so ffprobe reports
`yuva444p12le`. It uses the same missing-FFmpeg error, cancellation, timeout and
partial-file cleanup as MP4. The movie has its own size cap
(`ProResOptions::max_video_bytes`, 2 GiB by default, 1 MiB–16 GiB), because the 256 MiB MP4 budget
would be reached within seconds of ProRes.

**Colour.** Kilsat renders 8-bit sRGB-encoded RGBA. **There is no ACES or OCIO
pipeline** and no transform between the renderer and the file: the tags only
state how the code values should be read. ProRes uses the Rec.709 Y'CbCr matrix
in limited range, with BT.709 primaries. `ColorTag::Srgb` (the default) writes
transfer `iec61966-2-1`, and `ColorTag::Rec709` writes `bt709` as a relabel of
the same pixels. The tags are stamped on every frame with FFmpeg's `setparams`
filter. FFmpeg 9 writes the frame's own colour properties and ignores
`-color_trc` given alone. Exported PNGs carry `sRGB` (perceptual) and
`gAMA` (45455) chunks after `IHDR`, and their pixel data is unchanged. The
existing H.264 MP4 path is still untagged and uses FFmpeg's default BT.601
matrix; this is a known follow-up. EXR output is not implemented and needs a
dependency decision first.

Verified on macOS Apple Silicon with Homebrew FFmpeg 9.0.2 by running
`REAL_FFMPEG=/opt/homebrew/bin/ffmpeg cargo test -p motion-export --lib prores:: -- --include-ignored`:

- HQ: `prores`, profile `HQ`, `yuv422p10le`, `bt709`/`bt709`/`bt709`, tv range,
  6 frames, which equals the export frame count, plus a `pcm_s16le` 24 kHz audio stream.
- 4444: profile `4444`, `yuva444p12le`, transfer `iec61966-2-1`, 3 frames. After
  decoding, the transparent half has alpha 0 and the opaque half has alpha 255.

The official `opentimelineio` library has not read the golden file locally:
it had no build for Python 3.15. CI tries it and reports the check as skipped
when the library cannot be installed.

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
