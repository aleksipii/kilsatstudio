//! Hand-held prop catalog and the grip contract (ORK-G1-05).
//!
//! Catalog items are bounded vector parts drawn around a local `grip` joint;
//! `item.grip` is the point held in the hand. Attaching an item copies its parts
//! onto a hand joint so that `item.grip` lands on the grip point: the hand
//! joint's origin offset by the placement. The ordinary rig evaluator then
//! moves the prop with the hand, so preview, export and every camera agree.
use crate::{
    Affine2, EllipseDefinition, RigDefinition, RigJointDefinition, RigidPartDefinition, Vec2,
    ELLIPSE_SEGMENTS,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Upper bound on a serialized catalog.
pub const MAX_HAND_PROP_CATALOG_BYTES: usize = 128 * 1024;
/// Local joint that catalog parts are drawn around.
pub const GRIP_JOINT: &str = "grip";
/// Part id prefixes of props a character's own design already holds; such a
/// hand is not free for a catalog item.
pub const CHARACTER_PROP_PREFIXES: [&str; 4] = ["scythe", "clipboard", "parcel", "cane"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HandSide {
    #[default]
    Left,
    Right,
}
impl HandSide {
    pub fn joint(self) -> &'static str {
        match self {
            Self::Left => "hand_l",
            Self::Right => "hand_r",
        }
    }
    /// Prefix of part ids added by `attach_hand_prop` for this hand.
    pub fn part_prefix(self) -> &'static str {
        match self {
            Self::Left => "heldprop_l_",
            Self::Right => "heldprop_r_",
        }
    }
}

/// Hand-local placement of the grip point: offset, rotation and uniform scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GripPlacement {
    pub x: f64,
    pub y: f64,
    pub rotation_degrees: f64,
    pub scale: f64,
}
impl Default for GripPlacement {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            rotation_degrees: 0.0,
            scale: 1.0,
        }
    }
}
impl GripPlacement {
    pub fn validate(self) -> Result<(), PropError> {
        if ![self.x, self.y, self.rotation_degrees, self.scale]
            .iter()
            .all(|v| v.is_finite())
            || self.x.abs() > 100.0
            || self.y.abs() > 100.0
            || self.rotation_degrees.abs() > 180.0
            || !(0.25..=3.0).contains(&self.scale)
        {
            return Err(PropError::Placement);
        }
        Ok(())
    }
    /// Map an item-local point (around `item.grip`) to hand-joint-local space.
    pub fn to_hand(self, grip: [f64; 2], [x, y]: [f64; 2]) -> [f64; 2] {
        let (sin, cos) = self.rotation_degrees.to_radians().sin_cos();
        let x = (x - grip[0]) * self.scale;
        let y = (y - grip[1]) * self.scale;
        [self.x + cos * x - sin * y, self.y + sin * x + cos * y]
    }
    /// World position of the grip point for a hand joint's world transform.
    pub fn grip_world(self, hand: Affine2) -> Vec2 {
        hand.point(Vec2::new(self.x, self.y))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandPropItem {
    pub id: String,
    pub name: String,
    pub width: f64,
    pub height: f64,
    /// Item-local point held in the hand.
    pub grip: [f64; 2],
    pub parts: Vec<RigidPartDefinition>,
    /// Parts drawn over the item (fingers wrapped around it).
    pub grip_parts: Vec<RigidPartDefinition>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandPropCatalog {
    pub schema_version: u32,
    pub items: Vec<HandPropItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PropError {
    #[error("hand prop catalog exceeds 128 KiB")]
    TooLarge,
    #[error("hand prop catalog structure: {0}")]
    Syntax(String),
    #[error("hand prop catalog needs schema_version 1 and 1..=32 items")]
    Version,
    #[error("hand prop item name, size or part count is invalid")]
    Item,
    #[error("hand prop parts must use the local grip joint and bounded untextured vector shapes")]
    Part,
    #[error("hand prop {item}: {source}")]
    Geometry { item: String, source: crate::Error },
    #[error("grip offset must be within ±100, rotation ±180° and scale 0.25..=3")]
    Placement,
    #[error("character has no {0} joint")]
    MissingHand(&'static str),
    #[error("this hand already holds the character's own prop")]
    HandOccupied,
    #[error("rig with the attached prop is invalid: {0}")]
    Rig(crate::Error),
}

impl HandPropCatalog {
    /// Parse and validate a JSON catalog.
    pub fn decode(source: &str) -> Result<Self, PropError> {
        if source.len() > MAX_HAND_PROP_CATALOG_BYTES {
            return Err(PropError::TooLarge);
        }
        let catalog: Self =
            serde_json::from_str(source).map_err(|e| PropError::Syntax(e.to_string()))?;
        catalog.validate()?;
        Ok(catalog)
    }
    pub fn validate(&self) -> Result<(), PropError> {
        if self.schema_version != 1 || !(1..=32).contains(&self.items.len()) {
            return Err(PropError::Version);
        }
        let mut ids = BTreeSet::new();
        for item in &self.items {
            if item.id.is_empty()
                || item.id.len() > 64
                || !item
                    .id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_')
                || !ids.insert(&item.id)
                || item.name.trim().is_empty()
                || item.name.len() > 128
                || ![item.width, item.height]
                    .iter()
                    .all(|n| n.is_finite() && *n > 0.0 && *n <= 512.0)
                || !item.grip.iter().all(|n| n.is_finite() && n.abs() <= 512.0)
                || item.parts.is_empty()
                || item.parts.len() + item.grip_parts.len() > 32
            {
                return Err(PropError::Item);
            }
            let parts: Vec<_> = item.parts.iter().chain(&item.grip_parts).cloned().collect();
            if !parts.iter().all(valid_catalog_part) {
                return Err(PropError::Part);
            }
            catalog_rig(parts)
                .validate()
                .map_err(|source| PropError::Geometry {
                    item: item.id.clone(),
                    source,
                })?;
        }
        Ok(())
    }
}

fn valid_catalog_part(part: &RigidPartDefinition) -> bool {
    let bounded = |n: &f64| n.is_finite() && n.abs() <= 512.0;
    part.joint == GRIP_JOINT
        && part.id.len() <= 64
        && part.texture.is_none()
        && part.skin.is_empty()
        && part.variants.is_empty()
        && part.smooth == 0
        && part.points.iter().flatten().all(bounded)
        && part
            .ellipse
            .is_none_or(|e| [e.cx, e.cy, e.rx, e.ry].iter().all(bounded))
}

fn catalog_rig(parts: Vec<RigidPartDefinition>) -> RigDefinition {
    RigDefinition {
        version: 1,
        id: "hand_prop_catalog".into(),
        joints: vec![RigJointDefinition {
            id: GRIP_JOINT.into(),
            parent: None,
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
        }],
        parts,
        ..Default::default()
    }
}

/// Why a hand cannot take a catalog item, if any.
pub fn hand_availability(rig: &RigDefinition, hand: HandSide) -> Result<(), PropError> {
    if !rig.joints.iter().any(|j| j.id == hand.joint()) {
        return Err(PropError::MissingHand(hand.joint()));
    }
    if rig.parts.iter().any(|p| {
        p.joint == hand.joint()
            && CHARACTER_PROP_PREFIXES
                .iter()
                .any(|prefix| p.id.starts_with(prefix))
    }) {
        return Err(PropError::HandOccupied);
    }
    Ok(())
}

/// Item parts re-expressed on the hand joint so `item.grip` lands on the
/// placement's grip point. Rotated non-circular ellipses become polygons.
pub fn grip_parts(
    item: &HandPropItem,
    hand: HandSide,
    placement: GripPlacement,
) -> Vec<RigidPartDefinition> {
    let transform = |p: [f64; 2]| placement.to_hand(item.grip, p);
    item.parts
        .iter()
        .chain(&item.grip_parts)
        .map(|original| {
            let mut part = original.clone();
            part.id = format!("{}{}_{}", hand.part_prefix(), item.id, original.id);
            part.joint = hand.joint().into();
            if let Some(ellipse) = part.ellipse {
                if placement.rotation_degrees == 0.0 || ellipse.rx == ellipse.ry {
                    let [cx, cy] = transform([ellipse.cx, ellipse.cy]);
                    part.ellipse = Some(EllipseDefinition {
                        cx,
                        cy,
                        rx: ellipse.rx * placement.scale,
                        ry: ellipse.ry * placement.scale,
                    });
                } else {
                    part.points = (0..ELLIPSE_SEGMENTS)
                        .map(|i| {
                            let angle = std::f64::consts::TAU * i as f64 / ELLIPSE_SEGMENTS as f64;
                            transform([
                                ellipse.cx + ellipse.rx * angle.cos(),
                                ellipse.cy + ellipse.ry * angle.sin(),
                            ])
                        })
                        .collect();
                    part.ellipse = None;
                }
            } else {
                part.points = part.points.iter().copied().map(transform).collect();
            }
            if let Some(outline) = &mut part.outline {
                outline.width *= placement.scale;
            }
            part
        })
        .collect()
}

/// Replace whatever this hand holds with `item` and validate the result.
/// The rig is left unchanged on error.
pub fn attach_hand_prop(
    rig: &mut RigDefinition,
    hand: HandSide,
    item: &HandPropItem,
    placement: GripPlacement,
) -> Result<(), PropError> {
    placement.validate()?;
    hand_availability(rig, hand)?;
    let mut next = rig.clone();
    next.parts.retain(|p| !p.id.starts_with(hand.part_prefix()));
    next.parts.extend(grip_parts(item, hand, placement));
    next.validate().map_err(PropError::Rig)?;
    *rig = next;
    Ok(())
}

/// Remove this hand's catalog prop and validate the result.
pub fn detach_hand_prop(rig: &mut RigDefinition, hand: HandSide) -> Result<(), PropError> {
    let mut next = rig.clone();
    next.parts.retain(|p| !p.id.starts_with(hand.part_prefix()));
    next.validate().map_err(PropError::Rig)?;
    *rig = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOG: &str = include_str!("../../../examples/stick-studio/hand-props.json");

    fn rig(extra_part: Option<&str>) -> RigDefinition {
        let joint = |id: &str, parent: Option<&str>, y: f64| RigJointDefinition {
            id: id.into(),
            parent: parent.map(Into::into),
            x: 0.0,
            y,
            rotation: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
        };
        let mut parts = vec![RigidPartDefinition {
            id: "body".into(),
            joint: "root".into(),
            points: vec![[-2.0, 0.0], [2.0, 0.0], [0.0, -4.0]],
            color: [0, 0, 0, 255],
            texture: None,
            skin: Vec::new(),
            variants: Default::default(),
            ellipse: None,
            smooth: 0,
            outline: None,
        }];
        if let Some(id) = extra_part {
            let mut part = parts[0].clone();
            part.id = id.into();
            part.joint = "hand_r".into();
            parts.push(part);
        }
        RigDefinition {
            version: 1,
            id: "test".into(),
            joints: vec![
                joint("root", None, 0.0),
                joint("hand_l", Some("root"), 10.0),
                joint("hand_r", Some("root"), 20.0),
            ],
            parts,
            ..Default::default()
        }
    }

    #[test]
    fn shipped_catalog_decodes_and_round_trips() {
        let catalog = HandPropCatalog::decode(CATALOG).unwrap();
        assert!(catalog.items.iter().any(|i| i.id == "puhelin"));
        let json = serde_json::to_string(&catalog).unwrap();
        assert_eq!(HandPropCatalog::decode(&json).unwrap(), catalog);
    }

    #[test]
    fn malformed_catalogs_are_rejected_with_specific_errors() {
        assert_eq!(
            HandPropCatalog::decode(&" ".repeat(MAX_HAND_PROP_CATALOG_BYTES + 1)),
            Err(PropError::TooLarge)
        );
        assert!(matches!(
            HandPropCatalog::decode("{"),
            Err(PropError::Syntax(_))
        ));
        let mut catalog = HandPropCatalog::decode(CATALOG).unwrap();
        catalog.schema_version = 2;
        assert_eq!(catalog.validate(), Err(PropError::Version));
        let mut catalog = HandPropCatalog::decode(CATALOG).unwrap();
        catalog.items[1].id = catalog.items[0].id.clone();
        assert_eq!(catalog.validate(), Err(PropError::Item));
        let mut catalog = HandPropCatalog::decode(CATALOG).unwrap();
        catalog.items[0].parts[0].joint = "hand_r".into();
        assert_eq!(catalog.validate(), Err(PropError::Part));
        let mut catalog = HandPropCatalog::decode(CATALOG).unwrap();
        catalog.items[0].grip = [f64::NAN, 0.0];
        assert_eq!(catalog.validate(), Err(PropError::Item));
    }

    #[test]
    fn attach_places_grip_on_hand_and_detach_removes_it() {
        let item = &HandPropCatalog::decode(CATALOG).unwrap().items[0];
        let placement = GripPlacement {
            x: 3.0,
            y: -2.0,
            rotation_degrees: 30.0,
            scale: 0.5,
        };
        let mut definition = rig(None);
        attach_hand_prop(&mut definition, HandSide::Right, item, placement).unwrap();
        let added: Vec<_> = definition
            .parts
            .iter()
            .filter(|p| p.id.starts_with("heldprop_r_"))
            .collect();
        assert_eq!(added.len(), item.parts.len() + item.grip_parts.len());
        assert!(added.iter().all(|p| p.joint == "hand_r"));
        let added = added.len();
        // The item's grip point maps to the placement offset in hand space.
        let at_grip = placement.to_hand(item.grip, item.grip);
        assert!((at_grip[0] - 3.0).abs() < 1e-12 && (at_grip[1] + 2.0).abs() < 1e-12);
        // Re-attaching replaces rather than duplicates.
        attach_hand_prop(&mut definition, HandSide::Right, item, placement).unwrap();
        assert_eq!(
            definition
                .parts
                .iter()
                .filter(|p| p.id.starts_with("heldprop_r_"))
                .count(),
            added
        );
        detach_hand_prop(&mut definition, HandSide::Right).unwrap();
        assert_eq!(definition, rig(None));
    }

    #[test]
    fn occupied_missing_hand_and_bad_placement_leave_rig_unchanged() {
        let item = &HandPropCatalog::decode(CATALOG).unwrap().items[0];
        let mut occupied = rig(Some("cane_staff"));
        let before = occupied.clone();
        assert_eq!(
            attach_hand_prop(
                &mut occupied,
                HandSide::Right,
                item,
                GripPlacement::default()
            ),
            Err(PropError::HandOccupied)
        );
        assert_eq!(occupied, before);
        assert!(hand_availability(&occupied, HandSide::Left).is_ok());
        let mut no_hands = rig(None);
        no_hands.joints.truncate(1);
        assert_eq!(
            hand_availability(&no_hands, HandSide::Left),
            Err(PropError::MissingHand("hand_l"))
        );
        for placement in [
            GripPlacement {
                scale: 4.0,
                ..Default::default()
            },
            GripPlacement {
                x: f64::NAN,
                ..Default::default()
            },
            GripPlacement {
                rotation_degrees: 181.0,
                ..Default::default()
            },
        ] {
            let mut definition = rig(None);
            assert_eq!(
                attach_hand_prop(&mut definition, HandSide::Left, item, placement),
                Err(PropError::Placement)
            );
            assert_eq!(definition, rig(None));
        }
    }
}
