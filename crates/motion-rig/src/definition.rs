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
}
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
/// Alpha 0 hides the part. A skinned part's variant polygon must keep its point count.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartVariantDefinition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<[f64; 2]>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<PartTextureDefinition>,
}
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
    /// Joint-local polygon vertices; source order is draw order.
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
}
fn valid_id(id: &str) -> bool {
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
fn validate_variants(part: &RigidPartDefinition) -> Result<(), Error> {
    if part.variants.len() > MAX_VARIANTS_PER_PART {
        return Err(Error::InvalidDefinition);
    }
    for (id, variant) in &part.variants {
        if !valid_id(id) {
            return Err(Error::InvalidDefinition);
        }
        if let Some(points) = &variant.points {
            validate_polygon(points)?;
            if !part.skin.is_empty() && points.len() != part.points.len() {
                return Err(Error::InvalidDefinition);
            }
        }
        if let Some(t) = &variant.texture {
            validate_texture(t)?;
        }
    }
    Ok(())
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
        let rig = Rig::new(joints).map_err(|_| Error::InvalidDefinition)?;
        let mut parts = BTreeSet::new();
        for part in &self.parts {
            if !valid_id(&part.id)
                || !parts.insert(&part.id)
                || !ids.contains_key(part.joint.as_str())
                || !(3..=64).contains(&part.points.len())
                || part
                    .points
                    .iter()
                    .flatten()
                    .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
            {
                return Err(Error::InvalidDefinition);
            }
            // Reject crossings and retraced edges so nonzero and even-odd renderers
            // agree on the accepted simple polygons, including concave outlines.
            if !simple_polygon(&part.points) {
                return Err(Error::InvalidDefinition);
            }
            validate_skin_and_texture(part, &ids)?;
            validate_variants(part)?;
            let area: f64 = part
                .points
                .iter()
                .zip(part.points.iter().cycle().skip(1))
                .take(part.points.len())
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
                PreparedPart {
                    joint,
                    definition: part.clone(),
                    skin,
                }
            })
            .collect();
        let acting = self.prepare_acting();
        let prepared = PreparedRig {
            rig,
            ids,
            parts,
            acting,
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
        let expressions = self
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
        PreparedActing {
            mouth,
            expressions,
            pupils,
            travel,
            blink,
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
}
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
}
#[derive(Debug, Clone)]
struct PreparedPart {
    joint: usize,
    definition: RigidPartDefinition,
    /// Per vertex: (joint, weight, bind matrix). Empty means rigid.
    skin: Vec<Vec<(usize, f64, Affine2)>>,
}
impl PreparedRig {
    pub fn joint_index(&self, id: &str) -> Option<usize> {
        self.ids.get(id).copied()
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
            .map(|index| self.evaluate_part(index, &world, None))
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
        Ok(self.rig.evaluate(&pose)?)
    }
    /// Evaluate one part, optionally with the variant at `variant` (an index into
    /// the part's sorted variant ids) overriding polygon, colour and texture.
    fn evaluate_part(
        &self,
        index: usize,
        world: &[Affine2],
        variant: Option<usize>,
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
        let color = variant.and_then(|v| v.color).unwrap_or(part.color);
        let texture_definition = variant
            .and_then(|v| v.texture.as_ref())
            .or(part.texture.as_ref());
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
                })
            }
        };
        Ok(EvaluatedPart {
            id: part.id.clone(),
            points,
            color,
            texture,
            skinned,
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
            .map(|index| self.evaluate_part(index, &world, picks[index]))
            .collect::<Result<Vec<_>, _>>()?;
        let mouth = match &self.acting.mouth {
            None => None,
            Some((part, table)) => {
                let mut shapes = Vec::new();
                for (slot, variant) in table.iter().enumerate() {
                    if let Some(variant) = variant {
                        shapes.push((
                            VISEME_NAMES[slot],
                            self.evaluate_part(*part, &world, Some(*variant))?,
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
                texture: None,
                skin: Vec::new(),
                variants: BTreeMap::new(),
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
}
