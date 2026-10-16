//! Deterministic, graphics-independent scene evaluation shared by preview and export.
pub use motion_animation::{Vec2, CHEST, HEAD, HIP, LEFT_FOOT, LEFT_HAND, RIGHT_FOOT, RIGHT_HAND};
use motion_script::{Event, Script};

#[derive(Debug, Clone, PartialEq)]
pub struct ActorPose {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub emotion: String,
    pub rotation: f64,
    pub scale: Vec2,
    /// Wave envelope in 0..=1; zero outside a wave event.
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
    #[error(transparent)]
    Script(#[from] motion_script::Error),
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

pub fn evaluate(
    script: &Script,
    scene_index: usize,
    time_seconds: f64,
) -> Result<EvaluatedScene, Error> {
    script.validate()?;
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
                    Event::Emotion { value, .. } => pose.emotion = value.clone(),
                    Event::Speak {
                        emotion: Some(value),
                        ..
                    } => pose.emotion = value.clone(),
                    Event::Gesture { duration, .. } => {
                        let progress = (time_seconds - at) / duration;
                        if (0.0..1.0).contains(&progress) {
                            pose.wave = pose.wave.max((std::f64::consts::PI * progress).sin());
                        }
                    }
                    Event::Speak { .. } => {}
                }
            }
            pose.skeleton = motion_animation::humanoid(walk_seconds, pose.wave);
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
            pose
        })
        .collect();
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
        assert_eq!(middle.actors[0].skeleton[RIGHT_HAND].y, -85.0);
        assert!(evaluate(&script, 0, f64::NAN).is_err());
        assert!(evaluate(&script, 1, 0.0).is_err());
    }
    #[test]
    fn rejects_invalid_commands_and_overlapping_moves() {
        assert!(motion_script::parse(&SCRIPT.replace("at: 4.0", "at: 1.0")).is_err());
        assert!(motion_script::parse(&SCRIPT.replace("value: wave", "value: dance")).is_err());
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
