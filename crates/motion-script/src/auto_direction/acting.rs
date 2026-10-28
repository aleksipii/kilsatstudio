//! Conservative acting proposals, limited to explicit emotion and supported gaze.
//! Plain dialogue has no action cue: it does not justify invented waves, listener
//! emotions or acoustic conditioning. Gesture interpretation remains explicit work.
use super::{TimedLine, MAX_LINES, SAMPLE_RATE};
use crate::{Event, GazeKey, Script, ShotList};
use std::collections::BTreeSet;

/// Add acting on a detached candidate, preserving authored speech and cameras.
/// Only eye-equipped custom rigs receive gaze keys; procedural actors do not
/// currently expose a corresponding gaze track in MotionScript.
pub(super) fn apply_acting(
    script: &Script,
    lines: &[TimedLine],
    shots: &mut ShotList,
) -> Result<(), String> {
    if lines.is_empty() || lines.len() > MAX_LINES || shots.shots.len() != lines.len() {
        return Err("Acting requires one shot per line and 1–16 lines.".into());
    }
    let participants: BTreeSet<_> = lines.iter().map(|line| line.line.actor.as_str()).collect();
    if participants.len() > 2 {
        return Err("Acting supports at most two speaking characters.".into());
    }
    for actor in &participants {
        if !script.characters.contains_key(*actor) {
            return Err(format!("Acting references unknown character {actor}."));
        }
    }
    let mut candidate = shots.clone();
    for (timed, shot) in lines.iter().zip(&mut candidate.shots) {
        let line = shot
            .line
            .as_ref()
            .ok_or("Acting shot has no dialogue line.")?;
        if line.actor != timed.line.actor || line.text != timed.line.text {
            return Err(format!(
                "Acting shot {} does not match its source line.",
                shot.id
            ));
        }
        let duration = timed.duration_samples as f64 / SAMPLE_RATE as f64;
        if timed.duration_samples == 0
            || timed.duration_samples > super::MAX_DURATION_SAMPLES
            || !line.duration.is_finite()
            || (line.duration - duration).abs() > 0.5 / SAMPLE_RATE as f64
        {
            return Err(format!(
                "Acting shot {} has inconsistent speech timing.",
                shot.id
            ));
        }
        let value = timed.line.emotion.as_deref().unwrap_or("neutral");
        crate::validate_emotion(value).map_err(|error| error.to_string())?;
        let emotion = Event::Emotion {
            at: 0.0,
            actor: timed.line.actor.clone(),
            value: value.into(),
        };
        // Do not replace or compete with an existing authored initial expression.
        if let Some(existing) = shot.events.iter().find(|event| {
            matches!(event, Event::Emotion { at, actor, .. }
                if *at == 0.0 && actor == &timed.line.actor)
        }) {
            if existing != &emotion {
                return Err(format!(
                    "Acting shot {} has a conflicting initial emotion.",
                    shot.id
                ));
            }
        } else {
            shot.events.push(emotion);
        }
        // Mutual attention is stage blocking, not an inferred emotional reaction.
        // Existing gaze decisions and solo actors remain untouched.
        if participants.len() == 2 {
            for actor in &participants {
                let has_eyes = script.characters[*actor]
                    .rig
                    .as_ref()
                    .is_some_and(|rig| rig.eyes.is_some());
                let visible = shot
                    .composition
                    .layers
                    .get(*actor)
                    .is_none_or(|layer| layer.visible);
                if !has_eyes || !visible || shot.composition.gaze_tracks.contains_key(*actor) {
                    continue;
                }
                if let Some(target) = participants.iter().find(|target| *target != actor) {
                    let target_visible = shot
                        .composition
                        .layers
                        .get(*target)
                        .is_none_or(|layer| layer.visible);
                    if target_visible {
                        shot.composition.gaze_tracks.insert(
                            (*actor).into(),
                            vec![GazeKey {
                                at: 0.0,
                                x: None,
                                y: None,
                                target: Some((*target).into()),
                            }],
                        );
                    }
                }
            }
        }
        if shot.events.len() > crate::MAX_SHOT_EVENTS {
            return Err(format!("Acting shot {} exceeds the event budget.", shot.id));
        }
    }
    *shots = candidate;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_direction::{IntentLine, TimingSource};
    use crate::{ShotLine, ShotSpec};

    fn timed(actor: &str, emotion: Option<&str>) -> TimedLine {
        TimedLine {
            line: IntentLine {
                actor: actor.into(),
                text: "A line.".into(),
                emotion: emotion.map(Into::into),
                source_line: 1,
            },
            start_sample: SAMPLE_RATE,
            duration_samples: 2 * SAMPLE_RATE,
            timing_source: TimingSource::SuppliedSamples,
        }
    }

    fn make_shots(lines: &[TimedLine]) -> ShotList {
        ShotList {
            shots: lines
                .iter()
                .enumerate()
                .map(|(index, timed)| ShotSpec {
                    id: format!("acting_{index}"),
                    actor: Some(timed.line.actor.clone()),
                    line: Some(ShotLine {
                        actor: timed.line.actor.clone(),
                        text: timed.line.text.clone(),
                        emotion: timed.line.emotion.clone(),
                        audio: None,
                        duration: 2.0,
                    }),
                    lead_in: Some(if index == 0 { 1.0 } else { 0.0 }),
                    hold: Some(0.25),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn procedural_actors_get_explicit_or_neutral_emotion_without_invented_actions() {
        let script = crate::parse(include_str!("../../../../examples/hello-emma.yaml")).unwrap();
        let lines = [timed("EMMA", None), timed("ALEX", Some("surprised"))];
        let mut shots = make_shots(&lines);
        let original_lines: Vec<_> = shots.shots.iter().map(|shot| shot.line.clone()).collect();
        apply_acting(&script, &lines, &mut shots).unwrap();
        assert!(
            matches!(&shots.shots[0].events[0], Event::Emotion { value, at, .. } if value == "neutral" && *at == 0.0)
        );
        assert!(
            matches!(&shots.shots[1].events[0], Event::Emotion { value, .. } if value == "surprised")
        );
        for shot in &shots.shots {
            assert!(shot.composition.gaze_tracks.is_empty());
            assert!(shot.composition.camera.is_empty());
            assert!(!shot
                .events
                .iter()
                .any(|event| matches!(event, Event::Speak { .. } | Event::Gesture { .. })));
        }
        assert_eq!(
            original_lines,
            shots
                .shots
                .iter()
                .map(|shot| shot.line.clone())
                .collect::<Vec<_>>()
        );
        crate::expand_shots(&script, &shots).unwrap();
        let once = shots.clone();
        apply_acting(&script, &lines, &mut shots).unwrap();
        assert_eq!(shots, once);
    }

    #[test]
    fn custom_eyes_receive_valid_gaze_without_unsupported_gestures() {
        let script = crate::parse(include_str!("../../../../examples/acting-rig.yaml")).unwrap();
        let lines = [timed("PUPPET", Some("happy")), timed("FRIEND", None)];
        let mut shots = make_shots(&lines);
        // Imported references satisfy source validation; this test reads no WAV.
        for shot in &mut shots.shots {
            shot.line.as_mut().unwrap().audio = Some("fixture.wav".into());
        }
        apply_acting(&script, &lines, &mut shots).unwrap();
        for shot in &shots.shots {
            assert_eq!(
                shot.composition.gaze_tracks["PUPPET"][0].target.as_deref(),
                Some("FRIEND")
            );
            assert!(!shot.composition.gaze_tracks.contains_key("FRIEND"));
            assert!(!shot
                .events
                .iter()
                .any(|event| matches!(event, Event::Gesture { .. })));
        }
        crate::expand_shots(&script, &shots).unwrap();
    }

    #[test]
    fn invalid_later_line_leaves_all_shots_unchanged() {
        let script = crate::parse(include_str!("../../../../examples/hello-emma.yaml")).unwrap();
        let lines = [timed("EMMA", None), timed("ALEX", Some("sarcastic"))];
        let mut shots = make_shots(&lines);
        let original = shots.clone();
        assert!(apply_acting(&script, &lines, &mut shots).is_err());
        assert_eq!(shots, original);
        let lines = [timed("EMMA", None)];
        let mut shots = make_shots(&lines);
        shots.shots[0].line.as_mut().unwrap().duration = f64::NAN;
        assert!(apply_acting(&script, &lines, &mut shots).is_err());
    }
}
