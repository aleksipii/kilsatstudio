//! Optional reaction cuts and bounded acting on the existing shot compiler.
//! Inputs here remain caller-supplied: only the desktop PCM worker verifies WAVs.
use super::{
    acting, director, parser, timing, DirectionResult, TimingSource, MAX_DURATION_SAMPLES,
    MAX_LINES, MAX_SOURCE_BYTES, SAMPLE_RATE,
};
use crate::{ActorEasing, ActorKey, CameraKey, Composition, Event, ReactionShot, Script};
use std::collections::BTreeSet;

const REACTION_SAMPLES: u64 = SAMPLE_RATE / 2;

/// Richer editable draft. Speech durations are estimated unless explicitly supplied.
pub fn direct_conversational(
    source: &str,
    dialogue: &str,
    supplied_samples: Option<&[u64]>,
) -> Result<DirectionResult, String> {
    build(source, dialogue, supplied_samples, None)
}

/// Pure caller-data adapter. The caller must verify the PCM and paths before use.
/// References are inserted before shared semantic validation, allowing imported
/// speech on characters without a synthesis voice. No file is opened here.
pub fn direct_with_audio(
    source: &str,
    dialogue: &str,
    samples: &[u64],
    paths: &[String],
) -> Result<DirectionResult, String> {
    build(source, dialogue, Some(samples), Some(paths))
}

fn build(
    source: &str,
    dialogue: &str,
    supplied: Option<&[u64]>,
    paths: Option<&[String]>,
) -> Result<DirectionResult, String> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err("Screenplay exceeds the 256 KiB source limit.".into());
    }
    let script = crate::parse(source).map_err(|e| e.to_string())?;
    let first_scene = script.scenes.len();
    let lines = parser::parse_dialogue(dialogue, &script)?;
    let timed = timing::time_lines(&lines, supplied)?;
    let mut shots = director::build_shots_with_audio(&script, &timed, paths)?;
    acting::apply_acting(&script, &timed, &mut shots)?;
    let base_samples = timed
        .last()
        .map(|line| line.start_sample + line.duration_samples + super::GAP_SAMPLES)
        .ok_or("No dialogue")?;
    let mut remaining = ((MAX_DURATION_SAMPLES - base_samples) / REACTION_SAMPLES)
        .min(4)
        .min((MAX_LINES - lines.len()) as u64) as usize;
    let participants: BTreeSet<_> = lines.iter().map(|line| line.actor.as_str()).collect();
    let mut reactions = 0;
    for (index, (shot, line)) in shots.shots.iter_mut().zip(&timed).enumerate() {
        let lead = if index == 0 {
            super::LEAD_SAMPLES as f64 / SAMPLE_RATE as f64
        } else {
            0.0
        };
        let seconds = line.duration_samples as f64 / SAMPLE_RATE as f64;
        accent(
            &script,
            &mut shot.composition,
            &line.line.actor,
            lead,
            seconds,
            index,
        )?;
        // Alternating listening beats and final line punctuation are staging,
        // not inferred intent, emotion or an acoustic speech-style control.
        let candidate = index % 2 == 1
            || index + 1 == lines.len()
            || line.line.text.ends_with('?')
            || line.line.text.ends_with('!');
        if remaining > 0 && candidate && participants.len() == 2 {
            let listener = participants
                .iter()
                .find(|actor| **actor != line.line.actor)
                .ok_or("Missing listener")?;
            let mut composition = Composition {
                layers: shot.composition.layers.clone(),
                ..Default::default()
            };
            if script.characters[*listener]
                .rig
                .as_ref()
                .is_some_and(|rig| rig.eyes.is_some())
            {
                composition.gaze_tracks.insert(
                    (*listener).into(),
                    vec![crate::GazeKey {
                        at: 0.0,
                        x: None,
                        y: None,
                        target: Some(line.line.actor.clone()),
                    }],
                );
            }
            accent(&script, &mut composition, listener, 0.0, 0.5, index + 1)?;
            shot.reaction = Some(ReactionShot {
                actor: Some((*listener).into()),
                hold: Some(0.5),
                composition,
                ..Default::default()
            });
            remaining -= 1;
            reactions += 1;
        }
        // Slow framing changes are applied only after geometry-safe fit below.
        shot.push = Some(1.0);
    }
    let mut expansion = crate::expand_shots(&script, &shots).map_err(|e| e.to_string())?;
    if expansion.plan.len() > MAX_LINES {
        return Err("Reaction cuts exceed sixteen final shots".into());
    }
    let mut line_index = 0usize;
    for plan in &expansion.plan {
        if let (Some(start), Some(end)) = (plan.dialogue_start, plan.dialogue_end) {
            let line = &timed[line_index];
            if (((end - start) * SAMPLE_RATE as f64) - line.duration_samples as f64).abs() > 1.0 {
                return Err("Shot compiler changed actual speech duration".into());
            }
            let scene = &expansion.script.scenes[plan.scene_index];
            let spoken: Vec<_> = scene
                .events
                .iter()
                .filter_map(|event| {
                    if let Event::Speak { actor, text, .. } = event {
                        Some((actor, text))
                    } else {
                        None
                    }
                })
                .collect();
            if spoken != vec![(&line.line.actor, &line.line.text)] {
                return Err("Shot compiler changed dialogue order/text".into());
            }
            line_index += 1;
        }
    }
    if line_index != lines.len() {
        return Err("A dialogue line disappeared".into());
    }
    // Explicit target list follows the actual expansion order, including reactions.
    let targets: Vec<_> = shots
        .shots
        .iter()
        .flat_map(|shot| {
            let mut targets = vec![shot.actor.clone()];
            if let Some(reaction) = &shot.reaction {
                targets.push(reaction.actor.clone());
            }
            targets
        })
        .collect();
    for (offset, target) in targets.iter().enumerate() {
        fit_camera(
            &script,
            &mut expansion.script.scenes[first_scene + offset],
            target.as_deref(),
            offset,
        )?;
    }
    expansion.script.validate().map_err(|e| e.to_string())?;
    let duration_samples =
        expansion.script.scenes[first_scene..]
            .iter()
            .try_fold(0u64, |n, scene| {
                n.checked_add((scene.duration * SAMPLE_RATE as f64).round() as u64)
                    .ok_or("Duration overflow")
            })?;
    if duration_samples > MAX_DURATION_SAMPLES {
        return Err("Directed sequence exceeds60seconds; speech was not truncated".into());
    }
    let output = crate::to_source(&expansion.script).map_err(|e| e.to_string())?;
    if output.len() > MAX_SOURCE_BYTES {
        return Err("Directed screenplay exceeds the 256 KiB source limit.".into());
    }
    Ok(DirectionResult {source:output,first_scene,shot_count:targets.len(),duration_samples,
        timing_source:if supplied.is_some(){TimingSource::SuppliedSamples}else{TimingSource::Estimated},
        diagnostics:vec![
            if supplied.is_some(){"Timing uses caller-supplied 24kHz sample counts; this pure adapter has not verified PCM or audio rights.".into()}else{"Speech timing is text-estimated, not measured audio.".into()},
            format!("Added {reactions} optional half-second listening reactions, bounded head accents and geometry-aware camera fit. All original speech is retained. Review acting, framing and comic timing."),
            "No automatic action/intent recognition, accurate lip-sync, professional artistic acceptance or five-minute production guarantee is claimed.".into(),
        ]})
}

fn accent(
    script: &Script,
    composition: &mut Composition,
    actor: &str,
    lead: f64,
    seconds: f64,
    index: usize,
) -> Result<(), String> {
    let character = script
        .characters
        .get(actor)
        .ok_or("Unknown acting target")?;
    let end = lead + seconds.min(0.8);
    let midpoint = lead + (end - lead) * 0.5;
    if !(end > midpoint && midpoint > lead) {
        return Ok(());
    }
    let sign = if index.is_multiple_of(2) { 1.0 } else { -1.0 };
    if let Some(rig) = &character.rig {
        if let Some(head) = rig
            .joints
            .iter()
            .find(|joint| joint.id.eq_ignore_ascii_case("head"))
        {
            let keys = vec![
                ActorKey {
                    at: lead,
                    ..Default::default()
                },
                ActorKey {
                    at: midpoint,
                    rotation: sign * 0.035,
                    easing: ActorEasing::Smoothstep,
                    ..Default::default()
                },
                ActorKey {
                    at: end,
                    ..Default::default()
                },
            ];
            composition
                .joint_tracks
                .entry(actor.into())
                .or_default()
                .entry(head.id.clone())
                .or_insert(keys);
        }
    } else {
        composition
            .bone_tracks
            .entry(actor.into())
            .or_default()
            .entry("head".into())
            .or_insert_with(|| {
                vec![
                    ActorKey {
                        at: lead,
                        ..Default::default()
                    },
                    ActorKey {
                        at: midpoint,
                        y: 2.0,
                        easing: ActorEasing::Smoothstep,
                        ..Default::default()
                    },
                    ActorKey {
                        at: end,
                        ..Default::default()
                    },
                ]
            });
    }
    Ok(())
}

fn fit_camera(
    script: &Script,
    scene: &mut crate::Scene,
    target: Option<&str>,
    index: usize,
) -> Result<(), String> {
    let width = f64::from(script.project.width);
    let height = f64::from(script.project.height);
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    let mut work = 0usize;
    for (id, character) in &script.characters {
        if target.is_some_and(|target| target != id)
            || !scene
                .composition
                .layers
                .get(id)
                .is_none_or(|layer| layer.visible)
        {
            continue;
        }
        let root = scene
            .composition
            .actor_tracks
            .get(id)
            .and_then(|keys| keys.first())
            .cloned()
            .unwrap_or_default();
        let mut add = |x: f64, y: f64, margin: f64| -> Result<(), String> {
            work += 1;
            if work > 200_000 {
                return Err("Camera geometry work budget exceeded".into());
            }
            let (sin, cos) = root.rotation.sin_cos();
            let x0 = x * root.scale_x;
            let y0 = y * root.scale_y;
            let world_x = width / 2.0 + root.x + x0 * cos - y0 * sin;
            let world_y = height * 0.7 + root.y + x0 * sin + y0 * cos;
            if !world_x.is_finite() || !world_y.is_finite() {
                return Err("Non-finite rig geometry".into());
            }
            bounds[0] = bounds[0].min(world_x - margin);
            bounds[1] = bounds[1].min(world_y - margin);
            bounds[2] = bounds[2].max(world_x + margin);
            bounds[3] = bounds[3].max(world_y + margin);
            Ok(())
        };
        if let Some(definition) = &character.rig {
            let rig = definition.prepare().map_err(|e| e.to_string())?;
            let mut times = vec![0.0, scene.duration];
            if let Some(tracks) = scene.composition.joint_tracks.get(id) {
                for keys in tracks.values() {
                    times.extend(keys.iter().map(|key| key.at));
                }
            }
            times.sort_by(f64::total_cmp);
            times.dedup();
            for time in times {
                let mut offsets = Vec::new();
                if let Some(tracks) = scene.composition.joint_tracks.get(id) {
                    for (joint, keys) in tracks {
                        let track = motion_animation::Track::new(
                            keys.iter().map(ActorKey::animation_key).collect(),
                        )
                        .map_err(|e| e.to_string())?;
                        let value = track.sample(time).map_err(|e| e.to_string())?;
                        offsets.push((rig.joint_index(joint).ok_or("Missing joint")?, value));
                    }
                }
                let emotion = scene
                    .events
                    .iter()
                    .find_map(|event| {
                        if let Event::Emotion { actor, value, .. } = event {
                            (actor == id).then_some(value.as_str())
                        } else {
                            None
                        }
                    })
                    .unwrap_or("neutral");
                for blink in [false, true] {
                    let (mut parts, mouth) = rig
                        .evaluate_acting(
                            &offsets,
                            motion_rig::ActingInput {
                                emotion,
                                blink,
                                view: motion_rig::FaceView::Front,
                            },
                        )
                        .map_err(|e| e.to_string())?;
                    if let Some(mouth) = mouth {
                        parts.extend(mouth.shapes.into_iter().map(|(_, part)| part));
                    }
                    for part in parts {
                        let margin = 12.0
                            + definition
                                .eyes
                                .as_ref()
                                .map_or(0.0, |eyes| eyes.travel.abs())
                                * root.scale_x.max(root.scale_y)
                            + part
                                .outline
                                .map_or(0.0, |o| o.width * root.scale_x.max(root.scale_y));
                        for point in part.points {
                            add(point.x, point.y, margin)?;
                        }
                    }
                }
            }
        } else {
            for (x, y) in [(-90.0, -310.0), (90.0, -310.0), (-90.0, 15.0), (90.0, 15.0)] {
                add(x, y, 12.0)?;
            }
        }
    }
    if bounds.iter().any(|v| !v.is_finite()) {
        return Err("Cannot frame an empty stage".into());
    }
    let safe = (width * 0.82 / (bounds[2] - bounds[0]).max(1.0))
        .min(height * 0.82 / (bounds[3] - bounds[1]).max(1.0))
        .min(2.4);
    if !safe.is_finite() || safe < 0.05 {
        return Err(
            "Rig is too large to frame safely; enlarge the canvas or resize the rig".into(),
        );
    }
    let zoom = safe.min(if target.is_none() {
        1.0
    } else {
        1.5 + (index % 3) as f64 * 0.25
    });
    let push = if scene.duration > 1.0 && target.is_some() {
        1.025
    } else {
        1.0
    };
    let x = (bounds[0] + bounds[2]) / 2.0 - width / 2.0;
    let y = (bounds[1] + bounds[3]) / 2.0 - height / 2.0;
    scene.composition.camera = vec![CameraKey {
        at: 0.0,
        x,
        y,
        zoom: zoom / push,
        angle: None,
    }];
    if push > 1.0 {
        scene.composition.camera.push(CameraKey {
            at: scene.duration,
            x,
            y,
            zoom,
            angle: None,
        });
    }
    Ok(())
}
