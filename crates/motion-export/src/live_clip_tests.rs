use super::*;
use std::sync::Arc;

const SOURCE: &str = r#"
project: {fps: 8, width: 256, height: 256}
characters: {EMMA: {asset: characters/emma}}
animation_clips:
  reach:
    duration: 2
    bone_tracks:
      right_hand:
        - {at: 0.5, x: 20, y: 6, scale_x: 1.375}
        - {at: 1.5, x: 36, y: 6, scale_x: 1.375}
      head: [{at: 0, rotation: -3}]
scenes:
  - id: intro
    duration: 1
    events: [{type: gesture, at: 0, actor: EMMA, value: wave, duration: 1}]
    composition:
      actor_tracks:
        EMMA: [{at: 0, x: 5}, {at: 1, x: 9}]
      clip_weights:
        EMMA: {posture: 0.4, walk: 0.2, gesture: 0.5}
      bone_tracks:
        EMMA:
          right_hand: [{at: 0, x: 4, y: 2, scale_x: 1.125}]
          head: [{at: 0, rotation: 3}]
          chest: [{at: 0, y: 3}]
      clip_instances:
        - {id: reach_a, clip: reach, actor: EMMA, at: 0.25, duration: 0.5, weight: 0.5}
"#;

fn rendered_for(script: &Script) -> RenderedScene {
    let (_, mut rendered) = crate::tests::fixture();
    let samples = (script.scenes[0].duration * SAMPLE_RATE as f64).round() as u64;
    rendered.manifest.total_frames = samples;
    rendered.audio.samples.resize(samples as usize, 0.0);
    rendered
}

fn assert_frames_and_preview(actual: &Script, expected: &Script, frame: u64) {
    let rendered = rendered_for(actual);
    let cancel = CancellationToken::new();
    for png in [false, true] {
        let encoded = encode_frame(actual, 0, &rendered, frame, png, &cancel, None)
            .unwrap()
            .2;
        assert_eq!(
            encoded,
            encode_frame(expected, 0, &rendered, frame, png, &cancel, None)
                .unwrap()
                .2,
            "frame {frame}, PNG={png}"
        );
        if png {
            let preview = ScenePreview::new(
                Arc::new(actual.clone()),
                0,
                Arc::new(ArtContext::default()),
                Some(Arc::new(rendered.manifest.clone())),
            )
            .unwrap();
            let pixels = resvg::tiny_skia::Pixmap::decode_png(&encoded).unwrap();
            let sample = frame * SAMPLE_RATE as u64 / actual.project.fps as u64;
            assert_eq!(
                preview.render(sample, 256, &cancel).unwrap().rgba,
                pixels.data()
            );
        }
    }
}

#[test]
fn live_windows_weights_and_retiming_match_independent_authored_frames() {
    let original = motion_script::parse(SOURCE).unwrap();
    let (_, rendered) = crate::tests::fixture();
    for weight in [0.0, 0.5, 1.0] {
        let mut actual = original.clone();
        actual.scenes[0].composition.clip_instances[0].weight = weight;
        let preview = ScenePreview::new(
            Arc::new(actual.clone()),
            0,
            Arc::new(ArtContext::default()),
            Some(Arc::new(rendered.manifest.clone())),
        )
        .unwrap();
        // Before/start/interior/end/after, with repeat and backwards seeks.
        for frame in [6, 2, 4, 1, 5, 3, 7, 2] {
            let mut expected = original.clone();
            expected.scenes[0].composition.clip_instances.clear();
            if (2..6).contains(&frame) && weight != 0.0 {
                // Independently authored samples: 2-second clip compressed into
                // [.25,.75), first/last key holding at .25/.625. These are not
                // calculated by the live sampler or an offset-blending helper.
                let x = if weight == 0.5 {
                    match frame {
                        2 | 3 => 12.0,
                        4 => 16.0,
                        5 => 20.0,
                        _ => unreachable!(),
                    }
                } else {
                    match frame {
                        2 | 3 => 20.0,
                        4 => 28.0,
                        5 => 36.0,
                        _ => unreachable!(),
                    }
                };
                let bones = expected.scenes[0]
                    .composition
                    .bone_tracks
                    .get_mut("EMMA")
                    .unwrap();
                let hand = &mut bones.get_mut("right_hand").unwrap()[0];
                hand.x = x;
                hand.y = if weight == 0.5 { 4.0 } else { 6.0 };
                hand.scale_x = if weight == 0.5 { 1.25 } else { 1.375 };
                bones.get_mut("head").unwrap()[0].rotation = if weight == 0.5 {
                    std::f64::consts::PI
                } else {
                    -3.0
                };
            }
            assert_frames_and_preview(&actual, &expected, frame);
            let cancel = CancellationToken::new();
            let png = encode_frame(&expected, 0, &rendered, frame, true, &cancel, None)
                .unwrap()
                .2;
            let pixels = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
            assert_eq!(
                preview
                    .render(frame * SAMPLE_RATE as u64 / 8, 256, &cancel)
                    .unwrap()
                    .rgba,
                pixels.data()
            );
        }
    }
}

#[test]
fn overlapping_live_layers_preserve_missing_bones_and_change_pixels_in_source_order() {
    let mut actual = motion_script::parse(SOURCE).unwrap();
    actual.animation_clips.get_mut("reach").unwrap().bone_tracks = BTreeMap::from([(
        "right_hand".into(),
        vec![motion_script::ActorKey {
            x: 20.0,
            ..Default::default()
        }],
    )]);
    let mut second = actual.animation_clips["reach"].clone();
    second.bone_tracks.get_mut("right_hand").unwrap()[0].x = 36.0;
    actual.animation_clips.insert("farther".into(), second);
    let mut instance = actual.scenes[0].composition.clip_instances[0].clone();
    instance.id = "reach_b".into();
    instance.clip = "farther".into();
    actual.scenes[0].composition.clip_instances.push(instance);
    let mut expected = actual.clone();
    expected.scenes[0].composition.clip_instances.clear();
    let hand = &mut expected.scenes[0]
        .composition
        .bone_tracks
        .get_mut("EMMA")
        .unwrap()
        .get_mut("right_hand")
        .unwrap()[0];
    // Base (4,2,1.125), then two half overrides (20,0,1), (36,0,1).
    hand.x = 24.0;
    hand.y = 0.5;
    hand.scale_x = 1.03125;
    assert_frames_and_preview(&actual, &expected, 4);
    let mut reversed = actual.clone();
    reversed.scenes[0].composition.clip_instances.reverse();
    expected.scenes[0]
        .composition
        .bone_tracks
        .get_mut("EMMA")
        .unwrap()
        .get_mut("right_hand")
        .unwrap()[0]
        .x = 20.0;
    assert_frames_and_preview(&reversed, &expected, 4);
    let (_, rendered) = crate::tests::fixture();
    let cancel = CancellationToken::new();
    for png in [false, true] {
        assert_ne!(
            encode_frame(&actual, 0, &rendered, 4, png, &cancel, None)
                .unwrap()
                .2,
            encode_frame(&reversed, 0, &rendered, 4, png, &cancel, None)
                .unwrap()
                .2
        );
    }
}

#[test]
fn live_library_replacement_survives_project_roundtrip_without_mutating_copied_tracks() {
    let mut script = motion_script::parse(SOURCE).unwrap();
    let mut copied = script.scenes[0].clone();
    copied.id = "copied".into();
    copied.composition.clip_instances.clear();
    script.scenes.push(copied);
    let source = motion_script::apply_animation_clip_retimed(
        &serde_json::to_string(&script).unwrap(),
        1,
        "EMMA",
        "reach",
        0.25,
        0.5,
    )
    .unwrap();
    let document = motion_project::ProjectDocument::new(source.clone()).unwrap();
    let loaded = motion_project::decode(&motion_project::encode(&document).unwrap()).unwrap();
    assert_eq!(loaded.source, source);
    let before = motion_script::parse(&loaded.source).unwrap();
    let replacement =
        motion_script::capture_animation_clip(&loaded.source, 0, "EMMA", "reach", true).unwrap();
    let after = motion_script::parse(&replacement).unwrap();
    assert_eq!(
        before.scenes[1].composition.bone_tracks,
        after.scenes[1].composition.bone_tracks
    );
    assert_eq!(
        before.scenes[0].composition.clip_instances,
        after.scenes[0].composition.clip_instances
    );
    // Replacement captures the base offsets. Blending them over themselves is
    // now identical to manually removing the live layer at this sample.
    let mut expected = before.clone();
    expected.scenes[0].composition.clip_instances.clear();
    assert_frames_and_preview(&after, &expected, 4);
    let (_, rendered) = crate::tests::fixture();
    let cancel = CancellationToken::new();
    for png in [false, true] {
        assert_ne!(
            encode_frame(&before, 0, &rendered, 4, png, &cancel, None)
                .unwrap()
                .2,
            encode_frame(&after, 0, &rendered, 4, png, &cancel, None)
                .unwrap()
                .2
        );
        assert_eq!(
            encode_frame(&before, 1, &rendered, 4, png, &cancel, None)
                .unwrap()
                .2,
            encode_frame(&after, 1, &rendered, 4, png, &cancel, None)
                .unwrap()
                .2
        );
    }
}

fn enveloped_script() -> Script {
    let mut script = motion_script::parse(SOURCE).unwrap();
    script.scenes[0].duration = 1.25;
    script.animation_clips.get_mut("reach").unwrap().bone_tracks = BTreeMap::from([(
        "right_hand".into(),
        vec![motion_script::ActorKey {
            x: 20.0,
            ..Default::default()
        }],
    )]);
    let instance = &mut script.scenes[0].composition.clip_instances[0];
    instance.at = 0.0;
    instance.duration = 1.0;
    instance.fade_in = 0.25;
    instance.fade_out = 0.25;
    instance.weight_keys = [(0.0, 0.25), (0.5, 0.75), (1.0, 0.25)]
        .into_iter()
        .map(|(time, weight)| motion_script::InstanceWeightKey {
            time,
            weight,
            easing: motion_script::ActorEasing::Linear,
        })
        .collect();
    script
}

#[test]
fn faded_animated_weights_match_authored_pixels_after_roundtrip_and_repeated_seeks() {
    let script = enveloped_script();
    let document =
        motion_project::ProjectDocument::new(serde_json::to_string(&script).unwrap()).unwrap();
    let loaded = motion_project::decode(&motion_project::encode(&document).unwrap()).unwrap();
    let actual = motion_script::parse(&loaded.source).unwrap();
    assert_eq!(
        actual.scenes[0].composition.clip_instances,
        script.scenes[0].composition.clip_instances
    );
    let rendered = rendered_for(&actual);
    let preview = ScenePreview::new(
        Arc::new(actual.clone()),
        0,
        Arc::new(ArtContext::default()),
        Some(Arc::new(rendered.manifest.clone())),
    )
    .unwrap();
    // Static .5 times the independently interpolated multiplier, times both
    // fades. Binary fractions keep exact authored offsets reproducible.
    // Start/end have zero influence, so the authored base survives each edge.
    for (frame, x, y, scale_x) in [
        (8, 4.0, 2.0, 1.125),
        (0, 4.0, 2.0, 1.125),
        (4, 10.0, 1.25, 1.078125),
        (2, 8.0, 1.5, 1.09375),
        (6, 8.0, 1.5, 1.09375),
        (1, 5.5, 1.8125, 1.11328125),
        (4, 10.0, 1.25, 1.078125),
    ] {
        let mut expected = actual.clone();
        expected.scenes[0].composition.clip_instances.clear();
        let hand = &mut expected.scenes[0]
            .composition
            .bone_tracks
            .get_mut("EMMA")
            .unwrap()
            .get_mut("right_hand")
            .unwrap()[0];
        hand.x = x;
        hand.y = y;
        hand.scale_x = scale_x;
        assert_frames_and_preview(&actual, &expected, frame);
        let cancel = CancellationToken::new();
        let png = encode_frame(&expected, 0, &rendered, frame, true, &cancel, None)
            .unwrap()
            .2;
        assert_eq!(
            preview
                .render(frame * SAMPLE_RATE as u64 / 8, 256, &cancel)
                .unwrap()
                .rgba,
            resvg::tiny_skia::Pixmap::decode_png(&png).unwrap().data()
        );
    }
}

#[test]
fn animated_overlaps_keep_source_order_and_default_envelopes_keep_existing_frames() {
    let original = motion_script::parse(SOURCE).unwrap();
    let explicit = motion_script::parse(&SOURCE.replace(
        "weight: 0.5}",
        "weight: 0.5, fade_in: 0, fade_out: 0, weight_keys: []}",
    ))
    .unwrap();
    for frame in [1, 2, 4, 6] {
        assert_frames_and_preview(&explicit, &original, frame);
    }
    let mut actual = enveloped_script();
    let mut second = actual.animation_clips["reach"].clone();
    second.bone_tracks.get_mut("right_hand").unwrap()[0].x = 36.0;
    actual.animation_clips.insert("farther".into(), second);
    let mut instance = actual.scenes[0].composition.clip_instances[0].clone();
    instance.id = "reach_b".into();
    instance.clip = "farther".into();
    instance.weight_keys.truncate(1);
    instance.weight_keys[0].weight = 0.5;
    actual.scenes[0].composition.clip_instances.push(instance);
    let mut expected = actual.clone();
    expected.scenes[0].composition.clip_instances.clear();
    let hand = &mut expected.scenes[0]
        .composition
        .bone_tracks
        .get_mut("EMMA")
        .unwrap()
        .get_mut("right_hand")
        .unwrap()[0];
    // At .5 seconds: first weight .375, second .25. Authored base is
    // (4,2,1.125), clip targets (20,0,1) and (36,0,1).
    hand.x = 16.5;
    hand.y = 0.9375;
    hand.scale_x = 1.05859375;
    assert_frames_and_preview(&actual, &expected, 4);
    let mut reversed = actual.clone();
    reversed.scenes[0].composition.clip_instances.reverse();
    expected.scenes[0]
        .composition
        .bone_tracks
        .get_mut("EMMA")
        .unwrap()
        .get_mut("right_hand")
        .unwrap()[0]
        .x = 15.0;
    assert_frames_and_preview(&reversed, &expected, 4);
    let (_, rendered) = crate::tests::fixture();
    let cancel = CancellationToken::new();
    for png in [false, true] {
        assert_ne!(
            encode_frame(&actual, 0, &rendered, 4, png, &cancel, None)
                .unwrap()
                .2,
            encode_frame(&reversed, 0, &rendered, 4, png, &cancel, None)
                .unwrap()
                .2
        );
    }
}

#[test]
fn looping_masked_instances_match_authored_frames_across_cycles_and_project_roundtrip() {
    let mut script = motion_script::parse(SOURCE).unwrap();
    script.scenes[0].duration = 5.0;
    // A leaf joint's rotation does not move its plotted skeleton point. Use
    // translation so the excluded head track also changes actual pixels.
    let clip = script.animation_clips.get_mut("reach").unwrap();
    clip.bone_tracks.get_mut("head").unwrap()[0].x = 24.0;
    let layer = &mut script.scenes[0].composition.clip_instances[0];
    layer.duration = 4.5;
    layer.looping = true;
    layer.joint_mask = vec!["right_hand".into()];
    let document =
        motion_project::ProjectDocument::new(serde_json::to_string(&script).unwrap()).unwrap();
    let loaded = motion_project::decode(&motion_project::encode(&document).unwrap()).unwrap();
    let actual = motion_script::parse(&loaded.source).unwrap();
    assert_eq!(
        actual.scenes[0].composition.clip_instances,
        script.scenes[0].composition.clip_instances
    );
    // Original clip duration two seconds, instance starts at .25. The mask
    // deliberately excludes the clip's head translation. Samples cross two cycle
    // resets, run backwards, repeat and include the half-open instance endpoint.
    for (frame, x) in [
        (38, None),
        (2, Some(12.0)),
        (10, Some(16.0)),
        (18, Some(12.0)),
        (26, Some(16.0)),
        (34, Some(12.0)),
        (14, Some(20.0)),
        (6, Some(12.0)),
        (10, Some(16.0)),
    ] {
        let mut expected = actual.clone();
        expected.scenes[0].composition.clip_instances.clear();
        if let Some(x) = x {
            let hand = &mut expected.scenes[0]
                .composition
                .bone_tracks
                .get_mut("EMMA")
                .unwrap()
                .get_mut("right_hand")
                .unwrap()[0];
            hand.x = x;
            hand.y = 4.0;
            hand.scale_x = 1.25;
        }
        assert_frames_and_preview(&actual, &expected, frame);
    }
    let mut all_joints = actual.clone();
    all_joints.scenes[0].composition.clip_instances[0]
        .joint_mask
        .clear();
    let rendered = rendered_for(&actual);
    let cancel = CancellationToken::new();
    for png in [false, true] {
        assert_ne!(
            encode_frame(&actual, 0, &rendered, 10, png, &cancel, None)
                .unwrap()
                .2,
            encode_frame(&all_joints, 0, &rendered, 10, png, &cancel, None)
                .unwrap()
                .2
        );
    }
}

#[test]
fn omitted_loop_and_mask_keep_existing_pixels_and_explicit_all_joint_selection_matches() {
    let original = motion_script::parse(SOURCE).unwrap();
    let explicit = motion_script::parse(&SOURCE.replace(
        "weight: 0.5}",
        "weight: 0.5, looping: false, joint_mask: []}",
    ))
    .unwrap();
    let mut selected = explicit.clone();
    selected.scenes[0].composition.clip_instances[0].joint_mask = motion_script::HUMANOID_BONES
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    for frame in [0, 2, 4, 5, 6] {
        assert_frames_and_preview(&explicit, &original, frame);
        assert_frames_and_preview(&selected, &original, frame);
    }
}
