use super::*;
use std::sync::Arc;

fn fixture() -> (Script, RenderedScene) {
    let (_, rendered) = crate::tests::fixture();
    let script = motion_script::parse(
        "project: {fps: 4, width: 256, height: 256}\ncharacters: {EMMA: {asset: characters/emma}}\nscenes:\n  - id: intro\n    duration: 1\n    events: [{type: gesture, at: 0, actor: EMMA, value: wave, duration: 1}]\n",
    )
    .unwrap();
    (script, rendered)
}

#[test]
fn explicit_default_weights_preserve_geometry_and_encoded_frames() {
    let (baseline, rendered) = fixture();
    let mut explicit = baseline.clone();
    explicit.scenes[0]
        .composition
        .clip_weights
        .insert("EMMA".into(), motion_script::ClipWeights::default());
    let cancel = CancellationToken::new();
    for frame in 0..4 {
        let time = frame as f64 / 4.0;
        assert_eq!(
            motion_scene::evaluate(&baseline, 0, time).unwrap().actors,
            motion_scene::evaluate(&explicit, 0, time).unwrap().actors
        );
        for png in [false, true] {
            assert_eq!(
                encode_frame(&baseline, 0, &rendered, frame, png, &cancel, None)
                    .unwrap()
                    .2,
                encode_frame(&explicit, 0, &rendered, frame, png, &cancel, None)
                    .unwrap()
                    .2
            );
        }
    }
}

#[test]
fn gesture_suppression_survives_export_and_preview_without_removing_authored_offsets() {
    let (baseline, rendered) = fixture();
    let mut weighted = baseline.clone();
    weighted.scenes[0].composition.clip_weights.insert(
        "EMMA".into(),
        motion_script::ClipWeights {
            posture: 0.4,
            walk: 0.2,
            gesture: 0.0,
        },
    );
    let mut authored = weighted.clone();
    authored.scenes[0].composition.bone_tracks.insert(
        "EMMA".into(),
        BTreeMap::from([(
            "right_hand".into(),
            vec![motion_script::ActorKey {
                x: 12.0,
                ..Default::default()
            }],
        )]),
    );
    let before = motion_scene::evaluate(&weighted, 0, 0.5).unwrap();
    let after = motion_scene::evaluate(&authored, 0, 0.5).unwrap();
    let hand = motion_script::HUMANOID_BONES
        .iter()
        .position(|name| *name == "right_hand")
        .unwrap();
    let head = motion_script::HUMANOID_BONES
        .iter()
        .position(|name| *name == "head")
        .unwrap();
    assert_eq!(after.actors[0].wave, 0.0);
    assert_eq!(
        after.actors[0].skeleton[hand].x,
        before.actors[0].skeleton[hand].x + 12.0
    );
    assert_eq!(
        after.actors[0].skeleton[head],
        before.actors[0].skeleton[head]
    );
    assert_eq!(after.actors[0].x, before.actors[0].x);
    assert_eq!(after.actors[0].emotion, before.actors[0].emotion);

    let cancel = CancellationToken::new();
    // Active wave at its midpoint: the user-facing switch must affect real pixels.
    for png in [false, true] {
        assert_ne!(
            encode_frame(&baseline, 0, &rendered, 2, png, &cancel, None)
                .unwrap()
                .2,
            encode_frame(&weighted, 0, &rendered, 2, png, &cancel, None)
                .unwrap()
                .2
        );
    }

    let art = ArtContext::default();
    let preview = ScenePreview::new(
        Arc::new(authored.clone()),
        0,
        Arc::new(art.clone()),
        Some(Arc::new(rendered.manifest.clone())),
    )
    .unwrap();
    let svg_dir = crate::tests::destination();
    let png_dir = crate::tests::destination();
    export_scene_with_art(&authored, 0, &rendered, &art, &svg_dir, &cancel, |_| {}).unwrap();
    export_png_scene_with_art(&authored, 0, &rendered, &art, &png_dir, &cancel, |_| {}).unwrap();
    // Includes pre-speech, both mouth intervals, and their shared sample clock.
    for frame in 0..4 {
        let sample = frame * SAMPLE_RATE as u64 / authored.project.fps as u64;
        let png = fs::read(png_dir.join(format!("frame_{frame:06}.png"))).unwrap();
        let svg = fs::read(svg_dir.join(format!("frame_{frame:06}.svg"))).unwrap();
        let svg_png = media::rasterize(std::str::from_utf8(&svg).unwrap()).unwrap();
        let exported = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
        let from_svg = resvg::tiny_skia::Pixmap::decode_png(&svg_png).unwrap();
        assert_eq!(exported.data(), from_svg.data());
        assert_eq!(
            preview.render(sample, 256, &cancel).unwrap().rgba,
            exported.data()
        );
    }
    fs::remove_dir_all(svg_dir).unwrap();
    fs::remove_dir_all(png_dir).unwrap();
}

fn gesture_weights(gesture: f64) -> motion_script::ClipWeights {
    motion_script::ClipWeights {
        gesture,
        ..Default::default()
    }
}

fn assert_track_frame_matches_static(
    animated: &Script,
    rendered: &RenderedScene,
    frame: u64,
    expected_gesture: f64,
) {
    let mut expected = animated.clone();
    expected.scenes[0].composition.clip_weight_tracks.clear();
    expected.scenes[0]
        .composition
        .clip_weights
        .insert("EMMA".into(), gesture_weights(expected_gesture));
    let cancel = CancellationToken::new();
    for png in [false, true] {
        assert_eq!(
            encode_frame(animated, 0, rendered, frame, png, &cancel, None)
                .unwrap()
                .2,
            encode_frame(&expected, 0, rendered, frame, png, &cancel, None)
                .unwrap()
                .2,
            "frame {frame}, PNG={png}"
        );
    }
}

#[test]
fn animated_strength_exports_match_static_samples_and_random_seek_preview() {
    let (mut animated, rendered) = fixture();
    animated.project.fps = 8;
    animated.scenes[0].composition.clip_weight_tracks.insert(
        "EMMA".into(),
        vec![
            motion_script::ClipWeightKeyframe {
                time: 0.25,
                weights: gesture_weights(0.0),
                easing: motion_script::ActorEasing::Linear,
            },
            motion_script::ClipWeightKeyframe {
                time: 0.75,
                weights: gesture_weights(1.0),
                easing: motion_script::ActorEasing::Linear,
            },
        ],
    );
    let cancel = CancellationToken::new();
    let art = ArtContext::default();
    let preview = ScenePreview::new(
        Arc::new(animated.clone()),
        0,
        Arc::new(art.clone()),
        Some(Arc::new(rendered.manifest.clone())),
    )
    .unwrap();
    let svg_dir = crate::tests::destination();
    let png_dir = crate::tests::destination();
    export_scene_with_art(&animated, 0, &rendered, &art, &svg_dir, &cancel, |_| {}).unwrap();
    export_png_scene_with_art(&animated, 0, &rendered, &art, &png_dir, &cancel, |_| {}).unwrap();
    // Seek backwards and repeat frames, including both speech interval boundaries.
    for frame in [5, 1, 7, 2, 4, 0, 6, 3, 5, 2] {
        let time = frame as f64 / 8.0;
        let strength = ((time - 0.25) / 0.5).clamp(0.0, 1.0);
        assert_track_frame_matches_static(&animated, &rendered, frame, strength);
        let sample = frame * SAMPLE_RATE as u64 / 8;
        let png = fs::read(png_dir.join(format!("frame_{frame:06}.png"))).unwrap();
        let svg = fs::read(svg_dir.join(format!("frame_{frame:06}.svg"))).unwrap();
        let rasterized = media::rasterize(std::str::from_utf8(&svg).unwrap()).unwrap();
        let exported = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
        let from_svg = resvg::tiny_skia::Pixmap::decode_png(&rasterized).unwrap();
        assert_eq!(exported.data(), from_svg.data());
        assert_eq!(
            preview.render(sample, 256, &cancel).unwrap().rgba,
            exported.data()
        );
    }
    let mut unweighted = animated.clone();
    unweighted.scenes[0].composition.clip_weight_tracks.clear();
    // Compare at the same gesture-envelope time: the track must affect actual pixels.
    assert_ne!(
        encode_frame(&animated, 0, &rendered, 4, true, &cancel, None)
            .unwrap()
            .2,
        encode_frame(&unweighted, 0, &rendered, 4, true, &cancel, None)
            .unwrap()
            .2
    );
    fs::remove_dir_all(svg_dir).unwrap();
    fs::remove_dir_all(png_dir).unwrap();
}

#[test]
fn animated_strength_uses_outgoing_hold_and_smoothstep_in_encoded_frames() {
    let (mut animated, rendered) = fixture();
    animated.project.fps = 8;
    animated.scenes[0].composition.clip_weight_tracks.insert(
        "EMMA".into(),
        vec![
            motion_script::ClipWeightKeyframe {
                time: 0.0,
                weights: gesture_weights(0.0),
                easing: motion_script::ActorEasing::Hold,
            },
            motion_script::ClipWeightKeyframe {
                time: 0.5,
                weights: gesture_weights(0.25),
                easing: motion_script::ActorEasing::Smoothstep,
            },
            motion_script::ClipWeightKeyframe {
                time: 1.0,
                weights: gesture_weights(1.0),
                easing: motion_script::ActorEasing::Linear,
            },
        ],
    );
    for (frame, expected) in [(1, 0.0), (3, 0.0), (4, 0.25), (5, 0.3671875), (6, 0.625)] {
        assert_track_frame_matches_static(&animated, &rendered, frame, expected);
    }
}

#[test]
fn removing_strength_track_restores_static_fallback_encoded_frames() {
    let (mut baseline, rendered) = fixture();
    baseline.scenes[0]
        .composition
        .clip_weights
        .insert("EMMA".into(), gesture_weights(0.375));
    let source = serde_json::to_string(&baseline).unwrap();
    let tracked = motion_script::apply_clip_weight_track(
        &source,
        0,
        "EMMA",
        vec![motion_script::ClipWeightKeyframe {
            time: 0.0,
            weights: gesture_weights(0.0),
            easing: motion_script::ActorEasing::Linear,
        }],
    )
    .unwrap();
    let removed = motion_script::apply_clip_weight_track(&tracked, 0, "EMMA", Vec::new()).unwrap();
    let empty_track = motion_script::parse(&removed).unwrap();
    let cancel = CancellationToken::new();
    for frame in [2, 0, 3, 1] {
        for png in [false, true] {
            assert_eq!(
                encode_frame(&baseline, 0, &rendered, frame, png, &cancel, None)
                    .unwrap()
                    .2,
                encode_frame(&empty_track, 0, &rendered, frame, png, &cancel, None)
                    .unwrap()
                    .2
            );
        }
    }
}
