# Model sheets, silhouette check and character styles

PRO task P08 ([task file](pro-studio/P08-model-sheet-tyylit.md)). Code:
`crates/motion-render/src/model_sheet.rs` (sheet, silhouette, style application),
`crates/motion-art/src/style.rs` (style presets) and
`apps/studio-desktop/src/model_sheet_panel.rs` (desktop panel).

## Model sheet

`motion_render::model_sheet::render_model_sheet(rig, &ModelSheetOptions { style, images })`
returns a `ModelSheet` with a self-contained SVG (1800 px wide) and the measured
data drawn on it. All content comes from the rig definition:

| Row | Source |
| --- | --- |
| Turnaround: front, side, back | `PreparedRig::evaluate_acting` with `FaceView::{Front, Side, Back}`, neutral emotion, rest pose; one shared scale and baseline |
| Silhouette | The front rest pose filled black, with the limb readings below it |
| Head-height ruler | Lines every head height from the top of the figure; total in heads |
| Expressions (6) | The five screenplay emotions (`EMOTION_NAMES`) and the rig's blink, cropped to the head joint's parts. An emotion the rig does not define is labelled `(ei omaa)` and shows the neutral face |
| Mouth shapes | Every viseme in the rig's `visemes.map`, in `VISEME_NAMES` order (13 for the stick characters) |
| Palette | Unique part, variant and outline colours in part order; one swatch per RGB; fully transparent (blink-hiding) colours are omitted |

The **head joint** is the joint of the mouth part, else of the first pupil, else
of the first expression part, else a joint named `head`. **Head height** is the
height of the visible parts on that joint at the front rest pose; **total
height** covers every visible part, including held props. A rig without a head
joint still gets a sheet with no ruler, expression or mouth rows.

The **turnaround uses the views the rig model has**: front, side (features
squashed toward +x) and back (features hidden). The rig format has no
three-quarter view, so the sheet does not invent one. Bodies of the rigid stick
rigs look the same in every view; only the face turns.

Output is deterministic: the same rig, style and images give the same bytes
(coordinates are printed with fixed precision; ordering comes from the
definition). Labels use the bundled caption font family, so the PNG rasterizer
(`motion_export::rasterize_svg_png`, the same renderer and font database as PNG
export) draws identical text on every machine. Textured parts need
`options.images`; a missing image is a `RigTexture` error.

## Silhouette check

`silhouette_check(rig, parts)` measures an evaluated pose: either
`rest_pose(rig, style)` or any `PreparedRig::evaluate_acting` result (parts are
matched to joints by id). It rasterizes the figure at 256 cells along its longer
side and, for every stick limb present in the rig (`STICK_LIMBS`: `arm_l`, `arm_r`,
`leg_l`, `leg_r`, each the subtree from its upper joint, so held props count
with the hand), reports:

- `visible_fraction`: the share of the limb's own area not covered by the rest of
  the figure (area change);
- `contour_gain`: how much the limb lengthens the silhouette contour, relative to
  its own contour (contour change);
- `warning`: `visible_fraction < 0.5` (`MIN_VISIBLE_LIMB_FRACTION`).

It is a warning only; nothing is blocked. All six stick characters have no
warnings at rest (limbs 74–99 % visible across styles). Folding Päsmä's right arm
across the body (upper arm +0.5 rad, forearm +2.5 rad) drops it to 38 % and warns.
Rigs without the stick limb joints get an empty report.

## Character style presets

`motion_art::style::CharacterStyle` serializes as `default`, `stick`,
`rubber_hose`, `flat_tv` or `geometric`. `CharacterStyle::preset()` gives the
measurable settings, and `motion_render::model_sheet::apply_character_style(rig, style)`
returns a restyled copy:

| Setting | How it is applied |
| --- | --- |
| `line_width_scale` | Parts with a polygon at least 2.5× longer than wide (limb lines) are scaled across their principal axis about the centroid. Parts with point-replacing variants are left alone |
| `outline` | `Keep`; `Off` removes all part and variant outlines; `On` gives every part one silhouette-mode outline of the preset colour and width |
| `joint_rounding` | Sets `smooth` on polygon parts, capped so a rounded polygon stays within the 64 vertices rig validation accepts (a 14-point limb gets at most 4) |
| `palette` | Colours are mapped by relative luminance: below 0.08 to ink, above 0.6 to paper, mid tones to the three accents in first-appearance order; alpha is kept |
| `squash_stretch` | Default amount recorded for P03's squash-and-stretch field. **No evaluator reads it yet** |

| Style | Lines | Outline | Rounding | Palette | Squash |
| --- | --- | --- | --- | --- | --- |
| `default` | as authored | as authored | as authored | as authored | 0 |
| `stick` | 0.75× | off | 2 | greys | 0 |
| `rubber_hose` | 1.6× | on, 2.0 | 8 (capped) | ink, cream, red/yellow/green | 0.35 |
| `flat_tv` | 1.25× | on, 3.0 | 3 | plum ink, warm skin, blue/orange/green | 0.1 |
| `geometric` | 1.0× | off | 0 (sharp) | navy, pale blue, teal/amber/indigo | 0 |

The directions are generic and original; no existing production's characters or
graphics are copied. `default` is the identity: the restyled rig equals the
input, and scene rendering is unchanged (the existing render golden tests still
pass). Joints, part ids, variants and the viseme, expression and eye maps are
kept, so a styled rig animates like the original.

**Not done in P08:** a MotionScript `style:` key on a character. It would go in
`motion-script` (a shared hot file coordinated by the PRO orchestrator); the
serde names above are ready for it. Until then a style only affects model sheets.

## Desktop panel

Edit tab → *Rig-nivelet, säännöt ja klipit* → *Model sheet ja hahmotyyli*: pick a
style, then *Tarkista siluetti ja mittasuhteet* (measure only) or *Tallenna model
sheet (SVG + PNG)* into an existing folder (`<rig id>-<style>-model-sheet.svg/.png`).
Rendering and PNG encoding run on a background thread, one job at a time. The
panel previews the style only; the project's rig is not modified.

## Tests

- `crates/motion-render/tests/model_sheet.rs`: every stick character in every
  style renders twice to identical bytes, with 6 expressions, 13 mouth shapes,
  3–5 head heights, opaque swatches and 4 limb readings; SHA-256 reference hashes
  of Päsmä's sheet in all five styles; `default` is the identity and every preset
  keeps rigs valid and structurally identical; preset settings reach the rig;
  rest poses do not warn while a folded arm does; limbless rigs give an empty
  report and a sheet without face rows.
- `crates/motion-art/src/style.rs`: style names round-trip through serde; the
  default preset changes nothing; luminance-role palette mapping keeps alpha.
- `apps/studio-desktop/src/model_sheet_panel.rs`: the panel job writes SVG and a
  PNG, measures proportions, and reports a missing folder as an error.

The reference hashes are SVG bytes. The PNGs were checked visually during
development but are not pinned, because raster output depends on the
rasterizer version. Regenerate the hashes with
`MODEL_SHEET_PRINT=1 cargo test -p motion-render --test model_sheet -- --nocapture`
after reviewing the SVGs written by `MODEL_SHEET_OUT=<dir>`.
