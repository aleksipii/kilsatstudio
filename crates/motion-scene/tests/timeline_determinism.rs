//! A04: direct seek ≡ frame-by-frame playback, overlap order and time boundaries.
use motion_scene::time::{frame_to_sample, frame_to_seconds, sample_to_frame, sample_to_seconds};
use motion_scene::{frame_count, EvaluatedScene, SequenceTimeline, ValidatedScript};
use std::sync::Arc;

/// Touching moves, tied emotions, overlapping gestures, background and camera
/// keys, an actor track, fades and a time-triggered camera rule.
const SCRIPT: &str = include_str!("../../../examples/trackspec-v1/valid.yaml");

fn validated() -> ValidatedScript {
    ValidatedScript::new(Arc::new(motion_script::parse(SCRIPT).unwrap())).unwrap()
}

/// Debug output distinguishes every f64 bit pattern that matters here (including -0.0).
fn bits(scene: &EvaluatedScene) -> String {
    format!("{scene:?}")
}

#[test]
fn direct_seek_equals_frame_by_frame_playback_bitwise() {
    let script = motion_script::parse(SCRIPT).unwrap();
    let fps = script.project.fps;
    for (index, scene) in script.scenes.iter().enumerate() {
        let frames = frame_count(scene.duration, fps, 10_000).unwrap();
        // Playback: one long-lived evaluator stepping every frame in order.
        let player = validated();
        let played: Vec<_> = (0..frames)
            .map(|f| {
                bits(
                    &player
                        .evaluate(index, frame_to_seconds(f, fps).unwrap())
                        .unwrap(),
                )
            })
            .collect();
        // Seeks: fresh evaluators, scattered order, backwards scrubbing, other scenes first.
        let mut order: Vec<u64> = (0..frames).rev().collect();
        order.rotate_left(frames as usize / 3);
        let seeker = validated();
        let _ = seeker.evaluate(1 - index, 0.75).unwrap();
        for f in order {
            let time = frame_to_seconds(f, fps).unwrap();
            assert_eq!(
                bits(&seeker.evaluate(index, time).unwrap()),
                played[f as usize]
            );
            let fresh = validated();
            assert_eq!(
                bits(&fresh.evaluate(index, time).unwrap()),
                played[f as usize]
            );
            // Sample-addressed evaluation is the same function.
            let sample = frame_to_sample(f, fps).unwrap();
            assert_eq!(
                bits(&seeker.evaluate_sample(index, sample).unwrap()),
                played[f as usize]
            );
            // The one-shot API agrees with the prepared one.
            assert_eq!(
                bits(&motion_scene::evaluate(&script, index, time).unwrap()),
                played[f as usize]
            );
        }
    }
}

#[test]
fn sequence_global_seek_matches_stepping_through_the_sequence() {
    let script = motion_script::parse(SCRIPT).unwrap();
    let timeline = SequenceTimeline::compile(&script).unwrap();
    let player = validated();
    let fps = script.project.fps;
    let mut frame = 0;
    let mut stepped = Vec::new();
    loop {
        let global = frame_to_sample(frame, fps).unwrap();
        if global >= timeline.total_samples() {
            break;
        }
        let contributions = timeline.sample(global).unwrap();
        let scenes: Vec<_> = contributions
            .iter()
            .map(|c| {
                bits(
                    &player
                        .evaluate_sample(c.scene_index, c.local_sample)
                        .unwrap(),
                )
            })
            .collect();
        stepped.push((global, contributions, scenes));
        frame += 1;
    }
    // 4 s + 1.5 s with a cut: 5.5 s at 24 fps.
    assert_eq!(stepped.len(), 132);
    let seeker = validated();
    for (global, contributions, scenes) in stepped.iter().rev() {
        assert_eq!(&timeline.sample(*global).unwrap(), contributions);
        for (c, expected) in contributions.iter().zip(scenes) {
            let local = sample_to_seconds(c.local_sample);
            assert_eq!(
                &bits(&seeker.evaluate(c.scene_index, local).unwrap()),
                expected
            );
        }
    }
    // The scene boundary frame belongs to the next scene at local sample 0.
    let boundary = sample_to_frame(96_000, fps).unwrap();
    assert_eq!(boundary, 96);
    let (_, at_boundary, _) = &stepped[boundary as usize];
    assert_eq!(
        (at_boundary[0].scene_index, at_boundary[0].local_sample),
        (1, 0)
    );
    let (_, before, _) = &stepped[boundary as usize - 1];
    assert_eq!(before[0].scene_index, 0);
}

#[test]
fn time_boundaries() {
    let player = validated();
    let start = player.evaluate(0, 0.0).unwrap();
    assert_eq!(bits(&player.evaluate(0, -0.0).unwrap()), bits(&start));
    assert_eq!(start.time.to_bits(), 0.0_f64.to_bits());
    assert_eq!(start.opacity, 0.0);
    // The scene end is inclusive; anything past it, negative or non-finite is rejected.
    let end = player.evaluate(0, 4.0).unwrap();
    assert_eq!(end.opacity, 0.0);
    for bad in [
        4.0 + 1e-12,
        -1e-12,
        -1.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        assert!(
            matches!(
                player.evaluate(0, bad),
                Err(motion_scene::Error::InvalidTime)
            ),
            "{bad}"
        );
    }
    assert!(player.evaluate_sample(0, 96_001).is_err());
    assert!(player.evaluate_sample(0, 96_000).is_ok());
    assert!(matches!(
        player.evaluate(2, 0.0),
        Err(motion_scene::Error::MissingScene(2))
    ));
    // The last exported frame is strictly before the end.
    let frames = frame_count(4.0, 24, 1_000).unwrap();
    assert_eq!(frames, 96);
    assert!(frame_to_seconds(frames - 1, 24).unwrap() < 4.0);
}

fn emma(scene: &EvaluatedScene) -> &motion_scene::ActorPose {
    scene.actors.iter().find(|a| a.id == "EMMA").unwrap()
}

#[test]
fn touching_moves_hand_over_exact_endpoints() {
    let player = validated();
    // Each move starts at the previous move's resolved `to`.
    let at = |t: f64| {
        let pose = player.evaluate(0, t).unwrap();
        let actor = emma(&pose);
        (actor.x, actor.y)
    };
    assert_eq!(at(0.0), (960.0, 1080.0 * 0.7));
    assert_eq!(at(1.5), (500.3, 740.7));
    assert_eq!(at(2.125), (900.1, 700.9));
    assert_eq!(at(4.0), (333.3, 777.7));
    // Reference: the pre-A04 per-call chain over the same events (rounding tolerance).
    let script = motion_script::parse(SCRIPT).unwrap();
    for frame in 0..=96 {
        let t = frame as f64 / 24.0;
        let (mut x, mut y) = (960.0, 1080.0 * 0.7);
        for event in &script.scenes[0].events {
            if let motion_script::Event::Move {
                at,
                actor,
                x: tx,
                y: ty,
                duration,
            } = event
            {
                if actor == "EMMA" && *at <= t {
                    let p = ((t - at) / duration).clamp(0.0, 1.0);
                    x += (tx - x) * p;
                    y += (ty - y) * p;
                }
            }
        }
        // Pre-A04 rounding drifted by an ulp at endpoints; resolved spans are exact.
        let (rx, ry) = at(t);
        assert!(
            (rx - x).abs() <= 1e-9 && (ry - y).abs() <= 1e-9,
            "frame {frame}"
        );
    }
}

#[test]
fn overlap_order_for_step_and_envelope_properties() {
    let player = validated();
    // Equal-time emotions: the later written event wins.
    assert_eq!(emma(&player.evaluate(0, 1.0).unwrap()).emotion, "sad");
    assert_eq!(emma(&player.evaluate(0, 0.999).unwrap()).emotion, "neutral");
    // Background keys: the latest key at or before t.
    let color = |t: f64| player.evaluate(0, t).unwrap().background_color;
    assert_eq!(color(0.999), "#101010");
    assert_eq!(color(1.0), "#202020");
    assert_eq!(color(2.5), "#303030");
    // Overlapping gestures: the strongest envelope wins and finished ones revert to 0.
    let wave = |t: f64| emma(&player.evaluate(0, t).unwrap()).wave;
    let a = motion_animation::gesture_envelope((2.25 - 1.5) / 1.0);
    let b = motion_animation::gesture_envelope((2.25 - 2.0) / 1.0);
    assert_eq!(wave(2.25), a.max(b));
    assert_eq!(wave(3.5), 0.0);
}

#[test]
fn overlapping_moves_for_one_actor_are_rejected() {
    let overlapping = include_str!("../../../examples/trackspec-v1/invalid-overlap.yaml");
    let error = motion_script::parse(overlapping).unwrap_err().to_string();
    assert!(error.contains("overlapping move events"), "{error}");
}
