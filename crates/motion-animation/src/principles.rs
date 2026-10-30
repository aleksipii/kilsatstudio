//! Animation principles as tunable parameters for the procedural rig layer.
//!
//! Anticipation, overshoot (with settle), follow-through, overlapping action,
//! squash and stretch, and the moving hold were fixed constants in
//! [`crate::rig_motion`]. [`Principles::DEFAULT`] keeps those exact values, so
//! output with default principles is bit-identical to the old constants.
//! Squash and stretch and the moving hold are off by default.

/// Tunable animation principles for one character or one action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Principles {
    /// Depth of the move against the action before it starts, as a fraction of
    /// the full pose.
    pub anticipation: f64,
    /// Overshoot strength of the attack ("back" easing constant). Zero settles
    /// without passing the pose.
    pub overshoot: f64,
    /// Depth of the swing past rest when the pose is released.
    pub follow_through: f64,
    /// Progress lag of child joints (forearms, head) behind their drivers.
    pub overlap: f64,
    /// Largest stretch (or squash) of the torso as a fraction of its length.
    /// The torso keeps its area: `scale.x * scale.y == 1`.
    pub squash_stretch: f64,
    /// Strength of the breathing and sway during long holds; zero is off.
    pub moving_hold: f64,
}

/// One validated principle field: name and inclusive range.
struct Field {
    name: &'static str,
    min: f64,
    max: f64,
    get: fn(&Principles) -> f64,
}

const FIELDS: [Field; 6] = [
    Field {
        name: "anticipation",
        min: 0.0,
        max: 0.5,
        get: |p| p.anticipation,
    },
    Field {
        name: "overshoot",
        min: 0.0,
        max: 3.0,
        get: |p| p.overshoot,
    },
    Field {
        name: "follow_through",
        min: 0.0,
        max: 1.0,
        get: |p| p.follow_through,
    },
    Field {
        name: "overlap",
        min: 0.0,
        max: 0.3,
        get: |p| p.overlap,
    },
    Field {
        name: "squash_stretch",
        min: 0.0,
        max: 0.3,
        get: |p| p.squash_stretch,
    },
    Field {
        name: "moving_hold",
        min: 0.0,
        max: 1.0,
        get: |p| p.moving_hold,
    },
];

/// A principle value outside its allowed range.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("principles.{field} must be a finite number in {min}..={max}, got {value}")]
pub struct PrinciplesError {
    pub field: &'static str,
    pub value: f64,
    pub min: f64,
    pub max: f64,
}

/// Names of the built-in presets, in [`Principles::preset`] order.
pub const PRINCIPLE_PRESETS: [&str; 3] = ["subtle", "cartoon", "snappy"];

impl Principles {
    /// The values the rig layer has always used; squash and stretch and the
    /// moving hold off.
    pub const DEFAULT: Self = Self {
        anticipation: 0.12,
        overshoot: 1.3,
        follow_through: 0.25,
        overlap: 0.08,
        squash_stretch: 0.0,
        moving_hold: 0.0,
    };

    /// A built-in preset: `subtle` (restrained, naturalistic), `cartoon`
    /// (broad, elastic) or `snappy` (little anticipation, hard overshoot).
    pub fn preset(name: &str) -> Option<Self> {
        Some(match name {
            "subtle" => Self {
                anticipation: 0.06,
                overshoot: 0.6,
                follow_through: 0.12,
                overlap: 0.05,
                squash_stretch: 0.0,
                moving_hold: 0.5,
            },
            "cartoon" => Self {
                anticipation: 0.25,
                overshoot: 2.2,
                follow_through: 0.45,
                overlap: 0.14,
                squash_stretch: 0.18,
                moving_hold: 1.0,
            },
            "snappy" => Self {
                anticipation: 0.08,
                overshoot: 1.8,
                follow_through: 0.15,
                overlap: 0.04,
                squash_stretch: 0.08,
                moving_hold: 0.6,
            },
            _ => return None,
        })
    }

    /// Every field must be finite and inside its range; the error names the
    /// first field that is not.
    pub fn validate(&self) -> Result<(), PrinciplesError> {
        for field in &FIELDS {
            let value = (field.get)(self);
            if !value.is_finite() || value < field.min || value > field.max {
                return Err(PrinciplesError {
                    field: field.name,
                    value,
                    min: field.min,
                    max: field.max,
                });
            }
        }
        Ok(())
    }

    /// Inclusive range of a field by name, for diagnostics and editors.
    pub fn range(field: &str) -> Option<(f64, f64)> {
        FIELDS
            .iter()
            .find(|f| f.name == field)
            .map(|f| (f.min, f.max))
    }
}

impl Default for Principles {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Area-preserving scale for a signed stretch `s` in `-0.3..=0.3`: positive
/// lengthens y, negative squashes it; always `x * y == 1` up to rounding.
pub(crate) fn squash_scale(s: f64) -> (f64, f64) {
    let s = if s.is_finite() {
        s.clamp(-0.3, 0.3)
    } else {
        0.0
    };
    let y = 1.0 + s;
    (1.0 / y, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_presets_validate() {
        assert_eq!(Principles::default(), Principles::DEFAULT);
        Principles::DEFAULT.validate().unwrap();
        for name in PRINCIPLE_PRESETS {
            Principles::preset(name).unwrap().validate().unwrap();
        }
        assert!(Principles::preset("wobbly").is_none());
    }

    #[test]
    fn validation_names_the_field_and_range() {
        let error = Principles {
            overshoot: 3.5,
            ..Principles::DEFAULT
        }
        .validate()
        .unwrap_err();
        assert_eq!(error.field, "overshoot");
        assert_eq!((error.min, error.max), (0.0, 3.0));
        assert_eq!(
            error.to_string(),
            "principles.overshoot must be a finite number in 0..=3, got 3.5"
        );
        for (field, bad) in [
            ("anticipation", -0.01),
            ("follow_through", f64::NAN),
            ("overlap", 0.31),
            ("squash_stretch", f64::INFINITY),
            ("moving_hold", 1.5),
        ] {
            let mut p = Principles::DEFAULT;
            match field {
                "anticipation" => p.anticipation = bad,
                "follow_through" => p.follow_through = bad,
                "overlap" => p.overlap = bad,
                "squash_stretch" => p.squash_stretch = bad,
                _ => p.moving_hold = bad,
            }
            assert_eq!(p.validate().unwrap_err().field, field);
        }
        // Range ends are allowed.
        let edges = Principles {
            anticipation: 0.5,
            overshoot: 0.0,
            follow_through: 1.0,
            overlap: 0.3,
            squash_stretch: 0.3,
            moving_hold: 1.0,
        };
        edges.validate().unwrap();
        assert_eq!(Principles::range("overlap"), Some((0.0, 0.3)));
        assert_eq!(Principles::range("nope"), None);
    }

    #[test]
    fn squash_scale_preserves_area_and_is_bounded() {
        for i in -40..=40 {
            let (x, y) = squash_scale(i as f64 / 100.0);
            assert!((x * y - 1.0).abs() < 1e-12);
            assert!((0.7 - 1e-12..=1.3 + 1e-12).contains(&y));
        }
        assert_eq!(squash_scale(f64::NAN), (1.0, 1.0));
    }
}
