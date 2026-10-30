//! Deterministic character model sheets, silhouette checks and style presets
//! applied to rigs (docs/model-sheet.md).
//!
//! Everything on a sheet comes from the rig definition: the turnaround uses the
//! views the rig evaluates (front, side, back), the expression row uses the
//! rig's emotion variants and blink, the mouth row uses its viseme map, the
//! ruler measures the head joint's parts and the swatches list its colours. The
//! same rig and options always give the same SVG bytes.
use crate::{append_outline, append_rig_texture, escape, Error as RenderError};
use motion_art::style::{CharacterStyle, OutlineChoice};
use motion_art::{Color, ImageResource};
use motion_rig::{
    ActingInput, EvaluatedPart, FaceView, OutlineDefinition, OutlineMode, PreparedRig,
    RigDefinition, EMOTION_NAMES, STICK_LIMBS,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

#[derive(Debug, thiserror::Error)]
pub enum ModelSheetError {
    #[error("rig: {0}")]
    Rig(#[from] motion_rig::Error),
    #[error(transparent)]
    Render(#[from] RenderError),
    #[error("rig has no visible parts to draw")]
    Empty,
}

// ---------------------------------------------------------------------------
// Style presets applied to rigs
// ---------------------------------------------------------------------------

/// Parts at least this many times longer than wide count as limb lines whose
/// width the preset's `line_width_scale` changes.
pub const LIMB_ASPECT: f64 = 2.5;
/// Vertex limit of one rounded polygon accepted by rig validation.
pub const MAX_ROUNDED_POINTS: usize = 64;

/// A restyled copy of `rig`. `CharacterStyle::Default` returns it unchanged.
/// Joints, part ids, variants and the viseme/expression/eye maps are kept, so
/// a styled rig animates exactly like the original.
pub fn apply_character_style(rig: &RigDefinition, style: CharacterStyle) -> RigDefinition {
    let preset = style.preset();
    let mut out = rig.clone();
    if preset.line_width_scale != 1.0 {
        for part in &mut out.parts {
            // Variant polygons must keep matching the base; leave swapped parts.
            if part.ellipse.is_none()
                && part.points.len() >= 3
                && part.variants.values().all(|v| v.points.is_none())
            {
                scale_limb_width(&mut part.points, preset.line_width_scale);
            }
        }
    }
    match preset.outline {
        OutlineChoice::Keep => {}
        OutlineChoice::Off => {
            for part in &mut out.parts {
                part.outline = None;
                for variant in part.variants.values_mut() {
                    variant.outline = None;
                }
            }
        }
        OutlineChoice::On { color, width } => {
            for part in &mut out.parts {
                part.outline = Some(OutlineDefinition {
                    color: [color.r, color.g, color.b, color.a],
                    width,
                    mode: OutlineMode::Silhouette,
                });
                for variant in part.variants.values_mut() {
                    variant.outline = None;
                }
            }
        }
    }
    if let Some(samples) = preset.joint_rounding {
        for part in &mut out.parts {
            if part.ellipse.is_some() || part.points.is_empty() {
                continue;
            }
            // A rounded polygon holds at most MAX_ROUNDED_POINTS vertices
            // (rig validation), so dense polygons get fewer samples per corner.
            let control = part
                .variants
                .values()
                .filter(|v| v.smooth.is_none())
                .filter_map(|v| v.points.as_ref().map(Vec::len))
                .fold(part.points.len(), usize::max);
            let cap = (MAX_ROUNDED_POINTS / control) as u32;
            let samples = samples.min(cap);
            if samples == 0 || samples >= 2 {
                part.smooth = samples;
            }
        }
    }
    if let Some(palette) = preset.palette {
        let source = rig_colors(&out);
        let mapped = palette.map(&source);
        let table: BTreeMap<[u8; 4], [u8; 4]> = source
            .iter()
            .zip(&mapped)
            .map(|(a, b)| (rgba(*a), rgba(*b)))
            .collect();
        let swap = |c: &mut [u8; 4]| {
            if let Some(new) = table.get(c) {
                *c = *new;
            }
        };
        for part in &mut out.parts {
            swap(&mut part.color);
            if let Some(o) = &mut part.outline {
                swap(&mut o.color);
            }
            for variant in part.variants.values_mut() {
                if let Some(c) = &mut variant.color {
                    swap(c);
                }
                if let Some(o) = &mut variant.outline {
                    swap(&mut o.color);
                }
            }
        }
    }
    out
}

fn rgba(c: Color) -> [u8; 4] {
    [c.r, c.g, c.b, c.a]
}

/// Unique part, variant and outline colours in first-appearance (part) order.
fn rig_colors(rig: &RigDefinition) -> Vec<Color> {
    let mut seen = BTreeSet::new();
    let mut colors = Vec::new();
    let mut push = |c: [u8; 4]| {
        if seen.insert(c) {
            colors.push(Color {
                r: c[0],
                g: c[1],
                b: c[2],
                a: c[3],
            });
        }
    };
    for part in &rig.parts {
        push(part.color);
        if let Some(o) = part.outline {
            push(o.color);
        }
        for variant in part.variants.values() {
            if let Some(c) = variant.color {
                push(c);
            }
            if let Some(o) = variant.outline {
                push(o.color);
            }
        }
    }
    colors
}

/// Scale an elongated polygon across its principal axis about its centroid.
fn scale_limb_width(points: &mut [[f64; 2]], factor: f64) {
    let n = points.len() as f64;
    let (cx, cy) = points
        .iter()
        .fold((0.0, 0.0), |(x, y), p| (x + p[0] / n, y + p[1] / n));
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for p in points.iter() {
        let (dx, dy) = (p[0] - cx, p[1] - cy);
        sxx += dx * dx;
        syy += dy * dy;
        sxy += dx * dy;
    }
    let angle = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let (ux, uy) = (angle.cos(), angle.sin());
    let (nx, ny) = (-uy, ux);
    let extent = |ax: f64, ay: f64| {
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for p in points.iter() {
            let d = (p[0] - cx) * ax + (p[1] - cy) * ay;
            lo = lo.min(d);
            hi = hi.max(d);
        }
        hi - lo
    };
    let (length, width) = (extent(ux, uy), extent(nx, ny));
    if width <= 1e-9 || length < LIMB_ASPECT * width {
        return;
    }
    for p in points.iter_mut() {
        let (dx, dy) = (p[0] - cx, p[1] - cy);
        let along = dx * ux + dy * uy;
        let across = (dx * nx + dy * ny) * factor;
        *p = [cx + along * ux + across * nx, cy + along * uy + across * ny];
    }
}

// ---------------------------------------------------------------------------
// Silhouette check
// ---------------------------------------------------------------------------

/// Raster cells along the longer side of the silhouette's bounding box.
pub const SILHOUETTE_RESOLUTION: f64 = 256.0;
/// A limb warns when less than this fraction of its own area lies outside the
/// rest of the silhouette.
pub const MIN_VISIBLE_LIMB_FRACTION: f64 = 0.5;

/// How well one limb reads in the all-black silhouette.
#[derive(Debug, Clone, PartialEq)]
pub struct LimbSilhouette {
    /// `arm_l`, `arm_r`, `leg_l` or `leg_r` (the stick limb convention).
    pub limb: &'static str,
    /// Share of the limb's own area not covered by the rest of the figure.
    pub visible_fraction: f64,
    /// Change of the silhouette's contour length caused by the limb, relative
    /// to the limb's own contour. Near zero: the limb adds no readable shape.
    pub contour_gain: f64,
    /// `visible_fraction < MIN_VISIBLE_LIMB_FRACTION`. A warning, never a block.
    pub warning: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SilhouetteReport {
    /// Limbs of the stick convention that the rig has, arms first.
    pub limbs: Vec<LimbSilhouette>,
}

impl SilhouetteReport {
    pub fn warnings(&self) -> impl Iterator<Item = &LimbSilhouette> {
        self.limbs.iter().filter(|l| l.warning)
    }
}

/// Measure whether the limbs of an evaluated pose stand out from the body when
/// the whole figure is drawn black. `parts` are evaluated parts of `rig` (from
/// `PreparedRig::evaluate_acting` or a scene's actor pose); parts are matched to
/// joints by id, fully transparent parts are ignored. Rigs without the stick
/// limb joints give an empty report.
pub fn silhouette_check(rig: &RigDefinition, parts: &[EvaluatedPart]) -> SilhouetteReport {
    let part_joint: BTreeMap<&str, &str> = rig
        .parts
        .iter()
        .map(|p| (p.id.as_str(), p.joint.as_str()))
        .collect();
    let visible: Vec<&EvaluatedPart> = parts.iter().filter(|p| p.color[3] > 0).collect();
    let Some(grid) = Grid::around(&visible) else {
        return SilhouetteReport::default();
    };
    let full = grid.rasterize(visible.iter().copied());
    let full_perimeter = grid.perimeter(&full);
    let mut limbs = Vec::new();
    for chain in STICK_LIMBS {
        if !rig.joints.iter().any(|j| j.id == chain.upper) {
            continue;
        }
        let joints = subtree(rig, chain.upper);
        let in_limb = |p: &&EvaluatedPart| {
            part_joint
                .get(p.id.as_str())
                .is_some_and(|j| joints.contains(*j))
        };
        let limb = grid.rasterize(visible.iter().copied().filter(in_limb));
        let rest = grid.rasterize(visible.iter().copied().filter(|p| !in_limb(p)));
        let own = limb.iter().filter(|c| **c).count();
        if own == 0 {
            continue;
        }
        let exposed = limb.iter().zip(&rest).filter(|(l, r)| **l && !**r).count();
        let visible_fraction = exposed as f64 / own as f64;
        let contour_gain =
            (full_perimeter - grid.perimeter(&rest)) / grid.perimeter(&limb).max(f64::EPSILON);
        limbs.push(LimbSilhouette {
            limb: chain.name,
            visible_fraction,
            contour_gain,
            warning: visible_fraction < MIN_VISIBLE_LIMB_FRACTION,
        });
    }
    SilhouetteReport { limbs }
}

/// Ids of `root` and every joint below it.
fn subtree<'a>(rig: &'a RigDefinition, root: &'a str) -> BTreeSet<&'a str> {
    let mut set = BTreeSet::from([root]);
    // Joints are stored parent-before-child, so one pass finds every descendant.
    for joint in &rig.joints {
        if joint.parent.as_deref().is_some_and(|p| set.contains(p)) {
            set.insert(joint.id.as_str());
        }
    }
    set
}

/// Pixel-centre sampling grid for coverage masks.
struct Grid {
    x0: f64,
    y0: f64,
    cell: f64,
    width: usize,
    height: usize,
}

impl Grid {
    fn around(parts: &[&EvaluatedPart]) -> Option<Self> {
        let [x0, y0, x1, y1] = bounds(parts.iter().copied())?;
        let side = (x1 - x0).max(y1 - y0);
        if side <= 0.0 {
            return None;
        }
        let cell = side / SILHOUETTE_RESOLUTION;
        // Two empty cells of margin keep the outer contour inside the grid.
        Some(Self {
            x0: x0 - 2.0 * cell,
            y0: y0 - 2.0 * cell,
            cell,
            width: ((x1 - x0) / cell).ceil() as usize + 4,
            height: ((y1 - y0) / cell).ceil() as usize + 4,
        })
    }

    fn rasterize<'a>(&self, parts: impl Iterator<Item = &'a EvaluatedPart>) -> Vec<bool> {
        let mut mask = vec![false; self.width * self.height];
        let mut crossings = Vec::new();
        for part in parts {
            let points = &part.points;
            let (ymin, ymax) = points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                (lo.min(p.y), hi.max(p.y))
            });
            let row0 = ((ymin - self.y0) / self.cell - 0.5).ceil().max(0.0) as usize;
            let row1 = (((ymax - self.y0) / self.cell - 0.5).floor() as usize).min(self.height - 1);
            for row in row0..=row1 {
                let y = self.y0 + (row as f64 + 0.5) * self.cell;
                crossings.clear();
                for (i, a) in points.iter().enumerate() {
                    let b = points[(i + 1) % points.len()];
                    if (a.y <= y) != (b.y <= y) {
                        crossings.push(a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y));
                    }
                }
                crossings.sort_by(f64::total_cmp);
                for pair in crossings.as_chunks::<2>().0 {
                    let c0 = ((pair[0] - self.x0) / self.cell - 0.5).ceil().max(0.0) as usize;
                    let c1 = (((pair[1] - self.x0) / self.cell - 0.5).ceil().max(0.0) as usize)
                        .min(self.width);
                    for cell in &mut mask[row * self.width + c0.min(c1)..row * self.width + c1] {
                        *cell = true;
                    }
                }
            }
        }
        mask
    }

    /// Contour length: filled/empty cell edges times the cell size.
    fn perimeter(&self, mask: &[bool]) -> f64 {
        let at = |x: isize, y: isize| {
            x >= 0
                && y >= 0
                && (x as usize) < self.width
                && (y as usize) < self.height
                && mask[y as usize * self.width + x as usize]
        };
        let mut edges = 0usize;
        for y in 0..self.height as isize {
            for x in 0..self.width as isize {
                if at(x, y) {
                    edges += [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .filter(|(dx, dy)| !at(x + dx, y + dy))
                        .count();
                }
            }
        }
        edges as f64 * self.cell
    }
}

/// `[x0, y0, x1, y1]` of the visible parts' vertices.
fn bounds<'a>(parts: impl Iterator<Item = &'a EvaluatedPart>) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    for part in parts.filter(|p| p.color[3] > 0) {
        for p in &part.points {
            let r = b.get_or_insert([p.x, p.y, p.x, p.y]);
            r[0] = r[0].min(p.x);
            r[1] = r[1].min(p.y);
            r[2] = r[2].max(p.x);
            r[3] = r[3].max(p.y);
        }
    }
    b
}

fn union(a: Option<[f64; 4]>, b: Option<[f64; 4]>) -> Option<[f64; 4]> {
    match (a, b) {
        (Some(a), Some(b)) => Some([
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ]),
        (a, None) => a,
        (None, b) => b,
    }
}

// ---------------------------------------------------------------------------
// Model sheet
// ---------------------------------------------------------------------------

/// Character height measured in head heights at the front rest pose.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Proportions {
    /// Height of the head joint's visible parts.
    pub head_height: f64,
    /// Height of every visible part.
    pub total_height: f64,
    /// `total_height / head_height`.
    pub heads: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ModelSheetOptions<'a> {
    pub style: CharacterStyle,
    /// Project images for textured parts; required only when the rig has them.
    pub images: Option<&'a BTreeMap<String, ImageResource>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelSheet {
    pub svg: String,
    pub width: u32,
    pub height: u32,
    /// `None` when the rig has no identifiable head joint.
    pub proportions: Option<Proportions>,
    /// Swatch colours (one per RGB, opaque or translucent) in first-appearance order.
    pub palette: Vec<[u8; 4]>,
    /// Silhouette check of the front rest pose.
    pub silhouette: SilhouetteReport,
    /// Labels of the expression row, in order.
    pub expressions: Vec<String>,
    /// Viseme names of the mouth row, in order.
    pub mouths: Vec<&'static str>,
}

const SHEET_WIDTH: f64 = 1800.0;
const MARGIN: f64 = 40.0;
const RULER: f64 = 120.0;
const TURN_W: f64 = 380.0;
const TURN_H: f64 = 520.0;
const FACE: f64 = 260.0;
const MOUTH_W: f64 = 120.0;
const MOUTH_H: f64 = 150.0;
const GAP: f64 = 20.0;
const SWATCH: f64 = 70.0;
const INK: &str = "#24282d";
const FRAME: &str = "#c9ced3";
const WARN: &str = "#b3261e";

/// Joint holding the face: the mouth part's, else the pupils', else the first
/// expression part's, else a joint named `head`.
fn head_joint(rig: &RigDefinition) -> Option<&str> {
    let joint_of = |id: &str| {
        rig.parts
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.joint.as_str())
    };
    rig.visemes
        .as_ref()
        .and_then(|v| joint_of(&v.part))
        .or_else(|| {
            rig.eyes
                .as_ref()
                .and_then(|e| e.pupils.first())
                .and_then(|id| joint_of(id))
        })
        .or_else(|| {
            rig.expressions
                .values()
                .flatten()
                .next()
                .and_then(|e| joint_of(&e.part))
        })
        .or_else(|| {
            rig.joints
                .iter()
                .find(|j| j.id == "head")
                .map(|j| j.id.as_str())
        })
}

struct Sheet<'a> {
    svg: String,
    images: Option<&'a BTreeMap<String, ImageResource>>,
    embedded: u64,
    cells: usize,
}

impl Sheet<'_> {
    fn text(&mut self, x: f64, y: f64, size: f64, anchor: &str, color: &str, text: &str) {
        write!(
            self.svg,
            "<text x=\"{x:.3}\" y=\"{y:.3}\" font-family=\"{}\" font-size=\"{size}\" text-anchor=\"{anchor}\" fill=\"{color}\">{}</text>",
            crate::CAPTION_FONT_FAMILY,
            escape(text)
        )
        .unwrap();
    }

    fn frame(&mut self, x: f64, y: f64, w: f64, h: f64) {
        write!(
            self.svg,
            "<rect x=\"{x:.3}\" y=\"{y:.3}\" width=\"{w:.3}\" height=\"{h:.3}\" fill=\"none\" stroke=\"{FRAME}\" stroke-width=\"1\"/>"
        )
        .unwrap();
    }

    /// Draw `parts` scaled by `scale` and moved by `(tx, ty)`, clipped to the
    /// cell. Outline passes match the scene renderer: silhouette outlines first,
    /// then per part behind-outline, fill, texture and over-outline.
    #[allow(clippy::too_many_arguments)]
    fn figure(
        &mut self,
        parts: &[EvaluatedPart],
        cell: [f64; 4],
        scale: f64,
        tx: f64,
        ty: f64,
        black: bool,
    ) -> Result<(), RenderError> {
        let id = self.cells;
        self.cells += 1;
        write!(
            self.svg,
            "<defs><clipPath id=\"mscell{id}\"><rect x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\"/></clipPath></defs><g clip-path=\"url(#mscell{id})\"><g transform=\"translate({tx:.3} {ty:.3}) scale({scale:.6})\">",
            cell[0], cell[1], cell[2], cell[3]
        )
        .unwrap();
        if black {
            for part in parts.iter().filter(|p| p.color[3] > 0) {
                polygon(&mut self.svg, part, [0, 0, 0, 255]);
            }
        } else {
            for part in parts {
                if let Some(o) = part.outline.filter(|o| o.mode == OutlineMode::Silhouette) {
                    append_outline(&mut self.svg, part, o.color, o.width * 2.0);
                }
            }
            for (index, part) in parts.iter().enumerate() {
                if let Some(o) = part.outline.filter(|o| o.mode == OutlineMode::Behind) {
                    append_outline(&mut self.svg, part, o.color, o.width * 2.0);
                }
                polygon(&mut self.svg, part, part.color);
                append_rig_texture(
                    &mut self.svg,
                    part,
                    &format!("mstex{id}x{index}"),
                    self.images,
                    &mut self.embedded,
                )?;
                if let Some(o) = part.outline.filter(|o| o.mode == OutlineMode::Over) {
                    append_outline(&mut self.svg, part, o.color, o.width);
                }
            }
        }
        self.svg.push_str("</g></g>");
        Ok(())
    }
}

fn polygon(svg: &mut String, part: &EvaluatedPart, color: [u8; 4]) {
    svg.push_str("<polygon points=\"");
    for p in &part.points {
        write!(svg, "{:.3},{:.3} ", p.x, p.y).unwrap();
    }
    write!(
        svg,
        "\" fill=\"#{:02x}{:02x}{:02x}\" fill-opacity=\"{:.6}\"/>",
        color[0],
        color[1],
        color[2],
        f64::from(color[3]) / 255.0
    )
    .unwrap();
}

/// Fit `b` into a `w`×`h` cell at `(x, y)` with `pad` on every side, centred.
fn fit(b: [f64; 4], x: f64, y: f64, w: f64, h: f64, pad: f64) -> (f64, f64, f64) {
    let scale =
        ((w - 2.0 * pad) / (b[2] - b[0]).max(1e-9)).min((h - 2.0 * pad) / (b[3] - b[1]).max(1e-9));
    let tx = x + w / 2.0 - scale * (b[0] + b[2]) / 2.0;
    let ty = y + h / 2.0 - scale * (b[1] + b[3]) / 2.0;
    (scale, tx, ty)
}

/// Render the model sheet of `rig` restyled with `options.style`.
pub fn render_model_sheet(
    rig: &RigDefinition,
    options: &ModelSheetOptions<'_>,
) -> Result<ModelSheet, ModelSheetError> {
    let rig = apply_character_style(rig, options.style);
    let prepared = rig.prepare()?;
    let pose = |emotion: &str, blink: bool, view: FaceView| {
        prepared.evaluate_acting(
            &[],
            ActingInput {
                emotion,
                blink,
                view,
            },
        )
    };
    let views = [
        ("edestä", FaceView::Front),
        ("sivulta", FaceView::Side),
        ("takaa", FaceView::Back),
    ];
    let mut turnaround = Vec::new();
    for (label, view) in views {
        turnaround.push((label, pose("neutral", false, view)?.0));
    }
    let front = &turnaround[0].1;
    let body = turnaround
        .iter()
        .fold(None, |b, (_, parts)| union(b, bounds(parts.iter())))
        .ok_or(ModelSheetError::Empty)?;
    let front_bounds = bounds(front.iter()).ok_or(ModelSheetError::Empty)?;

    let head = head_joint(&rig);
    let part_joint: BTreeMap<&str, &str> = rig
        .parts
        .iter()
        .map(|p| (p.id.as_str(), p.joint.as_str()))
        .collect();
    let on_head =
        |p: &&EvaluatedPart| head.is_some() && part_joint.get(p.id.as_str()) == head.as_ref();
    let proportions = bounds(front.iter().filter(on_head)).map(|h| {
        let head_height = h[3] - h[1];
        let total_height = front_bounds[3] - front_bounds[1];
        Proportions {
            head_height,
            total_height,
            heads: total_height / head_height.max(1e-9),
        }
    });

    let mut sheet = Sheet {
        svg: String::new(),
        images: options.images,
        embedded: 0,
        cells: 0,
    };
    let title = format!("{} · model sheet · tyyli {}", rig.id, options.style.name());
    sheet.text(MARGIN, 52.0, 28.0, "start", INK, &title);
    let mut y = 80.0;

    // Turnaround, silhouette and head-height ruler.
    sheet.text(
        MARGIN,
        y + 24.0,
        20.0,
        "start",
        INK,
        "Kääntökuva ja mittasuhteet",
    );
    let top = y + 40.0;
    let (scale, _, _) = fit(body, 0.0, 0.0, TURN_W, TURN_H - 30.0, 30.0);
    let cell_x = |i: usize| MARGIN + RULER + i as f64 * (TURN_W + GAP);
    // Shared scale and baseline for every view, so heights compare directly.
    let ty = top + 30.0 - scale * body[1];
    let center = (body[0] + body[2]) / 2.0;
    let silhouette = silhouette_check(&rig, front);
    for (i, (label, parts)) in turnaround
        .iter()
        .map(|(l, p)| (*l, p))
        .chain(std::iter::once(("siluetti", front)))
        .enumerate()
    {
        let x = cell_x(i);
        sheet.frame(x, top, TURN_W, TURN_H);
        let tx = x + TURN_W / 2.0 - scale * center;
        sheet.figure(
            parts,
            [x, top, TURN_W, TURN_H - 30.0],
            scale,
            tx,
            ty,
            i == 3,
        )?;
        sheet.text(
            x + TURN_W / 2.0,
            top + TURN_H - 10.0,
            16.0,
            "middle",
            INK,
            label,
        );
    }
    // Limb readings sit below the silhouette cell, two per line.
    for (index, limb) in silhouette.limbs.iter().enumerate() {
        let text = format!(
            "{} {:.0} % näkyy{}",
            limb.limb,
            limb.visible_fraction * 100.0,
            if limb.warning { " · varoitus" } else { "" }
        );
        let color = if limb.warning { WARN } else { INK };
        let x = cell_x(3) + 10.0 + (index % 2) as f64 * (TURN_W / 2.0);
        let line_y = top + TURN_H + 20.0 + (index / 2) as f64 * 18.0;
        sheet.text(x, line_y, 14.0, "start", color, &text);
    }
    if let Some(p) = proportions {
        let head_top = ty + scale * front_bounds[1];
        let step = scale * p.head_height;
        let right = cell_x(2) + TURN_W;
        let mut k = 0;
        while f64::from(k) <= p.heads.floor() {
            let ly = head_top + step * f64::from(k);
            write!(
                sheet.svg,
                "<line x1=\"{:.3}\" y1=\"{ly:.3}\" x2=\"{right:.3}\" y2=\"{ly:.3}\" stroke=\"{FRAME}\" stroke-dasharray=\"6 4\"/>",
                MARGIN + 40.0
            )
            .unwrap();
            sheet.text(MARGIN + 30.0, ly + 5.0, 14.0, "end", INK, &k.to_string());
            k += 1;
        }
        let bottom = head_top + scale * p.total_height;
        write!(
            sheet.svg,
            "<line x1=\"{:.3}\" y1=\"{bottom:.3}\" x2=\"{right:.3}\" y2=\"{bottom:.3}\" stroke=\"{INK}\"/><line x1=\"{:.3}\" y1=\"{head_top:.3}\" x2=\"{:.3}\" y2=\"{bottom:.3}\" stroke=\"{INK}\" stroke-width=\"2\"/>",
            MARGIN + 40.0,
            MARGIN + 50.0,
            MARGIN + 50.0
        )
        .unwrap();
        sheet.text(
            MARGIN + 60.0,
            bottom + 18.0,
            14.0,
            "start",
            INK,
            &format!("{:.2} päänkorkeutta", p.heads),
        );
    }
    y = top + TURN_H + 20.0 + 18.0 * silhouette.limbs.len().div_ceil(2) as f64;

    // Expressions and mouth shapes, cropped to the head joint's parts.
    let mut expressions = Vec::new();
    let mut mouths = Vec::new();
    if head.is_some() {
        let mut faces = Vec::new();
        for emotion in EMOTION_NAMES {
            let label = if emotion == "neutral" || rig.expressions.contains_key(emotion) {
                emotion.to_string()
            } else {
                format!("{emotion} (ei omaa)")
            };
            faces.push((label, pose(emotion, false, FaceView::Front)?.0));
        }
        if rig.eyes.as_ref().is_some_and(|e| e.blink.is_some()) {
            faces.push(("blink".into(), pose("neutral", true, FaceView::Front)?.0));
        }
        let (front_parts, mouth_shapes) = pose("neutral", false, FaceView::Front)?;
        let crop = faces
            .iter()
            .fold(None, |b, (_, parts)| {
                union(b, bounds(parts.iter().filter(on_head)))
            })
            .ok_or(ModelSheetError::Empty)?;

        sheet.text(MARGIN, y + 24.0, 20.0, "start", INK, "Ilmeet");
        let row = y + 40.0;
        for (i, (label, parts)) in faces.iter().enumerate() {
            let x = MARGIN + i as f64 * (FACE + GAP);
            sheet.frame(x, row, FACE, FACE);
            let (s, tx, ty) = fit(crop, x, row, FACE, FACE - 24.0, 18.0);
            sheet.figure(parts, [x, row, FACE, FACE - 24.0], s, tx, ty, false)?;
            sheet.text(
                x + FACE / 2.0,
                row + FACE - 10.0,
                16.0,
                "middle",
                INK,
                label,
            );
            expressions.push(label.clone());
        }
        y = row + FACE + 20.0;

        if let Some(shapes) = mouth_shapes {
            sheet.text(MARGIN, y + 24.0, 20.0, "start", INK, "Suumuodot");
            let row = y + 40.0;
            for (i, (name, mouth)) in shapes.shapes.iter().enumerate() {
                let mut parts = front_parts.clone();
                parts[shapes.part] = mouth.clone();
                let x = MARGIN + i as f64 * (MOUTH_W + 10.0);
                sheet.frame(x, row, MOUTH_W, MOUTH_H);
                let (s, tx, ty) = fit(crop, x, row, MOUTH_W, MOUTH_H - 22.0, 8.0);
                sheet.figure(&parts, [x, row, MOUTH_W, MOUTH_H - 22.0], s, tx, ty, false)?;
                sheet.text(
                    x + MOUTH_W / 2.0,
                    row + MOUTH_H - 8.0,
                    13.0,
                    "middle",
                    INK,
                    name,
                );
                mouths.push(*name);
            }
            y = row + MOUTH_H + 20.0;
        }
    }

    // Palette swatches.
    // Fully transparent colours only hide parts (blinks); they are not swatches.
    let palette: Vec<[u8; 4]> = rig_colors(&rig)
        .into_iter()
        .filter(|c| c.a > 0)
        .map(rgba)
        .fold(Vec::new(), |mut list, c| {
            // One swatch per RGB; opacity variants of a colour share it.
            if !list.iter().any(|s: &[u8; 4]| s[..3] == c[..3]) {
                list.push(c);
            }
            list
        });
    sheet.text(MARGIN, y + 24.0, 20.0, "start", INK, "Paletti");
    let per_row = ((SHEET_WIDTH - 2.0 * MARGIN + GAP) / (SWATCH + GAP)).floor() as usize;
    let row = y + 40.0;
    for (i, c) in palette.iter().enumerate() {
        let x = MARGIN + (i % per_row) as f64 * (SWATCH + GAP);
        let sy = row + (i / per_row) as f64 * (SWATCH + 40.0);
        write!(
            sheet.svg,
            "<rect x=\"{x:.3}\" y=\"{sy:.3}\" width=\"{SWATCH}\" height=\"{SWATCH}\" fill=\"#{:02x}{:02x}{:02x}\" fill-opacity=\"{:.6}\" stroke=\"{FRAME}\"/>",
            c[0],
            c[1],
            c[2],
            f64::from(c[3]) / 255.0
        )
        .unwrap();
        let hex = format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
        sheet.text(
            x + SWATCH / 2.0,
            sy + SWATCH + 18.0,
            13.0,
            "middle",
            INK,
            &hex,
        );
    }
    let rows = palette.len().div_ceil(per_row).max(1);
    y = row + rows as f64 * (SWATCH + 40.0);

    let width = SHEET_WIDTH as u32;
    let height = (y + MARGIN).ceil() as u32;
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>{}</svg>\n",
        sheet.svg
    );
    Ok(ModelSheet {
        svg,
        width,
        height,
        proportions,
        palette,
        silhouette,
        expressions,
        mouths,
    })
}

/// The front neutral pose of `rig` (restyled), for silhouette checks of the
/// rest pose outside a scene.
pub fn rest_pose(
    rig: &RigDefinition,
    style: CharacterStyle,
) -> Result<(RigDefinition, Vec<EvaluatedPart>), ModelSheetError> {
    let rig = apply_character_style(rig, style);
    let prepared: PreparedRig = rig.prepare()?;
    let parts = prepared
        .evaluate_acting(
            &[],
            ActingInput {
                emotion: "neutral",
                blink: false,
                view: FaceView::Front,
            },
        )?
        .0;
    Ok((rig, parts))
}
