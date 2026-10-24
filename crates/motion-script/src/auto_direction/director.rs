//! Conservative shot selection on the existing shot-expansion path.
use super::{TimedLine, GAP_SAMPLES, LEAD_SAMPLES, MAX_DURATION_SAMPLES, MAX_LINES, SAMPLE_RATE};
use crate::{Composition, CutRules, Layer, Script, ShotLine, ShotList, ShotPreset, ShotSpec};
use std::collections::{BTreeMap, BTreeSet};

/// Make a stable stage and a bounded, editable shot for each scheduled line.
/// Framing uses proportional upper-body targets, not inferred custom-rig bounds.
pub(super) fn build_shots(script: &Script, lines: &[TimedLine]) -> Result<ShotList, String> {
    build_shots_with_audio(script, lines, None)
}

pub(super) fn build_shots_with_audio(
    script: &Script,
    lines: &[TimedLine],
    audio: Option<&[String]>,
) -> Result<ShotList, String> {
    if audio.is_some_and(|paths| {
        paths.len() != lines.len()
            || paths
                .iter()
                .any(|p| p.trim().is_empty() || p.len() > 4096 || p.contains('\0'))
    }) {
        return Err("Supply one bounded nonempty audio reference per dialogue line".into());
    }
    script.validate().map_err(|error| error.to_string())?;
    if lines.is_empty() || lines.len() > MAX_LINES {
        return Err(format!("Direction needs 1–{MAX_LINES} dialogue lines."));
    }
    let mut speakers = BTreeSet::new();
    let mut expected_start = LEAD_SAMPLES;
    for line in lines {
        if !script.characters.contains_key(&line.line.actor) {
            return Err(format!(
                "Line {}: unknown actor {}",
                line.line.source_line, line.line.actor
            ));
        }
        if line.line.text.trim().is_empty() || line.duration_samples == 0 {
            return Err(format!(
                "Line {}: dialogue and duration must be nonempty",
                line.line.source_line
            ));
        }
        if line.start_sample != expected_start {
            return Err(format!(
                "Line {}: schedule does not match the shared sample clock",
                line.line.source_line
            ));
        }
        expected_start = expected_start
            .checked_add(line.duration_samples)
            .and_then(|sample| sample.checked_add(GAP_SAMPLES))
            .ok_or("Dialogue sample schedule overflow")?;
        if expected_start > MAX_DURATION_SAMPLES {
            return Err(
                "Directed dialogue exceeds the 60-second profile; speech is not truncated.".into(),
            );
        }
        speakers.insert(line.line.actor.clone());
    }
    if speakers.len() > 2 {
        return Err("Automatic direction supports at most two speaking characters.".into());
    }
    let (width, height) = (
        f64::from(script.project.width),
        f64::from(script.project.height),
    );
    let place: BTreeMap<String, [f64; 2]> = speakers
        .iter()
        .enumerate()
        .map(|(index, actor)| {
            let fraction = if speakers.len() == 1 {
                0.5
            } else if index == 0 {
                0.32
            } else {
                0.68
            };
            (actor.clone(), [width * fraction, height * 0.70])
        })
        .collect();
    let composition = Composition {
        layers: script
            .characters
            .keys()
            .map(|actor| {
                (
                    actor.clone(),
                    Layer {
                        z: 0,
                        visible: speakers.contains(actor),
                    },
                )
            })
            .collect(),
        ..Default::default()
    };
    let existing_ids: BTreeSet<_> = script
        .scenes
        .iter()
        .map(|scene| scene.id.as_str())
        .collect();
    let mut group = script
        .scenes
        .len()
        .checked_add(1)
        .ok_or("Scene identifier overflow")?;
    while (1..=lines.len())
        .any(|index| existing_ids.contains(format!("auto_{group}_{index}").as_str()))
    {
        group = group.checked_add(1).ok_or("Scene identifier overflow")?;
    }
    let shots = lines
        .iter()
        .enumerate()
        .map(|(index, timed)| {
            let preset = if index == 0 {
                ShotPreset::Establishing
            } else if index + 1 == lines.len() {
                ShotPreset::CloseUp
            } else {
                ShotPreset::Medium
            };
            let establishing = preset == ShotPreset::Establishing;
            ShotSpec {
                id: format!("auto_{group}_{}", index + 1),
                preset,
                actor: (!establishing).then(|| timed.line.actor.clone()),
                point: establishing.then_some([width * 0.5, height * 0.5]),
                offset: if establishing {
                    [0.0, 0.0]
                } else {
                    [0.0, -height * 0.16]
                },
                // Static interior shots retain framing across a continuing speaker.
                // An intentional final close-up is the only automatic emphasis.
                push: Some(1.0),
                line: Some(ShotLine {
                    actor: timed.line.actor.clone(),
                    text: timed.line.text.clone(),
                    emotion: timed.line.emotion.clone(),
                    audio: audio.map(|paths| paths[index].clone()),
                    duration: timed.duration_samples as f64 / SAMPLE_RATE as f64,
                }),
                lead_in: Some(if index == 0 {
                    LEAD_SAMPLES as f64 / SAMPLE_RATE as f64
                } else {
                    0.0
                }),
                hold: Some(GAP_SAMPLES as f64 / SAMPLE_RATE as f64),
                place: place.clone(),
                composition: composition.clone(),
                ..Default::default()
            }
        })
        .collect();
    let result = ShotList {
        cut: CutRules {
            lead_in: 0.0,
            tail: GAP_SAMPLES as f64 / SAMPLE_RATE as f64,
            reaction_hold: 0.5,
        },
        shots,
    };
    // Exercise the same semantic gate used by preview/export before handing the
    // plan to acting. The orchestrator validates again after acting additions.
    crate::expand_shots(script, &result).map_err(|error| error.to_string())?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_direction::{IntentLine, TimingSource};

    fn script() -> Script {
        crate::parse(include_str!("../../../../examples/hello-emma.yaml")).unwrap()
    }

    fn lines(actors: &[&str]) -> Vec<TimedLine> {
        actors
            .iter()
            .enumerate()
            .map(|(index, actor)| TimedLine {
                line: IntentLine {
                    actor: (*actor).into(),
                    text: format!("Line {index}"),
                    emotion: None,
                    source_line: index + 1,
                },
                start_sample: LEAD_SAMPLES + index as u64 * (SAMPLE_RATE + GAP_SAMPLES),
                duration_samples: SAMPLE_RATE,
                timing_source: TimingSource::SuppliedSamples,
            })
            .collect()
    }

    #[test]
    fn stable_stage_axis_framing_and_authored_scenes_survive_expansion() {
        let mut script = script();
        script
            .characters
            .insert("UNUSED".into(), script.characters["EMMA"].clone());
        let before = crate::to_source(&script).unwrap();
        let list = build_shots(&script, &lines(&["EMMA", "ALEX", "EMMA", "EMMA"])).unwrap();
        assert_eq!(list.shots[0].preset, ShotPreset::Establishing);
        assert_eq!(list.shots[1].preset, ShotPreset::Medium);
        assert_eq!(list.shots[2].preset, ShotPreset::Medium);
        assert_eq!(list.shots[3].preset, ShotPreset::CloseUp);
        for shot in &list.shots {
            assert!(shot.place["ALEX"][0] < shot.place["EMMA"][0]);
            assert_eq!(shot.place, list.shots[0].place);
            assert!(!shot.composition.layers["UNUSED"].visible);
            assert!(shot.composition.layers["EMMA"].visible);
            assert_eq!(shot.push, Some(1.0));
        }
        let expanded = crate::expand_shots(&script, &list).unwrap();
        assert_eq!(expanded.script.scenes.len(), script.scenes.len() + 4);
        assert_eq!(expanded.script.scenes[0].id, script.scenes[0].id);
        assert_eq!(crate::to_source(&script).unwrap(), before);
        assert_eq!(
            list,
            build_shots(&script, &lines(&["EMMA", "ALEX", "EMMA", "EMMA"])).unwrap()
        );
    }

    #[test]
    fn unique_ids_and_single_actor_centred_with_valid_cameras_at_various_sizes() {
        for (width, height) in [(320, 180), (1080, 1920), (1920, 1080), (4096, 2160)] {
            let mut script = script();
            script.project.width = width;
            script.project.height = height;
            script.scenes[0].id = "auto_2_1".into();
            script.scenes[0].events.clear();
            let list = build_shots(&script, &lines(&["EMMA", "EMMA", "EMMA"])).unwrap();
            assert_eq!(list.shots[0].id, "auto_3_1");
            let expanded = crate::expand_shots(&script, &list).unwrap();
            for scene in expanded.script.scenes.iter().skip(1) {
                assert_eq!(scene.composition.camera.len(), 1);
                let camera = &scene.composition.camera[0];
                assert!(camera.x.is_finite() && camera.y.is_finite());
                assert!((1.0..=2.4).contains(&camera.zoom));
            }
            assert_eq!(list.shots[0].place["EMMA"][0], f64::from(width) * 0.5);
            assert!(!list.shots[0].composition.layers["ALEX"].visible);
            // Proportional upper-body target is centred; the actor origin stays
            // on-screen even in the final tight framing.
            let shot = list.shots.last().unwrap();
            let camera = &expanded.script.scenes.last().unwrap().composition.camera[0];
            let origin_y = (shot.place["EMMA"][1] - f64::from(height) * 0.5 - camera.y)
                * camera.zoom
                + f64::from(height) * 0.5;
            assert!((0.0..=f64::from(height)).contains(&origin_y));
        }
    }

    #[test]
    fn rejects_unknown_actors_bad_timing_and_excess_work() {
        let mut script = script();
        assert!(build_shots(&script, &[]).is_err());
        assert!(build_shots(&script, &lines(&["UNKNOWN"])).is_err());
        assert!(build_shots(&script, &lines(&["EMMA"; MAX_LINES + 1])).is_err());
        let mut bad = lines(&["EMMA"]);
        bad[0].start_sample = 0;
        assert!(build_shots(&script, &bad).is_err());
        bad[0].start_sample = LEAD_SAMPLES;
        bad[0].duration_samples = u64::MAX;
        assert!(build_shots(&script, &bad).is_err());
        script
            .characters
            .insert("THIRD".into(), script.characters["EMMA"].clone());
        assert!(build_shots(&script, &lines(&["EMMA", "ALEX", "THIRD"])).is_err());
    }
}
