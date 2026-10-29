//! Screen taps drive the free hand: Veikko holds the catalog phone in his
//! right hand (`holds:`) and presses it with his left at every tap.
use motion_rig::{EvaluatedPart, TapError, Vec2, TAP_APPROACH_SECONDS, TAP_HOLD_SECONDS};
use motion_scene::{TapSkip, ValidatedScript};
use std::sync::Arc;

const TAPS: [f64; 2] = [1.5, 3.0];

fn veikko() -> serde_yaml::Value {
    let project: serde_json::Value = serde_json::from_str(include_str!(
        "../../../examples/stick-studio/characters/veikko.kilsat"
    ))
    .unwrap();
    serde_yaml::from_str(project["source"].as_str().unwrap()).unwrap()
}
fn yaml(text: &str) -> serde_yaml::Value {
    serde_yaml::from_str(text).unwrap()
}

/// Veikko lifts the phone in front of his chest; a recording with taps at
/// source 0.5 s and 2.0 s starts at scene 1.0 s (scene taps 1.5 s and 3.0 s).
fn script(taps: &str, edit: impl FnOnce(&mut serde_yaml::Value)) -> ValidatedScript {
    let mut value = veikko();
    value["characters"]["VEIKKO"]["holds"] = yaml("{item: puhelin, hand: right}");
    let composition = &mut value["scenes"][0]["composition"];
    composition["joint_tracks"]["VEIKKO"]["upper_arm_r"] = yaml("[{at: 0, rotation: 1.1}]");
    composition["joint_tracks"]["VEIKKO"]["forearm_r"] = yaml("[{at: 0, rotation: 1.3}]");
    composition["screens"] = yaml(&format!(
        "[{{actor: VEIKKO, slot: phone, at: 1.0, taps: {taps}, \
         source: {{type: video, asset: assets/screens/demo.mov, fps: 30, duration: 4}}}}]"
    ));
    edit(&mut value);
    let script = motion_script::parse(&serde_yaml::to_string(&value).unwrap()).unwrap();
    ValidatedScript::new(Arc::new(script)).unwrap()
}

fn part<'a>(parts: &'a [EvaluatedPart], id: &str) -> &'a EvaluatedPart {
    parts.iter().find(|p| p.id == id).unwrap()
}
fn centroid(part: &EvaluatedPart) -> Vec2 {
    let n = part.points.len() as f64;
    let (x, y) = part
        .points
        .iter()
        .fold((0.0, 0.0), |(x, y), p| (x + p.x, y + p.y));
    Vec2::new(x / n, y / n)
}
/// Slot centre in actor space, from the video frame's texture transform.
fn screen_centre(parts: &[EvaluatedPart]) -> Vec2 {
    let texture = part(parts, "heldprop_r_puhelin_screen")
        .texture
        .as_ref()
        .unwrap();
    let [x, y, w, h] = texture.rest_bounds;
    texture.transform.point(Vec2::new(x + 0.5 * w, y + 0.5 * h))
}
fn parts(script: &ValidatedScript, time: f64) -> Vec<EvaluatedPart> {
    script.evaluate(0, time).unwrap().actors[0]
        .rigid_parts
        .clone()
}
fn distance(a: Vec2, b: Vec2) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

#[test]
fn the_free_hand_touches_the_screen_exactly_at_each_tap() {
    let tapping = script("[0.5, 2.0]", |_| {});
    assert!(tapping.tap_diagnostics().is_empty());
    for tap in TAPS {
        let parts = parts(&tapping, tap);
        // The hand ellipse is centred 2 units from the hand joint, so the
        // joint sits on the slot centre exactly when this distance is 2.
        let d = distance(centroid(part(&parts, "hand_l")), screen_centre(&parts));
        assert!((d - 2.0).abs() < 1e-6, "tap {tap}: {d}");
        // Contact holds briefly, then releases.
        let held = self::parts(&tapping, tap + 0.5 * TAP_HOLD_SECONDS);
        let d = distance(centroid(part(&held, "hand_l")), screen_centre(&held));
        assert!((d - 2.0).abs() < 1e-6);
    }
    // Approaching: closer than at rest, not yet touching.
    let rest = parts(&tapping, 0.5);
    let near = parts(&tapping, TAPS[0] - 0.1);
    let gap = |p: &[EvaluatedPart]| distance(centroid(part(p, "hand_l")), screen_centre(p));
    let rest_gap = distance(
        centroid(part(&rest, "hand_l")),
        screen_centre(&parts(&tapping, 1.0)),
    );
    assert!(rest_gap > 30.0, "{rest_gap}");
    assert!(gap(&near) > 2.0 + 1e-3 && gap(&near) < rest_gap);
}

#[test]
fn nothing_moves_outside_the_press_windows() {
    let tapping = script("[0.5, 2.0]", |_| {});
    let still = script("[]", |_| {});
    let end = TAP_HOLD_SECONDS + motion_rig::TAP_RELEASE_SECONDS;
    for time in [
        0.0,
        1.0,
        TAPS[0] - TAP_APPROACH_SECONDS - 1e-3,
        TAPS[0] + end + 1e-3,
        2.5,
        TAPS[1] - TAP_APPROACH_SECONDS - 1e-3,
        TAPS[1] + end + 1e-3,
        4.5,
        5.0,
    ] {
        assert_eq!(
            tapping.evaluate(0, time).unwrap(),
            still.evaluate(0, time).unwrap(),
            "{time}"
        );
    }
    for time in [TAPS[0] - 0.1, TAPS[0], TAPS[1] + TAP_HOLD_SECONDS + 0.05] {
        assert_ne!(
            part(&parts(&tapping, time), "hand_l"),
            part(&parts(&still, time), "hand_l"),
            "{time}"
        );
        // Only the tapping arm moves; the phone stays put.
        assert_eq!(
            part(&parts(&tapping, time), "heldprop_r_puhelin_screen"),
            part(&parts(&still, time), "heldprop_r_puhelin_screen")
        );
    }
}

#[test]
fn seeking_matches_frame_by_frame_playback() {
    let tapping = script("[0.5, 2.0]", |_| {});
    let fps = 24;
    let frames = motion_scene::frame_count(5.0, fps, 1000).unwrap();
    let time = |frame| motion_scene::time::frame_to_seconds(frame, fps).unwrap();
    let played: Vec<_> = (0..frames)
        .map(|f| tapping.evaluate(0, time(f)).unwrap())
        .collect();
    // Seek backwards with a fresh evaluator, and through the uncached path.
    let fresh = script("[0.5, 2.0]", |_| {});
    for frame in (0..frames).rev() {
        assert_eq!(
            fresh.evaluate(0, time(frame)).unwrap(),
            played[frame as usize]
        );
    }
    for frame in [0, 35, 36, 72, 119] {
        let raw = motion_scene::evaluate(&source_script(), 0, time(frame)).unwrap();
        assert_eq!(raw, played[frame as usize], "frame {frame}");
    }
}
/// The same script as `script("[0.5, 2.0]")`, unvalidated.
fn source_script() -> motion_script::Script {
    let mut value = veikko();
    value["characters"]["VEIKKO"]["holds"] = yaml("{item: puhelin, hand: right}");
    let composition = &mut value["scenes"][0]["composition"];
    composition["joint_tracks"]["VEIKKO"]["upper_arm_r"] = yaml("[{at: 0, rotation: 1.1}]");
    composition["joint_tracks"]["VEIKKO"]["forearm_r"] = yaml("[{at: 0, rotation: 1.3}]");
    composition["screens"] = yaml(
        "[{actor: VEIKKO, slot: phone, at: 1.0, taps: [0.5, 2.0], \
         source: {type: video, asset: assets/screens/demo.mov, fps: 30, duration: 4}}]",
    );
    motion_script::parse(&serde_yaml::to_string(&value).unwrap()).unwrap()
}

#[test]
fn authored_keys_on_the_tapping_arm_take_precedence() {
    let keyed = |value: &mut serde_yaml::Value| {
        value["scenes"][0]["composition"]["joint_tracks"]["VEIKKO"]["forearm_l"] =
            yaml("[{at: 0, rotation: -0.4}, {at: 5, rotation: 0.2}]");
    };
    let tapping = script("[0.5, 2.0]", keyed);
    let still = script("[]", keyed);
    for time in [TAPS[0], TAPS[1]] {
        assert_eq!(
            tapping.evaluate(0, time).unwrap(),
            still.evaluate(0, time).unwrap()
        );
    }
    let diagnostics = tapping.tap_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].skip,
        TapSkip::AuthoredArm {
            joint: "forearm_l".into()
        }
    );
    assert!(diagnostics[0].to_string().contains("take precedence"));
}

#[test]
fn a_rig_without_a_free_arm_reports_a_typed_diagnostic() {
    // The left hand already holds a coffee cup: no hand is free to tap.
    let busy = |value: &mut serde_yaml::Value| {
        let rig: motion_rig::RigDefinition =
            serde_yaml::from_value(value["characters"]["VEIKKO"]["rig"].clone()).unwrap();
        let mut rig = rig;
        let catalog = motion_rig::stick_studio_hand_props().unwrap();
        let cup = catalog.items.iter().find(|i| i.id == "kahvikuppi").unwrap();
        motion_rig::attach_hand_prop(
            &mut rig,
            motion_rig::HandSide::Left,
            cup,
            Default::default(),
        )
        .unwrap();
        value["characters"]["VEIKKO"]["rig"] = serde_yaml::to_value(rig).unwrap();
    };
    let tapping = script("[0.5, 2.0]", busy);
    let still = script("[]", busy);
    let diagnostics = tapping.tap_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        (diagnostics[0].actor.as_str(), diagnostics[0].slot.as_str()),
        ("VEIKKO", "phone")
    );
    assert!(matches!(
        &diagnostics[0].skip,
        TapSkip::NoFreeArm(TapError::NoFreeArm { reason, .. }) if reason.contains("hand_l already holds")
    ));
    // Evaluation still succeeds and the arm is left alone.
    assert_eq!(
        tapping.evaluate(0, TAPS[0]).unwrap(),
        still.evaluate(0, TAPS[0]).unwrap()
    );
}
