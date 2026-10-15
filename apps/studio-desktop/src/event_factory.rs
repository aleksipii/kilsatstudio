//! Detached event drafts. Applying a draft must still validate the whole document.
use motion_script::{Event, Script};

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Move,
    Emotion,
    Gesture,
    Speak,
}

fn snapped_time(script: &Script, scene_index: usize, at: f64) -> Result<f64, String> {
    if !at.is_finite() {
        return Err("Event time must be finite".into());
    }
    let scene = script
        .scenes
        .get(scene_index)
        .ok_or("Select an existing scene")?;
    let count = motion_scene::frame_count(scene.duration, script.project.fps, (1_u64 << 53) - 1)
        .map_err(|error| error.to_string())?;
    let rate = f64::from(script.project.fps);
    Ok((at * rate).round().clamp(0.0, (count - 1) as f64) / rate)
}

/// Create a draft at the nearest scene frame, strictly before the scene endpoint.
/// Speech uses the character's voice configuration when applied; no audio is fabricated.
pub fn create(
    script: &Script,
    scene_index: usize,
    actor: &str,
    at: f64,
    kind: Kind,
) -> Result<Event, String> {
    if !script.characters.contains_key(actor) {
        return Err("Select an existing character".into());
    }
    let at = snapped_time(script, scene_index, at)?;
    let actor = actor.to_owned();
    let duration = (script.scenes[scene_index].duration - at).min(1.0);
    Ok(match kind {
        Kind::Move => {
            let scene = motion_scene::evaluate(script, scene_index, at)
                .map_err(|error| error.to_string())?;
            let pose = scene
                .actors
                .iter()
                .find(|pose| pose.id == actor)
                .ok_or("Character pose is unavailable")?;
            Event::Move {
                at,
                actor,
                x: pose.x,
                y: pose.y,
                duration,
            }
        }
        Kind::Emotion => Event::Emotion {
            at,
            actor,
            value: "neutral".into(),
        },
        Kind::Gesture => Event::Gesture {
            at,
            actor,
            value: "wave".into(),
            duration,
        },
        Kind::Speak => Event::Speak {
            at,
            actor,
            text: "New dialogue.".into(),
            emotion: None,
            audio: None,
        },
    })
}

/// Clone every field except the frame-snapped start time. A duration may extend
/// past the scene end: this is an editable draft, rejected on Apply until fixed.
pub fn duplicate_at(
    script: &Script,
    scene_index: usize,
    event_index: usize,
    at: f64,
) -> Result<Event, String> {
    let at = snapped_time(script, scene_index, at)?;
    let mut event = script.scenes[scene_index]
        .events
        .get(event_index)
        .ok_or("Select an existing event")?
        .clone();
    match &mut event {
        Event::Move { at: start, .. }
        | Event::Emotion { at: start, .. }
        | Event::Gesture { at: start, .. }
        | Event::Speak { at: start, .. } => *start = at,
    }
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script() -> Script {
        motion_script::parse(include_str!("../../../examples/dialogue.yaml")).unwrap()
    }

    #[test]
    fn creates_bounded_defaults_without_touching_source() {
        let script = script();
        let before = format!("{script:?}");
        for kind in [Kind::Move, Kind::Emotion, Kind::Gesture, Kind::Speak] {
            let event = create(&script, 0, "EMMA", 100.0, kind).unwrap();
            assert_eq!(event.actor_time(), (287.0 / 24.0, "EMMA"));
            match event {
                Event::Move { duration, x, y, .. } => {
                    assert!(duration > 0.0 && duration <= 1.0);
                    assert!(287.0 / 24.0 + duration <= 12.0);
                    assert_eq!((x, y), (960.0, 756.0));
                }
                Event::Gesture {
                    duration, value, ..
                } => {
                    assert!(duration > 0.0 && duration <= 1.0);
                    assert_eq!(value, "wave");
                }
                Event::Emotion { value, .. } => assert_eq!(value, "neutral"),
                Event::Speak { text, audio, .. } => {
                    assert_eq!(text, "New dialogue.");
                    assert!(audio.is_none());
                }
            }
        }
        assert_eq!(format!("{script:?}"), before);
        assert_eq!(
            create(&script, 0, "EMMA", -2.0, Kind::Speak)
                .unwrap()
                .actor_time()
                .0,
            0.0
        );
        assert_eq!(
            create(&script, 0, "EMMA", 0.07, Kind::Speak)
                .unwrap()
                .actor_time()
                .0,
            2.0 / 24.0
        );
    }

    #[test]
    fn validates_draft_inputs() {
        let script = script();
        assert!(create(&script, 0, "MISSING", 0.0, Kind::Speak).is_err());
        assert!(create(&script, 8, "EMMA", 0.0, Kind::Speak).is_err());
        for at in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(create(&script, 0, "EMMA", at, Kind::Speak).is_err());
            assert!(duplicate_at(&script, 0, 0, at).is_err());
        }
        assert!(duplicate_at(&script, 0, 100, 1.0).is_err());
    }

    #[test]
    fn fractional_and_subframe_scenes_keep_positive_durations() {
        let mut script = script();
        script.scenes[0].events.clear();
        for end in [1.01, 0.001, 1.0 / 24.0] {
            script.scenes[0].duration = end;
            match create(&script, 0, "EMMA", end, Kind::Gesture).unwrap() {
                Event::Gesture { at, duration, .. } => {
                    assert!(at < end);
                    assert!(duration > 0.0);
                    assert!(at + duration <= end);
                }
                _ => panic!("expected gesture"),
            }
        }
        script.project.fps = 0;
        assert!(create(&script, 0, "EMMA", 0.0, Kind::Speak).is_err());
    }

    #[test]
    fn duplication_preserves_all_fields_and_source() {
        let mut script = script();
        if let Event::Speak { audio, .. } = &mut script.scenes[0].events[0] {
            *audio = Some("audio/original.wav".into());
        }
        let before = format!("{:?}", script.scenes[0].events[0]);
        let event = duplicate_at(&script, 0, 0, 3.0).unwrap();
        match event {
            Event::Speak {
                at,
                actor,
                text,
                emotion,
                audio,
            } => {
                assert_eq!(at, 3.0);
                assert_eq!(actor, "EMMA");
                assert_eq!(text, "Hello Alex! Welcome to Kilsat Studio.");
                assert_eq!(emotion.as_deref(), Some("happy"));
                assert_eq!(audio.as_deref(), Some("audio/original.wav"));
            }
            _ => panic!("expected speech"),
        }
        assert_eq!(format!("{:?}", script.scenes[0].events[0]), before);
    }

    #[test]
    fn move_uses_evaluated_pose_and_duplicate_keeps_duration() {
        let mut script = script();
        script.scenes[0].events.push(Event::Move {
            at: 0.0,
            actor: "EMMA".into(),
            x: 100.0,
            y: 200.0,
            duration: 2.0,
        });
        let event = create(&script, 0, "EMMA", 1.0, Kind::Move).unwrap();
        match event {
            Event::Move { x, y, duration, .. } => assert_eq!((x, y, duration), (530.0, 478.0, 1.0)),
            _ => panic!("expected move"),
        }
        match duplicate_at(&script, 0, 2, 12.0).unwrap() {
            Event::Move { at, duration, .. } => {
                assert_eq!(duration, 2.0);
                assert!(at + duration > 12.0);
            }
            _ => panic!("expected move"),
        }
    }
}
