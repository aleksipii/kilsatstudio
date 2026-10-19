//! Strict MotionScript scene and dialogue schema.
pub use motion_animation::{ClipWeights, HUMANOID_BONES};
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
    #[serde(default)]
    pub transition: Transition,
    pub background_color: String,
    pub background_keys: Vec<BackgroundKey>,
    pub camera: Vec<CameraKey>,
    pub layers: BTreeMap<String, Layer>,
    pub actor_tracks: BTreeMap<String, Vec<ActorKey>>,
    pub clip_weights: BTreeMap<String, ClipWeights>,
    pub clip_weight_tracks: BTreeMap<String, Vec<ClipWeightKeyframe>>,
    pub clip_instances: Vec<ClipInstance>,
    pub bone_tracks: BTreeMap<String, BTreeMap<String, Vec<ActorKey>>>,
    pub fade_in: f64,
    pub fade_out: f64,
}
impl Default for Composition {
    fn default() -> Self {
        Self {
            transition: Transition::Cut,
            background_color: "#181f2c".into(),
            background_keys: vec![],
            camera: vec![],
            layers: BTreeMap::new(),
            actor_tracks: BTreeMap::new(),
            clip_weights: BTreeMap::new(),
            clip_weight_tracks: BTreeMap::new(),
            clip_instances: Vec::new(),
            bone_tracks: BTreeMap::new(),
            fade_in: 0.0,
            fade_out: 0.0,
        }
    }
}
/// How a live clip combines with the accumulated original-rig authored offset.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipBlendMode {
    #[default]
    Override,
    Additive,
}
impl ClipBlendMode {
    fn is_override(&self) -> bool {
        *self == Self::Override
    }
}
/// Ordered scene-local layer referencing a project animation clip.
/// Mixed modes retain saved row order; additive then override can differ from
/// override then additive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipInstance {
    pub id: String,
    pub clip: String,
    pub actor: String,
    pub at: f64,
    pub duration: f64,
    #[serde(default = "full_clip_instance_weight")]
    pub weight: f64,
    #[serde(default, skip_serializing_if = "ClipBlendMode::is_override")]
    pub blend_mode: ClipBlendMode,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    #[serde(default)]
    pub weight_keys: Vec<InstanceWeightKey>,
    /// Repeat at the library clip's original speed; otherwise stretch once.
    #[serde(default)]
    pub looping: bool,
    /// Empty selects all authored joints; names do not include descendants.
    #[serde(default)]
    pub joint_mask: Vec<String>,
}
/// Instance-local scalar multiplier; outgoing easing controls the next interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceWeightKey {
    pub time: f64,
    pub weight: f64,
    #[serde(default)]
    pub easing: ActorEasing,
}
impl ClipInstance {
    /// Sample a validated instance on the scene clock, with a half-open active
    /// interval. Keys use local seconds, endpoint clamping and outgoing easing.
    /// This allocation-free sampler assumes source validation has succeeded.
    pub fn sample_weight(&self, scene_time: f64) -> f64 {
        if !scene_time.is_finite() || scene_time < self.at || scene_time >= self.at + self.duration
        {
            return 0.0;
        }
        let local = scene_time - self.at;
        let upper = self.weight_keys.partition_point(|key| key.time <= local);
        let multiplier = if self.weight_keys.is_empty() {
            1.0
        } else if upper == 0 {
            self.weight_keys[0].weight
        } else {
            let left = &self.weight_keys[upper - 1];
            if let Some(right) = self.weight_keys.get(upper) {
                let t = (local - left.time) / (right.time - left.time);
                let t = match left.easing {
                    ActorEasing::Linear => t,
                    ActorEasing::Smoothstep => t * t * (3.0 - 2.0 * t),
                    ActorEasing::Hold => 0.0,
                };
                left.weight + (right.weight - left.weight) * t
            } else {
                left.weight
            }
        };
        let fade_in = if self.fade_in > 0.0 {
            (local / self.fade_in).min(1.0)
        } else {
            1.0
        };
        let fade_out = if self.fade_out > 0.0 {
            ((self.duration - local) / self.fade_out).min(1.0)
        } else {
            1.0
        };
        self.weight * multiplier * fade_in * fade_out
    }
}
fn full_clip_instance_weight() -> f64 {
    1.0
}
/// Scene-local animation of all three procedural layer strengths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipWeightKeyframe {
    pub time: f64,
    pub weights: ClipWeights,
    #[serde(default)]
    pub easing: ActorEasing,
}
impl Composition {
    /// Sample a validated track using the outgoing key's easing. An absent track
    /// uses the static actor override; before/after endpoints clamp to that key.
    pub fn sample_clip_weights(&self, actor: &str, time: f64) -> ClipWeights {
        let fallback = self.clip_weights.get(actor).copied().unwrap_or_default();
        let Some(keys) = self
            .clip_weight_tracks
            .get(actor)
            .filter(|keys| !keys.is_empty())
        else {
            return fallback;
        };
        if !time.is_finite() {
            return fallback;
        }
        let upper = keys.partition_point(|key| key.time <= time);
        if upper == 0 {
            return keys[0].weights;
        }
        let left = &keys[upper - 1];
        let Some(right) = keys.get(upper) else {
            return left.weights;
        };
        let t = (time - left.time) / (right.time - left.time);
        let t = match left.easing {
            ActorEasing::Linear => t,
            ActorEasing::Smoothstep => t * t * (3.0 - 2.0 * t),
            ActorEasing::Hold => 0.0,
        };
        let blend = |a: f64, b: f64| a + (b - a) * t;
        ClipWeights {
            posture: blend(left.weights.posture, right.weights.posture),
            walk: blend(left.weights.walk, right.weights.walk),
            gesture: blend(left.weights.gesture, right.weights.gesture),
        }
    }
}

/// Outgoing transition to the next scene on the 24 kHz sequence clock.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Transition {
    #[default]
    Cut,
    Crossfade {
        duration: f64,
    },
}
impl<'de> Deserialize<'de> for Transition {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Unlike tagged unit variants, an empty struct rejects extra fields.
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
        enum Repr {
            Cut {},
            Crossfade { duration: f64 },
        }
        Ok(match Repr::deserialize(deserializer)? {
            Repr::Cut {} => Self::Cut,
            Repr::Crossfade { duration } => Self::Crossfade { duration },
        })
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
    pub animation_clips: BTreeMap<String, AnimationClip>,
    #[serde(default)]
    pub project: Project,
    pub characters: BTreeMap<String, Character>,
    pub scenes: Vec<Scene>,
}
/// Project-local authored parent-local offsets for the original seven-bone rig.
/// Copying tracks is independent; live instances retain a reference to this definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationClip {
    pub duration: f64,
    pub bone_tracks: BTreeMap<String, Vec<ActorKey>>,
}
fn valid_clip_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
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

/// Atomically change an actor's procedural layer intensities. Defaults remove the override.
pub fn apply_clip_weights(
    source: &str,
    scene_index: usize,
    actor: &str,
    weights: ClipWeights,
) -> Result<String, Error> {
    edit_source(source, |candidate| {
        if !candidate.characters.contains_key(actor) || !weights.is_valid() {
            return Err(Error::Validation(
                "clip weights require a known actor and finite intensities 0..1".into(),
            ));
        }
        let scene = candidate
            .scenes
            .get_mut(scene_index)
            .ok_or_else(missing_event)?;
        if weights == ClipWeights::default() {
            scene.composition.clip_weights.remove(actor);
        } else {
            scene.composition.clip_weights.insert(actor.into(), weights);
        }
        Ok(())
    })
}

/// Replace an actor's procedural weight track atomically; empty removes it.
pub fn apply_clip_weight_track(
    source: &str,
    scene_index: usize,
    actor: &str,
    keys: Vec<ClipWeightKeyframe>,
) -> Result<String, Error> {
    edit_source(source, |candidate| {
        if !candidate.characters.contains_key(actor) {
            return Err(Error::Validation(format!(
                "unknown clip weight track actor {actor}"
            )));
        }
        let scene = candidate
            .scenes
            .get_mut(scene_index)
            .ok_or_else(missing_event)?;
        if keys.is_empty() {
            scene.composition.clip_weight_tracks.remove(actor);
        } else {
            scene
                .composition
                .clip_weight_tracks
                .insert(actor.into(), keys);
        }
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

/// Atomically replace one actor's parent-local bone track. Empty keys remove it.
pub fn apply_bone_track(
    source: &str,
    scene_index: usize,
    actor: &str,
    bone: &str,
    keys: &[ActorKey],
) -> Result<String, Error> {
    edit_source(source, |candidate| {
        if !candidate.characters.contains_key(actor) {
            return Err(Error::Validation(format!(
                "unknown bone-track actor {actor}"
            )));
        }
        if !motion_animation::HUMANOID_BONES.contains(&bone) {
            return Err(Error::Validation(format!("unknown humanoid bone {bone}")));
        }
        let scene = candidate
            .scenes
            .get_mut(scene_index)
            .ok_or_else(missing_event)?;
        if keys.is_empty() {
            if let Some(tracks) = scene.composition.bone_tracks.get_mut(actor) {
                tracks.remove(bone);
                if tracks.is_empty() {
                    scene.composition.bone_tracks.remove(actor);
                }
            }
        } else {
            scene
                .composition
                .bone_tracks
                .entry(actor.into())
                .or_default()
                .insert(bone.into(), keys.to_vec());
        }
        Ok(())
    })
}

/// Capture all authored bone tracks for one actor. Existing names require explicit replacement.
pub fn capture_animation_clip(
    source: &str,
    scene_index: usize,
    actor: &str,
    name: &str,
    replace: bool,
) -> Result<String, Error> {
    edit_source(source, |candidate| {
        if !valid_clip_name(name) {
            return Err(Error::Validation(
                "clip name requires 1..64 ASCII letters, digits, underscores or hyphens".into(),
            ));
        }
        if candidate.animation_clips.contains_key(name) && !replace {
            return Err(Error::Validation(format!(
                "clip {name} already exists; explicitly enable replacement"
            )));
        }
        if !candidate.characters.contains_key(actor) {
            return Err(Error::Validation(format!("unknown clip actor {actor}")));
        }
        let scene = candidate
            .scenes
            .get(scene_index)
            .ok_or_else(missing_event)?;
        let tracks = scene
            .composition
            .bone_tracks
            .get(actor)
            .filter(|tracks| !tracks.is_empty())
            .ok_or_else(|| {
                Error::Validation(format!(
                    "{actor}: author at least one bone track before capturing a clip"
                ))
            })?;
        candidate.animation_clips.insert(
            name.into(),
            AnimationClip {
                duration: scene.duration,
                bone_tracks: tracks.clone(),
            },
        );
        Ok(())
    })
}

/// Replace every target actor bone track with an independent copy, without retiming.
pub fn apply_animation_clip(
    source: &str,
    scene_index: usize,
    actor: &str,
    name: &str,
) -> Result<String, Error> {
    edit_source(source, |candidate| {
        if !candidate.characters.contains_key(actor) {
            return Err(Error::Validation(format!("unknown clip actor {actor}")));
        }
        let clip = candidate
            .animation_clips
            .get(name)
            .ok_or_else(|| Error::Validation(format!("unknown animation clip {name}")))?;
        let scene = candidate
            .scenes
            .get_mut(scene_index)
            .ok_or_else(missing_event)?;
        if clip.duration > scene.duration {
            return Err(Error::Validation(format!(
                "clip {name} duration {} exceeds target scene duration {}; clips are not retimed",
                clip.duration, scene.duration
            )));
        }
        scene
            .composition
            .bone_tracks
            .insert(actor.into(), clip.bone_tracks.clone());
        Ok(())
    })
}

/// Copy every clip bone track into an explicit scene-local interval, replacing target tracks.
/// Only key times change; the library entry and other scene stages remain independent.
pub fn apply_animation_clip_retimed(
    source: &str,
    scene_index: usize,
    actor: &str,
    name: &str,
    at: f64,
    duration: f64,
) -> Result<String, Error> {
    edit_source(source, |candidate| {
        let end = at + duration;
        if !at.is_finite() || at < 0.0 || !duration.is_finite() || duration <= 0.0 {
            return Err(Error::Validation(
                "retimed clip requires a finite nonnegative start and finite positive duration"
                    .into(),
            ));
        }
        if !end.is_finite() || end <= at {
            return Err(Error::Validation(
                "retimed clip interval overflows or collapses at this numeric precision".into(),
            ));
        }
        if !candidate.characters.contains_key(actor) {
            return Err(Error::Validation(format!("unknown clip actor {actor}")));
        }
        let clip = candidate
            .animation_clips
            .get(name)
            .ok_or_else(|| Error::Validation(format!("unknown animation clip {name}")))?;
        let scene = candidate
            .scenes
            .get_mut(scene_index)
            .ok_or_else(missing_event)?;
        if end > scene.duration {
            return Err(Error::Validation(format!(
                "retimed clip {name} ends at {end}, exceeding target scene duration {}",
                scene.duration
            )));
        }
        let mut tracks = clip.bone_tracks.clone();
        for (bone, keys) in &mut tracks {
            let mut previous = None;
            for key in keys {
                let mapped = at + (key.at / clip.duration) * duration;
                if !mapped.is_finite()
                    || mapped < at
                    || mapped > end
                    || previous.is_some_and(|time| mapped <= time)
                {
                    return Err(Error::Validation(format!(
                        "retimed clip {name} bone {bone}: key times overflow or collapse; choose a wider interval or smaller start"
                    )));
                }
                key.at = mapped;
                previous = Some(mapped);
            }
        }
        scene.composition.bone_tracks.insert(actor.into(), tracks);
        Ok(())
    })
}

/// Add or update an instance by scene-local ID, preserving its existing layer position.
pub fn upsert_clip_instance(
    source: &str,
    scene_index: usize,
    instance: ClipInstance,
) -> Result<String, Error> {
    edit_source(source, |candidate| {
        let scene = candidate
            .scenes
            .get_mut(scene_index)
            .ok_or_else(missing_event)?;
        let instances = &mut scene.composition.clip_instances;
        if let Some(existing) = instances.iter_mut().find(|item| item.id == instance.id) {
            *existing = instance;
        } else {
            instances.push(instance);
        }
        Ok(())
    })
}

/// Remove one live layer without altering its definition or independent copied tracks.
pub fn remove_clip_instance(source: &str, scene_index: usize, id: &str) -> Result<String, Error> {
    edit_source(source, |candidate| {
        let scene = candidate
            .scenes
            .get_mut(scene_index)
            .ok_or_else(missing_event)?;
        let instances = &mut scene.composition.clip_instances;
        let index = instances
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| Error::Validation(format!("unknown clip instance {id}")))?;
        instances.remove(index);
        Ok(())
    })
}

/// Move a live layer to an absolute index in the complete, unfiltered scene list.
pub fn reorder_clip_instance(
    source: &str,
    scene_index: usize,
    id: &str,
    new_index: usize,
) -> Result<String, Error> {
    edit_source(source, |candidate| {
        let scene = candidate
            .scenes
            .get_mut(scene_index)
            .ok_or_else(missing_event)?;
        let instances = &mut scene.composition.clip_instances;
        if new_index >= instances.len() {
            return Err(Error::Validation(
                "clip instance layer index is out of range".into(),
            ));
        }
        let index = instances
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| Error::Validation(format!("unknown clip instance {id}")))?;
        let instance = instances.remove(index);
        instances.insert(new_index, instance);
        Ok(())
    })
}

/// Remove an unreferenced library entry; independent copied tracks remain intact.
pub fn remove_animation_clip(source: &str, name: &str) -> Result<String, Error> {
    edit_source(source, |candidate| {
        let references: Vec<_> = candidate
            .scenes
            .iter()
            .flat_map(|scene| {
                scene
                    .composition
                    .clip_instances
                    .iter()
                    .filter(|instance| instance.clip == name)
                    .map(move |instance| (scene.id.as_str(), instance.id.as_str()))
            })
            .collect();
        if let Some((scene, instance)) = references.first() {
            return Err(Error::Validation(format!(
                "clip {name} is referenced by {} instance(s); first scene {scene}, instance {instance}; remove instances before deleting the definition",
                references.len()
            )));
        }
        candidate
            .animation_clips
            .remove(name)
            .ok_or_else(|| Error::Validation(format!("unknown animation clip {name}")))?;
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
            scene.composition.bone_tracks.remove(id);
            scene.composition.clip_weights.remove(id);
            scene.composition.clip_weight_tracks.remove(id);
            scene
                .composition
                .clip_instances
                .retain(|instance| instance.actor != id);
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
    to_source(&candidate)
}

/// Validate and encode an edited document as bounded, readable MotionScript YAML.
pub fn to_source(script: &Script) -> Result<String, Error> {
    script.validate()?;
    let output = serde_yaml::to_string(script)?;
    if output.len() > 1_048_576 {
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
        if self.animation_clips.len() > 64 {
            return Err(fail("animation library exceeds 64 clips".into()));
        }
        let mut library_keys = 0usize;
        for (name, clip) in &self.animation_clips {
            if !valid_clip_name(name)
                || !clip.duration.is_finite()
                || clip.duration <= 0.0
                || clip.duration > 600.0
                || clip.bone_tracks.is_empty()
            {
                return Err(fail(format!("clip {name}: require a 1..64 ASCII identifier, duration above zero and at most 600 seconds, and at least one bone track")));
            }
            for (bone, keys) in &clip.bone_tracks {
                library_keys = library_keys.saturating_add(keys.len());
                if library_keys > 65536
                    || !HUMANOID_BONES.contains(&bone.as_str())
                    || keys.iter().any(|key| {
                        key.at > clip.duration
                            || !motion_animation::valid_humanoid_offset(key.animation_key().value)
                    })
                    || motion_animation::Track::new(
                        keys.iter().map(ActorKey::animation_key).collect(),
                    )
                    .is_err()
                {
                    return Err(fail(format!("clip {name}: require original rig bones, 1..4096 increasing finite keys within the clip, bounded offsets/rotation within ±1000, scale 0.1..4, and at most 65536 library keys")));
                }
            }
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
        let mut project_instances = 0usize;
        let mut project_instance_keys = 0usize;
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
            project_instances = project_instances.saturating_add(composition.clip_instances.len());
            if composition.clip_instances.len() > 128 || project_instances > 1024 {
                return Err(fail(format!(
                    "{}: clip instances exceed 128 per scene or 1024 per project",
                    s.id
                )));
            }
            let mut scene_instance_keys = 0usize;
            let mut instance_ids = std::collections::BTreeSet::new();
            let mut actor_instances = BTreeMap::new();
            for instance in &composition.clip_instances {
                if !valid_clip_name(&instance.id) || !instance_ids.insert(&instance.id) {
                    return Err(fail(format!(
                        "{}: invalid or duplicate clip instance ID {}",
                        s.id, instance.id
                    )));
                }
                if !self.animation_clips.contains_key(&instance.clip)
                    || !self.characters.contains_key(&instance.actor)
                {
                    return Err(fail(format!(
                        "{}: instance {} references unknown clip {} or actor {}",
                        s.id, instance.id, instance.clip, instance.actor
                    )));
                }
                let mut selected = std::collections::BTreeSet::new();
                if instance.joint_mask.len() > HUMANOID_BONES.len()
                    || instance.joint_mask.iter().any(|joint| {
                        !HUMANOID_BONES.contains(&joint.as_str()) || !selected.insert(joint)
                    })
                {
                    return Err(fail(format!(
                        "{}: instance {} joint mask requires at most seven unique humanoid joints",
                        s.id, instance.id
                    )));
                }
                let count = actor_instances.entry(&instance.actor).or_insert(0usize);
                *count += 1;
                if *count > 32 {
                    return Err(fail(format!(
                        "{}: actor {} exceeds 32 clip instances",
                        s.id, instance.actor
                    )));
                }
                let end = instance.at + instance.duration;
                if !instance.at.is_finite()
                    || instance.at < 0.0
                    || !instance.duration.is_finite()
                    || instance.duration <= 0.0
                    || !end.is_finite()
                    || end <= instance.at
                    || end > s.duration
                    || !instance.weight.is_finite()
                    || !(0.0..=1.0).contains(&instance.weight)
                {
                    return Err(fail(format!("{}: instance {} requires finite start >= 0, duration > 0, distinct end within scene, and weight 0..1", s.id, instance.id)));
                }
                if !instance.fade_in.is_finite()
                    || !instance.fade_out.is_finite()
                    || instance.fade_in < 0.0
                    || instance.fade_out < 0.0
                    || instance.fade_in > instance.duration
                    || instance.fade_out > instance.duration
                    || instance.fade_in + instance.fade_out > instance.duration
                {
                    return Err(fail(format!("{}: instance {} requires finite nonnegative fades with sum within duration", s.id, instance.id)));
                }
                scene_instance_keys =
                    scene_instance_keys.saturating_add(instance.weight_keys.len());
                project_instance_keys =
                    project_instance_keys.saturating_add(instance.weight_keys.len());
                if instance.weight_keys.len() > 256
                    || scene_instance_keys > 8192
                    || project_instance_keys > 65536
                {
                    return Err(fail(format!("{}: instance weight keys exceed 256 per instance, 8192 per scene or 65536 per project", s.id)));
                }
                let mut previous = None;
                for key in &instance.weight_keys {
                    if !key.time.is_finite()
                        || key.time < 0.0
                        || key.time > instance.duration
                        || previous.is_some_and(|time| key.time <= time)
                        || !key.weight.is_finite()
                        || !(0.0..=1.0).contains(&key.weight)
                    {
                        return Err(fail(format!("{}: instance {} requires strictly increasing local key times within duration and finite weights 0..1", s.id, instance.id)));
                    }
                    previous = Some(key.time);
                }
            }
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
            for (actor, weights) in &composition.clip_weights {
                if !self.characters.contains_key(actor) || !weights.is_valid() {
                    return Err(fail(format!("{}: clip weights require a known actor and finite intensities 0..1 ({actor})", s.id)));
                }
            }
            for (actor, keys) in &composition.clip_weight_tracks {
                if !self.characters.contains_key(actor) || keys.is_empty() || keys.len() > 256 {
                    return Err(fail(format!(
                        "{}: clip weight tracks require a known actor and 1..256 keys",
                        s.id
                    )));
                }
                let mut previous = None;
                for key in keys {
                    if !key.time.is_finite()
                        || key.time < 0.0
                        || key.time > s.duration
                        || previous.is_some_and(|time| key.time <= time)
                        || !key.weights.is_valid()
                    {
                        return Err(fail(format!("{}: clip weight tracks require increasing finite scene times and strengths 0..1", s.id)));
                    }
                    previous = Some(key.time);
                }
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
            let mut bone_key_count = 0usize;
            for (actor, tracks) in &composition.bone_tracks {
                if !self.characters.contains_key(actor) || tracks.is_empty() {
                    return Err(fail(format!(
                        "{}: bone tracks need a known actor and at least one bone",
                        s.id
                    )));
                }
                for (bone, keys) in tracks {
                    bone_key_count = bone_key_count.saturating_add(keys.len());
                    if bone_key_count > 65536
                        || !motion_animation::HUMANOID_BONES.contains(&bone.as_str())
                        || keys.iter().any(|key| {
                            key.at > s.duration
                                || !motion_animation::valid_humanoid_offset(
                                    key.animation_key().value,
                                )
                        })
                        || motion_animation::Track::new(
                            keys.iter().map(ActorKey::animation_key).collect(),
                        )
                        .is_err()
                    {
                        return Err(fail(format!("{}: bone tracks require original rig names, 1..4096 increasing scene keys, at most 65536 keys per scene, offsets/rotation within ±1000 and scale 0.1..4", s.id)));
                    }
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
        // Cuts retain legacy duration validation; crossfades require a
        // representable integer clock on both neighboring scenes.
        let mut incoming = 0_u64;
        for (index, scene) in self.scenes.iter().enumerate() {
            let outgoing = match scene.composition.transition {
                Transition::Cut => 0,
                Transition::Crossfade { duration } => {
                    let next = self
                        .scenes
                        .get(index + 1)
                        .ok_or_else(|| fail("final scene cannot crossfade".into()))?;
                    let overlap = transition_samples(duration).ok_or_else(|| {
                        fail("crossfade duration requires at least one finite 24 kHz sample".into())
                    })?;
                    let current = transition_samples(scene.duration).ok_or_else(|| {
                        fail("crossfade scene duration exceeds sequence clock".into())
                    })?;
                    let following = transition_samples(next.duration).ok_or_else(|| {
                        fail("crossfade scene duration exceeds sequence clock".into())
                    })?;
                    if overlap >= current || overlap >= following {
                        return Err(fail(
                            "crossfade must be shorter than both neighboring scenes".into(),
                        ));
                    }
                    overlap
                }
            };
            if incoming > 0 && outgoing > 0 {
                let length = transition_samples(scene.duration)
                    .ok_or_else(|| fail("invalid sequence duration".into()))?;
                if incoming
                    .checked_add(outgoing)
                    .is_none_or(|sum| sum > length)
                {
                    return Err(fail(
                        "incoming and outgoing crossfades cannot overlap".into(),
                    ));
                }
            }
            incoming = outgoing;
        }
        Ok(())
    }
}
fn transition_samples(seconds: f64) -> Option<u64> {
    let samples = (seconds * 24_000.0).round();
    (seconds.is_finite() && seconds > 0.0 && samples >= 1.0 && samples < u64::MAX as f64)
        .then_some(samples as u64)
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

#[cfg(test)]
mod transition_tests {
    use super::*;
    const SOURCE: &str = "characters: {}\nscenes: [{id: a, duration: 2, events: []}, {id: b, duration: 2, events: []}, {id: c, duration: 2, events: []}]";
    #[test]
    fn crossfade_validation_rejects_invalid_neighbors_and_triple_overlap() {
        for duration in [0.0, -1.0, f64::NAN, f64::INFINITY, 0.000001, 2.0, 3.0] {
            let mut value = parse(SOURCE).unwrap();
            value.scenes[0].composition.transition = Transition::Crossfade { duration };
            assert!(value.validate().is_err(), "duration {duration}");
        }
        let mut value = parse(SOURCE).unwrap();
        value.scenes[2].composition.transition = Transition::Crossfade { duration: 0.5 };
        assert!(value.validate().is_err());
        value.scenes[2].composition.transition = Transition::Cut;
        value.scenes[0].composition.transition = Transition::Crossfade { duration: 1.25 };
        value.scenes[1].composition.transition = Transition::Crossfade { duration: 1.0 };
        assert!(value.validate().is_err());
        value.scenes[0].composition.transition = Transition::Crossfade { duration: 1.0 };
        assert!(value.validate().is_ok());
    }
    #[test]
    fn composition_edit_validates_transition_atomically_and_strictly() {
        let mut composition = Composition {
            transition: Transition::Crossfade { duration: 0.5 },
            ..Default::default()
        };
        let updated = apply_composition(SOURCE, 0, composition.clone()).unwrap();
        assert_eq!(
            parse(&updated).unwrap().scenes[0].composition.transition,
            composition.transition
        );
        composition.transition = Transition::Crossfade { duration: 2.0 };
        assert!(apply_composition(&updated, 0, composition).is_err());
        assert_eq!(
            parse(&updated).unwrap().scenes[0].composition.transition,
            Transition::Crossfade { duration: 0.5 }
        );
        for invalid in [
            "type: cut\nduration: 1",
            "type: cut\nduration: null",
            "type: cut\nunexpected: true",
            "type: crossfade\nduration: 1\nunexpected: true",
            "type: dissolve",
        ] {
            assert!(
                serde_yaml::from_str::<Transition>(invalid).is_err(),
                "{invalid}"
            );
        }
        assert_eq!(
            serde_yaml::from_str::<Transition>("type: cut").unwrap(),
            Transition::Cut
        );
    }
}

#[cfg(test)]
mod bone_track_tests {
    use super::*;
    const SOURCE: &str =
        "characters: {A: {asset: a}, B: {asset: b}}\nscenes: [{id: shot, duration: 4, events: []}]";
    #[test]
    fn aggregate_scene_key_budget_and_strict_bone_schema_are_enforced() {
        let mut script = parse(SOURCE).unwrap();
        let character = script.characters["A"].clone();
        script.characters.insert("C".into(), character);
        let keys: Vec<_> = (0..4096)
            .map(|i| ActorKey {
                at: i as f64 / 1024.0,
                ..Default::default()
            })
            .collect();
        for actor in ["A", "B", "C"] {
            for bone in HUMANOID_BONES {
                script.scenes[0]
                    .composition
                    .bone_tracks
                    .entry(actor.into())
                    .or_default()
                    .insert(bone.into(), keys.clone());
            }
        }
        assert!(script.validate().is_err());
        script.scenes[0].composition.bone_tracks.remove("C");
        assert!(script.validate().is_ok());
        let invalid = "characters: {A: {asset: a}}\nscenes: [{id: shot, duration: 4, events: [], composition: {bone_tracks: {A: {head: [{at: 0, unknown: 1}]}}}}]";
        assert!(parse(invalid).is_err());
    }
    #[test]
    fn bone_tracks_roundtrip_atomic_rejection_removal_and_character_cleanup() {
        let keys = vec![
            ActorKey::default(),
            ActorKey {
                at: 4.0,
                rotation: 0.5,
                easing: ActorEasing::Hold,
                ..Default::default()
            },
        ];
        let source = apply_bone_track(SOURCE, 0, "A", "chest", &keys).unwrap();
        assert_eq!(
            parse(&source).unwrap().scenes[0].composition.bone_tracks["A"]["chest"],
            keys
        );
        for (actor, bone) in [("missing", "head"), ("A", "unknown")] {
            assert!(apply_bone_track(&source, 0, actor, bone, &keys).is_err());
        }
        for key in [
            ActorKey {
                at: 4.1,
                ..Default::default()
            },
            ActorKey {
                x: 1000.1,
                ..Default::default()
            },
            ActorKey {
                rotation: f64::NAN,
                ..Default::default()
            },
            ActorKey {
                scale_x: 4.01,
                ..Default::default()
            },
            ActorKey {
                scale_y: 0.09,
                ..Default::default()
            },
        ] {
            assert!(apply_bone_track(&source, 0, "A", "head", &[key]).is_err());
        }
        assert!(apply_bone_track(
            &source,
            0,
            "A",
            "head",
            &[ActorKey::default(), ActorKey::default()]
        )
        .is_err());
        assert!(apply_bone_track(&source, 1, "A", "head", &keys).is_err());
        assert_eq!(
            parse(&source).unwrap().scenes[0].composition.bone_tracks["A"]["chest"],
            keys
        );
        let removed = apply_bone_track(&source, 0, "A", "chest", &[]).unwrap();
        assert!(parse(&removed).unwrap().scenes[0]
            .composition
            .bone_tracks
            .is_empty());
        let deleted = remove_character(&source, "A").unwrap();
        assert!(parse(&deleted).unwrap().scenes[0]
            .composition
            .bone_tracks
            .is_empty());
        assert!(
            apply_scene_settings(&source, 0, parse(&source).unwrap().project, "shot", 3.0).is_err()
        );
    }
}

#[cfg(test)]
mod clip_weight_tests {
    use super::*;
    const SOURCE: &str =
        "characters: {A: {asset: a}, B: {asset: b}}\nscenes: [{id: shot, duration: 4, events: []}]";

    #[test]
    fn missing_defaults_roundtrip_and_atomic_weight_edits() {
        assert!(parse(SOURCE).unwrap().scenes[0]
            .composition
            .clip_weights
            .is_empty());
        let weights = ClipWeights {
            posture: 0.2,
            walk: 0.5,
            gesture: 0.0,
        };
        let source = apply_clip_weights(SOURCE, 0, "A", weights).unwrap();
        assert_eq!(
            parse(&source).unwrap().scenes[0].composition.clip_weights["A"],
            weights
        );
        assert!(apply_clip_weights(&source, 0, "unknown", weights).is_err());
        assert!(apply_clip_weights(&source, 1, "A", weights).is_err());
        for bad in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            assert!(apply_clip_weights(
                &source,
                0,
                "A",
                ClipWeights {
                    walk: bad,
                    ..weights
                }
            )
            .is_err());
        }
        assert_eq!(
            parse(&source).unwrap().scenes[0].composition.clip_weights["A"],
            weights
        );
        let reset = apply_clip_weights(&source, 0, "A", ClipWeights::default()).unwrap();
        assert!(parse(&reset).unwrap().scenes[0]
            .composition
            .clip_weights
            .is_empty());
        let removed = remove_character(&source, "A").unwrap();
        assert!(parse(&removed).unwrap().scenes[0]
            .composition
            .clip_weights
            .is_empty());
    }

    #[test]
    fn strict_weight_schema_and_actor_validation() {
        let source = |map: &str| {
            format!("characters: {{A: {{asset: a}}}}\nscenes: [{{id: shot, duration: 4, events: [], composition: {{clip_weights: {map}}}}}]")
        };
        let parsed = parse(&source("{A: {walk: 0.25}}")).unwrap();
        assert_eq!(
            parsed.scenes[0].composition.clip_weights["A"],
            ClipWeights {
                walk: 0.25,
                ..Default::default()
            }
        );
        for map in [
            "{unknown: {}}",
            "{A: {walk: true}}",
            "{A: {walk: .nan}}",
            "{A: {walk: .inf}}",
            "{A: {walk: -1}}",
            "{A: {posture: 2}}",
            "{A: {unexpected: 0}}",
        ] {
            assert!(parse(&source(map)).is_err(), "{map}");
        }
    }
}

#[cfg(test)]
mod clip_weight_track_tests {
    use super::*;
    const SOURCE: &str =
        "characters: {A: {asset: a}, B: {asset: b}}\nscenes: [{id: shot, duration: 4, events: []}]";
    fn key(
        time: f64,
        posture: f64,
        walk: f64,
        gesture: f64,
        easing: ActorEasing,
    ) -> ClipWeightKeyframe {
        ClipWeightKeyframe {
            time,
            weights: ClipWeights {
                posture,
                walk,
                gesture,
            },
            easing,
        }
    }
    #[test]
    fn track_sampling_clamps_and_uses_outgoing_easing_independently() {
        let mut composition = Composition::default();
        let fallback = ClipWeights {
            posture: 0.1,
            walk: 0.2,
            gesture: 0.3,
        };
        composition.clip_weights.insert("A".into(), fallback);
        assert_eq!(composition.sample_clip_weights("A", 1.0), fallback);
        let keys = vec![
            key(1.0, 0.0, 1.0, 0.2, ActorEasing::Linear),
            key(3.0, 1.0, 0.0, 0.6, ActorEasing::Hold),
        ];
        composition
            .clip_weight_tracks
            .insert("A".into(), keys.clone());
        assert_eq!(composition.sample_clip_weights("A", 0.0), keys[0].weights);
        assert_eq!(composition.sample_clip_weights("A", 4.0), keys[1].weights);
        let middle = composition.sample_clip_weights("A", 2.0);
        assert_eq!((middle.posture, middle.walk), (0.5, 0.5));
        assert!((middle.gesture - 0.4).abs() < 1e-12);
        composition.clip_weight_tracks.get_mut("A").unwrap()[0].easing = ActorEasing::Smoothstep;
        assert_eq!(composition.sample_clip_weights("A", 1.5).posture, 0.15625);
        composition.clip_weight_tracks.get_mut("A").unwrap()[0].easing = ActorEasing::Hold;
        assert_eq!(composition.sample_clip_weights("A", 2.999).posture, 0.0);
        assert_eq!(composition.sample_clip_weights("A", 3.0), keys[1].weights);
        assert_eq!(
            composition.sample_clip_weights("B", 2.0),
            ClipWeights::default()
        );
        composition
            .clip_weight_tracks
            .insert("A".into(), vec![keys[0].clone()]);
        assert_eq!(composition.sample_clip_weights("A", 4.0), keys[0].weights);
    }
    #[test]
    fn track_edits_roundtrip_reject_invalid_atomically_and_clean_up() {
        let keys = vec![
            key(0.0, 0.0, 0.3, 1.0, ActorEasing::Linear),
            key(4.0, 1.0, 0.8, 0.0, ActorEasing::Smoothstep),
        ];
        let source = apply_clip_weight_track(SOURCE, 0, "A", keys.clone()).unwrap();
        assert_eq!(
            parse(&source).unwrap().scenes[0]
                .composition
                .clip_weight_tracks["A"],
            keys
        );
        assert!(apply_clip_weight_track(&source, 1, "A", keys.clone()).is_err());
        assert!(apply_clip_weight_track(&source, 0, "unknown", keys.clone()).is_err());
        for invalid in [f64::NAN, f64::INFINITY, -0.1, 4.1] {
            let mut bad = keys.clone();
            bad[0].time = invalid;
            assert!(apply_clip_weight_track(&source, 0, "A", bad).is_err());
        }
        for invalid in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            let mut bad = keys.clone();
            bad[0].weights.walk = invalid;
            assert!(apply_clip_weight_track(&source, 0, "A", bad).is_err());
        }
        for bad in [
            vec![keys[0].clone(), keys[0].clone()],
            vec![keys[1].clone(), keys[0].clone()],
            (0..257)
                .map(|i| key(i as f64 / 100.0, 1.0, 1.0, 1.0, ActorEasing::Linear))
                .collect(),
        ] {
            assert!(apply_clip_weight_track(&source, 0, "A", bad).is_err());
        }
        let boundary: Vec<_> = (0..256)
            .map(|i| key(i as f64 / 100.0, 1.0, 1.0, 1.0, ActorEasing::Linear))
            .collect();
        assert!(apply_clip_weight_track(&source, 0, "A", boundary).is_ok());
        assert_eq!(
            parse(&source).unwrap().scenes[0]
                .composition
                .clip_weight_tracks["A"],
            keys
        );
        assert!(
            parse(&apply_clip_weight_track(&source, 0, "A", vec![]).unwrap())
                .unwrap()
                .scenes[0]
                .composition
                .clip_weight_tracks
                .is_empty()
        );
        assert!(parse(&remove_character(&source, "A").unwrap())
            .unwrap()
            .scenes[0]
            .composition
            .clip_weight_tracks
            .is_empty());
    }
    #[test]
    fn strict_track_yaml_rejects_unknown_fields_and_empty_serialized_tracks() {
        let source = |track: &str| {
            format!("characters: {{A: {{asset: a}}}}\nscenes: [{{id: shot, duration: 4, events: [], composition: {{clip_weight_tracks: {{A: {track}}}}}}}]")
        };
        assert!(parse(&source("[{time: 0, weights: {walk: 0.5}}]")).is_ok());
        for track in [
            "[]",
            "[{time: 0, weights: {}, extra: 1}]",
            "[{time: 0, weights: {extra: 1}}]",
            "[{time: 0, weights: {}, easing: cubic}]",
            "[{time: true, weights: {}}]",
            "[{time: 0}]",
        ] {
            assert!(parse(&source(track)).is_err(), "{track}");
        }
    }
}

#[cfg(test)]
mod animation_library_tests {
    use super::*;
    fn source() -> String {
        "characters: {A: {asset: a}, B: {asset: b}}\nscenes: [{id: source, duration: 4, events: [], composition: {bone_tracks: {A: {head: [{at: 0, x: 2}, {at: 4, x: 4}]}, B: {chest: [{at: 0, y: 3}]}}}}, {id: target, duration: 6, events: [], composition: {bone_tracks: {B: {chest: [{at: 0, y: 9}]}}}}, {id: short, duration: 2, events: []}]".into()
    }
    #[test]
    fn retimed_clip_maps_all_bones_and_preserves_values_and_independent_state() {
        let mut script = parse(&source()).unwrap();
        let keys = [0.0, 1.0, 4.0].map(|at| ActorKey {
            at,
            x: 2.0,
            y: -3.0,
            rotation: 0.7,
            scale_x: 1.2,
            scale_y: 0.8,
            easing: ActorEasing::Smoothstep,
        });
        let tracks: BTreeMap<_, _> = HUMANOID_BONES
            .into_iter()
            .map(|bone| (bone.into(), keys.to_vec()))
            .collect();
        script.scenes[1]
            .composition
            .actor_tracks
            .insert("B".into(), keys.to_vec());
        script.scenes[1].composition.clip_weights.insert(
            "B".into(),
            ClipWeights {
                walk: 0.3,
                ..Default::default()
            },
        );
        script.animation_clips.insert(
            "clip".into(),
            AnimationClip {
                duration: 4.0,
                bone_tracks: tracks.clone(),
            },
        );
        let source = serde_yaml::to_string(&script).unwrap();
        let before = parse(&source).unwrap();
        let result = apply_animation_clip_retimed(&source, 1, "B", "clip", 2.0, 2.0).unwrap();
        let after = parse(&result).unwrap();
        for (bone, original) in &tracks {
            let mapped = &after.scenes[1].composition.bone_tracks["B"][bone];
            for ((key, original), time) in mapped.iter().zip(original).zip([2.0, 2.5, 4.0]) {
                let mut expected = original.clone();
                expected.at = time;
                assert_eq!(key, &expected);
            }
        }
        let mut restored = after.clone();
        restored.scenes[1].composition.bone_tracks =
            before.scenes[1].composition.bone_tracks.clone();
        assert_eq!(serde_yaml::to_string(&restored).unwrap(), source);
        let removed = parse(&remove_animation_clip(&result, "clip").unwrap()).unwrap();
        assert_eq!(
            removed.scenes[1].composition.bone_tracks,
            after.scenes[1].composition.bone_tracks
        );
    }
    #[test]
    fn retimed_clip_compresses_long_clips_without_changing_original_apply() {
        let source = capture_animation_clip(&source(), 0, "A", "clip", false).unwrap();
        assert!(apply_animation_clip(&source, 2, "B", "clip").is_err());
        let compressed = apply_animation_clip_retimed(&source, 2, "B", "clip", 0.5, 1.5).unwrap();
        let compressed = parse(&compressed).unwrap();
        let keys = &compressed.scenes[2].composition.bone_tracks["B"]["head"];
        assert_eq!(keys[0].at, 0.5);
        assert_eq!(keys[1].at, 2.0);
        let original = parse(&apply_animation_clip(&source, 1, "B", "clip").unwrap()).unwrap();
        assert_eq!(
            original.scenes[1].composition.bone_tracks["B"],
            original.animation_clips["clip"].bone_tracks
        );
    }
    #[test]
    fn retimed_clip_rejects_invalid_intervals_references_and_precision_collapse() {
        let source = capture_animation_clip(&source(), 0, "A", "clip", false).unwrap();
        for (at, duration) in [
            (-1.0, 1.0),
            (f64::NAN, 1.0),
            (f64::INFINITY, 1.0),
            (0.0, 0.0),
            (0.0, -1.0),
            (0.0, f64::NAN),
            (0.0, f64::INFINITY),
            (f64::MAX, f64::MAX),
            (5.0, 2.0),
            (1.0, f64::MIN_POSITIVE),
        ] {
            assert!(apply_animation_clip_retimed(&source, 1, "B", "clip", at, duration).is_err());
        }
        for (index, actor, name) in [
            (99, "B", "clip"),
            (1, "missing", "clip"),
            (1, "B", "missing"),
        ] {
            assert!(apply_animation_clip_retimed(&source, index, actor, name, 0.0, 1.0).is_err());
        }
        let mut script = parse(&source).unwrap();
        script
            .animation_clips
            .get_mut("clip")
            .unwrap()
            .bone_tracks
            .get_mut("head")
            .unwrap()
            .insert(
                1,
                ActorKey {
                    at: f64::MIN_POSITIVE,
                    ..Default::default()
                },
            );
        let source = serde_yaml::to_string(&script).unwrap();
        assert!(parse(&source).is_ok());
        let error = apply_animation_clip_retimed(&source, 1, "B", "clip", 1.0, 1.0)
            .unwrap_err()
            .to_string();
        assert!(error.contains("collapse"), "{error}");
        assert_eq!(
            parse(&source).unwrap().animation_clips["clip"].bone_tracks["head"][1].at,
            f64::MIN_POSITIVE
        );
    }
    #[test]
    fn capture_apply_replace_remove_are_independent_and_preserve_other_state() {
        let original = source();
        assert!(parse(&original).unwrap().animation_clips.is_empty());
        let captured = capture_animation_clip(&original, 0, "A", "Head-1", false).unwrap();
        let parsed = parse(&captured).unwrap();
        let clip = parsed.animation_clips["Head-1"].clone();
        assert_eq!(clip.duration, 4.0);
        assert_eq!(
            clip.bone_tracks,
            parsed.scenes[0].composition.bone_tracks["A"]
        );
        assert!(capture_animation_clip(&captured, 0, "B", "Head-1", false).is_err());
        let replaced = capture_animation_clip(&captured, 0, "B", "Head-1", true).unwrap();
        assert!(parse(&replaced).unwrap().animation_clips["Head-1"]
            .bone_tracks
            .contains_key("chest"));
        let same_scene = apply_animation_clip(&captured, 0, "B", "Head-1").unwrap();
        assert_eq!(
            parse(&same_scene).unwrap().scenes[0]
                .composition
                .bone_tracks["A"],
            clip.bone_tracks
        );
        let applied = apply_animation_clip(&captured, 1, "B", "Head-1").unwrap();
        let parsed = parse(&applied).unwrap();
        assert_eq!(
            parsed.scenes[1].composition.bone_tracks["B"],
            clip.bone_tracks
        );
        assert!(!parsed.scenes[1].composition.bone_tracks["B"].contains_key("chest"));
        assert_eq!(
            parsed.scenes[0].composition,
            parse(&captured).unwrap().scenes[0].composition
        );
        let deleted_actor = remove_character(&captured, "A").unwrap();
        assert!(apply_animation_clip(&deleted_actor, 1, "B", "Head-1").is_ok());
        let removed = remove_animation_clip(&applied, "Head-1").unwrap();
        let parsed = parse(&removed).unwrap();
        assert!(parsed.animation_clips.is_empty());
        assert_eq!(
            parsed.scenes[1].composition.bone_tracks["B"],
            clip.bone_tracks
        );
        assert!(remove_animation_clip(&removed, "Head-1").is_err());
        assert!(apply_animation_clip(&captured, 2, "B", "Head-1").is_err());
        assert!(apply_animation_clip(&captured, 1, "unknown", "Head-1").is_err());
        assert!(apply_animation_clip(&captured, 99, "B", "Head-1").is_err());
        assert!(apply_animation_clip(&captured, 1, "B", "missing").is_err());
        for name in ["", "bad name", "é", &"a".repeat(65)] {
            assert!(capture_animation_clip(&original, 0, "A", name, false).is_err());
        }
        assert!(capture_animation_clip(&original, 1, "A", "empty", false).is_err());
    }
    #[test]
    fn library_schema_and_numeric_boundaries_are_strict() {
        let base =
            parse(&capture_animation_clip(&source(), 0, "A", "clip", false).unwrap()).unwrap();
        let invalid = |edit: fn(&mut AnimationClip)| {
            let mut script = base.clone();
            edit(script.animation_clips.get_mut("clip").unwrap());
            assert!(script.validate().is_err());
        };
        for edit in [
            (|clip: &mut AnimationClip| clip.duration = f64::NAN) as fn(&mut AnimationClip),
            |clip| clip.duration = 0.0,
            |clip| clip.duration = 600.001,
            |clip| clip.bone_tracks.clear(),
            |clip| {
                clip.bone_tracks
                    .insert("tail".into(), vec![ActorKey::default()]);
            },
            |clip| {
                clip.bone_tracks.get_mut("head").unwrap().clear();
            },
            |clip| clip.bone_tracks.get_mut("head").unwrap()[0].at = -1.0,
            |clip| clip.bone_tracks.get_mut("head").unwrap()[0].x = f64::INFINITY,
            |clip| clip.bone_tracks.get_mut("head").unwrap()[0].scale_x = 0.0,
            |clip| clip.bone_tracks.get_mut("head").unwrap()[1].at = 0.0,
            |clip| clip.bone_tracks.get_mut("head").unwrap()[1].at = 4.001,
        ] {
            invalid(edit);
        }
        let mut yaml = serde_yaml::to_value(&base).unwrap();
        yaml["animation_clips"]["clip"]["unknown"] = serde_yaml::Value::Bool(true);
        assert!(parse(&serde_yaml::to_string(&yaml).unwrap()).is_err());
        let yaml = serde_yaml::to_string(&base).unwrap();
        assert!(parse(&yaml.replace("bone_tracks:", "unknown_tracks:")).is_err());
        let mut max = base.clone();
        let clip = max.animation_clips["clip"].clone();
        for i in 1..64 {
            max.animation_clips.insert(format!("clip{i}"), clip.clone());
        }
        assert!(max.validate().is_ok());
        max.animation_clips.insert("overflow".into(), clip);
        assert!(max.validate().is_err());
    }
    #[test]
    fn library_key_budget_is_global_and_per_track_is_bounded() {
        let mut script = parse(&source()).unwrap();
        let keys: Vec<_> = (0..4096)
            .map(|i| ActorKey {
                at: i as f64 / 10.0,
                ..ActorKey::default()
            })
            .collect();
        let clip = AnimationClip {
            duration: 600.0,
            bone_tracks: BTreeMap::from([("head".into(), keys.clone())]),
        };
        for i in 0..16 {
            script.animation_clips.insert(format!("c{i}"), clip.clone());
        }
        assert!(script.validate().is_ok());
        script.animation_clips.insert(
            "overflow".into(),
            AnimationClip {
                duration: 1.0,
                bone_tracks: BTreeMap::from([("head".into(), vec![ActorKey::default()])]),
            },
        );
        assert!(script.validate().is_err());
        script.animation_clips.clear();
        let mut too_many = keys;
        too_many.push(ActorKey {
            at: 410.0,
            ..ActorKey::default()
        });
        script.animation_clips.insert(
            "too-many".into(),
            AnimationClip {
                duration: 600.0,
                bone_tracks: BTreeMap::from([("head".into(), too_many)]),
            },
        );
        assert!(script.validate().is_err());
        let long_scene = source().replace("duration: 4", "duration: 601");
        assert!(capture_animation_clip(&long_scene, 0, "A", "too-long", false).is_err());
    }
}

#[cfg(test)]
mod live_clip_instance_tests {
    use super::*;

    fn source() -> String {
        let source = "characters: {A: {asset: a}, B: {asset: b}}\nscenes: [{id: source, duration: 4, events: [], composition: {bone_tracks: {A: {head: [{at: 0, x: 2}, {at: 4, x: 4}]}}}}, {id: target, duration: 6, events: []}]";
        capture_animation_clip(source, 0, "A", "head", false).unwrap()
    }

    fn instance(id: &str) -> ClipInstance {
        ClipInstance {
            id: id.into(),
            clip: "head".into(),
            actor: "B".into(),
            at: 1.0,
            duration: 2.0,
            weight: 1.0,
            blend_mode: ClipBlendMode::Override,
            fade_in: 0.0,
            fade_out: 0.0,
            weight_keys: Vec::new(),
            looping: false,
            joint_mask: Vec::new(),
        }
    }

    fn weight_key(time: f64, weight: f64, easing: ActorEasing) -> InstanceWeightKey {
        InstanceWeightKey {
            time,
            weight,
            easing,
        }
    }

    #[test]
    fn looping_masks_default_roundtrip_and_reject_invalid_selections() {
        let source = source();
        let mut layer = instance("masked");
        let original = upsert_clip_instance(&source, 1, layer.clone()).unwrap();
        let without_fields = original
            .lines()
            .filter(|line| {
                !line.trim_start().starts_with("looping:")
                    && !line.trim_start().starts_with("joint_mask:")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            parse(&without_fields).unwrap().scenes[1]
                .composition
                .clip_instances[0],
            layer
        );
        layer.looping = true;
        layer.joint_mask = vec!["head".into(), "right_hand".into()];
        let updated = upsert_clip_instance(&source, 1, layer.clone()).unwrap();
        assert_eq!(
            parse(&updated).unwrap().scenes[1]
                .composition
                .clip_instances[0],
            layer
        );
        for mask in [vec!["head", "head"], vec!["unknown"], vec!["head"; 8]] {
            layer.joint_mask = mask.into_iter().map(str::to_owned).collect();
            assert!(upsert_clip_instance(&updated, 1, layer.clone()).is_err());
        }
        assert!(parse(&updated.replace("looping: true", "looping: forever")).is_err());
    }

    #[test]
    fn instance_envelope_samples_products_endpoints_easing_and_seeks() {
        let mut item = instance("envelope");
        item.duration = 4.0;
        item.weight = 0.8;
        for time in [1.0, 1.25, 4.999] {
            assert_eq!(item.sample_weight(time), 0.8);
        }
        for time in [0.0, 5.0, f64::NAN, f64::INFINITY] {
            assert_eq!(item.sample_weight(time), 0.0);
        }
        item.fade_in = 1.0;
        item.fade_out = 1.0;
        item.weight_keys = vec![
            weight_key(1.0, 0.25, ActorEasing::Linear),
            weight_key(3.0, 0.75, ActorEasing::Hold),
        ];
        assert_eq!(item.sample_weight(1.0), 0.0);
        assert_eq!(item.sample_weight(1.5), 0.1);
        assert_eq!(item.sample_weight(3.0), 0.4);
        assert!((item.sample_weight(4.5) - 0.3).abs() < 1e-12);
        assert_eq!(item.sample_weight(2.0), 0.2); // backward seek is stateless
        item.fade_in = 0.0;
        item.fade_out = 0.0;
        item.weight_keys = vec![
            weight_key(0.0, 0.0, ActorEasing::Smoothstep),
            weight_key(4.0, 1.0, ActorEasing::Linear),
        ];
        assert_eq!(item.sample_weight(2.0), 0.125);
        item.weight_keys[0].easing = ActorEasing::Hold;
        assert_eq!(item.sample_weight(4.999), 0.0);
        item.weight_keys[1].time = 2.0;
        assert_eq!(item.sample_weight(3.0), 0.8);
        let mut adjacent = instance("next");
        adjacent.at = 5.0;
        assert_eq!(item.sample_weight(5.0), 0.0);
        assert_eq!(adjacent.sample_weight(5.0), 1.0);
    }

    #[test]
    fn instance_envelopes_validate_even_when_weight_zero_and_edits_are_atomic() {
        let source = source();
        let mut item = instance("envelope");
        item.weight = 0.0;
        for invalid in [f64::NAN, f64::INFINITY, -0.01, 2.01] {
            item.fade_in = invalid;
            assert!(upsert_clip_instance(&source, 1, item.clone()).is_err());
            item.fade_in = 0.0;
            item.fade_out = invalid;
            assert!(upsert_clip_instance(&source, 1, item.clone()).is_err());
            item.fade_out = 0.0;
        }
        item.fade_in = 1.0;
        item.fade_out = 1.01;
        assert!(upsert_clip_instance(&source, 1, item.clone()).is_err());
        item.fade_out = 1.0;
        item.weight_keys = vec![weight_key(2.0, 1.0, ActorEasing::Linear)];
        let valid = upsert_clip_instance(&source, 1, item.clone()).unwrap();
        item.duration = 1.0;
        assert!(upsert_clip_instance(&valid, 1, item.clone()).is_err());
        assert_eq!(
            parse(&valid).unwrap().scenes[1].composition.clip_instances[0].duration,
            2.0
        );
        item.fade_in = 0.0;
        item.fade_out = 0.0;
        item.duration = 2.0;
        for invalid in [f64::NAN, f64::INFINITY, -0.1, 2.01] {
            item.weight_keys = vec![weight_key(invalid, 0.5, ActorEasing::Linear)];
            assert!(upsert_clip_instance(&source, 1, item.clone()).is_err());
        }
        for invalid in [f64::NAN, f64::INFINITY, -0.1, 1.01] {
            item.weight_keys = vec![weight_key(0.0, invalid, ActorEasing::Linear)];
            assert!(upsert_clip_instance(&source, 1, item.clone()).is_err());
        }
        for times in [[1.0, 1.0], [1.0, 0.0]] {
            item.weight_keys = times
                .map(|t| weight_key(t, 1.0, ActorEasing::Linear))
                .to_vec();
            assert!(upsert_clip_instance(&source, 1, item.clone()).is_err());
        }
        for yaml in [
            "{time: 0, weight: 1, unknown: 0}",
            "{time: true, weight: 1}",
            "{time: 0, weight: true}",
            "{time: 0, weight: 1, easing: missing}",
        ] {
            assert!(serde_yaml::from_str::<InstanceWeightKey>(yaml).is_err());
        }
    }

    #[test]
    fn instance_envelope_key_budgets_cover_instance_scene_and_project() {
        let mut script = parse(&source()).unwrap();
        let mut item = instance("keys");
        item.weight_keys = (0..256)
            .map(|i| weight_key(i as f64 / 256.0, 1.0, ActorEasing::Linear))
            .collect();
        script.scenes[1].composition.clip_instances = vec![item.clone()];
        assert!(script.validate().is_ok());
        script.scenes[1].composition.clip_instances[0]
            .weight_keys
            .push(weight_key(2.0, 1.0, ActorEasing::Linear));
        assert!(script.validate().is_err());
        let layers: Vec<_> = (0..32)
            .map(|i| {
                let mut layer = item.clone();
                layer.id = format!("key-{i}");
                layer
            })
            .collect();
        script.scenes[1].composition.clip_instances = layers.clone();
        assert!(script.validate().is_ok());
        let mut extra = item;
        extra.id = "extra".into();
        extra.actor = "A".into();
        script.scenes[1].composition.clip_instances.push(extra);
        assert!(script.validate().is_err());
        script.scenes[1].composition.clip_instances = layers;
        let template = script.scenes[1].clone();
        script.scenes = (0..8)
            .map(|i| {
                let mut scene = template.clone();
                scene.id = format!("scene-{i}");
                scene
            })
            .collect();
        assert!(script.validate().is_ok());
        let mut extra_scene = template;
        extra_scene.id = "over-budget".into();
        script.scenes.push(extra_scene);
        assert!(script.validate().is_err());
    }

    #[test]
    fn blend_modes_default_strict_roundtrip_and_atomic_upsert() {
        let yaml = "{id: a, clip: head, actor: B, at: 0, duration: 1}";
        let legacy: ClipInstance = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(legacy.blend_mode, ClipBlendMode::Override);
        assert!(!serde_yaml::to_string(&legacy)
            .unwrap()
            .contains("blend_mode"));
        for (name, mode) in [
            ("override", ClipBlendMode::Override),
            ("additive", ClipBlendMode::Additive),
        ] {
            let explicit =
                format!("{{id: a, clip: head, actor: B, at: 0, duration: 1, blend_mode: {name}}}");
            let item: ClipInstance = serde_yaml::from_str(&explicit).unwrap();
            assert_eq!(item.blend_mode, mode);
            assert_eq!(
                serde_yaml::from_str::<ClipInstance>(&serde_yaml::to_string(&item).unwrap())
                    .unwrap(),
                item
            );
        }
        for mode in ["Additive", "replace", "null", "1", "{additive: true}"] {
            let invalid =
                format!("{{id: a, clip: head, actor: B, at: 0, duration: 1, blend_mode: {mode}}}");
            assert!(serde_yaml::from_str::<ClipInstance>(&invalid).is_err());
        }
        let original = source();
        let mut additive = instance("add");
        additive.blend_mode = ClipBlendMode::Additive;
        let saved = upsert_clip_instance(&original, 1, additive.clone()).unwrap();
        let saved = upsert_clip_instance(&saved, 1, instance("over")).unwrap();
        let reopened = parse(&saved).unwrap();
        assert_eq!(
            reopened.scenes[1].composition.clip_instances,
            vec![additive.clone(), instance("over")]
        );
        let before = saved.clone();
        additive.duration = 99.0;
        assert!(upsert_clip_instance(&saved, 1, additive).is_err());
        assert_eq!(saved, before);
        assert_eq!(
            parse(&saved).unwrap().scenes[1].composition.clip_instances,
            reopened.scenes[1].composition.clip_instances
        );
    }

    #[test]
    fn strict_instance_schema_defaults_and_required_fields() {
        let source = source();
        assert!(parse(&source).unwrap().scenes[0]
            .composition
            .clip_instances
            .is_empty());
        let yaml = "{id: a, clip: head, actor: B, at: 0, duration: 1}";
        assert_eq!(
            serde_yaml::from_str::<ClipInstance>(yaml).unwrap().weight,
            1.0
        );
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        for field in ["id", "clip", "actor", "at", "duration"] {
            let mut missing = value.clone();
            missing.as_mapping_mut().unwrap().remove(field);
            assert!(serde_yaml::from_value::<ClipInstance>(missing).is_err());
        }
        for extra in [", unknown: 1", ", weight: true"] {
            assert!(serde_yaml::from_str::<ClipInstance>(&format!(
                "{}{} }}",
                &yaml[..yaml.len() - 1],
                extra
            ))
            .is_err());
        }
    }

    #[test]
    fn instance_validation_rejects_all_invalid_numbers_and_references_even_at_zero_weight() {
        let source = source();
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for field in 0..3 {
                let mut item = instance("a");
                match field {
                    0 => item.at = invalid,
                    1 => item.duration = invalid,
                    _ => item.weight = invalid,
                }
                assert!(upsert_clip_instance(&source, 1, item).is_err());
            }
        }
        let invalid_edits: [fn(&mut ClipInstance); 11] = [
            |item| item.at = -1.0,
            |item| item.duration = 0.0,
            |item| item.duration = -1.0,
            |item| item.duration = 6.0,
            |item| item.weight = -0.01,
            |item| item.weight = 1.01,
            |item| item.clip = "missing".into(),
            |item| item.actor = "missing".into(),
            |item| item.id = "".into(),
            |item| item.id = "bad id".into(),
            |item| item.id = "x".repeat(65),
        ];
        for edit in invalid_edits {
            let mut item = instance("a");
            item.weight = 0.0;
            edit(&mut item);
            assert!(upsert_clip_instance(&source, 1, item).is_err());
        }
        let mut script = parse(&source).unwrap();
        script.scenes[1].duration = f64::MAX;
        let mut item = instance("a");
        item.at = f64::MAX;
        item.duration = 1.0;
        script.scenes[1].composition.clip_instances = vec![item.clone()];
        assert!(script.validate().is_err()); // precision collapse
        item.duration = f64::MAX;
        script.scenes[1].composition.clip_instances = vec![item];
        assert!(script.validate().is_err()); // overflow
        let mut script = parse(&source).unwrap();
        script.scenes[1].composition.clip_instances = vec![instance("same"), instance("same")];
        assert!(script.validate().is_err());
        let mut endpoint = instance(&"x".repeat(64));
        endpoint.at = 0.0;
        endpoint.duration = 6.0;
        endpoint.weight = 0.0;
        assert!(upsert_clip_instance(&source, 1, endpoint).is_ok());
    }

    #[test]
    fn instance_budgets_include_inactive_zero_weight_layers_at_every_boundary() {
        let mut script = parse(&source()).unwrap();
        for actor in ["C", "D", "E"] {
            script
                .characters
                .insert(actor.into(), script.characters["B"].clone());
        }
        let layers: Vec<_> = (0..128)
            .map(|index| {
                let mut item = instance(&format!("i{index}"));
                item.actor = ["A", "B", "C", "D"][index / 32].into();
                item.weight = 0.0;
                item
            })
            .collect();
        script.scenes[1].composition.clip_instances = layers.clone();
        assert!(script.validate().is_ok());
        script.scenes[1]
            .composition
            .clip_instances
            .push(ClipInstance {
                actor: "E".into(),
                ..instance("overflow")
            });
        assert!(script.validate().is_err());
        script.scenes[1].composition.clip_instances = layers[..32].to_vec();
        assert!(script.validate().is_ok());
        script.scenes[1]
            .composition
            .clip_instances
            .push(ClipInstance {
                actor: "A".into(),
                ..instance("actor_overflow")
            });
        assert!(script.validate().is_err());
        script.scenes.truncate(1);
        script.scenes[0].composition.clip_instances = layers;
        for index in 1..8 {
            let mut scene = script.scenes[0].clone();
            scene.id = format!("scene{index}");
            script.scenes.push(scene);
        }
        assert!(script.validate().is_ok());
        let mut scene = script.scenes[0].clone();
        scene.id = "overflow".into();
        scene.composition.clip_instances = vec![instance("extra")];
        script.scenes.push(scene);
        assert!(script.validate().is_err());
    }

    #[test]
    fn atomic_instance_edits_preserve_layer_order_and_copied_tracks() {
        let source = apply_animation_clip(&source(), 1, "B", "head").unwrap();
        let copied = parse(&source).unwrap().scenes[1]
            .composition
            .bone_tracks
            .clone();
        let source = upsert_clip_instance(&source, 1, instance("first")).unwrap();
        let source = upsert_clip_instance(&source, 1, instance("second")).unwrap();
        let mut changed = instance("first");
        changed.weight = 0.5;
        let source = upsert_clip_instance(&source, 1, changed.clone()).unwrap();
        let script = parse(&source).unwrap();
        assert_eq!(
            script.scenes[1].composition.clip_instances,
            vec![changed, instance("second")]
        );
        let moved = reorder_clip_instance(&source, 1, "first", 1).unwrap();
        assert_eq!(
            parse(&moved).unwrap().scenes[1].composition.clip_instances[0].id,
            "second"
        );
        for (index, id, target) in [(1, "first", 2), (1, "missing", 0), (99, "first", 0)] {
            assert!(reorder_clip_instance(&source, index, id, target).is_err());
        }
        assert!(remove_clip_instance(&source, 1, "missing").is_err());
        assert!(remove_clip_instance(&source, 99, "first").is_err());
        let before = source.clone();
        let mut invalid = instance("first");
        invalid.duration = 99.0;
        assert!(upsert_clip_instance(&source, 1, invalid).is_err());
        assert_eq!(source, before);
        let removed = remove_clip_instance(&moved, 1, "first").unwrap();
        let script = parse(&removed).unwrap();
        assert_eq!(
            script.scenes[1].composition.clip_instances,
            vec![instance("second")]
        );
        assert_eq!(script.scenes[1].composition.bone_tracks, copied);
    }

    #[test]
    fn live_reference_guard_replacement_character_cleanup_and_scene_lifecycle() {
        let source = apply_animation_clip(&source(), 1, "B", "head").unwrap();
        let copied = parse(&source).unwrap().scenes[1]
            .composition
            .bone_tracks
            .clone();
        let source = upsert_clip_instance(&source, 1, instance("live")).unwrap();
        let duplicated = duplicate_scene(&source, 1, "duplicate").unwrap();
        let error = remove_animation_clip(&duplicated, "head")
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("2 instance(s)") && error.contains("target") && error.contains("live")
        );
        assert!(capture_animation_clip(&source, 0, "A", "head", false).is_err());
        let replaced = capture_animation_clip(&source, 1, "B", "head", true).unwrap();
        let script = parse(&replaced).unwrap();
        assert_eq!(script.animation_clips["head"].duration, 6.0);
        assert_eq!(
            script.scenes[1].composition.clip_instances,
            vec![instance("live")]
        );
        assert_eq!(script.scenes[1].composition.bone_tracks, copied);
        // A candidate replacement cannot bypass definition validation.
        let mut invalid = parse(&source).unwrap();
        invalid.scenes[0].duration = 601.0;
        let invalid = serde_yaml::to_string(&invalid).unwrap();
        assert!(capture_animation_clip(&invalid, 0, "A", "head", true).is_err());
        let deleted = remove_scene(&duplicated, 1).unwrap();
        assert_eq!(
            parse(&deleted).unwrap().scenes[1]
                .composition
                .clip_instances,
            vec![instance("live")]
        );
        let no_actor = remove_character(&duplicated, "B").unwrap();
        let script = parse(&no_actor).unwrap();
        assert!(script
            .scenes
            .iter()
            .all(|scene| scene.composition.clip_instances.is_empty()));
        assert!(script.animation_clips.contains_key("head"));
        assert!(remove_animation_clip(&no_actor, "head").is_ok());
        let no_instance = remove_clip_instance(&source, 1, "live").unwrap();
        let deleted = remove_animation_clip(&no_instance, "head").unwrap();
        assert_eq!(
            parse(&deleted).unwrap().scenes[1].composition.bone_tracks,
            copied
        );
    }
}
