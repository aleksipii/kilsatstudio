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
/// Scene-local composition; camera coordinates are offsets from the canvas center.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Composition {
    pub background_color: String,
    pub background_keys: Vec<BackgroundKey>,
    pub camera: Vec<CameraKey>,
    pub layers: BTreeMap<String, Layer>,
    pub actor_tracks: BTreeMap<String, Vec<ActorKey>>,
    pub fade_in: f64,
    pub fade_out: f64,
}
impl Default for Composition {
    fn default() -> Self {
        Self {
            background_color: "#181f2c".into(),
            background_keys: vec![],
            camera: vec![],
            layers: BTreeMap::new(),
            actor_tracks: BTreeMap::new(),
            fade_in: 0.0,
            fade_out: 0.0,
        }
    }
}
/// A scene-local step change, effective at and after its timestamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackgroundKey {
    pub at: f64,
    pub color: String,
}
fn safe_background_color(color: &str) -> bool {
    let bytes = color.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(u8::is_ascii_hexdigit)
}
/// Root offsets layered over movement events; rotation is in radians.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ActorKey {
    pub at: f64,
    pub x: f64,
    pub y: f64,
    pub rotation: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub easing: ActorEasing,
}
impl Default for ActorKey {
    fn default() -> Self {
        Self {
            at: 0.0,
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            easing: ActorEasing::Linear,
        }
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorEasing {
    #[default]
    Linear,
    #[serde(rename = "smoothstep")]
    Smoothstep,
    Hold,
}
impl ActorKey {
    pub fn animation_key(&self) -> motion_animation::Keyframe {
        motion_animation::Keyframe {
            time: self.at,
            value: motion_animation::Transform2 {
                translation: motion_animation::Vec2::new(self.x, self.y),
                rotation: self.rotation,
                scale: motion_animation::Vec2::new(self.scale_x, self.scale_y),
            },
            easing: match self.easing {
                ActorEasing::Linear => motion_animation::Easing::Linear,
                ActorEasing::Smoothstep => motion_animation::Easing::SmoothStep,
                ActorEasing::Hold => motion_animation::Easing::Hold,
            },
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraKey {
    pub at: f64,
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Layer {
    pub z: i32,
    pub visible: bool,
}
impl Default for Layer {
    fn default() -> Self {
        Self {
            z: 0,
            visible: true,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    #[serde(default)]
    pub composition: Composition,
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
    edit_source(source, |candidate| {
        let event = candidate
            .scenes
            .get_mut(scene_index)
            .and_then(|scene| scene.events.get_mut(event_index))
            .ok_or_else(missing_event)?;
        *event = replacement;
        Ok(())
    })
}

/// Insert an event before `event_index`, or append when it equals the event count.
///
/// The entire candidate screenplay is validated before returning normalized YAML.
/// Comments are not preserved. Input/output are bounded to one MiB; no files change.
pub fn insert_event(
    source: &str,
    scene_index: usize,
    event_index: usize,
    event: Event,
) -> Result<String, Error> {
    edit_source(source, |candidate| {
        let scene = candidate
            .scenes
            .get_mut(scene_index)
            .ok_or_else(missing_event)?;
        if event_index > scene.events.len() {
            return Err(missing_event());
        }
        scene.events.insert(event_index, event);
        Ok(())
    })
}

/// Remove an event atomically, returning validated, normalized YAML.
///
/// Empty scenes remain valid. Comments are not preserved; input/output are bounded
/// to one MiB. This function has no file side effects.
pub fn remove_event(source: &str, scene_index: usize, event_index: usize) -> Result<String, Error> {
    edit_source(source, |candidate| {
        let scene = candidate
            .scenes
            .get_mut(scene_index)
            .ok_or_else(missing_event)?;
        if event_index >= scene.events.len() {
            return Err(missing_event());
        }
        scene.events.remove(event_index);
        Ok(())
    })
}

/// Set or remove a character's voice atomically after validating the whole script.
///
/// Removing a voice requires imported audio for every spoken line of that actor.
/// YAML formatting is normalized and comments are not preserved. Input/output are
/// bounded to one MiB; this function has no file side effects.
pub fn apply_voice(source: &str, actor: &str, voice: Option<VoiceConfig>) -> Result<String, Error> {
    edit_source(source, |candidate| {
        let character = candidate
            .characters
            .get_mut(actor)
            .ok_or_else(|| Error::Validation(format!("unknown actor {actor}")))?;
        character.voice = voice;
        Ok(())
    })
}

fn missing_event() -> Error {
    Error::Validation("selected scene or event no longer exists".into())
}

/// Update project settings and one scene's identity/duration as one transaction.
///
/// The whole candidate screenplay is validated, including events in other scenes
/// affected by project dimensions. Imported audio is not loaded or measured.
/// YAML formatting is normalized and comments are not preserved. Input/output
/// are bounded to one MiB; this function has no file side effects.
pub fn apply_scene_settings(
    source: &str,
    scene_index: usize,
    project: Project,
    scene_id: &str,
    duration_seconds: f64,
) -> Result<String, Error> {
    edit_source(source, |candidate| {
        let scene = candidate
            .scenes
            .get_mut(scene_index)
            .ok_or_else(|| Error::Validation("selected scene no longer exists".into()))?;
        scene.id = scene_id.into();
        scene.duration = duration_seconds;
        candidate.project = project;
        Ok(())
    })
}

/// Update scene settings and composition together, validating only the final candidate.
/// This permits duration edits accompanied by retiming keys in the same transaction.
/// Input/output bounds and YAML normalization match `apply_scene_settings`.
pub fn apply_scene_settings_with_composition(
    source: &str,
    scene_index: usize,
    project: Project,
    scene_id: &str,
    duration_seconds: f64,
    composition: Composition,
) -> Result<String, Error> {
    edit_source(source, |candidate| {
        let scene = candidate
            .scenes
            .get_mut(scene_index)
            .ok_or_else(missing_event)?;
        scene.id = scene_id.into();
        scene.duration = duration_seconds;
        scene.composition = composition;
        candidate.project = project;
        Ok(())
    })
}

/// Replace scene composition atomically; validate all camera keys and actor references.
/// YAML formatting is normalized and comments are removed; no files are changed.
pub fn apply_composition(
    source: &str,
    scene_index: usize,
    composition: Composition,
) -> Result<String, Error> {
    edit_source(source, |candidate| {
        candidate
            .scenes
            .get_mut(scene_index)
            .ok_or_else(missing_event)?
            .composition = composition;
        Ok(())
    })
}

/// Replace or remove one actor's root track atomically, validating the entire script.
pub fn apply_actor_track(
    source: &str,
    scene_index: usize,
    actor: &str,
    keys: &[ActorKey],
) -> Result<String, Error> {
    edit_source(source, |candidate| {
        if !candidate.characters.contains_key(actor) {
            return Err(Error::Validation(format!("unknown track actor {actor}")));
        }
        let scene = candidate
            .scenes
            .get_mut(scene_index)
            .ok_or_else(missing_event)?;
        if keys.is_empty() {
            scene.composition.actor_tracks.remove(actor);
        } else {
            scene
                .composition
                .actor_tracks
                .insert(actor.into(), keys.to_vec());
        }
        Ok(())
    })
}

/// Append a scene after validating the complete candidate screenplay.
pub fn add_scene(source: &str, scene: Scene) -> Result<String, Error> {
    edit_source(source, |script| {
        script.scenes.push(scene);
        Ok(())
    })
}

/// Duplicate one scene, preserving its events and requiring a unique new identifier.
pub fn duplicate_scene(source: &str, index: usize, new_id: &str) -> Result<String, Error> {
    edit_source(source, |script| {
        let mut scene = script
            .scenes
            .get(index)
            .cloned()
            .ok_or_else(|| Error::Validation("selected scene no longer exists".into()))?;
        scene.id = new_id.into();
        script.scenes.insert(index + 1, scene);
        Ok(())
    })
}

/// Remove a scene; a screenplay must retain at least one scene.
pub fn remove_scene(source: &str, index: usize) -> Result<String, Error> {
    edit_source(source, |script| {
        if index >= script.scenes.len() {
            return Err(Error::Validation("selected scene no longer exists".into()));
        }
        if script.scenes.len() == 1 {
            return Err(Error::Validation("cannot remove the final scene".into()));
        }
        script.scenes.remove(index);
        Ok(())
    })
}

/// Add a character without replacing an existing stable identifier.
pub fn add_character(source: &str, id: &str, character: Character) -> Result<String, Error> {
    edit_source(source, |script| {
        if script.characters.contains_key(id) {
            return Err(Error::Validation(format!("character {id} already exists")));
        }
        script.characters.insert(id.into(), character);
        Ok(())
    })
}

/// Remove an unused character and its layer metadata atomically.
/// Event references in any scene reject the transaction.
pub fn remove_character(source: &str, id: &str) -> Result<String, Error> {
    edit_source(source, |script| {
        if script.characters.remove(id).is_none() {
            return Err(Error::Validation(format!(
                "character {id} no longer exists"
            )));
        }
        for scene in &mut script.scenes {
            scene.composition.layers.remove(id);
            scene.composition.actor_tracks.remove(id);
        }
        Ok(())
    })
}

fn edit_source(
    source: &str,
    edit: impl FnOnce(&mut Script) -> Result<(), Error>,
) -> Result<String, Error> {
    const LIMIT: usize = 1_048_576;
    if source.len() > LIMIT {
        return Err(Error::Validation("screenplay exceeds one MiB".into()));
    }
    let mut candidate = parse(source)?;
    edit(&mut candidate)?;
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
                    || v.preset.is_empty()
                    || v.preset.len() > 128
                    || !v
                        .preset
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_')
                {
                    return Err(fail(format!(
                        "{id}: expected kokoro, en-us/en-gb, preset of 1..128 ASCII letters/digits/underscores, and speed 0.5..2.0"
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
            let composition = &s.composition;
            if !safe_background_color(&composition.background_color) {
                return Err(fail(format!("{}: background color requires #RRGGBB", s.id)));
            }
            if composition.background_keys.len() > 4096 {
                return Err(fail("background exceeds 4096 keys".into()));
            }
            let mut previous = None;
            for key in &composition.background_keys {
                if !key.at.is_finite()
                    || key.at < 0.0
                    || key.at > s.duration
                    || previous.is_some_and(|at| key.at <= at)
                    || !safe_background_color(&key.color)
                {
                    return Err(fail(format!(
                        "{}: background keys require increasing scene times and #RRGGBB colors",
                        s.id
                    )));
                }
                previous = Some(key.at);
            }
            if [composition.fade_in, composition.fade_out]
                .iter()
                .any(|v| !v.is_finite() || *v < 0.0)
                || composition.fade_in + composition.fade_out > s.duration
            {
                return Err(fail(format!(
                    "{}: fades must be finite, nonnegative, and fit the scene",
                    s.id
                )));
            }
            if composition.camera.len() > 4096 {
                return Err(fail("camera exceeds 4096 keys".into()));
            }
            let mut previous = None;
            for key in &composition.camera {
                if !key.at.is_finite()
                    || key.at < 0.0
                    || key.at > s.duration
                    || previous.is_some_and(|at| key.at <= at)
                    || !key.x.is_finite()
                    || !key.y.is_finite()
                    || key.x.abs() > 1_000_000.0
                    || key.y.abs() > 1_000_000.0
                    || !key.zoom.is_finite()
                    || !(0.05..=20.0).contains(&key.zoom)
                {
                    return Err(fail(format!("{}: camera keys require increasing scene times, finite offsets and zoom 0.05..20", s.id)));
                }
                previous = Some(key.at);
            }
            for actor in composition.layers.keys() {
                if !self.characters.contains_key(actor) {
                    return Err(fail(format!("unknown layer actor {actor}")));
                }
            }
            for (actor, keys) in &composition.actor_tracks {
                if !self.characters.contains_key(actor) {
                    return Err(fail(format!("unknown track actor {actor}")));
                }
                if keys.iter().any(|key| key.at > s.duration)
                    || motion_animation::Track::new(
                        keys.iter().map(ActorKey::animation_key).collect(),
                    )
                    .is_err()
                {
                    return Err(fail(format!("{}: actor tracks need 1..4096 increasing scene times and bounded finite transforms with positive scale", s.id)));
                }
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
                        if motion_animation::Gesture::parse(value).is_none() {
                            return Err(Error::Unsupported(format!(
                                "gesture {value}: supported gestures are {}",
                                motion_animation::GESTURES.join(", ")
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
    #[test]
    fn lifecycle_preserves_order_and_roundtrips_fields() {
        let appended = insert_event(SOURCE, 0, 1, movement("EMMA", 3.0)).unwrap();
        let inserted = insert_event(
            &appended,
            0,
            1,
            Event::Speak {
                at: 2.0,
                actor: "EMMA".into(),
                text: "Hello!".into(),
                emotion: Some("happy".into()),
                audio: Some("dialogue.wav".into()),
            },
        )
        .unwrap();
        let script = parse(&inserted).unwrap();
        assert_eq!(
            script.scenes[0]
                .events
                .iter()
                .map(|e| e.actor_time().0)
                .collect::<Vec<_>>(),
            vec![0.0, 2.0, 3.0]
        );
        assert!(
            matches!(&script.scenes[0].events[1], Event::Speak {text, emotion: Some(emotion), audio: Some(audio), ..} if text == "Hello!" && emotion == "happy" && audio == "dialogue.wav")
        );
        let removed = remove_event(&inserted, 0, 1).unwrap();
        assert_eq!(
            serde_yaml::to_value(parse(&removed).unwrap()).unwrap(),
            serde_yaml::to_value(parse(&appended).unwrap()).unwrap()
        );
    }
    #[test]
    fn lifecycle_accepts_empty_scene() {
        let empty = remove_event(SOURCE, 0, 0).unwrap();
        assert!(parse(&empty).unwrap().scenes[0].events.is_empty());
        let restored = insert_event(&empty, 0, 0, movement("EMMA", 0.0)).unwrap();
        assert_eq!(parse(&restored).unwrap().scenes[0].events.len(), 1);
        assert!(remove_event(&empty, 0, 0).is_err());
    }
    #[test]
    fn lifecycle_rejects_invalid_candidates_without_mutating_source() {
        let original = SOURCE.to_owned();
        for event in [
            movement("MISSING", 2.0),
            movement("EMMA", 0.5),
            movement("EMMA", 4.5),
        ] {
            assert!(insert_event(&original, 0, 1, event).is_err());
            assert_eq!(original, SOURCE);
        }
        for (scene, event) in [(1, 0), (0, 2), (usize::MAX, usize::MAX)] {
            assert!(insert_event(&original, scene, event, movement("EMMA", 2.0)).is_err());
            assert!(remove_event(&original, scene, event).is_err());
        }
        assert!(remove_event(&original, 0, 1).is_err());
        assert_eq!(original, SOURCE);
        let oversized = " ".repeat(1_048_577);
        assert!(insert_event(&oversized, 0, 0, movement("EMMA", 2.0)).is_err());
        assert!(remove_event(&oversized, 0, 0).is_err());
        let huge = Event::Speak {
            at: 2.0,
            actor: "EMMA".into(),
            text: "a".repeat(1_048_577),
            emotion: None,
            audio: Some("voice.wav".into()),
        };
        assert!(insert_event(SOURCE, 0, 1, huge).is_err());
    }
}

#[cfg(test)]
mod voice_edit_tests {
    use super::*;
    const SOURCE: &str = "characters:\n  EMMA:\n    asset: characters/emma\n    voice: { provider: kokoro, preset: af_heart, language: en-us, speed: 1.0 }\n  ALEX:\n    asset: characters/alex\n    voice: { provider: kokoro, preset: am_michael, language: en-us, speed: 1.0 }\nscenes:\n  - id: intro\n    duration: 5\n    events:\n      - { type: speak, actor: EMMA, at: 0, text: Hello, audio: imported.wav }\n      - { type: speak, actor: ALEX, at: 1, text: Welcome }\n";
    fn voice() -> VoiceConfig {
        VoiceConfig {
            provider: "kokoro".into(),
            preset: "bf_emma".into(),
            language: "en-gb".into(),
            speed: 1.25,
        }
    }
    #[test]
    fn updates_preserve_other_characters_events_and_imports() {
        let before = parse(SOURCE).unwrap();
        let updated = parse(&apply_voice(SOURCE, "EMMA", Some(voice())).unwrap()).unwrap();
        assert_eq!(
            updated.characters["EMMA"].voice.as_ref().unwrap().preset,
            "bf_emma"
        );
        assert_eq!(
            updated.characters["EMMA"].voice.as_ref().unwrap().language,
            "en-gb"
        );
        assert_eq!(
            updated.characters["EMMA"].voice.as_ref().unwrap().speed,
            1.25
        );
        assert_eq!(
            serde_yaml::to_value(&updated.characters["ALEX"]).unwrap(),
            serde_yaml::to_value(&before.characters["ALEX"]).unwrap()
        );
        assert_eq!(
            serde_yaml::to_value(&updated.scenes).unwrap(),
            serde_yaml::to_value(&before.scenes).unwrap()
        );
    }
    #[test]
    fn removing_voice_requires_imports_for_every_spoken_line() {
        let updated = apply_voice(SOURCE, "EMMA", None).unwrap();
        assert!(parse(&updated).unwrap().characters["EMMA"].voice.is_none());
        assert!(apply_voice(SOURCE, "ALEX", None).is_err());
        let unspoken = SOURCE.replace("actor: ALEX", "actor: EMMA");
        assert!(apply_voice(&unspoken, "ALEX", None).is_ok());
        assert!(apply_voice(&unspoken, "EMMA", None).is_err());
    }
    #[test]
    fn invalid_configs_are_atomic_and_presets_are_safe_identifiers() {
        let original = SOURCE.to_owned();
        assert!(apply_voice(&original, "MISSING", Some(voice())).is_err());
        let mut invalid = Vec::new();
        let mut v = voice();
        v.provider = "cloud".into();
        invalid.push(v);
        let mut v = voice();
        v.language = "fi".into();
        invalid.push(v);
        for preset in [
            "",
            "../af_heart",
            "voices/af_heart",
            "af-heart",
            "ä",
            "af heart",
            &"a".repeat(129),
        ] {
            let mut v = voice();
            v.preset = preset.into();
            invalid.push(v);
        }
        for speed in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.49, 2.01] {
            let mut v = voice();
            v.speed = speed;
            invalid.push(v);
        }
        for v in invalid {
            assert!(apply_voice(&original, "EMMA", Some(v)).is_err());
            assert_eq!(original, SOURCE);
        }
        for speed in [0.5, 2.0] {
            let mut v = voice();
            v.speed = speed;
            v.preset = "A".repeat(128);
            assert!(apply_voice(SOURCE, "EMMA", Some(v)).is_ok());
        }
        assert!(parse(&SOURCE.replace("af_heart", "../af_heart")).is_err());
    }
    #[test]
    fn voice_edits_share_document_size_bounds() {
        assert!(apply_voice(&" ".repeat(1_048_577), "EMMA", Some(voice())).is_err());
        // Build a valid source just below the bound, then expand its short preset.
        let mut source = parse(SOURCE).unwrap();
        source
            .characters
            .get_mut("EMMA")
            .unwrap()
            .voice
            .as_mut()
            .unwrap()
            .preset = "a".into();
        source.project.title = "a".into();
        let base = serde_yaml::to_string(&source).unwrap();
        source.project.title = "a".repeat(1_048_560 - base.len() + 1);
        let bounded = serde_yaml::to_string(&source).unwrap();
        assert_eq!(bounded.len(), 1_048_560);
        let mut v = voice();
        v.preset = "a".repeat(128);
        assert!(apply_voice(&bounded, "EMMA", Some(v)).is_err());
    }
}

#[cfg(test)]
mod scene_settings_tests {
    use super::*;

    const SOURCE: &str = "project: {title: Studio, fps: 24, width: 1920, height: 1080}\ncharacters:\n  EMMA:\n    asset: characters/emma\n    default_pose: idle\n    voice: {provider: kokoro, preset: af_heart, language: en-us, speed: 1.0}\nscenes:\n  - id: intro\n    duration: 5\n    background: backgrounds/studio.svg\n    events:\n      - {type: speak, actor: EMMA, at: 1, text: Hello, audio: imported.wav}\n      - {type: gesture, actor: EMMA, at: 2, value: wave, duration: 2}\n  - id: outro\n    duration: 8\n    background: backgrounds/outro.svg\n    events:\n      - {type: move, actor: EMMA, at: 1, x: 800, y: 500, duration: 2}\n";

    #[test]
    fn settings_transaction_preserves_scenes_characters_and_audio() {
        let before = parse(SOURCE).unwrap();
        let project = Project {
            title: "Revised Studio".into(),
            fps: 60,
            width: 1280,
            height: 720,
        };
        let after =
            parse(&apply_scene_settings(SOURCE, 0, project.clone(), "opening", 6.0).unwrap())
                .unwrap();
        assert_eq!(
            serde_yaml::to_value(&after.project).unwrap(),
            serde_yaml::to_value(project).unwrap()
        );
        assert_eq!(after.scenes[0].id, "opening");
        assert_eq!(after.scenes[0].duration, 6.0);
        assert_eq!(after.scenes[0].background, before.scenes[0].background);
        assert_eq!(
            serde_yaml::to_value(&after.scenes[0].events).unwrap(),
            serde_yaml::to_value(&before.scenes[0].events).unwrap()
        );
        assert_eq!(
            serde_yaml::to_value(&after.scenes[1]).unwrap(),
            serde_yaml::to_value(&before.scenes[1]).unwrap()
        );
        assert_eq!(
            serde_yaml::to_value(&after.characters).unwrap(),
            serde_yaml::to_value(&before.characters).unwrap()
        );
    }

    #[test]
    fn invalid_settings_reject_the_entire_transaction() {
        let original = SOURCE.to_owned();
        let base = parse(SOURCE).unwrap().project;
        let mut projects = Vec::new();
        let mut p = base.clone();
        p.title = "  ".into();
        projects.push(p);
        for fps in [0, 241] {
            let mut p = base.clone();
            p.fps = fps;
            projects.push(p);
        }
        for (width, height) in [
            (0, 1080),
            (1920, 0),
            (16385, 1080),
            (1920, 16385),
            (799, 1080),
            (1920, 499),
        ] {
            let mut p = base.clone();
            p.width = width;
            p.height = height;
            projects.push(p);
        }
        for p in projects {
            assert!(apply_scene_settings(&original, 0, p, "changed", 6.0).is_err());
        }
        for id in ["", " ", "outro"] {
            assert!(apply_scene_settings(&original, 0, base.clone(), id, 6.0).is_err());
        }
        for duration in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1.0,
            0.0,
            1.0,
            3.99,
        ] {
            assert!(apply_scene_settings(&original, 0, base.clone(), "changed", duration).is_err());
        }
        for index in [2, usize::MAX] {
            assert!(apply_scene_settings(&original, index, base.clone(), "changed", 6.0).is_err());
        }
        assert_eq!(original, SOURCE);
        assert_eq!(parse(&original).unwrap().project.title, "Studio");
        // An exact event-end boundary and the FPS endpoints are valid.
        for fps in [1, 240] {
            let mut p = base.clone();
            p.fps = fps;
            assert!(apply_scene_settings(SOURCE, 0, p, "opening", 4.0).is_ok());
        }
    }

    #[test]
    fn settings_transactions_share_input_and_output_size_limits() {
        let project = parse(SOURCE).unwrap().project;
        assert!(
            apply_scene_settings(&" ".repeat(1_048_577), 0, project.clone(), "opening", 6.0)
                .is_err()
        );
        let mut huge_project = project.clone();
        huge_project.title = "a".repeat(1_048_577);
        assert!(apply_scene_settings(SOURCE, 0, huge_project, "opening", 6.0).is_err());
        assert!(apply_scene_settings(SOURCE, 0, project, &"a".repeat(1_048_577), 6.0).is_err());
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    const SOURCE: &str = "characters: {EMMA: {asset: emma}}\nscenes: [{id: intro, duration: 3, events: [{type: emotion, at: 0, actor: EMMA, value: happy}]}]\n";
    #[test]
    fn scene_lifecycle_preserves_events_and_rejects_last_or_duplicate() {
        let copy = duplicate_scene(SOURCE, 0, "copy").unwrap();
        let script = parse(&copy).unwrap();
        assert_eq!(script.scenes.len(), 2);
        assert_eq!(
            serde_yaml::to_value(&script.scenes[0].events).unwrap(),
            serde_yaml::to_value(&script.scenes[1].events).unwrap()
        );
        assert!(duplicate_scene(SOURCE, 0, "intro").is_err());
        assert!(duplicate_scene(SOURCE, usize::MAX, "copy").is_err());
        assert!(remove_scene(SOURCE, 0).is_err());
        assert_eq!(
            parse(&remove_scene(&copy, 0).unwrap()).unwrap().scenes[0].id,
            "copy"
        );
        assert!(add_scene(
            SOURCE,
            Scene {
                composition: Composition::default(),
                background: None,
                id: "next".into(),
                duration: 0.,
                events: vec![]
            }
        )
        .is_err());
    }
    #[test]
    fn character_removal_rejects_dangling_references() {
        assert!(remove_character(SOURCE, "EMMA").is_err());
        assert!(remove_character(SOURCE, "missing").is_err());
        let actor = parse(SOURCE).unwrap().characters["EMMA"].clone();
        assert!(add_character(SOURCE, "EMMA", actor.clone()).is_err());
        let added = add_character(SOURCE, "ALEX", actor).unwrap();
        let removed = remove_character(&added, "ALEX").unwrap();
        assert_eq!(parse(&removed).unwrap().characters.len(), 1);
        assert_eq!(parse(SOURCE).unwrap().scenes.len(), 1);
    }
}

#[cfg(test)]
mod composition_tests {
    use super::*;
    const SOURCE: &str =
        "characters: {EMMA: {asset: emma}}\nscenes: [{id: intro, duration: 4, events: []}]";
    #[test]
    fn deleting_unused_character_removes_layers_but_preserves_event_safety() {
        let mut composition = Composition::default();
        composition.layers.insert("EMMA".into(), Layer::default());
        let source = apply_composition(SOURCE, 0, composition).unwrap();
        let removed = parse(&remove_character(&source, "EMMA").unwrap()).unwrap();
        assert!(removed.characters.is_empty());
        assert!(removed.scenes[0].composition.layers.is_empty());
        let with_event = insert_event(
            &source,
            0,
            0,
            Event::Emotion {
                at: 0.0,
                actor: "EMMA".into(),
                value: "happy".into(),
            },
        )
        .unwrap();
        assert!(remove_character(&with_event, "EMMA").is_err());
        assert_eq!(
            parse(&with_event).unwrap().scenes[0]
                .composition
                .layers
                .len(),
            1
        );
    }
    #[test]
    fn composition_transaction_preserves_events_and_rejects_invalid_keys() {
        let mut composition = Composition {
            camera: vec![
                CameraKey {
                    at: 0.0,
                    x: 0.0,
                    y: 0.0,
                    zoom: 1.0,
                },
                CameraKey {
                    at: 4.0,
                    x: 100.0,
                    y: 50.0,
                    zoom: 2.0,
                },
            ],
            background_color: "#12abEF".into(),
            fade_in: 1.0,
            fade_out: 1.0,
            ..Composition::default()
        };
        composition.layers.insert(
            "EMMA".into(),
            Layer {
                z: -1,
                visible: false,
            },
        );
        let updated = apply_composition(SOURCE, 0, composition.clone()).unwrap();
        assert_eq!(parse(&updated).unwrap().scenes[0].composition, composition);
        assert!(parse(&updated).unwrap().scenes[0].events.is_empty());
        for at in [0.0, -1.0, 5.0, f64::NAN] {
            let mut invalid = composition.clone();
            invalid.camera[1].at = at;
            assert!(apply_composition(SOURCE, 0, invalid).is_err());
        }
        for zoom in [0.0, 21.0, f64::INFINITY] {
            let mut invalid = composition.clone();
            invalid.camera[1].zoom = zoom;
            assert!(apply_composition(SOURCE, 0, invalid).is_err());
        }
        let mut invalid = composition.clone();
        invalid.layers.insert("MISSING".into(), Layer::default());
        assert!(apply_composition(SOURCE, 0, invalid).is_err());
        let mut invalid = composition.clone();
        invalid.background_color = "#ffffff\"/>".into();
        assert!(apply_composition(SOURCE, 0, invalid).is_err());
        let mut invalid = composition.clone();
        invalid.fade_out = 4.0;
        assert!(apply_composition(SOURCE, 0, invalid).is_err());
        assert!(apply_composition(SOURCE, 1, composition).is_err());
        assert_eq!(
            parse(SOURCE).unwrap().scenes[0].composition,
            Composition::default()
        );
    }
}

#[cfg(test)]
mod actor_track_tests {
    use super::*;
    const SOURCE: &str =
        "characters: {A: {asset: a}, B: {asset: b}}\nscenes: [{id: shot, duration: 4, events: []}]";
    #[test]
    fn actor_track_transaction_roundtrip_removal_and_invalid_rollback() {
        let keys = vec![
            ActorKey::default(),
            ActorKey {
                at: 4.0,
                x: 100.0,
                easing: ActorEasing::Smoothstep,
                ..Default::default()
            },
        ];
        let updated = apply_actor_track(SOURCE, 0, "A", &keys).unwrap();
        let script = parse(&updated).unwrap();
        assert_eq!(script.scenes[0].composition.actor_tracks["A"], keys);
        let removed = apply_actor_track(&updated, 0, "A", &[]).unwrap();
        assert!(parse(&removed).unwrap().scenes[0]
            .composition
            .actor_tracks
            .is_empty());
        for key in [
            ActorKey {
                at: 4.1,
                ..Default::default()
            },
            ActorKey {
                scale_x: 0.0,
                ..Default::default()
            },
            ActorKey {
                rotation: f64::NAN,
                ..Default::default()
            },
            ActorKey {
                y: f64::INFINITY,
                ..Default::default()
            },
        ] {
            assert!(apply_actor_track(&updated, 0, "A", &[key]).is_err());
            assert_eq!(
                parse(&updated).unwrap().scenes[0].composition.actor_tracks["A"],
                keys
            );
        }
        assert!(apply_actor_track(SOURCE, 0, "unknown", &keys).is_err());
        assert!(apply_actor_track(SOURCE, 1, "A", &keys).is_err());
        assert!(
            apply_actor_track(SOURCE, 0, "A", &[ActorKey::default(), ActorKey::default()]).is_err()
        );
        assert!(apply_scene_settings(&updated, 0, script.project, "shot", 3.0).is_err());
        let deleted = remove_character(&updated, "A").unwrap();
        assert!(parse(&deleted).unwrap().scenes[0]
            .composition
            .actor_tracks
            .is_empty());
    }
}

#[cfg(test)]
mod background_key_tests {
    use super::*;
    const SOURCE: &str = "characters: {A: {asset: a}}\nscenes: [{id: shot, duration: 4, events: [{type: emotion, at: 1, actor: A, value: happy}]}]";
    fn composition() -> Composition {
        Composition {
            background_keys: vec![
                BackgroundKey {
                    at: 0.0,
                    color: "#00aAFF".into(),
                },
                BackgroundKey {
                    at: 4.0,
                    color: "#FFFFFF".into(),
                },
            ],
            ..Composition::default()
        }
    }
    #[test]
    fn background_edit_roundtrips_and_invalid_edits_leave_source_intact() {
        let original = parse(SOURCE).unwrap();
        let updated = apply_composition(SOURCE, 0, composition()).unwrap();
        let parsed = parse(&updated).unwrap();
        assert_eq!(parsed.scenes[0].composition, composition());
        assert_eq!(
            serde_yaml::to_string(&parsed.scenes[0].events).unwrap(),
            serde_yaml::to_string(&original.scenes[0].events).unwrap()
        );
        for at in [-1.0, 0.0, 4.01, f64::INFINITY, f64::NAN] {
            let mut invalid = composition();
            invalid.background_keys[1].at = at;
            assert!(apply_composition(&updated, 0, invalid).is_err());
            assert_eq!(
                parse(&updated).unwrap().scenes[0].composition,
                parsed.scenes[0].composition
            );
        }
        for color in ["", "red", "#fff", "#GG0000", "#ffffff\"/>"] {
            let mut invalid = composition();
            invalid.background_keys[0].color = color.into();
            assert!(apply_composition(&updated, 0, invalid).is_err());
        }
        let mut invalid = composition();
        invalid.background_keys.reverse();
        assert!(apply_composition(&updated, 0, invalid).is_err());
        let mut removed = composition();
        removed.background_keys.clear();
        let cleared = apply_composition(&updated, 0, removed).unwrap();
        assert!(parse(&cleared).unwrap().scenes[0]
            .composition
            .background_keys
            .is_empty());
    }
    #[test]
    fn duration_and_background_retiming_share_one_atomic_transaction() {
        let initial = apply_composition(SOURCE, 0, composition()).unwrap();
        let project = parse(&initial).unwrap().project;
        let mut shorter = composition();
        shorter.background_keys[1].at = 2.0;
        // The old endpoint lies beyond the shorter duration until keys are retimed.
        assert!(apply_scene_settings(&initial, 0, project.clone(), "short", 2.0).is_err());
        let shortened = apply_scene_settings_with_composition(
            &initial,
            0,
            project.clone(),
            "short",
            2.0,
            shorter.clone(),
        )
        .unwrap();
        let scene = &parse(&shortened).unwrap().scenes[0];
        assert_eq!(scene.duration, 2.0);
        assert_eq!(scene.composition, shorter);
        let mut longer = shorter;
        longer.background_keys.push(BackgroundKey {
            at: 6.0,
            color: "#abcdef".into(),
        });
        assert!(apply_composition(&shortened, 0, longer.clone()).is_err());
        let extended = apply_scene_settings_with_composition(
            &shortened,
            0,
            project.clone(),
            "long",
            6.0,
            longer.clone(),
        )
        .unwrap();
        let parsed = parse(&extended).unwrap();
        assert_eq!(parsed.scenes[0].duration, 6.0);
        assert_eq!(parsed.scenes[0].composition, longer);
        assert!(apply_scene_settings_with_composition(
            &extended,
            0,
            project,
            "invalid",
            3.0,
            longer.clone()
        )
        .is_err());
        assert_eq!(parse(&extended).unwrap().scenes[0].composition, longer);
        assert_eq!(parse(&extended).unwrap().scenes[0].id, "long");
    }

    #[test]
    fn background_key_schema_is_strict_and_budget_is_bounded() {
        for key in [
            "{at: 1, color: '#112233', typo: true}",
            "{at: 1}",
            "{color: '#112233'}",
        ] {
            let source = format!("characters: {{}}\nscenes: [{{id: shot, duration: 4, events: [], composition: {{background_keys: [{key}]}}}}]");
            assert!(parse(&source).is_err());
        }
        let mut value = composition();
        value.background_keys = (0..4096)
            .map(|i| BackgroundKey {
                at: i as f64 / 1024.0,
                color: "#112233".into(),
            })
            .collect();
        assert!(apply_composition(SOURCE, 0, value.clone()).is_ok());
        value.background_keys.push(BackgroundKey {
            at: 4.0,
            color: "#112233".into(),
        });
        assert!(apply_composition(SOURCE, 0, value).is_err());
        assert!(parse(SOURCE).unwrap().scenes[0]
            .composition
            .background_keys
            .is_empty());
    }
}
