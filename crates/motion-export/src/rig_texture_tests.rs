use super::*;
use std::sync::Arc;

const HEAD: &str = "project: {fps: 4, width: 256, height: 256}\ncharacters:\n  PUPPET:\n    asset: custom\n    rig:\n      version: 1\n      id: puppet\n";
const TRACKS: &str = "scenes:\n  - id: intro\n    duration: 2\n    events: []\n    composition:\n      joint_tracks:\n        PUPPET:\n";

fn quadrants() -> BTreeMap<String, motion_art::ImageResource> {
    // Opaque red, green / blue, white 2x2 image.
    let rgba = vec![
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];
    BTreeMap::from([(
        "tex".to_string(),
        motion_art::ImageResource::from_rgba(2, 2, rgba).unwrap(),
    )])
}
fn solid() -> BTreeMap<String, motion_art::ImageResource> {
    BTreeMap::from([(
        "tex".to_string(),
        motion_art::ImageResource::from_rgba(1, 1, vec![255, 0, 255, 255]).unwrap(),
    )])
}
fn rigid_script() -> Script {
    let source = format!(
        "{HEAD}      joints: [{{id: root, parent: null}}]\n      parts: [{{id: body, joint: root, points: [[-20,-20],[20,-20],[20,20],[-20,20]], color: [0,0,0,255], texture: {{asset: tex}}}}]\n{TRACKS}          root: [{{at: 0}}, {{at: 1, rotation: 1.5707963267948966}}, {{at: 2, rotation: 1.5707963267948966}}]\n"
    );
    motion_script::parse(&source).unwrap()
}
fn skinned_script() -> Script {
    let source = format!(
        "{HEAD}      joints: [{{id: root, parent: null}}, {{id: arm, parent: root, x: 10}}]\n      parts:\n        - id: bar\n          joint: root\n          points: [[0,-2],[10,-2],[20,-2],[20,2],[10,2],[0,2]]\n          color: [0,0,255,255]\n          texture: {{asset: tex}}\n          skin:\n            - [{{joint: root, weight: 1}}]\n            - [{{joint: root, weight: 0.5}}, {{joint: arm, weight: 0.5}}]\n            - [{{joint: arm, weight: 1}}]\n            - [{{joint: arm, weight: 1}}]\n            - [{{joint: root, weight: 0.5}}, {{joint: arm, weight: 0.5}}]\n            - [{{joint: root, weight: 1}}]\n{TRACKS}          arm: [{{at: 0}}, {{at: 1, rotation: 1.5707963267948966}}, {{at: 2, rotation: 1.5707963267948966}}]\n"
    );
    motion_script::parse(&source).unwrap()
}
fn raster(
    script: &Script,
    images: &BTreeMap<String, motion_art::ImageResource>,
    time: f64,
) -> (resvg::tiny_skia::Pixmap, (f64, f64)) {
    let scene = motion_scene::evaluate(script, 0, time).unwrap();
    let svg = motion_render::render_svg_with_art(
        &scene,
        256,
        256,
        &[],
        "intro",
        &motion_art::Document::default(),
        images,
    )
    .unwrap();
    let pixels = resvg::tiny_skia::Pixmap::decode_png(&media::rasterize(&svg).unwrap()).unwrap();
    (pixels, (scene.actors[0].x, scene.actors[0].y))
}
fn at(p: &resvg::tiny_skia::Pixmap, origin: (f64, f64), dx: f64, dy: f64) -> [u8; 3] {
    let c = p
        .pixel(
            (origin.0 + dx).floor() as u32,
            (origin.1 + dy).floor() as u32,
        )
        .unwrap();
    [c.red(), c.green(), c.blue()]
}

#[test]
fn texture_pixels_follow_the_joint_and_preview_matches_the_svg_png_path() {
    let script = rigid_script();
    let images = quadrants();
    let (rest, o) = raster(&script, &images, 0.0);
    // Probe beyond the interpolation band between image pixel centers.
    assert_eq!(at(&rest, o, -15.0, -15.0), [255, 0, 0]);
    assert_eq!(at(&rest, o, 15.0, -15.0), [0, 255, 0]);
    assert_eq!(at(&rest, o, -15.0, 15.0), [0, 0, 255]);
    assert_eq!(at(&rest, o, 15.0, 15.0), [255, 255, 255]);
    // A quarter turn clockwise moves the red quadrant to the top right.
    let (turned, o) = raster(&script, &images, 1.0);
    assert_eq!(at(&turned, o, 15.0, -15.0), [255, 0, 0]);
    assert_eq!(at(&turned, o, 15.0, 15.0), [0, 255, 0]);
    assert_eq!(at(&turned, o, -15.0, -15.0), [0, 0, 255]);
    assert_eq!(at(&turned, o, -15.0, 15.0), [255, 255, 255]);
    // Preview shares the evaluated scene and rasterizer, and seeking is deterministic.
    let art = Arc::new(ArtContext {
        document: motion_art::Document::default(),
        images,
    });
    let preview = ScenePreview::new(Arc::new(script.clone()), 0, art.clone(), None).unwrap();
    let cancel = CancellationToken::new();
    let mut first = Vec::new();
    for sample in [24000u64, 0, 24000, 12000, 0] {
        let frame = preview.render(sample, 256, &cancel).unwrap();
        let (expected, _) = raster(&script, &art.images, sample as f64 / 24000.0);
        assert_eq!(frame.rgba, expected.data());
        if sample == 24000 {
            first.push(frame.rgba);
        }
    }
    assert_eq!(first[0], first[1]);
}

#[test]
fn missing_texture_resources_fail_preflight_and_rendering() {
    let script = rigid_script();
    let scene = motion_scene::evaluate(&script, 0, 0.0).unwrap();
    let error = motion_render::render_svg_with_art(
        &scene,
        256,
        256,
        &[],
        "intro",
        &motion_art::Document::default(),
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert!(matches!(error, motion_render::Error::RigTexture(_)));
    assert!(error.to_string().contains("tex"));
    assert!(motion_render::render_svg(&scene, 256, 256).is_err());
    let empty = Arc::new(ArtContext::default());
    let error = match ScenePreview::new(Arc::new(script), 0, empty, None) {
        Err(error) => error.to_string(),
        Ok(_) => panic!("preview accepted a missing rig texture"),
    };
    assert!(error.contains("missing rig texture image resource: tex"));
}

#[test]
fn skinned_textured_part_draws_deformed_outline_and_is_deterministic_across_seeks() {
    let script = skinned_script();
    let images = solid();
    let (rest, o) = raster(&script, &images, 0.0);
    assert_eq!(at(&rest, o, 15.0, 0.0), [255, 0, 255]);
    assert_ne!(at(&rest, o, 10.0, 7.0), [255, 0, 255]); // not covered at rest
    let (bent, o) = raster(&script, &images, 1.0);
    // The far half of the bar swung down; the old location is empty again.
    assert_eq!(at(&bent, o, 10.0, 7.0), [255, 0, 255]);
    assert_ne!(at(&bent, o, 15.0, 0.0), [255, 0, 255]);
    for time in [1.0, 0.0, 1.0] {
        let again = raster(&script, &images, time).0;
        assert_eq!(
            again.data(),
            if time == 0.0 {
                rest.data()
            } else {
                bent.data()
            }
        );
    }
    // The exact hand-computed vertices are in the SVG polygon.
    let scene = motion_scene::evaluate(&script, 0, 1.0).unwrap();
    let part = &scene.actors[0].rigid_parts[0];
    assert!(part.skinned);
    assert!((part.points[1].x - 11.0).abs() < 1e-9 && (part.points[1].y + 1.0).abs() < 1e-9);
}

#[test]
fn legacy_rigs_load_and_serialize_without_texture_or_skin_fields() {
    let source = format!(
        "{HEAD}      joints: [{{id: root, parent: null}}]\n      parts: [{{id: body, joint: root, points: [[0,0],[10,0],[0,10]], color: [1,2,3,255]}}]\nscenes: [{{id: intro, duration: 2, events: []}}]\n"
    );
    let script = motion_script::parse(&source).unwrap();
    let part = &script.characters["PUPPET"].rig.as_ref().unwrap().parts[0];
    assert!(part.texture.is_none() && part.skin.is_empty());
    let json = serde_json::to_string(&script).unwrap();
    assert!(!json.contains("texture") && !json.contains("skin"));
    assert!(!motion_scene::evaluate(&script, 0, 0.0).unwrap().actors[0].rigid_parts[0].skinned);
}
