use crate::story::*;
use crate::{Event, Script};

const STARTER: &str = include_str!("../../../examples/stick-studio/stick-studio.kilsat");
const EXAMPLE: &str = include_str!("../../../examples/story/kahvitauko.yaml");

fn catalog() -> StoryCatalog {
    let document: serde_json::Value = serde_json::from_str(STARTER).unwrap();
    let script: Script = crate::parse(document["source"].as_str().unwrap()).unwrap();
    let characters = script
        .characters
        .iter()
        .map(|(id, character)| {
            (
                id.clone(),
                CatalogCharacter {
                    name: id.to_lowercase(),
                    description: "test character".into(),
                    character: character.clone(),
                    accent: "#5e91ae".into(),
                    english_voice: "am_michael".into(),
                    finnish_voice: "piper_harri".into(),
                    speed: 1.0,
                },
            )
        })
        .collect();
    let sets = ["kahvila", "toimisto"]
        .into_iter()
        .map(|id| {
            (
                id.to_string(),
                CatalogSet {
                    name: id.into(),
                    description: "test set".into(),
                    background_color: "#eddec9".into(),
                },
            )
        })
        .collect();
    StoryCatalog { characters, sets }
}

fn minimal(beats: &str) -> String {
    format!(
        "kilsat_story: 1\ntitle: Test\nlanguage: en\ncast:\n  VEIKKO: {{}}\n  SISU: {{}}\nscenes:\n  - id: one\n    set: toimisto\n    positions: {{VEIKKO: left, SISU: right}}\n    beats:\n{beats}"
    )
}

fn errors(source: &str) -> Vec<StoryIssue> {
    compile_story(source, &catalog(), None).unwrap_err()
}

#[test]
fn example_compiles_to_a_valid_deterministic_screenplay() {
    let catalog = catalog();
    let first = compile_story(EXAMPLE, &catalog, None).unwrap();
    let second = compile_story(EXAMPLE, &catalog, None).unwrap();
    let source = crate::to_source(&first.script).unwrap();
    assert_eq!(source, crate::to_source(&second.script).unwrap());
    // The output is ordinary MotionScript that the normal parser accepts.
    let reparsed = crate::parse(&source).unwrap();
    assert_eq!(reparsed.scenes.len(), 2);
    assert_eq!(first.timing, StoryTiming::Estimated);
    assert_eq!(first.lines.len(), 5);
    assert_eq!(
        first.scene_sets,
        vec![
            ("kahvila_aamu".into(), "kahvila".into()),
            ("sisu_saapuu".into(), "kahvila".into())
        ]
    );
    let script = &first.script;
    assert_eq!(script.characters.len(), 3);
    assert!(script.scenes[0].events.iter().any(|e| matches!(
        e,
        Event::Hold { actor, item: Some(item), .. } if actor == "VEIKKO" && item == "kahvikuppi"
    )));
    assert_eq!(first.furniture.len(), 1, "VEIKKO sits on a chair");
    let voice = script.characters["SISU"].voice.as_ref().unwrap();
    assert_eq!(
        (voice.provider.as_str(), voice.language.as_str()),
        ("piper", "fi-fi")
    );
    assert!(script.project.captions.is_some());
    // SISU is not on stage in the first scene.
    let opening = &script.scenes[0];
    assert!(!opening.composition.layers["SISU"].visible);
    assert!(opening.composition.layers["VEIKKO"].visible);
    assert!(first.shot_count >= 4);
    assert!(opening.composition.camera.len() >= 4);
    assert!(opening.composition.gaze_tracks.contains_key("VEIKKO"));
    assert!((first.duration - script.scenes.iter().map(|s| s.duration).sum::<f64>()).abs() < 1e-9);
}

#[test]
fn dialogue_lines_never_overlap_and_listeners_look_at_the_speaker() {
    let result = compile_story(EXAMPLE, &catalog(), None).unwrap();
    for pair in result.lines.windows(2) {
        if pair[0].scene == pair[1].scene {
            assert!(pair[0].at + pair[0].duration <= pair[1].at);
        }
    }
    // In the second scene VEIKKO first looks at the nearer SISU, then turns to
    // PASMA when PASMA speaks.
    let scene = &result.script.scenes[1];
    let line = result
        .lines
        .iter()
        .find(|l| l.scene == "sisu_saapuu" && l.actor == "PASMA")
        .unwrap();
    let veikko = &scene.composition.gaze_tracks["VEIKKO"];
    assert_eq!(veikko[0].target.as_deref(), Some("SISU"));
    assert!(veikko
        .iter()
        .any(|key| key.at > line.at && key.target.as_deref() == Some("PASMA")));
}

#[test]
fn supplied_line_lengths_drive_timing() {
    let catalog = catalog();
    let measured: Vec<MeasuredLine> = [2.0, 1.5, 3.0, 2.25, 1.75]
        .into_iter()
        .enumerate()
        .map(|(i, seconds)| MeasuredLine {
            seconds,
            audio: Some(format!("assets/story/{i}.wav")),
        })
        .collect();
    let result = compile_story(EXAMPLE, &catalog, Some(&measured)).unwrap();
    assert_eq!(result.timing, StoryTiming::Supplied);
    for (line, measured) in result.lines.iter().zip(&measured) {
        assert_eq!(line.duration, measured.seconds);
        assert_eq!(line.audio, measured.audio);
    }
    let speak_audio: Vec<_> = result
        .script
        .scenes
        .iter()
        .flat_map(|s| &s.events)
        .filter_map(|e| match e {
            Event::Speak { audio, .. } => audio.clone(),
            _ => None,
        })
        .collect();
    assert_eq!(speak_audio.len(), 5);
    let line = |seconds: f64, audio: Option<&str>| MeasuredLine {
        seconds,
        audio: audio.map(str::to_string),
    };
    assert!(compile_story(EXAMPLE, &catalog, Some(&[line(1.0, None)])).is_err());
    let mut bad = measured.clone();
    bad[0] = line(f64::NAN, None);
    assert!(compile_story(EXAMPLE, &catalog, Some(&bad)).is_err());
    bad[0] = line(1.0, Some("../outside.wav"));
    assert!(compile_story(EXAMPLE, &catalog, Some(&bad)).is_err());
    bad[0] = line(1.0, Some("/abs.wav"));
    assert!(compile_story(EXAMPLE, &catalog, Some(&bad)).is_err());
}

#[test]
fn hard_cuts_land_on_frames_and_camera_keys_increase() {
    let result = compile_story(EXAMPLE, &catalog(), None).unwrap();
    for scene in &result.script.scenes {
        let keys = &scene.composition.camera;
        assert_eq!(keys[0].at, 0.0);
        assert!(keys.windows(2).all(|pair| pair[1].at > pair[0].at));
        // Close-up 2.4 pushed in by 30 % is the tightest framing.
        assert!(keys
            .iter()
            .all(|key| key.zoom >= 1.0 && key.zoom <= 2.4 * 1.3 + 1e-9));
        // Each hard cut is a key pair less than a frame apart, with the
        // incoming key on a frame boundary.
        for pair in keys.windows(2) {
            if pair[1].at - pair[0].at < 1.0 / 48.0 {
                let frames = pair[1].at * 24.0;
                assert!((frames - frames.round()).abs() < 1e-6);
            }
        }
    }
}

#[test]
fn walks_use_gaits_and_profile_facing() {
    let result = compile_story(EXAMPLE, &catalog(), None).unwrap();
    let scene = &result.script.scenes[1];
    let moves: Vec<_> = scene
        .events
        .iter()
        .filter_map(|event| match event {
            Event::Move { actor, gait, .. } => Some((actor.as_str(), gait.as_deref())),
            _ => None,
        })
        .collect();
    // SISU walks past VEIKKO, who steps aside and back.
    assert_eq!(
        moves,
        vec![
            ("SISU", Some("walk")),
            ("VEIKKO", Some("walk")),
            ("VEIKKO", Some("walk")),
            ("PASMA", Some("run"))
        ]
    );
    assert!(result
        .notes
        .iter()
        .any(|n| n.contains("VEIKKO steps aside")));
    assert!(scene.events.iter().any(
        |e| matches!(e, Event::Face { actor, value, .. } if actor == "SISU" && value == "right")
    ));
}

#[test]
fn fenced_replies_are_accepted() {
    let reply = format!("Here is your story:\n\n```yaml\n{EXAMPLE}```\nEnjoy!\n");
    assert_eq!(extract_story_text(&reply), EXAMPLE);
    compile_story(&reply, &catalog(), None).unwrap();
    assert_eq!(extract_story_text("a: 1\n"), "a: 1\n");
}

#[test]
fn every_problem_is_reported_with_a_path() {
    let source = minimal(
        "      - say: VEIKKO\n        line: Hi\n        emotion: furious\n        gesture: dance\n      - say: NOUTAJA\n        line: Hello\n      - do: SISU\n        move_to: moon\n      - pause: 99\n",
    )
    .replace("set: toimisto", "set: mars");
    let issues = errors(&source);
    let find = |path: &str, kind: IssueKind| {
        assert!(
            issues.iter().any(|i| i.path == path && i.kind == kind),
            "missing {path} {kind:?} in {issues:#?}"
        )
    };
    find("scenes[0].set", IssueKind::Reference);
    find("scenes[0].beats[0].emotion", IssueKind::Unsupported);
    find("scenes[0].beats[0].gesture", IssueKind::Unsupported);
    find("scenes[0].beats[1].say", IssueKind::Reference);
    find("scenes[0].beats[2].move_to", IssueKind::Unsupported);
    find("scenes[0].beats[3].pause", IssueKind::Bound);
    let report = issues_report(&issues);
    assert!(report.contains("furious") && report.contains("neutral, happy"));
}

#[test]
fn schema_and_syntax_errors_carry_positions() {
    let unknown = minimal("      - say: VEIKKO\n        line: Hi\n        mood: happy\n");
    let issue = &errors(&unknown)[0];
    assert_eq!(issue.kind, IssueKind::Schema);
    assert!(issue.line.is_some());
    let broken = "kilsat_story: 1\ntitle: a: b\n";
    assert_eq!(errors(broken)[0].kind, IssueKind::Syntax);
    assert_eq!(errors("")[0].kind, IssueKind::Schema);
    let version = minimal("      - pause: 1\n").replace("kilsat_story: 1", "kilsat_story: 2");
    assert!(errors(&version).iter().any(|i| i.path == "kilsat_story"));
}

#[test]
fn beat_shape_and_references_are_strict() {
    for (beats, path) in [
        ("      - say: VEIKKO\n", "scenes[0].beats[0].line"),
        (
            "      - say: VEIKKO\n        do: SISU\n        line: x\n",
            "scenes[0].beats[0]",
        ),
        (
            "      - do: VEIKKO\n        line: x\n",
            "scenes[0].beats[0].line",
        ),
        (
            "      - do: VEIKKO\n        look: VEIKKO\n",
            "scenes[0].beats[0].look",
        ),
        (
            "      - do: VEIKKO\n        gait: run\n",
            "scenes[0].beats[0].gait",
        ),
        (
            "      - say: VEIKKO\n        line: x\n        reaction: {by: VEIKKO}\n",
            "scenes[0].beats[0].reaction.by",
        ),
        (
            "      - do: VEIKKO\n        move_to: VEIKKO\n",
            "scenes[0].beats[0].move_to",
        ),
        (
            "      - do: VEIKKO\n        move_to: backstage\n",
            "scenes[0].beats[0].move_to",
        ),
        (
            "      - pause: 1\n        emotion: happy\n",
            "scenes[0].beats[0]",
        ),
    ] {
        let issues = errors(&minimal(beats));
        assert!(
            issues.iter().any(|i| i.path == path),
            "{beats}: {issues:#?}"
        );
    }
    let shared = minimal("      - pause: 1\n").replace("SISU: right", "SISU: left");
    assert!(errors(&shared).iter().any(|i| i.kind == IssueKind::Invalid));
    let bad_voice =
        minimal("      - pause: 1\n").replace("VEIKKO: {}", "VEIKKO: {voice: piper_harri}");
    assert!(errors(&bad_voice)
        .iter()
        .any(|i| i.path == "cast.VEIKKO.voice"));
    let bad_prop = minimal("      - pause: 1\n").replace("VEIKKO: {}", "VEIKKO: {holds: banana}");
    assert!(errors(&bad_prop)
        .iter()
        .any(|i| i.path == "cast.VEIKKO.holds"));
    let long = minimal(&format!(
        "      - say: VEIKKO\n        line: {}\n",
        "word ".repeat(100)
    ));
    assert!(errors(&long).iter().any(|i| i.kind == IssueKind::Bound));
}

#[test]
fn guide_lists_the_catalog_and_its_example_compiles() {
    let catalog = catalog();
    let guide = authoring_guide(&catalog);
    for id in catalog.characters.keys().chain(catalog.sets.keys()) {
        assert!(guide.contains(id.as_str()));
    }
    assert!(guide.contains("puhelin"));
    let example = extract_story_text(&guide[guide.find("EXAMPLE").unwrap()..]);
    compile_story(example, &catalog, None).unwrap();
}

#[test]
fn estimates_are_bounded_and_monotonic() {
    assert_eq!(estimate_line_seconds("Hi"), 1.0);
    assert!(estimate_line_seconds("one two three four five six") > 2.0);
    assert!(estimate_line_seconds("Kahvitauko on palaverin jälkeen") > 1.8);
}

#[test]
fn camera_moves_shape_the_shot() {
    let catalog = catalog();
    let zooms = |camera: &str| {
        let text = minimal(&format!(
            "      - say: VEIKKO\n        line: One two three four five six seven.\n        shot: medium\n        camera: {camera}\n"
        ));
        let result = compile_story(&text, &catalog, None).unwrap();
        result.script.scenes[0].composition.camera.clone()
    };
    let push = zooms("push_in");
    let (first, last) = (&push[push.len() - 2], push.last().unwrap());
    assert!(first.zoom < last.zoom, "push_in zooms in");
    let pull = zooms("pull_out");
    assert!(pull[pull.len() - 2].zoom > pull.last().unwrap().zoom);
    let pan = zooms("pan_right");
    assert!(
        pan[pan.len() - 2].x < pan.last().unwrap().x,
        "pan_right moves right"
    );
    let shake = zooms("shake");
    assert!(shake.len() > push.len() + 5, "shake adds per-frame keys");
    let still = zooms("static");
    let (a, b) = (&still[still.len() - 2], still.last().unwrap());
    assert_eq!((a.x, a.y, a.zoom), (b.x, b.y, b.zoom));
    assert!(
        errors(&minimal("      - pause: 1\n        camera: dolly_zoom\n"))
            .iter()
            .any(|i| i.path == "scenes[0].beats[0].camera")
    );
}

#[test]
fn new_gestures_and_speaking_rate_compile() {
    let catalog = catalog();
    for gesture in motion_animation::GESTURES {
        let text = minimal(&format!(
            "      - do: VEIKKO\n        gesture: {gesture}\n      - say: SISU\n        line: Yes.\n        gesture: {gesture}\n"
        ));
        compile_story(&text, &catalog, None).unwrap_or_else(|e| panic!("{gesture}: {e:?}"));
    }
    let fast = minimal("      - pause: 1\n").replace("VEIKKO: {}", "VEIKKO: {speed: 1.2}");
    let result = compile_story(&fast, &catalog, None).unwrap();
    assert_eq!(
        result.script.characters["VEIKKO"]
            .voice
            .as_ref()
            .unwrap()
            .speed,
        1.2f32
    );
    let wrong = minimal("      - pause: 1\n").replace("VEIKKO: {}", "VEIKKO: {speed: 3}");
    assert!(errors(&wrong).iter().any(|i| i.path == "cast.VEIKKO.speed"));
}

#[test]
fn props_and_sitting_become_timed_events_and_furniture() {
    let catalog = catalog();
    let text = minimal(
        "      - do: VEIKKO\n        move_to: center\n        holds: kahvikuppi\n      - do: VEIKKO\n        pose: sit\n      - say: SISU\n        line: Comfortable?\n      - do: VEIKKO\n        holds: none\n      - do: VEIKKO\n        pose: stand\n      - do: VEIKKO\n        move_to: left\n",
    );
    let result = compile_story(&text, &catalog, None).unwrap();
    let events = &result.script.scenes[0].events;
    let walk_end = events
        .iter()
        .find_map(|e| match e {
            Event::Move { at, duration, .. } => Some(at + duration),
            _ => None,
        })
        .unwrap();
    let holds: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Hold { at, item, .. } => Some((*at, item.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(holds.len(), 2);
    assert!(holds[0].0 >= walk_end - 1e-9, "picked up after arriving");
    assert_eq!(holds[0].1.as_deref(), Some("kahvikuppi"));
    assert_eq!(holds[1].1, None);
    let poses: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Pose { value, .. } => Some(value.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(poses, vec!["sit", "stand"]);
    assert_eq!(result.furniture.len(), 1);
    let chair = &result.furniture[0];
    assert_eq!(chair.scene, "one");
    assert!(chair.seat < chair.floor && (chair.x - 960.0).abs() < 1e-6);
}

#[test]
fn walking_while_seated_and_unknown_props_are_reported() {
    let seated = minimal(
        "      - do: VEIKKO\n        pose: sit\n      - do: VEIKKO\n        move_to: center\n",
    );
    assert!(errors(&seated)
        .iter()
        .any(|i| i.path == "scenes[0].beats[1].move_to"));
    let unknown = minimal("      - do: VEIKKO\n        holds: banana\n");
    assert!(errors(&unknown)
        .iter()
        .any(|i| i.path == "scenes[0].beats[0].holds"));
    let on_line = minimal("      - say: VEIKKO\n        line: Hi\n        pose: sit\n");
    assert!(errors(&on_line).iter().any(|i| i.kind == IssueKind::Schema));
}

#[test]
fn review_flags_off_screen_speakers_fast_speech_and_long_static_shots() {
    let catalog = catalog();
    // The camera stays on SISU while VEIKKO speaks far left.
    let off = minimal(
        "      - say: VEIKKO\n        line: Over here, over here.\n        shot: close_up\n        on: SISU\n",
    );
    let result = compile_story(&off, &catalog, None).unwrap();
    assert!(result
        .review
        .iter()
        .any(|n| matches!(n, ReviewNote::SpeakerOffScreen { actor, .. } if actor == "VEIKKO")));
    let fast = [MeasuredLine {
        seconds: 1.0,
        audio: None,
    }];
    let rushed = minimal("      - say: VEIKKO\n        line: Twenty-five characters is!\n");
    let result = compile_story(&rushed, &catalog, Some(&fast)).unwrap();
    assert!(result
        .review
        .iter()
        .any(|n| matches!(n, ReviewNote::FastSpeech { .. })));
    let long = minimal("      - do: VEIKKO\n        emotion: sad\n        hold: 10\n        shot: medium\n        camera: static\n      - do: SISU\n        emotion: happy\n");
    let result = compile_story(&long, &catalog, None).unwrap();
    assert!(
        result.review.is_empty(),
        "an explicit static move is intended: {:?}",
        result.review
    );
    let long = long.replace("        camera: static\n", "");
    let result = compile_story(&long, &catalog, None).unwrap();
    assert!(result
        .review
        .iter()
        .any(|n| matches!(n, ReviewNote::LongStaticShot { .. })));
    // The bundled example passes the automatic checks.
    assert_eq!(
        compile_story(EXAMPLE, &catalog, None).unwrap().review,
        vec![]
    );
}

#[test]
fn principles_reach_the_cast_and_beat_gestures() {
    let catalog = catalog();
    let text = minimal(
        "      - do: VEIKKO\n        gesture: wave\n        principles: {preset: snappy, squash_stretch: 0.1}\n      - say: SISU\n        line: Hi.\n        gesture: nod\n",
    )
    .replace("VEIKKO: {}", "VEIKKO: {principles: cartoon}");
    let result = compile_story(&text, &catalog, None).unwrap();
    let veikko = result.script.characters["VEIKKO"].principles.as_ref();
    assert_eq!(veikko.unwrap().preset.as_deref(), Some("cartoon"));
    assert!(result.script.characters["SISU"].principles.is_none());
    let gestures: Vec<_> = result.script.scenes[0]
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Gesture {
                actor, principles, ..
            } => Some((actor.as_str(), principles.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(gestures.len(), 2);
    let (actor, spec) = &gestures[0];
    assert_eq!(*actor, "VEIKKO");
    assert_eq!(spec.as_ref().unwrap().squash_stretch, Some(0.1));
    assert!(gestures[1].1.is_none());
    // The compiled screenplay keeps the blocks through a source round trip.
    let source = crate::to_source(&result.script).unwrap();
    let reparsed = crate::parse(&source).unwrap();
    assert_eq!(
        reparsed.characters["VEIKKO"].principles,
        result.script.characters["VEIKKO"].principles
    );

    let bad =
        minimal("      - pause: 1\n").replace("VEIKKO: {}", "VEIKKO: {principles: {overshoot: 5}}");
    let issue = errors(&bad)
        .into_iter()
        .find(|i| i.path == "cast.VEIKKO.principles")
        .expect("bounded");
    assert!(issue.message.contains("0..=3"), "{}", issue.message);
    let lonely = minimal("      - do: VEIKKO\n        face: left\n        principles: snappy\n");
    assert!(errors(&lonely)
        .iter()
        .any(|i| i.path.ends_with(".principles")));
}

fn moves_of<'a>(script: &'a Script, actor: &str) -> Vec<&'a Event> {
    script.scenes[0]
        .events
        .iter()
        .filter(|e| matches!(e, Event::Move { actor: a, .. } if a == actor))
        .collect()
}

fn faces_of(script: &Script, actor: &str) -> Vec<String> {
    script.scenes[0]
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Face {
                actor: a, value, ..
            } if a == actor => Some(value.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn actors_enter_and_exit_through_offscreen_positions() {
    let source = minimal(
        "      - do: SISU\n        move_to: center\n      - say: SISU\n        line: Morning.\n      - do: SISU\n        move_to: offscreen_left\n        gait: run\n",
    )
    .replace(
        "{VEIKKO: left, SISU: right}",
        "{VEIKKO: offscreen_left, SISU: offscreen_right}",
    );
    let result = compile_story(&source, &catalog(), None).unwrap();
    let moves = moves_of(&result.script, "SISU");
    assert_eq!(moves.len(), 2);
    // Every compiled move validates, though the actor starts and ends outside the frame.
    crate::parse(&crate::to_source(&result.script).unwrap()).unwrap();
    for event in moves {
        let Event::Move { ease, .. } = event else {
            unreachable!()
        };
        assert_eq!(
            ease.as_deref(),
            Some("in_out"),
            "walks speed up and slow down"
        );
    }
    // Enters walking left, exits running left, both in profile.
    assert_eq!(faces_of(&result.script, "SISU")[0], "left");
    // Several actors may wait outside the frame on the same side.
    let crowd = minimal("      - pause: 1\n").replace(
        "{VEIKKO: left, SISU: right}",
        "{VEIKKO: offscreen_left, SISU: offscreen_left}",
    );
    compile_story(&crowd, &catalog(), None).unwrap();
}

#[test]
fn move_to_a_character_stops_at_talking_distance_and_looks_at_them() {
    let source = minimal("      - do: VEIKKO\n        move_to: SISU\n").replace(
        "{VEIKKO: left, SISU: right}",
        "{VEIKKO: far_left, SISU: far_right}",
    );
    let result = compile_story(&source, &catalog(), None).unwrap();
    let scene = &result.script.scenes[0];
    let start = scene.composition.actor_tracks["VEIKKO"][0].x;
    let Event::Move { x, .. } = moves_of(&result.script, "VEIKKO")[0] else {
        unreachable!()
    };
    // The move is relative to the centre; VEIKKO stops left of SISU.
    let stop = x + start;
    let sisu = 0.86 * 1920.0;
    assert!((sisu - stop - 0.16 * 1920.0).abs() < 1e-6, "{stop}");
    assert!(scene.composition.gaze_tracks["VEIKKO"]
        .iter()
        .any(|k| k.target.as_deref() == Some("SISU")));
}

#[test]
fn speakers_can_walk_while_talking() {
    let source = minimal(
        "      - say: VEIKKO\n        line: Wait for me, I am coming with you.\n        move_to: center\n        gait: jog\n",
    );
    let result = compile_story(&source, &catalog(), None).unwrap();
    let speak_at = result.script.scenes[0]
        .events
        .iter()
        .find_map(|e| match e {
            Event::Speak { at, .. } => Some(*at),
            _ => None,
        })
        .unwrap();
    let Event::Move { at, gait, .. } = moves_of(&result.script, "VEIKKO")[0] else {
        unreachable!()
    };
    assert_eq!((*at, gait.as_deref()), (speak_at, Some("jog")));
}

#[test]
fn new_gaits_compile_and_backwards_faces_away_from_travel() {
    for gait in motion_animation::GAITS {
        let source = minimal(&format!(
            "      - do: VEIKKO\n        move_to: center\n        gait: {gait}\n"
        ));
        let result =
            compile_story(&source, &catalog(), None).unwrap_or_else(|e| panic!("{gait}: {e:?}"));
        assert_eq!(moves_of(&result.script, "VEIKKO").len(), 1, "{gait}");
    }
    // VEIKKO travels right (left → center) but faces left while backing up.
    let back = minimal("      - do: VEIKKO\n        move_to: center\n        gait: backwards\n");
    let result = compile_story(&back, &catalog(), None).unwrap();
    assert_eq!(faces_of(&result.script, "VEIKKO")[0], "left");
    // Slower gaits take longer over the same distance.
    let duration = |gait: &str| {
        let source = minimal(&format!(
            "      - do: VEIKKO\n        move_to: far_right\n        gait: {gait}\n"
        ))
        .replace(
            "{VEIKKO: left, SISU: right}",
            "{VEIKKO: far_left, SISU: center}",
        );
        let result = compile_story(&source, &catalog(), None).unwrap();
        match moves_of(&result.script, "VEIKKO")[0] {
            Event::Move { duration, .. } => *duration,
            _ => unreachable!(),
        }
    };
    assert!(duration("sneak") > duration("walk"));
    assert!(duration("sprint") < duration("run"));
    // A leap keeps constant speed and is limited to a short hop.
    let Event::Move { ease, .. } = moves_of(
        &compile_story(
            &minimal("      - do: VEIKKO\n        move_to: center\n        gait: leap\n"),
            &catalog(),
            None,
        )
        .unwrap()
        .script,
        "VEIKKO",
    )[0]
    .clone() else {
        unreachable!()
    };
    assert_eq!(ease, None);
    let far = minimal("      - do: VEIKKO\n        move_to: far_right\n        gait: leap\n")
        .replace("SISU: right", "SISU: center");
    assert!(errors(&far)
        .iter()
        .any(|i| i.path == "scenes[0].beats[0].gait" && i.kind == IssueKind::Invalid));
}

#[test]
fn body_poses_compile_and_block_walking_until_standing() {
    for pose in ["crouch", "kneel", "lie"] {
        let source = minimal(&format!("      - do: VEIKKO\n        pose: {pose}\n"));
        let result = compile_story(&source, &catalog(), None).unwrap();
        assert!(result.script.scenes[0]
            .events
            .iter()
            .any(|e| matches!(e, Event::Pose { value, .. } if value == pose)));
        let walk = minimal(&format!(
            "      - do: VEIKKO\n        pose: {pose}\n      - do: VEIKKO\n        move_to: center\n"
        ));
        assert!(errors(&walk)
            .iter()
            .any(|i| i.path == "scenes[0].beats[1].move_to"));
    }
    // Standing up in the walking beat happens before the walk.
    let up = minimal(
        "      - do: VEIKKO\n        pose: kneel\n      - do: VEIKKO\n        pose: stand\n        move_to: center\n",
    );
    let result = compile_story(&up, &catalog(), None).unwrap();
    let events = &result.script.scenes[0].events;
    let stand = events
        .iter()
        .find_map(|e| match e {
            Event::Pose { at, value, .. } if value == "stand" => Some(*at),
            _ => None,
        })
        .unwrap();
    let Event::Move { at, .. } = moves_of(&result.script, "VEIKKO")[0] else {
        unreachable!()
    };
    assert!(*at > stand);
}

fn move_spans(script: &Script, actor: &str) -> Vec<(f64, f64, f64, f64, Option<String>)> {
    moves_of(script, actor)
        .into_iter()
        .map(|e| match e {
            Event::Move {
                at,
                duration,
                x,
                y,
                ease,
                ..
            } => (*at, *duration, *x, *y, ease.clone()),
            _ => unreachable!(),
        })
        .collect()
}

#[test]
fn follow_walks_after_the_leader_and_stops_behind() {
    let source = minimal(
        "      - do: VEIKKO\n        move_to: far_right\n        gait: jog\n      - do: SISU\n        follow: VEIKKO\n",
    )
    .replace("{VEIKKO: left, SISU: right}", "{VEIKKO: center, SISU: far_left}");
    let result = compile_story(&source, &catalog(), None).unwrap();
    let leader = move_spans(&result.script, "VEIKKO")[0].clone();
    let follower = move_spans(&result.script, "SISU")[0].clone();
    assert!(
        (follower.0 - (leader.0 + 0.4)).abs() < 1.0 / 24.0,
        "starts just after"
    );
    assert!(
        follower.0 + follower.1 >= leader.0 + leader.1,
        "never overtakes"
    );
    let start = result.script.scenes[0].composition.actor_tracks["SISU"][0].x;
    let stop = follower.2 + start;
    assert!(
        (0.86 * 1920.0 - 0.12 * 1920.0 - stop).abs() < 1e-6,
        "{stop}"
    );
    // Takes the leader's gait.
    assert!(result.script.scenes[0].events.iter().any(
        |e| matches!(e, Event::Move { actor, gait, .. } if actor == "SISU" && gait.as_deref() == Some("jog"))
    ));
    let lonely = minimal("      - do: SISU\n        follow: VEIKKO\n");
    assert!(errors(&lonely)
        .iter()
        .any(|i| i.path == "scenes[0].beats[0].follow"));
}

#[test]
fn a_path_is_one_journey_through_its_waypoints() {
    let source = minimal("      - do: VEIKKO\n        path: [center, far_right, right]\n")
        .replace("SISU: right", "SISU: offscreen_right");
    let result = compile_story(&source, &catalog(), None).unwrap();
    let spans = move_spans(&result.script, "VEIKKO");
    assert_eq!(spans.len(), 3);
    let eases: Vec<_> = spans.iter().map(|s| s.4.as_deref()).collect();
    assert_eq!(eases, [Some("in"), None, Some("out")]);
    for pair in spans.windows(2) {
        assert!(
            (pair[0].0 + pair[0].1 - pair[1].0).abs() < 1e-9,
            "back to back"
        );
    }
    // Turning round at far_right shows as a facing change; left → right → left.
    let faces = faces_of(&result.script, "VEIKKO");
    assert_eq!(&faces[..3], ["right", "right", "left"]);
}

#[test]
fn depth_moves_change_size_height_and_facing() {
    let source = minimal("      - do: VEIKKO\n        depth: far\n      - do: VEIKKO\n        depth: near\n        move_to: center\n");
    let result = compile_story(&source, &catalog(), None).unwrap();
    let spans = move_spans(&result.script, "VEIKKO");
    let root = 1080.0 * 0.7;
    assert_eq!(spans[0].3, root - 60.0, "far is higher on screen");
    assert_eq!(spans[1].3, root + 20.0, "near is lower");
    assert_eq!(faces_of(&result.script, "VEIKKO")[0], "back", "walks away");
    let keys = &result.script.scenes[0].composition.actor_tracks["VEIKKO"];
    let sizes: Vec<f64> = keys.iter().map(|k| k.scale_x / 1.6).collect();
    assert_eq!(sizes.len(), 5);
    assert!(
        (sizes[2] - 0.7).abs() < 1e-9 && (sizes[4] - 1.25).abs() < 1e-9,
        "{sizes:?}"
    );
    crate::parse(&crate::to_source(&result.script).unwrap()).unwrap();
}

#[test]
fn pace_look_back_and_carrying_shape_the_walk() {
    let walk = |extra: &str| {
        let source = minimal(&format!(
            "      - do: VEIKKO\n        move_to: far_right\n{extra}"
        ))
        .replace(
            "{VEIKKO: left, SISU: right}",
            "{VEIKKO: far_left, SISU: offscreen_right}",
        );
        compile_story(&source, &catalog(), None).unwrap()
    };
    let normal = move_spans(&walk("").script, "VEIKKO")[0].1;
    let slow = move_spans(&walk("        pace: 0.5\n").script, "VEIKKO")[0].1;
    assert!((slow / normal - 2.0).abs() < 0.05, "{slow} vs {normal}");
    let back = walk("        look_back: true\n");
    let spans = move_spans(&back.script, "VEIKKO");
    assert_eq!(spans.len(), 2);
    assert!(
        spans[1].0 - (spans[0].0 + spans[0].1) >= 0.9 - 1e-9,
        "stops to look"
    );
    assert_eq!(
        &faces_of(&back.script, "VEIKKO")[..3],
        ["right", "left", "right"]
    );
    // Both hands full: a carry, unless the beat names a gait.
    let loaded = minimal(
        "      - do: VEIKKO\n        holds: kahvikuppi\n      - do: VEIKKO\n        holds: kirja\n      - do: VEIKKO\n        move_to: center\n",
    );
    let result = compile_story(&loaded, &catalog(), None).unwrap();
    assert!(result.script.scenes[0]
        .events
        .iter()
        .any(|e| matches!(e, Event::Move { gait, .. } if gait.as_deref() == Some("carry"))));
}

#[test]
fn travel_fields_are_validated() {
    for (beats, path) in [
        (
            "      - do: VEIKKO\n        path: [center]\n",
            "scenes[0].beats[0].path",
        ),
        (
            "      - do: VEIKKO\n        path: [center, moon]\n",
            "scenes[0].beats[0].path[1]",
        ),
        (
            "      - do: VEIKKO\n        follow: VEIKKO\n",
            "scenes[0].beats[0].follow",
        ),
        (
            "      - do: VEIKKO\n        move_to: center\n        pace: 3\n",
            "scenes[0].beats[0].pace",
        ),
        (
            "      - do: VEIKKO\n        pace: 0.8\n",
            "scenes[0].beats[0].pace",
        ),
        (
            "      - do: VEIKKO\n        look_back: true\n",
            "scenes[0].beats[0].look_back",
        ),
        (
            "      - do: VEIKKO\n        depth: underground\n",
            "scenes[0].beats[0].depth",
        ),
        (
            "      - do: VEIKKO\n        move_to: center\n        path: [left, right]\n",
            "scenes[0].beats[0]",
        ),
        (
            "      - say: VEIKKO\n        line: x\n        follow: SISU\n",
            "scenes[0].beats[0].follow",
        ),
    ] {
        let issues = errors(&minimal(beats));
        assert!(
            issues.iter().any(|i| i.path == path),
            "{beats}: {issues:#?}"
        );
    }
}

#[test]
fn stories_paint_by_depth_and_pass_pace_to_the_steps() {
    let source = minimal("      - do: VEIKKO\n        move_to: center\n        pace: 0.5\n");
    let result = compile_story(&source, &catalog(), None).unwrap();
    let composition = &result.script.scenes[0].composition;
    assert!(composition.depth_sort);
    assert!(composition.layers.values().all(|layer| layer.z == 0));
    assert!(result.script.scenes[0]
        .events
        .iter()
        .any(|e| matches!(e, Event::Move { pace: Some(p), .. } if *p == 0.5)));
    // The written source round-trips through validation.
    crate::parse(&crate::to_source(&result.script).unwrap()).unwrap();
}
