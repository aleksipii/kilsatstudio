//! Key easing names and scene/character timing (exposure and blocking).
//!
//! ```yaml
//! characters:
//!   EMMA: {asset: characters/emma, timing: {on: 1}}
//! scenes:
//!   - id: intro
//!     composition:
//!       timing: {on: 2, interpolation: stepped}
//!       actor_tracks:
//!         EMMA:
//!           - {at: 0.0, x: 0, easing: ease_in_out}
//!           - {at: 1.0, x: 80, easing: back(1.3)}
//! ```
//!
//! Timing is evaluation-only: it never rewrites keys. A character's fields
//! override the scene's; unset fields default to on ones with spline curves.
use crate::{Error, Script};
pub use motion_animation::{
    Exposure, Interpolation, DEFAULT_BACK_OVERSHOOT, MAX_BACK_OVERSHOOT, MAX_EXPOSURE,
};
use serde::{Deserialize, Serialize};

/// MotionScript easing names, in the order editors should present them.
/// `back` also takes an overshoot: `back(1.3)`.
pub const EASINGS: [&str; 7] = [
    "linear",
    "smoothstep",
    "hold",
    "ease_in",
    "ease_out",
    "ease_in_out",
    "back",
];

/// Outgoing easing of an authored key; serialized as its MotionScript name.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum ActorEasing {
    #[default]
    Linear,
    Smoothstep,
    Hold,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// Ease-out past the next key, settling onto it; see
    /// `motion_animation::Easing::Back`.
    Back(f64),
}

impl ActorEasing {
    pub fn animation_easing(self) -> motion_animation::Easing {
        use motion_animation::Easing;
        match self {
            ActorEasing::Linear => Easing::Linear,
            ActorEasing::Smoothstep => Easing::SmoothStep,
            ActorEasing::Hold => Easing::Hold,
            ActorEasing::EaseIn => Easing::EaseIn,
            ActorEasing::EaseOut => Easing::EaseOut,
            ActorEasing::EaseInOut => Easing::EaseInOut,
            ActorEasing::Back(overshoot) => Easing::Back { overshoot },
        }
    }
    /// Blend progress for normalized interval progress `t`.
    pub fn apply(self, t: f64) -> f64 {
        self.animation_easing().apply(t)
    }
    pub fn is_valid(self) -> bool {
        self.animation_easing().is_valid()
    }
}

impl std::fmt::Display for ActorEasing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActorEasing::Linear => f.write_str("linear"),
            ActorEasing::Smoothstep => f.write_str("smoothstep"),
            ActorEasing::Hold => f.write_str("hold"),
            ActorEasing::EaseIn => f.write_str("ease_in"),
            ActorEasing::EaseOut => f.write_str("ease_out"),
            ActorEasing::EaseInOut => f.write_str("ease_in_out"),
            // Rust's shortest round-trip formatting keeps `back(1.3)` exact.
            ActorEasing::Back(overshoot) => write!(f, "back({overshoot})"),
        }
    }
}

impl std::str::FromStr for ActorEasing {
    type Err = String;
    fn from_str(name: &str) -> Result<Self, String> {
        let easing = match name {
            "linear" => ActorEasing::Linear,
            "smoothstep" => ActorEasing::Smoothstep,
            "hold" => ActorEasing::Hold,
            "ease_in" => ActorEasing::EaseIn,
            "ease_out" => ActorEasing::EaseOut,
            "ease_in_out" => ActorEasing::EaseInOut,
            "back" => ActorEasing::Back(DEFAULT_BACK_OVERSHOOT),
            _ => {
                let Some(argument) = name
                    .strip_prefix("back(")
                    .and_then(|rest| rest.strip_suffix(')'))
                else {
                    return Err(format!(
                        "unknown easing `{name}`; supported: {}, back(<overshoot>)",
                        EASINGS.join(", ")
                    ));
                };
                let overshoot = argument.trim().parse::<f64>().map_err(|_| {
                    format!("easing `{name}`: overshoot must be a number, as in back(1.3)")
                })?;
                ActorEasing::Back(overshoot)
            }
        };
        if !easing.is_valid() {
            return Err(overshoot_message(name));
        }
        Ok(easing)
    }
}

fn overshoot_message(name: &str) -> String {
    format!("easing `{name}`: back overshoot must be finite and within 0..={MAX_BACK_OVERSHOOT}")
}

impl Serialize for ActorEasing {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ActorEasing {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        name.parse().map_err(serde::de::Error::custom)
    }
}

/// Exposure and blocking for a scene or a character; unset fields inherit.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Timing {
    /// Hold each evaluated pose for 1, 2 or 3 frames.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<Exposure>,
    /// `stepped` evaluates every key as `hold` for blocking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interpolation: Option<Interpolation>,
}

/// The exposure and interpolation one actor is evaluated with.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedTiming {
    pub on: Exposure,
    pub interpolation: Interpolation,
}

impl Timing {
    pub fn is_unset(&self) -> bool {
        self.on.is_none() && self.interpolation.is_none()
    }
    /// Character fields override scene fields; unset means on ones, spline.
    pub fn resolve(scene: &Timing, character: &Timing) -> ResolvedTiming {
        ResolvedTiming {
            on: character.on.or(scene.on).unwrap_or_default(),
            interpolation: character
                .interpolation
                .or(scene.interpolation)
                .unwrap_or_default(),
        }
    }
}

impl Script {
    /// Timing for `actor` in scene `scene_index`; defaults when either is absent.
    pub fn actor_timing(&self, scene_index: usize, actor: &str) -> ResolvedTiming {
        let unset = Timing::default();
        let scene = self
            .scenes
            .get(scene_index)
            .map_or(&unset, |scene| &scene.composition.timing);
        let character = self
            .characters
            .get(actor)
            .map_or(&unset, |character| &character.timing);
        Timing::resolve(scene, character)
    }
}

/// Reject unknown easing names and timing values with their document path,
/// before typed deserialization loses the location.
pub(crate) fn precheck(document: &serde_yaml::Value) -> Result<(), Error> {
    let mut path = String::new();
    walk_easings(document, &mut path)?;
    if let Some(characters) = document
        .get("characters")
        .and_then(serde_yaml::Value::as_mapping)
    {
        for (id, character) in characters {
            if let Some(timing) = character.get("timing") {
                check_timing(timing, &format!("characters.{}.timing", key_text(id)))?;
            }
        }
    }
    if let Some(scenes) = document
        .get("scenes")
        .and_then(serde_yaml::Value::as_sequence)
    {
        for (index, scene) in scenes.iter().enumerate() {
            if let Some(timing) = scene.get("composition").and_then(|c| c.get("timing")) {
                check_timing(timing, &format!("scenes[{index}].composition.timing"))?;
            }
        }
    }
    Ok(())
}

fn key_text(key: &serde_yaml::Value) -> String {
    match key {
        serde_yaml::Value::String(text) => text.clone(),
        other => serde_yaml::to_string(other)
            .map(|text| text.trim().to_owned())
            .unwrap_or_default(),
    }
}

fn walk_easings(value: &serde_yaml::Value, path: &mut String) -> Result<(), Error> {
    let length = path.len();
    match value {
        serde_yaml::Value::Mapping(mapping) => {
            for (key, child) in mapping {
                let key = key_text(key);
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(&key);
                if key == "easing" {
                    if let Some(name) = child.as_str() {
                        if let Err(message) = name.parse::<ActorEasing>() {
                            return Err(located(path, message));
                        }
                    }
                } else {
                    walk_easings(child, path)?;
                }
                path.truncate(length);
            }
        }
        serde_yaml::Value::Sequence(items) => {
            for (index, child) in items.iter().enumerate() {
                path.push_str(&format!("[{index}]"));
                walk_easings(child, path)?;
                path.truncate(length);
            }
        }
        serde_yaml::Value::Tagged(tagged) => walk_easings(&tagged.value, path)?,
        _ => {}
    }
    Ok(())
}

/// Unknown names are unsupported; out-of-range numbers are invalid.
fn located(path: &str, message: String) -> Error {
    if message.starts_with("unknown") {
        Error::Unsupported(format!("{path}: {message}"))
    } else {
        Error::Validation(format!("{path}: {message}"))
    }
}

fn check_timing(timing: &serde_yaml::Value, path: &str) -> Result<(), Error> {
    let Some(mapping) = timing.as_mapping() else {
        return Err(Error::Validation(format!(
            "{path}: expected a mapping such as {{on: 2, interpolation: stepped}}"
        )));
    };
    for (key, value) in mapping {
        let key = key_text(key);
        match key.as_str() {
            "on" => {
                let frames = value
                    .as_u64()
                    .filter(|f| (1..=u64::from(MAX_EXPOSURE)).contains(f));
                if frames.is_none() {
                    return Err(Error::Validation(format!(
                        "{path}.on: exposure must be 1, 2 or 3 frames, got {}",
                        key_text(value)
                    )));
                }
            }
            "interpolation" => {
                let name = value.as_str().unwrap_or_default();
                if !["spline", "stepped"].contains(&name) {
                    return Err(Error::Unsupported(format!(
                        "{path}.interpolation: unknown interpolation `{}`; supported: spline, stepped",
                        key_text(value)
                    )));
                }
            }
            other => {
                return Err(Error::Unsupported(format!(
                    "{path}.{other}: unknown timing key; supported: on, interpolation"
                )))
            }
        }
    }
    Ok(())
}

/// Reject easings built in code (not parsed) with invalid parameters.
pub(crate) fn validate(script: &Script) -> Result<(), Error> {
    let tracks = |name: &str| {
        let name = name.to_owned();
        move |easing: &ActorEasing| {
            if easing.is_valid() {
                Ok(())
            } else {
                Err(Error::Validation(format!(
                    "{name}: {}",
                    overshoot_message(&easing.to_string())
                )))
            }
        }
    };
    let check_keys = |name: &str, keys: &[crate::ActorKey]| {
        keys.iter()
            .map(|key| &key.easing)
            .try_for_each(tracks(name))
    };
    for (id, clip) in &script.animation_clips {
        for (bone, keys) in &clip.bone_tracks {
            check_keys(&format!("animation_clips.{id}.bone_tracks.{bone}"), keys)?;
        }
    }
    for (id, clip) in &script.rig_clips {
        for (joint, keys) in &clip.joint_tracks {
            check_keys(&format!("rig_clips.{id}.joint_tracks.{joint}"), keys)?;
        }
    }
    for (index, scene) in script.scenes.iter().enumerate() {
        let composition = &scene.composition;
        let at = format!("scenes[{index}].composition");
        for (actor, keys) in &composition.actor_tracks {
            check_keys(&format!("{at}.actor_tracks.{actor}"), keys)?;
        }
        for (field, nested) in [
            ("bone_tracks", &composition.bone_tracks),
            ("joint_tracks", &composition.joint_tracks),
        ] {
            for (actor, tracks) in nested {
                for (part, keys) in tracks {
                    check_keys(&format!("{at}.{field}.{actor}.{part}"), keys)?;
                }
            }
        }
        for (actor, keys) in &composition.clip_weight_tracks {
            keys.iter()
                .map(|key| &key.easing)
                .try_for_each(tracks(&format!("{at}.clip_weight_tracks.{actor}")))?;
        }
        for instance in &composition.clip_instances {
            instance
                .weight_keys
                .iter()
                .map(|key| &key.easing)
                .try_for_each(tracks(&format!("{at}.clip_instances.{}", instance.id)))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_names_round_trip_and_old_names_serialize_unchanged() {
        for (name, easing) in [
            ("linear", ActorEasing::Linear),
            ("smoothstep", ActorEasing::Smoothstep),
            ("hold", ActorEasing::Hold),
            ("ease_in", ActorEasing::EaseIn),
            ("ease_out", ActorEasing::EaseOut),
            ("ease_in_out", ActorEasing::EaseInOut),
            ("back(1.3)", ActorEasing::Back(1.3)),
            ("back(0)", ActorEasing::Back(0.0)),
        ] {
            assert_eq!(name.parse::<ActorEasing>().unwrap(), easing);
            assert_eq!(easing.to_string(), name);
            let yaml = serde_yaml::to_string(&easing).unwrap();
            assert_eq!(serde_yaml::from_str::<ActorEasing>(&yaml).unwrap(), easing);
        }
        assert_eq!(
            "back".parse::<ActorEasing>().unwrap(),
            ActorEasing::Back(DEFAULT_BACK_OVERSHOOT)
        );
        assert_eq!("back( 2.5 )".parse(), Ok(ActorEasing::Back(2.5)));
    }

    #[test]
    fn invalid_easings_explain_themselves() {
        for bad in ["bounce", "Linear", "back(", "ease-in", ""] {
            assert!(
                bad.parse::<ActorEasing>().unwrap_err().contains("unknown"),
                "{bad}"
            );
        }
        for bad in ["back(x)", "back(-1)", "back(11)", "back(NaN)", "back(inf)"] {
            let error = bad.parse::<ActorEasing>().unwrap_err();
            assert!(
                !error.contains("unknown") && error.contains("back"),
                "{bad}: {error}"
            );
        }
    }

    #[test]
    fn character_timing_overrides_scene_fields_independently() {
        let scene = Timing {
            on: Some(Exposure::TWOS),
            interpolation: Some(Interpolation::Stepped),
        };
        let character = Timing {
            on: Some(Exposure::ONES),
            interpolation: None,
        };
        assert_eq!(
            Timing::resolve(&scene, &character),
            ResolvedTiming {
                on: Exposure::ONES,
                interpolation: Interpolation::Stepped
            }
        );
        assert_eq!(
            Timing::resolve(&Timing::default(), &Timing::default()),
            ResolvedTiming::default()
        );
        assert!(Timing::default().is_unset() && !scene.is_unset());
    }
}
