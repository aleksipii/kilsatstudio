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
//!       anticipation_frames: 5     # anticipation lasts 5 frames
//!       overlap_frames: 3          # forearms and head trail by 3 frames
//! ```
//!
//! Resolution: the defaults (the values the rig layer always used), then the
//! character's block, then a gesture's block. A `preset` restarts from that
//! preset; explicit fields override it. See `motion_animation::Principles`.
//!
//! Frame counts are converted per gesture with the project frame rate, see
//! [`timed_gesture_principles`]. Without them the anticipation and the overlap
//! are fixed fractions of the gesture (12 % and 8 % by default), so their length
//! in frames grows with the gesture.
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
    /// Length of the anticipation in frames, 1–24, whatever the gesture's
    /// length. Fitted into 2–30 % of the gesture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anticipation_frames: Option<u32>,
    /// Lag of child joints (forearms, head) in frames, 0–12, instead of the
    /// fractional `overlap`. At most 30 % of the gesture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlap_frames: Option<u32>,
}

/// Allowed `anticipation_frames` and `overlap_frames`.
pub const ANTICIPATION_FRAMES: std::ops::RangeInclusive<u32> = 1..=24;
pub const OVERLAP_FRAMES: std::ops::RangeInclusive<u32> = 0..=12;

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
    #[serde(default)]
    anticipation_frames: Option<u32>,
    #[serde(default)]
    overlap_frames: Option<u32>,
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
            anticipation_frames: block.anticipation_frames,
            overlap_frames: block.overlap_frames,
        })
    }
}

impl PrinciplesSpec {
    /// Apply this block on top of `base` and validate the result. `context`
    /// names the owner in errors (for example `character mika`).
    pub fn resolve(&self, base: Principles, context: &str) -> Result<Principles, Error> {
        if let Some(frames) = self.anticipation_frames {
            if !ANTICIPATION_FRAMES.contains(&frames) {
                return Err(Error::Validation(format!(
                    "{context}: principles.anticipation_frames must be in 1..=24, got {frames}"
                )));
            }
        }
        if let Some(frames) = self.overlap_frames {
            if !OVERLAP_FRAMES.contains(&frames) {
                return Err(Error::Validation(format!(
                    "{context}: principles.overlap_frames must be in 0..=12, got {frames}"
                )));
            }
            if self.overlap.is_some() {
                return Err(Error::Validation(format!(
                    "{context}: principles take either overlap or overlap_frames, not both"
                )));
            }
        }
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

/// The frame count that applies to a gesture: the gesture block's own, or
/// else the character's unless the gesture block restarts from a preset or
/// sets the fractional form (`fraction`).
fn inherited_frames(
    character: Option<&PrinciplesSpec>,
    gesture: Option<&PrinciplesSpec>,
    frames: fn(&PrinciplesSpec) -> Option<u32>,
    fraction: fn(&PrinciplesSpec) -> bool,
) -> Option<u32> {
    if let Some(gesture) = gesture {
        if let Some(count) = frames(gesture) {
            return Some(count);
        }
        if gesture.preset.is_some() || fraction(gesture) {
            return None;
        }
    }
    character.and_then(frames)
}

/// Principles of one gesture of `seconds` at `fps`, including the frame
/// counts: `overlap_frames` becomes `overlap` (at most 0.3) and
/// `anticipation_frames` becomes `anticipation_length` of the driver joints'
/// curve (fitted into 0.02–0.3).
pub fn timed_gesture_principles(
    character: Option<&PrinciplesSpec>,
    gesture: Option<&PrinciplesSpec>,
    actor: &str,
    seconds: f64,
    fps: u32,
) -> Result<Principles, Error> {
    let base = character_principles(character, actor)?;
    let mut p = gesture_principles(base, gesture, actor)?;
    if !seconds.is_finite() || seconds <= 0.0 || fps == 0 {
        return Ok(p);
    }
    let frames_per_gesture = seconds * fps as f64;
    if let Some(frames) = inherited_frames(
        character,
        gesture,
        |s| s.overlap_frames,
        |s| s.overlap.is_some(),
    ) {
        p.overlap = (frames as f64 / frames_per_gesture).min(motion_animation::MAX_OVERLAP);
    }
    if let Some(frames) = inherited_frames(character, gesture, |s| s.anticipation_frames, |_| false)
    {
        // Driver joints run their curve over `1 - overlap` of the gesture.
        let length = frames as f64 / (frames_per_gesture * (1.0 - p.overlap));
        p.anticipation_length = length.clamp(
            motion_animation::MIN_ANTICIPATION_LENGTH,
            motion_animation::MAX_ANTICIPATION_LENGTH,
        );
    }
    Ok(p)
}

/// Whether `anticipation_frames` fits a gesture of `seconds` at `fps`
/// without being shortened to 30 % of it; `None` without frame counts.
pub fn anticipation_fits(
    character: Option<&PrinciplesSpec>,
    gesture: Option<&PrinciplesSpec>,
    seconds: f64,
    fps: u32,
) -> Option<(u32, bool)> {
    let frames = inherited_frames(character, gesture, |s| s.anticipation_frames, |_| false)?;
    let p = timed_gesture_principles(character, gesture, "", seconds, fps).ok()?;
    let wanted = frames as f64 / (seconds * fps as f64 * (1.0 - p.overlap));
    Some((frames, wanted <= motion_animation::MAX_ANTICIPATION_LENGTH))
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
    fn frame_counts_become_fractions_of_each_gesture() {
        let character = spec("{anticipation_frames: 5, overlap_frames: 3}").unwrap();
        // 1 s at 24 fps: overlap 3/24, anticipation 5 / (24 * (1 - 3/24)) = 5/21.
        let p = timed_gesture_principles(Some(&character), None, "mika", 1.0, 24).unwrap();
        assert_eq!(p.overlap, 3.0 / 24.0);
        assert!((p.anticipation_length - 5.0 / 21.0).abs() < 1e-12);
        // Twice as long: the same frames are half the fraction.
        let long = timed_gesture_principles(Some(&character), None, "mika", 2.0, 24).unwrap();
        assert_eq!(long.overlap, 3.0 / 48.0);
        assert!((long.anticipation_length - 5.0 / 45.0).abs() < 1e-12);
        // A short gesture fits the anticipation into 30 %.
        let short = timed_gesture_principles(Some(&character), None, "mika", 0.5, 24).unwrap();
        assert_eq!(short.anticipation_length, 0.3);
        assert_eq!(
            anticipation_fits(Some(&character), None, 0.5, 24),
            Some((5, false))
        );
        assert_eq!(
            anticipation_fits(Some(&character), None, 2.0, 24),
            Some((5, true))
        );
        assert_eq!(anticipation_fits(None, None, 1.0, 24), None);
        // Without frame counts nothing changes.
        assert_eq!(
            timed_gesture_principles(None, None, "mika", 1.0, 24).unwrap(),
            Principles::DEFAULT
        );
    }

    #[test]
    fn gesture_blocks_override_or_reset_inherited_frames() {
        let character = spec("{anticipation_frames: 5, overlap_frames: 3}").unwrap();
        let own = spec("{anticipation_frames: 4}").unwrap();
        let p = timed_gesture_principles(Some(&character), Some(&own), "m", 1.0, 24).unwrap();
        assert_eq!(p.overlap, 3.0 / 24.0, "overlap frames inherited");
        assert!((p.anticipation_length - 4.0 / 21.0).abs() < 1e-12);
        let fraction = spec("{overlap: 0.2}").unwrap();
        let p = timed_gesture_principles(Some(&character), Some(&fraction), "m", 1.0, 24).unwrap();
        assert_eq!(
            p.overlap, 0.2,
            "an explicit fraction wins over inherited frames"
        );
        let preset = spec("snappy").unwrap();
        let p = timed_gesture_principles(Some(&character), Some(&preset), "m", 1.0, 24).unwrap();
        // A preset restarts the timing too; the moving hold stays the character's.
        let snappy = Principles {
            moving_hold: 0.0,
            ..Principles::preset("snappy").unwrap()
        };
        assert_eq!(p, snappy);
    }

    #[test]
    fn frame_counts_are_validated() {
        for (yaml, field) in [
            ("{anticipation_frames: 0}", "anticipation_frames"),
            ("{anticipation_frames: 25}", "anticipation_frames"),
            ("{overlap_frames: 13}", "overlap_frames"),
            ("{overlap: 0.1, overlap_frames: 2}", "not both"),
        ] {
            let error = spec(yaml)
                .unwrap()
                .resolve(Principles::DEFAULT, "character mika")
                .unwrap_err()
                .to_string();
            assert!(error.contains(field), "{yaml}: {error}");
        }
        assert!(spec("{anticipation_frames: -1}").is_err());
        assert!(spec("{anticipation_frames: 2.5}").is_err());
        let roundtrip = spec("{anticipation_frames: 5, overlap_frames: 0}").unwrap();
        let yaml = serde_yaml::to_string(&roundtrip).unwrap();
        assert_eq!(spec(&yaml).unwrap(), roundtrip);
    }

    #[test]
    fn serializes_back_to_an_equivalent_block() {
        let original = spec("{preset: snappy, squash_stretch: 0.1}").unwrap();
        let yaml = serde_yaml::to_string(&original).unwrap();
        assert_eq!(spec(&yaml).unwrap(), original);
    }
}
