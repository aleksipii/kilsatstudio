//! Measured CPU/GPU parity harness (feature `gpu`).
//!
//! CPU reference: `render_svg_with_art` rasterized by resvg through the same
//! `media::rasterize` used by preview and export. Candidate: the experimental
//! evaluated-scene GPU renderer. Both receive the identical `EvaluatedScene`,
//! artwork document and decoded images. Tolerances are regression guards derived
//! from measurements recorded in `docs/gpu-parity.md`; they are not perceptual
//! acceptance. Hardware tests are `#[ignore]` and are run by hand:
//! `KILSAT_GPU_BACKEND=metal cargo test -p motion-export --features gpu
//!  gpu_parity -- --ignored --nocapture`
use super::*;
use motion_art::*;
use motion_render::gpu::Backend;
use motion_render::gpu_scene::{compare_decoded_rgba, GpuSceneRenderer, ImageDifference};
use motion_render::scene_commands::{self, Labels};
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

const SIZE: u32 = 128;

/// Documented acceptance numbers for one fixture, in 8-bit channel units.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tolerance {
    pub max_channel_error: u8,
    pub max_rms: f64,
    pub max_changed_fraction: f64,
}

impl Tolerance {
    /// Returns the exceeded limits, or an empty list when within tolerance.
    pub(crate) fn violations(&self, d: &ImageDifference) -> Vec<String> {
        let mut out = Vec::new();
        let max = *d.maximum_channel_error.iter().max().unwrap();
        if max > self.max_channel_error {
            out.push(format!(
                "max channel error {max} > {}",
                self.max_channel_error
            ));
        }
        let rms = d.rms_channel_error.iter().cloned().fold(0.0, f64::max);
        if rms > self.max_rms {
            out.push(format!("rms {rms:.3} > {:.3}", self.max_rms));
        }
        if d.changed_pixel_fraction > self.max_changed_fraction {
            out.push(format!(
                "changed fraction {:.4} > {:.4}",
                d.changed_pixel_fraction, self.max_changed_fraction
            ));
        }
        out
    }
}

struct Fixture {
    name: &'static str,
    scene: motion_scene::EvaluatedScene,
    document: Document,
    images: BTreeMap<String, ImageResource>,
    tolerance: Tolerance,
}

fn base_scene(background: &str) -> motion_scene::EvaluatedScene {
    let script = motion_script::parse(&format!(
        "project: {{fps: 4, width: {SIZE}, height: {SIZE}}}\ncharacters:\n  EMMA: {{asset: characters/emma}}\nscenes:\n  - id: s\n    duration: 1.0\n    events: []\n"
    ))
    .unwrap();
    let mut scene = motion_scene::evaluate(&script, 0, 0.0).unwrap();
    scene.background_color = background.into();
    scene.actors[0].x = 64.0;
    scene.actors[0].y = 80.0;
    scene
}

fn no_actors(background: &str) -> motion_scene::EvaluatedScene {
    let mut scene = base_scene(background);
    scene.actors.clear();
    scene
}

const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color { r, g, b, a }
}
fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}
fn pt(x: f64, y: f64) -> Point {
    Point { x, y }
}
fn layer(id: &str, content: Content, clip_shape: Option<ClipShape>) -> Layer {
    Layer {
        id: id.into(),
        scene_id: "s".into(),
        name: id.into(),
        visible: true,
        plane: Plane::Foreground,
        active: None,
        opacity: 1.0,
        attachment: None,
        clip: None,
        clip_shape,
        blend: BlendMode::Normal,
        content,
    }
}
fn fill(r: Rect, c: Color) -> Content {
    Content::Rectangle { rect: r, color: c }
}
fn doc(layers: Vec<Layer>) -> Document {
    Document {
        layers,
        ..Default::default()
    }
}
fn gradient_image() -> ImageResource {
    let mut data = Vec::new();
    for y in 0..8u32 {
        for x in 0..8u32 {
            let alpha = if (x + y) % 5 == 0 { 160 } else { 255 };
            data.extend([(x * 36) as u8, (y * 36) as u8, 128, alpha]);
        }
    }
    ImageResource::from_rgba(8, 8, data).unwrap()
}

fn tol(max_channel_error: u8, max_rms: f64, max_changed_fraction: f64) -> Tolerance {
    Tolerance {
        max_channel_error,
        max_rms,
        max_changed_fraction,
    }
}

fn fixtures() -> Vec<Fixture> {
    let mut v = Vec::new();
    let mut push = |name, scene, document, images: BTreeMap<String, ImageResource>, tolerance| {
        v.push(Fixture {
            name,
            scene,
            document,
            images,
            tolerance,
        })
    };
    let none = BTreeMap::new;
    push(
        "actor_labels_omitted",
        base_scene("#181f2c"),
        Document::default(),
        none(),
        TOL_ACTOR,
    );
    let mut zoomed = base_scene("#181f2c");
    zoomed.camera.zoom = 1.5;
    zoomed.camera.x = 10.0;
    zoomed.camera.y = -6.0;
    push(
        "actor_camera_zoom_pan",
        zoomed,
        Document::default(),
        none(),
        TOL_ACTOR,
    );
    let mut rotated = base_scene("#101010");
    rotated.actors[0].rotation = 0.4;
    rotated.actors[0].scale = motion_scene::Vec2::new(1.2, 0.9);
    rotated.opacity = 0.8;
    push(
        "actor_rotated_scaled_faded",
        rotated,
        Document::default(),
        none(),
        TOL_ACTOR,
    );
    push(
        "shapes_rect_ellipse_stroke",
        no_actors("#202020"),
        doc(vec![
            layer(
                "r",
                fill(rect(8.0, 8.0, 50.0, 40.0), rgba(220, 40, 40, 255)),
                None,
            ),
            layer(
                "e",
                Content::Ellipse {
                    rect: rect(40.0, 30.0, 70.0, 50.0),
                    color: rgba(40, 200, 90, 180),
                },
                None,
            ),
            layer(
                "s",
                Content::Stroke {
                    points: vec![pt(10.0, 100.0), pt(60.0, 90.0), pt(115.0, 115.0)],
                    color: rgba(250, 250, 40, 255),
                    width: 6.0,
                },
                None,
            ),
        ]),
        none(),
        TOL_SHAPES,
    );
    let mut camera = no_actors("#202020");
    camera.camera.zoom = 2.0;
    camera.camera.x = 12.0;
    camera.camera.y = 8.0;
    push(
        "shapes_camera_zoom",
        camera,
        doc(vec![layer(
            "r",
            fill(rect(30.0, 30.0, 40.0, 40.0), rgba(60, 120, 250, 255)),
            None,
        )]),
        none(),
        TOL_SHAPES,
    );
    push(
        "image_upscaled",
        no_actors("#000000"),
        doc(vec![layer(
            "i",
            Content::Image {
                asset_id: "g".into(),
                rect: rect(16.0, 16.0, 96.0, 96.0),
            },
            None,
        )]),
        BTreeMap::from([("g".into(), gradient_image())]),
        TOL_IMAGE,
    );
    let mask_target = |clip| {
        doc(vec![layer(
            "m",
            fill(rect(0.0, 0.0, 128.0, 128.0), rgba(255, 60, 60, 255)),
            Some(clip),
        )])
    };
    push(
        "mask_hard_ellipse",
        no_actors("#000000"),
        mask_target(ClipShape::Ellipse {
            rect: rect(20.0, 20.0, 88.0, 70.0),
        }),
        none(),
        TOL_MASK_EDGE,
    );
    push(
        "mask_hard_polygon",
        no_actors("#000000"),
        mask_target(ClipShape::Polygon {
            points: vec![
                pt(20.0, 20.0),
                pt(100.0, 30.0),
                pt(80.0, 100.0),
                pt(25.0, 80.0),
            ],
            feather: 0.0,
            motion: Vec::new(),
        }),
        none(),
        TOL_MASK_EDGE,
    );
    push(
        "mask_soft_ellipse",
        no_actors("#000000"),
        mask_target(ClipShape::SoftEllipse {
            rect: rect(14.0, 14.0, 100.0, 100.0),
            feather: 0.4,
            keys: Vec::new(),
        }),
        none(),
        TOL_SOFT_ELLIPSE,
    );
    for (name, channel) in [
        ("mask_image_alpha", MaskChannel::Alpha),
        ("mask_image_luminance", MaskChannel::Luminance),
    ] {
        push(
            name,
            no_actors("#000000"),
            mask_target(ClipShape::Image {
                asset_id: "g".into(),
                rect: rect(16.0, 16.0, 96.0, 96.0),
                channel,
                keys: Vec::new(),
            }),
            BTreeMap::from([("g".into(), gradient_image())]),
            TOL_IMAGE,
        );
    }
    let stroke = |points: Vec<Point>, radius, hardness, erase| MaskStroke {
        points,
        radius,
        pressures: Vec::new(),
        hardness,
        erase,
    };
    push(
        "mask_painted",
        no_actors("#000000"),
        mask_target(ClipShape::Painted {
            base_visible: false,
            strokes: vec![
                stroke(vec![pt(30.0, 40.0), pt(100.0, 60.0)], 18.0, 1.0, false),
                stroke(vec![pt(64.0, 50.0)], 7.0, 1.0, true),
                stroke(vec![pt(40.0, 100.0), pt(90.0, 100.0)], 12.0, 0.5, false),
            ],
            motion: Vec::new(),
        }),
        none(),
        TOL_MASK_SOFT,
    );
    push(
        "mask_feathered_polygon",
        no_actors("#000000"),
        mask_target(ClipShape::Polygon {
            points: vec![
                pt(30.0, 30.0),
                pt(98.0, 30.0),
                pt(98.0, 98.0),
                pt(30.0, 98.0),
            ],
            feather: 3.0,
            motion: Vec::new(),
        }),
        none(),
        TOL_MASK_SOFT,
    );
    // Animated polygon/painted masks: motion sampled at scene time 0.5 between
    // two keys (translate, uniform scale, rotation about the shape centre).
    let motion = vec![
        MaskMotionKey::default(),
        MaskMotionKey {
            time: 1.0,
            x: 14.0,
            y: -8.0,
            scale: 1.3,
            rotation: 0.6,
        },
    ];
    let at_half = |mut scene: motion_scene::EvaluatedScene| {
        scene.time = 0.5;
        scene
    };
    push(
        "mask_hard_polygon_motion",
        at_half(no_actors("#000000")),
        mask_target(ClipShape::Polygon {
            points: vec![
                pt(20.0, 20.0),
                pt(100.0, 30.0),
                pt(80.0, 100.0),
                pt(25.0, 80.0),
            ],
            feather: 0.0,
            motion: motion.clone(),
        }),
        none(),
        TOL_MASK_EDGE,
    );
    push(
        "mask_feathered_polygon_motion",
        at_half(no_actors("#000000")),
        mask_target(ClipShape::Polygon {
            points: vec![
                pt(30.0, 30.0),
                pt(98.0, 30.0),
                pt(98.0, 98.0),
                pt(30.0, 98.0),
            ],
            feather: 3.0,
            motion: motion.clone(),
        }),
        none(),
        TOL_MASK_SOFT,
    );
    push(
        "mask_painted_motion",
        at_half(no_actors("#000000")),
        mask_target(ClipShape::Painted {
            base_visible: false,
            strokes: vec![
                stroke(vec![pt(30.0, 40.0), pt(100.0, 60.0)], 18.0, 1.0, false),
                stroke(vec![pt(64.0, 50.0)], 7.0, 1.0, true),
                stroke(vec![pt(40.0, 100.0), pt(90.0, 100.0)], 12.0, 0.5, false),
            ],
            motion,
        }),
        none(),
        TOL_MASK_SOFT,
    );
    // Custom rig parts, evaluated by the real motion-rig code the CPU path uses.
    let rig_scene = |parts: Vec<motion_rig::EvaluatedPart>| {
        let mut scene = no_actors("#101820");
        scene.actors = base_scene("#101820").actors;
        scene.actors[0].rigid_parts = parts;
        scene
    };
    let joint = |id: &str, parent: Option<&str>, x: f64, rotation: f64, sx: f64| {
        motion_rig::RigJointDefinition {
            id: id.into(),
            parent: parent.map(Into::into),
            x,
            y: 0.0,
            rotation,
            scale_x: sx,
            scale_y: 1.0,
        }
    };
    let part =
        |joint: &str, points: Vec<[f64; 2]>, texture, skin| motion_rig::RigidPartDefinition {
            id: "p".into(),
            joint: joint.into(),
            points,
            color: [200, 60, 60, 200],
            texture,
            skin,
            ..Default::default()
        };
    let tex = |uv| {
        Some(motion_rig::PartTextureDefinition {
            asset: "g".into(),
            uv,
        })
    };
    let outline = vec![
        [-30.0, -20.0],
        [10.0, -28.0],
        [34.0, -4.0],
        [20.0, 26.0],
        [-6.0, 8.0],
        [-28.0, 24.0],
    ];
    let eval = |definition: motion_rig::RigDefinition| definition.evaluate_parts().unwrap();
    let rigid = |texture| motion_rig::RigDefinition {
        version: 1,
        id: "r".into(),
        joints: vec![joint("root", None, 0.0, 0.5, 1.3)],
        parts: vec![part("root", outline.clone(), texture, Vec::new())],
        ..Default::default()
    };
    let images_g = || BTreeMap::from([("g".into(), gradient_image())]);
    push(
        "rig_part_flat",
        rig_scene(eval(rigid(None))),
        Document::default(),
        none(),
        TOL_RIG,
    );
    push(
        "rig_part_textured_rotated",
        rig_scene(eval(rigid(tex([0.0, 0.0, 1.0, 1.0])))),
        Document::default(),
        images_g(),
        TOL_RIG_TEXTURE,
    );
    push(
        "rig_part_textured_subrect",
        rig_scene(eval(rigid(tex([0.25, 0.125, 0.75, 1.0])))),
        Document::default(),
        images_g(),
        TOL_RIG_TEXTURE,
    );
    let weight = |joint: &str, weight| motion_rig::VertexWeight {
        joint: joint.into(),
        weight,
    };
    let bar = vec![
        [-30.0, -8.0],
        [0.0, -8.0],
        [30.0, -8.0],
        [30.0, 8.0],
        [0.0, 8.0],
        [-30.0, 8.0],
    ];
    let skin = vec![
        vec![weight("root", 1.0)],
        vec![weight("root", 0.5), weight("arm", 0.5)],
        vec![weight("arm", 1.0)],
        vec![weight("arm", 1.0)],
        vec![weight("root", 0.5), weight("arm", 0.5)],
        vec![weight("root", 1.0)],
    ];
    let skinned = |texture| {
        let definition = motion_rig::RigDefinition {
            version: 1,
            id: "k".into(),
            joints: vec![
                joint("root", None, 0.0, 0.0, 1.0),
                joint("arm", Some("root"), 0.0, 0.0, 1.0),
            ],
            parts: vec![part("root", bar.clone(), texture, skin.clone())],
            ..Default::default()
        };
        let prepared = definition.prepare().unwrap();
        let arm = prepared.joint_index("arm").unwrap();
        prepared
            .evaluate_parts(&[(
                arm,
                motion_rig::Transform2 {
                    rotation: 0.7,
                    ..Default::default()
                },
            )])
            .unwrap()
    };
    push(
        "rig_part_skinned_flat",
        rig_scene(skinned(None)),
        Document::default(),
        none(),
        TOL_RIG,
    );
    push(
        "rig_part_skinned_textured",
        rig_scene(skinned(tex([0.0, 0.0, 1.0, 1.0]))),
        Document::default(),
        images_g(),
        TOL_RIG_TEXTURE,
    );
    let mut grouped = doc(vec![
        layer(
            "back",
            fill(rect(0.0, 0.0, 128.0, 128.0), rgba(200, 120, 40, 255)),
            None,
        ),
        {
            let mut l = layer(
                "a",
                fill(rect(10.0, 10.0, 70.0, 70.0), rgba(90, 160, 255, 255)),
                None,
            );
            l.opacity = 0.9;
            l
        },
        layer(
            "b",
            Content::Ellipse {
                rect: rect(40.0, 40.0, 70.0, 70.0),
                color: rgba(250, 80, 160, 220),
            },
            None,
        ),
    ]);
    grouped.groups.push(Group {
        id: "g".into(),
        scene_id: "s".into(),
        plane: Plane::Foreground,
        name: "g".into(),
        visible: true,
        opacity: 0.85,
        blend: BlendMode::Multiply,
        children: vec![ArtUnit::Layer("a".into()), ArtUnit::Layer("b".into())],
    });
    grouped.order = vec![ArtUnit::Layer("back".into()), ArtUnit::Group("g".into())];
    push(
        "group_multiply",
        no_actors("#000000"),
        grouped.clone(),
        none(),
        TOL_GROUP,
    );
    grouped.groups[0].blend = BlendMode::Screen;
    push(
        "group_screen",
        no_actors("#000000"),
        grouped,
        none(),
        TOL_GROUP,
    );
    let mut blended = doc(vec![
        layer(
            "back",
            fill(rect(0.0, 0.0, 128.0, 128.0), rgba(200, 120, 40, 255)),
            None,
        ),
        layer(
            "top",
            Content::Ellipse {
                rect: rect(20.0, 20.0, 90.0, 90.0),
                color: rgba(70, 140, 220, 255),
            },
            None,
        ),
    ]);
    blended.layers[1].blend = BlendMode::Multiply;
    push(
        "layer_blend_multiply",
        no_actors("#000000"),
        blended,
        none(),
        TOL_GROUP,
    );
    let mut mixed = doc(vec![layer(
        "m",
        fill(rect(0.0, 0.0, 128.0, 128.0), rgba(255, 220, 0, 160)),
        Some(ClipShape::Ellipse {
            rect: rect(10.0, 10.0, 108.0, 60.0),
        }),
    )]);
    mixed.layers[0].plane = Plane::Background;
    push(
        "actor_with_masked_art",
        base_scene("#181f2c"),
        mixed,
        none(),
        TOL_ACTOR,
    );
    v
}

// Regression-guard tolerances; see docs/gpu-parity.md for the measured values
// they were derived from and what they do not establish.
const fn limits(max_channel_error: u8, max_rms: f64, max_changed_fraction: f64) -> Tolerance {
    Tolerance {
        max_channel_error,
        max_rms,
        max_changed_fraction,
    }
}
const TOL_ACTOR: Tolerance = limits(60, 4.0, 0.10);
const TOL_SHAPES: Tolerance = limits(40, 4.0, 0.30);
const TOL_IMAGE: Tolerance = limits(24, 2.5, 0.60);
const TOL_MASK_EDGE: Tolerance = limits(60, 4.0, 0.06);
const TOL_MASK_SOFT: Tolerance = limits(100, 8.0, 0.35);
const TOL_SOFT_ELLIPSE: Tolerance = limits(4, 1.0, 0.25);
const TOL_RIG: Tolerance = limits(60, 4.0, 0.30);
const TOL_RIG_TEXTURE: Tolerance = limits(110, 4.0, 0.30);
const TOL_GROUP: Tolerance = limits(30, 4.0, 0.40);

fn cpu_reference(f: &Fixture) -> Vec<u8> {
    let svg =
        motion_render::render_svg_with_art(&f.scene, SIZE, SIZE, &[], "s", &f.document, &f.images)
            .unwrap();
    let png = media::rasterize(&svg).unwrap();
    let pixmap = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
    assert_eq!((pixmap.width(), pixmap.height()), (SIZE, SIZE));
    pixmap.data().to_vec()
}

fn backend() -> Backend {
    match std::env::var("KILSAT_GPU_BACKEND").as_deref() {
        Ok("metal") => Backend::Metal,
        Ok("vulkan") => Backend::Vulkan,
        Ok("gl") => Backend::Gl,
        _ => panic!("set KILSAT_GPU_BACKEND explicitly"),
    }
}

#[test]
fn tolerance_gate_rejects_each_exceeded_limit() {
    let reference = vec![100u8; (SIZE * SIZE * 4) as usize];
    let mut candidate = reference.clone();
    candidate[0] = 140;
    let d = compare_decoded_rgba(SIZE, SIZE, &reference, &candidate).unwrap();
    assert!(tol(40, 1.0, 0.01).violations(&d).is_empty());
    assert_eq!(tol(39, 1.0, 0.01).violations(&d).len(), 1);
    assert_eq!(tol(255, 0.1, 0.01).violations(&d).len(), 1);
    assert_eq!(tol(255, 1.0, 0.00001).violations(&d).len(), 1);
}

/// The CPU reference path draws no actor label: the label `<text>` is emitted with
/// `display="none"` (resvg now has text support for captions, with a font database
/// holding only the bundled caption font); omitting labels on the GPU therefore
/// matches what the CPU rasterizer actually produces. Captions are not part of
/// the GPU path at all (see docs/captions.md). Runs without a GPU.
#[test]
fn cpu_rasterizer_draws_no_actor_label_text() {
    let scene = base_scene("#181f2c");
    let svg = motion_render::render_svg(&scene, SIZE, SIZE).unwrap();
    assert!(svg.contains("<text "));
    let start = svg.find("<text ").unwrap();
    let end = start + svg[start..].find("</text>").unwrap() + "</text>".len();
    let without = format!("{}{}", &svg[..start], &svg[end..]);
    let a = media::rasterize(&svg).unwrap();
    let b = media::rasterize(&without).unwrap();
    assert_eq!(
        resvg::tiny_skia::Pixmap::decode_png(&a).unwrap().data(),
        resvg::tiny_skia::Pixmap::decode_png(&b).unwrap().data()
    );
}

#[test]
#[ignore = "requires explicit KILSAT_GPU_BACKEND and an actual GPU; run by hand"]
fn gpu_parity_fixtures_within_documented_tolerance() {
    let cancel = AtomicBool::new(false);
    let mut renderer = GpuSceneRenderer::new(backend(), &cancel).unwrap();
    let mut failures = Vec::new();
    let mut report =
        String::from("fixture | max R,G,B,A | rms R,G,B,A | changed | edge rms | interior rms\n");
    for f in fixtures() {
        let reference = cpu_reference(&f);
        let commands = scene_commands::build_with_art(
            &f.scene,
            SIZE,
            SIZE,
            &[],
            Labels::OmitAcknowledged,
            "s",
            &f.document,
            &f.images,
        )
        .unwrap();
        let frame = renderer.render(&commands, &cancel).unwrap();
        assert!(
            frame.validation_errors.is_empty(),
            "{}: {:?}",
            f.name,
            frame.validation_errors
        );
        let d = compare_decoded_rgba(SIZE, SIZE, &reference, &frame.rgba).unwrap();
        report += &format!(
            "{} | {:?} | {:.2?} | {:.4} | {:.2} | {:.2}\n",
            f.name,
            d.maximum_channel_error,
            d.rms_channel_error,
            d.changed_pixel_fraction,
            d.edge_rms_error,
            d.interior_rms_error
        );
        for v in f.tolerance.violations(&d) {
            failures.push(format!("{}: {v}", f.name));
        }
    }
    println!("{report}");
    if let Ok(path) = std::env::var("KILSAT_PARITY_REPORT") {
        std::fs::write(path, &report).unwrap();
    }
    assert!(
        failures.is_empty(),
        "parity tolerance exceeded:\n{}",
        failures.join("\n")
    );
}

/// The pressure/soft-brush probe from `motion-render`'s ignored hardware test, rendered
/// through the CPU path at 64x64: this is the reference value the GPU probe is checked against.
fn pressure_probe_document() -> Document {
    let stroke = |radius, pressure, hardness, erase| motion_art::MaskStroke {
        points: vec![motion_art::Point { x: 32.5, y: 32.5 }],
        radius,
        pressures: vec![pressure],
        hardness,
        erase,
    };
    Document {
        layers: vec![motion_art::Layer {
            id: "mask".into(),
            scene_id: "s".into(),
            name: "mask".into(),
            visible: true,
            plane: motion_art::Plane::Foreground,
            active: None,
            opacity: 1.0,
            attachment: None,
            clip: None,
            blend: motion_art::BlendMode::Normal,
            content: Content::Rectangle {
                rect: rect(0.0, 0.0, 64.0, 64.0),
                color: rgba(255, 0, 0, 255),
            },
            clip_shape: Some(ClipShape::Painted {
                base_visible: false,
                strokes: vec![
                    stroke(40.0, 0.5, 0.5, false),
                    stroke(8.0, 1.0, 1.0, true),
                    stroke(2.0, 1.0, 1.0, false),
                ],
                motion: Vec::new(),
            }),
        }],
        ..Default::default()
    }
}

#[test]
fn cpu_soft_pressure_brush_profile_is_the_reference_for_the_gpu_probe() {
    let mut scene = no_actors("#000000");
    scene.width = 64;
    scene.height = 64;
    let svg = motion_render::render_svg_with_art(
        &scene,
        64,
        64,
        &[],
        "s",
        &pressure_probe_document(),
        &BTreeMap::new(),
    )
    .unwrap();
    let pixmap = resvg::tiny_skia::Pixmap::decode_png(&media::rasterize(&svg).unwrap()).unwrap();
    let red = |x: usize| pixmap.data()[(32 * 64 + x) * 4];
    let profile: Vec<u8> = (36..=54).map(red).collect();
    println!("CPU red for x=36..=54: {profile:?}");
    assert_eq!(red(32), 255); // repaint restores the erased centre
    assert_eq!(red(56), 0); // outside the pressure-scaled footprint
    assert!(
        profile[5..].windows(2).all(|w| w[0] >= w[1]),
        "soft edge must fall monotonically: {profile:?}"
    );
}
