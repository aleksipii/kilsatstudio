# CPU/GPU parity measurements and GPU font position

Status: measured regression guard for the experimental GPU path. It is **not**
perceptual validation and **not** a claim that the GPU path equals the CPU path.

## Harness

`crates/motion-export/src/gpu_parity_tests.rs` (feature `gpu` of `motion-export`).
Each fixture gives the identical `EvaluatedScene`, artwork `Document` and decoded
images to both renderers at 128x128:

- CPU reference: `render_svg_with_art` rasterized by resvg via `media::rasterize`
  (the preview/export path).
- Candidate: `scene_commands::build_with_art` + `GpuSceneRenderer` with
  `Labels::OmitAcknowledged` on Metal.

Differences use `compare_decoded_rgba` (byte-space RGBA, no colour-space
conversion). Each fixture has a `Tolerance` (max channel error, max per-channel
RMS, changed-pixel fraction); the hardware test fails if any fixture exceeds it.
Run by hand (ignored by default; needs a real GPU):

```sh
KILSAT_GPU_BACKEND=metal KILSAT_PARITY_REPORT=report.txt \
  cargo test -p motion-export --features gpu gpu_parity -- --ignored --nocapture
```

Default-run tests (no GPU, under `--features gpu`): the tolerance gate rejects each
exceeded limit, and the CPU rasterizer draws nothing for actor `<text>` labels.

Fixtures: actor (default, camera zoom/pan, rotated/scaled/faded), shapes
(rect/ellipse/stroke, camera zoom), upscaled image, hard ellipse/polygon masks, soft
ellipse mask, image alpha/luminance masks, painted mask, feathered polygon mask,
group Multiply/Screen, layer Multiply, actor with masked background art.

## Recorded measurements

Machine: Apple M1, macOS 27.0.1 (`sw_vers`), backend Metal, base commit `0efa65d`
(this branch adds the harness on top). Two consecutive runs produced byte-identical
reports. Single machine, single driver; 128x128 synthetic fixtures only.

| Fixture | Max R,G,B,A | RMS R,G,B,A | Changed px | Tolerance (max / rms / changed) |
| --- | --- | --- | --- | --- |
| actor_labels_omitted | 26,39,47,0 | 1.38,1.76,1.90,0 | 3.65% | 60 / 4.0 / 10% |
| actor_camera_zoom_pan | 17,26,31,0 | 1.28,1.93,2.19,0 | 4.69% | 60 / 4.0 / 10% |
| actor_rotated_scaled_faded | 40,39,47,0 | 2.04,2.45,2.78,0 | 5.76% | 60 / 4.0 / 10% |
| actor_with_masked_art | 35,30,54,0 | 2.49,2.16,2.12,0 | 6.29% | 60 / 4.0 / 10% |
| shapes_rect_ellipse_stroke | 23,23,8,0 | 2.40,1.88,0.63,0 | 21.43% | 40 / 4.0 / 30% |
| shapes_camera_zoom | 2,7,16,0 | 0.23,0.82,1.87,0 | 1.37% | 40 / 4.0 / 30% |
| image_upscaled | 11,11,6,0 | 1.13,1.13,1.00,0 | 48.51% | 24 / 2.5 / 60% |
| mask_image_alpha | 12,4,4,0 | 1.64,0.44,0.44,0 | 27.78% | 24 / 2.5 / 60% |
| mask_image_luminance | 10,3,3,0 | 1.16,0.57,0.57,0 | 47.84% | 24 / 2.5 / 60% |
| mask_hard_ellipse | 42,10,10,0 | 2.75,0.65,0.65,0 | 2.48% | 60 / 4.0 / 6% |
| mask_hard_polygon | 29,7,7,0 | 1.95,0.47,0.47,0 | 2.32% | 60 / 4.0 / 6% |
| mask_soft_ellipse | 0,1,1,0 | 0,0.37,0.37,0 | 13.53% | 4 / 1.0 / 25% |
| mask_painted | 35,8,8,0 | 6.73,1.66,1.66,0 | 9.37% | 100 / 8.0 / 35% |
| mask_feathered_polygon | 64,15,15,0 | 3.91,0.95,0.95,0 | 25.20% | 100 / 8.0 / 35% |
| group_multiply | 15,9,3,0 | 2.88,1.68,0.54,0 | 8.18% | 30 / 4.0 / 40% |
| group_screen | 15,9,13,0 | 2.66,1.78,2.17,0 | 28.64% | 30 / 4.0 / 40% |
| layer_blend_multiply | 15,9,3,0 | 2.91,1.65,0.53,0 | 5.47% | 30 / 4.0 / 40% |

Reading the numbers: alpha is exact everywhere (the frames are opaque). Interior
RMS is about 0 for geometry fixtures, so the error is concentrated on edges: the
GPU uses an analytic 1-pixel smoothstep edge, resvg uses tiny-skia coverage, and
actors differ at smile/join geometry. Maximum single-pixel edge errors reach
26-64 of 255 on actors, hard masks and the feathered polygon (corners are
approximate). Image/group fixtures differ by small interior amounts (max 6-15),
consistent with bilinear sampling and blend rounding differences. The tolerances
are the measured values plus headroom, so they catch regressions; they do not
define acceptable quality and are not tight.

Not established: other GPUs/drivers/OS versions, larger frames, animated mask
motion on the GPU (unsupported), perceptual equivalence, performance.

## Fonts / GPU text

Finding: the CPU path draws **no label text at all**. `render_svg*` emits
`<text font-family="sans-serif">` for each actor, but export/preview rasterize with
`resvg 0.45.1` built with `default-features = false, features = ["raster-images"]`.
usvg compiles `<text>` handling only under its `text` feature, and
`usvg::Options::default()` has an empty font database; Cargo.lock has no
`fontdb`/`rustybuzz`. The test `cpu_rasterizer_draws_no_actor_label_text`
asserts that rasterizing with and without the `<text>` element yields identical
pixels. No `.ttf/.otf/.woff` file is bundled in the repository.

Consequences:

- `Labels::OmitAcknowledged` on the GPU is pixel-consistent with what the in-repo
  CPU rasterizer produces today; the parity numbers above therefore include no
  label error. It remains an explicit opt-in because other SVG consumers that have
  fonts would draw the labels.
- There are no CPU glyph outlines or advances to reproduce. A GPU text path using
  "the same glyphs as the CPU path" is currently undefined, and any GPU text would
  introduce text that the CPU preview/export does not show (a parity regression).
- No font with a verified permissive licence is bundled, and none was added.

Not implemented (deliberately, rather than faking it). Design and blockers:

1. Decide product intent: should labels be drawn at all? If yes, enable a text
   path in the CPU rasterizer first (resvg `text` feature plus an explicit
   `fontdb` loaded from a bundled font, never system fonts, for determinism).
2. Bundle one font with a clearly permissive licence (e.g. an OFL-1.1 or Apache-2.0
   sans) with its licence text in the NOTICE corpus; this needs the licence/rights
   review the project already tracks.
3. GPU: read the same outlines and advances with `ttf-parser` (already in the lock
   file), flatten curves deterministically to polygons, and either triangulate them
   into the existing polygon command (even-odd/nonzero winding is already in the
   shader) or bake an SDF atlas; add `label` fixtures to the harness so CPU/GPU text
   error is measured like the rest. Shaping/kerning beyond simple advances would
   need rustybuzz on both paths.
4. Budget: the shader loops all commands per fragment; glyph polygons must count
   toward `MAX_FRAGMENT_WORK` or move to a tiled/instanced pipeline.
