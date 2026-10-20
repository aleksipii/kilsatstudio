//! Versioned project-owned rigid character definitions. No external assets or inference.
use crate::{Affine2, Error, Joint, Rig, Transform2, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigDefinition {
    pub version: u32,
    pub id: String,
    pub joints: Vec<RigJointDefinition>,
    pub parts: Vec<RigidPartDefinition>,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
fn validate_skin_and_texture(
    part: &RigidPartDefinition,
    ids: &BTreeMap<&str, usize>,
) -> Result<(), Error> {
    if let Some(t) = &part.texture {
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
        if self.parts.iter().filter(|p| p.texture.is_some()).count() > MAX_TEXTURED_PARTS {
            return Err(Error::InvalidDefinition);
        }
        Ok((rig, ids))
    }
    /// Project image assets this rig's textures need.
    pub fn image_assets(&self) -> impl Iterator<Item = &str> {
        self.parts
            .iter()
            .filter_map(|p| p.texture.as_ref().map(|t| t.asset.as_str()))
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
        let prepared = PreparedRig { rig, ids, parts };
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
}
/// A validated rig hierarchy that can be posed with per-joint offsets.
#[derive(Debug, Clone)]
pub struct PreparedRig {
    rig: Rig,
    ids: BTreeMap<String, usize>,
    parts: Vec<PreparedPart>,
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
        let world = self.rig.evaluate(&pose)?;
        self.parts
            .iter()
            .map(|prepared| {
                let part = &prepared.definition;
                let transform = world[prepared.joint];
                let skinned = !prepared.skin.is_empty();
                let points: Vec<_> = if skinned {
                    // Deterministic linear blend of posed influence transforms.
                    part.points
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
                    part.points
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
                let texture = match &part.texture {
                    None => None,
                    Some(t) => {
                        let (mut x0, mut y0, mut x1, mut y1) =
                            (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                        for p in &part.points {
                            x0 = x0.min(p[0]);
                            y0 = y0.min(p[1]);
                            x1 = x1.max(p[0]);
                            y1 = y1.max(p[1]);
                        }
                        if x1 - x0 < 1e-9 || y1 - y0 < 1e-9 {
                            return Err(Error::InvalidDefinition);
                        }
                        let matrix = if skinned {
                            fit_affine(&part.points, &points).ok_or(Error::InvalidDefinition)?
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
                    color: part.color,
                    texture,
                    skinned,
                })
            })
            .collect()
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
            }],
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
