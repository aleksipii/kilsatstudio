//! Versioned project-owned rigid character definitions. No external assets or inference.
use crate::{Error, Joint, Rig, Transform2, Vec2};
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
}
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedPart {
    pub id: String,
    pub points: Vec<Vec2>,
    pub color: [u8; 4],
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
        Ok((rig, ids))
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
        let parts = self
            .parts
            .iter()
            .map(|part| (ids[part.joint.as_str()], part.clone()))
            .collect();
        let prepared = PreparedRig { rig, ids, parts };
        prepared.evaluate_parts(&[])?;
        Ok(prepared)
    }
}
/// A validated rig hierarchy that can be posed with per-joint offsets.
#[derive(Debug, Clone)]
pub struct PreparedRig {
    rig: Rig,
    ids: BTreeMap<String, usize>,
    parts: Vec<(usize, RigidPartDefinition)>,
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
            .map(|(joint, part)| {
                let transform = world[*joint];
                let points: Vec<_> = part
                    .points
                    .iter()
                    .map(|p| transform.point(Vec2::new(p[0], p[1])))
                    .collect();
                if points.iter().any(|p| {
                    !p.x.is_finite()
                        || !p.y.is_finite()
                        || p.x.abs() > 1_000_000.0
                        || p.y.abs() > 1_000_000.0
                }) {
                    return Err(Error::InvalidDefinition);
                }
                Ok(EvaluatedPart {
                    id: part.id.clone(),
                    points,
                    color: part.color,
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
}
