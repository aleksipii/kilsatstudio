//! Pixel-level tests for scene lights and animated image props (SVG reference
//! rasterized by resvg, the same path used by preview and export).
use super::*;
use motion_art::{
    BlendMode, Color, Content, Document, Flicker, FlickerMode, ImageFrame, ImageResource, Layer,
    Light, LightKey, LightSource, Plane, Playback, Point, Rect, ScreenGlow, TimeRange,
};
use resvg::tiny_skia::Pixmap;
use std::sync::Arc;

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}
fn light(id: &str, source: LightSource) -> Light {
    Light {
        id: id.into(),
        scene_id: "intro".into(),
        visible: true,
        active: None,
        flicker: None,
        source,
    }
}
fn radial(x: f64, y: f64, radius: f64, color: Color, blend: BlendMode) -> LightSource {
    LightSource::Radial {
        position: Point { x, y },
        radius,
        aspect: 1.0,
        softness: 1.0,
        color,
        intensity: 1.0,
        blend,
        attachment: None,
        track: Vec::new(),
        screen_glow: None,
    }
}
fn animated(id: &str, frames: &[(&str, f64)], playback: Playback, rect: Rect) -> Layer {
    Layer {
        id: id.into(),
        scene_id: "intro".into(),
        name: id.into(),
        visible: true,
        plane: Plane::Foreground,
        active: None,
        opacity: 1.0,
        attachment: None,
        clip: None,
        clip_shape: None,
        blend: BlendMode::Normal,
        content: Content::AnimatedImage {
            frames: frames
                .iter()
                .map(|(asset, duration)| ImageFrame {
                    asset_id: (*asset).into(),
                    duration: *duration,
                })
                .collect(),
            rect,
            playback,
        },
    }
}
fn solid(r: u8, g: u8, b: u8) -> ImageResource {
    ImageResource::from_rgba(1, 1, vec![r, g, b, 255]).unwrap()
}
fn images() -> BTreeMap<String, ImageResource> {
    BTreeMap::from([
        ("red".into(), solid(255, 0, 0)),
        ("green".into(), solid(0, 255, 0)),
        ("blue".into(), solid(0, 0, 255)),
        ("black".into(), solid(0, 0, 0)),
        ("phone".into(), solid(40, 120, 255)),
    ])
}
const BOX: Rect = Rect {
    x: 10.0,
    y: 10.0,
    width: 50.0,
    height: 50.0,
};
fn scene_at(
    script: &Script,
    time: f64,
    background: &str,
    actors: bool,
) -> motion_scene::EvaluatedScene {
    let mut scene = motion_scene::evaluate(script, 0, time).unwrap();
    scene.background_color = background.into();
    if !actors {
        scene.actors.clear();
    }
    scene
}
fn svg_of(scene: &motion_scene::EvaluatedScene, doc: &Document) -> String {
    motion_render::render_svg_with_art(scene, 640, 360, &[], "intro", doc, &images()).unwrap()
}
fn raster(script: &Script, time: f64, background: &str, doc: &Document) -> Pixmap {
    let scene = scene_at(script, time, background, false);
    Pixmap::decode_png(&media::rasterize(&svg_of(&scene, doc)).unwrap()).unwrap()
}
fn px(map: &Pixmap, x: u32, y: u32) -> [u8; 3] {
    let p = map.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue()]
}
fn doc_with(lights: Vec<Light>, layers: Vec<Layer>) -> Document {
    Document {
        layers,
        lights,
        ..Default::default()
    }
}
fn script() -> Script {
    crate::tests::fixture().0
}
fn close(actual: [u8; 3], expected: [u8; 3], tolerance: u8) {
    for c in 0..3 {
        assert!(
            actual[c].abs_diff(expected[c]) <= tolerance,
            "{actual:?} vs {expected:?}"
        );
    }
}

#[test]
fn radial_screen_light_brightens_and_tints_only_its_area() {
    let script = script();
    let base = raster(&script, 0.0, "#202020", &Document::default());
    let lit = raster(
        &script,
        0.0,
        "#202020",
        &doc_with(
            vec![light(
                "lamp",
                radial(300.0, 100.0, 80.0, rgb(255, 40, 40), BlendMode::Screen),
            )],
            vec![],
        ),
    );
    let centre = px(&lit, 300, 100);
    assert!(centre[0] > 240, "{centre:?}");
    assert!(centre[1] < 100 && centre[2] < 100, "red tint: {centre:?}");
    // Falloff is monotonic and the far side is bitwise untouched.
    let mid = px(&lit, 340, 100);
    assert!(mid[0] > px(&base, 340, 100)[0] && mid[0] < centre[0]);
    for (x, y) in [(500, 100), (300, 250), (0, 0), (639, 359)] {
        assert_eq!(px(&lit, x, y), px(&base, x, y));
    }
}

#[test]
fn multiply_light_darkens_and_hard_edge_has_no_falloff() {
    let script = script();
    let mut source = radial(300.0, 100.0, 60.0, rgb(0, 0, 0), BlendMode::Multiply);
    if let LightSource::Radial { softness, .. } = &mut source {
        *softness = 0.0;
    }
    let lit = raster(
        &script,
        0.0,
        "#808080",
        &doc_with(vec![light("shade", source)], vec![]),
    );
    close(px(&lit, 300, 100), [0, 0, 0], 2);
    close(px(&lit, 345, 100), [0, 0, 0], 2);
    close(px(&lit, 380, 100), [128, 128, 128], 1);
}

#[test]
fn ambient_multiply_tint_scales_with_strength() {
    let script = script();
    let ambient = |strength| {
        doc_with(
            vec![light(
                "room",
                LightSource::Ambient {
                    color: rgb(255, 128, 0),
                    strength,
                },
            )],
            vec![],
        )
    };
    close(
        px(&raster(&script, 0.0, "#808080", &ambient(1.0)), 320, 180),
        [128, 64, 0],
        2,
    );
    close(
        px(&raster(&script, 0.0, "#808080", &ambient(0.5)), 320, 180),
        [128, 96, 64],
        3,
    );
    close(
        px(&raster(&script, 0.0, "#808080", &ambient(0.0)), 320, 180),
        [128, 128, 128],
        0,
    );
}

#[test]
fn vignette_darkens_edges_but_not_the_centre() {
    let script = script();
    let lit = raster(
        &script,
        0.0,
        "#808080",
        &doc_with(
            vec![light(
                "edge",
                LightSource::Vignette {
                    strength: 1.0,
                    inner: 0.2,
                },
            )],
            vec![],
        ),
    );
    close(px(&lit, 320, 180), [128, 128, 128], 1);
    assert!(px(&lit, 2, 2)[0] < 25, "{:?}", px(&lit, 2, 2));
    assert!(px(&lit, 320, 5)[0] < px(&lit, 320, 60)[0]);
}

#[test]
fn glow_stays_bright_over_ambient_darkening() {
    let script = script();
    let doc = doc_with(
        vec![
            light(
                "glow",
                radial(300.0, 100.0, 80.0, rgb(255, 200, 120), BlendMode::Screen),
            ),
            light(
                "room",
                LightSource::Ambient {
                    color: rgb(40, 20, 80),
                    strength: 1.0,
                },
            ),
        ],
        vec![],
    );
    let lit = raster(&script, 0.0, "#808080", &doc);
    assert!(
        px(&lit, 300, 100)[0] > 230,
        "document order must not matter"
    );
    assert!(px(&lit, 600, 300)[0] < 30);
}

#[test]
fn flicker_is_deterministic_across_seeks_and_bounded() {
    let script = script();
    for mode in [FlickerMode::Sine, FlickerMode::Noise] {
        let flicker = Flicker {
            mode,
            amount: 0.8,
            rate_hz: 3.0,
            seed: 7,
        };
        let mut lamp = light(
            "lamp",
            radial(300.0, 100.0, 80.0, rgb(255, 255, 255), BlendMode::Screen),
        );
        lamp.flicker = Some(flicker);
        let doc = doc_with(vec![lamp], vec![]);
        let times = [0.1, 0.35, 0.1, 0.9, 0.35, 0.1];
        let frames: Vec<_> = times
            .iter()
            .map(|t| px(&raster(&script, *t, "#101010", &doc), 300, 100))
            .collect();
        assert_eq!(frames[0], frames[2]);
        assert_eq!(frames[0], frames[5]);
        assert_eq!(frames[1], frames[4]);
        assert_ne!(frames[0], frames[1], "{mode:?} must actually vary");
        for t in [0.0, 0.1, 0.35, 0.9, 12.3] {
            let f = flicker.factor(t);
            assert!((0.2..=1.0).contains(&f), "{f}");
            assert_eq!(f.to_bits(), flicker.factor(t).to_bits());
        }
        let other = Flicker { seed: 8, ..flicker };
        assert!((0..40)
            .any(|i| flicker.factor(f64::from(i) * 0.07) != other.factor(f64::from(i) * 0.07)));
    }
}

#[test]
fn light_follows_a_track_and_attaches_to_an_actor() {
    let script = script();
    let mut moving = radial(0.0, 0.0, 40.0, rgb(255, 255, 255), BlendMode::Screen);
    if let LightSource::Radial { track, .. } = &mut moving {
        *track = vec![
            LightKey {
                time: 0.0,
                x: 100.0,
                y: 80.0,
            },
            LightKey {
                time: 1.0,
                x: 500.0,
                y: 80.0,
            },
        ];
    }
    let doc = doc_with(vec![light("mover", moving)], vec![]);
    assert!(px(&raster(&script, 0.0, "#101010", &doc), 100, 80)[0] > 200);
    assert!(px(&raster(&script, 0.5, "#101010", &doc), 300, 80)[0] > 200);
    assert!(px(&raster(&script, 0.5, "#101010", &doc), 100, 80)[0] < 30);

    let scene = scene_at(&script, 0.0, "#101010", true);
    let emma = scene.actors.iter().find(|a| a.id == "EMMA").unwrap();
    let (ax, ay) = (emma.x, emma.y);
    let mut attached = radial(0.0, -120.0, 30.0, rgb(255, 255, 255), BlendMode::Screen);
    if let LightSource::Radial { attachment, .. } = &mut attached {
        *attachment = Some("EMMA".into());
    }
    let svg = svg_of(&scene, &doc_with(vec![light("halo", attached)], vec![]));
    let map = Pixmap::decode_png(&media::rasterize(&svg).unwrap()).unwrap();
    assert!(px(&map, (ax + 0.0) as u32, (ay - 120.0) as u32)[0] > 200);
}

#[test]
fn frame_selection_at_exact_boundaries_holds_and_loops() {
    let script = script();
    let frames = [("red", 0.25), ("green", 0.25), ("blue", 0.25)];
    let sample = |layer: Layer, t: f64| {
        let doc = doc_with(vec![], vec![layer]);
        px(&raster(&script, t, "#000000", &doc), 30, 30)
    };
    let hold = animated("p", &frames, Playback::Hold, BOX);
    assert_eq!(sample(hold.clone(), 0.0), [255, 0, 0]);
    assert_eq!(sample(hold.clone(), 0.249), [255, 0, 0]);
    assert_eq!(
        sample(hold.clone(), 0.25),
        [0, 255, 0],
        "boundary selects next"
    );
    assert_eq!(sample(hold.clone(), 0.5), [0, 0, 255]);
    assert_eq!(sample(hold.clone(), 0.99), [0, 0, 255], "hold keeps last");
    // total 0.75: 0.75 wraps to red, 0.9 is 0.15 into the loop (red).
    let looped = animated("p", &frames, Playback::Loop, BOX);
    assert_eq!(sample(looped.clone(), 0.75), [255, 0, 0]);
    assert_eq!(sample(looped.clone(), 0.99), [255, 0, 0]);
    assert_eq!(sample(looped, 0.0), [255, 0, 0]);
    // Time starts at the layer's active interval.
    let mut late = animated("p", &frames, Playback::Hold, BOX);
    late.active = Some(TimeRange {
        start: 0.5,
        end: 1.0,
    });
    assert_eq!(sample(late.clone(), 0.5), [255, 0, 0]);
    assert_eq!(sample(late.clone(), 0.74), [255, 0, 0]);
    assert_eq!(sample(late, 0.75), [0, 255, 0]);
    // Exactly one frame is embedded per rendered SVG.
    let doc = doc_with(vec![], vec![hold]);
    let svg = svg_of(&scene_at(&script, 0.3, "#000000", false), &doc);
    assert_eq!(svg.matches("<image ").count(), 1);
}

#[test]
fn screen_glow_follows_the_displayed_frame() {
    let script = script();
    let prop_rect = Rect {
        x: 400.0,
        y: 150.0,
        width: 60.0,
        height: 100.0,
    };
    let phone = animated(
        "phone",
        &[("phone", 0.5), ("black", 0.5)],
        Playback::Hold,
        prop_rect,
    );
    let mut glow = radial(0.0, 0.0, 130.0, rgb(255, 255, 255), BlendMode::Screen);
    if let LightSource::Radial { screen_glow, .. } = &mut glow {
        *screen_glow = Some(ScreenGlow {
            layer_id: "phone".into(),
            gain: 2.0,
            tint: 1.0,
        });
    }
    let doc = doc_with(vec![light("phone_glow", glow)], vec![phone]);
    let base = raster(&script, 0.0, "#101010", &Document::default());
    let on = raster(&script, 0.25, "#101010", &doc);
    let sample = px(&on, 430, 100);
    assert!(sample[2] > sample[0] + 20, "cool (blue) glow: {sample:?}");
    assert!(sample[2] > px(&base, 430, 100)[2] + 20);
    // A black frame emits nothing.
    let off = raster(&script, 0.75, "#101010", &doc);
    assert_eq!(px(&off, 430, 100), px(&base, 430, 100));
}

#[test]
fn legacy_documents_serialize_and_render_unchanged() {
    let doc = doc_with(vec![], vec![]);
    let value = serde_json::to_value(&doc).unwrap();
    assert!(value.get("lights").is_none());
    let svg = svg_of(&scene_at(&script(), 0.0, "#202020", true), &doc);
    assert!(!svg.contains("radialGradient") && !svg.contains("light"));
    assert!(svg.ends_with("</g></g></svg>\n"));
    let reread: Document = serde_json::from_str(r#"{"layers":[]}"#).unwrap();
    assert!(reread.lights.is_empty());
}

fn lit_context() -> ArtContext {
    let phone = animated(
        "phone",
        &[("phone", 0.5), ("black", 0.5)],
        Playback::Loop,
        Rect {
            x: 500.0,
            y: 200.0,
            width: 40.0,
            height: 60.0,
        },
    );
    let mut glow = radial(0.0, 0.0, 90.0, rgb(255, 255, 255), BlendMode::Screen);
    if let LightSource::Radial { screen_glow, .. } = &mut glow {
        *screen_glow = Some(ScreenGlow {
            layer_id: "phone".into(),
            gain: 2.0,
            tint: 1.0,
        });
    }
    let mut lamp = light(
        "lamp",
        radial(560.0, 40.0, 70.0, rgb(255, 170, 80), BlendMode::Screen),
    );
    lamp.flicker = Some(Flicker {
        mode: FlickerMode::Noise,
        amount: 0.5,
        rate_hz: 6.0,
        seed: 3,
    });
    ArtContext {
        document: doc_with(
            vec![
                light(
                    "room",
                    LightSource::Ambient {
                        color: rgb(60, 40, 110),
                        strength: 0.8,
                    },
                ),
                lamp,
                light("phone_glow", glow),
                light(
                    "edge",
                    LightSource::Vignette {
                        strength: 0.6,
                        inner: 0.3,
                    },
                ),
            ],
            vec![phone],
        ),
        images: images(),
    }
}

#[test]
fn preview_png_export_and_svg_raster_agree() {
    let (script, rendered) = crate::tests::fixture();
    let art = Arc::new(lit_context());
    let preview = ScenePreview::new(Arc::new(script.clone()), 0, art.clone(), None).unwrap();
    let cancel = CancellationToken::new();
    for sample in [0u64, 6000, 12000, 18000] {
        let scene = motion_scene::evaluate(&script, 0, sample as f64 / 24000.0).unwrap();
        let svg = motion_render::render_svg_with_art(
            &scene,
            640,
            360,
            &[],
            "intro",
            &art.document,
            &art.images,
        )
        .unwrap();
        let pixels = Pixmap::decode_png(&media::rasterize(&svg).unwrap()).unwrap();
        assert_eq!(
            preview.render(sample, 640, &cancel).unwrap().rgba,
            pixels.data(),
            "sample {sample}"
        );
    }
    // PNG export frames (fps 4) sample whole quarter seconds; compare a lit
    // region far from the actors against the SVG reference.
    let destination = crate::tests::destination();
    export_png_scene_with_art(&script, 0, &rendered, &art, &destination, &cancel, |_| {}).unwrap();
    for index in 0..4u32 {
        let png = Pixmap::load_png(destination.join(format!("frame_{index:06}.png"))).unwrap();
        let scene = motion_scene::evaluate(&script, 0, f64::from(index) / 4.0).unwrap();
        let svg = motion_render::render_svg_with_art(
            &scene,
            640,
            360,
            &[],
            "intro",
            &art.document,
            &art.images,
        )
        .unwrap();
        let reference = Pixmap::decode_png(&media::rasterize(&svg).unwrap()).unwrap();
        for (x, y) in [(560, 40), (520, 230), (600, 330), (4, 4)] {
            assert_eq!(px(&png, x, y), px(&reference, x, y), "frame {index}");
        }
    }
    std::fs::remove_dir_all(destination).unwrap();
}

#[test]
fn export_preflight_rejects_missing_frames_and_out_of_scene_lights() {
    let (script, rendered) = crate::tests::fixture();
    let cancel = CancellationToken::new();
    let run = |art: &ArtContext| {
        let destination = crate::tests::destination();
        let result =
            export_png_scene_with_art(&script, 0, &rendered, art, &destination, &cancel, |_| {});
        assert!(!destination.exists(), "preflight must precede output");
        result.unwrap_err().to_string()
    };
    // A later frame's asset is missing even though frame zero renders.
    let mut art = lit_context();
    art.images.remove("black");
    assert!(run(&art).contains("missing artwork image"));
    // Track key beyond the scene.
    let mut art = lit_context();
    if let LightSource::Radial { track, .. } = &mut art.document.lights[1].source {
        *track = vec![LightKey {
            time: 2.0,
            x: 1.0,
            y: 1.0,
        }];
    }
    assert!(run(&art).contains("exceeds scene duration"));
    // Light active interval beyond the scene.
    let mut art = lit_context();
    art.document.lights[1].active = Some(TimeRange {
        start: 0.0,
        end: 5.0,
    });
    assert!(run(&art).contains("exceeds scene duration"));
    // A light cannot reference a screen layer that does not exist.
    let mut art = lit_context();
    if let LightSource::Radial { screen_glow, .. } = &mut art.document.lights[2].source {
        screen_glow.as_mut().unwrap().layer_id = "nope".into();
    }
    assert!(run(&art).contains("screen glow"));
}

#[test]
fn night_room_example_validates_and_lights_face_cool_and_lamp_warm() {
    let doc: Document =
        serde_json::from_str(include_str!("../../../examples/night-room-lighting.json")).unwrap();
    doc.validate().unwrap();
    let mut script = script();
    script.scenes[0].duration = 4.0;
    let mut frames = images();
    frames.insert("phone-home".into(), solid(70, 130, 240));
    frames.insert("phone-chat".into(), solid(230, 240, 255));
    frames.insert("phone-dark".into(), solid(0, 0, 0));
    let render = |time: f64| {
        let scene = scene_at(&script, time, "#000000", false);
        let svg = motion_render::render_svg_with_art(&scene, 640, 360, &[], "intro", &doc, &frames)
            .unwrap();
        Pixmap::decode_png(&media::rasterize(&svg).unwrap()).unwrap()
    };
    let on = render(0.0);
    let face = px(&on, 390, 230);
    assert!(
        face[2] > face[0],
        "phone light is cool on the face: {face:?}"
    );
    let lamp = px(&on, 85, 85);
    assert!(lamp[0] > lamp[2] + 40, "lamp glow is warm: {lamp:?}");
    let corner = px(&on, 630, 20);
    assert!(
        corner.iter().all(|c| *c < 60),
        "room stays dark: {corner:?}"
    );
    // The final loop frame is black, so the cool glow is gone.
    let off = render(1.6);
    let dark_face = px(&off, 390, 230);
    assert!(dark_face[2] < face[2], "{dark_face:?} vs {face:?}");
    assert_eq!(render(0.0).data(), on.data(), "deterministic re-render");
}
