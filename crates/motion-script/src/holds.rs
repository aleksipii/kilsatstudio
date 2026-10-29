//! `holds:` shorthand: a character holds a stick-studio catalog prop.
//!
//! ```yaml
//! characters:
//!   LIISA:
//!     asset: characters/liisa
//!     rig: { ... }          # a custom rig with hand_l / hand_r joints
//!     holds: {item: puhelin, hand: right}
//! ```
//!
//! The authored `rig` stays as written; `Character::resolved_rig` attaches the
//! item from `motion_rig::stick_studio_hand_props` with `attach_hand_prop` and
//! the default grip placement. Validation checks the result (so screen cues
//! may name the item's slot, for example `phone`) and scene evaluation uses
//! it, so the source round-trips as the short form. Attaching is idempotent:
//! a rig that already holds a catalog prop in that hand has it replaced.
use crate::{Character, Script};
use motion_rig::{HandSide, PropError, RigDefinition};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// A hand-held catalog prop named in MotionScript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeldProp {
    /// Catalog item id, for example `puhelin` or `tabletti`.
    pub item: String,
    /// `left` or `right`.
    pub hand: HandSide,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum HoldsError {
    #[error("holds needs a custom rig with hand_l / hand_r joints")]
    NoRig,
    #[error("holds: unknown item {item}; the stick-studio hand prop catalog has {known}")]
    UnknownItem { item: String, known: String },
    #[error("holds {item} in the {hand:?} hand: {source}")]
    Prop {
        item: String,
        hand: HandSide,
        source: PropError,
    },
}

impl Character {
    /// The rig this character is evaluated with: `rig` with the `holds:` prop
    /// attached, or `rig` unchanged when nothing is held.
    pub fn resolved_rig(&self) -> Result<Option<Cow<'_, RigDefinition>>, HoldsError> {
        let Some(held) = &self.holds else {
            return Ok(self.rig.as_ref().map(Cow::Borrowed));
        };
        let rig = self.rig.as_ref().ok_or(HoldsError::NoRig)?;
        let prop = |source| HoldsError::Prop {
            item: held.item.clone(),
            hand: held.hand,
            source,
        };
        let catalog = motion_rig::stick_studio_hand_props().map_err(prop)?;
        let item = catalog
            .items
            .iter()
            .find(|i| i.id == held.item)
            .ok_or_else(|| HoldsError::UnknownItem {
                item: held.item.clone(),
                known: catalog
                    .items
                    .iter()
                    .map(|i| i.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            })?;
        let mut resolved = rig.clone();
        motion_rig::attach_hand_prop(&mut resolved, held.hand, item, Default::default())
            .map_err(prop)?;
        Ok(Some(Cow::Owned(resolved)))
    }
}

/// Check a timed `hold` event: a custom rig with that hand free of the
/// character's own prop, and a catalog item when one is named.
pub(crate) fn validate_hold(
    actor: &str,
    character: &Character,
    hand: HandSide,
    item: Option<&str>,
) -> Result<(), crate::Error> {
    let rig = character
        .resolved_rig()
        .map_err(|e| crate::Error::Validation(format!("character {actor}: {e}")))?
        .ok_or_else(|| {
            crate::Error::Unsupported(format!(
                "hold for {actor}: timed hand props need a custom rig with hand joints"
            ))
        })?;
    let mut rig = rig.into_owned();
    held_rig(&mut rig, hand, item)
        .map_err(|e| crate::Error::Validation(format!("hold for {actor}: {e}")))
}

/// Put `item` (or nothing) into `hand` of an already resolved rig.
pub fn held_rig(
    rig: &mut RigDefinition,
    hand: HandSide,
    item: Option<&str>,
) -> Result<(), HoldsError> {
    let prop = |source| HoldsError::Prop {
        item: item.unwrap_or("nothing").into(),
        hand,
        source,
    };
    match item {
        None => motion_rig::detach_hand_prop(rig, hand).map_err(prop),
        Some(item) => {
            let catalog = motion_rig::stick_studio_hand_props().map_err(prop)?;
            let entry = catalog
                .items
                .iter()
                .find(|entry| entry.id == item)
                .ok_or_else(|| HoldsError::UnknownItem {
                    item: item.into(),
                    known: catalog
                        .items
                        .iter()
                        .map(|entry| entry.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                })?;
            motion_rig::attach_hand_prop(rig, hand, entry, Default::default()).map_err(prop)
        }
    }
}

impl Script {
    /// True when any character uses the `holds:` shorthand.
    pub fn has_held_props(&self) -> bool {
        self.characters.values().any(|c| c.holds.is_some())
    }
    /// Replace every character's `rig` with its resolved rig (see
    /// `Character::resolved_rig`). `holds` is kept, so resolving twice gives
    /// the same script. Evaluators call this once before preparing rigs.
    pub fn resolve_held_props(&mut self) -> Result<(), crate::Error> {
        for (id, character) in &mut self.characters {
            if character.holds.is_none() {
                continue;
            }
            let rig = character
                .resolved_rig()
                .map_err(|e| crate::Error::Validation(format!("character {id}: {e}")))?
                .map(Cow::into_owned);
            character.rig = rig;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn veikko_source() -> String {
        let project: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/stick-studio/characters/veikko.kilsat"
        ))
        .unwrap();
        project["source"].as_str().unwrap().to_owned()
    }
    fn with_holds(holds: &str) -> String {
        let mut value: serde_yaml::Value = serde_yaml::from_str(&veikko_source()).unwrap();
        value["characters"]["VEIKKO"]["holds"] = serde_yaml::from_str(holds).unwrap();
        serde_yaml::to_string(&value).unwrap()
    }
    fn screen_cue(script: &str) -> String {
        let mut value: serde_yaml::Value = serde_yaml::from_str(script).unwrap();
        value["scenes"][0]["composition"]["screens"] = serde_yaml::from_str(
            "[{actor: VEIKKO, slot: phone, at: 0.5, source: {type: image, asset: screens/demo}}]",
        )
        .unwrap();
        serde_yaml::to_string(&value).unwrap()
    }

    #[test]
    fn holds_parses_validates_and_round_trips_as_shorthand() {
        let source = with_holds("{item: puhelin, hand: right}");
        let script = crate::parse(&source).unwrap();
        let veikko = &script.characters["VEIKKO"];
        assert_eq!(
            veikko.holds,
            Some(HeldProp {
                item: "puhelin".into(),
                hand: HandSide::Right
            })
        );
        // The authored rig is unchanged; the resolved rig carries the phone and its slot.
        let authored = veikko.rig.as_ref().unwrap();
        assert!(authored.screens.is_empty());
        let resolved = veikko.resolved_rig().unwrap().unwrap();
        assert!(resolved
            .parts
            .iter()
            .any(|p| p.id == "heldprop_r_puhelin_screen" && p.joint == "hand_r"));
        assert_eq!(resolved.screens[0].id, "phone");
        // A screen cue may name the slot the shorthand provides.
        crate::parse(&screen_cue(&source)).unwrap();
        // Round trip keeps the short form.
        let text = crate::to_source(&script).unwrap();
        assert!(text.contains("holds:") && !text.contains("heldprop_"));
        let again = crate::parse(&text).unwrap();
        assert_eq!(again.characters["VEIKKO"].holds, veikko.holds);
        assert_eq!(again.characters["VEIKKO"].rig, veikko.rig);
        // Resolving is idempotent.
        let mut once = script.clone();
        once.resolve_held_props().unwrap();
        let mut twice = once.clone();
        twice.resolve_held_props().unwrap();
        assert_eq!(
            once.characters["VEIKKO"].rig,
            twice.characters["VEIKKO"].rig
        );
        assert_eq!(
            once.characters["VEIKKO"].rig.as_ref(),
            Some(resolved.as_ref())
        );
        once.validate().unwrap();
    }

    #[test]
    fn holds_errors_are_actionable() {
        let message = |holds: &str| crate::parse(&with_holds(holds)).unwrap_err().to_string();
        let unknown = message("{item: kirves, hand: left}");
        assert!(
            unknown.contains("unknown item kirves") && unknown.contains("puhelin"),
            "{unknown}"
        );
        // Unknown hands and fields are schema errors.
        assert!(matches!(
            crate::parse(&with_holds("{item: puhelin, hand: middle}")),
            Err(crate::Error::Syntax(_))
        ));
        assert!(matches!(
            crate::parse(&with_holds("{item: puhelin, hand: left, grip: 1}")),
            Err(crate::Error::Syntax(_))
        ));
        // Without a rig there are no hands to hold it.
        let mut value: serde_yaml::Value =
            serde_yaml::from_str(&with_holds("{item: puhelin, hand: left}")).unwrap();
        value["characters"]["VEIKKO"]
            .as_mapping_mut()
            .unwrap()
            .remove("rig");
        let error = crate::parse(&serde_yaml::to_string(&value).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("holds needs a custom rig"), "{error}");
        // A screen cue on a slot nobody holds is still rejected.
        let error = crate::parse(&screen_cue(&veikko_source()))
            .unwrap_err()
            .to_string();
        assert!(error.contains("no screen slot phone"), "{error}");
    }
}
