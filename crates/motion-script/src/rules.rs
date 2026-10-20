//! Reusable, conditional animation rules.
//!
//! A rule is declarative data: a trigger (an authored event kind or a fixed
//! scene time), optional conditions and one bounded effect. Expansion is a pure,
//! deterministic function from a validated [`Script`] to an ordinary script whose
//! compositions contain the generated clip instances and camera keys. Rules fire
//! only on authored events, never on their own outputs, so recursion and cycles
//! cannot occur. There is no scripting language and no runtime evaluation.
use crate::{
    camera_preset_keys, valid_clip_name, validate_emotion, CameraKey, CameraPreset, ClipBlendMode,
    ClipInstance, Error, Event, Script,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Maximum named rules per screenplay.
pub const MAX_RULES: usize = 64;
/// Maximum generated outputs (clip instances plus camera segments) per scene.
pub const MAX_RULE_OUTPUTS_PER_SCENE: usize = 128;
/// Maximum generated outputs across the whole screenplay.
pub const MAX_RULE_OUTPUTS: usize = 1024;
/// Rule names are valid clip identifiers of at most this length, so generated
/// instance IDs (`r_<rule>_<n>`) stay within the 64-character identifier limit.
pub const MAX_RULE_NAME: usize = 40;

/// Event kind (or fixed time) that fires a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleTrigger {
    Emotion,
    Speak,
    Gesture,
    Move,
    /// Fires once per scene at `at`.
    Time,
}

/// Camera template used by a camera effect; same geometry as [`CameraPreset`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleCameraPreset {
    PushIn,
    PullOut,
    PanLeft,
    PanRight,
}

impl RuleCameraPreset {
    fn preset(self) -> CameraPreset {
        match self {
            Self::PushIn => CameraPreset::PushIn,
            Self::PullOut => CameraPreset::PullOut,
            Self::PanLeft => CameraPreset::PanLeft,
            Self::PanRight => CameraPreset::PanRight,
        }
    }
}

fn one() -> f64 {
    1.0
}

/// Conditions that must all hold; an empty condition always holds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuleCondition {
    /// Scene IDs in which the rule may fire; empty means every scene.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scenes: Vec<String>,
    /// Subject actor: the event's actor, or for `time` triggers the actor the
    /// condition (and, by default, the effect) refers to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    /// Subject actor's emotion at the trigger time (latest `emotion` event at or
    /// before it, `neutral` if none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emotion: Option<String>,
    /// Inclusive lower bound on the trigger time, in scene seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<f64>,
    /// Exclusive upper bound on the trigger time, in scene seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<f64>,
}

/// Bounded effect that maps onto an existing composition capability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuleEffect {
    /// Add a live clip instance for the target actor (original rig only).
    Clip {
        clip: String,
        /// Defaults to the trigger actor, then `when.actor`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        actor: Option<String>,
        /// Seconds after the trigger time.
        #[serde(default)]
        offset: f64,
        duration: f64,
        #[serde(default = "one")]
        weight: f64,
        #[serde(default, skip_serializing_if = "ClipBlendMode::is_override")]
        blend_mode: ClipBlendMode,
        #[serde(default)]
        fade_in: f64,
        #[serde(default)]
        fade_out: f64,
    },
    /// Add two camera keys using a preset over `[trigger + offset, + duration]`.
    Camera {
        preset: RuleCameraPreset,
        #[serde(default)]
        offset: f64,
        duration: f64,
    },
}

/// One named reusable rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub on: RuleTrigger,
    /// Emotion or gesture name the triggering event must carry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Scene time for `time` triggers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<f64>,
    #[serde(default, rename = "when", skip_serializing_if = "is_default")]
    pub condition: RuleCondition,
    #[serde(rename = "then")]
    pub effect: RuleEffect,
}

fn is_default(condition: &RuleCondition) -> bool {
    *condition == RuleCondition::default()
}

/// Typed rule failure; converts to [`Error`] (`Unsupported` stays `Unsupported`).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RuleError {
    #[error("rule {rule}: unknown {kind} {name}")]
    UnknownReference {
        rule: String,
        kind: &'static str,
        name: String,
    },
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("rules conflict: {0}")]
    Conflict(String),
    #[error("rule bound exceeded: {0}")]
    Bound(String),
    #[error("rule {rule}: {message}")]
    Invalid { rule: String, message: String },
}

impl From<RuleError> for Error {
    fn from(error: RuleError) -> Self {
        match error {
            RuleError::Unsupported(message) => Error::Unsupported(message),
            other => Error::Validation(other.to_string()),
        }
    }
}

fn invalid(rule: &str, message: impl Into<String>) -> RuleError {
    RuleError::Invalid {
        rule: rule.into(),
        message: message.into(),
    }
}

/// Report unsupported trigger/effect/preset names explicitly before typed decoding.
pub(crate) fn precheck(document: &serde_yaml::Value) -> Result<(), Error> {
    let Some(rules) = document
        .get("rules")
        .and_then(serde_yaml::Value::as_mapping)
    else {
        return Ok(());
    };
    let check = |value: Option<&serde_yaml::Value>, what: &str, ok: &[&str]| match value
        .and_then(serde_yaml::Value::as_str)
    {
        Some(name) if !ok.contains(&name) => Err(Error::Unsupported(format!(
            "rule {what} {name}: supported values are {}",
            ok.join(", ")
        ))),
        _ => Ok(()),
    };
    for rule in rules.values() {
        check(
            rule.get("on"),
            "trigger",
            &["emotion", "speak", "gesture", "move", "time"],
        )?;
        let then = rule.get("then");
        check(
            then.and_then(|t| t.get("type")),
            "effect",
            &["clip", "camera"],
        )?;
        check(
            then.and_then(|t| t.get("preset")),
            "camera preset",
            &["push_in", "pull_out", "pan_left", "pan_right"],
        )?;
    }
    Ok(())
}

struct Firing<'a> {
    time: f64,
    actor: Option<&'a str>,
}

impl Script {
    /// Expand rules into an ordinary script (rules cleared) and validate the result.
    ///
    /// Output order is deterministic: rules by ascending name, then firings by
    /// ascending trigger time and authored event order. Generated clip instances
    /// are appended after authored ones with IDs `r_<rule>_<n>` (`n` from 1 per
    /// rule and scene), so rule names control override layering. Preview, seek
    /// and export all evaluate this one expanded script.
    pub fn expand_rules(&self) -> Result<Script, RuleError> {
        let mut out = self.clone();
        out.rules.clear();
        if self.rules.is_empty() {
            out.validate().map_err(base_error)?;
            return Ok(out);
        }
        if self.rules.len() > MAX_RULES {
            return Err(RuleError::Bound(format!("more than {MAX_RULES} rules")));
        }
        for (name, rule) in &self.rules {
            self.check_rule(name, rule)?;
        }
        let mut total = 0usize;
        for scene in &mut out.scenes {
            let original = self.scenes.iter().find(|s| s.id == scene.id).unwrap();
            let mut order: Vec<usize> = (0..original.events.len()).collect();
            order.sort_by(|a, b| {
                original.events[*a]
                    .actor_time()
                    .0
                    .total_cmp(&original.events[*b].actor_time().0)
            });
            let mut scene_outputs = 0usize;
            let mut camera_spans: Vec<(f64, f64, String)> = Vec::new();
            let mut new_camera: Vec<CameraKey> = Vec::new();
            let mut ids: std::collections::BTreeSet<String> = original
                .composition
                .clip_instances
                .iter()
                .map(|i| i.id.clone())
                .collect();
            for (name, rule) in &self.rules {
                if !rule.condition.scenes.is_empty() && !rule.condition.scenes.contains(&scene.id) {
                    continue;
                }
                let mut firings = Vec::new();
                if rule.on == RuleTrigger::Time {
                    let at = rule.at.expect("checked");
                    if at < original.duration {
                        firings.push(Firing {
                            time: at,
                            actor: rule
                                .condition
                                .actor
                                .as_deref()
                                .or(effect_actor(&rule.effect)),
                        });
                    }
                } else {
                    for index in &order {
                        let event = &original.events[*index];
                        let matched = match (rule.on, event) {
                            (RuleTrigger::Emotion, Event::Emotion { value, .. })
                            | (RuleTrigger::Gesture, Event::Gesture { value, .. }) => {
                                rule.value.as_ref().is_none_or(|v| v == value)
                            }
                            (RuleTrigger::Speak, Event::Speak { .. })
                            | (RuleTrigger::Move, Event::Move { .. }) => true,
                            _ => false,
                        };
                        if matched {
                            let (time, actor) = event.actor_time();
                            firings.push(Firing {
                                time,
                                actor: Some(actor),
                            });
                        }
                    }
                }
                let mut n = 0usize;
                for firing in firings {
                    if !self.condition_holds(name, rule, original, &order, &firing)? {
                        continue;
                    }
                    n += 1;
                    scene_outputs += 1;
                    total += 1;
                    if scene_outputs > MAX_RULE_OUTPUTS_PER_SCENE || total > MAX_RULE_OUTPUTS {
                        return Err(RuleError::Bound(format!(
                            "rule outputs exceed {MAX_RULE_OUTPUTS_PER_SCENE} per scene or {MAX_RULE_OUTPUTS} per screenplay (scene {})",
                            scene.id
                        )));
                    }
                    let start = firing.time
                        + match &rule.effect {
                            RuleEffect::Clip { offset, .. } | RuleEffect::Camera { offset, .. } => {
                                *offset
                            }
                        };
                    match &rule.effect {
                        RuleEffect::Clip {
                            clip,
                            actor,
                            duration,
                            weight,
                            blend_mode,
                            fade_in,
                            fade_out,
                            ..
                        } => {
                            let target = actor
                                .as_deref()
                                .or(firing.actor)
                                .ok_or_else(|| invalid(name, "clip effect has no target actor"))?;
                            if !self.characters.contains_key(target) {
                                return Err(RuleError::UnknownReference {
                                    rule: name.clone(),
                                    kind: "actor",
                                    name: target.into(),
                                });
                            }
                            let id = format!("r_{name}_{n}");
                            if !ids.insert(id.clone()) {
                                return Err(RuleError::Conflict(format!(
                                    "scene {}: generated clip instance {id} duplicates an authored ID",
                                    scene.id
                                )));
                            }
                            scene.composition.clip_instances.push(ClipInstance {
                                id,
                                clip: clip.clone(),
                                actor: target.into(),
                                at: start,
                                duration: *duration,
                                weight: *weight,
                                blend_mode: *blend_mode,
                                normalized_group: None,
                                fade_in: *fade_in,
                                fade_out: *fade_out,
                                weight_keys: vec![],
                                looping: false,
                                joint_mask: vec![],
                            });
                        }
                        RuleEffect::Camera {
                            preset, duration, ..
                        } => {
                            let end = start + duration;
                            if !start.is_finite() || start < 0.0 || !end.is_finite() {
                                return Err(invalid(name, "camera span must be finite and >= 0"));
                            }
                            if let Some((_, _, other)) = camera_spans
                                .iter()
                                .find(|(s, e, _)| start <= *e && *s <= end)
                            {
                                return Err(RuleError::Conflict(format!(
                                    "scene {}: camera outputs of rules {other} and {name} overlap",
                                    scene.id
                                )));
                            }
                            if let Some(key) = original
                                .composition
                                .camera
                                .iter()
                                .find(|k| k.at >= start && k.at <= end)
                            {
                                return Err(RuleError::Conflict(format!(
                                    "scene {}: rule {name} camera span [{start}, {end}] contains authored camera key at {}",
                                    scene.id, key.at
                                )));
                            }
                            camera_spans.push((start, end, name.clone()));
                            let keys = camera_preset_keys(preset.preset(), *duration)
                                .map_err(|e| invalid(name, e.to_string()))?;
                            new_camera.extend(keys.into_iter().map(|mut k| {
                                k.at += start;
                                k
                            }));
                        }
                    }
                }
            }
            if !new_camera.is_empty() {
                scene.composition.camera.extend(new_camera);
                scene
                    .composition
                    .camera
                    .sort_by(|a, b| a.at.total_cmp(&b.at));
            }
        }
        out.validate().map_err(base_error)?;
        Ok(out)
    }

    fn check_rule(&self, name: &str, rule: &Rule) -> Result<(), RuleError> {
        if !valid_clip_name(name) || name.len() > MAX_RULE_NAME {
            return Err(invalid(
                name,
                format!("name must be 1..{MAX_RULE_NAME} ASCII letters, digits, '_' or '-'"),
            ));
        }
        let finite = |v: f64| v.is_finite();
        match rule.on {
            RuleTrigger::Time => {
                if rule.value.is_some() {
                    return Err(invalid(name, "time triggers do not take a value"));
                }
                if !rule.at.is_some_and(|at| finite(at) && at >= 0.0) {
                    return Err(invalid(name, "time trigger requires finite at >= 0"));
                }
            }
            on => {
                if rule.at.is_some() {
                    return Err(invalid(name, "only time triggers take at"));
                }
                match (on, rule.value.as_deref()) {
                    (_, None) => {}
                    (RuleTrigger::Emotion, Some(v)) => validate_emotion(v).map_err(base_error)?,
                    (RuleTrigger::Gesture, Some(v)) => {
                        if motion_animation::Gesture::parse(v).is_none() {
                            return Err(RuleError::Unsupported(format!(
                                "rule {name}: gesture {v}: supported gestures are {}",
                                motion_animation::GESTURES.join(", ")
                            )));
                        }
                    }
                    _ => return Err(invalid(name, "value applies only to emotion and gesture")),
                }
            }
        }
        let condition = &rule.condition;
        for scene in &condition.scenes {
            if !self.scenes.iter().any(|s| &s.id == scene) {
                return Err(RuleError::UnknownReference {
                    rule: name.into(),
                    kind: "scene",
                    name: scene.clone(),
                });
            }
        }
        if let Some(actor) = &condition.actor {
            if !self.characters.contains_key(actor) {
                return Err(RuleError::UnknownReference {
                    rule: name.into(),
                    kind: "actor",
                    name: actor.clone(),
                });
            }
        }
        if let Some(emotion) = &condition.emotion {
            validate_emotion(emotion).map_err(base_error)?;
        }
        for bound in [condition.from, condition.until].into_iter().flatten() {
            if !finite(bound) || bound < 0.0 {
                return Err(invalid(name, "from/until require finite values >= 0"));
            }
        }
        if let (Some(from), Some(until)) = (condition.from, condition.until) {
            if until <= from {
                return Err(invalid(name, "until must be greater than from"));
            }
        }
        match &rule.effect {
            RuleEffect::Clip {
                clip,
                actor,
                offset,
                duration,
                ..
            } => {
                if !self.animation_clips.contains_key(clip) {
                    return Err(RuleError::UnknownReference {
                        rule: name.into(),
                        kind: "clip",
                        name: clip.clone(),
                    });
                }
                if let Some(actor) = actor {
                    if !self.characters.contains_key(actor) {
                        return Err(RuleError::UnknownReference {
                            rule: name.into(),
                            kind: "actor",
                            name: actor.clone(),
                        });
                    }
                }
                if rule.on == RuleTrigger::Time && condition.actor.is_none() && actor.is_none() {
                    return Err(invalid(name, "time trigger with a clip needs an actor"));
                }
                if !finite(*offset) || !finite(*duration) || *duration <= 0.0 {
                    return Err(invalid(
                        name,
                        "offset and duration must be finite, duration > 0",
                    ));
                }
            }
            RuleEffect::Camera {
                offset, duration, ..
            } => {
                if !finite(*offset) || !finite(*duration) || *duration <= 0.0 {
                    return Err(invalid(
                        name,
                        "offset and duration must be finite, duration > 0",
                    ));
                }
            }
        }
        if condition.emotion.is_some()
            && rule.on == RuleTrigger::Time
            && condition.actor.is_none()
            && effect_actor(&rule.effect).is_none()
        {
            return Err(invalid(
                name,
                "emotion condition on a time trigger needs an actor",
            ));
        }
        Ok(())
    }

    fn condition_holds(
        &self,
        name: &str,
        rule: &Rule,
        scene: &crate::Scene,
        order: &[usize],
        firing: &Firing<'_>,
    ) -> Result<bool, RuleError> {
        let c = &rule.condition;
        if c.from.is_some_and(|from| firing.time < from)
            || c.until.is_some_and(|until| firing.time >= until)
        {
            return Ok(false);
        }
        if let Some(actor) = &c.actor {
            if firing.actor != Some(actor.as_str()) {
                return Ok(false);
            }
        }
        if let Some(wanted) = &c.emotion {
            let subject = firing
                .actor
                .ok_or_else(|| invalid(name, "emotion condition needs a subject actor"))?;
            let mut current = "neutral";
            for index in order {
                if let Event::Emotion { at, actor, value } = &scene.events[*index] {
                    if actor == subject && *at <= firing.time {
                        current = value;
                    }
                }
            }
            if current != wanted {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn effect_actor(effect: &RuleEffect) -> Option<&str> {
    match effect {
        RuleEffect::Clip { actor, .. } => actor.as_deref(),
        RuleEffect::Camera { .. } => None,
    }
}

fn base_error(error: Error) -> RuleError {
    match error {
        Error::Unsupported(message) => RuleError::Unsupported(message),
        Error::Validation(message) => {
            RuleError::Conflict(format!("expanded screenplay invalid: {message}"))
        }
        Error::Syntax(error) => {
            RuleError::Conflict(format!("expanded screenplay invalid: {error}"))
        }
    }
}

/// Rule map alias used by [`Script`].
pub type Rules = BTreeMap<String, Rule>;

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
