# G2 scene export: command, regression and measurements (A10)

Scene: [`examples/g2-minimal-scene/scene.kilsat`](../../../examples/g2-minimal-scene/README.md)
(A08, SHA-256 `8ba99dc39ed851d076f69370c83a35237b738aaa6a062ae1bba75366dcbd8f03`),
1920x1080, 24 fps, 4 s, silent, phone in VEIKKO's right hand (KS-003c).

## Reproducible export command

From the repository root; `OUT_DIR` must not exist:

```sh
cargo run --locked --release -p motion-export --example g2_export -- OUT_DIR svg
cargo run --locked --release -p motion-export --example g2_export -- OUT_DIR png
cargo run --locked --release -p motion-export --example g2_export -- OUT_DIR mp4 --ffmpeg /ABSOLUTE/PATH/ffmpeg
```

The command loads the scene, prepares audio with `ModelPolicy::Strict`
(`motion_voice::model_free::prepare_scene_model_free`, `DisabledSpeechProvider`;
the scene has no dialogue, so the result is 96 000 samples of silence) and exports
with the same evaluator and renderer as the desktop preview. It prints a JSON
summary: frame count and `frame_count(duration, fps)`, audio samples and seconds,
video seconds, per-frame SHA-256 digest, the model-backed steps the strict policy
refused (`model_policy_refused`, read back from `ModelPolicy::permit`) and timings.
It exits with an error if the frame count does not match, or if an SVG export of
the recorded G2 scene differs from `G2_SVG_SEQUENCE_SHA256`
(`sequence_matches_reference`). `--runs N` adds N timed runs after one discarded
warm-up. Source: [`crates/motion-export/examples/g2_export.rs`](../../../crates/motion-export/examples/g2_export.rs).

## Frame clock

Frame `i` is evaluated at `t = floor(i × 24000 / fps) / 24000` s through
`motion_audio::clock::frame_to_sample` (ORK-G1-04). The desktop preview
(`ScenePreview::render`) evaluates the same sample with the same
`ValidatedScript` and `render_svg_with_art`. At 24 fps this is exactly `i / 24`.
Tested by `export_preview_parity_tests` (exported SVG equals a direct evaluation
at `i / fps`, and the preview's pixels equal the exported SVG rasterized at the
preview size, for frames 0, 1, 23, 47, 48, 72 and 95).

## Regression hashes

The values live only in [`crates/motion-export/tests/support/g2.rs`](../../../crates/motion-export/tests/support/g2.rs)
(`G2_SCENE_SHA256`, `G2_SVG_*`, `G2_PNG_*`). The test `tests/g2_export.rs`, the
`g2_export` command and the `Export QA` workflow read that file; the table below is
the record at the time of writing. A PR that changes the G2 scene fails first on
`G2_SCENE_SHA256` with a message to re-record the hashes in the same PR.
`sequence` is SHA-256 of the 96 per-frame SHA-256 hex strings joined with `\n`
(trailing `\n`).

| Output | Frame 24 | Frame 72 | Sequence | Asserted on |
| --- | --- | --- | --- | --- |
| SVG | `5f8f2ffc832fa87bca76c7009a432fa9a9770710ae463188cf65c89f47507c8f` | `08394cfaf48232eafa2dc03b9828f4814302f278381c82e897892e77f1efe08e` | `1de95277c7718eb151cbf3ef57043814df2e44dd7f685aea0ba617838fe8e481` | every platform (CI: macOS, Ubuntu) |
| PNG | `8bee3aa180d1041408a9e04e4d5fe2ef31215f2f47df040db8b7389a52edc658` | `85c0e33ec5cf848f0c21bcf17c245eb929da4623d776b7868eb575a6bece7f23` | `6988655b93873dba7d1b4cfe9654cf69fabef0f61d2d1c035554ee93eced3809` (recorded, not asserted) | Apple Silicon macOS only |

The PNG frame 24 and 72 hashes equal the export
hashes A08 recorded independently in [`../g2-render/README.md`](../g2-render/README.md).
PNG rasterization is floating-point work in resvg/tiny-skia; identical PNG bytes on
other CPUs are not claimed. Not every frame inside a shot is identical: SVG frames
46, 48, 49, 86 and 89 differ from the frame before them.

## Failed exports

| Case | Result | Test |
| --- | --- | --- |
| FFmpeg missing or not an absolute path | `Error::EncoderUnavailable { path }`, no output, no frame rendered | `missing_ffmpeg_is_a_typed_error_before_any_output` |
| Cancellation after 10 frames | `Error::Cancelled`, directory removed | `cancellation_during_export_is_typed_and_removes_partial_output` |
| Write fails mid-sequence (frame path taken) | `Error::Io`, directory removed | `io_failure_mid_export_is_typed_and_removes_partial_output` |
| Cleanup itself fails | `Error::PartialOutput { path, cause, cleanup }`: original error kept, leftover directory named | `failed_cleanup_reports_the_cause_and_the_leftover_directory` (unix, non-root) |
| Streaming MP4: encoder blocks past the deadline and cleanup fails | `Error::PartialOutput` naming the staging directory, cause = deadline | `streaming::tests::failed_streaming_cleanup_reports_the_cause_and_the_leftover_directory` (unix, non-root) |
| Real full disk (2 MiB RAM disk) | `Error::Io` with `StorageFull` (ENOSPC), directory removed | `full_disk_is_typed_and_removes_partial_output` (ignored; CI macOS runs it) |

Full-disk test locally (macOS, no sudo needed):

```sh
dev=$(hdiutil attach -nomount ram://4096 | awk '{print $1}')
diskutil erasevolume HFS+ KILSATFULL "$dev"
KILSAT_TEST_FULL_VOLUME=/Volumes/KILSATFULL cargo test -p motion-export --test g2_export full_disk -- --ignored
diskutil eject "$dev"
```

## Model-free run

[`crates/motion-export/scripts/g2_model_free_run.sh`](../../../crates/motion-export/scripts/g2_model_free_run.sh)
runs the export under `sandbox-exec` with all network access denied and every
process start denied except the export binary (and, for MP4, the resolved FFmpeg),
with an empty `HOME` and `PATH=/nonexistent`. It fails if anything was written under
`HOME` or a model-like file (`.onnx`, `.pth`, `.pt`, `.safetensors`, `.bin`, `.npz`)
appears in the output. CI checks first that the same sandbox profile really blocks
`curl` and `/bin/echo`. `sandbox-exec` is deprecated by Apple but works on macOS 15
runners and on macOS 27 locally.

Limits: the sandbox shows that no network connection and no other process was
started. It does not by itself prove that no model file was read in-process
(`allow default` permits file reads); that guarantee comes from `ModelPolicy::Strict`
and `DisabledSpeechProvider`. The summary field `model_policy_refused` lists the
steps `ModelPolicy::Strict` refused in that run. The G2 scene has no dialogue, so the
strict path has not yet been run with imported WAV speech (waits for A07, KS-002e).

```sh
cargo build --locked --release -p motion-export --example g2_export
crates/motion-export/scripts/g2_model_free_run.sh target/release/examples/g2_export OUT_DIR png
```

## Measurements

The measurements below were taken on the scene before the phone was added
(scene SHA-256 `bf58fc78…`, commits `d9b88ae`, `c594cb8` and `71ad594`).

Wall-clock time of the whole export call (evaluation, rendering, PNG encoding,
writing files, WAV and manifest), release build. This is offline export time. It
is **not** a playback, preview or display frame rate, and nothing here measures
60/120 FPS playback or 100 000 entities. The G2 scene has 2 characters.

Local, 28.10.2026, [`local-apple-m1-2026-10-28.json`](local-apple-m1-2026-10-28.json):
commit `71ad594` (main `10b012a` + this follow-up; the SVG/PNG export code is the
same as in `10b012a`), same machine, macOS and rustc as below, AC power, 1 warm-up
+ 7 measured runs. The machine was busy: load average about 10 (1 min) and 24
(5 min) right after the runs, under 4 GiB free disk.

| Format | Median | Min | Max | Median per frame |
| --- | --- | --- | --- | --- |
| SVG | 30.5 ms | 30.4 ms | 31.8 ms | 0.32 ms |
| PNG | 1964.7 ms | 1879.1 ms | 2194.3 ms | 20.47 ms |

PNG was 28 % slower than on 27.10. with the same code paths; the two runs differ in
background load, so the difference is not attributed to the code. Neither run is
an isolated benchmark.

Local, 27.10.2026, [`local-apple-m1.json`](local-apple-m1.json):

| Conditions | |
| --- | --- |
| Machine | iMac21,2, Apple M1 (4P + 4E cores), 8 GiB |
| macOS | 27.0.1 (26A434), AC power, interactive session (not isolated) |
| Toolchain | rustc 1.99.1, `--release` |
| Commit | `d9b88ae` + this branch |
| Scene / output | G2, 1920x1080, 24 fps, 96 frames, local SSD, 4 export workers |
| Runs | 1 warm-up discarded, 7 measured |

| Format | Median | Min | Max | Median per frame |
| --- | --- | --- | --- | --- |
| SVG | 30.4 ms | 24.3 ms | 33.6 ms | 0.32 ms |
| PNG | 1531.9 ms | 1485.5 ms | 1544.8 ms | 15.96 ms |

CI, [run 40558399213](https://github.com/aleksipii/kilsatstudio/actions/runs/40558399213) (commit `c594cb8`; head `d5203c0` re-run [40560366530](https://github.com/aleksipii/kilsatstudio/actions/runs/40560366530) also passed): GitHub `macos-15`, VirtualMac2,1
Apple M1 (virtual), 3 CPUs, 7 GiB, macOS 15.7.9 (24G830), rustc 1.99.1, 5 measured runs:
SVG median 31.9 ms (31.4–32.8), PNG median 3038.0 ms (2830.9–3095.3). The same run
decoded the MP4 as 96 frames, video 4.000 s, audio 4.000 s.

The `Export QA` workflow measures the same command on the GitHub `macos-15`
runner (shared virtual machine, 5 measured runs) and writes the table with the
runner's model, macOS version, rustc and commit into the job summary and the
`g2-export-qa-<sha>` artifact. CI timings are not comparable with a physical Mac.

MP4 encoding was not run locally (no FFmpeg installed on this machine); the CI
job encodes the MP4 and checks 96 decoded frames and audio/video duration.
