//! `acting:` profiles: how one character carries itself and moves.
//!
//! ```yaml
//! characters:
//!   PASMA:
//!     acting: proud                # a preset name, or a block:
//!   VEIKKO:
//!     acting:
//!       preset: tired              # optional starting point
//!       energy: 0.9
//! ```
//!
//! | Field | Range | Effect |
//! | --- | --- | --- |
//! | `posture` | -1–1 | Standing posture: negative slouches, positive stands tall. |
//! | `energy` | 0.5–1.5 | Size of gestures (1 = as drawn). |
//! | `expressiveness` | 0.5–1.5 | Multiplies every emotion's intensity. |
//! | `tempo` | 0.7–1.4 | Speed of gestures and walks when a story is compiled. |
//!
//! Posture, energy and expressiveness are applied by scene evaluation to the
//! rig layer, so they need a custom rig. `tempo` changes no evaluation: event
//! times and durations are written once, and the story compiler reads `tempo`
//! when it times gestures and walks.
use crate::Error;
use serde::{Deserialize, Deserializer, Serialize};

/// A resolved acting profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActingProfile {
    pub posture: f64,
    pub energy: f64,
    pub expressiveness: f64,
    pub tempo: f64,
}
impl ActingProfile {
    /// The profile every character had before profiles existed.
    pub const DEFAULT: Self = Self {
        posture: 0.0,
        energy: 1.0,
        expressiveness: 1.0,
        tempo: 1.0,
    };
    pub fn preset(name: &str) -> Option<Self> {
        ACTING_PRESETS
            .iter()
            .position(|n| *n == name)
            .map(|i| PRESET_VALUES[i])
    }
}
impl Default for ActingProfile {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Preset names, in the order the guide lists them.
pub const ACTING_PRESETS: [&str; 6] = ["calm", "lively", "nervous", "proud", "tired", "stoic"];
const PRESET_VALUES: [ActingProfile; 6] = [
    // calm: upright, unhurried, moderate gestures.
    ActingProfile {
        posture: 0.2,
        energy: 0.85,
        expressiveness: 0.85,
        tempo: 0.9,
    },
    // lively: big, quick and open.
    ActingProfile {
        posture: 0.3,
        energy: 1.25,
        expressiveness: 1.2,
        tempo: 1.15,
    },
    // nervous: hunched, quick, every feeling shows.
    ActingProfile {
        posture: -0.35,
        energy: 1.1,
        expressiveness: 1.3,
        tempo: 1.25,
    },
    // proud: chest out, measured.
    ActingProfile {
        posture: 0.8,
        energy: 1.05,
        expressiveness: 0.9,
        tempo: 0.95,
    },
    // tired: slumped, small and slow.
    ActingProfile {
        posture: -0.7,
        energy: 0.7,
        expressiveness: 0.8,
        tempo: 0.8,
    },
    // stoic: still and contained.
    ActingProfile {
        posture: 0.4,
        energy: 0.6,
        expressiveness: 0.6,
        tempo: 0.9,
    },
];

pub const POSTURE_RANGE: std::ops::RangeInclusive<f64> = -1.0..=1.0;
pub const ENERGY_RANGE: std::ops::RangeInclusive<f64> = 0.5..=1.5;
pub const EXPRESSIVENESS_RANGE: std::ops::RangeInclusive<f64> = 0.5..=1.5;
pub const TEMPO_RANGE: std::ops::RangeInclusive<f64> = 0.7..=1.4;

/// An `acting:` block. A bare string is shorthand for `{preset: <name>}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ActingSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posture: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expressiveness: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tempo: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Block {
    #[serde(default)]
    preset: Option<String>,
    #[serde(default)]
    posture: Option<f64>,
    #[serde(default)]
    energy: Option<f64>,
    #[serde(default)]
    expressiveness: Option<f64>,
    #[serde(default)]
    tempo: Option<f64>,
}

impl<'de> Deserialize<'de> for ActingSpec {
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
                "acting must be a preset name ({}) or a block",
                ACTING_PRESETS.join(", ")
            )));
        }
        let block: Block = serde_yaml::from_value(value)
            .map_err(|error| serde::de::Error::custom(format!("acting: {error}")))?;
        Ok(Self {
            preset: block.preset,
            posture: block.posture,
            energy: block.energy,
            expressiveness: block.expressiveness,
            tempo: block.tempo,
        })
    }
}

impl ActingSpec {
    /// Shorthand for a preset.
    pub fn preset(name: &str) -> Self {
        Self {
            preset: Some(name.to_owned()),
            ..Self::default()
        }
    }
    /// The preset (or the default profile) with this block's fields over it,
    /// validated. `context` names the owner in errors.
    pub fn resolve(&self, context: &str) -> Result<ActingProfile, Error> {
        let mut profile = match &self.preset {
            Some(name) => ActingProfile::preset(name).ok_or_else(|| {
                Error::Unsupported(format!(
                    "{context}: acting preset {name}: supported presets are {}",
                    ACTING_PRESETS.join(", ")
                ))
            })?,
            None => ActingProfile::DEFAULT,
        };
        let fields = [
            ("posture", &mut profile.posture, self.posture, POSTURE_RANGE),
            ("energy", &mut profile.energy, self.energy, ENERGY_RANGE),
            (
                "expressiveness",
                &mut profile.expressiveness,
                self.expressiveness,
                EXPRESSIVENESS_RANGE,
            ),
            ("tempo", &mut profile.tempo, self.tempo, TEMPO_RANGE),
        ];
        for (name, slot, value, range) in fields {
            if let Some(value) = value {
                if !value.is_finite() || !range.contains(&value) {
                    return Err(Error::Validation(format!(
                        "{context}: acting.{name} must be {}–{}, got {value}",
                        range.start(),
                        range.end()
                    )));
                }
                *slot = value;
            }
        }
        Ok(profile)
    }
}

/// Acting profile of one character: its block, or the default profile.
pub fn character_acting(spec: Option<&ActingSpec>, actor: &str) -> Result<ActingProfile, Error> {
    match spec {
        Some(spec) => spec.resolve(&format!("character {actor}")),
        None => Ok(ActingProfile::DEFAULT),
    }
}

/// Default emotion intensity: the full expression, as before intensities existed.
pub const DEFAULT_INTENSITY: f64 = 1.0;

/// Check an emotion `intensity` (finite, 0–1).
pub fn validate_intensity(intensity: Option<f64>, context: &str) -> Result<(), Error> {
    match intensity {
        Some(value) if !value.is_finite() || !(0.0..=1.0).contains(&value) => {
            Err(Error::Validation(format!(
                "{context}: intensity must be 0–1 (0.3 subtle, 0.6 clear, 1 full), got {value}"
            )))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_is_a_preset_and_blocks_override_it() {
        let spec: ActingSpec = serde_yaml::from_str("tired").unwrap();
        assert_eq!(spec.resolve("x").unwrap(), PRESET_VALUES[4]);
        let spec: ActingSpec = serde_yaml::from_str("{preset: tired, energy: 1.2}").unwrap();
        let profile = spec.resolve("x").unwrap();
        assert_eq!(profile.energy, 1.2);
        assert_eq!(profile.posture, -0.7);
        let spec: ActingSpec = serde_yaml::from_str("{posture: 0.5}").unwrap();
        assert_eq!(
            spec.resolve("x").unwrap(),
            ActingProfile {
                posture: 0.5,
                ..ActingProfile::DEFAULT
            }
        );
    }

    #[test]
    fn every_preset_is_in_range() {
        for name in ACTING_PRESETS {
            let p = ActingSpec::preset(name).resolve(name).unwrap();
            let again = ActingSpec {
                preset: None,
                posture: Some(p.posture),
                energy: Some(p.energy),
                expressiveness: Some(p.expressiveness),
                tempo: Some(p.tempo),
            };
            assert_eq!(again.resolve(name).unwrap(), p, "{name}");
        }
    }

    #[test]
    fn bad_values_are_named() {
        let error = ActingSpec::preset("grumpy")
            .resolve("character A")
            .unwrap_err();
        assert!(matches!(error, Error::Unsupported(_)));
        assert!(error.to_string().contains("calm, lively"));
        let spec: ActingSpec = serde_yaml::from_str("{energy: 3}").unwrap();
        let error = spec.resolve("character A").unwrap_err().to_string();
        assert!(error.contains("acting.energy must be 0.5–1.5"), "{error}");
        assert!(serde_yaml::from_str::<ActingSpec>("{mood: 1}").is_err());
        assert!(serde_yaml::from_str::<ActingSpec>("[1]").is_err());
        assert!(validate_intensity(Some(1.2), "x").is_err());
        assert!(validate_intensity(Some(f64::NAN), "x").is_err());
        assert!(validate_intensity(Some(0.0), "x").is_ok());
        assert!(validate_intensity(None, "x").is_ok());
    }
}
