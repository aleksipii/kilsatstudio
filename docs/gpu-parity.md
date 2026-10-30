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
group Multiply/Screen, layer Multiply, actor with masked background art, and (added
with the animated-mask and rig-part support below) animated hard/feathered polygon
and painted masks plus flat, textured and skinned custom rig parts.

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
| mask_painted | 62,14,14,0 | 2.71,0.63,0.63,0 | 8.03% | 100 / 8.0 / 35% |
| mask_feathered_polygon | 64,15,15,0 | 3.91,0.95,0.95,0 | 25.20% | 100 / 8.0 / 35% |
| mask_hard_polygon_motion | 39,9,9,0 | 1.87,0.46,0.46,0 | 2.64% | 60 / 4.0 / 6% |
| mask_feathered_polygon_motion | 62,14,14,0 | 4.18,0.98,0.98,0 | 30.84% | 100 / 8.0 / 35% |
| mask_painted_motion | 69,16,16,0 | 2.73,0.64,0.64,0 | 10.44% | 100 / 8.0 / 35% |
| rig_part_flat | 40,8,6,0 | 1.11,0.48,0.19,0 | 19.23% | 60 / 4.0 / 30% |
| rig_part_skinned_flat | 21,5,4,0 | 1.05,0.33,0.19,0 | 6.19% | 60 / 4.0 / 30% |
| rig_part_textured_rotated | 33,68,29,0 | 1.15,1.20,0.83,0 | 16.41% | 110 / 4.0 / 30% |
| rig_part_textured_subrect | 29,68,28,0 | 1.02,1.24,0.79,0 | 16.22% | 110 / 4.0 / 30% |
| rig_part_skinned_textured | 60,88,35,0 | 2.06,2.20,1.41,0 | 5.54% | 110 / 4.0 / 30% |
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

### Animated polygon and painted masks, textured and skinned rig parts

The 17 rows above are unchanged (re-run on Metal: identical numbers). The new rows
were measured on the same machine; two consecutive runs of the whole harness
produced byte-identical reports.

- Animated polygon/painted masks: the command builder calls
  `ClipShape::sample_motion(scene.time)` (the same sampler the SVG path uses) and
  transforms the polygon vertices, or the brush circle centres/radius and quad
  vertices, with the same `translate/rotate/scale about pivot` the SVG
  `svg_transform` expresses. Painted base coverage is not moved (as on the CPU).
  The feathered polygon's sigma is multiplied by the motion scale, because the
  CPU blur sits inside the transformed group. Errors match the static fixtures
  (the motion fixtures use scale 1.3, rotation 0.6 rad, translation (14,-8)
  at t = 0.5 s of a 0..1 s track), so the `UnsupportedFeature` for animated
  polygon/painted masks is removed. The transform is in f64 on the CPU; only the
  result is cast to f32.
- Textured rig parts: a flat polygon command (as before) plus one textured
  command (kind 20). The CPU placement (`uv` sub-rectangle over `rest_bounds`,
  carried by `EvaluatedTexture::transform`) is folded into an affine map from
  actor-local coordinates to image uv computed on the CPU in f64; the shader
  samples the decoded image (same bilinear sampler and storage as artwork
  images, deduplicated per asset, same pixel budget) and multiplies by the
  analytic polygon coverage. Fragment work is charged as polygon vertices + 5.
  A singular transform draws nothing, as on the CPU. Maximum single-pixel
  errors reach 68-88 of 255 where the polygon edge crosses the high-contrast
  gradient texture; RMS stays about 1-2 and the interior RMS is 0.04-0.40.
- Skinned parts: `motion-rig` already delivers the deformed polygon and the
  least-squares affine fit, so the GPU path draws exactly those; flat skinned
  fill matches within the same edge error as flat rigid parts. The texture is
  not warped per triangle on either path.
- Fixtures use a 8x8 gradient with partial alpha at 128x128 and a handful of
  poses; they are regression guards, not proof for arbitrary artwork or rigs.

Not established: other GPUs/drivers/OS versions, larger frames, perceptual
equivalence, performance.

### Rig screen brightness and glow (2026-10-28)

`rig-screen-brightness-glow` in `new_coverage_fixtures` (unaccepted
measurements, `gpu_parity_new_gpu_coverage_measurements`): a tilted 22x40 glass
showing the 8x8 gradient at brightness 1.4 with glow 0.9 (radius 18) over a
dark actor scene. Apple M1, Metal, macOS 27.0.1; two runs gave identical
numbers: max 58,57,37,0, RMS 1.317,1.293,1.204,0, 14.05% changed, edge RMS
4.37, interior RMS 0.20. The errors sit on the glass edge over the
high-contrast gradient, as for the other textured rig rows; inside the glow
ellipse (kind 24, the same smoothstep stops as the SVG gradient) the interior
RMS stays at 0.20. No tolerance was added from this single measurement.

### Soft-brush edge fix (2026-10-20)

The ignored hardware test `gpu_scene::tests::painted_pressure_soft_masks_real_gpu_interior_probes`
failed on this Mac (red 162 at x=45 where the CPU gives 179; the whole soft edge was
about 15 levels too dark). Cause: the GPU combined the 32 nested coverage bands with
`max(coverage, antialiasing * band level)`, which scales each band's own edge down,
while the CPU paints the bands in order. The shader now blends toward each band's level
in the same outer-to-inner order. The probe passes, and `mask_painted` / `mask_painted_motion`
were re-measured: RMS fell from 6.7 to 2.7 and the changed-pixel fraction from 9.4% /
11.5% to 8.0% / 10.4%, but the maximum single-channel error **rose** from 35/38 to
62/69 (still inside the 100 tolerance). The rows above are the new measurements; the
cause of the larger isolated maximum was not investigated. The CPU reference profile
for the probe is asserted by `cpu_soft_pressure_brush_profile_is_the_reference_for_the_gpu_probe`.

## Fonts / GPU text

Update (captions workstream): the CPU rasterizer now has resvg text support with a
font database holding only the bundled caption font, and draws captions (see
[captions](captions.md)). Actor labels are still not drawn (their `<text>` is
`display="none"`), so the label statements below remain true; the GPU path draws
no captions and no labels. The paragraph below describes the earlier state.

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

## Multiplane parallax (P05)

Fixtures `multiplane_pan_zoom` and `multiplane_atmosphere` (camera x 20, y −6,
zoom 1.5; Far 0.3 rectangle, Mid 0.6 ellipse, Near 1.0 rectangle; the second adds
atmosphere strength 0.6). CPU: `parallax::render_svg_with_multiplane` through
`media::rasterize`; GPU: `scene_commands::build_with_multiplane` on Metal.
Measured 2026-10-30 on Apple M1, macOS 27.0.1, with tolerance `TOL_SHAPES`
(40 / 4.0 / 30%). The 25 existing fixtures reproduced the table above unchanged
in the same run.

| Fixture | Max R,G,B,A | RMS R,G,B,A | Changed px | Edge RMS | Interior RMS |
| --- | --- | --- | --- | --- | --- |
| multiplane_pan_zoom | 14,30,24,0 | 1.61,1.05,0.72,0 | 3.41% | 4.22 | 0.00 |
| multiplane_atmosphere | 17,31,21,0 | 1.72,1.10,0.83,0 | 3.39% | 4.52 | 0.00 |

Interior RMS 0 means the per-layer placement and the hazed colours agree; the
differences are edge antialiasing, as for the other shape fixtures. Single
machine and driver; synthetic 128×128 fixtures only.
