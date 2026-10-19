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

fn square(feather: f64) -> ClipShape {
    let point = |x, y| motion_art::Point { x, y };
    ClipShape::Polygon {
        motion: Vec::new(),
        points: vec![
            point(20.0, 20.0),
            point(80.0, 20.0),
            point(80.0, 80.0),
            point(20.0, 80.0),
        ],
        feather,
    }
}

#[test]
fn feathered_polygon_blurs_around_the_edge_and_agrees_across_preview_png_and_svg() {
    let (script, rendered, mut art) = fixture();
    let cancel = CancellationToken::new();
    let mut row = |feather: f64| {
        art.document.layers[0].clip_shape = Some(square(feather));
        art.document.validate().unwrap();
        let preview = ScenePreview::new(
            Arc::new(script.clone()),
            0,
            Arc::new(art.clone()),
            Some(Arc::new(rendered.manifest.clone())),
        )
        .unwrap();
        let (sample, _, png) =
            encode_frame(&script, 0, &rendered, 0, true, &cancel, Some(&art)).unwrap();
        let png = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
        let (_, _, svg) =
            encode_frame(&script, 0, &rendered, 0, false, &cancel, Some(&art)).unwrap();
        let svg = String::from_utf8(svg).unwrap();
        let raster =
            resvg::tiny_skia::Pixmap::decode_png(&media::rasterize_scaled(&svg, None).unwrap())
                .unwrap();
        let frame = preview.render(sample, 1024, &cancel).unwrap();
        assert_eq!(png.data(), raster.data());
        assert_eq!(frame.rgba, png.data());
        (0..100).map(|x| red(&frame, x, 50)).collect::<Vec<_>>()
    };
    let hard = row(0.0);
    assert!(hard[..20].iter().all(|v| *v == 0));
    assert!(hard[20..80].iter().all(|v| *v == 255));
    assert!(hard[80..].iter().all(|v| *v == 0));

    let soft = row(4.0);
    assert_eq!(soft[50], 255);
    assert_eq!(soft[0], 0);
    assert_eq!(soft[99], 0);
    // Edge pixel centres sit half a unit either side of x=20: about half coverage.
    assert!((100..=156).contains(&soft[20]), "{}", soft[20]);
    assert!(soft[19] > 0, "feather must bleed outside the polygon");
    assert!(soft[21] < 255, "feather must soften inside the polygon");
    // Coverage rises monotonically across the left edge and mirrors the right edge.
    assert!(soft[8..=35].windows(2).all(|w| w[0] <= w[1]));
    for offset in 0..12 {
        let (left, right) = (soft[20 + offset], soft[79 - offset]);
        assert!(left.abs_diff(right) <= 2, "{offset}: {left} {right}");
    }
}

#[test]
fn polygon_feather_validates_persists_and_keeps_legacy_documents_readable() {
    assert!(square(0.0).validate().is_ok());
    assert!(square(1000.0).validate().is_ok());
    for bad in [-0.5, f64::NAN, f64::INFINITY, 1000.5] {
        assert!(square(bad).validate().is_err(), "{bad}");
    }
    let legacy: ClipShape = serde_json::from_str(
        r#"{"type":"polygon","points":[{"x":0,"y":0},{"x":4,"y":0},{"x":0,"y":4}]}"#,
    )
    .unwrap();
    assert!(matches!(legacy, ClipShape::Polygon { feather, .. } if feather == 0.0));
    assert!(!serde_json::to_string(&legacy).unwrap().contains("feather"));
    let soft = square(2.5);
    assert_eq!(
        serde_json::from_str::<ClipShape>(&serde_json::to_string(&soft).unwrap()).unwrap(),
        soft
    );
}

fn image_mask(channel: motion_art::MaskChannel) -> ClipShape {
    ClipShape::Image {
        asset_id: "mask".into(),
        rect: Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        channel,
        keys: Vec::new(),
    }
}

/// Renders frame zero through preview, PNG and vector rasterisation (which must agree).
fn render_row(
    script: &Script,
    rendered: &RenderedScene,
    art: &ArtContext,
    y: usize,
) -> Result<Vec<u8>, Error> {
    render_row_at(script, rendered, art, 0, y)
}
fn render_row_at(
    script: &Script,
    rendered: &RenderedScene,
    art: &ArtContext,
    index: u64,
    y: usize,
) -> Result<Vec<u8>, Error> {
    let cancel = CancellationToken::new();
    let preview = ScenePreview::new(
        Arc::new(script.clone()),
        0,
        Arc::new(art.clone()),
        Some(Arc::new(rendered.manifest.clone())),
    )?;
    let (sample, _, png) = encode_frame(script, 0, rendered, index, true, &cancel, Some(art))?;
    let png = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
    let (_, _, svg) = encode_frame(script, 0, rendered, index, false, &cancel, Some(art))?;
    let svg = String::from_utf8(svg).unwrap();
    let raster =
        resvg::tiny_skia::Pixmap::decode_png(&media::rasterize_scaled(&svg, None).unwrap())
            .unwrap();
    assert_eq!(png.data(), raster.data());
    let frame = preview.render(sample, 1024, &cancel)?;
    assert_eq!(frame.rgba, png.data());
    Ok((0..100).map(|x| red(&frame, x, y)).collect())
}

#[test]
fn image_masks_select_alpha_or_luminance_and_require_their_resource() {
    let (script, rendered, mut art) = fixture();
    // Left half opaque white, right half opaque black: alpha is full, luminance splits.
    let strip = |left: [u8; 4], right: [u8; 4]| {
        let rgba = (0..100)
            .flat_map(|x| if x < 50 { left } else { right })
            .collect();
        motion_art::ImageResource::from_rgba(100, 1, rgba).unwrap()
    };
    let opaque = strip([255, 255, 255, 255], [0, 0, 0, 255]);
    let clear = strip([255, 255, 255, 255], [255, 255, 255, 0]);
    art.document.layers[0].clip_shape = Some(image_mask(motion_art::MaskChannel::Luminance));
    art.document.validate().unwrap();
    // Missing resource fails closed instead of drawing unmasked content.
    assert!(render_row(&script, &rendered, &art, 50).is_err());

    art.images.insert("mask".into(), opaque);
    let row = render_row(&script, &rendered, &art, 50).unwrap();
    assert!(row[5..40].iter().all(|v| *v >= 250), "{row:?}");
    assert!(row[60..95].iter().all(|v| *v <= 5), "{row:?}");

    art.document.layers[0].clip_shape = Some(image_mask(motion_art::MaskChannel::Alpha));
    let row = render_row(&script, &rendered, &art, 50).unwrap();
    assert!(row.iter().all(|v| *v == 255), "opaque image alpha: {row:?}");

    art.images.insert("mask".into(), clear);
    let row = render_row(&script, &rendered, &art, 50).unwrap();
    assert!(row[5..40].iter().all(|v| *v >= 250), "{row:?}");
    assert!(row[60..95].iter().all(|v| *v <= 5), "{row:?}");

    art.document.layers[0].clip_shape = Some(image_mask(motion_art::MaskChannel::Luminance));
    let row = render_row(&script, &rendered, &art, 50).unwrap();
    // SVG luminance masks are weighted by alpha, so transparent white shows nothing.
    assert!(row[5..40].iter().all(|v| *v >= 250), "{row:?}");
    assert!(row[60..95].iter().all(|v| *v <= 5), "{row:?}");
}

#[test]
fn image_mask_validates_references_and_persistence() {
    let shape = image_mask(motion_art::MaskChannel::Luminance);
    shape.validate().unwrap();
    assert_eq!(shape.image_asset(), Some("mask"));
    let ClipShape::Image { rect, .. } = &shape else {
        unreachable!()
    };
    for bad in [
        ClipShape::Image {
            asset_id: String::new(),
            rect: *rect,
            channel: Default::default(),
            keys: Vec::new(),
        },
        ClipShape::Image {
            asset_id: "mask".into(),
            rect: Rect {
                width: 0.0,
                ..*rect
            },
            channel: Default::default(),
            keys: Vec::new(),
        },
    ] {
        assert!(bad.validate().is_err());
    }
    assert_eq!(
        serde_json::from_str::<ClipShape>(&serde_json::to_string(&shape).unwrap()).unwrap(),
        shape
    );
    let legacy: ClipShape = serde_json::from_str(
        r#"{"type":"image","asset_id":"m","rect":{"x":0,"y":0,"width":1,"height":1}}"#,
    )
    .unwrap();
    assert!(matches!(
        legacy,
        ClipShape::Image {
            channel: motion_art::MaskChannel::Alpha,
            ..
        }
    ));
    let (_, _, art) = fixture();
    let layer = &art.document.layers[0];
    let mut masked = layer.clone();
    masked.clip_shape = Some(shape);
    assert_eq!(masked.image_assets().collect::<Vec<_>>(), ["mask"]);
}

#[test]
fn painted_masks_preserve_order_dabs_and_base_coverage_across_preview_png_and_svg() {
    use motion_art::{MaskStroke, Point};
    let (script, rendered, mut art) = fixture();
    for base_visible in [false, true] {
        art.document.layers[0].clip_shape = Some(ClipShape::Painted {
            motion: Vec::new(),
            base_visible,
            strokes: Vec::new(),
        });
        let row = render_row(&script, &rendered, &art, 50).unwrap();
        assert!(row.iter().all(|v| *v == if base_visible { 255 } else { 0 }));
        let mut strokes = vec![MaskStroke {
            points: vec![Point { x: 20.0, y: 50.0 }, Point { x: 80.0, y: 50.0 }],
            radius: 15.0,
            erase: false,
            pressures: Vec::new(),
            hardness: 1.0,
        }];
        art.document.layers[0].clip_shape = Some(ClipShape::Painted {
            motion: Vec::new(),
            base_visible,
            strokes: strokes.clone(),
        });
        let row = render_row(&script, &rendered, &art, 50).unwrap();
        assert_eq!(row[0], if base_visible { 255 } else { 0 });
        assert_eq!(row[50], 255);
        strokes.push(MaskStroke {
            points: vec![Point { x: 50.0, y: 50.0 }],
            radius: 10.0,
            erase: true,
            pressures: Vec::new(),
            hardness: 1.0,
        });
        art.document.layers[0].clip_shape = Some(ClipShape::Painted {
            motion: Vec::new(),
            base_visible,
            strokes: strokes.clone(),
        });
        let row = render_row(&script, &rendered, &art, 50).unwrap();
        assert_eq!(row[25], 255);
        assert_eq!(row[50], 0, "opaque black must erase luminance coverage");
        assert_eq!(row[75], 255);
        strokes.push(MaskStroke {
            points: vec![Point { x: 50.0, y: 50.0 }],
            radius: 3.0,
            erase: false,
            pressures: Vec::new(),
            hardness: 1.0,
        });
        art.document.layers[0].clip_shape = Some(ClipShape::Painted {
            motion: Vec::new(),
            base_visible,
            strokes,
        });
        // Project metadata serialisation must preserve stroke order and both dabs.
        art.document = serde_json::from_slice(&serde_json::to_vec(&art.document).unwrap()).unwrap();
        art.document.validate().unwrap();
        for _ in 0..2 {
            let row = render_row(&script, &rendered, &art, 50).unwrap();
            assert_eq!(row[0], if base_visible { 255 } else { 0 });
            assert_eq!(row[43], 0);
            assert_eq!(row[50], 255, "later paint must restore erased coverage");
            assert_eq!(row[57], 0);
        }
        // A round dab must not become a square brush or a missing SVG move-only path.
        let row = render_row(&script, &rendered, &art, 58).unwrap();
        assert!(row[50] < 100);
        assert_eq!(row[41], 255);
        let preview = ScenePreview::new(
            Arc::new(script.clone()),
            0,
            Arc::new(art.clone()),
            Some(Arc::new(rendered.manifest.clone())),
        )
        .unwrap();
        let cancel = CancellationToken::new();
        for index in [3, 0, 2, 1, 0] {
            let (sample, _, png) =
                encode_frame(&script, 0, &rendered, index, true, &cancel, Some(&art)).unwrap();
            let pixels = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
            assert_eq!(
                preview.render(sample, 1024, &cancel).unwrap().rgba,
                pixels.data()
            );
        }
    }
}

#[test]
fn dormant_painted_mask_export_work_is_rejected_before_output_creation() {
    let (mut script, mut rendered, mut art) = fixture();
    script.project.fps = 240;
    script.scenes[0].duration = 37.5;
    rendered.audio.samples.resize(900_000, 0.0);
    rendered.manifest.total_frames = 900_000;
    art.document.layers[0].visible = false;
    art.document.layers[0].clip_shape = Some(ClipShape::Painted {
        motion: Vec::new(),
        base_visible: true,
        strokes: vec![motion_art::MaskStroke {
            points: (0..500)
                .map(|index| motion_art::Point {
                    x: index as f64 / 10.0,
                    y: 50.0,
                })
                .collect(),
            radius: 3.0,
            erase: true,
            pressures: Vec::new(),
            hardness: 1.0,
        }],
    });
    art.document.validate().unwrap();
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

#[test]
fn soft_pressure_masks_have_gradual_edges_variable_width_and_ordered_repainting() {
    use motion_art::{MaskStroke, Point};
    let (script, rendered, mut art) = fixture();
    for base_visible in [false, true] {
        let mut strokes = vec![MaskStroke {
            points: vec![Point { x: 50.0, y: 50.0 }],
            radius: 24.0,
            erase: base_visible,
            pressures: vec![1.0],
            hardness: 0.0,
        }];
        art.document.layers[0].clip_shape = Some(ClipShape::Painted {
            motion: Vec::new(),
            base_visible,
            strokes: strokes.clone(),
        });
        let row = render_row(&script, &rendered, &art, 50).unwrap();
        let coverage = |x: usize| if base_visible { 255 - row[x] } else { row[x] };
        assert!(coverage(50) > 220);
        assert!(coverage(60) > 50 && coverage(60) < 210);
        assert!(coverage(68) > 10 && coverage(68) < coverage(60));
        assert_eq!(coverage(80), 0);
        // Repeated tablet samples at one location must not accumulate opacity.
        let mut overlapping = strokes.clone();
        overlapping[0].points = vec![Point { x: 50.0, y: 50.0 }; 8];
        overlapping[0].pressures = vec![1.0; 8];
        art.document.layers[0].clip_shape = Some(ClipShape::Painted {
            motion: Vec::new(),
            base_visible,
            strokes: overlapping,
        });
        assert_eq!(render_row(&script, &rendered, &art, 50).unwrap(), row);
        strokes.push(MaskStroke {
            points: vec![Point { x: 50.0, y: 50.0 }],
            radius: 5.0,
            erase: !base_visible,
            pressures: vec![1.0],
            hardness: 1.0,
        });
        art.document.layers[0].clip_shape = Some(ClipShape::Painted {
            motion: Vec::new(),
            base_visible,
            strokes,
        });
        // Persistence, PNG export, vector export and repeated CPU preview share the brush.
        art.document = serde_json::from_slice(&serde_json::to_vec(&art.document).unwrap()).unwrap();
        let row = render_row(&script, &rendered, &art, 50).unwrap();
        assert_eq!(row[50], if base_visible { 255 } else { 0 });
    }
    art.document.layers[0].clip_shape = Some(ClipShape::Painted {
        motion: Vec::new(),
        base_visible: false,
        strokes: vec![MaskStroke {
            points: vec![Point { x: 25.0, y: 50.0 }, Point { x: 75.0, y: 50.0 }],
            radius: 20.0,
            erase: false,
            pressures: vec![0.25, 1.0],
            hardness: 1.0,
        }],
    });
    let row = render_row(&script, &rendered, &art, 60).unwrap();
    assert_eq!(row[25], 0, "quarter pressure must narrow the brush");
    assert_eq!(row[75], 255);
}

#[test]
fn soft_brush_expansion_is_charged_before_sequence_output_creation() {
    let (mut script, mut rendered, mut art) = fixture();
    script.project.fps = 240;
    script.scenes[0].duration = 37.5;
    rendered.audio.samples.resize(900_000, 0.0);
    rendered.manifest.total_frames = 900_000;
    // Raw point work fits comfortably; expanded soft-brush geometry does not.
    art.document.layers[0].visible = false;
    art.document.layers[0].clip_shape = Some(ClipShape::Painted {
        motion: Vec::new(),
        base_visible: true,
        strokes: vec![motion_art::MaskStroke {
            points: (0..100)
                .map(|index| motion_art::Point {
                    x: index as f64,
                    y: 50.0,
                })
                .collect(),
            radius: 3.0,
            erase: true,
            pressures: vec![1.0; 100],
            hardness: 0.0,
        }],
    });
    art.document.validate().unwrap();
    let scene_dest = crate::tests::destination();
    let error = export_png_scene_with_art(
        &script,
        0,
        &rendered,
        &art,
        &scene_dest,
        &CancellationToken::new(),
        |_| {},
    )
    .unwrap_err();
    assert!(error.to_string().contains("frame visits"), "{error}");
    assert!(!scene_dest.exists());
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

#[test]
fn pressure_stroke_self_crossing_and_sharp_turns_do_not_cancel_coverage() {
    use motion_art::{MaskStroke, Point};
    let (script, rendered, mut art) = fixture();
    art.document.layers[0].clip_shape = Some(ClipShape::Painted {
        motion: Vec::new(),
        base_visible: false,
        strokes: vec![MaskStroke {
            // A bow tie crosses itself, then reverses over its first diagonal.
            points: vec![
                Point { x: 20.0, y: 20.0 },
                Point { x: 80.0, y: 80.0 },
                Point { x: 20.0, y: 80.0 },
                Point { x: 80.0, y: 20.0 },
                Point { x: 20.0, y: 20.0 },
                Point { x: 80.0, y: 80.0 },
            ],
            radius: 10.0,
            erase: false,
            pressures: vec![0.4, 1.0, 0.6, 0.9, 0.7, 0.5],
            hardness: 1.0,
        }],
    });
    let crossing = render_row(&script, &rendered, &art, 50).unwrap();
    assert_eq!(crossing[50], 255, "crossing segments must form a union");
    assert_eq!(crossing[10], 0);
    let turns = render_row(&script, &rendered, &art, 80).unwrap();
    assert_eq!(turns[20], 255, "sharp turns retain their endpoint disk");
    assert_eq!(turns[50], 255, "opposite winding must not punch a hole");
    assert_eq!(turns[80], 255);
}

#[test]
fn animated_image_masks_match_static_expectations_and_preview_export_on_reverse_seeks() {
    let (script, rendered, mut art) = fixture();
    art.images.insert(
        "mask".into(),
        motion_art::ImageResource::from_rgba(1, 1, vec![255; 4]).unwrap(),
    );
    let base = Rect {
        x: 0.0,
        y: 0.0,
        width: 40.0,
        height: 100.0,
    };
    art.document.layers[0].clip_shape = Some(ClipShape::Image {
        asset_id: "mask".into(),
        rect: base,
        channel: motion_art::MaskChannel::Alpha,
        keys: vec![
            motion_art::RectMaskKey {
                time: 0.0,
                rect: base,
            },
            motion_art::RectMaskKey {
                time: 1.0,
                rect: Rect { x: 60.0, ..base },
            },
        ],
    });
    art.document = serde_json::from_str(&serde_json::to_string(&art.document).unwrap()).unwrap();
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
        assert_eq!(
            preview.render(sample, 1024, &cancel).unwrap().rgba,
            actual.data()
        );
        let (_, _, svg) =
            encode_frame(&script, 0, &rendered, index, false, &cancel, Some(&art)).unwrap();
        let svg_png = media::rasterize_scaled(&String::from_utf8(svg).unwrap(), None).unwrap();
        assert_eq!(
            resvg::tiny_skia::Pixmap::decode_png(&svg_png)
                .unwrap()
                .data(),
            actual.data()
        );
        let mut expected = art.clone();
        expected.document.layers[0].clip_shape = Some(ClipShape::Image {
            asset_id: "mask".into(),
            rect: Rect {
                x: 60.0 * (index as f64 * 0.25).min(1.0),
                ..base
            },
            channel: motion_art::MaskChannel::Alpha,
            keys: Vec::new(),
        });
        let (_, _, expected_png) =
            encode_frame(&script, 0, &rendered, index, true, &cancel, Some(&expected)).unwrap();
        assert_eq!(
            actual.data(),
            resvg::tiny_skia::Pixmap::decode_png(&expected_png)
                .unwrap()
                .data(),
            "frame {index}"
        );
    }
}

fn motion(time: f64, x: f64, rotation: f64) -> motion_art::MaskMotionKey {
    motion_art::MaskMotionKey {
        time,
        x,
        rotation,
        ..Default::default()
    }
}
fn bar(feather: f64, motion: Vec<motion_art::MaskMotionKey>) -> ClipShape {
    let point = |x, y| motion_art::Point { x, y };
    ClipShape::Polygon {
        points: vec![
            point(20.0, 40.0),
            point(80.0, 40.0),
            point(80.0, 60.0),
            point(20.0, 60.0),
        ],
        feather,
        motion,
    }
}

#[test]
fn polygon_motion_translates_rotates_seeks_deterministically_and_agrees_across_outputs() {
    let (script, rendered, mut art) = fixture();
    // Frames are quarter seconds: index 3 is t=0.75, the last key.
    art.document.layers[0].clip_shape = Some(bar(
        0.0,
        vec![motion(0.0, 0.0, 0.0), motion(0.75, 30.0, 0.0)],
    ));
    art.document.validate().unwrap();
    let start = render_row_at(&script, &rendered, &art, 0, 50).unwrap();
    assert!(start[..20].iter().all(|v| *v == 0) && start[20..80].iter().all(|v| *v == 255));
    let end = render_row_at(&script, &rendered, &art, 3, 50).unwrap();
    assert!(end[..50].iter().all(|v| *v == 0), "{end:?}");
    assert!(end[50..100].iter().all(|v| *v == 255), "{end:?}");
    // Halfway (t=0.375 is between frames) and out-of-order seeks stay deterministic.
    for index in [3u64, 1, 0, 3, 2, 1] {
        let again = render_row_at(&script, &rendered, &art, index, 50).unwrap();
        let expected_shift = [0, 10, 20, 30][index as usize];
        assert_eq!(again[20 + expected_shift - 2], 0, "{index}");
        assert_eq!(again[20 + expected_shift + 2], 255, "{index}");
    }
    // A quarter turn about the bar's own centre makes it tall instead of wide.
    art.document.layers[0].clip_shape = Some(bar(
        0.0,
        vec![
            motion(0.0, 0.0, 0.0),
            motion(0.75, 0.0, std::f64::consts::FRAC_PI_2),
        ],
    ));
    let column = |index| {
        (0..100)
            .map(|y| {
                let cancel = CancellationToken::new();
                let preview = ScenePreview::new(
                    Arc::new(script.clone()),
                    0,
                    Arc::new(art.clone()),
                    Some(Arc::new(rendered.manifest.clone())),
                )
                .unwrap();
                let (sample, _, _) =
                    encode_frame(&script, 0, &rendered, index, true, &cancel, Some(&art)).unwrap();
                let frame = preview.render(sample, 1024, &cancel).unwrap();
                red(&frame, 50, y)
            })
            .collect::<Vec<_>>()
    };
    let flat = column(0);
    let tall = column(3);
    assert!(flat[45..55].iter().all(|v| *v == 255) && flat[..35].iter().all(|v| *v == 0));
    assert!(tall[25..75].iter().all(|v| *v >= 250), "{tall:?}");
    assert!(tall[..15].iter().all(|v| *v == 0) && tall[85..].iter().all(|v| *v == 0));
}

#[test]
fn feathered_polygon_and_painted_masks_move_with_their_motion_tracks() {
    let (script, rendered, mut art) = fixture();
    art.document.layers[0].clip_shape = Some(bar(
        3.0,
        vec![motion(0.0, 0.0, 0.0), motion(0.75, 20.0, 0.0)],
    ));
    art.document.validate().unwrap();
    let row = render_row_at(&script, &rendered, &art, 3, 50).unwrap();
    // The left edge moved from x=20 to x=40 and stays about half covered there.
    assert!(row[10] == 0 && row[30] < 20, "{row:?}");
    assert!((100..=156).contains(&row[40]), "{row:?}");
    assert!(row[60] == 255);
    let painted = |motion| ClipShape::Painted {
        base_visible: false,
        strokes: vec![motion_art::MaskStroke {
            points: vec![motion_art::Point { x: 30.0, y: 50.0 }],
            radius: 8.0,
            erase: false,
            pressures: Vec::new(),
            hardness: 1.0,
        }],
        motion,
    };
    art.document.layers[0].clip_shape = Some(painted(vec![
        motion(0.0, 0.0, 0.0),
        motion(0.75, 40.0, 0.0),
    ]));
    art.document.validate().unwrap();
    let before = render_row_at(&script, &rendered, &art, 0, 50).unwrap();
    let after = render_row_at(&script, &rendered, &art, 3, 50).unwrap();
    assert_eq!((before[30], before[70]), (255, 0), "{before:?}");
    assert_eq!((after[30], after[70]), (0, 255), "{after:?}");
}

#[test]
fn mask_motion_validates_bounds_scene_duration_persistence_and_legacy_json() {
    let valid = bar(0.0, vec![motion(0.0, 0.0, 0.0), motion(0.5, 10.0, 1.0)]);
    valid.validate().unwrap();
    assert_eq!(valid.key_count(), 2);
    assert!(valid.validate_scene_duration(0.5).is_ok());
    assert!(valid.validate_scene_duration(0.4).is_err());
    let mut bad = [
        vec![motion(0.5, 0.0, 0.0), motion(0.5, 1.0, 0.0)],
        vec![motion(-1.0, 0.0, 0.0)],
        vec![motion(0.0, f64::NAN, 0.0)],
        vec![motion(0.0, 2.0e6, 0.0)],
        vec![motion(0.0, 0.0, 2000.0)],
        vec![motion_art::MaskMotionKey {
            scale: 0.0,
            ..Default::default()
        }],
        vec![motion_art::MaskMotionKey {
            scale: 1000.0,
            ..Default::default()
        }],
        (0..129).map(|i| motion(i as f64, 0.0, 0.0)).collect(),
    ]
    .into_iter();
    assert!(bad.all(|keys| bar(0.0, keys).validate().is_err()));
    let json = serde_json::to_string(&valid).unwrap();
    assert_eq!(serde_json::from_str::<ClipShape>(&json).unwrap(), valid);
    // Static shapes omit `motion`, and legacy documents without it still load.
    assert!(!serde_json::to_string(&bar(0.0, Vec::new()))
        .unwrap()
        .contains("motion"));
    let legacy: ClipShape =
        serde_json::from_str(r#"{"type":"painted","base_visible":true,"strokes":[]}"#).unwrap();
    assert_eq!(legacy.key_count(), 0);
    assert!(legacy.sample_motion(1.0).is_none());
    // Sampling interpolates linearly and holds the endpoints.
    let held = valid.sample_motion(9.0).unwrap();
    assert_eq!((held.x, held.rotation), (10.0, 1.0));
    let mid = valid.sample_motion(0.25).unwrap();
    assert_eq!((mid.x, mid.rotation), (5.0, 0.5));
    assert_eq!(mid.pivot, motion_art::Point { x: 50.0, y: 50.0 });
}
