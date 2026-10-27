//! EventLog v1: external input recorded as replayable data (ORK-G1-03).
//!
//! An event log is the complete record of input that did not come from the
//! screenplay: key presses, limb drags, camera head-pose measurements and limb
//! releases. Each event has a scene ID and an integer, scene-local sample
//! position on the dialogue sample clock, so replaying the same log against the
//! same screenplay reproduces the same result. There is no hidden history: a
//! consumer sees exactly the events in the log, in log order.
//!
//! The log is plain data. Validation checks structure ([`EventLog::validate`])
//! and references into a screenplay ([`EventLog::validate_for`]); nothing here
//! reads devices or clocks.
use crate::{valid_clip_name, Script, HUMANOID_BONES};
use serde::{Deserialize, Serialize};

/// Current serialized format version.
pub const EVENT_LOG_VERSION: u32 = 1;
/// Maximum events in one log.
pub const MAX_LOGGED_EVENTS: usize = 4096;
/// Upper bound for the recorded sample rate (samples per second).
pub const MAX_EVENT_LOG_SAMPLE_RATE: u32 = 192_000;
/// Maximum length of a key name.
pub const MAX_KEY_NAME: usize = 32;
/// Maximum serialized input accepted by [`EventLog::from_json`].
pub const MAX_EVENT_LOG_BYTES: usize = 4 * 1024 * 1024;
/// Largest accepted drag coordinate magnitude, in scene units.
pub const MAX_DRAG_COORDINATE: f64 = 100_000.0;

/// Replayable record of external input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventLog {
    /// Format version; only [`EVENT_LOG_VERSION`] is supported.
    pub version: u32,
    /// Samples per second of the clock that `sample` positions count. The log
    /// records it instead of assuming one, so a different rate is detectable.
    pub sample_rate: u32,
    #[serde(default)]
    pub events: Vec<LoggedEvent>,
}

/// One external event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoggedEvent {
    /// Scene the event belongs to.
    pub scene: String,
    /// Scene-local position in samples from the scene start.
    pub sample: u64,
    /// Device or origin that produced the event.
    pub source: EventSource,
    /// What happened.
    pub value: ExternalEvent,
}

/// Origin of an external event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    Keyboard,
    /// Mouse, trackpad or tablet.
    Pointer,
    /// Local camera head-pose measurement.
    Camera,
    /// Typed or edited by hand.
    Manual,
}

/// Payload of an external event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExternalEvent {
    /// A key press. Key names are 1..32 ASCII letters, digits, `_` or `-`.
    Key { key: String },
    /// Pointer target position for a limb joint, in scene units.
    Drag {
        actor: String,
        joint: String,
        x: f64,
        y: f64,
    },
    /// Head pose in radians: `|roll| <= pi/2`, `|yaw|, |pitch| <= pi`.
    /// `yaw` and `pitch` are absent when not measured.
    CameraPose {
        roll: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        yaw: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pitch: Option<f64>,
    },
    /// A dragged limb joint is let go.
    Release { actor: String, joint: String },
}

impl ExternalEvent {
    /// Stable snake_case name of the event type.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Key { .. } => "key",
            Self::Drag { .. } => "drag",
            Self::CameraPose { .. } => "camera_pose",
            Self::Release { .. } => "release",
        }
    }

    fn allows(&self, source: EventSource) -> bool {
        use EventSource::*;
        source == Manual
            || matches!(
                (self, source),
                (Self::Key { .. }, Keyboard)
                    | (Self::Drag { .. }, Pointer)
                    | (Self::CameraPose { .. }, Camera)
                    | (Self::Release { .. }, Pointer | Keyboard)
            )
    }
}

/// Typed event-log failure. `path` uses the serialized layout, e.g. `events[3].value.x`.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EventLogError {
    #[error("event log syntax: {0}")]
    Syntax(String),
    #[error("event log version {0} is unsupported; supported version is {EVENT_LOG_VERSION}")]
    UnsupportedVersion(u32),
    #[error("event log bound exceeded: {0}")]
    Bound(String),
    #[error("{path}: {message}")]
    Invalid { path: String, message: String },
    #[error("{path}: unknown {kind} {name}")]
    UnknownReference {
        path: String,
        kind: &'static str,
        name: String,
    },
    #[error("{path}: sample {sample} is before the previous event of scene {scene} at sample {previous}")]
    Order {
        path: String,
        scene: String,
        sample: u64,
        previous: u64,
    },
}

fn invalid(index: usize, field: &str, message: impl Into<String>) -> EventLogError {
    EventLogError::Invalid {
        path: path(index, field),
        message: message.into(),
    }
}

fn path(index: usize, field: &str) -> String {
    if field.is_empty() {
        format!("events[{index}]")
    } else {
        format!("events[{index}].{field}")
    }
}

impl EventLog {
    /// Empty version-1 log at `sample_rate`.
    pub fn new(sample_rate: u32) -> Self {
        Self {
            version: EVENT_LOG_VERSION,
            sample_rate,
            events: Vec::new(),
        }
    }

    /// Scene-local time of a sample position in seconds.
    pub fn seconds(&self, sample: u64) -> f64 {
        sample as f64 / f64::from(self.sample_rate)
    }

    /// Parse and structurally validate a JSON log.
    pub fn from_json(source: &str) -> Result<Self, EventLogError> {
        if source.len() > MAX_EVENT_LOG_BYTES {
            return Err(EventLogError::Bound(format!(
                "input larger than {MAX_EVENT_LOG_BYTES} bytes"
            )));
        }
        let value: serde_json::Value =
            serde_json::from_str(source).map_err(|e| EventLogError::Syntax(e.to_string()))?;
        // Report the version before field errors so a future format is not
        // misreported as a syntax error.
        if let Some(version) = value.get("version").and_then(serde_json::Value::as_u64) {
            if version != u64::from(EVENT_LOG_VERSION) {
                return Err(EventLogError::UnsupportedVersion(
                    u32::try_from(version).unwrap_or(u32::MAX),
                ));
            }
        }
        let log: Self =
            serde_json::from_value(value).map_err(|e| EventLogError::Syntax(e.to_string()))?;
        log.validate()?;
        Ok(log)
    }

    /// Validate, then serialize as pretty JSON with a trailing newline.
    pub fn to_json(&self) -> Result<String, EventLogError> {
        self.validate()?;
        let mut text =
            serde_json::to_string_pretty(self).map_err(|e| EventLogError::Syntax(e.to_string()))?;
        text.push('\n');
        Ok(text)
    }

    /// Structural validation: version, sample rate, bounds, finite values,
    /// identifiers, source/type compatibility and per-scene sample order.
    pub fn validate(&self) -> Result<(), EventLogError> {
        if self.version != EVENT_LOG_VERSION {
            return Err(EventLogError::UnsupportedVersion(self.version));
        }
        if !(1..=MAX_EVENT_LOG_SAMPLE_RATE).contains(&self.sample_rate) {
            return Err(EventLogError::Invalid {
                path: "sample_rate".into(),
                message: format!("must be 1..={MAX_EVENT_LOG_SAMPLE_RATE}"),
            });
        }
        if self.events.len() > MAX_LOGGED_EVENTS {
            return Err(EventLogError::Bound(format!(
                "more than {MAX_LOGGED_EVENTS} events"
            )));
        }
        let mut last: std::collections::BTreeMap<&str, u64> = Default::default();
        for (index, event) in self.events.iter().enumerate() {
            if !valid_clip_name(&event.scene) {
                return Err(invalid(
                    index,
                    "scene",
                    "scene ID must be a valid identifier",
                ));
            }
            if !event.value.allows(event.source) {
                return Err(invalid(
                    index,
                    "source",
                    format!(
                        "{} events cannot come from {:?} input",
                        event.value.kind(),
                        event.source
                    ),
                ));
            }
            match &event.value {
                ExternalEvent::Key { key } => {
                    if !valid_key(key) {
                        return Err(invalid(
                            index,
                            "value.key",
                            format!(
                                "key must be 1..{MAX_KEY_NAME} ASCII letters, digits, '_' or '-'"
                            ),
                        ));
                    }
                }
                ExternalEvent::Drag { actor, joint, x, y } => {
                    check_target(index, actor, joint)?;
                    for (name, v) in [("x", *x), ("y", *y)] {
                        if !v.is_finite() || v.abs() > MAX_DRAG_COORDINATE {
                            return Err(invalid(
                                index,
                                &format!("value.{name}"),
                                format!("must be finite with magnitude <= {MAX_DRAG_COORDINATE}"),
                            ));
                        }
                    }
                }
                ExternalEvent::CameraPose { roll, yaw, pitch } => {
                    if !roll.is_finite() || roll.abs() > std::f64::consts::FRAC_PI_2 {
                        return Err(invalid(
                            index,
                            "value.roll",
                            "must be finite radians within ±pi/2",
                        ));
                    }
                    for (name, v) in [("yaw", yaw), ("pitch", pitch)] {
                        if v.is_some_and(|v| !v.is_finite() || v.abs() > std::f64::consts::PI) {
                            return Err(invalid(
                                index,
                                &format!("value.{name}"),
                                "must be finite radians within ±pi",
                            ));
                        }
                    }
                }
                ExternalEvent::Release { actor, joint } => check_target(index, actor, joint)?,
            }
            if let Some(previous) = last.insert(&event.scene, event.sample) {
                if event.sample < previous {
                    return Err(EventLogError::Order {
                        path: path(index, "sample"),
                        scene: event.scene.clone(),
                        sample: event.sample,
                        previous,
                    });
                }
            }
        }
        Ok(())
    }

    /// [`validate`](Self::validate), then check every event against `script`:
    /// the scene exists, the sample lies inside `[0, duration)`, and drag or
    /// release targets name an existing actor and one of its joints (rig joints
    /// for custom rigs, [`HUMANOID_BONES`] otherwise).
    pub fn validate_for(&self, script: &Script) -> Result<(), EventLogError> {
        self.validate()?;
        for (index, event) in self.events.iter().enumerate() {
            let Some(scene) = script.scenes.iter().find(|s| s.id == event.scene) else {
                return Err(EventLogError::UnknownReference {
                    path: path(index, "scene"),
                    kind: "scene",
                    name: event.scene.clone(),
                });
            };
            if self.seconds(event.sample) >= scene.duration {
                return Err(invalid(
                    index,
                    "sample",
                    format!(
                        "sample {} at {} Hz is not before the end of scene {} ({} s)",
                        event.sample, self.sample_rate, scene.id, scene.duration
                    ),
                ));
            }
            if let ExternalEvent::Drag { actor, joint, .. }
            | ExternalEvent::Release { actor, joint } = &event.value
            {
                let Some(character) = script.characters.get(actor) else {
                    return Err(EventLogError::UnknownReference {
                        path: path(index, "value.actor"),
                        kind: "actor",
                        name: actor.clone(),
                    });
                };
                let known = match &character.rig {
                    Some(rig) => rig.joints.iter().any(|j| &j.id == joint),
                    None => HUMANOID_BONES.contains(&joint.as_str()),
                };
                if !known {
                    return Err(EventLogError::UnknownReference {
                        path: path(index, "value.joint"),
                        kind: "joint",
                        name: joint.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Events of one scene in log order, with their log indices.
    pub fn scene_events<'a>(
        &'a self,
        scene: &'a str,
    ) -> impl Iterator<Item = (usize, &'a LoggedEvent)> + 'a {
        self.events
            .iter()
            .enumerate()
            .filter(move |(_, e)| e.scene == scene)
    }
}

pub(crate) fn valid_key(key: &str) -> bool {
    key.len() <= MAX_KEY_NAME && valid_clip_name(key)
}

fn check_target(index: usize, actor: &str, joint: &str) -> Result<(), EventLogError> {
    if !valid_clip_name(actor) {
        return Err(invalid(
            index,
            "value.actor",
            "actor must be a valid identifier",
        ));
    }
    if !valid_clip_name(joint) {
        return Err(invalid(
            index,
            "value.joint",
            "joint must be a valid identifier",
        ));
    }
    Ok(())
}
