//! Structural acceptance only: these tests do not establish visual direction,
//! speech synthesis, lip-sync quality or physical Mac acceptance.
use motion_script::auto_direction::{
    direct, TimingSource, GAP_SAMPLES, LEAD_SAMPLES, MAX_DURATION_SAMPLES, MAX_LINES, SAMPLE_RATE,
};
use motion_script::{Event, Script};

const SOURCE: &str = include_str!("../../../examples/hello-emma.yaml");

fn added_speech(script: &Script, first_scene: usize) -> Vec<(String, String)> {
    script.scenes[first_scene..]
        .iter()
        .flat_map(|scene| scene.events.iter())
        .filter_map(|event| match event {
            Event::Speak { actor, text, .. } => Some((actor.clone(), text.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn preserves_authored_scene_and_exact_finnish_english_dialogue() {
    let input = "EMMA: Hyvää huomenta: ääkköset säilyvät!\nALEX: Good morning — yes, exactly.";
    let result = direct(SOURCE, input, Some(&[12 * SAMPLE_RATE, 18 * SAMPLE_RATE])).unwrap();
    let original = motion_script::parse(SOURCE).unwrap();
    let generated = motion_script::parse(&result.source).unwrap();
    assert_eq!(result.first_scene, original.scenes.len());
    assert_eq!(
        result.shot_count,
        generated.scenes.len() - result.first_scene
    );
    for (before, after) in original.scenes.iter().zip(&generated.scenes) {
        assert_eq!(
            serde_yaml::to_string(before).unwrap(),
            serde_yaml::to_string(after).unwrap()
        );
    }
    assert_eq!(
        serde_yaml::to_string(&original.characters).unwrap(),
        serde_yaml::to_string(&generated.characters).unwrap()
    );
    assert_eq!(
        added_speech(&generated, result.first_scene),
        vec![
            ("EMMA".into(), "Hyvää huomenta: ääkköset säilyvät!".into()),
            ("ALEX".into(), "Good morning — yes, exactly.".into()),
        ]
    );
    assert_eq!(
        result.duration_samples,
        LEAD_SAMPLES + 30 * SAMPLE_RATE + 2 * GAP_SAMPLES
    );
    assert_eq!(result.timing_source, TimingSource::SuppliedSamples);
    for scene in &generated.scenes[result.first_scene..] {
        assert!(scene.duration.is_finite() && scene.duration > 0.0);
        assert!((scene.duration * f64::from(generated.project.fps)).ceil() >= 1.0);
        assert!(!scene.composition.camera.is_empty());
        assert!(scene
            .composition
            .camera
            .iter()
            .all(|key| key.at >= 0.0 && key.at <= scene.duration));
        for event in &scene.events {
            if let Event::Speak { audio, .. } = event {
                assert!(
                    audio.is_none(),
                    "the director must not invent an audio file"
                );
            }
        }
    }
}

#[test]
fn identical_inputs_are_deterministic_and_timing_provenance_is_distinct() {
    let dialogue = "EMMA: Shall we begin?\nALEX: Yes, let us begin.";
    let estimated = direct(SOURCE, dialogue, None).unwrap();
    let repeated = direct(SOURCE, dialogue, None).unwrap();
    assert_eq!(estimated.source, repeated.source);
    assert_eq!(estimated.duration_samples, repeated.duration_samples);
    assert_eq!(estimated.diagnostics, repeated.diagnostics);
    assert_eq!(estimated.timing_source, TimingSource::Estimated);
    assert!(
        !estimated.diagnostics.is_empty(),
        "draft timing needs a visible diagnostic"
    );
    let supplied = direct(SOURCE, dialogue, Some(&[8 * SAMPLE_RATE, 9 * SAMPLE_RATE])).unwrap();
    assert_eq!(supplied.timing_source, TimingSource::SuppliedSamples);
    assert_ne!(estimated.duration_samples, supplied.duration_samples);
    assert_eq!(
        added_speech(
            &motion_script::parse(&estimated.source).unwrap(),
            estimated.first_scene
        ),
        added_speech(
            &motion_script::parse(&supplied.source).unwrap(),
            supplied.first_scene
        )
    );
}

#[test]
fn sixty_second_budget_includes_lead_and_final_gap_without_cutting_speech() {
    let speech = MAX_DURATION_SAMPLES - LEAD_SAMPLES - GAP_SAMPLES;
    let at_limit = direct(SOURCE, "EMMA: A complete supplied line.", Some(&[speech])).unwrap();
    assert_eq!(at_limit.duration_samples, MAX_DURATION_SAMPLES);
    let generated = motion_script::parse(&at_limit.source).unwrap();
    let added_seconds: f64 = generated.scenes[at_limit.first_scene..]
        .iter()
        .map(|s| s.duration)
        .sum();
    assert!((added_seconds * SAMPLE_RATE as f64 - MAX_DURATION_SAMPLES as f64).abs() <= 1.0);
    assert!(direct(
        SOURCE,
        "EMMA: A complete supplied line.",
        Some(&[speech + 1])
    )
    .is_err());
    assert!(direct(SOURCE, "EMMA: No integer overflow.", Some(&[u64::MAX])).is_err());
    assert!(direct(SOURCE, "EMMA: No zero-length speech.", Some(&[0])).is_err());
}

#[test]
fn rejects_a_third_known_speaker_and_malformed_timing_without_partial_output() {
    let mut base = motion_script::parse(SOURCE).unwrap();
    base.characters
        .insert("THIRD".into(), base.characters["ALEX"].clone());
    let source = motion_script::to_source(&base).unwrap();
    assert!(direct(&source, "EMMA: One.\nALEX: Two.\nTHIRD: Three.", None).is_err());
    assert!(direct(SOURCE, "EMMA: One.\nALEX: Two.", Some(&[SAMPLE_RATE])).is_err());
    assert!(direct(SOURCE, "MISSING: One.", None).is_err());
    assert!(direct(SOURCE, "", None).is_err());
    assert!(direct(SOURCE, &"EMMA: One.\n".repeat(MAX_LINES + 1), None).is_err());
    // Failed pure-function calls cannot mutate the supplied screenplay.
    assert_eq!(source, motion_script::to_source(&base).unwrap());
}

#[test]
fn conversational_reactions_keep_all_speech_and_leave_original_scenes_unchanged() {
    let result = motion_script::auto_direction::direct_conversational(
        SOURCE,
        "EMMA[happy]: Hei!\nALEX: Terve!\nEMMA: Aloitetaan.",
        Some(&[24000, 36000, 48000]),
    )
    .unwrap();
    let script = motion_script::parse(&result.source).unwrap();
    assert!(result.shot_count > 3 && result.shot_count <= 7);
    assert_eq!(added_speech(&script, result.first_scene).len(), 3);
    let original = motion_script::parse(SOURCE).unwrap();
    assert_eq!(
        serde_yaml::to_string(&script.scenes[..result.first_scene]).unwrap(),
        serde_yaml::to_string(&original.scenes).unwrap()
    );
    assert!(script.scenes[result.first_scene..].iter().any(|scene| scene
        .id
        .ends_with("_reaction")
        && scene
            .events
            .iter()
            .all(|event| !matches!(event, Event::Speak { .. }))));
    let repeated = motion_script::auto_direction::direct_conversational(
        SOURCE,
        "EMMA[happy]: Hei!\nALEX: Terve!\nEMMA: Aloitetaan.",
        Some(&[24000, 36000, 48000]),
    )
    .unwrap();
    assert_eq!(result.source, repeated.source);
}

#[test]
fn actual_audio_references_allow_voiceless_imports_and_do_not_claim_pcm_verification() {
    let mut base = motion_script::parse(SOURCE).unwrap();
    for character in base.characters.values_mut() {
        character.voice = None;
    }
    for scene in &mut base.scenes {
        scene
            .events
            .retain(|event| !matches!(event, Event::Speak { .. }));
    }
    let source = motion_script::to_source(&base).unwrap();
    let result = motion_script::auto_direction::direct_with_audio(
        &source,
        "EMMA: Hei.\nALEX: Terve.",
        &[24000, 24000],
        &["audio/one.wav".into(), "audio/two.wav".into()],
    )
    .unwrap();
    let script = motion_script::parse(&result.source).unwrap();
    let refs: Vec<_> = script.scenes[result.first_scene..]
        .iter()
        .flat_map(|s| &s.events)
        .filter_map(|event| {
            if let Event::Speak { audio, .. } = event {
                audio.as_deref()
            } else {
                None
            }
        })
        .collect();
    assert_eq!(refs, vec!["audio/one.wav", "audio/two.wav"]);
    assert!(result
        .diagnostics
        .iter()
        .any(|text| text.contains("not verified")));
    assert!(motion_script::auto_direction::direct_with_audio(
        &source,
        "EMMA: Hei.",
        &[24000],
        &[String::new()]
    )
    .is_err());
}

#[test]
fn optional_reactions_never_truncate_full_speech_or_exceed_final_shot_budget() {
    let result = motion_script::auto_direction::direct_conversational(
        SOURCE,
        "EMMA: First.\nALEX: Second.",
        Some(&[29 * SAMPLE_RATE, 29 * SAMPLE_RATE + SAMPLE_RATE / 2]),
    )
    .unwrap();
    assert_eq!(result.duration_samples, MAX_DURATION_SAMPLES);
    assert_eq!(result.shot_count, 2);
    let mut dialogue = String::new();
    for index in 0..MAX_LINES {
        dialogue.push_str(if index % 2 == 0 {
            "EMMA: Hei.\n"
        } else {
            "ALEX: Terve.\n"
        });
    }
    let result = motion_script::auto_direction::direct_conversational(
        SOURCE,
        &dialogue,
        Some(&[SAMPLE_RATE / 2; MAX_LINES]),
    )
    .unwrap();
    assert_eq!(result.shot_count, MAX_LINES);
    assert_eq!(
        added_speech(
            &motion_script::parse(&result.source).unwrap(),
            result.first_scene
        )
        .len(),
        MAX_LINES
    );
}

#[test]
fn custom_rigs_get_head_only_accents_and_finite_geometry_aware_cameras_on_small_canvas() {
    let mut base = motion_script::parse(include_str!("../../../examples/acting-rig.yaml")).unwrap();
    base.project.width = 320;
    base.project.height = 180;
    let source = motion_script::to_source(&base).unwrap();
    let result = motion_script::auto_direction::direct_with_audio(
        &source,
        "PUPPET: Hei!\nFRIEND: Terve!",
        &[24000, 24000],
        &["one.wav".into(), "two.wav".into()],
    )
    .unwrap();
    let script = motion_script::parse(&result.source).unwrap();
    assert!(script.scenes[result.first_scene..].iter().all(|scene| scene
        .composition
        .camera
        .iter()
        .all(|key| key.at >= 0.0
            && key.at <= scene.duration
            && key.zoom.is_finite()
            && key.zoom >= 0.05
            && key.x.is_finite()
            && key.y.is_finite())));
    assert!(script.scenes[result.first_scene..]
        .iter()
        .all(|scene| !scene
            .events
            .iter()
            .any(|event| matches!(event,Event::Gesture{actor,..}if actor=="PUPPET"))));
}

#[test]
fn framed_custom_head_accent_returns_to_rest_and_the_first_shot_contains_its_geometry() {
    let mut base = motion_script::parse(include_str!("../../../examples/acting-rig.yaml")).unwrap();
    base.project.width = 320;
    base.project.height = 180;
    let source = motion_script::to_source(&base).unwrap();
    let result = motion_script::auto_direction::direct_with_audio(
        &source,
        "PUPPET: Hei!\nFRIEND: Terve!",
        &[24000, 24000],
        &["one.wav".into(), "two.wav".into()],
    )
    .unwrap();
    let directed = motion_script::parse(&result.source).unwrap();
    let scene = &directed.scenes[result.first_scene];
    let keys = &scene.composition.joint_tracks["PUPPET"]["head"];
    assert_eq!(keys.len(), 3);
    assert_eq!(keys[0].rotation, 0.0);
    assert_ne!(keys[1].rotation, 0.0);
    assert_eq!(keys[2].rotation, 0.0);
    assert!(keys
        .iter()
        .all(|key| key.x == 0.0 && key.y == 0.0 && key.scale_x == 1.0 && key.scale_y == 1.0));
    let root = &scene.composition.actor_tracks["PUPPET"][0];
    let camera = &scene.composition.camera[0];
    let rig = base.characters["PUPPET"].rig.as_ref().unwrap();
    for part in rig.evaluate_parts().unwrap() {
        for point in part.points {
            let x = (root.x + point.x - camera.x) * camera.zoom + 160.0;
            let y = (126.0 + root.y + point.y - 90.0 - camera.y) * camera.zoom + 90.0;
            assert!((0.0..=320.0).contains(&x), "x={x}");
            assert!((0.0..=180.0).contains(&y), "y={y}");
        }
    }
}
