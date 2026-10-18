use super::*;
use std::sync::Arc;

const SOURCE: &str = "project: {fps: 4, width: 256, height: 256}\ncharacters: {EMMA: {asset: characters/emma}}\nscenes:\n  - id: source\n    duration: 1\n    events: []\n    composition:\n      bone_tracks:\n        EMMA:\n          right_hand: [{at: 0, x: 0}, {at: 0.75, x: 24}]\n          chest: [{at: 0, y: 3}]\n  - id: intro\n    duration: 1\n    events: [{type: gesture, at: 0, actor: EMMA, value: wave, duration: 1}]\n    composition:\n      bone_tracks:\n        EMMA:\n          head: [{at: 0, x: -20}]\n";

fn applied_fixture() -> (Script, Script, RenderedScene, String) {
    let (_, rendered) = crate::tests::fixture();
    let captured =
        motion_script::capture_animation_clip(SOURCE, 0, "EMMA", "reach", false).unwrap();
    let applied = motion_script::apply_animation_clip(&captured, 1, "EMMA", "reach").unwrap();
    // An independent expected target: do not derive its tracks from the library
    // or the capture/apply helpers under test.
    let mut expected = motion_script::parse(SOURCE).unwrap();
    expected.scenes[1].composition.bone_tracks.insert(
        "EMMA".into(),
        BTreeMap::from([
            (
                "right_hand".into(),
                vec![
                    motion_script::ActorKey::default(),
                    motion_script::ActorKey {
                        at: 0.75,
                        x: 24.0,
                        ..Default::default()
                    },
                ],
            ),
            (
                "chest".into(),
                vec![motion_script::ActorKey {
                    y: 3.0,
                    ..Default::default()
                }],
            ),
        ]),
    );
    (
        motion_script::parse(&applied).unwrap(),
        expected,
        rendered,
        applied,
    )
}

#[test]
fn applied_library_clip_matches_authored_svg_png_and_random_seek_preview() {
    let (applied, expected, rendered, _) = applied_fixture();
    let cancel = CancellationToken::new();
    let preview = ScenePreview::new(
        Arc::new(applied.clone()),
        1,
        Arc::new(ArtContext::default()),
        Some(Arc::new(rendered.manifest.clone())),
    )
    .unwrap();
    for frame in [2, 0, 3, 1, 2] {
        let time = frame as f64 / applied.project.fps as f64;
        assert_eq!(
            motion_scene::evaluate(&applied, 1, time).unwrap().actors,
            motion_scene::evaluate(&expected, 1, time).unwrap().actors
        );
        for png in [false, true] {
            let actual = encode_frame(&applied, 1, &rendered, frame, png, &cancel, None)
                .unwrap()
                .2;
            let expected = encode_frame(&expected, 1, &rendered, frame, png, &cancel, None)
                .unwrap()
                .2;
            assert_eq!(actual, expected, "frame {frame}, PNG={png}");
            let png_bytes = if png {
                actual
            } else {
                media::rasterize(std::str::from_utf8(&actual).unwrap()).unwrap()
            };
            let pixels = resvg::tiny_skia::Pixmap::decode_png(&png_bytes).unwrap();
            let sample = frame * SAMPLE_RATE as u64 / applied.project.fps as u64;
            assert_eq!(
                preview.render(sample, 256, &cancel).unwrap().rgba,
                pixels.data()
            );
        }
    }
    let before = motion_script::parse(SOURCE).unwrap();
    assert_ne!(
        encode_frame(&applied, 1, &rendered, 2, true, &cancel, None)
            .unwrap()
            .2,
        encode_frame(&before, 1, &rendered, 2, true, &cancel, None)
            .unwrap()
            .2
    );
}

#[test]
fn project_roundtrip_and_library_deletion_preserve_baked_clip_frames() {
    let (applied, _, rendered, source) = applied_fixture();
    let document = motion_project::ProjectDocument::new(source.clone()).unwrap();
    let loaded = motion_project::decode(&motion_project::encode(&document).unwrap()).unwrap();
    assert_eq!(loaded.source, source);
    let roundtrip = motion_script::parse(&loaded.source).unwrap();
    let without_library = motion_script::remove_animation_clip(&loaded.source, "reach").unwrap();
    let without_library = motion_script::parse(&without_library).unwrap();
    assert_eq!(
        applied.scenes[1].composition.bone_tracks,
        without_library.scenes[1].composition.bone_tracks
    );
    let cancel = CancellationToken::new();
    for frame in [0, 2, 3] {
        for png in [false, true] {
            let expected = encode_frame(&applied, 1, &rendered, frame, png, &cancel, None)
                .unwrap()
                .2;
            for candidate in [&roundtrip, &without_library] {
                assert_eq!(
                    encode_frame(candidate, 1, &rendered, frame, png, &cancel, None)
                        .unwrap()
                        .2,
                    expected
                );
            }
        }
    }
}

#[test]
fn retimed_library_clip_matches_authored_frames_and_shuffled_preview_seeks() {
    let (_, rendered) = crate::tests::fixture();
    let captured =
        motion_script::capture_animation_clip(SOURCE, 0, "EMMA", "reach", false).unwrap();
    let source =
        motion_script::apply_animation_clip_retimed(&captured, 1, "EMMA", "reach", 0.25, 0.5)
            .unwrap();
    let applied = motion_script::parse(&source).unwrap();
    // Independent authored keys: the one-second clip is halved and starts at
    // 0.25. Its last hand key is at 0.625, not at the requested clip end.
    let mut expected = motion_script::parse(SOURCE).unwrap();
    expected.scenes[1].composition.bone_tracks.insert(
        "EMMA".into(),
        BTreeMap::from([
            (
                "right_hand".into(),
                vec![
                    motion_script::ActorKey {
                        at: 0.25,
                        ..Default::default()
                    },
                    motion_script::ActorKey {
                        at: 0.625,
                        x: 24.0,
                        ..Default::default()
                    },
                ],
            ),
            (
                "chest".into(),
                vec![motion_script::ActorKey {
                    at: 0.25,
                    y: 3.0,
                    ..Default::default()
                }],
            ),
        ]),
    );
    let original = motion_script::parse(&captured).unwrap();
    assert_eq!(
        serde_json::to_value(&applied.scenes[0]).unwrap(),
        serde_json::to_value(&original.scenes[0]).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&applied.scenes[1].events).unwrap(),
        serde_json::to_value(&original.scenes[1].events).unwrap()
    );
    assert_eq!(applied.animation_clips, original.animation_clips);
    let cancel = CancellationToken::new();
    let preview = ScenePreview::new(
        Arc::new(applied.clone()),
        1,
        Arc::new(ArtContext::default()),
        Some(Arc::new(rendered.manifest.clone())),
    )
    .unwrap();
    // Includes pre-first and post-last keys, backwards seeks and repeat seeks.
    for frame in [3, 0, 2, 1, 3, 2, 0] {
        let time = frame as f64 / applied.project.fps as f64;
        assert_eq!(
            motion_scene::evaluate(&applied, 1, time).unwrap().actors,
            motion_scene::evaluate(&expected, 1, time).unwrap().actors
        );
        for png in [false, true] {
            let actual = encode_frame(&applied, 1, &rendered, frame, png, &cancel, None)
                .unwrap()
                .2;
            assert_eq!(
                actual,
                encode_frame(&expected, 1, &rendered, frame, png, &cancel, None)
                    .unwrap()
                    .2
            );
            let png_bytes = if png {
                actual
            } else {
                media::rasterize(std::str::from_utf8(&actual).unwrap()).unwrap()
            };
            let pixels = resvg::tiny_skia::Pixmap::decode_png(&png_bytes).unwrap();
            let sample = frame * SAMPLE_RATE as u64 / applied.project.fps as u64;
            assert_eq!(
                preview.render(sample, 256, &cancel).unwrap().rgba,
                pixels.data()
            );
        }
    }
    let unretimed = motion_script::parse(
        &motion_script::apply_animation_clip(&captured, 1, "EMMA", "reach").unwrap(),
    )
    .unwrap();
    assert_ne!(
        encode_frame(&applied, 1, &rendered, 2, true, &cancel, None)
            .unwrap()
            .2,
        encode_frame(&unretimed, 1, &rendered, 2, true, &cancel, None)
            .unwrap()
            .2
    );

    let document = motion_project::ProjectDocument::new(source.clone()).unwrap();
    let loaded = motion_project::decode(&motion_project::encode(&document).unwrap()).unwrap();
    assert_eq!(loaded.source, source);
    let without_library = motion_script::remove_animation_clip(&loaded.source, "reach").unwrap();
    let restored = motion_script::parse(&without_library).unwrap();
    assert_eq!(
        restored.scenes[1].composition.bone_tracks,
        expected.scenes[1].composition.bone_tracks
    );
    for frame in [0, 2, 3] {
        assert_eq!(
            encode_frame(&restored, 1, &rendered, frame, true, &cancel, None)
                .unwrap()
                .2,
            encode_frame(&expected, 1, &rendered, frame, true, &cancel, None)
                .unwrap()
                .2
        );
    }
}
