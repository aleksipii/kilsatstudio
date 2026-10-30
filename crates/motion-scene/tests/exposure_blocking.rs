//! P01: exposure (on ones/twos/threes) and stepped blocking are applied once,
//! before evaluation, so preview, seek and export show the same poses.
use motion_scene::time::{exposed_seconds, frame_to_sample, frame_to_seconds};
use motion_scene::{ActorPose, EvaluatedScene, ValidatedScript};
use motion_script::timing::{Exposure, Interpolation};
use std::sync::Arc;

/// Two actors with eased root tracks and a move; `{timing}` is spliced into
/// the scene composition and `{character}` into actor A.
fn source(timing: &str, character: &str) -> String {
    format!(
        r#"
project: {{title: Exposure, fps: 24, width: 640, height: 360}}
characters:
  A: {{asset: characters/a{character}}}
  B: {{asset: characters/b}}
scenes:
  - id: one
    duration: 2.0
    composition:
      {timing}
      actor_tracks:
        A:
          - {{at: 0.0, x: 0, easing: ease_in_out}}
          - {{at: 1.0, x: 240, rotation: 0.5, easing: back(1.3)}}
          - {{at: 2.0, x: 0, easing: linear}}
        B:
          - {{at: 0.0, y: 0, easing: ease_in}}
          - {{at: 2.0, y: 96, easing: ease_out}}
    events:
      - {{type: move, at: 0.0, actor: B, x: 100, y: 200, duration: 2.0}}
  - id: two
    duration: 1.0
    events: []
"#
    )
}

fn validated(source: &str) -> ValidatedScript {
    ValidatedScript::new(Arc::new(motion_script::parse(source).unwrap())).unwrap()
}

fn actor<'a>(scene: &'a EvaluatedScene, id: &str) -> &'a ActorPose {
    scene.actors.iter().find(|actor| actor.id == id).unwrap()
}

fn frames(script: &ValidatedScript, scene: usize, count: u64) -> Vec<EvaluatedScene> {
    (0..count)
        .map(|f| {
            script
                .evaluate(scene, frame_to_seconds(f, 24).unwrap())
                .unwrap()
        })
        .collect()
}

#[test]
fn on_twos_odd_frames_repeat_their_predecessor() {
    let script = validated(&source("timing: {on: 2}", ""));
    let played = frames(&script, 0, 48);
    for f in (1..48).step_by(2) {
        let (even, odd) = (&played[f - 1], &played[f]);
        assert_eq!(odd.actors, even.actors, "frame {f}");
        // The scene clock itself is not quantized.
        assert_ne!(odd.time, even.time);
    }
    // Animation still advances between groups.
    for f in (2..48).step_by(2) {
        assert_ne!(
            actor(&played[f], "A").x,
            actor(&played[f - 2], "A").x,
            "{f}"
        );
    }
}

#[test]
fn on_threes_hold_each_group_of_three() {
    let script = validated(&source("timing: {on: 3}", ""));
    let played = frames(&script, 0, 48);
    for f in 0..48 {
        assert_eq!(played[f].actors, played[f - f % 3].actors, "frame {f}");
    }
}

#[test]
fn direct_seek_equals_frame_by_frame_playback_with_exposure() {
    let text = source(
        "timing: {on: 2, interpolation: stepped}",
        ", timing: {on: 3}",
    );
    let played: Vec<_> = frames(&validated(&text), 0, 48)
        .iter()
        .map(|scene| format!("{scene:?}"))
        .collect();
    let seeker = validated(&text);
    let _ = seeker.evaluate(1, 0.5).unwrap();
    let mut order: Vec<u64> = (0..48).rev().collect();
    order.rotate_left(17);
    for f in order {
        let seek = seeker
            .evaluate(0, frame_to_seconds(f, 24).unwrap())
            .unwrap();
        assert_eq!(format!("{seek:?}"), played[f as usize], "frame {f}");
    }
    // Raw evaluation shares the same quantization.
    let raw = motion_script::parse(&text).unwrap();
    for f in [0, 1, 5, 23, 47] {
        let seek = motion_scene::evaluate(&raw, 0, frame_to_seconds(f, 24).unwrap()).unwrap();
        assert_eq!(format!("{seek:?}"), played[f as usize], "frame {f}");
    }
}

#[test]
fn preview_samples_and_export_frames_share_one_quantization() {
    // Export evaluates frame seconds; preview evaluates 24 kHz samples, which
    // may fall anywhere inside a frame. Both land on the exposure group start.
    let script = validated(&source("timing: {on: 2}", ""));
    for f in 0..48u64 {
        let export = script
            .evaluate(0, frame_to_seconds(f, 24).unwrap())
            .unwrap();
        let first = frame_to_sample(f, 24).unwrap();
        for sample in [first, first + 1, first + 500, first + 999] {
            let preview = script.evaluate_sample(0, sample).unwrap();
            assert_eq!(preview.actors, export.actors, "frame {f} sample {sample}");
        }
        let start = exposed_seconds(frame_to_seconds(f, 24).unwrap(), 24, Exposure::TWOS);
        assert_eq!(start.unwrap(), frame_to_seconds(f - f % 2, 24).unwrap());
    }
}

#[test]
fn absent_timing_and_on_ones_evaluate_exactly_as_before() {
    let plain = validated(&source("", ""));
    let ones = validated(&source("timing: {on: 1, interpolation: spline}", ""));
    for time in [0.0, 0.01, 0.333, 0.75, 1.0, 1.4999, 2.0] {
        let a = plain.evaluate(0, time).unwrap();
        assert_eq!(
            format!("{a:?}"),
            format!("{:?}", ones.evaluate(0, time).unwrap())
        );
        assert_eq!(a.time, time);
    }
    // On ones, times between frames are not snapped.
    assert_ne!(
        actor(&plain.evaluate(0, 0.5).unwrap(), "A").x,
        actor(&plain.evaluate(0, 0.5 + 1.0 / 48.0).unwrap(), "A").x
    );
    assert_eq!(exposed_seconds(0.0123, 24, Exposure::ONES).unwrap(), 0.0123);
}

#[test]
fn stepped_blocking_holds_keys_without_changing_stored_data() {
    let text = source("timing: {interpolation: stepped}", "");
    let parsed = motion_script::parse(&text).unwrap();
    let script = validated(&text);
    let at = |time: f64| actor(&script.evaluate(0, time).unwrap(), "A").clone();
    let base = at(0.0);
    assert_eq!(at(0.99).x, base.x);
    assert_eq!(at(1.0).x, base.x + 240.0);
    assert_eq!(at(1.6).x, base.x + 240.0);
    assert_eq!(at(1.6).rotation, 0.5);
    // Spline evaluation of the same keys moves in between (and overshoots).
    let spline = validated(&source("", ""));
    let a = |time: f64| actor(&spline.evaluate(0, time).unwrap(), "A").x;
    assert!(a(0.5) > base.x && a(0.5) < base.x + 240.0);
    // Back eases toward the next key (x 0) and passes it before settling.
    let lowest = (0..=100)
        .map(|i| a(1.0 + i as f64 / 100.0))
        .fold(f64::MAX, f64::min);
    assert!(lowest < base.x, "back overshoot passes the key");
    assert_eq!(a(2.0), base.x);
    // Stored keys keep their easing; only the scene timing is added.
    let keys = &parsed.scenes[0].composition.actor_tracks["A"];
    assert_eq!(keys[0].easing, motion_script::ActorEasing::EaseInOut);
    assert_eq!(keys[1].easing, motion_script::ActorEasing::Back(1.3));
    let round_trip = motion_script::parse(&motion_script::to_source(&parsed).unwrap()).unwrap();
    assert_eq!(
        round_trip.scenes[0].composition,
        parsed.scenes[0].composition
    );
    assert_eq!(
        round_trip.scenes[0].composition.timing.interpolation,
        Some(Interpolation::Stepped)
    );
}

#[test]
fn character_timing_overrides_the_scene_per_actor() {
    // Scene on twos and stepped; A is on ones with spline curves.
    let text = source(
        "timing: {on: 2, interpolation: stepped}",
        ", timing: {on: 1, interpolation: spline}",
    );
    let script = validated(&text);
    let played = frames(&script, 0, 24);
    for f in (1..24).step_by(2) {
        assert_ne!(
            actor(&played[f], "A").x,
            actor(&played[f - 1], "A").x,
            "{f}"
        );
        assert_eq!(
            actor(&played[f], "B").y,
            actor(&played[f - 1], "B").y,
            "{f}"
        );
        assert_eq!(
            actor(&played[f], "B").x,
            actor(&played[f - 1], "B").x,
            "{f}"
        );
    }
}

#[test]
fn scene_camera_and_fades_stay_on_ones() {
    let text = source("timing: {on: 2}\n      fade_in: 1.0", "");
    let played = frames(&validated(&text), 0, 24);
    for f in (1..24).step_by(2) {
        assert!(played[f].opacity > played[f - 1].opacity, "{f}");
    }
}
