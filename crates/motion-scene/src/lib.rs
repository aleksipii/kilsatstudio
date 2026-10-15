//! Deterministic, graphics-independent scene evaluation shared by preview and export.
use motion_script::{Event, Script};

#[derive(Debug, Clone, PartialEq)]
pub struct ActorPose {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub emotion: String,
    /// Wave envelope in 0..=1; zero outside a wave event.
    pub wave: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedScene {
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
    let actors = script
        .characters
        .keys()
        .map(|id| {
            let mut pose = ActorPose {
                id: id.clone(),
                x: script.project.width as f64 / 2.0,
                y: script.project.height as f64 * 0.7,
                emotion: "neutral".into(),
                wave: 0.0,
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
            pose
        })
        .collect();
    Ok(EvaluatedScene {
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
        assert_eq!(middle.actors[0].emotion, "happy");
        assert_eq!(middle.actors[0].wave, 1.0);
        assert_eq!(evaluate(&script, 0, 3.5).unwrap().actors[0].wave, 0.0);
        assert_eq!(evaluate(&script, 0, 5.0).unwrap().actors[0].x, 600.0);
        assert_eq!(evaluate(&script, 0, 8.0).unwrap().actors[0].x, 800.0);
        assert_eq!(middle, evaluate(&script, 0, 3.0).unwrap());
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
}
