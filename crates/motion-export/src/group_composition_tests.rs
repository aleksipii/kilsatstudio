use super::*;
use motion_art::{ArtUnit, BlendMode, Color, Content, Document, Group, Layer, Plane, Rect};

#[test]
fn supplied_studio_demo_reopens_and_shares_normalized_rig_group_preview_pixels() {
    use std::sync::Arc;
    let document = motion_project::decode(include_bytes!(
        "../../../examples/studio-production-demo.kilsat"
    ))
    .unwrap();
    let reopened = motion_project::decode(&motion_project::encode(&document).unwrap()).unwrap();
    assert_eq!(reopened.source, document.source);
    let script = motion_script::parse(&reopened.source).unwrap();
    let art = Arc::new(ArtContext {
        document: reopened.art,
        images: BTreeMap::new(),
    });
    let preview = ScenePreview::new(Arc::new(script.clone()), 0, art.clone(), None).unwrap();
    let cancel = CancellationToken::new();
    for sample in [72000u64, 0, 24000, 48000, 24000] {
        let scene = motion_scene::evaluate(&script, 0, sample as f64 / 24000.0).unwrap();
        assert_eq!(
            scene
                .actors
                .iter()
                .find(|actor| actor.id == "PUPPET")
                .unwrap()
                .rigid_parts
                .len(),
            3
        );
        let svg = motion_render::render_svg_with_art(
            &scene,
            640,
            360,
            &[],
            "studio_demo",
            &art.document,
            &art.images,
        )
        .unwrap();
        let pixels =
            resvg::tiny_skia::Pixmap::decode_png(&media::rasterize(&svg).unwrap()).unwrap();
        assert_eq!(
            preview.render(sample, 640, &cancel).unwrap().rgba,
            pixels.data()
        );
    }
    let mut expected = script.clone();
    expected.scenes[0].composition.clip_instances.clear();
    expected.scenes[0].composition.bone_tracks.insert(
        "EMMA".into(),
        BTreeMap::from([(
            "right_hand".into(),
            vec![motion_script::ActorKey {
                x: 19.0,
                y: -25.0,
                ..Default::default()
            }],
        )]),
    );
    let actual = motion_scene::evaluate(&script, 0, 1.0).unwrap();
    let expected = motion_scene::evaluate(&expected, 0, 1.0).unwrap();
    assert_eq!(
        actual
            .actors
            .iter()
            .find(|actor| actor.id == "EMMA")
            .unwrap()
            .skeleton,
        expected
            .actors
            .iter()
            .find(|actor| actor.id == "EMMA")
            .unwrap()
            .skeleton
    );
}

fn leaf(id: &str, color: Color, x: f64) -> Layer {
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
        content: Content::Rectangle {
            rect: Rect {
                x,
                y: 0.0,
                width: 40.0,
                height: 40.0,
            },
            color,
        },
    }
}
fn raster(doc: &Document) -> resvg::tiny_skia::Pixmap {
    let script = motion_script::parse("project: {fps: 4, width: 256, height: 256}\ncharacters: {EMMA: {asset: characters/emma}}\nscenes: [{id: intro, duration: 1, events: []}]\n").unwrap();
    let mut scene = motion_scene::evaluate(&script, 0, 0.0).unwrap();
    scene.actors.clear();
    scene.background_color = "#000000".into();
    let svg =
        motion_render::render_svg_with_art(&scene, 256, 256, &[], "intro", doc, &BTreeMap::new())
            .unwrap();
    resvg::tiny_skia::Pixmap::decode_png(&media::rasterize(&svg).unwrap()).unwrap()
}
#[test]
fn isolated_opacity_applies_once_after_overlapping_children_and_nested_roundtrip() {
    let red = Color {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    let blue = Color {
        r: 0,
        g: 0,
        b: 255,
        a: 255,
    };
    let mut doc = Document {
        layers: vec![leaf("red", red, 0.0), leaf("blue", blue, 20.0)],
        ..Default::default()
    };
    doc.apply(motion_art::Command::CreateGroup(Group {
        id: "outer".into(),
        scene_id: "intro".into(),
        plane: Plane::Foreground,
        name: "Outer".into(),
        visible: true,
        opacity: 0.5,
        blend: BlendMode::Normal,
        children: Vec::new(),
    }))
    .unwrap();
    for (index, id) in ["red", "blue"].into_iter().enumerate() {
        doc.apply(motion_art::Command::Move {
            unit: ArtUnit::Layer(id.into()),
            parent: Some("outer".into()),
            index,
        })
        .unwrap();
    }
    let pixels = raster(&doc);
    let channel = |x, y| pixels.pixel(x, y).unwrap();
    // Opaque blue covers red before the completed group is blended at 50%.
    let overlap = channel(30, 20);
    assert_eq!(overlap.red(), 0);
    assert_eq!(overlap.green(), 0);
    assert!((127..=128).contains(&overlap.blue()));
    let red_only = channel(10, 20);
    assert!((127..=128).contains(&red_only.red()));
    assert_eq!(red_only.blue(), 0);
    let restored: Document = serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
    assert_eq!(pixels.data(), raster(&restored).data());
    doc.groups[0].visible = false;
    let hidden = raster(&doc);
    assert_eq!(hidden.pixel(30, 20).unwrap().blue(), 0);
}
#[test]
fn nested_group_opacity_multiplies_completed_surfaces_and_ungroup_preserves_order() {
    let mut doc = Document {
        layers: vec![leaf(
            "white",
            Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            0.0,
        )],
        ..Default::default()
    };
    for id in ["outer", "inner"] {
        doc.apply(motion_art::Command::CreateGroup(Group {
            id: id.into(),
            scene_id: "intro".into(),
            plane: Plane::Foreground,
            name: id.into(),
            visible: true,
            opacity: 0.5,
            blend: BlendMode::Normal,
            children: Vec::new(),
        }))
        .unwrap();
    }
    doc.apply(motion_art::Command::Move {
        unit: ArtUnit::Layer("white".into()),
        parent: Some("inner".into()),
        index: 0,
    })
    .unwrap();
    doc.apply(motion_art::Command::Move {
        unit: ArtUnit::Group("inner".into()),
        parent: Some("outer".into()),
        index: 0,
    })
    .unwrap();
    let pixels = raster(&doc);
    let p = pixels.pixel(20, 20).unwrap();
    assert!((63..=64).contains(&p.red()));
    assert_eq!(p.red(), p.green());
    assert_eq!(p.red(), p.blue());
    doc.apply(motion_art::Command::Ungroup { id: "inner".into() })
        .unwrap();
    let pixels = raster(&doc);
    assert!((127..=128).contains(&pixels.pixel(20, 20).unwrap().red()));
}

fn grouped_fixture() -> (Script, RenderedScene, ArtContext) {
    let (mut script, mut rendered) = crate::tests::fixture();
    script.project.width = 100;
    script.project.height = 100;
    script.characters.clear();
    script.scenes[0].composition.background_color = "#000000".into();
    rendered.manifest.dialogue.clear();
    let mut blue = leaf(
        "blue",
        Color {
            r: 0,
            g: 0,
            b: 255,
            a: 255,
        },
        20.0,
    );
    blue.active = Some(motion_art::TimeRange {
        start: 0.0,
        end: 0.5,
    });
    let mut document = Document {
        layers: vec![
            leaf(
                "red",
                Color {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                0.0,
            ),
            blue,
        ],
        ..Default::default()
    };
    document
        .apply(motion_art::Command::CreateGroup(Group {
            id: "outer".into(),
            scene_id: "intro".into(),
            plane: Plane::Foreground,
            name: "Outer".into(),
            visible: true,
            opacity: 0.5,
            blend: BlendMode::Normal,
            children: Vec::new(),
        }))
        .unwrap();
    for (index, id) in ["red", "blue"].into_iter().enumerate() {
        document
            .apply(motion_art::Command::Move {
                unit: ArtUnit::Layer(id.into()),
                parent: Some("outer".into()),
                index,
            })
            .unwrap();
    }
    (
        script,
        rendered,
        ArtContext {
            document,
            images: BTreeMap::new(),
        },
    )
}

#[test]
fn grouped_png_export_matches_preview_on_backwards_seeks_and_half_open_child_timing() {
    use std::sync::Arc;
    let (script, rendered, mut art) = grouped_fixture();
    // Exercise the persisted representation, rather than sharing editor-only state.
    art.document = serde_json::from_str(&serde_json::to_string(&art.document).unwrap()).unwrap();
    let cancel = CancellationToken::new();
    let destination = crate::tests::destination();
    export_png_scene_with_art(&script, 0, &rendered, &art, &destination, &cancel, |_| {}).unwrap();
    let preview = ScenePreview::new(
        Arc::new(script.clone()),
        0,
        Arc::new(art),
        Some(Arc::new(rendered.manifest.clone())),
    )
    .unwrap();
    for index in [3u64, 0, 2, 1, 2] {
        let png = fs::read(destination.join(format!("frame_{index:06}.png"))).unwrap();
        let actual = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
        let sample = index * u64::from(SAMPLE_RATE) / u64::from(script.project.fps);
        let frame = preview.render(sample, 100, &cancel).unwrap();
        assert_eq!(frame.rgba, actual.data());
        let overlap = actual.pixel(30, 20).unwrap();
        // Child end is excluded at exactly 0.5 s. Group opacity still applies once.
        if index < 2 {
            assert_eq!(overlap.red(), 0);
            assert!((127..=128).contains(&overlap.blue()));
        } else {
            assert!((127..=128).contains(&overlap.red()));
            assert_eq!(overlap.blue(), 0);
        }
    }
    fs::remove_dir_all(destination).unwrap();
}

#[test]
fn dormant_group_original_canvas_budget_rejects_before_export_and_preview_allocation() {
    use std::sync::Arc;
    let (mut script, rendered, mut art) = grouped_fixture();
    script.project.width = 3000;
    script.project.height = 3000;
    art.document.groups[0].visible = false;
    // Preview scaling and invisibility cannot bypass original-canvas isolation budgets.
    let error = group_work(&art, &script, 1, false).unwrap_err();
    assert!(error.to_string().contains("isolated-group"));
    let error = ScenePreview::new(Arc::new(script.clone()), 0, Arc::new(art.clone()), None)
        .err()
        .expect("oversized dormant groups must fail preview preflight");
    assert!(error.to_string().contains("isolated-group"));
    let destination = crate::tests::destination();
    let error = export_png_sequence(
        &script,
        &[rendered],
        Some(&art),
        &destination,
        &CancellationToken::new(),
        |_| {},
    )
    .unwrap_err();
    assert!(error.to_string().contains("isolated-group"));
    assert!(!destination.exists());
}

#[test]
fn group_targets_and_retained_images_share_single_scene_working_set_budget() {
    let (mut script, rendered, mut art) = grouped_fixture();
    script.project.width = 2000;
    script.project.height = 2000;
    assert!(group_work(&art, &script, 4, false).unwrap() < 128 * 1024 * 1024);
    let image = motion_art::ImageResource::from_rgba(2000, 2000, vec![0; 2000 * 2000 * 4]).unwrap();
    art.images.insert("retained".into(), image);
    let destination = crate::tests::destination();
    let error = export_png_scene_with_art(&script, 0, &rendered, &art, &destination, &CancellationToken::new(), |_| {}).unwrap_err();
    assert!(error.to_string().contains("grouped scene working set"));
    assert!(!destination.exists());
}
