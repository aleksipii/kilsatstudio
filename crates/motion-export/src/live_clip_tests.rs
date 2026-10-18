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

fn assert_frames_and_preview(actual: &Script, expected: &Script, frame: u64) {
    let (_, rendered) = crate::tests::fixture();
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
