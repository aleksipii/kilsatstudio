//! Deterministic, graphics-independent scene evaluation shared by preview and export.
use motion_animation::HumanoidState;
pub use motion_animation::{Vec2, CHEST, HEAD, HIP, LEFT_FOOT, LEFT_HAND, RIGHT_FOOT, RIGHT_HAND};
pub use motion_animation::{GESTURES, POSTURE_BLEND_SECONDS};
use motion_lipsync::Viseme;
use motion_script::{Event, Script};
mod sequence;
pub use sequence::{SceneContribution, SceneSpan, SequenceTimeline, SEQUENCE_SAMPLE_RATE};

/// Head dip, in local skeleton units, at a fully open mouth.
pub const SPEECH_HEAD_DIP: f64 = 3.0;

/// Rule-based mouth openness used for speech head motion; rest and closed are zero.
pub fn viseme_openness(viseme: Viseme) -> f64 {
    match viseme {
        Viseme::Rest | Viseme::Closed => 0.0,
        Viseme::A => 1.0,
        Viseme::O | Viseme::U => 0.7,
        Viseme::E | Viseme::I => 0.4,
        _ => 0.3,
    }
}

/// Applies speech head motion from the same per-actor visemes the mouth renders,
/// so preview and export share one audio sample clock. Missing actors are ignored;
/// actors without a viseme are unchanged.
pub fn apply_speech(scene: &mut EvaluatedScene, mouths: &[(&str, Viseme)]) {
    for actor in &mut scene.actors {
        if let Some((_, viseme)) = mouths.iter().find(|(id, _)| *id == actor.id) {
            actor.skeleton[HEAD].y += viseme_openness(*viseme) * SPEECH_HEAD_DIP;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActorPose {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub emotion: String,
    pub rotation: f64,
    pub scale: Vec2,
    /// Envelope of the active gesture in 0..=1; zero outside gesture events.
    pub wave: f64,
    /// Evaluated local-space skeletal joints shared by preview and export.
    pub skeleton: [Vec2; 7],
}
impl ActorPose {
    /// Transform any local skeleton, facial, or artwork point through the root pose.
    pub fn local_to_world(&self, point: Vec2) -> Vec2 {
        motion_animation::Transform2 {
            translation: Vec2::new(self.x, self.y),
            rotation: self.rotation,
            scale: self.scale,
        }
        .affine()
        .point(point)
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraPose {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}
impl Default for CameraPose {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}
impl CameraPose {
    /// Transform a world point into canvas coordinates; preview and export share this rule.
    pub fn project(self, x: f64, y: f64, width: u32, height: u32) -> (f64, f64) {
        let cx = width as f64 * 0.5;
        let cy = height as f64 * 0.5;
        (
            cx + (x - cx - self.x) * self.zoom,
            cy + (y - cy - self.y) * self.zoom,
        )
    }
}
fn camera_at(keys: &[motion_script::CameraKey], time: f64) -> CameraPose {
    let Some(first) = keys.first() else {
        return CameraPose::default();
    };
    let mut left = first;
    for right in keys.iter().skip(1) {
        if time <= right.at {
            let t = ((time - left.at) / (right.at - left.at)).clamp(0.0, 1.0);
            return CameraPose {
                x: left.x + (right.x - left.x) * t,
                y: left.y + (right.y - left.y) * t,
                zoom: left.zoom + (right.zoom - left.zoom) * t,
            };
        }
        left = right;
    }
    CameraPose {
        x: left.x,
        y: left.y,
        zoom: left.zoom,
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedScene {
    /// Validated scene-local evaluation time in seconds.
    pub time: f64,
    pub camera: CameraPose,
    pub background_color: String,
    /// Whole-canvas opacity over black, for fades at scene edges.
    pub opacity: f64,
    pub width: u32,
    pub height: u32,
    pub actors: Vec<ActorPose>,
}
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid sequence: {0}")]
    InvalidSequence(String),
    #[error(transparent)]
    Script(#[from] motion_script::Error),
    #[error(transparent)]
    Animation(#[from] motion_animation::Error),
    #[error("invalid prepared animation reference: {0}")]
    InvalidAnimation(String),
    #[error("scene index {0} does not exist")]
    MissingScene(usize),
    #[error("evaluation time must be finite and within scene duration")]
    InvalidTime,
    #[error(
        "frame clock requires a finite positive duration, positive fps, and positive frame budget"
    )]
    InvalidFrameClock,
    #[error("frame count exceeds the configured export budget")]
    FrameBudget,
}

/// Count samples at `frame / fps` strictly before the scene endpoint.
///
/// The explicit budget bounds allocations and work before export starts.
/// Fractional final frames are included, while floating multiplication rounding
/// must not introduce an extra sample exactly at the scene endpoint.
pub fn frame_count(duration_seconds: f64, fps: u32, max_frames: u64) -> Result<u64, Error> {
    if !duration_seconds.is_finite() || duration_seconds <= 0.0 || fps == 0 || max_frames == 0 {
        return Err(Error::InvalidFrameClock);
    }
    let mut count = (duration_seconds * fps as f64).ceil();
    if !count.is_finite() || count >= u64::MAX as f64 {
        return Err(Error::FrameBudget);
    }
    if count >= 1.0 && (count - 1.0) / fps as f64 >= duration_seconds {
        count -= 1.0;
    }
    let count = count as u64;
    if count == 0 || count > max_frames {
        return Err(Error::FrameBudget);
    }
    Ok(count)
}

/// An immutable script validated once for repeated scene evaluation.
#[derive(Clone)]
pub struct ValidatedScript {
    script: std::sync::Arc<Script>,
    prepared: std::sync::Arc<PreparedAnimation>,
}
type BoneTracks = [Option<motion_animation::Track>; 7];
#[derive(Default)]
struct PreparedAnimation {
    library: std::collections::BTreeMap<String, BoneTracks>,
    scenes: Vec<std::collections::BTreeMap<String, BoneTracks>>,
}
fn prepare_bones(tracks: &std::collections::BTreeMap<String, Vec<motion_script::ActorKey>>) -> Result<BoneTracks, Error> {
    let mut prepared: BoneTracks = std::array::from_fn(|_| None);
    for (bone, keys) in tracks {
        let joint = motion_animation::HUMANOID_BONES.iter().position(|name| *name == bone.as_str())
            .ok_or_else(|| Error::InvalidAnimation(bone.clone()))?;
        prepared[joint] = Some(motion_animation::Track::new(keys.iter().map(motion_script::ActorKey::animation_key).collect())?);
    }
    Ok(prepared)
}
impl PreparedAnimation {
    fn new(script: &Script) -> Result<Self, Error> {
        // Unused definitions remain source data only. In particular, direct raw
        // evaluation of projects using copied tracks must not copy the library.
        let referenced = script.scenes.iter()
            .flat_map(|scene| scene.composition.clip_instances.iter())
            .map(|instance| instance.clip.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let library = referenced.into_iter().map(|id| {
            let clip = script.animation_clips.get(id)
                .ok_or_else(|| Error::InvalidAnimation(id.into()))?;
            Ok((id.to_owned(), prepare_bones(&clip.bone_tracks)?))
        }).collect::<Result<_, Error>>()?;
        let scenes = script.scenes.iter().map(|scene| scene.composition.bone_tracks.iter().map(|(actor, tracks)| Ok((actor.clone(), prepare_bones(tracks)?))).collect::<Result<_, Error>>()).collect::<Result<_, Error>>()?;
        Ok(Self { library, scenes })
    }
    fn offsets(&self, script: &Script, scene_index: usize, actor: &str, time: f64) -> Result<Vec<(usize, motion_animation::Transform2)>, Error> {
        let mut values = [None; 7];
        if let Some(tracks) = self.scenes[scene_index].get(actor) {
            for (value, track) in values.iter_mut().zip(tracks) {
                if let Some(track) = track { *value = Some(track.sample(time)?); }
            }
        }
        for instance in &script.scenes[scene_index].composition.clip_instances {
            if instance.actor != actor || time < instance.at || time >= instance.at + instance.duration || instance.weight == 0.0 { continue; }
            let clip = script.animation_clips.get(&instance.clip).ok_or_else(|| Error::InvalidAnimation(instance.clip.clone()))?;
            let tracks = self.library.get(&instance.clip).ok_or_else(|| Error::InvalidAnimation(instance.clip.clone()))?;
            let clip_time = (time - instance.at) / instance.duration * clip.duration;
            if !clip_time.is_finite() || clip_time < 0.0 || clip_time > clip.duration { return Err(Error::InvalidTime); }
            for (value, track) in values.iter_mut().zip(tracks) {
                if let Some(track) = track {
                    *value = Some(motion_animation::blend_humanoid_offset(value.unwrap_or_default(), track.sample(clip_time)?, instance.weight)?);
                }
            }
        }
        Ok(values.into_iter().enumerate().filter_map(|(joint, value)| value.map(|value| (joint, value))).collect())
    }
}
impl ValidatedScript {
    pub fn new(script: std::sync::Arc<Script>) -> Result<Self, Error> {
        script.validate()?;
        let prepared = std::sync::Arc::new(PreparedAnimation::new(&script)?);
        Ok(Self { script, prepared })
    }
    pub fn evaluate(&self, scene_index: usize, time_seconds: f64) -> Result<EvaluatedScene, Error> {
        evaluate_validated(&self.script, &self.prepared, scene_index, time_seconds)
    }
}
pub fn evaluate(script: &Script, scene_index: usize, time_seconds: f64) -> Result<EvaluatedScene, Error> {
    script.validate()?;
    let prepared = PreparedAnimation::new(script)?;
    evaluate_validated(script, &prepared, scene_index, time_seconds)
}
fn evaluate_validated(script: &Script, prepared: &PreparedAnimation, scene_index: usize, time_seconds: f64) -> Result<EvaluatedScene, Error> {
    let scene = script
        .scenes
        .get(scene_index)
        .ok_or(Error::MissingScene(scene_index))?;
    if !time_seconds.is_finite() || time_seconds < 0.0 || time_seconds > scene.duration {
        return Err(Error::InvalidTime);
    }
    let mut events: Vec<_> = scene.events.iter().collect();
    // Stable ordering preserves source order for simultaneous expression events.
    events.sort_by(|a, b| a.actor_time().0.total_cmp(&b.actor_time().0));
    let mut actors: Vec<_> = script
        .characters
        .keys()
        .filter(|id| {
            scene
                .composition
                .layers
                .get(*id)
                .is_none_or(|layer| layer.visible)
        })
        .map(|id| {
            let mut pose = ActorPose {
                id: id.clone(),
                x: script.project.width as f64 / 2.0,
                y: script.project.height as f64 * 0.7,
                emotion: "neutral".into(),
                rotation: 0.0,
                scale: Vec2::new(1.0, 1.0),
                wave: 0.0,
                skeleton: motion_animation::humanoid(None, 0.0),
            };
            let mut walk_seconds = None;
            let mut emotion_changes = Vec::new();
            let mut gesture = None;
            let mut set_emotion = |pose: &mut ActorPose, value: &String, at: f64| {
                if pose.emotion != *value {
                    pose.emotion = value.clone();
                    emotion_changes.push((value.clone(), time_seconds - at));
                }
            };
            for event in &events {
                let (at, actor) = event.actor_time();
                if actor != id || at > time_seconds {
                    continue;
                }
                match event {
                    Event::Move { x, y, duration, .. } => {
                        // Overlapping moves are rejected by validation, so each starting
                        // pose is the completed endpoint of the preceding movement.
                        let progress = ((time_seconds - at) / duration).clamp(0.0, 1.0);
                        walk_seconds = if progress < 1.0 && (pose.x != *x || pose.y != *y) {
                            Some(time_seconds - at)
                        } else {
                            None
                        };
                        pose.x += (x - pose.x) * progress;
                        pose.y += (y - pose.y) * progress;
                    }
                    Event::Emotion { value, .. } => set_emotion(&mut pose, value, at),
                    Event::Speak {
                        emotion: Some(value),
                        ..
                    } => set_emotion(&mut pose, value, at),
                    Event::Gesture {
                        value, duration, ..
                    } => {
                        // Validation guarantees a known gesture. The strongest active
                        // envelope wins; ties keep the earlier event.
                        let progress = (time_seconds - at) / duration;
                        let weight = motion_animation::gesture_envelope(progress);
                        if weight > pose.wave {
                            pose.wave = weight;
                            gesture = motion_animation::Gesture::parse(value)
                                .map(|g| (g, progress, weight));
                        }
                    }
                    Event::Speak { .. } => {}
                }
            }
            let emotion_changes: Vec<_> = emotion_changes
                .iter()
                .map(|(emotion, since)| (emotion.as_str(), *since))
                .collect();
            let offsets = prepared.offsets(script, scene_index, id, time_seconds)?;
            let weights = scene.composition.sample_clip_weights(id, time_seconds);
            pose.wave *= weights.gesture;
            pose.skeleton = motion_animation::evaluate_humanoid_with_weights_and_offsets(
                &HumanoidState {
                    walk_seconds,
                    emotion_changes: &emotion_changes,
                    gesture,
                },
                &offsets,
                weights,
            )
            ?;
            if let Some(keys) = scene.composition.actor_tracks.get(id) {
                // Validation above guarantees this track and scene-clock sample are valid.
                let value = motion_animation::Track::new(
                    keys.iter()
                        .map(motion_script::ActorKey::animation_key)
                        .collect(),
                )
                .expect("validated actor track")
                .sample(time_seconds)
                .expect("validated scene time");
                pose.x += value.translation.x;
                pose.y += value.translation.y;
                pose.rotation = value.rotation;
                pose.scale = value.scale;
            }
            Ok(pose)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    actors.sort_by_key(|actor| {
        scene
            .composition
            .layers
            .get(&actor.id)
            .map_or(0, |layer| layer.z)
    });
    let composition = &scene.composition;
    let fade_in = if composition.fade_in > 0.0 {
        (time_seconds / composition.fade_in).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let fade_out = if composition.fade_out > 0.0 {
        ((scene.duration - time_seconds) / composition.fade_out).clamp(0.0, 1.0)
    } else {
        1.0
    };
    Ok(EvaluatedScene {
        time: time_seconds,
        camera: camera_at(&composition.camera, time_seconds),
        background_color: composition
            .background_keys
            .iter()
            .rev()
            .find(|key| key.at <= time_seconds)
            .map_or(&composition.background_color, |key| &key.color)
            .clone(),
        opacity: fade_in.min(fade_out),
        width: script.project.width,
        height: script.project.height,
        actors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const SCRIPT: &str = r#"
project: {title: Hello Emma, fps: 24, width: 1920, height: 1080}
characters:
  EMMA:
    asset: characters/emma
    default_pose: idle
    voice: {provider: kokoro, preset: af_heart, language: en-us, speed: 1.0}
scenes:
  - id: intro
    duration: 8.0
    background: studio
    events:
      - {type: move, at: 0.0, actor: EMMA, x: 400, y: 700, duration: 2.0}
      - {type: emotion, at: 2.0, actor: EMMA, value: happy}
      - {type: gesture, at: 2.5, actor: EMMA, value: wave, duration: 1.0}
      - {type: speak, at: 3.0, actor: EMMA, text: Hello! Welcome to Kilsat Studio.}
      - {type: move, at: 4.0, actor: EMMA, x: 800, y: 700, duration: 2.0}
"#;
    #[test]
    fn validated_script_retains_immutable_snapshot_and_evaluation_boundaries() {
        let mut script = std::sync::Arc::new(motion_script::parse(SCRIPT).unwrap());
        let validated = ValidatedScript::new(script.clone()).unwrap();
        for time in [0.0, 2.5, 7.9] {
            assert_eq!(
                validated.evaluate(0, time).unwrap(),
                evaluate(&script, 0, time).unwrap()
            );
        }
        // Mutation through another owner copies rather than invalidating the snapshot.
        std::sync::Arc::make_mut(&mut script).scenes[0].duration = -1.0;
        assert!(evaluate(&script, 0, 0.0).is_err());
        assert!(validated.evaluate(0, 0.0).is_ok());
        assert!(validated.evaluate(1, 0.0).is_err());
        assert!(validated.evaluate(0, f64::NAN).is_err());
        assert!(ValidatedScript::new(script).is_err());
    }
    #[test]
    fn combined_script_deterministic_boundaries_and_seeking() {
        let script = motion_script::parse(SCRIPT).unwrap();
        assert_eq!(evaluate(&script, 0, 0.0).unwrap().actors[0].x, 960.0);
        assert_eq!(evaluate(&script, 0, 1.0).unwrap().actors[0].x, 680.0);
        let middle = evaluate(&script, 0, 3.0).unwrap();
        assert_eq!(middle.time, 3.0);
        for time in [8.0, 0.0, 1.25, 3.0] {
            assert_eq!(evaluate(&script, 0, time).unwrap().time, time);
        }
        assert_eq!(middle.actors[0].emotion, "happy");
        assert_eq!(middle.actors[0].wave, 1.0);
        assert_eq!(evaluate(&script, 0, 3.5).unwrap().actors[0].wave, 0.0);
        assert_eq!(evaluate(&script, 0, 5.0).unwrap().actors[0].x, 600.0);
        assert_eq!(evaluate(&script, 0, 8.0).unwrap().actors[0].x, 800.0);
        assert_eq!(middle, evaluate(&script, 0, 3.0).unwrap());
        assert_ne!(
            evaluate(&script, 0, 1.0 / 6.0).unwrap().actors[0].skeleton,
            evaluate(&script, 0, 2.0).unwrap().actors[0].skeleton
        );
        // Wave lifts the hand 65 units; the happy posture raises the chest 3 more.
        assert_eq!(middle.actors[0].skeleton[RIGHT_HAND].y, -88.0);
        assert!(evaluate(&script, 0, f64::NAN).is_err());
        assert!(evaluate(&script, 1, 0.0).is_err());
    }
    #[test]
    fn rule_layers_select_gesture_clip_and_blend_posture() {
        let script = motion_script::parse(&SCRIPT.replace("value: wave", "value: nod")).unwrap();
        let at = |t| evaluate(&script, 0, t).unwrap().actors[0].clone();
        // Nod moves the head, not the hand.
        assert!(at(2.75).skeleton[HEAD].y > at(2.4).skeleton[HEAD].y);
        assert_eq!(at(2.75).skeleton[RIGHT_HAND], at(2.4).skeleton[RIGHT_HAND]);
        assert_eq!(at(3.5).skeleton, at(3.9).skeleton);
        // Happy posture eases in after the emotion event at 2.0 s.
        let before = at(1.99).skeleton[CHEST].y;
        let during = at(2.0 + POSTURE_BLEND_SECONDS / 2.0).skeleton[CHEST].y;
        let after = at(2.0 + POSTURE_BLEND_SECONDS).skeleton[CHEST].y;
        assert!(before > during && during > after);
        assert_eq!(after, at(2.4).skeleton[CHEST].y);
        // Seeking backwards yields the same pose as evaluating directly.
        let forward = at(2.1);
        at(7.0);
        assert_eq!(at(2.1), forward);
        // A second emotion inside the blend window continues from the in-flight pose.
        let quick = motion_script::parse(&SCRIPT.replace(
            "      - {type: gesture",
            "      - {type: emotion, at: 2.1, actor: EMMA, value: sad}\n      - {type: gesture",
        ))
        .unwrap();
        let chest = |t| evaluate(&quick, 0, t).unwrap().actors[0].skeleton[CHEST].y;
        assert!((chest(2.1) - chest(2.0999)).abs() < 0.01);
        assert!(chest(2.1 + POSTURE_BLEND_SECONDS) > chest(1.99));
        for name in motion_animation::GESTURES {
            motion_script::parse(&SCRIPT.replace("value: wave", &format!("value: {name}")))
                .unwrap();
        }
    }
    #[test]
    fn speech_head_motion_follows_supplied_visemes_only() {
        let script = motion_script::parse(SCRIPT).unwrap();
        let base = evaluate(&script, 0, 3.0).unwrap();
        let mut open = base.clone();
        apply_speech(&mut open, &[("EMMA", Viseme::A)]);
        assert_eq!(
            open.actors[0].skeleton[HEAD].y,
            base.actors[0].skeleton[HEAD].y + SPEECH_HEAD_DIP
        );
        assert_eq!(
            open.actors[0].skeleton[CHEST],
            base.actors[0].skeleton[CHEST]
        );
        let mut silent = base.clone();
        apply_speech(&mut silent, &[("EMMA", Viseme::Rest), ("OTHER", Viseme::A)]);
        assert_eq!(silent, base);
        let mut none = base.clone();
        apply_speech(&mut none, &[]);
        assert_eq!(none, base);
    }
    #[test]
    fn rejects_invalid_commands_and_overlapping_moves() {
        assert!(motion_script::parse(&SCRIPT.replace("at: 4.0", "at: 1.0")).is_err());
        assert!(matches!(
            motion_script::parse(&SCRIPT.replace("value: wave", "value: dance")),
            Err(motion_script::Error::Unsupported(message)) if message.contains("wave, point, nod, shrug")
        ));
        assert!(motion_script::parse(&SCRIPT.replace("value: happy", "value: confused")).is_err());
        assert!(motion_script::parse(&SCRIPT.replace("x: 800", "x: .nan")).is_err());
        assert!(motion_script::parse(&SCRIPT.replace("duration: 8.0", "duration: 5.0")).is_err());
    }
    #[test]
    fn exclusive_frame_count_handles_decimal_and_fractional_durations() {
        assert_eq!(frame_count(0.28, 25, 1000).unwrap(), 7);
        assert_eq!(frame_count(8.3, 30, 1000).unwrap(), 249);
        assert_eq!(frame_count(0.3, 24, 1000).unwrap(), 8);
        assert_eq!(frame_count(1.0, 24, 24).unwrap(), 24);
        assert_eq!(frame_count(0.001, 24, 1).unwrap(), 1);
        assert!(matches!(frame_count(1.0, 24, 23), Err(Error::FrameBudget)));
        assert!(matches!(
            frame_count(f64::MAX, 24, 1000),
            Err(Error::FrameBudget)
        ));
        for duration in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            assert!(matches!(
                frame_count(duration, 24, 1000),
                Err(Error::InvalidFrameClock)
            ));
        }
        assert!(matches!(
            frame_count(1.0, 0, 1000),
            Err(Error::InvalidFrameClock)
        ));
        assert!(matches!(
            frame_count(1.0, 24, 0),
            Err(Error::InvalidFrameClock)
        ));
    }
}

#[cfg(test)]
mod composition_tests {
    use super::*;
    #[test]
    fn camera_layers_fades_seek_deterministically_on_scene_clock() {
        let script = motion_script::parse(
            r##"
project: {width: 640, height: 360}
characters: {A: {asset: a}, B: {asset: b}, C: {asset: c}}
scenes:
- id: shot
  duration: 4
  events: []
  composition:
    background_color: '#aabbcc'
    fade_in: 1
    fade_out: 1
    camera: [{at: 0, x: 0, y: 0, zoom: 1}, {at: 4, x: 100, y: 40, zoom: 3}]
    layers: {A: {z: 2}, B: {z: -1}, C: {visible: false}}
"##,
        )
        .unwrap();
        let middle = evaluate(&script, 0, 2.0).unwrap();
        assert_eq!(
            middle
                .actors
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            vec!["B", "A"]
        );
        assert_eq!(
            middle.camera,
            CameraPose {
                x: 50.0,
                y: 20.0,
                zoom: 2.0
            }
        );
        assert_eq!(
            middle.camera.project(320.0, 180.0, 640, 360),
            (220.0, 140.0)
        );
        assert_eq!(middle.background_color, "#aabbcc");
        assert_eq!(middle.opacity, 1.0);
        assert_eq!(evaluate(&script, 0, 0.0).unwrap().opacity, 0.0);
        assert_eq!(evaluate(&script, 0, 0.5).unwrap().opacity, 0.5);
        assert_eq!(evaluate(&script, 0, 3.5).unwrap().opacity, 0.5);
        assert_eq!(evaluate(&script, 0, 4.0).unwrap().opacity, 0.0);
        assert_eq!(middle, evaluate(&script, 0, 2.0).unwrap());
    }
}

#[cfg(test)]
mod actor_track_tests {
    use super::*;
    #[test]
    fn root_tracks_layer_on_moves_share_clock_and_transform_face_points() {
        let script = motion_script::parse(
            r#"
project: {width: 640, height: 360}
characters: {A: {asset: a}, B: {asset: b}}
scenes:
- id: shot
  duration: 4
  events: [{type: move, at: 0, actor: A, x: 420, y: 252, duration: 4}]
  composition:
    actor_tracks:
      A:
      - {at: 0, x: 0, easing: smoothstep}
      - {at: 4, x: 80, rotation: 3.141592653589793, scale_x: 2, scale_y: 3}
      B:
      - {at: 1, y: 10, easing: hold}
      - {at: 3, y: 90}
"#,
        )
        .unwrap();
        let start = evaluate(&script, 0, 0.0).unwrap();
        assert_eq!(start.actors[0].x, 320.0);
        assert_eq!(start.actors[1].y, 262.0); // before first key clamps
        let quarter = evaluate(&script, 0, 1.0).unwrap();
        assert_eq!(quarter.actors[0].x, 345.0 + 12.5);
        assert_eq!(quarter.actors[0].scale, Vec2::new(1.15625, 1.3125));
        assert_eq!(evaluate(&script, 0, 2.99).unwrap().actors[1].y, 262.0);
        assert_eq!(evaluate(&script, 0, 3.0).unwrap().actors[1].y, 342.0);
        let end = evaluate(&script, 0, 4.0).unwrap();
        let a = &end.actors[0];
        assert_eq!(a.x, 500.0);
        let face = a.local_to_world(Vec2::new(0.0, -90.0));
        assert!((face.x - 500.0).abs() < 1e-9);
        assert!((face.y - 522.0).abs() < 1e-9);
        assert_eq!(quarter, evaluate(&script, 0, 1.0).unwrap());
    }
}

#[cfg(test)]
mod background_key_tests {
    use super::*;
    #[test]
    fn background_steps_are_inclusive_and_seek_order_independent() {
        let script = motion_script::parse(
            r##"
characters: {}
scenes:
- id: shot
  duration: 4
  events: []
  composition:
    background_color: '#112233'
    background_keys:
    - {at: 1, color: '#445566'}
    - {at: 3, color: '#778899'}
    - {at: 4, color: '#aabbcc'}
"##,
        )
        .unwrap();
        for (time, color) in [
            (4.0, "#aabbcc"),
            (0.0, "#112233"),
            (3.0, "#778899"),
            (0.999, "#112233"),
            (1.0, "#445566"),
            (2.999, "#445566"),
            (4.0, "#aabbcc"),
        ] {
            assert_eq!(evaluate(&script, 0, time).unwrap().background_color, color);
        }
        let mut at_zero = script.clone();
        at_zero.scenes[0].composition.background_keys[0].at = 0.0;
        assert_eq!(
            evaluate(&at_zero, 0, 0.0).unwrap().background_color,
            "#445566"
        );
    }
}

#[cfg(test)]
mod bone_track_scene_tests {
    use super::*;
    #[test]
    fn authored_local_keys_seek_interpolate_and_preserve_rule_gesture() {
        let source = "characters: {A: {asset: a}}\nscenes: [{id: shot, duration: 4, events: [{type: gesture, at: 0, actor: A, value: wave, duration: 4}]}]";
        let base = motion_script::parse(source).unwrap();
        let source = motion_script::apply_bone_track(
            source,
            0,
            "A",
            "right_hand",
            &[
                motion_script::ActorKey::default(),
                motion_script::ActorKey {
                    at: 4.0,
                    x: 40.0,
                    ..Default::default()
                },
            ],
        )
        .unwrap();
        let script = motion_script::parse(&source).unwrap();
        for time in [2.0, 0.0, 4.0, 1.0, 2.0] {
            let original = evaluate(&base, 0, time).unwrap();
            let actual = evaluate(&script, 0, time).unwrap();
            let joint = motion_animation::RIGHT_HAND;
            assert_eq!(
                actual.actors[0].skeleton[joint].x,
                original.actors[0].skeleton[joint].x + time * 10.0
            );
            assert_eq!(
                actual.actors[0].skeleton[joint].y,
                original.actors[0].skeleton[joint].y
            );
            assert_eq!(
                actual.actors[0].skeleton[motion_animation::HEAD],
                original.actors[0].skeleton[motion_animation::HEAD]
            );
        }
    }
}

#[cfg(test)]
mod clip_weight_tests {
    use super::*;

    #[test]
    fn actor_intensities_isolate_rules_and_keep_authored_and_speech_motion() {
        let source = "characters: {A: {asset: a}, B: {asset: b}}\nscenes: [{id: shot, duration: 4, events: [{type: gesture, actor: A, at: 0, value: wave, duration: 4}, {type: gesture, actor: B, at: 0, value: wave, duration: 4}, {type: emotion, actor: A, at: 0, value: happy}, {type: move, actor: A, at: 0, x: 500, y: 400, duration: 4}]}]";
        let source = motion_script::apply_bone_track(
            source,
            0,
            "A",
            "right_hand",
            &[motion_script::ActorKey {
                x: 20.0,
                ..Default::default()
            }],
        )
        .unwrap();
        let original = motion_script::parse(&source).unwrap();
        let off = motion_script::ClipWeights {
            posture: 0.0,
            walk: 0.0,
            gesture: 0.0,
        };
        let edited = motion_script::apply_clip_weights(&source, 0, "A", off).unwrap();
        let script = motion_script::parse(&edited).unwrap();
        let mut neutral = script.clone();
        neutral.scenes[0].events.clear();
        for time in [2.0, 0.0, 4.0, 0.02, 1.0, 2.0] {
            let before = evaluate(&original, 0, time).unwrap();
            let mut actual = evaluate(&script, 0, time).unwrap();
            let rest = evaluate(&neutral, 0, time).unwrap();
            assert_eq!(actual.actors[1], before.actors[1]);
            assert_eq!(actual.actors[0].skeleton, rest.actors[0].skeleton);
            assert_eq!(actual.actors[0].wave, 0.0);
            assert_eq!(actual.actors[0].x, before.actors[0].x);
            assert_eq!(actual.actors[0].y, before.actors[0].y);
            assert_eq!(actual.actors[0].emotion, before.actors[0].emotion);
            let head_y = actual.actors[0].skeleton[HEAD].y;
            apply_speech(&mut actual, &[("A", Viseme::A)]);
            assert_eq!(actual.actors[0].skeleton[HEAD].y, head_y + SPEECH_HEAD_DIP);
        }
        let half = motion_script::apply_clip_weights(
            &source,
            0,
            "A",
            motion_script::ClipWeights {
                gesture: 0.5,
                ..Default::default()
            },
        )
        .unwrap();
        let half = evaluate(&motion_script::parse(&half).unwrap(), 0, 2.0).unwrap();
        let full = evaluate(&original, 0, 2.0).unwrap();
        assert_eq!(half.actors[0].wave, full.actors[0].wave * 0.5);
        assert_eq!(half.actors[1].wave, full.actors[1].wave);
    }
}

#[cfg(test)]
mod animated_clip_weight_tests {
    use super::*;

    #[test]
    fn animated_strengths_match_static_evaluation_on_shared_scene_clock() {
        let source = "characters: {A: {asset: a}, B: {asset: b}}\nscenes: [{id: shot, duration: 4, events: [{type: gesture, actor: A, at: 0, value: wave, duration: 4}, {type: gesture, actor: B, at: 0, value: wave, duration: 4}, {type: emotion, actor: A, at: 0, value: happy}, {type: move, actor: A, at: 0, x: 500, y: 400, duration: 4}]}]";
        let keys = vec![
            motion_script::ClipWeightKeyframe {
                time: 0.0,
                weights: motion_script::ClipWeights {
                    posture: 0.0,
                    walk: 1.0,
                    gesture: 0.2,
                },
                easing: motion_script::ActorEasing::Smoothstep,
            },
            motion_script::ClipWeightKeyframe {
                time: 4.0,
                weights: motion_script::ClipWeights {
                    posture: 1.0,
                    walk: 0.0,
                    gesture: 0.8,
                },
                easing: motion_script::ActorEasing::Linear,
            },
        ];
        let tracked_source = motion_script::apply_clip_weight_track(source, 0, "A", keys).unwrap();
        let tracked = motion_script::parse(&tracked_source).unwrap();
        let original = motion_script::parse(source).unwrap();
        // Deliberately seek out of order: evaluation must not accumulate state.
        for time in [4.0, 1.0, 0.0, 2.5, 0.5, 1.0] {
            let t = time / 4.0;
            let eased = t * t * (3.0 - 2.0 * t);
            let expected_weights = motion_script::ClipWeights {
                posture: eased,
                walk: 1.0 - eased,
                gesture: 0.2 + (0.8 - 0.2) * eased,
            };
            let expected_source =
                motion_script::apply_clip_weights(source, 0, "A", expected_weights).unwrap();
            let expected =
                evaluate(&motion_script::parse(&expected_source).unwrap(), 0, time).unwrap();
            let actual = evaluate(&tracked, 0, time).unwrap();
            let before = evaluate(&original, 0, time).unwrap();
            assert_eq!(actual.actors, expected.actors);
            assert_eq!(actual.actors[1], before.actors[1]);
            assert_eq!(actual.actors[0].x, before.actors[0].x);
            assert_eq!(actual.actors[0].emotion, before.actors[0].emotion);
        }
    }
}

#[cfg(test)]
mod live_instance_tests {
    use super::*;
    use motion_animation::{Transform2, HUMANOID_BONES};
    fn script() -> Script {
        motion_script::parse("characters: {A: {asset: a}}\nanimation_clips:\n  first:\n    duration: 2\n    bone_tracks:\n      head: [{at: 0, x: 20}, {at: 2, x: 60}]\n  second:\n    duration: 1\n    bone_tracks:\n      head: [{at: 0, x: 100}]\n      right_hand: [{at: 0, y: 20}]\nscenes:\n  - id: shot\n    duration: 6\n    events: []\n    composition:\n      bone_tracks:\n        A:\n          head: [{at: 0, x: 8}]\n").unwrap()
    }
    fn instance(id:&str, clip:&str, at:f64, duration:f64, weight:f64) -> motion_script::ClipInstance {
        motion_script::ClipInstance {id:id.into(),clip:clip.into(),actor:"A".into(),at,duration,weight}
    }
    fn offsets(script: &Script, time:f64) -> Vec<(usize,Transform2)> {
        script.validate().unwrap();
        PreparedAnimation::new(script).unwrap().offsets(script,0,"A",time).unwrap()
    }
    fn bone(offsets:&[(usize,Transform2)], name:&str) -> Transform2 {
        let joint=HUMANOID_BONES.iter().position(|bone| *bone==name).unwrap();
        offsets.iter().find(|(i,_)| *i==joint).map(|(_,v)|*v).unwrap_or_default()
    }
    #[test]
    fn independently_expected_weights_order_and_missing_bones() {
        let mut script=script();
        for (weight,expected) in [(0.0,8.0),(0.5,24.0),(1.0,40.0)] {
            script.scenes[0].composition.clip_instances=vec![instance("a","first",1.0,2.0,weight)];
            let actual=offsets(&script,2.0);
            assert_eq!(bone(&actual,"head").translation.x,expected);
            assert_eq!(bone(&actual,"right_hand"),Transform2::default());
        }
        script.scenes[0].composition.clip_instances=vec![instance("a","first",1.0,2.0,0.5),instance("b","second",1.0,2.0,0.5)];
        let actual=offsets(&script,2.0);
        assert_eq!(bone(&actual,"head").translation.x,62.0);
        assert_eq!(bone(&actual,"right_hand").translation.y,10.0);
        script.scenes[0].composition.clip_instances.reverse();
        assert_eq!(bone(&offsets(&script,2.0),"head").translation.x,47.0);
    }
    #[test]
    fn half_open_compressed_adjacent_windows_endpoint_holding_and_seeks() {
        let mut script=script();
        script.scenes[0].composition.clip_instances=vec![instance("a","first",1.0,0.5,1.0),instance("b","second",1.5,0.5,1.0)];
        for (time,expected) in [(1.25,40.0),(0.0,8.0),(1.0,20.0),(1.5,100.0),(2.0,8.0),(6.0,8.0),(1.25,40.0)] {
            assert_eq!(bone(&offsets(&script,time),"head").translation.x,expected);
        }
        script.animation_clips.get_mut("first").unwrap().bone_tracks.get_mut("head").unwrap()[0].at=0.5;
        assert_eq!(bone(&offsets(&script,1.0),"head").translation.x,20.0);
        assert_eq!(bone(&offsets(&script,1.125),"head").translation.x,20.0);
        script.animation_clips.get_mut("first").unwrap().bone_tracks.get_mut("head").unwrap()[1].at=1.0;
        assert_eq!(bone(&offsets(&script,1.375),"head").translation.x,60.0);
    }
    #[test]
    fn prepared_snapshot_shared_by_clone_reuses_bounded_storage_and_matches_direct() {
        let mut script=script();
        script.scenes[0].composition.clip_instances=vec![instance("a","first",1.0,2.0,0.5)];
        let raw=std::sync::Arc::new(script);
        let prepared=ValidatedScript::new(raw.clone()).unwrap();
        let clone=prepared.clone();
        assert!(std::sync::Arc::ptr_eq(&prepared.prepared,&clone.prepared));
        let track=prepared.prepared.library["first"][HEAD].as_ref().unwrap() as *const motion_animation::Track;
        for time in [2.0,0.0,3.0,1.0,2.000020833333333,2.0] {
            assert_eq!(prepared.evaluate(0,time).unwrap(),evaluate(&raw,0,time).unwrap());
            assert_eq!(track,clone.prepared.library["first"][HEAD].as_ref().unwrap() as *const motion_animation::Track);
        }
        assert_eq!(prepared.prepared.library.len(),1);
        assert!(!prepared.prepared.library.contains_key("second"));
        assert_eq!(prepared.prepared.scenes.len(),1);
    }
    #[test]
    fn unused_library_is_not_prepared_and_duplicate_references_share_one_definition() {
        let mut script = script();
        let unused = PreparedAnimation::new(&script).unwrap();
        assert!(unused.library.is_empty());
        assert_eq!(unused.scenes[0]["A"].iter().flatten().count(), 1);
        script.scenes[0].composition.clip_instances = vec![
            instance("a", "first", 1.0, 2.0, 0.5),
            instance("b", "first", 2.0, 2.0, 0.0),
        ];
        script.validate().unwrap();
        let prepared = ValidatedScript::new(std::sync::Arc::new(script)).unwrap();
        assert_eq!(prepared.prepared.library.len(), 1);
        assert!(prepared.prepared.library.contains_key("first"));
        assert!(!prepared.prepared.library.contains_key("second"));
        let cloned = prepared.clone();
        assert!(std::sync::Arc::ptr_eq(&prepared.prepared, &cloned.prepared));
        for time in [3.0, 0.0, 1.5, 2.0, 1.5] {
            prepared.evaluate(0, time).unwrap();
        }
        assert_eq!(prepared.prepared.library.len(), 1);
        assert_eq!(prepared.prepared.library["first"].iter().flatten().count(), 1);
    }

    #[test]
    fn empty_instances_preserve_authored_geometry_and_independent_base_speech_root() {
        let script=script();
        let at=evaluate(&script,0,2.0).unwrap();
        assert_eq!(at.actors[0].skeleton[HEAD],Vec2::new(8.0,-90.0));
        let mut live=script.clone();
        live.scenes[0].composition.clip_instances=vec![instance("a","first",1.0,2.0,0.5)];
        let mut actual=evaluate(&live,0,2.0).unwrap();
        assert_eq!(actual.actors[0].skeleton[HEAD],Vec2::new(24.0,-90.0));
        assert_eq!(actual.actors[0].x,at.actors[0].x);
        assert_eq!(actual.actors[0].emotion,at.actors[0].emotion);
        apply_speech(&mut actual,&[("A",Viseme::A)]);
        assert_eq!(actual.actors[0].skeleton[HEAD],Vec2::new(24.0,-87.0));
    }
}
