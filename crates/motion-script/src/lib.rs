//! Strict MotionScript scene and dialogue schema.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceConfig {
    pub provider: String,
    pub preset: String,
    pub language: String,
    pub speed: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Character {
    #[serde(default)]
    pub default_pose: Option<String>,
    pub asset: String,
    pub voice: Option<VoiceConfig>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Move {
        at: f64,
        actor: String,
        x: f64,
        y: f64,
        duration: f64,
    },
    Emotion {
        at: f64,
        actor: String,
        value: String,
    },
    Gesture {
        at: f64,
        actor: String,
        value: String,
        duration: f64,
    },
    Speak {
        at: f64,
        actor: String,
        text: String,
        emotion: Option<String>,
        audio: Option<String>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    #[serde(default)]
    pub background: Option<String>,
    pub id: String,
    pub duration: f64,
    pub events: Vec<Event>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Project {
    pub title: String,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
}
impl Default for Project {
    fn default() -> Self {
        Self {
            title: "Untitled".into(),
            fps: 24,
            width: 1920,
            height: 1080,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    #[serde(default)]
    pub project: Project,
    pub characters: BTreeMap<String, Character>,
    pub scenes: Vec<Scene>,
}
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("MotionScript syntax: {0}")]
    Syntax(#[from] serde_yaml::Error),
    #[error("MotionScript unsupported: {0}")]
    Unsupported(String),
    #[error("MotionScript validation: {0}")]
    Validation(String),
}
pub fn parse(source: &str) -> Result<Script, Error> {
    let document: serde_yaml::Value = serde_yaml::from_str(source)?;
    if let Some(scenes) = document
        .get("scenes")
        .and_then(serde_yaml::Value::as_sequence)
    {
        for scene in scenes {
            if let Some(events) = scene.get("events").and_then(serde_yaml::Value::as_sequence) {
                for event in events {
                    if let Some(kind) = event.get("type").and_then(serde_yaml::Value::as_str) {
                        if !["speak", "move", "emotion", "gesture"].contains(&kind) {
                            return Err(Error::Unsupported(format!("event {kind}: supported commands are speak, move, emotion, gesture")));
                        }
                    }
                }
            }
        }
    }
    let script: Script = serde_yaml::from_value(document)?;
    script.validate()?;
    Ok(script)
}
/// Replace one event atomically after validating the entire candidate screenplay.
///
/// YAML is reserialized: formatting is normalized and comments are not preserved.
/// Input and output are limited to one MiB. This function has no file side effects.
pub fn apply_event(
    source: &str,
    scene_index: usize,
    event_index: usize,
    replacement: Event,
) -> Result<String, Error> {
    const LIMIT: usize = 1_048_576;
    if source.len() > LIMIT {
        return Err(Error::Validation("screenplay exceeds one MiB".into()));
    }
    let mut candidate = parse(source)?;
    let event = candidate
        .scenes
        .get_mut(scene_index)
        .and_then(|scene| scene.events.get_mut(event_index))
        .ok_or_else(|| Error::Validation("selected scene or event no longer exists".into()))?;
    *event = replacement;
    candidate.validate()?;
    let output = serde_yaml::to_string(&candidate)?;
    if output.len() > LIMIT {
        return Err(Error::Validation(
            "updated screenplay exceeds one MiB".into(),
        ));
    }
    Ok(output)
}

impl Script {
    pub fn validate(&self) -> Result<(), Error> {
        let fail = |s: String| Error::Validation(s);
        if self.project.title.trim().is_empty()
            || !(1..=240).contains(&self.project.fps)
            || self.project.width == 0
            || self.project.height == 0
            || self.project.width > 16384
            || self.project.height > 16384
        {
            return Err(fail(
                "project requires a title, fps 1..240, and dimensions 1..16384".into(),
            ));
        }
        for (id, c) in &self.characters {
            if c.default_pose.as_deref().is_some_and(|p| p != "idle") {
                return Err(Error::Unsupported(format!(
                    "{id}: default_pose supports only idle"
                )));
            }
            if id.trim().is_empty() || c.asset.trim().is_empty() {
                return Err(fail("empty character or asset".into()));
            }
            if let Some(v) = &c.voice {
                if v.provider != "kokoro"
                    || !["en-us", "en-gb"].contains(&v.language.as_str())
                    || !v.speed.is_finite()
                    || !(0.5..=2.0).contains(&v.speed)
                    || v.preset.trim().is_empty()
                {
                    return Err(fail(format!(
                        "{id}: expected kokoro, English language, preset, and speed 0.5..2.0"
                    )));
                }
            }
        }
        let mut ids = std::collections::BTreeSet::new();
        for s in &self.scenes {
            if s.id.trim().is_empty()
                || !ids.insert(&s.id)
                || !s.duration.is_finite()
                || s.duration <= 0.0
            {
                return Err(fail("invalid or duplicate scene".into()));
            }
            if s.background.as_ref().is_some_and(|b| b.trim().is_empty()) {
                return Err(fail(format!("{}: background must not be empty", s.id)));
            }
            let mut moves: BTreeMap<&str, Vec<(f64, f64)>> = BTreeMap::new();
            for e in &s.events {
                let (at, actor) = e.actor_time();
                let c = self
                    .characters
                    .get(actor)
                    .ok_or_else(|| fail(format!("unknown actor {actor}")))?;
                if !at.is_finite() || at < 0.0 || at >= s.duration {
                    return Err(fail(format!("{}: event time must be within scene", s.id)));
                }
                match e {
                    Event::Speak {
                        text,
                        audio,
                        emotion,
                        ..
                    } => {
                        if text.trim().is_empty()
                            || audio.as_ref().is_some_and(|a| a.trim().is_empty())
                        {
                            return Err(fail(format!("{}: invalid dialogue text/audio", s.id)));
                        }
                        if c.voice.is_none() && audio.is_none() {
                            return Err(fail(format!(
                                "{actor}: configure voice or imported audio"
                            )));
                        }
                        if let Some(value) = emotion {
                            validate_emotion(value)?;
                        }
                    }
                    Event::Move { x, y, duration, .. } => {
                        if !x.is_finite()
                            || !y.is_finite()
                            || *x < 0.0
                            || *y < 0.0
                            || *x > self.project.width as f64
                            || *y > self.project.height as f64
                        {
                            return Err(fail(format!(
                                "{actor}: move coordinates must be within project dimensions"
                            )));
                        }
                        validate_duration(*duration, at, s.duration)?;
                        moves.entry(actor).or_default().push((at, at + duration));
                    }
                    Event::Emotion { value, .. } => validate_emotion(value)?,
                    Event::Gesture {
                        value, duration, ..
                    } => {
                        if value != "wave" {
                            return Err(Error::Unsupported(format!(
                                "gesture {value}: supported gesture is wave"
                            )));
                        }
                        validate_duration(*duration, at, s.duration)?;
                    }
                }
            }
            for (actor, intervals) in &mut moves {
                intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
                if intervals.windows(2).any(|w| w[0].1 > w[1].0) {
                    return Err(fail(format!(
                        "{actor}: overlapping move events in scene {}",
                        s.id
                    )));
                }
            }
        }
        Ok(())
    }
}
impl Event {
    pub fn actor_time(&self) -> (f64, &str) {
        match self {
            Self::Speak { at, actor, .. }
            | Self::Move { at, actor, .. }
            | Self::Emotion { at, actor, .. }
            | Self::Gesture { at, actor, .. } => (*at, actor),
        }
    }
}
fn validate_emotion(value: &str) -> Result<(), Error> {
    if !["neutral", "happy", "sad", "surprised", "angry"].contains(&value) {
        return Err(Error::Unsupported(format!(
            "emotion {value}: supported values are neutral, happy, sad, surprised, angry"
        )));
    }
    Ok(())
}
fn validate_duration(duration: f64, at: f64, scene_duration: f64) -> Result<(), Error> {
    if !duration.is_finite() || duration <= 0.0 || at + duration > scene_duration {
        return Err(Error::Validation(
            "event duration must be positive, finite, and end within scene".into(),
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn example_and_unknown_actor() {
        let s = include_str!("../../../examples/dialogue.yaml");
        assert!(parse(s).is_ok());
        assert!(parse(&s.replace("actor: EMMA", "actor: MISSING")).is_err());
        assert!(parse(&s.replace("speed: 1.0", "speed: .nan")).is_err());
        assert!(parse(&s.replace("type: speak", "type: dance")).is_err());
    }
}

#[cfg(test)]
mod edit_tests {
    use super::*;
    const SOURCE: &str = "characters:\n  EMMA:\n    asset: characters/emma\nscenes:\n  - id: intro\n    duration: 5\n    events:\n      - type: move\n        actor: EMMA\n        at: 0\n        x: 10\n        y: 20\n        duration: 1\n";
    fn movement(actor: &str, at: f64) -> Event {
        Event::Move {
            actor: actor.into(),
            at,
            x: 200.0,
            y: 100.0,
            duration: 1.0,
        }
    }
    #[test]
    fn valid_move_and_imported_dialogue_roundtrip() {
        let moved = apply_event(SOURCE, 0, 0, movement("EMMA", 2.0)).unwrap();
        assert!(matches!(
            parse(&moved).unwrap().scenes[0].events[0],
            Event::Move {
                at: 2.0,
                x: 200.0,
                ..
            }
        ));
        let spoken = apply_event(
            &moved,
            0,
            0,
            Event::Speak {
                at: 1.0,
                actor: "EMMA".into(),
                text: "Hello, studio!".into(),
                emotion: Some("happy".into()),
                audio: Some("voice.wav".into()),
            },
        )
        .unwrap();
        assert!(
            matches!(&parse(&spoken).unwrap().scenes[0].events[0], Event::Speak {text, ..} if text == "Hello, studio!")
        );
    }
    #[test]
    fn invalid_updates_leave_source_unchanged() {
        let original = SOURCE.to_owned();
        for event in [
            movement("MISSING", 0.0),
            movement("EMMA", 4.5),
            movement("EMMA", f64::NAN),
            Event::Emotion {
                actor: "EMMA".into(),
                at: 0.0,
                value: "unsupported".into(),
            },
        ] {
            assert!(apply_event(&original, 0, 0, event).is_err());
            assert_eq!(original, SOURCE);
            assert!(parse(&original).is_ok());
        }
        assert!(apply_event(SOURCE, 1, 0, movement("EMMA", 0.0)).is_err());
        assert!(apply_event(SOURCE, 0, 1, movement("EMMA", 0.0)).is_err());
    }
    #[test]
    fn bounds_input_and_output() {
        assert!(apply_event(&" ".repeat(1_048_577), 0, 0, movement("EMMA", 0.0)).is_err());
        let event = Event::Speak {
            at: 0.0,
            actor: "EMMA".into(),
            text: "a".repeat(1_048_577),
            emotion: None,
            audio: Some("voice.wav".into()),
        };
        assert!(apply_event(SOURCE, 0, 0, event).is_err());
    }
}
