//! Persisted scene artwork and bounded raster resources shared by preview/export.
mod resource;
pub use resource::{load_image, ImageResource, MAX_IMAGE_BYTES, MAX_IMAGE_PIXELS, MAX_IMAGE_SIDE};
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
    Polygon {
        points: Vec<Point>,
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MaskKey {
    pub time: f64,
    pub rect: Rect,
    pub feather: f64,
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
pub const MAX_MASK_KEYS: usize = 128;
pub const MAX_TOTAL_MASK_KEYS: usize = 8192;
impl ClipShape {
    /// Scene-dependent bounds, after `Document::validate` checks the track itself.
    pub fn validate_scene_duration(&self, duration: f64) -> Result<(), Error> {
        if !duration.is_finite() || duration <= 0.0 {
            return invalid("mask scene duration must be finite and positive");
        }
        if let Self::SoftEllipse { keys, .. } = self {
            if keys.last().is_some_and(|key| key.time > duration) {
                return invalid("artwork mask key exceeds its scene duration");
            }
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
            Self::Polygon { .. } => None,
        }
    }
    pub fn validate(&self) -> Result<(), Error> {
        match self {
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
            Self::Polygon { points } => {
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
}
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
    pub fn validate(&self) -> Result<(), Error> {
        if self.layers.len() > MAX_LAYERS {
            return invalid("artwork exceeds 1024 layers");
        }
        let mut ids = BTreeSet::new();
        let mut point_count = 0;
        let mut mask_key_count = 0;
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
                if let ClipShape::SoftEllipse { keys, .. } = shape {
                    mask_key_count += keys.len();
                    if mask_key_count > MAX_TOTAL_MASK_KEYS {
                        return invalid("artwork exceeds 8192 animated mask keys");
                    }
                }
                if let ClipShape::Polygon { points } = shape {
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
        Ok(())
    }
    /// Apply a typed edit transactionally. Reference checks belong to the project boundary.
    pub fn apply(&mut self, command: Command) -> Result<(), Error> {
        let mut next = self.clone();
        match command {
            Command::Add(layer) => next.layers.push(layer),
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
                next.layers.remove(index);
            }
            Command::Reorder { id, index } => {
                if index >= next.layers.len() {
                    return invalid("layer order index out of bounds");
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
    Add(Layer),
    Replace(Layer),
    Remove { id: String },
    Reorder { id: String, index: usize },
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(Document { layers: vec![item] }.validate().is_err());
        let mut item = layer();
        if let Content::Stroke { points, .. } = &mut item.content {
            points[1].x = f64::INFINITY;
        }
        assert!(Document { layers: vec![item] }.validate().is_err());
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
            layers: vec![item.clone()]
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
            assert!(ClipShape::Polygon { points }.validate().is_err());
        }
        let concave = ClipShape::Polygon {
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
}
