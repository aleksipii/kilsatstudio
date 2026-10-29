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
    assert!(script.characters["VEIKKO"].holds.is_some());
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
    let lengths = [2.0, 1.5, 3.0, 2.25, 1.75];
    let result = compile_story(EXAMPLE, &catalog, Some(&lengths)).unwrap();
    assert_eq!(result.timing, StoryTiming::Supplied);
    for (line, length) in result.lines.iter().zip(lengths) {
        assert_eq!(line.duration, length);
    }
    assert!(compile_story(EXAMPLE, &catalog, Some(&[1.0])).is_err());
    assert!(compile_story(EXAMPLE, &catalog, Some(&[f64::NAN, 1.0, 1.0, 1.0, 1.0])).is_err());
}

#[test]
fn hard_cuts_land_on_frames_and_camera_keys_increase() {
    let result = compile_story(EXAMPLE, &catalog(), None).unwrap();
    for scene in &result.script.scenes {
        let keys = &scene.composition.camera;
        assert_eq!(keys[0].at, 0.0);
        assert!(keys.windows(2).all(|pair| pair[1].at > pair[0].at));
        assert!(keys.iter().all(|key| key.zoom >= 1.0 && key.zoom <= 2.5));
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
    assert_eq!(moves, vec![("SISU", Some("walk")), ("PASMA", Some("run"))]);
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
            "      - say: VEIKKO\n        line: x\n        move_to: center\n",
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
