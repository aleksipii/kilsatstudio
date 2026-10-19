//! Pixel-level mask acceptance on the same offline preview and export paths.
use super::*;
use motion_art::{BlendMode, ClipShape, Color, Content, Document, Layer, MaskKey, Plane, Rect};
use std::sync::Arc;

fn fixture() -> (Script, RenderedScene, ArtContext) {
    let (mut script, mut rendered) = crate::tests::fixture();
    script.project.width = 100;
    script.project.height = 100;
    script.characters.clear();
    script.scenes[0].composition.background_color = "#000000".into();
    rendered.manifest.dialogue.clear();
    let rect = Rect {
        x: 10.0,
        y: 10.0,
        width: 80.0,
        height: 80.0,
    };
    let shape = ClipShape::SoftEllipse {
        rect,
        feather: 0.0,
        keys: vec![
            MaskKey {
                time: 0.0,
                rect,
                feather: 0.0,
            },
            MaskKey {
                time: 0.5,
                rect,
                feather: 1.0,
            },
            MaskKey {
                time: 1.0,
                rect: Rect {
                    x: 30.0,
                    y: 10.0,
                    width: 60.0,
                    height: 80.0,
                },
                feather: 1.0,
            },
        ],
    };
    let art = ArtContext {
        document: Document {
            layers: vec![Layer {
                id: "soft".into(),
                scene_id: "intro".into(),
                name: "Soft mask".into(),
                visible: true,
                plane: Plane::Foreground,
                active: None,
                opacity: 1.0,
                attachment: None,
                clip: None,
                clip_shape: Some(shape),
                blend: BlendMode::Normal,
                content: Content::Rectangle {
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 100.0,
                        height: 100.0,
                    },
                    color: Color {
                        r: 255,
                        g: 0,
                        b: 0,
                        a: 255,
                    },
                },
            }],
            ..Default::default()
        },
        images: BTreeMap::new(),
    };
    (script, rendered, art)
}
fn red(frame: &SequencePreviewFrame, x: usize, y: usize) -> u8 {
    frame.rgba[(y * frame.width as usize + x) * 4]
}

#[test]
fn animated_soft_masks_match_independent_static_shapes_svg_png_and_repeated_preview_seeks() {
    let (script, rendered, art) = fixture();
    let art: ArtContext = ArtContext {
        document: serde_json::from_str(&serde_json::to_string(&art.document).unwrap()).unwrap(),
        ..art
    };
    let preview = ScenePreview::new(
        Arc::new(script.clone()),
        0,
        Arc::new(art.clone()),
        Some(Arc::new(rendered.manifest.clone())),
    )
    .unwrap();
    let cancel = CancellationToken::new();
    for index in [3, 0, 2, 1, 2] {
        let (sample, _, png) =
            encode_frame(&script, 0, &rendered, index, true, &cancel, Some(&art)).unwrap();
        let actual = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
        let (_, _, svg) =
            encode_frame(&script, 0, &rendered, index, false, &cancel, Some(&art)).unwrap();
        let svg = String::from_utf8(svg).unwrap();
        let vector_raster =
            resvg::tiny_skia::Pixmap::decode_png(&media::rasterize_scaled(&svg, None).unwrap())
                .unwrap();
        assert_eq!(actual.data(), vector_raster.data());
        let frame = preview.render(sample, 1024, &cancel).unwrap();
        assert_eq!(frame.rgba, actual.data());
        let mut expected = art.clone();
        let rect = Rect {
            x: if index == 3 { 20.0 } else { 10.0 },
            y: 10.0,
            width: if index == 3 { 70.0 } else { 80.0 },
            height: 80.0,
        };
        expected.document.layers[0].clip_shape = Some(if index == 0 {
            ClipShape::Ellipse { rect }
        } else {
            ClipShape::SoftEllipse {
                rect,
                feather: if index == 1 { 0.5 } else { 1.0 },
                keys: Vec::new(),
            }
        });
        let (_, _, expected_png) =
            encode_frame(&script, 0, &rendered, index, true, &cancel, Some(&expected)).unwrap();
        let expected_pixels = resvg::tiny_skia::Pixmap::decode_png(&expected_png).unwrap();
        assert_eq!(actual.data(), expected_pixels.data(), "frame {index}");
        assert_eq!(red(&frame, 5, 50), 0);
        if index == 0 {
            assert_eq!(red(&frame, 50, 50), 255);
            assert_eq!(red(&frame, 85, 50), 255);
        }
        if index == 2 {
            // Known radial coverage: center is opaque, the halfway radius is
            // about half coverage, and the outer eighth is mostly transparent.
            assert!(red(&frame, 50, 50) >= 245);
            assert!((115..=135).contains(&red(&frame, 70, 50)));
            assert!((15..=45).contains(&red(&frame, 85, 50)));
        }
    }
}

#[test]
fn future_mask_keys_are_rejected_before_preview_or_sequence_output() {
    let (script, rendered, mut art) = fixture();
    let Some(ClipShape::SoftEllipse { keys, .. }) = &mut art.document.layers[0].clip_shape else {
        panic!("soft mask")
    };
    keys.last_mut().unwrap().time = 1.1;
    let cancel = CancellationToken::new();
    assert!(ScenePreview::new(Arc::new(script.clone()), 0, Arc::new(art.clone()), None).is_err());
    assert!(render_sequence_frame(
        &script,
        std::slice::from_ref(&rendered),
        Some(&art),
        0,
        100,
        &cancel
    )
    .is_err());
    let dest = crate::tests::destination();
    let error =
        export_png_sequence(&script, &[rendered], Some(&art), &dest, &cancel, |_| {}).unwrap_err();
    assert!(error.to_string().contains("mask key"));
    assert!(!dest.exists());
}

#[test]
fn animated_mask_sampling_work_is_bounded_before_writing_frames() {
    let (mut script, mut rendered, mut art) = fixture();
    script.project.fps = 240;
    script.scenes[0].duration = 37.5;
    rendered.audio.samples.resize(900_000, 0.0);
    rendered.manifest.total_frames = 900_000;
    let mut layer = art.document.layers[0].clone();
    let Some(ClipShape::SoftEllipse { rect, keys, .. }) = &mut layer.clip_shape else {
        panic!("soft mask")
    };
    *keys = (0..motion_art::MAX_MASK_KEYS)
        .map(|index| MaskKey {
            time: index as f64 / motion_art::MAX_MASK_KEYS as f64,
            rect: *rect,
            feather: 0.5,
        })
        .collect();
    art.document.layers = (0..30)
        .map(|index| {
            let mut next = layer.clone();
            next.id = format!("mask-{index}");
            next
        })
        .collect();
    art.document.validate().unwrap();
    // 9000 frames × 30 layers × (content + bounded lookup + gradient)
    // exceeds four million visits even though the individual tracks are legal.
    let dest = crate::tests::destination();
    let error = export_png_sequence(
        &script,
        &[rendered],
        Some(&art),
        &dest,
        &CancellationToken::new(),
        |_| {},
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("artwork frame-work budget"),
        "{error}"
    );
    assert!(!dest.exists());
}
