//! Versioned project-owned rigid character definitions. No external assets or inference.
use crate::{Affine2, Error, Joint, Rig, Transform2, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigDefinition {
    pub version: u32,
    pub id: String,
    pub joints: Vec<RigJointDefinition>,
    pub parts: Vec<RigidPartDefinition>,
    /// Optional mouth sprite swap driven by the actor's viseme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visemes: Option<VisemeMapDefinition>,
    /// Optional emotion name to part variants (brows, eyes, mouth corners).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub expressions: BTreeMap<String, Vec<ExpressionVariant>>,
    /// Optional pupil gaze and deterministic blink configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eyes: Option<EyesDefinition>,
    /// Optional per-joint rotation limits keyed by joint id. Evaluation clamps the
    /// joint's local rotation (rest rotation plus animated offset) into the range.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub joint_limits: BTreeMap<String, JointLimitDefinition>,
    /// Named display slots (phone, tablet, laptop, TV, billboard screens); see
    /// `crate::screen`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub screens: Vec<crate::screen::ScreenSlotDefinition>,
}
/// Inclusive bounds, in radians, for a joint's local rotation (rest + offset).
/// Both bounds are finite, within one full turn of zero, ordered, and contain
/// the joint's rest rotation, so the rest pose is always reachable.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JointLimitDefinition {
    pub min_rotation: f64,
    pub max_rotation: f64,
}
/// Largest magnitude accepted for a joint limit bound (one full turn).
pub const MAX_JOINT_LIMIT: f64 = std::f64::consts::TAU;
/// Viseme names accepted in `visemes.map`; they match `motion_lipsync::Viseme`'s
/// snake_case serialization (a motion-scene test keeps the two in step).
pub const VISEME_NAMES: [&str; 13] = [
    "rest",
    "closed",
    "teeth_lip",
    "tongue_teeth",
    "tongue_up",
    "sibilant",
    "sh",
    "r",
    "a",
    "e",
    "i",
    "o",
    "u",
];
/// Emotion names the screenplay accepts.
pub const EMOTION_NAMES: [&str; 5] = ["neutral", "happy", "sad", "surprised", "angry"];
pub const MAX_VARIANTS_PER_PART: usize = 16;
pub const MAX_EXPRESSION_ENTRIES: usize = 16;
pub const MAX_EYE_PARTS: usize = 8;
pub const MAX_GAZE_TRAVEL: f64 = 500.0;
/// `part` is the mouth part; `map` sends viseme names to variant ids of it.
/// Visemes without an entry (and actors with no viseme) keep the base shape.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisemeMapDefinition {
    pub part: String,
    pub map: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpressionVariant {
    pub part: String,
    pub variant: String,
}
/// `pupils` translate toward the gaze target by at most `travel` units in actor
/// space. `blink` swaps `variant` onto `parts` for a short, seeded interval.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EyesDefinition {
    pub pupils: Vec<String>,
    pub travel: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blink: Option<BlinkDefinition>,
}
/// At most one blink per `interval`-second window, starting at a position inside
/// the window derived only from the actor id and window index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlinkDefinition {
    pub parts: Vec<String>,
    pub variant: String,
    #[serde(default = "default_blink_interval")]
    pub interval: f64,
    #[serde(default = "default_blink_duration")]
    pub duration: f64,
}
fn default_blink_interval() -> f64 {
    4.0
}
fn default_blink_duration() -> f64 {
    0.12
}
/// Alternate appearance of a part; omitted fields keep the base part's values.
/// Alpha 0 hides the part (fill and outline). A skinned part's variant polygon
/// must keep its point count. `points` and `ellipse` are mutually exclusive; either
/// replaces the base shape (polygon or ellipse) entirely.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartVariantDefinition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<[f64; 2]>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<PartTextureDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ellipse: Option<EllipseDefinition>,
    /// Overrides the part's `smooth` (0 turns rounding off for this variant).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smooth: Option<u32>,
    /// Overrides the part's outline (width 0 removes it for this variant).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<OutlineDefinition>,
}
/// A joint-local axis-aligned ellipse, expanded at evaluation time into a fixed
/// `ELLIPSE_SEGMENTS`-vertex polygon (vertex `i` at angle `2*pi*i/ELLIPSE_SEGMENTS`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EllipseDefinition {
    #[serde(default)]
    pub cx: f64,
    #[serde(default)]
    pub cy: f64,
    pub rx: f64,
    pub ry: f64,
}
/// Where a part's outline is drawn relative to fills.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutlineMode {
    /// A stroke of `width` centred on the edge, drawn over this part's fill.
    #[default]
    Over,
    /// A stroke drawn just before this part's fill, so only an outer band of
    /// `width` shows; parts drawn later (a head over a body) cover it.
    Behind,
    /// Drawn in an actor-wide pass before any fill: overlapping silhouette parts
    /// share one outer outline with no seams where they join.
    Silhouette,
}
/// Bold cut-out outline. `width` is in evaluated rig (actor) units, before the
/// actor's own scale and the camera; joint scale does not change it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutlineDefinition {
    #[serde(default = "black")]
    pub color: [u8; 4],
    pub width: f64,
    #[serde(default, skip_serializing_if = "is_default_mode")]
    pub mode: OutlineMode,
}
fn black() -> [u8; 4] {
    [0, 0, 0, 255]
}
fn is_default_mode(mode: &OutlineMode) -> bool {
    *mode == OutlineMode::Over
}
fn is_zero(value: &u32) -> bool {
    *value == 0
}
/// Vertices of an expanded ellipse part.
pub const ELLIPSE_SEGMENTS: usize = 48;
/// Largest accepted `smooth` value (quadratic samples per control-polygon corner).
pub const MAX_SMOOTH: u32 = 16;
/// Upper bound on vertices of one evaluated part after ellipse expansion and
/// rounding; renderers accept evaluated polygons up to this size.
pub const MAX_EVALUATED_POINTS: usize = 512;
/// Bound on all expanded vertices of a rig (every part and variant), which bounds
/// the quadratic simplicity checks run once during preparation.
pub const MAX_RIG_EXPANDED_POINTS: usize = 16_384;
pub const MAX_OUTLINE_WIDTH: f64 = 64.0;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigJointDefinition {
    pub id: String,
    pub parent: Option<String>,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default = "one")]
    pub scale_x: f64,
    #[serde(default = "one")]
    pub scale_y: f64,
}
fn one() -> f64 {
    1.0
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigidPartDefinition {
    pub id: String,
    pub joint: String,
    /// Joint-local polygon vertices (the control polygon when `smooth` is set).
    /// Omitted only when `ellipse` gives the shape. Part order is draw order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<[f64; 2]>,
    pub color: [u8; 4],
    /// Optional project image drawn inside the (possibly deformed) polygon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<PartTextureDefinition>,
    /// Optional per-vertex skin weights: empty (rigid), or one entry per point,
    /// each with one or two `{joint, weight}` influences summing to one. Points
    /// stay in the attachment joint's rest-local frame (the bind pose).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skin: Vec<Vec<VertexWeight>>,
    /// Named alternate appearances (sprite swap), selected by visemes,
    /// expressions and blinks.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variants: BTreeMap<String, PartVariantDefinition>,
    /// Optional ellipse shape instead of `points` (not combinable with `skin`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ellipse: Option<EllipseDefinition>,
    /// Corner rounding: 0 keeps the polygon; `n` in 2..=16 replaces every corner
    /// by a quadratic Bezier from the incoming edge midpoint to the outgoing edge
    /// midpoint (control point: the vertex), sampled `n` times: a closed uniform
    /// quadratic B-spline inside the control polygon's convex hull. Applied after
    /// posing and skinning, so deformed parts stay smooth.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub smooth: u32,
    /// Optional bold outline drawn by the SVG renderer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<OutlineDefinition>,
}
/// Image binding for a part. `uv` is `[u0, v0, u1, v1]`, the image sub-rectangle
/// (fractions of the image, v downwards) stretched over the part's rest-pose
/// bounding box; the polygon clips the result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartTextureDefinition {
    pub asset: String,
    #[serde(default = "full_uv")]
    pub uv: [f64; 4],
}
fn full_uv() -> [f64; 4] {
    [0.0, 0.0, 1.0, 1.0]
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VertexWeight {
    pub joint: String,
    pub weight: f64,
}
pub const MAX_TEXTURED_PARTS: usize = 16;
pub const SKIN_WEIGHT_TOLERANCE: f64 = 1e-6;
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedPart {
    pub id: String,
    pub points: Vec<Vec2>,
    pub color: [u8; 4],
    pub texture: Option<EvaluatedTexture>,
    /// True when vertices were deformed by skin weights.
    pub skinned: bool,
    /// Outline to draw; `None` when none is defined, its width is zero, or the
    /// fill or outline colour is fully transparent.
    pub outline: Option<EvaluatedOutline>,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvaluatedOutline {
    pub color: [u8; 4],
    pub width: f64,
    pub mode: OutlineMode,
}
/// Texture placement: `uv` maps onto `rest_bounds` (`[x, y, w, h]`, joint-local
/// rest coordinates); `transform` carries those coordinates into actor space.
/// It is exact for rigid parts and the least-squares affine fit of the rest
/// vertices onto the deformed ones for skinned parts.
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedTexture {
    pub asset: String,
    pub uv: [f64; 4],
    pub rest_bounds: [f64; 4],
    pub transform: Affine2,
    /// How the image's own aspect ratio meets `rest_bounds`; renderers resolve
    /// it with `EvaluatedTexture::placement` once the image size is known.
    pub fit: TextureFit,
    /// Multiplier on the image's colour channels (clamped to white); 1 draws
    /// the image unchanged. Screen slots use it for display brightness.
    pub brightness: f64,
    /// Soft light around the part, tinted by the image's mean colour; drawn
    /// behind the actor so it lights what is around it. Screen slots only.
    pub glow: Option<TextureGlow>,
}
/// A soft, screen-blended halo around a textured part: an ellipse centred on
/// `rest_bounds`, reaching `radius` rest units beyond it on every side, with a
/// smoothstep falloff from the centre. Its colour is the image's mean colour
/// normalised to full brightness, its opacity `strength x brightness x mean
/// luma` (clamped to 1), so dark pictures glow little and black none.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextureGlow {
    pub strength: f64,
    pub radius: f64,
}
impl TextureGlow {
    /// Ellipse `[cx, cy, rx, ry]` in rest coordinates around `rest_bounds`.
    pub fn ellipse(&self, rest_bounds: [f64; 4]) -> [f64; 4] {
        let [x, y, w, h] = rest_bounds;
        [
            x + w / 2.0,
            y + h / 2.0,
            w / 2.0 + self.radius,
            h / 2.0 + self.radius,
        ]
    }
    /// Straight sRGB colour and opacity of the glow for an image whose mean
    /// premultiplied colour is `mean` (0..1 per channel), or `None` when it
    /// would be invisible. Both renderers call this, so they agree exactly.
    pub fn paint(&self, mean: [f64; 3], brightness: f64) -> Option<([u8; 3], f64)> {
        let peak = mean.iter().copied().fold(0.0, f64::max);
        let luma = 0.2126 * mean[0] + 0.7152 * mean[1] + 0.0722 * mean[2];
        let alpha = (self.strength * brightness * luma).clamp(0.0, 1.0);
        if peak.is_nan() || peak <= 0.0 || alpha.is_nan() || alpha < 1.0 / 512.0 {
            return None;
        }
        let rgb = mean.map(|c| ((c / peak).clamp(0.0, 1.0) * 255.0).round() as u8);
        Some((rgb, alpha))
    }
}
/// Fitting of an image into its target rectangle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextureFit {
    /// Fill the rectangle exactly, distorting the image if aspects differ.
    #[default]
    Stretch,
    /// Fill the rectangle and crop the overflowing sides (centred).
    Cover,
    /// Show the whole image centred; uncovered bands show the part's fill.
    Contain,
}
impl EvaluatedTexture {
    /// `(rest_bounds, uv)` actually drawn for an image of `width` x `height`
    /// pixels. Both the SVG and the GPU renderer call this, so preview and
    /// export place images identically. Cover crops through `uv`; contain
    /// shrinks the rectangle.
    pub fn placement(&self, width: u32, height: u32) -> ([f64; 4], [f64; 4]) {
        let [x, y, w, h] = self.rest_bounds;
        let [u0, v0, u1, v1] = self.uv;
        // Aspect of the selected image region versus the target rectangle.
        let source = (f64::from(width.max(1)) * (u1 - u0)) / (f64::from(height.max(1)) * (v1 - v0));
        let target = w / h;
        if self.fit == TextureFit::Stretch || !source.is_finite() || !target.is_finite() {
            return (self.rest_bounds, self.uv);
        }
        let wider = source > target;
        match self.fit {
            TextureFit::Cover if wider => {
                let keep = (u1 - u0) * target / source;
                let u = u0 + (u1 - u0 - keep) / 2.0;
                (self.rest_bounds, [u, v0, u + keep, v1])
            }
            TextureFit::Cover => {
                let keep = (v1 - v0) * source / target;
                let v = v0 + (v1 - v0 - keep) / 2.0;
                (self.rest_bounds, [u0, v, u1, v + keep])
            }
            TextureFit::Contain if wider => {
                let height = w / source;
                ([x, y + (h - height) / 2.0, w, height], self.uv)
            }
            TextureFit::Contain => {
                let width = h * source;
                ([x + (w - width) / 2.0, y, width, h], self.uv)
            }
            TextureFit::Stretch => unreachable!("handled above"),
        }
    }
}
pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn on_segment(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> bool {
    cross(a, b, p) == 0.0
        && p[0] >= a[0].min(b[0])
        && p[0] <= a[0].max(b[0])
        && p[1] >= a[1].min(b[1])
        && p[1] <= a[1].max(b[1])
}
fn intersects(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let ab_c = cross(a, b, c);
    let ab_d = cross(a, b, d);
    let cd_a = cross(c, d, a);
    let cd_b = cross(c, d, b);
    let opposite = |x: f64, y: f64| (x < 0.0 && y > 0.0) || (x > 0.0 && y < 0.0);
    (opposite(ab_c, ab_d) && opposite(cd_a, cd_b))
        || on_segment(a, b, c)
        || on_segment(a, b, d)
        || on_segment(c, d, a)
        || on_segment(c, d, b)
}
// At most 64 vertices: bounded quadratic work during definition preparation,
// never a frame-time repair. Simple concave polygons retain either winding.
fn simple_polygon(points: &[[f64; 2]]) -> bool {
    let n = points.len();
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        let c = points[(i + 2) % n];
        if a == b {
            return false;
        }
        // Adjacent collinear edges may continue forward, but must not retrace.
        if cross(a, b, c) == 0.0
            && (a[0] - b[0]) * (c[0] - b[0]) + (a[1] - b[1]) * (c[1] - b[1]) > 0.0
        {
            return false;
        }
        for j in (i + 1)..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if intersects(a, b, points[j], points[(j + 1) % n]) {
                return false;
            }
        }
    }
    true
}
fn validate_texture(t: &PartTextureDefinition) -> Result<(), Error> {
    let [u0, v0, u1, v1] = t.uv;
    if t.asset.trim().is_empty()
        || t.asset.len() > 256
        || t.asset.chars().any(char::is_control)
        || t.uv
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        || u1 - u0 < 1e-3
        || v1 - v0 < 1e-3
    {
        return Err(Error::InvalidDefinition);
    }
    Ok(())
}
fn validate_polygon(points: &[[f64; 2]]) -> Result<(), Error> {
    if !(3..=64).contains(&points.len())
        || points
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
        || !simple_polygon(points)
    {
        return Err(Error::InvalidDefinition);
    }
    let area: f64 = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum();
    if !area.is_finite() || area.abs() < 1e-9 {
        return Err(Error::InvalidDefinition);
    }
    Ok(())
}
fn validate_ellipse(e: &EllipseDefinition) -> Result<(), Error> {
    let centre = [e.cx, e.cy]
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1_000_000.0);
    let radii = [e.rx, e.ry]
        .iter()
        .all(|v| v.is_finite() && (1e-3..=1_000_000.0).contains(v));
    if centre && radii {
        Ok(())
    } else {
        Err(Error::InvalidDefinition)
    }
}
fn validate_outline(o: &OutlineDefinition) -> Result<(), Error> {
    if o.width.is_finite() && (0.0..=MAX_OUTLINE_WIDTH).contains(&o.width) {
        Ok(())
    } else {
        Err(Error::InvalidDefinition)
    }
}
fn validate_smooth(samples: u32) -> Result<(), Error> {
    if samples == 0 || (2..=MAX_SMOOTH).contains(&samples) {
        Ok(())
    } else {
        Err(Error::InvalidDefinition)
    }
}
/// Fixed-segment polygon of an ellipse, starting at angle 0 (+x) with increasing angle.
fn ellipse_points(e: &EllipseDefinition) -> Vec<[f64; 2]> {
    (0..ELLIPSE_SEGMENTS)
        .map(|i| {
            let angle = std::f64::consts::TAU * i as f64 / ELLIPSE_SEGMENTS as f64;
            let (sin, cos) = angle.sin_cos();
            [e.cx + e.rx * cos, e.cy + e.ry * sin]
        })
        .collect()
}
/// Replace each corner by a quadratic Bezier between adjacent edge midpoints with
/// the vertex as control point, sampled `samples` times (`t = k / samples`,
/// `k < samples`). Zero returns the polygon unchanged. Bounded: callers validated
/// `points.len() * samples <= MAX_EVALUATED_POINTS`.
pub fn round_corners(points: &[Vec2], samples: u32) -> Vec<Vec2> {
    if samples == 0 {
        return points.to_vec();
    }
    let n = points.len();
    let mut out = Vec::with_capacity(n * samples as usize);
    for i in 0..n {
        let (prev, p, next) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n]);
        let a = Vec2::new((prev.x + p.x) * 0.5, (prev.y + p.y) * 0.5);
        let b = Vec2::new((p.x + next.x) * 0.5, (p.y + next.y) * 0.5);
        for k in 0..samples {
            let t = f64::from(k) / f64::from(samples);
            let u = 1.0 - t;
            let (wa, wp, wb) = (u * u, 2.0 * u * t, t * t);
            out.push(Vec2::new(
                wa * a.x + wp * p.x + wb * b.x,
                wa * a.y + wp * p.y + wb * b.y,
            ));
        }
    }
    out
}
/// Check one resolved shape (control polygon plus rounding) and return how many
/// vertices it expands to. The rounded polygon must remain simple.
fn validate_expanded(control: &[[f64; 2]], samples: u32) -> Result<usize, Error> {
    validate_smooth(samples)?;
    let count = control.len() * (samples.max(1) as usize);
    if count > MAX_EVALUATED_POINTS {
        return Err(Error::InvalidDefinition);
    }
    if samples > 0 {
        let points: Vec<Vec2> = control.iter().map(|p| Vec2::new(p[0], p[1])).collect();
        let rounded: Vec<[f64; 2]> = round_corners(&points, samples)
            .iter()
            .map(|p| [p.x, p.y])
            .collect();
        validate_polygon(&rounded)?;
    }
    Ok(count)
}
/// The part's control polygon: its points, or its expanded ellipse.
fn part_control(part: &RigidPartDefinition) -> Result<Vec<[f64; 2]>, Error> {
    match (&part.ellipse, part.points.is_empty()) {
        (Some(e), true) => {
            validate_ellipse(e)?;
            Ok(ellipse_points(e))
        }
        (None, false) => Ok(part.points.clone()),
        _ => Err(Error::InvalidDefinition),
    }
}
/// Validate variants and return the vertices their shapes expand to.
fn validate_variants(part: &RigidPartDefinition, base: &[[f64; 2]]) -> Result<usize, Error> {
    if part.variants.len() > MAX_VARIANTS_PER_PART {
        return Err(Error::InvalidDefinition);
    }
    let mut expanded = 0;
    for (id, variant) in &part.variants {
        if !valid_id(id) {
            return Err(Error::InvalidDefinition);
        }
        let control = match (&variant.points, &variant.ellipse) {
            (Some(_), Some(_)) => return Err(Error::InvalidDefinition),
            (Some(points), None) => {
                validate_polygon(points)?;
                if !part.skin.is_empty() && points.len() != part.points.len() {
                    return Err(Error::InvalidDefinition);
                }
                points.clone()
            }
            (None, Some(e)) => {
                if !part.skin.is_empty() {
                    return Err(Error::InvalidDefinition);
                }
                validate_ellipse(e)?;
                ellipse_points(e)
            }
            (None, None) => base.to_vec(),
        };
        if let Some(t) = &variant.texture {
            validate_texture(t)?;
        }
        if let Some(o) = &variant.outline {
            validate_outline(o)?;
        }
        expanded += validate_expanded(&control, variant.smooth.unwrap_or(part.smooth))?;
    }
    Ok(expanded)
}
fn validate_skin_and_texture(
    part: &RigidPartDefinition,
    ids: &BTreeMap<&str, usize>,
) -> Result<(), Error> {
    if let Some(t) = &part.texture {
        validate_texture(t)?;
    }
    if part.skin.is_empty() {
        return Ok(());
    }
    if part.skin.len() != part.points.len() {
        return Err(Error::InvalidDefinition);
    }
    for influences in &part.skin {
        let sum: f64 = influences.iter().map(|w| w.weight).sum();
        if !(1..=2).contains(&influences.len())
            || influences.iter().any(|w| {
                !w.weight.is_finite()
                    || !(0.0..=1.0).contains(&w.weight)
                    || !ids.contains_key(w.joint.as_str())
            })
            || (influences.len() == 2 && influences[0].joint == influences[1].joint)
            || (sum - 1.0).abs() > SKIN_WEIGHT_TOLERANCE
        {
            return Err(Error::InvalidDefinition);
        }
    }
    Ok(())
}
/// Least-squares affine mapping `src` onto `dst` (centered for conditioning).
fn fit_affine(src: &[[f64; 2]], dst: &[Vec2]) -> Option<Affine2> {
    let n = src.len() as f64;
    let (mx, my) = (
        src.iter().map(|p| p[0]).sum::<f64>() / n,
        src.iter().map(|p| p[1]).sum::<f64>() / n,
    );
    let (dx, dy) = (
        dst.iter().map(|p| p.x).sum::<f64>() / n,
        dst.iter().map(|p| p.y).sum::<f64>() / n,
    );
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    let (mut bx0, mut bx1, mut by0, mut by1) = (0.0, 0.0, 0.0, 0.0);
    for (s, d) in src.iter().zip(dst) {
        let (x, y) = (s[0] - mx, s[1] - my);
        sxx += x * x;
        sxy += x * y;
        syy += y * y;
        bx0 += x * (d.x - dx);
        bx1 += y * (d.x - dx);
        by0 += x * (d.y - dy);
        by1 += y * (d.y - dy);
    }
    let det = sxx * syy - sxy * sxy;
    if !det.is_finite() || det.abs() < 1e-12 * (sxx * syy).max(1e-300) {
        return None;
    }
    let solve = |b0: f64, b1: f64| ((b0 * syy - b1 * sxy) / det, (sxx * b1 - sxy * b0) / det);
    let (a, c) = solve(bx0, bx1);
    let (b, d) = solve(by0, by1);
    Some(Affine2 {
        a,
        b,
        c,
        d,
        tx: dx - a * mx - c * my,
        ty: dy - b * mx - d * my,
    })
}
impl JointLimitDefinition {
    /// Valid bounds that contain `rest`.
    pub fn contains_rest(self, rest: f64) -> bool {
        [self.min_rotation, self.max_rotation]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= MAX_JOINT_LIMIT)
            && self.min_rotation <= self.max_rotation
            && (self.min_rotation..=self.max_rotation).contains(&rest)
    }
    /// Clamp a local rotation into the bounds; non-finite input stays non-finite
    /// so pose validation still rejects it.
    pub fn clamp(self, rotation: f64) -> f64 {
        if rotation.is_finite() {
            rotation.clamp(self.min_rotation, self.max_rotation)
        } else {
            rotation
        }
    }
}
impl RigDefinition {
    fn build(&self) -> Result<(Rig, BTreeMap<&str, usize>), Error> {
        if self.version != 1
            || !valid_id(&self.id)
            || self.parts.is_empty()
            || self.parts.len() > 256
        {
            return Err(Error::InvalidDefinition);
        }
        let mut ids = BTreeMap::new();
        let mut joints = Vec::with_capacity(self.joints.len().min(256));
        if self.joints.is_empty() || self.joints.len() > 256 {
            return Err(Error::InvalidDefinition);
        }
        for joint in &self.joints {
            if !valid_id(&joint.id) || ids.contains_key(joint.id.as_str()) {
                return Err(Error::InvalidDefinition);
            }
            let parent = match &joint.parent {
                Some(id) => Some(*ids.get(id.as_str()).ok_or(Error::InvalidDefinition)?),
                None => None,
            };
            ids.insert(joint.id.as_str(), joints.len());
            joints.push(Joint {
                name: joint.id.clone(),
                parent,
                rest: Transform2 {
                    translation: Vec2::new(joint.x, joint.y),
                    rotation: joint.rotation,
                    scale: Vec2::new(joint.scale_x, joint.scale_y),
                },
            });
        }
        for (id, limit) in &self.joint_limits {
            let joint = ids
                .get(id.as_str())
                .map(|&i| &joints[i])
                .ok_or(Error::InvalidDefinition)?;
            if !limit.contains_rest(joint.rest.rotation) {
                return Err(Error::InvalidDefinition);
            }
        }
        let rig = Rig::new(joints).map_err(|_| Error::InvalidDefinition)?;
        let mut parts = BTreeSet::new();
        let mut expanded = 0usize;
        for part in &self.parts {
            if !valid_id(&part.id)
                || !parts.insert(&part.id)
                || !ids.contains_key(part.joint.as_str())
            {
                return Err(Error::InvalidDefinition);
            }
            if part.ellipse.is_some() && !part.skin.is_empty() {
                return Err(Error::InvalidDefinition);
            }
            let control = part_control(part)?;
            if !(3..=64).contains(&control.len())
                || control
                    .iter()
                    .flatten()
                    .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
            {
                return Err(Error::InvalidDefinition);
            }
            // Reject crossings and retraced edges so nonzero and even-odd renderers
            // agree on the accepted simple polygons, including concave outlines.
            if !simple_polygon(&control) {
                return Err(Error::InvalidDefinition);
            }
            validate_skin_and_texture(part, &ids)?;
            if let Some(o) = &part.outline {
                validate_outline(o)?;
            }
            expanded += validate_expanded(&control, part.smooth)?;
            expanded += validate_variants(part, &control)?;
            if expanded > MAX_RIG_EXPANDED_POINTS {
                return Err(Error::InvalidDefinition);
            }
            let area: f64 = control
                .iter()
                .zip(control.iter().cycle().skip(1))
                .take(control.len())
                .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
                .sum();
            if !area.is_finite() || area.abs() < 1e-9 {
                return Err(Error::InvalidDefinition);
            }
        }
        if self
            .parts
            .iter()
            .filter(|p| p.texture.is_some() || p.variants.values().any(|v| v.texture.is_some()))
            .count()
            > MAX_TEXTURED_PARTS
        {
            return Err(Error::InvalidDefinition);
        }
        self.validate_acting()?;
        crate::screen::validate_slots(self, &ids)?;
        Ok((rig, ids))
    }
    fn part(&self, id: &str) -> Result<&RigidPartDefinition, Error> {
        self.parts
            .iter()
            .find(|p| p.id == id)
            .ok_or(Error::InvalidDefinition)
    }
    fn variant_ref(&self, part: &str, variant: &str) -> Result<(), Error> {
        self.part(part)?
            .variants
            .contains_key(variant)
            .then_some(())
            .ok_or(Error::InvalidDefinition)
    }
    /// Variant ids exist, viseme/emotion names are known, counts are bounded and
    /// numbers are finite. Part and variant shapes were validated already.
    fn validate_acting(&self) -> Result<(), Error> {
        if let Some(v) = &self.visemes {
            if v.map.is_empty() || v.map.len() > VISEME_NAMES.len() {
                return Err(Error::InvalidDefinition);
            }
            for (name, variant) in &v.map {
                if !VISEME_NAMES.contains(&name.as_str()) {
                    return Err(Error::InvalidDefinition);
                }
                self.variant_ref(&v.part, variant)?;
            }
        }
        if self.expressions.len() > EMOTION_NAMES.len() {
            return Err(Error::InvalidDefinition);
        }
        for (emotion, entries) in &self.expressions {
            if !EMOTION_NAMES.contains(&emotion.as_str())
                || entries.is_empty()
                || entries.len() > MAX_EXPRESSION_ENTRIES
            {
                return Err(Error::InvalidDefinition);
            }
            let mut seen = BTreeSet::new();
            for entry in entries {
                if !seen.insert(entry.part.as_str()) {
                    return Err(Error::InvalidDefinition);
                }
                self.variant_ref(&entry.part, &entry.variant)?;
            }
        }
        if let Some(eyes) = &self.eyes {
            let unique = |ids: &[String]| {
                !ids.is_empty()
                    && ids.len() <= MAX_EYE_PARTS
                    && ids.iter().collect::<BTreeSet<_>>().len() == ids.len()
            };
            if !unique(&eyes.pupils)
                || !eyes.travel.is_finite()
                || !(0.0..=MAX_GAZE_TRAVEL).contains(&eyes.travel)
            {
                return Err(Error::InvalidDefinition);
            }
            for pupil in &eyes.pupils {
                self.part(pupil)?;
            }
            if let Some(blink) = &eyes.blink {
                if !unique(&blink.parts)
                    || !blink.interval.is_finite()
                    || !(1.0..=30.0).contains(&blink.interval)
                    || !blink.duration.is_finite()
                    || !(0.04..=0.5).contains(&blink.duration)
                {
                    return Err(Error::InvalidDefinition);
                }
                for part in &blink.parts {
                    self.variant_ref(part, &blink.variant)?;
                }
            }
        }
        Ok(())
    }
    /// Project image assets this rig's textures need.
    pub fn image_assets(&self) -> impl Iterator<Item = &str> {
        self.parts.iter().flat_map(|p| {
            p.texture
                .iter()
                .chain(p.variants.values().filter_map(|v| v.texture.as_ref()))
                .map(|t| t.asset.as_str())
        })
    }
    pub fn validate(&self) -> Result<(), Error> {
        self.evaluate_parts().map(|_| ())
    }
    pub fn evaluate_parts(&self) -> Result<Vec<EvaluatedPart>, Error> {
        self.prepare()?.evaluate_parts(&[])
    }
    /// Validate once so repeated posed evaluation avoids rebuilding the hierarchy.
    pub fn prepare(&self) -> Result<PreparedRig, Error> {
        let (rig, ids) = self.build()?;
        let ids: BTreeMap<String, usize> =
            ids.into_iter().map(|(id, i)| (id.to_owned(), i)).collect();
        let rest_world = rig
            .evaluate(&rig.rest_pose())
            .map_err(|_| Error::InvalidDefinition)?;
        let parts: Vec<PreparedPart> = self
            .parts
            .iter()
            .map(|part| {
                let joint = ids[part.joint.as_str()];
                let skin: Vec<Vec<(usize, f64, Affine2)>> = part
                    .skin
                    .iter()
                    .map(|influences| {
                        influences
                            .iter()
                            .map(|w| {
                                let j = ids[w.joint.as_str()];
                                // Bind matrix: attachment-local -> influence-local at rest.
                                let bind = rest_world[j].inverse().compose(rest_world[joint]);
                                (j, w.weight, bind)
                            })
                            .collect()
                    })
                    .collect();
                // Resolve ellipses to their fixed polygons once, so posing only
                // transforms control points (validated by `build`).
                let mut definition = part.clone();
                if let Some(e) = definition.ellipse.take() {
                    definition.points = ellipse_points(&e);
                }
                for variant in definition.variants.values_mut() {
                    if let Some(e) = variant.ellipse.take() {
                        variant.points = Some(ellipse_points(&e));
                    }
                }
                PreparedPart {
                    joint,
                    definition,
                    skin,
                }
            })
            .collect();
        let mut acting = self.prepare_acting();
        let joints: Vec<_> = acting.features.iter().map(|i| parts[*i].joint).collect();
        if joints
            .first()
            .is_some_and(|j| joints.iter().all(|k| k == j))
        {
            let xs = acting
                .features
                .iter()
                .flat_map(|i| parts[*i].definition.points.iter().map(|p| p[0]));
            let (lo, hi) = xs.fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
            if hi > lo {
                acting.side = Some(((lo + hi) / 2.0, (hi - lo) / 2.0));
            }
        }
        let limits = rig
            .joints()
            .iter()
            .map(|j| self.joint_limits.get(&j.name).copied())
            .collect();
        let screens = crate::screen::prepare_slots(self);
        let prepared = PreparedRig {
            rig,
            ids,
            parts,
            acting,
            limits,
            screens,
        };
        // Skinning is the identity at rest; still verify the outline stays simple.
        let rest = prepared.evaluate_parts(&[])?;
        for (part, evaluated) in prepared.parts.iter().zip(&rest) {
            if part.skin.is_empty() {
                continue;
            }
            let inverse = rest_world[part.joint].inverse();
            let local: Vec<[f64; 2]> = evaluated
                .points
                .iter()
                .map(|p| {
                    let q = inverse.point(*p);
                    [q.x, q.y]
                })
                .collect();
            if !simple_polygon(&local) {
                return Err(Error::InvalidDefinition);
            }
        }
        Ok(prepared)
    }
    fn part_index(&self, id: &str) -> usize {
        self.parts.iter().position(|p| p.id == id).unwrap_or(0)
    }
    fn variant_index(&self, part: &str, variant: &str) -> usize {
        self.parts[self.part_index(part)]
            .variants
            .keys()
            .position(|k| k == variant)
            .unwrap_or(0)
    }
    /// Resolve acting ids to indices. Only called after `build` validated them.
    fn prepare_acting(&self) -> PreparedActing {
        let mouth = self.visemes.as_ref().map(|v| {
            let mut table = [None; VISEME_NAMES.len()];
            for (name, variant) in &v.map {
                if let Some(slot) = VISEME_NAMES.iter().position(|n| n == name) {
                    table[slot] = Some(self.variant_index(&v.part, variant));
                }
            }
            (self.part_index(&v.part), table)
        });
        let expressions: BTreeMap<String, Vec<(usize, usize)>> = self
            .expressions
            .iter()
            .map(|(emotion, entries)| {
                let picks = entries
                    .iter()
                    .map(|e| {
                        (
                            self.part_index(&e.part),
                            self.variant_index(&e.part, &e.variant),
                        )
                    })
                    .collect();
                (emotion.clone(), picks)
            })
            .collect();
        let (pupils, travel, blink) = match &self.eyes {
            None => (Vec::new(), 0.0, None),
            Some(eyes) => (
                eyes.pupils.iter().map(|p| self.part_index(p)).collect(),
                eyes.travel,
                eyes.blink.as_ref().map(|b| PreparedBlink {
                    picks: b
                        .parts
                        .iter()
                        .map(|p| (self.part_index(p), self.variant_index(p, &b.variant)))
                        .collect(),
                    interval: b.interval,
                    duration: b.duration,
                }),
            ),
        };
        let mut features: Vec<usize> = mouth
            .iter()
            .map(|(part, _)| *part)
            .chain(expressions.values().flatten().map(|(part, _)| *part))
            .chain(pupils.iter().copied())
            .chain(
                blink
                    .iter()
                    .flat_map(|b| b.picks.iter().map(|(part, _)| *part)),
            )
            .collect();
        features.sort_unstable();
        features.dedup();
        PreparedActing {
            mouth,
            expressions,
            pupils,
            travel,
            blink,
            features,
            side: None,
        }
    }
}
#[derive(Debug, Clone, Default)]
struct PreparedActing {
    /// Mouth part index and, per `VISEME_NAMES` slot, the variant index.
    mouth: Option<(usize, [Option<usize>; VISEME_NAMES.len()])>,
    expressions: BTreeMap<String, Vec<(usize, usize)>>,
    pupils: Vec<usize>,
    travel: f64,
    blink: Option<PreparedBlink>,
    /// Facial feature parts: the mouth, expression, pupil and blink parts.
    features: Vec<usize>,
    /// Joint-local `(center x, half width)` of the features at rest, when they
    /// share one joint; used to turn the face toward +x in the side view.
    side: Option<(f64, f64)>,
}
#[derive(Debug, Clone)]
struct PreparedBlink {
    picks: Vec<(usize, usize)>,
    interval: f64,
    duration: f64,
}
/// Inputs that select variants for one sample (emotion and blink are known at
/// scene evaluation time; the viseme arrives later through the mouth shapes).
#[derive(Debug, Clone, Copy)]
pub struct ActingInput<'a> {
    pub emotion: &'a str,
    pub blink: bool,
    /// Which side of the head faces the viewer.
    pub view: FaceView,
}
/// Which side of a character faces the viewer. The side view turns toward +x;
/// a mirrored actor (negative x scale) therefore faces -x.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum FaceView {
    #[default]
    Front,
    Side,
    Back,
}
/// Fraction of the feature spread kept in the side view; the rest becomes the
/// shift toward the facing side.
const SIDE_SQUASH: f64 = 0.55;
/// Evaluated mouth alternatives for this pose: `(viseme name, part)` for each
/// mapped viseme, replacing `parts[part]` when that viseme is spoken.
#[derive(Debug, Clone, PartialEq)]
pub struct MouthShapes {
    pub part: usize,
    pub shapes: Vec<(&'static str, EvaluatedPart)>,
}
/// FNV-1a over the seed and window index: stable across platforms and runs.
fn blink_hash(seed: &str, window: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in seed.bytes().chain(window.to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}
/// A validated rig hierarchy that can be posed with per-joint offsets.
#[derive(Debug, Clone)]
pub struct PreparedRig {
    rig: Rig,
    ids: BTreeMap<String, usize>,
    parts: Vec<PreparedPart>,
    acting: PreparedActing,
    /// Rotation limits per joint index (validated by `build`).
    limits: Vec<Option<JointLimitDefinition>>,
    screens: Vec<crate::screen::PreparedScreen>,
}
#[derive(Debug, Clone)]
struct PreparedPart {
    joint: usize,
    definition: RigidPartDefinition,
    /// Per vertex: (joint, weight, bind matrix). Empty means rigid.
    skin: Vec<Vec<(usize, f64, Affine2)>>,
}
impl PreparedRig {
    pub(crate) fn screens(&self) -> &[crate::screen::PreparedScreen] {
        &self.screens
    }
    pub fn joint_index(&self, id: &str) -> Option<usize> {
        self.ids.get(id).copied()
    }
    /// The validated joint hierarchy (names, parents and rest transforms).
    pub fn rig(&self) -> &Rig {
        &self.rig
    }
    /// Rotation limits of a joint, if the definition sets any.
    pub fn joint_limit(&self, joint: usize) -> Option<JointLimitDefinition> {
        self.limits.get(joint).copied().flatten()
    }
    /// World transform of every joint for the given offsets (same rules as
    /// `evaluate_parts`, including joint limits). Consumers such as IK and grip
    /// lookups read joint positions from here instead of re-deriving them.
    pub fn world_transforms(&self, offsets: &[(usize, Transform2)]) -> Result<Vec<Affine2>, Error> {
        self.world(offsets)
    }
    /// Evaluate parts with `(joint index, offset)` pairs applied to the rest pose:
    /// translations and rotations add, scales multiply. Later pairs for the same
    /// joint replace earlier ones. Non-finite or out-of-range results fail.
    pub fn evaluate_parts(
        &self,
        offsets: &[(usize, Transform2)],
    ) -> Result<Vec<EvaluatedPart>, Error> {
        let world = self.world(offsets)?;
        (0..self.parts.len())
            .map(|index| self.evaluate_part(index, &world, None, FaceView::Front))
            .collect()
    }
    fn world(&self, offsets: &[(usize, Transform2)]) -> Result<Vec<Affine2>, Error> {
        let mut pose = self.rig.rest_pose();
        for (joint, offset) in offsets {
            let rest = self
                .rig
                .joints()
                .get(*joint)
                .ok_or(Error::InvalidDefinition)?
                .rest;
            pose[*joint] = Transform2 {
                translation: Vec2::new(
                    rest.translation.x + offset.translation.x,
                    rest.translation.y + offset.translation.y,
                ),
                rotation: rest.rotation + offset.rotation,
                scale: Vec2::new(rest.scale.x * offset.scale.x, rest.scale.y * offset.scale.y),
            };
        }
        for (local, limit) in pose.iter_mut().zip(&self.limits) {
            if let Some(limit) = limit {
                local.rotation = limit.clamp(local.rotation);
            }
        }
        self.rig.evaluate(&pose)
    }
    /// Evaluate one part, optionally with the variant at `variant` (an index into
    /// the part's sorted variant ids) overriding polygon, colour and texture.
    fn evaluate_part(
        &self,
        index: usize,
        world: &[Affine2],
        variant: Option<usize>,
        view: FaceView,
    ) -> Result<EvaluatedPart, Error> {
        let prepared = &self.parts[index];
        let part = &prepared.definition;
        let variant = match variant {
            Some(i) => Some(
                part.variants
                    .values()
                    .nth(i)
                    .ok_or(Error::InvalidDefinition)?,
            ),
            None => None,
        };
        let rest_points = variant
            .and_then(|v| v.points.as_ref())
            .unwrap_or(&part.points);
        let mut color = variant.and_then(|v| v.color).unwrap_or(part.color);
        let mut texture_definition = variant
            .and_then(|v| v.texture.as_ref())
            .or(part.texture.as_ref());
        let feature = view != FaceView::Front && self.acting.features.binary_search(&index).is_ok();
        let turned: Vec<[f64; 2]>;
        let rest_points = match (feature, view, self.acting.side) {
            (true, FaceView::Back, _) => {
                // The back of the head shows no face.
                color[3] = 0;
                texture_definition = None;
                rest_points
            }
            (true, FaceView::Side, Some((center, half))) => {
                let shift = (1.0 - SIDE_SQUASH) * half;
                turned = rest_points
                    .iter()
                    .map(|p| [center + (p[0] - center) * SIDE_SQUASH + shift, p[1]])
                    .collect();
                &turned
            }
            _ => rest_points,
        };
        let transform = world[prepared.joint];
        let skinned = !prepared.skin.is_empty();
        let points: Vec<_> = if skinned {
            // Deterministic linear blend of posed influence transforms.
            rest_points
                .iter()
                .zip(&prepared.skin)
                .map(|(p, influences)| {
                    let p = Vec2::new(p[0], p[1]);
                    influences
                        .iter()
                        .fold(Vec2::default(), |acc, (j, w, bind)| {
                            let q = world[*j].point(bind.point(p));
                            Vec2::new(acc.x + w * q.x, acc.y + w * q.y)
                        })
                })
                .collect()
        } else {
            rest_points
                .iter()
                .map(|p| transform.point(Vec2::new(p[0], p[1])))
                .collect()
        };
        if points.iter().any(|p| {
            !p.x.is_finite()
                || !p.y.is_finite()
                || p.x.abs() > 1_000_000.0
                || p.y.abs() > 1_000_000.0
        }) {
            return Err(Error::InvalidDefinition);
        }
        let texture = match texture_definition {
            None => None,
            Some(t) => {
                let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                for p in rest_points {
                    x0 = x0.min(p[0]);
                    y0 = y0.min(p[1]);
                    x1 = x1.max(p[0]);
                    y1 = y1.max(p[1]);
                }
                if x1 - x0 < 1e-9 || y1 - y0 < 1e-9 {
                    return Err(Error::InvalidDefinition);
                }
                let matrix = if skinned {
                    fit_affine(rest_points, &points).ok_or(Error::InvalidDefinition)?
                } else {
                    transform
                };
                Some(EvaluatedTexture {
                    asset: t.asset.clone(),
                    uv: t.uv,
                    rest_bounds: [x0, y0, x1 - x0, y1 - y0],
                    transform: matrix,
                    fit: TextureFit::Stretch,
                    brightness: 1.0,
                    glow: None,
                })
            }
        };
        // Rounding is affine-invariant, so rounding posed control points equals
        // posing the rounded rest shape for rigid parts, and follows skinning.
        let points = round_corners(
            &points,
            variant.and_then(|v| v.smooth).unwrap_or(part.smooth),
        );
        let outline = variant
            .and_then(|v| v.outline)
            .or(part.outline)
            .filter(|o| o.width > 0.0 && o.color[3] > 0 && color[3] > 0)
            .map(|o| EvaluatedOutline {
                color: o.color,
                width: o.width,
                mode: o.mode,
            });
        Ok(EvaluatedPart {
            id: part.id.clone(),
            points,
            color,
            texture,
            skinned,
            outline,
        })
    }
    /// True when the rig declares visemes, expressions or eyes.
    pub fn has_acting(&self) -> bool {
        let a = &self.acting;
        a.mouth.is_some() || !a.expressions.is_empty() || !a.pupils.is_empty()
    }
    /// Indices into the evaluated parts of the pupils that follow the gaze.
    pub fn pupil_parts(&self) -> &[usize] {
        &self.acting.pupils
    }
    /// Whether a blink is in progress at scene time `time` for the actor `seed`.
    /// A pure function of the rig's blink settings, the seed and the time: one
    /// blink at most per `interval`-second window, so the rate is bounded, and any
    /// seek order gives the same answer.
    pub fn blink_active(&self, seed: &str, time: f64) -> bool {
        let Some(blink) = &self.acting.blink else {
            return false;
        };
        if !time.is_finite() || time < 0.0 {
            return false;
        }
        let window = (time / blink.interval).floor();
        let fraction = (blink_hash(seed, window as u64) >> 11) as f64 / (1u64 << 53) as f64;
        let start = window * blink.interval + fraction * (blink.interval - blink.duration);
        time >= start && time < start + blink.duration
    }
    /// Evaluate parts like `evaluate_parts`, then select variants: the actor's
    /// emotion entries first, then the blink, so a blink closes the eyes whatever
    /// the expression. Also returns every mapped mouth shape for the same pose.
    pub fn evaluate_acting(
        &self,
        offsets: &[(usize, Transform2)],
        input: ActingInput<'_>,
    ) -> Result<(Vec<EvaluatedPart>, Option<MouthShapes>), Error> {
        let world = self.world(offsets)?;
        let mut picks: Vec<Option<usize>> = vec![None; self.parts.len()];
        if let Some(entries) = self.acting.expressions.get(input.emotion) {
            for (part, variant) in entries {
                picks[*part] = Some(*variant);
            }
        }
        if let (true, Some(blink)) = (input.blink, &self.acting.blink) {
            for (part, variant) in &blink.picks {
                picks[*part] = Some(*variant);
            }
        }
        let parts = (0..self.parts.len())
            .map(|index| self.evaluate_part(index, &world, picks[index], input.view))
            .collect::<Result<Vec<_>, _>>()?;
        let mouth = match &self.acting.mouth {
            None => None,
            Some((part, table)) => {
                let mut shapes = Vec::new();
                for (slot, variant) in table.iter().enumerate() {
                    if let Some(variant) = variant {
                        shapes.push((
                            VISEME_NAMES[slot],
                            self.evaluate_part(*part, &world, Some(*variant), input.view)?,
                        ));
                    }
                }
                Some(MouthShapes {
                    part: *part,
                    shapes,
                })
            }
        };
        Ok((parts, mouth))
    }
    /// Offset the pupil parts toward a gaze direction. `gaze` is clamped to the
    /// unit disc and scaled by the rig's travel radius, in actor space.
    pub fn apply_gaze(&self, parts: &mut [EvaluatedPart], gaze: [f64; 2]) {
        let length = gaze[0].hypot(gaze[1]);
        if !length.is_finite() || length == 0.0 || self.acting.travel == 0.0 {
            return;
        }
        let scale = self.acting.travel * length.min(1.0) / length;
        let (dx, dy) = (gaze[0] * scale, gaze[1] * scale);
        for index in &self.acting.pupils {
            let Some(part) = parts.get_mut(*index) else {
                continue;
            };
            for point in &mut part.points {
                point.x += dx;
                point.y += dy;
            }
            if let Some(texture) = &mut part.texture {
                texture.transform.tx += dx;
                texture.transform.ty += dy;
            }
        }
    }
    /// Mean vertex of the pupil parts: the actor-space point gaze is measured from.
    pub fn eye_anchor(&self, parts: &[EvaluatedPart]) -> Option<Vec2> {
        let (mut x, mut y, mut n) = (0.0, 0.0, 0.0);
        for index in &self.acting.pupils {
            for point in &parts.get(*index)?.points {
                x += point.x;
                y += point.y;
                n += 1.0;
            }
        }
        (n > 0.0).then(|| Vec2::new(x / n, y / n))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn definition() -> RigDefinition {
        RigDefinition {
            version: 1,
            id: "puppet".into(),
            joints: vec![
                RigJointDefinition {
                    id: "root".into(),
                    parent: None,
                    x: 10.0,
                    y: 20.0,
                    rotation: std::f64::consts::FRAC_PI_2,
                    scale_x: 2.0,
                    scale_y: 2.0,
                },
                RigJointDefinition {
                    id: "hand".into(),
                    parent: Some("root".into()),
                    x: 3.0,
                    y: 0.0,
                    rotation: 0.0,
                    scale_x: 1.0,
                    scale_y: 1.0,
                },
            ],
            parts: vec![RigidPartDefinition {
                id: "hand_shape".into(),
                joint: "hand".into(),
                points: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                color: [255, 0, 20, 128],
                ..Default::default()
            }],
            ..Default::default()
        }
    }
    #[test]
    fn hierarchy_and_polygon_are_evaluated_in_source_draw_order() {
        let d = definition();
        let parts = d.evaluate_parts().unwrap();
        assert_eq!(parts[0].id, "hand_shape");
        assert_eq!(parts[0].color, [255, 0, 20, 128]);
        for (actual, expected) in
            parts[0]
                .points
                .iter()
                .zip([[10.0, 26.0], [10.0, 28.0], [8.0, 26.0]])
        {
            assert!((actual.x - expected[0]).abs() < 1e-10);
            assert!((actual.y - expected[1]).abs() < 1e-10);
        }
    }
    #[test]
    fn posed_evaluation_offsets_rest_pose_and_matches_rest_with_no_offsets() {
        let d = definition();
        let prepared = d.prepare().unwrap();
        assert_eq!(
            prepared.evaluate_parts(&[]).unwrap(),
            d.evaluate_parts().unwrap()
        );
        let hand = prepared.joint_index("hand").unwrap();
        assert_eq!(prepared.joint_index("missing"), None);
        // Identity offsets are exactly the rest pose.
        let same = prepared
            .evaluate_parts(&[(hand, Transform2::default())])
            .unwrap();
        assert_eq!(same, d.evaluate_parts().unwrap());
        // +2 x in the (scaled, rotated) parent frame moves the hand by 4 in world y.
        let moved = prepared
            .evaluate_parts(&[(
                hand,
                Transform2 {
                    translation: Vec2::new(2.0, 0.0),
                    ..Transform2::default()
                },
            )])
            .unwrap();
        let rest = d.evaluate_parts().unwrap();
        for (a, b) in moved[0].points.iter().zip(&rest[0].points) {
            assert!((a.x - b.x).abs() < 1e-10 && (a.y - b.y - 4.0).abs() < 1e-10);
        }
        // Invalid offsets and joints fail instead of drawing garbage.
        let bad = Transform2 {
            translation: Vec2::new(f64::NAN, 0.0),
            ..Transform2::default()
        };
        assert!(prepared.evaluate_parts(&[(hand, bad)]).is_err());
        assert!(prepared
            .evaluate_parts(&[(99, Transform2::default())])
            .is_err());
    }
    #[test]
    fn unsupported_versions_references_degenerate_and_world_overflow_fail() {
        let original = definition();
        let mut d = original.clone();
        d.version = 2;
        assert!(d.validate().is_err());
        let mut d = original.clone();
        d.joints[0].parent = Some("hand".into());
        assert!(d.validate().is_err());
        let mut d = original.clone();
        d.parts[0].joint = "missing".into();
        assert!(d.validate().is_err());
        let mut d = original.clone();
        d.parts[0].points = vec![[0.0, 0.0]; 3];
        assert!(d.validate().is_err());
        let mut d = original.clone();
        d.parts[0].points[0][0] = f64::NAN;
        assert!(d.validate().is_err());
        let mut d = original;
        d.joints[0].scale_x = 1e6;
        assert!(d.validate().is_err());
    }
    #[test]
    fn nonzero_area_crossings_retraced_edges_and_touches_fail_but_concavity_is_valid() {
        for points in [
            vec![[0.0, 0.0], [4.0, 4.0], [0.0, 4.0], [3.0, 0.0]],
            vec![[0.0, 0.0], [4.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
            vec![[0.0, 0.0], [4.0, 0.0], [2.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
            vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [2.0, 0.0], [0.0, 4.0]],
        ] {
            let mut rig = definition();
            rig.parts[0].points = points;
            assert_eq!(rig.validate(), Err(Error::InvalidDefinition));
        }
        let mut rig = definition();
        rig.parts[0].points = vec![[0.0, 0.0], [4.0, 0.0], [2.0, 2.0], [4.0, 4.0], [0.0, 4.0]];
        assert!(rig.validate().is_ok());
        rig.parts[0].points.reverse();
        assert!(rig.validate().is_ok());
    }
    fn w(joint: &str, weight: f64) -> VertexWeight {
        VertexWeight {
            joint: joint.into(),
            weight,
        }
    }
    /// Root at the origin, arm joint at (10, 0); a bar spans both joints and is
    /// skinned so the far end follows the arm and the middle blends both.
    fn skinned() -> RigDefinition {
        let mut d = definition();
        let joint = |id: &str, parent: Option<&str>, x: f64| RigJointDefinition {
            id: id.into(),
            parent: parent.map(Into::into),
            x,
            y: 0.0,
            rotation: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
        };
        d.joints = vec![joint("root", None, 0.0), joint("arm", Some("root"), 10.0)];
        d.parts[0].joint = "root".into();
        d.parts[0].points = vec![
            [0.0, -2.0],
            [10.0, -2.0],
            [20.0, -2.0],
            [20.0, 2.0],
            [10.0, 2.0],
            [0.0, 2.0],
        ];
        d.parts[0].skin = vec![
            vec![w("root", 1.0)],
            vec![w("root", 0.5), w("arm", 0.5)],
            vec![w("arm", 1.0)],
            vec![w("arm", 1.0)],
            vec![w("root", 0.5), w("arm", 0.5)],
            vec![w("root", 1.0)],
        ];
        d
    }
    fn bend(prepared: &PreparedRig, angle: f64) -> Vec<EvaluatedPart> {
        let arm = prepared.joint_index("arm").unwrap();
        prepared
            .evaluate_parts(&[(
                arm,
                Transform2 {
                    rotation: angle,
                    ..Transform2::default()
                },
            )])
            .unwrap()
    }
    #[test]
    fn skinned_rest_pose_equals_unskinned_for_any_weights() {
        let d = skinned();
        let mut rigid = d.clone();
        rigid.parts[0].skin.clear();
        let a = d.evaluate_parts().unwrap();
        let b = rigid.evaluate_parts().unwrap();
        assert!(a[0].skinned && !b[0].skinned);
        for (p, q) in a[0].points.iter().zip(&b[0].points) {
            assert!((p.x - q.x).abs() < 1e-9 && (p.y - q.y).abs() < 1e-9);
        }
    }
    #[test]
    fn bent_joint_blend_matches_hand_computed_vertices() {
        let half_turn = std::f64::consts::FRAC_PI_2;
        let prepared = skinned().prepare().unwrap();
        let posed = bend(&prepared, half_turn);
        let expected = [
            [0.0, -2.0],
            [11.0, -1.0],
            [12.0, 10.0],
            [8.0, 10.0],
            [9.0, 1.0],
            [0.0, 2.0],
        ];
        for (p, e) in posed[0].points.iter().zip(expected) {
            assert!(
                (p.x - e[0]).abs() < 1e-9 && (p.y - e[1]).abs() < 1e-9,
                "{p:?} vs {e:?}"
            );
        }
        // Repeated and independently prepared evaluation is bit-for-bit identical.
        assert_eq!(posed, bend(&prepared, half_turn));
        assert_eq!(posed, bend(&skinned().prepare().unwrap(), half_turn));
    }
    #[test]
    #[allow(clippy::type_complexity)]
    fn invalid_skin_weights_joints_and_rest_outlines_are_rejected() {
        let ok = skinned();
        assert!(ok.validate().is_ok());
        let cases: Vec<Box<dyn Fn(&mut RigDefinition)>> = vec![
            Box::new(|d| d.parts[0].skin[0] = vec![w("root", 0.9)]),
            Box::new(|d| d.parts[0].skin[0] = vec![w("root", f64::NAN)]),
            Box::new(|d| d.parts[0].skin[0] = vec![w("root", 1.5), w("arm", -0.5)]),
            Box::new(|d| d.parts[0].skin[0] = vec![w("missing", 1.0)]),
            Box::new(|d| d.parts[0].skin[0] = vec![w("root", 0.3), w("root", 0.7)]),
            Box::new(|d| d.parts[0].skin[0] = vec![]),
            Box::new(|d| d.parts[0].skin[0] = vec![w("root", 0.3), w("arm", 0.3), w("root", 0.4)]),
            Box::new(|d| {
                d.parts[0].skin.pop();
            }),
            // A duplicated vertex makes a degenerate outline at rest.
            Box::new(|d| d.parts[0].points[1] = [0.0, -2.0]),
        ];
        for (i, mutate) in cases.iter().enumerate() {
            let mut d = ok.clone();
            mutate(&mut d);
            assert_eq!(d.validate(), Err(Error::InvalidDefinition), "case {i}");
        }
    }
    #[test]
    fn textures_validate_follow_rigid_motion_and_fit_skinned_deformation() {
        let mut d = definition();
        d.parts[0].texture = Some(PartTextureDefinition {
            asset: "tex".into(),
            uv: [0.0, 0.0, 1.0, 1.0],
        });
        assert_eq!(d.image_assets().collect::<Vec<_>>(), ["tex"]);
        let parts = d.evaluate_parts().unwrap();
        let t = parts[0].texture.as_ref().unwrap();
        assert_eq!(t.rest_bounds, [0.0, 0.0, 1.0, 1.0]);
        // The rigid placement is exactly the joint's world transform: origin at (10, 26).
        assert!((t.transform.tx - 10.0).abs() < 1e-10 && (t.transform.ty - 26.0).abs() < 1e-10);
        for bad in [
            [0.0, 0.0, 1.5, 1.0],
            [0.5, 0.0, 0.5, 1.0],
            [0.0, 0.0, f64::NAN, 1.0],
            [0.0, 0.9, 1.0, 0.1],
        ] {
            let mut e = d.clone();
            e.parts[0].texture.as_mut().unwrap().uv = bad;
            assert!(e.validate().is_err());
        }
        let mut e = d.clone();
        e.parts[0].texture.as_mut().unwrap().asset = "".into();
        assert!(e.validate().is_err());
        let mut e = d.clone();
        let part = e.parts[0].clone();
        e.parts = (0..=MAX_TEXTURED_PARTS)
            .map(|i| RigidPartDefinition {
                id: format!("p{i}"),
                ..part.clone()
            })
            .collect();
        assert!(e.validate().is_err());
        // All weight on one joint: the least-squares fit is exactly that joint.
        let mut s = skinned();
        s.parts[0].skin = vec![vec![w("arm", 1.0)]; 6];
        s.parts[0].texture = d.parts[0].texture.clone();
        let prepared = s.prepare().unwrap();
        let moved = bend(&prepared, 0.5);
        let fit = moved[0].texture.as_ref().unwrap().transform;
        let (sin, cos) = 0.5f64.sin_cos();
        assert!((fit.a - cos).abs() < 1e-9 && (fit.b - sin).abs() < 1e-9);
        assert!((fit.tx - (10.0 - 10.0 * cos)).abs() < 1e-9);
    }
    fn square() -> Vec<[f64; 2]> {
        vec![[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [0.0, 20.0]]
    }
    /// Largest turning angle (radians) between consecutive edges of a closed polygon.
    fn max_turn(points: &[Vec2]) -> f64 {
        let n = points.len();
        (0..n)
            .map(|i| {
                let (a, b, c) = (points[i], points[(i + 1) % n], points[(i + 2) % n]);
                let (ux, uy, vx, vy) = (b.x - a.x, b.y - a.y, c.x - b.x, c.y - b.y);
                (ux * vy - uy * vx).atan2(ux * vx + uy * vy).abs()
            })
            .fold(0.0, f64::max)
    }
    #[test]
    fn ellipse_parts_expand_to_a_fixed_polygon_through_the_joint_transform() {
        let mut d = definition();
        d.parts[0].points.clear();
        d.parts[0].ellipse = Some(EllipseDefinition {
            cx: 1.0,
            cy: 2.0,
            rx: 3.0,
            ry: 1.5,
        });
        let parts = d.evaluate_parts().unwrap();
        assert_eq!(parts[0].points.len(), ELLIPSE_SEGMENTS);
        // Root: translate (10, 20), rotate 90 degrees, scale 2; hand at x = 3.
        let hand = d.prepare().unwrap().world(&[]).unwrap()[1];
        for (i, p) in parts[0].points.iter().enumerate() {
            let angle = std::f64::consts::TAU * i as f64 / ELLIPSE_SEGMENTS as f64;
            let q = hand.point(Vec2::new(1.0 + 3.0 * angle.cos(), 2.0 + 1.5 * angle.sin()));
            assert!((p.x - q.x).abs() < 1e-9 && (p.y - q.y).abs() < 1e-9);
        }
        // 2:1 ellipse: the sharpest turn (at the long ends) is about 2 * TAU / 48.
        assert!(max_turn(&parts[0].points) < 0.3);
        // Serialization keeps `points` out for ellipse parts and legacy fields only otherwise.
        let legacy = definition();
        assert!(legacy.parts[0].ellipse.is_none() && legacy.parts[0].smooth == 0);
    }
    #[test]
    fn rounding_is_bounded_smooth_inside_the_hull_and_commutes_with_rigid_posing() {
        let mut d = definition();
        d.parts[0].points = square();
        d.parts[0].smooth = 8;
        let prepared = d.prepare().unwrap();
        let rounded = &prepared.evaluate_parts(&[]).unwrap()[0].points;
        assert_eq!(rounded.len(), 4 * 8);
        // No sharp corner survives: every turn is a small fraction of the 90 degrees.
        assert!(max_turn(rounded) < 0.25, "{}", max_turn(rounded));
        // Rounding the posed control polygon equals posing the rounded rest shape.
        let hand = prepared.world(&[]).unwrap()[1];
        let rest: Vec<Vec2> = square().iter().map(|p| Vec2::new(p[0], p[1])).collect();
        for (a, b) in round_corners(&rest, 8).iter().zip(rounded) {
            let q = hand.point(*a);
            assert!((q.x - b.x).abs() < 1e-9 && (q.y - b.y).abs() < 1e-9);
        }
        // Rest-space rounded square: corners cut, edge midpoints kept, inside the hull.
        let local = round_corners(&rest, 8);
        assert!(local
            .iter()
            .all(|p| (0.0..=20.0).contains(&p.x) && (0.0..=20.0).contains(&p.y)));
        assert!(local.iter().any(|p| p.x == 10.0 && p.y == 0.0));
        let corner = local
            .iter()
            .map(|p| p.x.hypot(p.y))
            .fold(f64::MAX, f64::min);
        assert!(corner > 3.0, "corner kept at distance {corner}");
        // Deterministic: repeated evaluation is bit-identical.
        assert_eq!(
            prepared.evaluate_parts(&[]).unwrap(),
            prepared.evaluate_parts(&[]).unwrap()
        );
        assert_eq!(round_corners(&rest, 0), rest);
    }
    #[test]
    fn skinned_parts_are_rounded_after_deformation() {
        let mut d = skinned();
        d.parts[0].smooth = 4;
        let mut plain = skinned();
        plain.parts[0].smooth = 0;
        let (a, b) = (d.prepare().unwrap(), plain.prepare().unwrap());
        let deformed = bend(&b, 0.6);
        let expected = round_corners(&deformed[0].points, 4);
        let rounded = bend(&a, 0.6);
        assert!(rounded[0].skinned);
        assert_eq!(rounded[0].points, expected);
    }
    #[test]
    fn outlines_resolve_per_variant_and_hide_with_transparent_fills() {
        let mut d = definition();
        let outline = OutlineDefinition {
            color: [0, 0, 0, 255],
            width: 2.0,
            mode: OutlineMode::Behind,
        };
        d.parts[0].color[3] = 255;
        d.parts[0].outline = Some(outline);
        d.parts[0].variants = BTreeMap::from([
            (
                "thin".to_string(),
                PartVariantDefinition {
                    outline: Some(OutlineDefinition {
                        width: 0.5,
                        mode: OutlineMode::Over,
                        ..outline
                    }),
                    ..Default::default()
                },
            ),
            (
                "none".to_string(),
                PartVariantDefinition {
                    outline: Some(OutlineDefinition {
                        width: 0.0,
                        ..outline
                    }),
                    ..Default::default()
                },
            ),
            (
                "hidden".to_string(),
                PartVariantDefinition {
                    color: Some([1, 2, 3, 0]),
                    ..Default::default()
                },
            ),
            (
                "round".to_string(),
                PartVariantDefinition {
                    ellipse: Some(EllipseDefinition {
                        cx: 0.0,
                        cy: 0.0,
                        rx: 2.0,
                        ry: 2.0,
                    }),
                    smooth: Some(0),
                    ..Default::default()
                },
            ),
        ]);
        d.expressions = BTreeMap::from([(
            "happy".to_string(),
            vec![ExpressionVariant {
                part: "hand_shape".into(),
                variant: "thin".into(),
            }],
        )]);
        let prepared = d.prepare().unwrap();
        let base = &prepared.evaluate_parts(&[]).unwrap()[0];
        assert_eq!(
            base.outline,
            Some(EvaluatedOutline {
                color: [0, 0, 0, 255],
                width: 2.0,
                mode: OutlineMode::Behind
            })
        );
        let world = prepared.world(&[]).unwrap();
        let variant = |name: &str| {
            let index = d.parts[0].variants.keys().position(|k| k == name).unwrap();
            prepared
                .evaluate_part(0, &world, Some(index), FaceView::Front)
                .unwrap()
        };
        let thin = variant("thin").outline.unwrap();
        assert_eq!((thin.width, thin.mode), (0.5, OutlineMode::Over));
        assert_eq!(variant("none").outline, None);
        assert_eq!(variant("hidden").outline, None);
        assert_eq!(variant("round").points.len(), ELLIPSE_SEGMENTS);
        let input = ActingInput {
            emotion: "happy",
            blink: false,
            view: FaceView::Front,
        };
        let (acting, _) = prepared.evaluate_acting(&[], input).unwrap();
        assert_eq!(acting[0].outline.unwrap().width, 0.5);
        // The expression part is a facial feature: the side view turns it toward
        // +x without widening it, and the back view hides it.
        let view = |view| {
            prepared
                .evaluate_acting(&[], ActingInput { view, ..input })
                .unwrap()
                .0
        };
        // The joint may be rotated, so compare whole shapes, not world x extents.
        assert_ne!(view(FaceView::Side)[0].points, acting[0].points);
        assert_eq!(view(FaceView::Front)[0].points, acting[0].points);
        let back = &view(FaceView::Back)[0];
        assert_eq!(back.color[3], 0);
        assert_eq!(back.outline, None);
        assert_eq!(view(FaceView::Back)[0].points, acting[0].points);
    }
    #[test]
    #[allow(clippy::type_complexity)]
    fn invalid_shapes_rounding_and_outlines_are_rejected() {
        let e = EllipseDefinition {
            cx: 0.0,
            cy: 0.0,
            rx: 4.0,
            ry: 4.0,
        };
        let cases: Vec<Box<dyn Fn(&mut RigDefinition)>> = vec![
            // An ellipse with points, or neither.
            Box::new(move |d| d.parts[0].ellipse = Some(e)),
            Box::new(|d| d.parts[0].points.clear()),
            Box::new(move |d| {
                d.parts[0].points.clear();
                d.parts[0].ellipse = Some(EllipseDefinition { rx: 0.0, ..e });
            }),
            Box::new(move |d| {
                d.parts[0].points.clear();
                d.parts[0].ellipse = Some(EllipseDefinition { cy: f64::NAN, ..e });
            }),
            Box::new(|d| d.parts[0].smooth = 1),
            Box::new(|d| d.parts[0].smooth = MAX_SMOOTH + 1),
            // 64 control points times 16 samples exceeds the evaluated bound.
            Box::new(|d| {
                d.parts[0].points = (0..64)
                    .map(|i| {
                        let a = std::f64::consts::TAU * f64::from(i) / 64.0;
                        [10.0 * a.cos(), 10.0 * a.sin()]
                    })
                    .collect();
                d.parts[0].smooth = 16;
            }),
            Box::new(|d| {
                d.parts[0].outline = Some(OutlineDefinition {
                    color: [0; 4],
                    width: f64::NAN,
                    mode: OutlineMode::Over,
                })
            }),
            Box::new(|d| {
                d.parts[0].outline = Some(OutlineDefinition {
                    color: [0; 4],
                    width: -1.0,
                    mode: OutlineMode::Over,
                })
            }),
            Box::new(|d| {
                d.parts[0].outline = Some(OutlineDefinition {
                    color: [0; 4],
                    width: MAX_OUTLINE_WIDTH + 1.0,
                    mode: OutlineMode::Over,
                })
            }),
            Box::new(move |d| {
                d.parts[0].variants.insert(
                    "v".into(),
                    PartVariantDefinition {
                        points: Some(square()),
                        ellipse: Some(e),
                        ..Default::default()
                    },
                );
            }),
            Box::new(|d| {
                d.parts[0].variants.insert(
                    "v".into(),
                    PartVariantDefinition {
                        smooth: Some(99),
                        ..Default::default()
                    },
                );
            }),
        ];
        for (i, mutate) in cases.iter().enumerate() {
            let mut d = definition();
            mutate(&mut d);
            assert_eq!(d.validate(), Err(Error::InvalidDefinition), "case {i}");
        }
        // Skinned parts cannot become ellipses (skin weights are per control point).
        let mut s = skinned();
        s.parts[0].variants.insert(
            "v".into(),
            PartVariantDefinition {
                ellipse: Some(e),
                ..Default::default()
            },
        );
        assert_eq!(s.validate(), Err(Error::InvalidDefinition));
        let mut s = skinned();
        s.parts[0].points.clear();
        s.parts[0].ellipse = Some(e);
        assert_eq!(s.validate(), Err(Error::InvalidDefinition));
    }
}
