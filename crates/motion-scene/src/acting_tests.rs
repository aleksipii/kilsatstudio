//! Acting layer: viseme/emotion variants, gaze and blink on custom rigid rigs.
use super::*;
use motion_script::Script;

const ACTING: &str = include_str!("../../../examples/acting-rig.yaml");
const FACE: usize = 0;
const EYE_L: usize = 1;
const PUPIL_L: usize = 3;
const BROW_L: usize = 5;
const MOUTH: usize = 7;
const PUPIL_RADIUS_TRAVEL: f64 = 4.0;

fn acting() -> Script {
    motion_script::parse(ACTING).unwrap()
}
fn puppet(scene: &EvaluatedScene) -> &ActorPose {
    scene.actors.iter().find(|a| a.id == "PUPPET").unwrap()
}
fn first(pose: &ActorPose, part: usize) -> (f64, f64) {
    let p = pose.rigid_parts[part].points[0];
    (p.x, p.y)
}
fn centroid(pose: &ActorPose, part: usize) -> (f64, f64) {
    let points = &pose.rigid_parts[part].points;
    let n = points.len() as f64;
    (
        points.iter().map(|p| p.x).sum::<f64>() / n,
        points.iter().map(|p| p.y).sum::<f64>() / n,
    )
}
/// Mouth first vertex in actor space for the head joint at y = -40.
fn mouth_with(script: &Script, time: f64, viseme: Option<Viseme>) -> motion_rig::EvaluatedPart {
    let mut scene = evaluate(script, 0, time).unwrap();
    match viseme {
        Some(v) => apply_speech(&mut scene, &[("PUPPET", v)]),
        None => apply_speech(&mut scene, &[]),
    }
    puppet(&scene).rigid_parts[MOUTH].clone()
}
/// A scene time (in 1/24 s steps) at which the puppet is, or is not, blinking.
fn blink_time(script: &Script, blinking: bool) -> f64 {
    let rig = script.characters["PUPPET"]
        .rig
        .as_ref()
        .unwrap()
        .prepare()
        .unwrap();
    (0..144)
        .map(|i| f64::from(i) / 24.0)
        .find(|t| rig.blink_active("PUPPET", *t) == blinking)
        .unwrap()
}

#[test]
fn every_rig_viseme_name_matches_a_lipsync_viseme() {
    assert_eq!(motion_rig::VISEME_NAMES.len(), 13);
    for name in motion_rig::VISEME_NAMES {
        let viseme = viseme_from_name(name).expect(name);
        assert_eq!(viseme_name(viseme), name);
        assert_eq!(
            serde_json::to_string(&viseme).unwrap(),
            format!("\"{name}\"")
        );
    }
}

#[test]
fn legacy_custom_rig_has_no_acting_state_and_serializes_without_new_keys() {
    let source = "characters:\n  P:\n    asset: custom\n    rig:\n      version: 1\n      id: plain\n      joints: [{id: root, parent: null}]\n      parts: [{id: body, joint: root, points: [[0,0],[10,0],[0,10]], color: [1,2,3,255]}]\nscenes: [{id: s, duration: 2, events: [{type: emotion, at: 0, actor: P, value: happy}]}]\n";
    let script = motion_script::parse(source).unwrap();
    let rig = script.characters["P"].rig.as_ref().unwrap();
    let json = serde_json::to_string(rig).unwrap();
    for key in ["variants", "visemes", "expressions", "eyes"] {
        assert!(!json.contains(key), "{key} leaked into legacy rig: {json}");
    }
    let mut scene = evaluate(&script, 0, 1.0).unwrap();
    apply_speech(&mut scene, &[("P", Viseme::A)]);
    assert!(scene.actors[0].mouth.is_none());
    assert_eq!(scene.actors[0].rigid_parts, rig.evaluate_parts().unwrap());
    let composition = serde_json::to_string(&script.scenes[0].composition).unwrap();
    assert!(!composition.contains("gaze"));
}

#[test]
fn visemes_select_mouth_variants_at_exact_samples() {
    let script = acting();
    // Base shape: no viseme, rest and unmapped visemes. Head joint sits at y = -40.
    for viseme in [None, Some(Viseme::Rest), Some(Viseme::Sibilant)] {
        let part = mouth_with(&script, 1.0, viseme);
        assert_eq!((part.points[0].x, part.points[0].y), (-10.0, -26.0));
    }
    for (viseme, expected) in [
        (Viseme::A, (-10.0, -30.0)),
        (Viseme::E, (-14.0, -28.0)),
        (Viseme::I, (-14.0, -28.0)),
        (Viseme::O, (-6.0, -30.0)),
        (Viseme::U, (-6.0, -30.0)),
    ] {
        let part = mouth_with(&script, 1.0, Some(viseme));
        assert_eq!((part.points[0].x, part.points[0].y), expected);
    }
    // Only the mouth changes; a viseme does not touch the other parts.
    let mut scene = evaluate(&script, 0, 1.0).unwrap();
    let before = puppet(&scene).rigid_parts.clone();
    apply_speech(&mut scene, &[("PUPPET", Viseme::A)]);
    let after = puppet(&scene).rigid_parts.clone();
    for (i, (a, b)) in before.iter().zip(&after).enumerate() {
        assert_eq!(a == b, i != MOUTH, "part {i}");
    }
    // Other actors' visemes and repeated applications change nothing further.
    let mut other = evaluate(&script, 0, 1.0).unwrap();
    apply_speech(&mut other, &[("FRIEND", Viseme::A)]);
    assert_eq!(puppet(&other).rigid_parts, before);
    apply_speech(&mut scene, &[("PUPPET", Viseme::A)]);
    assert_eq!(puppet(&scene).rigid_parts, after);
}

#[test]
fn viseme_selection_does_not_depend_on_call_order() {
    let script = acting();
    let samples = [
        (3.0, Viseme::O),
        (0.5, Viseme::A),
        (3.0, Viseme::Rest),
        (5.5, Viseme::E),
        (0.5, Viseme::A),
        (3.0, Viseme::O),
    ];
    let expected: Vec<_> = samples
        .iter()
        .map(|(t, v)| mouth_with(&script, *t, Some(*v)))
        .collect();
    let validated = ValidatedScript::new(std::sync::Arc::new(script)).unwrap();
    for _ in 0..2 {
        for ((t, v), want) in samples.iter().zip(&expected).rev() {
            let mut scene = validated.evaluate(0, *t).unwrap();
            apply_speech(&mut scene, &[("PUPPET", *v)]);
            assert_eq!(&puppet(&scene).rigid_parts[MOUTH], want);
        }
    }
}

#[test]
fn emotion_variants_follow_events_and_visemes_win_on_the_mouth() {
    let script = acting();
    let neutral = evaluate(&script, 0, 1.99).unwrap();
    assert_eq!(first(puppet(&neutral), BROW_L), (-22.0, -62.0));
    let happy = evaluate(&script, 0, 2.0).unwrap();
    assert_eq!(puppet(&happy).emotion, "happy");
    assert_eq!(first(puppet(&happy), BROW_L), (-22.0, -68.0));
    assert_eq!(puppet(&happy).rigid_parts[MOUTH].color, [150, 60, 70, 255]);
    assert_eq!(first(puppet(&happy), MOUTH), (-12.0, -27.0));
    // The viseme replaces the mouth part; the raised brows stay.
    let mut speaking = happy.clone();
    apply_speech(&mut speaking, &[("PUPPET", Viseme::A)]);
    assert_eq!(first(puppet(&speaking), MOUTH), (-10.0, -30.0));
    assert_eq!(
        puppet(&speaking).rigid_parts[MOUTH].color,
        [120, 40, 50, 255]
    );
    assert_eq!(first(puppet(&speaking), BROW_L), (-22.0, -68.0));
    let angry = evaluate(&script, 0, 4.0).unwrap();
    assert_eq!(first(puppet(&angry), BROW_L), (-22.0, -64.0));
    // An emotion with no expression entries keeps the base parts.
    assert_eq!(
        puppet(&neutral).rigid_parts[FACE],
        puppet(&angry).rigid_parts[FACE]
    );
}

#[test]
fn gaze_follows_screen_points_and_actors_within_the_travel_radius() {
    let script = acting();
    let rest = {
        let mut s = script.clone();
        s.scenes[0].composition.gaze_tracks.clear();
        centroid(puppet(&evaluate(&s, 0, 1.0).unwrap()), PUPIL_L)
    };
    let shift = |t: f64| {
        let c = centroid(puppet(&evaluate(&script, 0, t).unwrap()), PUPIL_L);
        (c.0 - rest.0, c.1 - rest.1)
    };
    let (x0, _) = shift(0.0);
    assert!((x0 + PUPIL_RADIUS_TRAVEL).abs() < 0.01, "left key: {x0}");
    let (x2, _) = shift(2.0);
    assert!((x2 - PUPIL_RADIUS_TRAVEL).abs() < 0.01, "right key: {x2}");
    // Halfway between a right-hand point and the friend on the left, gaze is near centre.
    assert!(shift(3.0).0.abs() < 0.6);
    // The friend stands to the left of the puppet.
    assert!(shift(4.0).0 < -3.0 && shift(6.0).0 < -3.0);
    for i in 0..=144 {
        let (dx, dy) = shift(f64::from(i) / 24.0);
        assert!(dx.hypot(dy) <= PUPIL_RADIUS_TRAVEL + 1e-9, "step {i}");
    }
    // A target on the actor itself leaves the pupils at rest.
    let mut near = script.clone();
    near.scenes[0].composition.gaze_tracks.insert(
        "PUPPET".into(),
        vec![motion_script::GazeKey {
            at: 0.0,
            x: Some(128.0),
            y: Some(179.2 - 47.0),
            target: None,
        }],
    );
    let c = centroid(puppet(&evaluate(&near, 0, 1.0).unwrap()), PUPIL_L);
    assert!((c.0 - rest.0).abs() < 1e-6 && (c.1 - rest.1).abs() < 1e-6);
}

#[test]
fn blinks_are_deterministic_bounded_and_independent_of_seek_order() {
    let script = acting();
    let rig = script.characters["PUPPET"]
        .rig
        .as_ref()
        .unwrap()
        .prepare()
        .unwrap();
    let times: Vec<f64> = (0..24 * 300).map(|i| f64::from(i) / 24.0).collect();
    let forward: Vec<bool> = times
        .iter()
        .map(|t| rig.blink_active("PUPPET", *t))
        .collect();
    let backward: Vec<bool> = times
        .iter()
        .rev()
        .map(|t| rig.blink_active("PUPPET", *t))
        .rev()
        .collect();
    assert_eq!(forward, backward);
    let mut stride: Vec<(usize, bool)> = (0..times.len())
        .map(|i| (i * 7919 % times.len(), false))
        .map(|(i, _)| (i, rig.blink_active("PUPPET", times[i])))
        .collect();
    stride.sort();
    assert!(stride.iter().all(|(i, b)| forward[*i] == *b));
    // Interval 3 s over 300 s: at most one blink per window and a fair number of them.
    let onsets = forward.windows(2).filter(|w| !w[0] && w[1]).count();
    assert!((50..=100).contains(&onsets), "{onsets} blinks");
    let open = forward.iter().filter(|b| **b).count() as f64 / forward.len() as f64;
    assert!(open < 0.1, "eyes closed {open} of the time");
    // The seed is the actor id: another id blinks at other times, same id repeats.
    let other: Vec<bool> = times
        .iter()
        .map(|t| rig.blink_active("FRIEND", *t))
        .collect();
    assert_ne!(forward, other);
    let again: Vec<bool> = times
        .iter()
        .map(|t| rig.blink_active("PUPPET", *t))
        .collect();
    assert_eq!(forward, again);
    // Whole-scene evaluation agrees with the closed form in any order.
    let validated = ValidatedScript::new(std::sync::Arc::new(script.clone())).unwrap();
    for t in [5.5, 0.0, 3.0, 5.5, 1.25, 0.0] {
        let scene = validated.evaluate(0, t).unwrap();
        let fresh = evaluate(&script, 0, t).unwrap();
        assert_eq!(scene, fresh);
        let closed = puppet(&scene).rigid_parts[EYE_L].color == [60, 40, 40, 255];
        assert_eq!(closed, rig.blink_active("PUPPET", t));
    }
    assert!(blink_time(&script, true) >= 0.0);
}

#[test]
fn blink_swaps_the_eye_variants_for_the_duration_only() {
    let script = acting();
    let (closed, open) = (blink_time(&script, true), blink_time(&script, false));
    let blinking = evaluate(&script, 0, closed).unwrap();
    let eye = &puppet(&blinking).rigid_parts[EYE_L];
    assert_eq!((eye.points[0].y, eye.color), (-48.0, [60, 40, 40, 255]));
    assert_eq!(puppet(&blinking).rigid_parts[PUPIL_L].color[3], 0);
    let awake = evaluate(&script, 0, open).unwrap();
    assert_eq!(
        puppet(&awake).rigid_parts[EYE_L].color,
        [255, 255, 255, 255]
    );
    assert_eq!(puppet(&awake).rigid_parts[PUPIL_L].color[3], 255);
}

fn rejected(edit: impl Fn(&str) -> String) -> String {
    match motion_script::parse(&edit(ACTING)) {
        Ok(script) => match script.validate() {
            Ok(()) => panic!("accepted an invalid acting definition"),
            Err(error) => error.to_string(),
        },
        Err(error) => error.to_string(),
    }
}

#[test]
fn invalid_acting_definitions_and_gaze_tracks_are_rejected() {
    let _ = rejected(|s| s.replace("map: {a: open,", "map: {a: nowhere,"));
    let _ = rejected(|s| s.replace("map: {a: open,", "map: {gasp: open,"));
    let _ = rejected(|s| {
        s.replace(
            "happy:\n          - {part: mouth",
            "joyful:\n          - {part: mouth",
        )
    });
    let _ = rejected(|s| {
        s.replace(
            "{part: brow_l, variant: raised}",
            "{part: brow_l, variant: missing}",
        )
    });
    let _ = rejected(|s| s.replace("pupils: [pupil_l, pupil_r]", "pupils: [pupil_l, ghost]"));
    let _ = rejected(|s| s.replace("travel: 4", "travel: 1000000"));
    let _ = rejected(|s| s.replace("duration: 0.15", "duration: 0"));
    let _ = rejected(|s| s.replace("interval: 3", "interval: 0.5"));
    let _ = rejected(|s| s.replace("variant: blink, interval", "variant: nope, interval"));
    // Gaze: unknown targets, self targets, mixed or half-specified keys, order, bounds.
    let _ = rejected(|s| s.replace("target: FRIEND", "target: NOBODY"));
    let _ = rejected(|s| s.replace("target: FRIEND", "target: PUPPET"));
    let _ = rejected(|s| s.replace("{at: 4, target: FRIEND}", "{at: 4, x: 1, target: FRIEND}"));
    let _ = rejected(|s| s.replace("{at: 2, x: 256, y: 128}", "{at: 2, x: 256}"));
    let _ = rejected(|s| s.replace("{at: 2, x: 256, y: 128}", "{at: 0, x: 256, y: 128}"));
    let _ = rejected(|s| s.replace("{at: 4, target: FRIEND}", "{at: 9, target: FRIEND}"));
    let _ = rejected(|s| s.replace("{at: 2, x: 256, y: 128}", "{at: 2, x: 1e12, y: 128}"));
    // Gaze needs a rig with eyes.
    let _ = rejected(|s| s.replace("PUPPET:\n          - {at: 0", "FRIEND:\n          - {at: 0"));
    // Too many variants on one part.
    let many: String = (0..17)
        .map(|i| format!("            v{i}: {{color: [1, 2, 3, 255]}}\n"))
        .collect();
    let message = rejected(|s| {
        s.replace(
            "            open: {points",
            &format!("{many}            open: {{points"),
        )
    });
    assert!(message.contains("custom rig"), "{message}");
}

#[test]
fn removing_a_gaze_target_actor_strips_only_keys_that_point_at_it() {
    let source = ACTING.replace(
        "      - {type: move, at: 0, actor: FRIEND, x: 40, y: 179, duration: 0.5}\n",
        "",
    );
    assert_ne!(source, ACTING);
    let edited = motion_script::remove_character(&source, "FRIEND").unwrap();
    let script = motion_script::parse(&edited).unwrap();
    let keys = &script.scenes[0].composition.gaze_tracks["PUPPET"];
    assert_eq!(keys.len(), 2);
    assert!(keys.iter().all(|k| k.target.is_none()));
}
