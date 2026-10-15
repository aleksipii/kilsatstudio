//! Agent 6 dialogue schema. Other screenplay commands are deliberately unsupported.
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
    pub asset: String,
    pub voice: Option<VoiceConfig>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
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
    pub id: String,
    pub duration: f64,
    pub events: Vec<Event>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub characters: BTreeMap<String, Character>,
    pub scenes: Vec<Scene>,
}
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("MotionScript syntax: {0}")]
    Syntax(#[from] serde_yaml::Error),
    #[error("MotionScript validation: {0}")]
    Validation(String),
}
pub fn parse(source: &str) -> Result<Script, Error> {
    let script: Script = serde_yaml::from_str(source)?;
    script.validate()?;
    Ok(script)
}
impl Script {
    pub fn validate(&self) -> Result<(), Error> {
        let fail = |s: String| Error::Validation(s);
        for (id, c) in &self.characters {
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
            for e in &s.events {
                let Event::Speak {
                    at,
                    actor,
                    text,
                    audio,
                    ..
                } = e;
                let c = self
                    .characters
                    .get(actor)
                    .ok_or_else(|| fail(format!("unknown actor {actor}")))?;
                if !at.is_finite() || *at < 0.0 || *at >= s.duration || text.trim().is_empty() {
                    return Err(fail(format!("{}: invalid dialogue time/text", s.id)));
                }
                if c.voice.is_none() && audio.is_none() {
                    return Err(fail(format!("{actor}: configure voice or imported audio")));
                }
            }
        }
        Ok(())
    }
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
