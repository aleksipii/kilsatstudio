//! `principles:` blocks: animation principles for a character or one gesture.
//!
//! ```yaml
//! characters:
//!   mika:
//!     principles: cartoon          # a preset name, or a block:
//!   aino:
//!     principles:
//!       preset: subtle             # optional starting point
//!       overshoot: 1.6
//!       moving_hold: off           # on, off or a strength 0..1
//! ```
//!
//! Resolution: the defaults (the values the rig layer always used), then the
//! character's block, then a gesture's block. A `preset` restarts from that
//! preset; explicit fields override it. See `motion_animation::Principles`.
use crate::Error;
use motion_animation::{Principles, PRINCIPLE_PRESETS};
use serde::{Deserialize, Deserializer, Serialize};

/// A `principles:` block. A bare string is shorthand for `{preset: <name>}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PrinciplesSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anticipation: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overshoot: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_through: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlap: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub squash_stretch: Option<f64>,
    /// Strength of the moving hold in `0..=1`; `off` is 0 and `on` is 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moving_hold: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Block {
    #[serde(default)]
    preset: Option<String>,
    #[serde(default)]
    anticipation: Option<f64>,
    #[serde(default)]
    overshoot: Option<f64>,
    #[serde(default)]
    follow_through: Option<f64>,
    #[serde(default)]
    overlap: Option<f64>,
    #[serde(default)]
    squash_stretch: Option<f64>,
    #[serde(default, deserialize_with = "moving_hold")]
    moving_hold: Option<f64>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MovingHold {
    Switch(bool),
    Amount(f64),
    Word(String),
}

fn moving_hold<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    let value = MovingHold::deserialize(deserializer)
        .map_err(|_| serde::de::Error::custom("moving_hold must be on, off or a number 0..1"))?;
    match value {
        MovingHold::Switch(on) => Ok(Some(if on { 1.0 } else { 0.0 })),
        MovingHold::Amount(amount) => Ok(Some(amount)),
        MovingHold::Word(word) => match word.as_str() {
            "on" => Ok(Some(1.0)),
            "off" => Ok(Some(0.0)),
            _ => Err(serde::de::Error::custom(format!(
                "moving_hold {word}: use on, off or a number 0..1"
            ))),
        },
    }
}

impl<'de> Deserialize<'de> for PrinciplesSpec {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_yaml::Value::deserialize(deserializer)?;
        if let Some(name) = value.as_str() {
            return Ok(Self {
                preset: Some(name.to_owned()),
                ..Self::default()
            });
        }
        if !value.is_mapping() {
            return Err(serde::de::Error::custom(format!(
                "principles must be a preset name ({}) or a block",
                PRINCIPLE_PRESETS.join(", ")
            )));
        }
        let block: Block = serde_yaml::from_value(value)
            .map_err(|error| serde::de::Error::custom(format!("principles: {error}")))?;
        Ok(Self {
            preset: block.preset,
            anticipation: block.anticipation,
            overshoot: block.overshoot,
            follow_through: block.follow_through,
            overlap: block.overlap,
            squash_stretch: block.squash_stretch,
            moving_hold: block.moving_hold,
        })
    }
}

impl PrinciplesSpec {
    /// Apply this block on top of `base` and validate the result. `context`
    /// names the owner in errors (for example `character mika`).
    pub fn resolve(&self, base: Principles, context: &str) -> Result<Principles, Error> {
        let mut p = match &self.preset {
            Some(name) => Principles::preset(name).ok_or_else(|| {
                Error::Unsupported(format!(
                    "{context}: principles preset {name}: supported presets are {}",
                    PRINCIPLE_PRESETS.join(", ")
                ))
            })?,
            None => base,
        };
        let fields = [
            (&mut p.anticipation, self.anticipation),
            (&mut p.overshoot, self.overshoot),
            (&mut p.follow_through, self.follow_through),
            (&mut p.overlap, self.overlap),
            (&mut p.squash_stretch, self.squash_stretch),
            (&mut p.moving_hold, self.moving_hold),
        ];
        for (slot, value) in fields {
            if let Some(value) = value {
                *slot = value;
            }
        }
        p.validate()
            .map_err(|error| Error::Validation(format!("{context}: {error}")))?;
        Ok(p)
    }
}

/// Principles of one character: its block over the defaults.
pub fn character_principles(
    spec: Option<&PrinciplesSpec>,
    actor: &str,
) -> Result<Principles, Error> {
    match spec {
        Some(spec) => spec.resolve(Principles::DEFAULT, &format!("character {actor}")),
        None => Ok(Principles::DEFAULT),
    }
}

/// Principles of one gesture: its block over the character's principles. The
/// moving hold belongs to the character, between actions, so a gesture block
/// may not set it.
pub fn gesture_principles(
    character: Principles,
    spec: Option<&PrinciplesSpec>,
    actor: &str,
) -> Result<Principles, Error> {
    let Some(spec) = spec else {
        return Ok(character);
    };
    let context = format!("gesture of {actor}");
    if spec.moving_hold.is_some() {
        return Err(Error::Unsupported(format!(
            "{context}: moving_hold applies between actions; set it in the character's principles"
        )));
    }
    // A preset on a gesture keeps the character's moving hold.
    let mut p = spec.resolve(character, &context)?;
    p.moving_hold = character.moving_hold;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(yaml: &str) -> Result<PrinciplesSpec, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    #[test]
    fn preset_shorthand_block_and_overrides() {
        let cartoon = Principles::preset("cartoon").unwrap();
        assert_eq!(
            spec("cartoon")
                .unwrap()
                .resolve(Principles::DEFAULT, "x")
                .unwrap(),
            cartoon
        );
        let tuned = spec("{preset: cartoon, overshoot: 0.5, moving_hold: off}")
            .unwrap()
            .resolve(Principles::DEFAULT, "x")
            .unwrap();
        assert_eq!(tuned.overshoot, 0.5);
        assert_eq!(tuned.moving_hold, 0.0);
        assert_eq!(tuned.anticipation, cartoon.anticipation);
        for (word, value) in [("on", 1.0), ("off", 0.0), ("true", 1.0), ("0.4", 0.4)] {
            let s = spec(&format!("{{moving_hold: {word}}}")).unwrap();
            assert_eq!(s.moving_hold, Some(value), "{word}");
        }
        // Without a preset, fields override the base.
        let base = spec("{anticipation: 0.3}")
            .unwrap()
            .resolve(Principles::DEFAULT, "x")
            .unwrap();
        assert_eq!(
            base,
            Principles {
                anticipation: 0.3,
                ..Principles::DEFAULT
            }
        );
    }

    #[test]
    fn errors_name_the_field_range_and_presets() {
        let error = spec("{overshoot: 9}")
            .unwrap()
            .resolve(Principles::DEFAULT, "character mika")
            .unwrap_err()
            .to_string();
        assert!(error.contains("character mika"), "{error}");
        assert!(error.contains("principles.overshoot"), "{error}");
        assert!(error.contains("0..=3"), "{error}");
        let error = spec("wobbly")
            .unwrap()
            .resolve(Principles::DEFAULT, "character mika")
            .unwrap_err();
        assert!(matches!(&error, Error::Unsupported(m) if m.contains("subtle, cartoon, snappy")));
        assert!(spec("{bounce: 1}")
            .unwrap_err()
            .to_string()
            .contains("unknown field"));
        assert!(spec("{moving_hold: sometimes}")
            .unwrap_err()
            .to_string()
            .contains("moving_hold"));
        assert!(spec("[1, 2]").is_err());
    }

    #[test]
    fn gesture_block_layers_over_the_character() {
        let character = character_principles(Some(&spec("cartoon").unwrap()), "mika").unwrap();
        let snappy = gesture_principles(character, Some(&spec("snappy").unwrap()), "mika").unwrap();
        assert_eq!(
            snappy.overshoot,
            Principles::preset("snappy").unwrap().overshoot
        );
        assert_eq!(snappy.moving_hold, character.moving_hold);
        let tweak =
            gesture_principles(character, Some(&spec("{overlap: 0.2}").unwrap()), "mika").unwrap();
        assert_eq!(tweak.overlap, 0.2);
        assert_eq!(tweak.overshoot, character.overshoot);
        assert_eq!(
            gesture_principles(character, None, "mika").unwrap(),
            character
        );
        let error =
            gesture_principles(character, Some(&spec("{moving_hold: on}").unwrap()), "mika");
        assert!(matches!(error, Err(Error::Unsupported(_))));
        assert_eq!(
            character_principles(None, "x").unwrap(),
            Principles::DEFAULT
        );
    }

    #[test]
    fn serializes_back_to_an_equivalent_block() {
        let original = spec("{preset: snappy, squash_stretch: 0.1}").unwrap();
        let yaml = serde_yaml::to_string(&original).unwrap();
        assert_eq!(spec(&yaml).unwrap(), original);
    }
}
