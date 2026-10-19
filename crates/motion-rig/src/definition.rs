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
            // Reject collapsed polygons; concave polygons remain supported by CPU paths.
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
        let (rig, ids) = self.build()?;
        let world = rig.evaluate(&rig.rest_pose())?;
        self.parts
            .iter()
            .map(|part| {
                let transform = world[ids[part.joint.as_str()]];
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
}
