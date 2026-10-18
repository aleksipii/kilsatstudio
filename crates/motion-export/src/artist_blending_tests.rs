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
    export_scene_with_art(&authored, 0, &rendered, &art, &svg_dir, &cancel, |_| {})
        .unwrap();
    export_png_scene_with_art(&authored, 0, &rendered, &art, &png_dir, &cancel, |_| {})
        .unwrap();
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
