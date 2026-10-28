//! Bounded native geometry attached to existing hand joints. The catalog types,
//! validation and grip contract live in `motion_rig::props` (ORK-G1-05); this
//! module edits script source and maps errors to the editor's Finnish messages.
use motion_rig::{HandSide, PropError, RigDefinition};
use std::sync::OnceLock;

#[cfg(test)]
use motion_rig::MAX_HAND_PROP_CATALOG_BYTES as MAX_CATALOG_BYTES;
const CATALOG: &str = include_str!("../../../examples/stick-studio/hand-props.json");

pub(crate) type Placement = motion_rig::GripPlacement;
pub(crate) type Item = motion_rig::HandPropItem;
pub(crate) type Catalog = motion_rig::HandPropCatalog;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Hand {
    #[default]
    Left,
    Right,
}
impl Hand {
    fn side(self) -> HandSide {
        match self {
            Self::Left => HandSide::Left,
            Self::Right => HandSide::Right,
        }
    }
    pub(crate) fn prefix(self) -> &'static str {
        self.side().part_prefix()
    }
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Left => "Vasen käsi",
            Self::Right => "Oikea käsi",
        }
    }
}

fn message(error: PropError) -> String {
    match error {
        PropError::TooLarge => "Esineluettelo ylittää 128 KiB:n rajan.".into(),
        PropError::Syntax(e) => format!("Esineluettelon rakenne: {e}"),
        PropError::Version => "Esineluettelo tarvitsee version 1 ja 1–32 esinettä.".into(),
        PropError::Item => "Esineen nimi, koko tai osamäärä ei kelpaa.".into(),
        PropError::Part => {
            "Esine tarvitsee paikallisen grip-nivelen ja rajatun vektorikuvituksen.".into()
        }
        PropError::Geometry { item, source } => format!("Esine {item}: {source}"),
        PropError::Placement => {
            "Otteen siirtymä saa olla ±100, kierto ±180° ja koko 0,25–3.".into()
        }
        PropError::MissingHand(joint) => format!("Hahmolta puuttuu {joint}-nivel."),
        PropError::HandOccupied => "Tässä kädessä on hahmon oma esine. Valitse toinen käsi.".into(),
        PropError::Rig(e) => format!("Otteen hahmorakenne: {e}"),
        PropError::Screen(item) => format!("Esineen {item} näyttöpaikka ei kelpaa."),
    }
}

pub(crate) fn catalog() -> Result<&'static Catalog, String> {
    static INSTANCE: OnceLock<Result<Catalog, String>> = OnceLock::new();
    INSTANCE
        .get_or_init(|| Catalog::decode(CATALOG).map_err(message))
        .as_ref()
        .map_err(Clone::clone)
}

pub(crate) fn hand_problem(rig: &RigDefinition, hand: Hand) -> Option<String> {
    motion_rig::hand_availability(rig, hand.side())
        .err()
        .map(message)
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
    placement.validate().map_err(message)?;
    let mut script = parse(source)?;
    let rig = character_rig(&mut script, actor)?;
    motion_rig::attach_hand_prop(rig, hand.side(), item, placement).map_err(message)?;
    serialize(&script)
}
pub(crate) fn remove(source: &str, actor: &str, hand: Hand) -> Result<String, String> {
    let mut script = parse(source)?;
    let rig = character_rig(&mut script, actor)?;
    motion_rig::detach_hand_prop(rig, hand.side()).map_err(|e| match e {
        PropError::Rig(e) => e.to_string(),
        other => message(other),
    })?;
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
