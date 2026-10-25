//! Bounded native geometry attached to existing hand joints. Preview, animation
//! and export use the ordinary rig evaluator; this module only edits source data.
use motion_rig::{RigDefinition, RigJointDefinition, RigidPartDefinition};
use serde::Deserialize;
use std::{collections::BTreeSet, sync::OnceLock};

const MAX_CATALOG_BYTES: usize = 128 * 1024;
const CATALOG: &str = include_str!("../../../examples/stick-studio/hand-props.json");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Hand {
    #[default]
    Left,
    Right,
}
impl Hand {
    pub(crate) fn joint(self) -> &'static str {
        match self {
            Self::Left => "hand_l",
            Self::Right => "hand_r",
        }
    }
    pub(crate) fn prefix(self) -> &'static str {
        match self {
            Self::Left => "heldprop_l_",
            Self::Right => "heldprop_r_",
        }
    }
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Left => "Vasen käsi",
            Self::Right => "Oikea käsi",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Placement {
    pub x: f64,
    pub y: f64,
    pub rotation_degrees: f64,
    pub scale: f64,
}
impl Default for Placement {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            rotation_degrees: 0.0,
            scale: 1.0,
        }
    }
}
impl Placement {
    fn validate(self) -> Result<(), String> {
        if ![self.x, self.y, self.rotation_degrees, self.scale]
            .iter()
            .all(|v| v.is_finite())
            || self.x.abs() > 100.0
            || self.y.abs() > 100.0
            || self.rotation_degrees.abs() > 180.0
            || !(0.25..=3.0).contains(&self.scale)
        {
            return Err("Otteen siirtymä saa olla ±100, kierto ±180° ja koko 0,25–3.".into());
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Item {
    pub id: String,
    pub name: String,
    pub width: f64,
    pub height: f64,
    pub grip: [f64; 2],
    pub parts: Vec<RigidPartDefinition>,
    pub grip_parts: Vec<RigidPartDefinition>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Catalog {
    schema_version: u32,
    pub items: Vec<Item>,
}
impl Catalog {
    pub(crate) fn decode(source: &str) -> Result<Self, String> {
        if source.len() > MAX_CATALOG_BYTES {
            return Err("Esineluettelo ylittää 128 KiB:n rajan.".into());
        }
        let result: Self =
            serde_json::from_str(source).map_err(|e| format!("Esineluettelon rakenne: {e}"))?;
        if result.schema_version != 1 || !(1..=32).contains(&result.items.len()) {
            return Err("Esineluettelo tarvitsee version 1 ja 1–32 esinettä.".into());
        }
        let mut ids = BTreeSet::new();
        for item in &result.items {
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
                return Err("Esineen nimi, koko tai osamäärä ei kelpaa.".into());
            }
            let parts: Vec<_> = item.parts.iter().chain(&item.grip_parts).cloned().collect();
            for part in &parts {
                if part.joint != "grip"
                    || part.id.len() > 64
                    || part.texture.is_some()
                    || !part.skin.is_empty()
                    || !part.variants.is_empty()
                    || part.smooth != 0
                    || part
                        .points
                        .iter()
                        .flatten()
                        .any(|n| !n.is_finite() || n.abs() > 512.0)
                    || part.ellipse.is_some_and(|e| {
                        [e.cx, e.cy, e.rx, e.ry]
                            .iter()
                            .any(|n| !n.is_finite() || n.abs() > 512.0)
                    })
                {
                    return Err(
                        "Esine tarvitsee paikallisen grip-nivelen ja rajatun vektorikuvituksen."
                            .into(),
                    );
                }
            }
            catalog_rig(parts)
                .validate()
                .map_err(|e| format!("Esine {}: {e}", item.id))?;
        }
        Ok(result)
    }
}
fn catalog_rig(parts: Vec<RigidPartDefinition>) -> RigDefinition {
    RigDefinition {
        version: 1,
        id: "hand_prop_catalog".into(),
        joints: vec![RigJointDefinition {
            id: "grip".into(),
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
pub(crate) fn catalog() -> Result<&'static Catalog, String> {
    static INSTANCE: OnceLock<Result<Catalog, String>> = OnceLock::new();
    INSTANCE
        .get_or_init(|| Catalog::decode(CATALOG))
        .as_ref()
        .map_err(Clone::clone)
}

pub(crate) fn hand_problem(rig: &RigDefinition, hand: Hand) -> Option<String> {
    if !rig.joints.iter().any(|j| j.id == hand.joint()) {
        return Some(format!("Hahmolta puuttuu {}-nivel.", hand.joint()));
    }
    if rig.parts.iter().any(|p| {
        p.joint == hand.joint()
            && ["scythe", "clipboard", "parcel", "cane"]
                .iter()
                .any(|prefix| p.id.starts_with(prefix))
    }) {
        return Some("Tässä kädessä on hahmon oma esine. Valitse toinen käsi.".into());
    }
    None
}
fn parse(source: &str) -> Result<motion_script::Script, String> {
    if source.len() > crate::project_history::MAX_SOURCE_BYTES {
        return Err("Käsikirjoitus ylittää muokkaimen 256 KiB:n rajan.".into());
    }
    motion_script::parse(source).map_err(|e| e.to_string())
}
fn serialize(script: &motion_script::Script) -> Result<String, String> {
    let source = motion_script::to_source(script).map_err(|e| e.to_string())?;
    parse(&source)?;
    Ok(source)
}
fn character_rig<'a>(
    script: &'a mut motion_script::Script,
    actor: &str,
) -> Result<&'a mut RigDefinition, String> {
    script
        .characters
        .get_mut(actor)
        .and_then(|c| c.rig.as_mut())
        .ok_or_else(|| "Valitse mukautettu nivelhahmo.".into())
}
pub(crate) fn attach(
    source: &str,
    actor: &str,
    hand: Hand,
    item: &Item,
    placement: Placement,
) -> Result<String, String> {
    placement.validate()?;
    let mut script = parse(source)?;
    let rig = character_rig(&mut script, actor)?;
    if let Some(error) = hand_problem(rig, hand) {
        return Err(error);
    }
    rig.parts.retain(|p| !p.id.starts_with(hand.prefix()));
    for original in item.parts.iter().chain(&item.grip_parts) {
        let mut part = original.clone();
        part.id = format!("{}{}_{}", hand.prefix(), item.id, original.id);
        part.joint = hand.joint().into();
        let (sin, cos) = placement.rotation_degrees.to_radians().sin_cos();
        let transform = |[x, y]: [f64; 2]| {
            let x = (x - item.grip[0]) * placement.scale;
            let y = (y - item.grip[1]) * placement.scale;
            [
                placement.x + cos * x - sin * y,
                placement.y + sin * x + cos * y,
            ]
        };
        if let Some(ellipse) = part.ellipse {
            if placement.rotation_degrees == 0.0 || ellipse.rx == ellipse.ry {
                let [cx, cy] = transform([ellipse.cx, ellipse.cy]);
                part.ellipse = Some(motion_rig::EllipseDefinition {
                    cx,
                    cy,
                    rx: ellipse.rx * placement.scale,
                    ry: ellipse.ry * placement.scale,
                });
            } else {
                part.points = (0..motion_rig::ELLIPSE_SEGMENTS)
                    .map(|i| {
                        let angle =
                            std::f64::consts::TAU * i as f64 / motion_rig::ELLIPSE_SEGMENTS as f64;
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
        rig.parts.push(part);
    }
    rig.validate()
        .map_err(|e| format!("Otteen hahmorakenne: {e}"))?;
    serialize(&script)
}
pub(crate) fn remove(source: &str, actor: &str, hand: Hand) -> Result<String, String> {
    let mut script = parse(source)?;
    let rig = character_rig(&mut script, actor)?;
    rig.parts.retain(|p| !p.id.starts_with(hand.prefix()));
    rig.validate().map_err(|e| e.to_string())?;
    serialize(&script)
}

#[derive(Default)]
pub(crate) struct State {
    pub item_index: usize,
    pub hand: Hand,
    pub placement: Placement,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> String {
        crate::stick_studio::load().unwrap().source
    }
    #[test]
    fn actual_catalog_attach_replace_remove_preserves_other_hand_and_source_data() {
        let before = source();
        let original = motion_script::parse(&before).unwrap();
        let catalog = catalog().unwrap();
        assert_eq!(catalog.items.len(), 9);
        for item in &catalog.items {
            let attached =
                attach(&before, "VEIKKO", Hand::Left, item, Placement::default()).unwrap();
            let parsed = motion_script::parse(&attached).unwrap();
            let rig = parsed.characters["VEIKKO"].rig.as_ref().unwrap();
            let prefix = format!("heldprop_l_{}_", item.id);
            assert_eq!(
                rig.parts
                    .iter()
                    .filter(|p| p.id.starts_with(&prefix))
                    .count(),
                item.parts.len() + item.grip_parts.len()
            );
            assert!(rig
                .parts
                .iter()
                .filter(|p| p.id.starts_with(&prefix))
                .all(|p| p.joint == "hand_l"));
            assert_eq!(
                serde_json::to_value(&parsed.scenes).unwrap(),
                serde_json::to_value(&original.scenes).unwrap()
            );
            assert_eq!(parsed.rig_clips, original.rig_clips);
            let replaced = attach(
                &attached,
                "VEIKKO",
                Hand::Left,
                &catalog.items[0],
                Placement {
                    rotation_degrees: 25.0,
                    scale: 1.1,
                    ..Default::default()
                },
            )
            .unwrap();
            let removed = remove(&replaced, "VEIKKO", Hand::Left).unwrap();
            let removed_script = motion_script::parse(&removed).unwrap();
            let mut expected = original.clone();
            expected
                .characters
                .get_mut("VEIKKO")
                .unwrap()
                .rig
                .as_mut()
                .unwrap()
                .parts
                .retain(|p| !p.id.starts_with(Hand::Left.prefix()));
            assert_eq!(
                serde_json::to_value(&removed_script).unwrap(),
                serde_json::to_value(&expected).unwrap()
            );
        }
    }
    #[test]
    fn occupied_hands_bad_geometry_and_nonfinite_placement_are_rejected() {
        let before = source();
        let item = &catalog().unwrap().items[0];
        for (actor, occupied, free) in [
            ("NOUTAJA", Hand::Right, Hand::Left),
            ("PASMA", Hand::Left, Hand::Right),
            ("KIIRE", Hand::Right, Hand::Left),
            ("SISU", Hand::Right, Hand::Left),
        ] {
            assert!(
                attach(&before, actor, occupied, item, Placement::default()).is_err(),
                "{actor}"
            );
            attach(&before, actor, free, item, Placement::default()).unwrap();
        }
        for placement in [
            Placement {
                x: f64::NAN,
                ..Default::default()
            },
            Placement {
                scale: 0.0,
                ..Default::default()
            },
            Placement {
                y: 101.0,
                ..Default::default()
            },
        ] {
            assert!(attach(&before, "VEIKKO", Hand::Left, item, placement).is_err());
        }
        let mut json: serde_json::Value = serde_json::from_str(CATALOG).unwrap();
        json["schema_version"] = 2.into();
        assert!(Catalog::decode(&json.to_string()).is_err());
        json["schema_version"] = 1.into();
        json["items"][0]["parts"][0]["joint"] = "outside".into();
        assert!(Catalog::decode(&json.to_string()).is_err());
        json["items"][0]["parts"][0]["joint"] = "grip".into();
        json["items"][0]["parts"][0]["texture"] = serde_json::json!({"asset":"external"});
        assert!(Catalog::decode(&json.to_string()).is_err());
        assert!(Catalog::decode(&" ".repeat(MAX_CATALOG_BYTES + 1)).is_err());
        assert!(attach(
            &(" ".repeat(crate::project_history::MAX_SOURCE_BYTES + 1)),
            "VEIKKO",
            Hand::Left,
            item,
            Placement::default()
        )
        .is_err());
    }
}
