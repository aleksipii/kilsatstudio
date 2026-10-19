//! Persisted scene artwork and bounded raster resources shared by preview/export.
mod resource;
pub use resource::{
    decode_image, load_image, ImageResource, MAX_IMAGE_BYTES, MAX_IMAGE_PIXELS, MAX_IMAGE_SIDE,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_LAYERS: usize = 1024;
pub const MAX_POINTS: usize = 65_536;
pub const MAX_TOTAL_POINTS: usize = 262_144;
pub const MAX_COORDINATE: f64 = 1_000_000.0;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Artwork validation: {0}")]
    Validation(String),
    #[error("Artwork image I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("Artwork image: {0}")]
    Image(#[from] image::ImageError),
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::Validation(message.into()))
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Content {
    Image {
        asset_id: String,
        rect: Rect,
    },
    Stroke {
        points: Vec<Point>,
        color: Color,
        width: f64,
    },
    Rectangle {
        rect: Rect,
        color: Color,
    },
    Ellipse {
        rect: Rect,
        color: Color,
    },
}
/// Rendering plane; older projects retain their foreground stacking order.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Plane {
    Background,
    #[default]
    Foreground,
}
/// Local geometric clip; legacy rectangular `Layer.clip` remains readable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClipShape {
    Ellipse {
        rect: Rect,
    },
    /// Simple polygon. A positive `feather` is a Gaussian standard deviation in
    /// scene units centred on the boundary (about half coverage on the edge).
    Polygon {
        points: Vec<Point>,
        #[serde(default, skip_serializing_if = "is_zero")]
        feather: f64,
    },
    /// Raster mask from a project image asset stretched over `rect`. Content is
    /// visible where the chosen channel of the image is opaque/bright.
    Image {
        asset_id: String,
        rect: Rect,
        #[serde(default)]
        channel: MaskChannel,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        keys: Vec<RectMaskKey>,
    },
    /// Ordered opaque vector paint/erase strokes over explicit initial coverage.
    Painted {
        base_visible: bool,
        strokes: Vec<MaskStroke>,
    },
    /// Elliptical alpha mask with an inward, normalized radial feather.
    /// Keys use scene-local seconds; their first/last values hold outside the track.
    SoftEllipse {
        rect: Rect,
        feather: f64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        keys: Vec<MaskKey>,
    },
}
/// One round brush gesture in layer-local scene units. Erasure paints black in
/// an explicitly luminance-based mask; later white strokes can restore coverage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MaskStroke {
    pub points: Vec<Point>,
    pub radius: f64,
    pub erase: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pressures: Vec<f64>,
    #[serde(default = "full_hardness", skip_serializing_if = "is_full_hardness")]
    pub hardness: f64,
}
fn full_hardness() -> f64 {
    1.0
}
fn is_full_hardness(value: &f64) -> bool {
    *value == 1.0
}
/// A union primitive of a pressure-scaled brush footprint.
#[derive(Debug, Clone, PartialEq)]
pub enum MaskBrushPrimitive {
    Circle { center: Point, radius: f64 },
    Polygon { points: [Point; 4] },
}
pub const MAX_MASK_STROKES: usize = 512;
pub const MAX_TOTAL_MASK_STROKES: usize = 4096;
pub const MAX_MASK_STROKE_POINTS: usize = 2048;
pub const MAX_MASK_RADIUS: f64 = 2048.0;
pub const MAX_TOTAL_MASK_RENDER_COST: usize = 1_048_576;
impl MaskStroke {
    /// Softness has 32 deterministic coverage levels, shared by CPU and GPU.
    /// A band is a union, so intersecting segments never accumulate opacity.
    pub fn coverage_bands(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        let count = if self.hardness == 1.0 { 1 } else { 32 };
        (1..=count).map(move |i| {
            let coverage = f64::from(i) / f64::from(count);
            let threshold = if count == 1 {
                1.0
            } else {
                coverage - 0.5 / f64::from(count)
            };
            (coverage, 1.0 - threshold * (1.0 - self.hardness))
        })
    }
    pub fn render_cost(&self) -> usize {
        self.points
            .len()
            .saturating_mul(if self.hardness == 1.0 { 2 } else { 64 })
    }
    /// Exact swept disks for a linearly changing radius: endpoint disks and
    /// their common external tangent quad. A contained disk needs no quad.
    pub fn band_geometry(&self, scale: f64) -> Vec<MaskBrushPrimitive> {
        let radius = |i: usize| self.radius * self.pressures.get(i).copied().unwrap_or(1.0) * scale;
        let mut result = Vec::with_capacity(self.points.len().saturating_mul(2));
        for (i, &center) in self.points.iter().enumerate() {
            let r = radius(i);
            if r > 0.0 {
                result.push(MaskBrushPrimitive::Circle { center, radius: r });
            }
            if i == 0 {
                continue;
            }
            let a = self.points[i - 1];
            let ra = radius(i - 1);
            let dx = center.x - a.x;
            let dy = center.y - a.y;
            let length = dx.hypot(dy);
            if length <= (r - ra).abs() || length == 0.0 {
                continue;
            }
            let ux = dx / length;
            let uy = dy / length;
            let slope = (ra - r) / length;
            let side = (1.0 - slope * slope).max(0.0).sqrt();
            let normal = |sign: f64| Point {
                x: ux * slope - uy * side * sign,
                y: uy * slope + ux * side * sign,
            };
            let n = normal(1.0);
            let m = normal(-1.0);
            result.push(MaskBrushPrimitive::Polygon {
                points: [
                    Point {
                        x: a.x + n.x * ra,
                        y: a.y + n.y * ra,
                    },
                    Point {
                        x: a.x + m.x * ra,
                        y: a.y + m.y * ra,
                    },
                    Point {
                        x: center.x + m.x * r,
                        y: center.y + m.y * r,
                    },
                    Point {
                        x: center.x + n.x * r,
                        y: center.y + n.y * r,
                    },
                ],
            });
        }
        result
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.points.is_empty()
            || self.points.len() > MAX_MASK_STROKE_POINTS
            || !self.radius.is_finite()
            || !(0.05..=MAX_MASK_RADIUS).contains(&self.radius)
            || !self.hardness.is_finite()
            || !(0.0..=1.0).contains(&self.hardness)
            || (!self.pressures.is_empty() && self.pressures.len() != self.points.len())
            || self
                .pressures
                .iter()
                .any(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
            || self
                .points
                .iter()
                .any(|p| !coordinate(p.x) || !coordinate(p.y))
        {
            return invalid(
                "mask stroke requires 1–2048 bounded finite points, radius 0.05–2048, hardness 0–1 and matching pressures 0–1",
            );
        }
        Ok(())
    }
}
/// Which channel of a mask image controls coverage.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MaskChannel {
    #[default]
    Alpha,
    Luminance,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MaskKey {
    pub time: f64,
    pub rect: Rect,
    pub feather: f64,
}
/// Scene-local image-mask rectangle key; endpoints hold outside the track.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RectMaskKey {
    pub time: f64,
    pub rect: Rect,
}
/// SVG/resvg sRGB backdrop blending. Normal preserves legacy source-over output.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
}
impl BlendMode {
    pub fn is_normal(&self) -> bool {
        *self == Self::Normal
    }
}
pub const MAX_MASK_POINTS: usize = 256;
/// Largest polygon feather standard deviation, in scene units.
pub const MAX_POLYGON_FEATHER: f64 = 1000.0;
fn is_zero(value: &f64) -> bool {
    *value == 0.0
}
pub const MAX_MASK_KEYS: usize = 128;
pub const MAX_TOTAL_MASK_KEYS: usize = 8192;
impl ClipShape {
    /// Scene-dependent bounds, after `Document::validate` checks the track itself.
    pub fn validate_scene_duration(&self, duration: f64) -> Result<(), Error> {
        if !duration.is_finite() || duration <= 0.0 {
            return invalid("mask scene duration must be finite and positive");
        }
        let last_time = match self {
            Self::SoftEllipse { keys, .. } => keys.last().map(|key| key.time),
            Self::Image { keys, .. } => keys.last().map(|key| key.time),
            _ => None,
        };
        if last_time.is_some_and(|time| time > duration) {
            return invalid("artwork mask key exceeds its scene duration");
        }
        Ok(())
    }
    /// Evaluate an elliptical mask without allocations. Call after validation.
    /// A feather of zero has a hard boundary; one fades from center to edge.
    pub fn sample_ellipse(&self, time: f64) -> Option<(Rect, f64)> {
        match self {
            Self::Ellipse { rect } => Some((*rect, 0.0)),
            Self::SoftEllipse {
                rect,
                feather,
                keys,
            } => {
                if keys.is_empty() || !time.is_finite() {
                    return Some((*rect, *feather));
                }
                let index = keys.partition_point(|key| key.time <= time);
                if index == 0 {
                    return Some((keys[0].rect, keys[0].feather));
                }
                let left = keys[index - 1];
                if index == keys.len() {
                    return Some((left.rect, left.feather));
                }
                let right = keys[index];
                let t = (time - left.time) / (right.time - left.time);
                let lerp = |a: f64, b: f64| a + (b - a) * t;
                Some((
                    Rect {
                        x: lerp(left.rect.x, right.rect.x),
                        y: lerp(left.rect.y, right.rect.y),
                        width: lerp(left.rect.width, right.rect.width),
                        height: lerp(left.rect.height, right.rect.height),
                    },
                    lerp(left.feather, right.feather),
                ))
            }
            Self::Polygon { .. } | Self::Image { .. } | Self::Painted { .. } => None,
        }
    }
    pub fn key_count(&self) -> usize {
        match self {
            Self::SoftEllipse { keys, .. } => keys.len(),
            Self::Image { keys, .. } => keys.len(),
            _ => 0,
        }
    }
    /// Sample the image mask independently of seek order, in scene-local seconds.
    pub fn sample_image_rect(&self, time: f64) -> Option<Rect> {
        let Self::Image { rect, keys, .. } = self else {
            return None;
        };
        if keys.is_empty() || !time.is_finite() {
            return Some(*rect);
        }
        let index = keys.partition_point(|key| key.time <= time);
        if index == 0 {
            return Some(keys[0].rect);
        }
        let left = keys[index - 1];
        if index == keys.len() {
            return Some(left.rect);
        }
        let right = keys[index];
        let t = (time - left.time) / (right.time - left.time);
        let lerp = |a: f64, b: f64| a + (b - a) * t;
        Some(Rect {
            x: lerp(left.rect.x, right.rect.x),
            y: lerp(left.rect.y, right.rect.y),
            width: lerp(left.rect.width, right.rect.width),
            height: lerp(left.rect.height, right.rect.height),
        })
    }
    /// Project image asset used by an image mask, if any.
    pub fn image_asset(&self) -> Option<&str> {
        match self {
            Self::Image { asset_id, .. } => Some(asset_id),
            _ => None,
        }
    }
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Painted { strokes, .. } => {
                if strokes.len() > MAX_MASK_STROKES {
                    return invalid("painted mask exceeds 512 strokes");
                }
                let mut points = 0;
                for stroke in strokes {
                    stroke.validate()?;
                    points += stroke.points.len();
                    if points > MAX_TOTAL_POINTS {
                        return invalid("painted mask exceeds total artwork point budget");
                    }
                }
                Ok(())
            }
            Self::Ellipse { rect } => rect.validate(),
            Self::SoftEllipse {
                rect,
                feather,
                keys,
            } => {
                rect.validate()?;
                if !feather.is_finite() || !(0.0..=1.0).contains(feather) {
                    return invalid("mask feather must be finite and between 0 and 1");
                }
                if keys.len() > MAX_MASK_KEYS {
                    return invalid("animated mask exceeds 128 keys");
                }
                let mut previous = None;
                for key in keys {
                    key.rect.validate()?;
                    if !key.time.is_finite()
                        || key.time < 0.0
                        || previous.is_some_and(|time| key.time <= time)
                        || !key.feather.is_finite()
                        || !(0.0..=1.0).contains(&key.feather)
                    {
                        return invalid("mask keys require increasing finite nonnegative seconds and feather 0–1");
                    }
                    previous = Some(key.time);
                }
                Ok(())
            }
            Self::Image { asset_id, rect, keys, .. } => {
                if !identifier(asset_id) {
                    return invalid("invalid mask image asset identifier");
                }
                rect.validate()?;
                if keys.len() > MAX_MASK_KEYS {
                    return invalid("animated mask exceeds 128 keys");
                }
                let mut previous = None;
                for key in keys {
                    key.rect.validate()?;
                    if !key.time.is_finite()
                        || key.time < 0.0
                        || previous.is_some_and(|time| key.time <= time)
                    {
                        return invalid("image mask keys require increasing finite nonnegative seconds");
                    }
                    previous = Some(key.time);
                }
                Ok(())
            }
            Self::Polygon { points, feather } => {
                if !feather.is_finite() || !(0.0..=MAX_POLYGON_FEATHER).contains(feather) {
                    return invalid("polygon feather must be finite and between 0 and 1000");
                }
                if !(3..=MAX_MASK_POINTS).contains(&points.len())
                    || points.iter().any(|p| !coordinate(p.x) || !coordinate(p.y))
                {
                    return invalid("polygon mask requires 3–256 bounded finite vertices");
                }
                let n = points.len();
                let mut area = 0.0;
                for i in 0..n {
                    let a = points[i];
                    let b = points[(i + 1) % n];
                    if a == b {
                        return invalid("polygon mask has a zero-length edge");
                    }
                    let c = points[(i + 2) % n];
                    let cross = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
                    let dot = (b.x - a.x) * (c.x - b.x) + (b.y - a.y) * (c.y - b.y);
                    if cross == 0.0 && dot < 0.0 {
                        return invalid("polygon mask has overlapping adjacent edges");
                    }
                    area += a.x * b.y - b.x * a.y;
                    for j in i + 1..n {
                        if j == i + 1 || (i == 0 && j == n - 1) {
                            continue;
                        }
                        if intersects(a, b, points[j], points[(j + 1) % n]) {
                            return invalid("polygon mask must not intersect or touch itself");
                        }
                    }
                }
                if area.abs() <= 1e-9 {
                    return invalid("polygon mask must enclose a nonzero area");
                }
                Ok(())
            }
        }
    }
}
fn intersects(a: Point, b: Point, c: Point, d: Point) -> bool {
    let cross =
        |a: Point, b: Point, c: Point| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    let on = |a: Point, b: Point, c: Point| {
        c.x >= a.x.min(b.x) && c.x <= a.x.max(b.x) && c.y >= a.y.min(b.y) && c.y <= a.y.max(b.y)
    };
    let (ab_c, ab_d, cd_a, cd_b) = (
        cross(a, b, c),
        cross(a, b, d),
        cross(c, d, a),
        cross(c, d, b),
    );
    (ab_c == 0.0 && on(a, b, c))
        || (ab_d == 0.0 && on(a, b, d))
        || (cd_a == 0.0 && on(c, d, a))
        || (cd_b == 0.0 && on(c, d, b))
        || ((ab_c > 0.0) != (ab_d > 0.0) && (cd_a > 0.0) != (cd_b > 0.0))
}
/// Scene-local, half-open visibility interval in seconds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TimeRange {
    pub start: f64,
    pub end: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub id: String,
    pub scene_id: String,
    pub name: String,
    pub visible: bool,
    #[serde(default)]
    pub plane: Plane,
    #[serde(default)]
    pub active: Option<TimeRange>,
    pub opacity: f64,
    /// Actor translation only; coordinates and clipping remain actor-local.
    pub attachment: Option<String>,
    pub clip: Option<Rect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_shape: Option<ClipShape>,
    #[serde(default, skip_serializing_if = "BlendMode::is_normal")]
    pub blend: BlendMode,
    pub content: Content,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub layers: Vec<Layer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Group>,
    /// Explicit root sibling order. Empty retains legacy flat ordering only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<ArtUnit>,
}
/// An item has exactly one owner: the root order or one group's children.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    content = "id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ArtUnit {
    Layer(String),
    Group(String),
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub id: String,
    pub scene_id: String,
    pub plane: Plane,
    pub name: String,
    pub visible: bool,
    pub opacity: f64,
    #[serde(default, skip_serializing_if = "BlendMode::is_normal")]
    pub blend: BlendMode,
    pub children: Vec<ArtUnit>,
}
#[derive(Debug, Clone, Copy)]
pub enum CompositionStep<'a> {
    EnterGroup(&'a Group),
    Layer(&'a Layer),
    LeaveGroup,
}
pub const MAX_GROUPS: usize = 256;
pub const MAX_GROUP_DEPTH: usize = 8;
pub const MAX_GROUP_CHILDREN: usize = 128;

fn identifier(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}
fn coordinate(value: f64) -> bool {
    value.is_finite() && value.abs() <= MAX_COORDINATE
}
impl Rect {
    pub fn validate(&self) -> Result<(), Error> {
        if !coordinate(self.x)
            || !coordinate(self.y)
            || !coordinate(self.width)
            || !coordinate(self.height)
            || self.width <= 0.0
            || self.height <= 0.0
            || !coordinate(self.x + self.width)
            || !coordinate(self.y + self.height)
        {
            return invalid("rectangle requires bounded finite position and positive dimensions");
        }
        Ok(())
    }
}
impl Layer {
    /// Every project image asset this layer needs: its content and its image mask.
    pub fn image_assets(&self) -> impl Iterator<Item = &str> {
        let content = match &self.content {
            Content::Image { asset_id, .. } => Some(asset_id.as_str()),
            _ => None,
        };
        content
            .into_iter()
            .chain(self.clip_shape.as_ref().and_then(ClipShape::image_asset))
    }
    /// Visibility at a scene-local time; an interval excludes its end boundary.
    pub fn is_active(&self, time: f64) -> bool {
        self.visible
            && time.is_finite()
            && time >= 0.0
            && self
                .active
                .is_none_or(|range| range.start <= time && time < range.end)
    }
}
impl Document {
    fn root_order(&self) -> Vec<ArtUnit> {
        if self.groups.is_empty() && self.order.is_empty() {
            self.layers
                .iter()
                .map(|l| ArtUnit::Layer(l.id.clone()))
                .collect()
        } else {
            self.order.clone()
        }
    }
    pub fn unit_scope(&self, unit: &ArtUnit) -> Option<(&str, Plane)> {
        match unit {
            ArtUnit::Layer(id) => self
                .layers
                .iter()
                .find(|l| &l.id == id)
                .map(|l| (l.scene_id.as_str(), l.plane)),
            ArtUnit::Group(id) => self
                .groups
                .iter()
                .find(|g| &g.id == id)
                .map(|g| (g.scene_id.as_str(), g.plane)),
        }
    }
    fn validate_groups(&self) -> Result<(), Error> {
        if self.groups.len() > MAX_GROUPS {
            return invalid("artwork exceeds 256 groups");
        }
        let mut ids: BTreeSet<&str> = self.layers.iter().map(|l| l.id.as_str()).collect();
        for group in &self.groups {
            if !identifier(&group.id)
                || !ids.insert(&group.id)
                || !identifier(&group.scene_id)
                || group.name.len() > 512
                || group.name.chars().any(char::is_control)
                || !group.opacity.is_finite()
                || !(0.0..=1.0).contains(&group.opacity)
                || group.children.len() > MAX_GROUP_CHILDREN
            {
                return invalid("invalid group identifiers, properties or child budget");
            }
        }
        fn visit(
            doc: &Document,
            units: &[ArtUnit],
            parent: Option<(&str, Plane)>,
            depth: usize,
            seen: &mut BTreeSet<String>,
        ) -> Result<(), Error> {
            for unit in units {
                let id = match unit {
                    ArtUnit::Layer(id) | ArtUnit::Group(id) => id,
                };
                let Some(scope) = doc.unit_scope(unit) else {
                    return invalid("unknown artwork order item");
                };
                if !seen.insert(id.clone()) {
                    return invalid("duplicate or cyclic artwork ownership");
                }
                if parent.is_some_and(|p| p != scope) {
                    return invalid("group children must share scene and plane");
                }
                if let ArtUnit::Group(id) = unit {
                    if depth >= MAX_GROUP_DEPTH {
                        return invalid("artwork exceeds group depth 8");
                    }
                    let group = doc.groups.iter().find(|g| &g.id == id).unwrap();
                    visit(doc, &group.children, Some(scope), depth + 1, seen)?;
                }
            }
            Ok(())
        }
        let mut seen = BTreeSet::new();
        visit(self, &self.root_order(), None, 0, &mut seen)?;
        if seen.len() != self.layers.len() + self.groups.len() {
            return invalid("every artwork item must appear exactly once in sibling order");
        }
        Ok(())
    }
    /// Validated bounded traversal; opacity and blend remain isolated group operations.
    pub fn composition<'a>(
        &'a self,
        scene: &str,
        plane: Plane,
        time: f64,
    ) -> Result<Vec<CompositionStep<'a>>, Error> {
        self.validate()?;
        fn emit<'a>(
            doc: &'a Document,
            units: &[ArtUnit],
            scene: &str,
            plane: Plane,
            time: f64,
            out: &mut Vec<CompositionStep<'a>>,
        ) {
            for unit in units {
                if doc.unit_scope(unit) != Some((scene, plane)) {
                    continue;
                }
                match unit {
                    ArtUnit::Layer(id) => {
                        let l = doc.layers.iter().find(|l| &l.id == id).unwrap();
                        if l.is_active(time) {
                            out.push(CompositionStep::Layer(l));
                        }
                    }
                    ArtUnit::Group(id) => {
                        let g = doc.groups.iter().find(|g| &g.id == id).unwrap();
                        if g.visible {
                            out.push(CompositionStep::EnterGroup(g));
                            emit(doc, &g.children, scene, plane, time, out);
                            out.push(CompositionStep::LeaveGroup);
                        }
                    }
                }
            }
        }
        let mut result = Vec::with_capacity(self.layers.len() + self.groups.len() * 2);
        emit(self, &self.root_order(), scene, plane, time, &mut result);
        Ok(result)
    }
    fn detach(&mut self, unit: &ArtUnit) -> Result<(Option<String>, usize), Error> {
        if let Some(index) = self.order.iter().position(|u| u == unit) {
            self.order.remove(index);
            return Ok((None, index));
        }
        for group in &mut self.groups {
            if let Some(index) = group.children.iter().position(|u| u == unit) {
                group.children.remove(index);
                return Ok((Some(group.id.clone()), index));
            }
        }
        invalid("artwork item has no sibling owner")
    }
    fn siblings(&mut self, parent: Option<&str>) -> Result<&mut Vec<ArtUnit>, Error> {
        match parent {
            None => Ok(&mut self.order),
            Some(id) => self
                .groups
                .iter_mut()
                .find(|g| g.id == id)
                .map(|g| &mut g.children)
                .ok_or_else(|| Error::Validation("group does not exist".into())),
        }
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.layers.len() > MAX_LAYERS {
            return invalid("artwork exceeds 1024 layers");
        }
        let mut ids = BTreeSet::new();
        let mut point_count = 0;
        let mut mask_key_count = 0;
        let mut mask_stroke_count = 0;
        let mut mask_render_cost = 0;
        for layer in &self.layers {
            if !identifier(&layer.id)
                || !ids.insert(&layer.id)
                || !identifier(&layer.scene_id)
                || layer.attachment.as_ref().is_some_and(|id| !identifier(id))
            {
                return invalid("layer identifiers must be valid and globally unique");
            }
            if layer.name.len() > 512
                || layer.name.chars().any(char::is_control)
                || !layer.opacity.is_finite()
                || !(0.0..=1.0).contains(&layer.opacity)
            {
                return invalid("layer name or opacity is invalid");
            }
            if layer.active.is_some_and(|range| {
                !range.start.is_finite()
                    || !range.end.is_finite()
                    || range.start < 0.0
                    || range.start >= range.end
            }) {
                return invalid("layer active interval requires finite nonnegative start < end");
            }
            if layer.clip.is_some() && layer.clip_shape.is_some() {
                return invalid("specify either legacy rectangular clip or clip_shape, not both");
            }
            if let Some(shape) = &layer.clip_shape {
                shape.validate()?;
                mask_key_count += shape.key_count();
                if mask_key_count > MAX_TOTAL_MASK_KEYS {
                    return invalid("artwork exceeds 8192 animated mask keys");
                }
                if let ClipShape::Painted { strokes, .. } = shape {
                    mask_render_cost += strokes.iter().map(MaskStroke::render_cost).sum::<usize>();
                    if mask_render_cost > MAX_TOTAL_MASK_RENDER_COST {
                        return invalid("artwork exceeds painted mask render work budget");
                    }
                    mask_stroke_count += strokes.len();
                    if mask_stroke_count > MAX_TOTAL_MASK_STROKES {
                        return invalid("artwork exceeds 4096 mask strokes");
                    }
                    point_count += strokes
                        .iter()
                        .map(|stroke| stroke.points.len())
                        .sum::<usize>();
                }
                if let ClipShape::Polygon { points, .. } = shape {
                    point_count += points.len();
                }
                if point_count > MAX_TOTAL_POINTS {
                    return invalid("total artwork point budget exceeded");
                }
            }
            if let Some(clip) = layer.clip {
                clip.validate()?;
            }
            match &layer.content {
                Content::Image { asset_id, rect } => {
                    if !identifier(asset_id) {
                        return invalid("invalid image asset identifier");
                    }
                    rect.validate()?;
                }
                Content::Rectangle { rect, .. } | Content::Ellipse { rect, .. } => {
                    rect.validate()?
                }
                Content::Stroke { points, width, .. } => {
                    point_count += points.len();
                    if points.len() < 2
                        || points.len() > MAX_POINTS
                        || point_count > MAX_TOTAL_POINTS
                        || !width.is_finite()
                        || !(0.1..=4096.0).contains(width)
                        || points.iter().any(|p| !coordinate(p.x) || !coordinate(p.y))
                    {
                        return invalid("stroke requires 2–65536 bounded points, width 0.1–4096, and a total point budget of 262144");
                    }
                }
            }
        }
        self.validate_groups()?;
        Ok(())
    }
    /// Apply a typed edit transactionally. Reference checks belong to the project boundary.
    pub fn apply(&mut self, command: Command) -> Result<(), Error> {
        let mut next = self.clone();
        if next.order.is_empty()
            && next.groups.is_empty()
            && !matches!(
                &command,
                Command::Add(_)
                    | Command::Replace(_)
                    | Command::Remove { .. }
                    | Command::Reorder { .. }
            )
        {
            next.order = next.root_order();
        }
        match command {
            Command::CreateGroup(group) => {
                next.order.push(ArtUnit::Group(group.id.clone()));
                next.groups.push(group);
            }
            Command::ReplaceGroup(group) => {
                let Some(target) = next.groups.iter_mut().find(|g| g.id == group.id) else {
                    return invalid("group does not exist");
                };
                *target = group;
            }
            Command::Move {
                unit,
                parent,
                index,
            } => {
                next.detach(&unit)?;
                let siblings = next.siblings(parent.as_deref())?;
                if index > siblings.len() {
                    return invalid("sibling index out of bounds");
                }
                siblings.insert(index, unit);
            }
            Command::Ungroup { id } => {
                let Some(index) = next.groups.iter().position(|g| g.id == id) else {
                    return invalid("group does not exist");
                };
                let (parent, position) = next.detach(&ArtUnit::Group(id))?;
                let group = next.groups.remove(index);
                next.siblings(parent.as_deref())?
                    .splice(position..position, group.children);
            }
            Command::Add(layer) => {
                if !next.order.is_empty() || !next.groups.is_empty() {
                    next.order.push(ArtUnit::Layer(layer.id.clone()));
                }
                next.layers.push(layer);
            }
            Command::Replace(layer) => {
                let Some(target) = next.layers.iter_mut().find(|v| v.id == layer.id) else {
                    return invalid("layer does not exist");
                };
                *target = layer;
            }
            Command::Remove { id } => {
                let Some(index) = next.layers.iter().position(|v| v.id == id) else {
                    return invalid("layer does not exist");
                };
                if !next.order.is_empty() || !next.groups.is_empty() {
                    next.detach(&ArtUnit::Layer(id))?;
                }
                next.layers.remove(index);
            }
            Command::Reorder { id, index } => {
                if index >= next.layers.len() {
                    return invalid("layer order index out of bounds");
                }
                if !next.groups.is_empty() || !next.order.is_empty() {
                    let unit = ArtUnit::Layer(id);
                    let (parent, _) = next.detach(&unit)?;
                    let siblings = next.siblings(parent.as_deref())?;
                    if index > siblings.len() {
                        return invalid("sibling order index out of bounds");
                    }
                    siblings.insert(index, unit);
                    next.validate()?;
                    *self = next;
                    return Ok(());
                }
                let Some(old) = next.layers.iter().position(|v| v.id == id) else {
                    return invalid("layer does not exist");
                };
                let layer = next.layers.remove(old);
                next.layers.insert(index, layer);
            }
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub enum Command {
    CreateGroup(Group),
    ReplaceGroup(Group),
    Move {
        unit: ArtUnit,
        parent: Option<String>,
        index: usize,
    },
    Ungroup {
        id: String,
    },
    Add(Layer),
    Replace(Layer),
    Remove {
        id: String,
    },
    Reorder {
        id: String,
        index: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    fn group(id: &str) -> Group {
        Group {
            id: id.into(),
            scene_id: "intro".into(),
            plane: Plane::Foreground,
            name: id.into(),
            visible: true,
            opacity: 0.5,
            blend: BlendMode::Normal,
            children: Vec::new(),
        }
    }
    #[test]
    fn legacy_order_nested_roundtrip_and_ungroup_preserve_children() {
        let mut doc = Document {
            layers: vec![layer()],
            ..Document::default()
        };
        let legacy = serde_json::to_value(&doc).unwrap();
        assert!(legacy.get("groups").is_none());
        assert!(legacy.get("order").is_none());
        doc.apply(Command::CreateGroup(group("outer"))).unwrap();
        doc.apply(Command::CreateGroup(group("inner"))).unwrap();
        doc.apply(Command::Move {
            unit: ArtUnit::Layer("stable".into()),
            parent: Some("inner".into()),
            index: 0,
        })
        .unwrap();
        doc.apply(Command::Move {
            unit: ArtUnit::Group("inner".into()),
            parent: Some("outer".into()),
            index: 0,
        })
        .unwrap();
        let restored: Document =
            serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
        assert_eq!(doc, restored);
        let steps = doc.composition("intro", Plane::Foreground, 0.0).unwrap();
        assert_eq!(steps.len(), 5);
        assert!(matches!(steps[0], CompositionStep::EnterGroup(g) if g.id == "outer"));
        assert!(matches!(steps[2], CompositionStep::Layer(l) if l.id == "stable"));
        doc.apply(Command::Ungroup { id: "inner".into() }).unwrap();
        assert_eq!(
            doc.groups[0].children,
            vec![ArtUnit::Layer("stable".into())]
        );
        doc.apply(Command::Ungroup { id: "outer".into() }).unwrap();
        assert_eq!(doc.order, vec![ArtUnit::Layer("stable".into())]);
        assert_eq!(doc.layers.len(), 1);
    }
    #[test]
    fn invalid_hierarchy_edits_are_atomic_and_hidden_ancestors_hide_timed_leaves() {
        let mut doc = Document {
            layers: vec![layer()],
            ..Document::default()
        };
        doc.apply(Command::CreateGroup(group("outer"))).unwrap();
        doc.apply(Command::Move {
            unit: ArtUnit::Layer("stable".into()),
            parent: Some("outer".into()),
            index: 0,
        })
        .unwrap();
        let before = doc.clone();
        for command in [
            Command::Move {
                unit: ArtUnit::Group("outer".into()),
                parent: Some("outer".into()),
                index: 0,
            },
            Command::Move {
                unit: ArtUnit::Layer("stable".into()),
                parent: Some("missing".into()),
                index: 0,
            },
            Command::Move {
                unit: ArtUnit::Layer("stable".into()),
                parent: None,
                index: 42,
            },
        ] {
            assert!(doc.apply(command).is_err());
            assert_eq!(doc, before);
        }
        let mut bad = group("outer");
        bad.plane = Plane::Background;
        assert!(doc.apply(Command::ReplaceGroup(bad)).is_err());
        assert_eq!(doc, before);
        let mut duplicate = doc.clone();
        duplicate.order.push(ArtUnit::Layer("stable".into()));
        assert!(duplicate.validate().is_err());
        let mut orphan = doc.clone();
        orphan.groups[0].children.clear();
        assert!(orphan.validate().is_err());
        doc.groups[0].visible = false;
        assert!(doc
            .composition("intro", Plane::Foreground, 5.0)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn depth_and_child_limits_are_checked_before_traversal() {
        let mut doc = Document::default();
        for n in 0..MAX_GROUP_DEPTH {
            let id = format!("g{n}");
            doc.apply(Command::CreateGroup(group(&id))).unwrap();
            if n > 0 {
                doc.apply(Command::Move {
                    unit: ArtUnit::Group(id),
                    parent: Some(format!("g{}", n - 1)),
                    index: 0,
                })
                .unwrap();
            }
        }
        doc.apply(Command::CreateGroup(group("too-deep"))).unwrap();
        let before = doc.clone();
        assert!(doc
            .apply(Command::Move {
                unit: ArtUnit::Group("too-deep".into()),
                parent: Some("g7".into()),
                index: 0
            })
            .is_err());
        assert_eq!(doc, before);
        let mut bad = group("bad");
        bad.children = vec![ArtUnit::Layer("missing".into()); MAX_GROUP_CHILDREN + 1];
        assert!(doc.apply(Command::CreateGroup(bad)).is_err());
        assert_eq!(doc, before);
    }
    fn soft_mask() -> ClipShape {
        ClipShape::SoftEllipse {
            rect: Rect {
                x: 90.0,
                y: 90.0,
                width: 10.0,
                height: 10.0,
            },
            feather: 0.0,
            keys: vec![
                MaskKey {
                    time: 1.0,
                    rect: Rect {
                        x: 0.0,
                        y: 2.0,
                        width: 10.0,
                        height: 20.0,
                    },
                    feather: 0.0,
                },
                MaskKey {
                    time: 3.0,
                    rect: Rect {
                        x: 20.0,
                        y: 6.0,
                        width: 30.0,
                        height: 40.0,
                    },
                    feather: 1.0,
                },
            ],
        }
    }
    #[test]
    fn animated_soft_masks_sample_linear_geometry_and_hold_endpoints_after_roundtrip() {
        let shape = soft_mask();
        shape.validate().unwrap();
        let encoded = serde_json::to_string(&shape).unwrap();
        let restored: ClipShape = serde_json::from_str(&encoded).unwrap();
        assert_eq!(shape, restored);
        for time in [3.0, 0.0, 2.0, 1.0, 4.0, 2.0] {
            assert_eq!(shape.sample_ellipse(time), restored.sample_ellipse(time));
        }
        assert_eq!(
            shape.sample_ellipse(2.0),
            Some((
                Rect {
                    x: 10.0,
                    y: 4.0,
                    width: 20.0,
                    height: 30.0
                },
                0.5
            ))
        );
        assert_eq!(shape.sample_ellipse(0.0), shape.sample_ellipse(1.0));
        assert_eq!(shape.sample_ellipse(4.0), shape.sample_ellipse(3.0));
        let ClipShape::SoftEllipse { rect, feather, .. } = shape else {
            panic!("soft mask")
        };
        let static_shape = ClipShape::SoftEllipse {
            rect,
            feather,
            keys: Vec::new(),
        };
        assert_eq!(static_shape.sample_ellipse(20.0), Some((rect, feather)));
        assert_eq!(
            ClipShape::Ellipse { rect }.sample_ellipse(2.0),
            Some((rect, 0.0))
        );
    }
    #[test]
    fn malformed_soft_mask_keys_are_rejected_transactionally() {
        let mut source = layer();
        source.clip = None;
        source.clip_shape = Some(soft_mask());
        let mut document = Document {
            layers: vec![source.clone()],
            ..Document::default()
        };
        document.validate().unwrap();
        let before = document.clone();
        for case in 0..8 {
            let mut edited = source.clone();
            let Some(ClipShape::SoftEllipse {
                rect,
                feather,
                keys,
            }) = &mut edited.clip_shape
            else {
                panic!("soft mask")
            };
            match case {
                0 => *feather = f64::NAN,
                1 => *feather = 1.1,
                2 => keys[1].time = 1.0,
                3 => keys[0].time = -1.0,
                4 => keys[1].time = f64::INFINITY,
                5 => keys[0].rect.width = 0.0,
                6 => keys[0].feather = -0.1,
                _ => rect.x = f64::NAN,
            }
            assert!(document.apply(Command::Replace(edited)).is_err());
            assert_eq!(document, before);
        }
    }
    #[test]
    fn mask_key_limits_bound_individual_tracks_and_document_totals() {
        let mut source = layer();
        source.clip = None;
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let keys: Vec<_> = (0..MAX_MASK_KEYS)
            .map(|i| MaskKey {
                time: i as f64,
                rect,
                feather: 0.5,
            })
            .collect();
        source.clip_shape = Some(ClipShape::SoftEllipse {
            rect,
            feather: 0.5,
            keys: keys.clone(),
        });
        let mut document = Document::default();
        for i in 0..MAX_TOTAL_MASK_KEYS / MAX_MASK_KEYS {
            let mut next = source.clone();
            next.id = format!("layer-{i}");
            document.layers.push(next);
        }
        document.validate().unwrap();
        let before = document.clone();
        assert!(document.apply(Command::Add(source)).is_err());
        assert_eq!(document, before);
        let mut too_many = keys;
        too_many.push(MaskKey {
            time: MAX_MASK_KEYS as f64,
            rect,
            feather: 0.5,
        });
        assert!(ClipShape::SoftEllipse {
            rect,
            feather: 0.0,
            keys: too_many
        }
        .validate()
        .is_err());
    }
    #[test]
    fn image_mask_track_is_bounded_deterministic_and_backward_compatible() {
        let base = Rect { x: 0.0, y: 0.0, width: 10.0, height: 10.0 };
        let keys = vec![
            RectMaskKey { time: 0.0, rect: base },
            RectMaskKey { time: 2.0, rect: Rect { x: 20.0, width: 30.0, ..base } },
        ];
        let shape = ClipShape::Image { asset_id: "mask".into(), rect: base, channel: MaskChannel::Alpha, keys: keys.clone() };
        shape.validate().unwrap();
        assert_eq!(shape.key_count(), 2);
        assert!(shape.validate_scene_duration(1.0).is_err());
        assert!(shape.validate_scene_duration(2.0).is_ok());
        for time in [1.0, 3.0, -1.0, 1.0] {
            let actual = shape.sample_image_rect(time).unwrap();
            let t = time.clamp(0.0, 2.0) / 2.0;
            assert_eq!(actual.x, 20.0 * t);
            assert_eq!(actual.width, 10.0 + 20.0 * t);
        }
        for bad in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
            let mut invalid = shape.clone();
            if let ClipShape::Image { keys, .. } = &mut invalid { keys[1].time = bad; }
            assert!(invalid.validate().is_err());
        }
        let mut excessive = shape.clone();
        if let ClipShape::Image { keys, .. } = &mut excessive { *keys = vec![keys[0]; MAX_MASK_KEYS + 1]; }
        assert!(excessive.validate().is_err());
        let mut invalid_rect = shape.clone();
        if let ClipShape::Image { keys, .. } = &mut invalid_rect {
            keys[1].rect.width = 0.0;
        }
        assert!(invalid_rect.validate().is_err());
        let mut doc = Document::default();
        for index in 0..65 {
            let mut item = layer();
            item.id = format!("mask{index}");
            item.clip = None;
            item.clip_shape = Some(ClipShape::Image {
                asset_id: "mask".into(), rect: base, channel: MaskChannel::Alpha,
                keys: (0..MAX_MASK_KEYS).map(|index| RectMaskKey { time: index as f64, rect: base }).collect(),
            });
            doc.layers.push(item);
        }
        assert!(doc.validate().unwrap_err().to_string().contains("8192"));
        let json = serde_json::to_string(&shape).unwrap();
        assert_eq!(serde_json::from_str::<ClipShape>(&json).unwrap(), shape);
        let legacy: ClipShape = serde_json::from_str(r#"{"type":"image","asset_id":"mask","rect":{"x":0,"y":0,"width":10,"height":10}}"#).unwrap();
        assert_eq!(legacy.key_count(), 0);
        assert!(!serde_json::to_string(&legacy).unwrap().contains("keys"));
    }
    fn layer() -> Layer {
        Layer {
            id: "stable".into(),
            scene_id: "intro".into(),
            name: "Ink".into(),
            visible: true,
            plane: Plane::Foreground,
            active: None,
            opacity: 1.0,
            attachment: None,
            clip_shape: None,
            blend: BlendMode::Normal,
            clip: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            }),
            content: Content::Stroke {
                points: vec![Point { x: 1.0, y: 2.0 }, Point { x: 3.0, y: 4.0 }],
                color: Color {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 255,
                },
                width: 2.0,
            },
        }
    }
    #[test]
    fn active_intervals_are_half_open_and_require_finite_nonnegative_time() {
        let mut item = layer();
        assert!(item.is_active(0.0));
        for time in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(!item.is_active(time));
        }
        item.active = Some(TimeRange {
            start: 1.0,
            end: 2.0,
        });
        assert!(!item.is_active(0.999));
        assert!(item.is_active(1.0));
        assert!(item.is_active(1.999));
        assert!(!item.is_active(2.0));
        item.visible = false;
        assert!(!item.is_active(1.5));
    }
    #[test]
    fn invalid_intervals_are_rejected_transactionally() {
        let mut doc = Document {
            layers: vec![layer()],
            ..Document::default()
        };
        let before = doc.clone();
        for (start, end) in [
            (-1.0, 2.0),
            (1.0, 1.0),
            (2.0, 1.0),
            (f64::NAN, 2.0),
            (0.0, f64::INFINITY),
            (0.0, f64::NAN),
        ] {
            let mut item = layer();
            item.active = Some(TimeRange { start, end });
            assert!(doc.apply(Command::Replace(item)).is_err());
            assert_eq!(doc, before);
        }
    }
    #[test]
    fn atomic_commands_preserve_stable_ids_and_reject_invalid_changes() {
        let mut doc = Document::default();
        doc.apply(Command::Add(layer())).unwrap();
        let before = doc.clone();
        assert!(doc.apply(Command::Add(layer())).is_err());
        assert_eq!(doc, before);
        let mut changed = layer();
        changed.opacity = f64::NAN;
        assert!(doc.apply(Command::Replace(changed)).is_err());
        assert_eq!(doc, before);
        let mut second = layer();
        second.id = "second".into();
        doc.apply(Command::Add(second)).unwrap();
        doc.apply(Command::Reorder {
            id: "stable".into(),
            index: 1,
        })
        .unwrap();
        assert_eq!(doc.layers[1].id, "stable");
    }
    #[test]
    fn rejects_unbounded_geometry_and_point_budgets() {
        let mut item = layer();
        item.clip.as_mut().unwrap().width = 0.0;
        assert!(Document {
            layers: vec![item],
            ..Document::default()
        }
        .validate()
        .is_err());
        let mut item = layer();
        if let Content::Stroke { points, .. } = &mut item.content {
            points[1].x = f64::INFINITY;
        }
        assert!(Document {
            layers: vec![item],
            ..Document::default()
        }
        .validate()
        .is_err());
    }
    #[test]
    fn mask_shapes_roundtrip_and_reject_ambiguous_or_invalid_geometry() {
        let mut item = layer();
        item.clip = None;
        item.clip_shape = Some(ClipShape::Ellipse {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 20.0,
            },
        });
        item.blend = BlendMode::Multiply;
        let doc = Document {
            layers: vec![item.clone()],
            ..Document::default()
        };
        doc.validate().unwrap();
        let json = serde_json::to_string(&doc).unwrap();
        let restored: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(doc, restored);
        item.clip = Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        });
        assert!(Document {
            layers: vec![item.clone()],
            ..Document::default()
        }
        .validate()
        .is_err());
        item.clip = None;
        for points in [
            vec![
                Point { x: 0.0, y: 0.0 },
                Point { x: 2.0, y: 2.0 },
                Point { x: 0.0, y: 2.0 },
                Point { x: 2.0, y: 0.0 },
            ],
            vec![Point { x: 0.0, y: 0.0 }; 3],
            vec![
                Point {
                    x: f64::INFINITY,
                    y: 0.0
                };
                3
            ],
            vec![Point { x: 0.0, y: 0.0 }; 257],
        ] {
            assert!(ClipShape::Polygon {
                points,
                feather: 0.0
            }
            .validate()
            .is_err());
        }
        let concave = ClipShape::Polygon {
            feather: 0.0,
            points: vec![
                Point { x: 0.0, y: 0.0 },
                Point { x: 4.0, y: 0.0 },
                Point { x: 2.0, y: 2.0 },
                Point { x: 4.0, y: 4.0 },
                Point { x: 0.0, y: 4.0 },
            ],
        };
        concave.validate().unwrap();
        assert!(serde_json::from_str::<ClipShape>(r#"{"type":"unknown"}"#).is_err());
        assert!(serde_json::from_str::<BlendMode>(r#""overlay""#).is_err());
        assert!(ClipShape::Ellipse {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: -1.0,
                height: 1.0
            }
        }
        .validate()
        .is_err());
    }

    fn mask_stroke() -> MaskStroke {
        MaskStroke {
            points: vec![Point { x: 2.0, y: 3.0 }],
            radius: 4.0,
            erase: false,
            pressures: Vec::new(),
            hardness: 1.0,
        }
    }

    #[test]
    fn pressure_defaults_validation_and_geometry_are_bounded() {
        let legacy = r#"{"points":[{"x":2.0,"y":3.0}],"radius":4.0,"erase":false}"#;
        let stroke: MaskStroke = serde_json::from_str(legacy).unwrap();
        assert_eq!(stroke, mask_stroke());
        assert_eq!(serde_json::to_string(&stroke).unwrap(), legacy);
        for pressure in [f64::NAN, f64::INFINITY, -0.01, 1.01] {
            assert!(MaskStroke {
                pressures: vec![pressure],
                ..stroke.clone()
            }
            .validate()
            .is_err());
        }
        for hardness in [f64::NAN, f64::INFINITY, -0.01, 1.01] {
            assert!(MaskStroke {
                hardness,
                ..stroke.clone()
            }
            .validate()
            .is_err());
        }
        assert!(MaskStroke {
            pressures: vec![0.5, 1.0],
            ..stroke.clone()
        }
        .validate()
        .is_err());
        let zero = MaskStroke {
            pressures: vec![0.0],
            ..stroke.clone()
        };
        assert!(zero.band_geometry(1.0).is_empty());
        let soft = MaskStroke {
            hardness: 0.0,
            ..stroke
        };
        assert_eq!(soft.coverage_bands().count(), 32);
        assert_eq!(soft.render_cost(), 64);
        assert!(soft
            .coverage_bands()
            .all(|(coverage, scale)| coverage > 0.0 && scale > 0.0));
    }

    #[test]
    fn soft_mask_expansion_has_a_document_work_budget() {
        let stroke = MaskStroke {
            points: vec![Point { x: 0.0, y: 0.0 }; MAX_MASK_STROKE_POINTS],
            hardness: 0.5,
            ..mask_stroke()
        };
        let mut item = layer();
        item.clip = None;
        item.clip_shape = Some(ClipShape::Painted {
            base_visible: true,
            strokes: vec![stroke; 9],
        });
        let doc = Document {
            layers: vec![item],
            ..Default::default()
        };
        assert!(doc
            .validate()
            .unwrap_err()
            .to_string()
            .contains("render work budget"));
    }

    #[test]
    fn pressure_sweep_uses_continuous_tangents_and_contained_disks() {
        let stroke = MaskStroke {
            points: vec![Point { x: 0.0, y: 0.0 }, Point { x: 20.0, y: 0.0 }],
            pressures: vec![0.25, 1.0],
            ..mask_stroke()
        };
        let geometry = stroke.band_geometry(1.0);
        assert_eq!(geometry.len(), 3);
        assert!(matches!(
            geometry[0],
            MaskBrushPrimitive::Circle { radius: 1.0, .. }
        ));
        assert!(matches!(
            geometry[1],
            MaskBrushPrimitive::Circle { radius: 4.0, .. }
        ));
        let MaskBrushPrimitive::Polygon { points } = &geometry[2] else {
            panic!("missing tangent");
        };
        assert!(points.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
        let contained = MaskStroke {
            points: vec![Point { x: 0.0, y: 0.0 }, Point { x: 1.0, y: 0.0 }],
            ..stroke
        };
        assert_eq!(contained.band_geometry(1.0).len(), 2);
    }

    #[test]
    fn painted_mask_roundtrip_and_atomic_invalid_edit() {
        let mut item = layer();
        item.clip = None;
        item.clip_shape = Some(ClipShape::Painted {
            base_visible: false,
            strokes: vec![
                mask_stroke(),
                MaskStroke {
                    erase: true,
                    ..mask_stroke()
                },
            ],
        });
        let mut doc = Document {
            layers: vec![item.clone()],
            ..Default::default()
        };
        doc.validate().unwrap();
        let encoded = serde_json::to_string(&doc).unwrap();
        assert_eq!(doc, serde_json::from_str::<Document>(&encoded).unwrap());
        let before = doc.clone();
        item.clip_shape = Some(ClipShape::Painted {
            base_visible: true,
            strokes: vec![MaskStroke {
                radius: f64::NAN,
                ..mask_stroke()
            }],
        });
        assert!(doc.apply(Command::Replace(item)).is_err());
        assert_eq!(doc, before);
        assert!(serde_json::from_str::<ClipShape>(
            r#"{"type":"painted","strokes":[],"unexpected":true}"#
        )
        .is_err());
    }

    #[test]
    fn painted_mask_rejects_invalid_and_excessive_work() {
        for radius in [f64::NAN, f64::INFINITY, -1.0, 0.0, MAX_MASK_RADIUS + 1.0] {
            assert!(MaskStroke {
                radius,
                ..mask_stroke()
            }
            .validate()
            .is_err());
        }
        for points in [
            vec![],
            vec![Point {
                x: f64::INFINITY,
                y: 0.0,
            }],
            vec![Point {
                x: MAX_COORDINATE + 1.0,
                y: 0.0,
            }],
            vec![Point { x: 0.0, y: 0.0 }; MAX_MASK_STROKE_POINTS + 1],
        ] {
            assert!(MaskStroke {
                points,
                ..mask_stroke()
            }
            .validate()
            .is_err());
        }
        assert!(ClipShape::Painted {
            base_visible: true,
            strokes: vec![mask_stroke(); MAX_MASK_STROKES + 1],
        }
        .validate()
        .is_err());
        let mut item = layer();
        item.clip = None;
        item.clip_shape = Some(ClipShape::Painted {
            base_visible: false,
            strokes: vec![mask_stroke(); MAX_MASK_STROKES],
        });
        let mut doc = Document::default();
        for index in 0..=MAX_TOTAL_MASK_STROKES / MAX_MASK_STROKES {
            item.id = format!("paint-{index}");
            doc.layers.push(item.clone());
        }
        assert!(doc
            .validate()
            .unwrap_err()
            .to_string()
            .contains("4096 mask strokes"));
        let long = MaskStroke {
            points: vec![Point { x: 0.0, y: 0.0 }; MAX_MASK_STROKE_POINTS],
            ..mask_stroke()
        };
        item.clip_shape = Some(ClipShape::Painted {
            base_visible: false,
            strokes: vec![long.clone(); MAX_TOTAL_POINTS / MAX_MASK_STROKE_POINTS],
        });
        // This mask fits individually; the layer's two content points exceed
        // the shared document budget and must still be rejected.
        item.clip_shape.as_ref().unwrap().validate().unwrap();
        assert!(Document {
            layers: vec![item],
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(ClipShape::Painted {
            base_visible: false,
            strokes: vec![long; MAX_TOTAL_POINTS / MAX_MASK_STROKE_POINTS + 1],
        }
        .validate()
        .is_err());
    }
}
