//! Screen slots end to end: MotionScript cues → scene evaluation → SVG pixels.
use super::*;
use motion_rig::{
    attach_hand_prop, GripPlacement, HandPropCatalog, HandSide, ScreenContent, TextureFit,
    SCREEN_OFF_COLOR,
};
use motion_scene::time::{frame_to_sample, frame_to_seconds};
use std::sync::Arc;

/// A 40x80 glass with a slot, one drawn UI overlay and screen cues.
const DEVICE: &str = r#"
project: {fps: 24, width: 256, height: 256}
characters:
  DEVICE:
    asset: custom
    rig:
      version: 1
      id: device
      joints: [{id: root, parent: null}]
      parts:
        - {id: glass, joint: root, points: [[-20,-40],[20,-40],[20,40],[-20,40]], color: [250,250,250,255]}
        - {id: ui, joint: root, points: [[-10,-10],[10,-10],[10,10],[-10,10]], color: [0,0,255,255]}
      screens:
        - {id: phone, part: glass, origin: [-20,-40], x_axis: [40,0], y_axis: [0,80], overlays: [ui]}
scenes:
  - id: intro
    duration: 5
    events: []
    composition:
      screens:
        - {actor: DEVICE, slot: phone, at: 1, source: {type: image, asset: wide}, fit: contain}
        - {actor: DEVICE, slot: phone, at: 2, source: {type: image, asset: wide}}
        - {actor: DEVICE, slot: phone, at: 3, source: {type: image, asset: wide}, fit: stretch}
        - {actor: DEVICE, slot: phone, at: 4, source: {type: "off"}}
"#;

/// 2x1: red | green.
fn images() -> BTreeMap<String, motion_art::ImageResource> {
    BTreeMap::from([(
        "wide".to_string(),
        motion_art::ImageResource::from_rgba(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 255]).unwrap(),
    )])
}

fn raster(script: &Script, time: f64) -> (resvg::tiny_skia::Pixmap, (f64, f64)) {
    let scene = motion_scene::evaluate(script, 0, time).unwrap();
    let svg = motion_render::render_svg_with_art(
        &scene,
        256,
        256,
        &[],
        "intro",
        &motion_art::Document::default(),
        &images(),
    )
    .unwrap();
    let pixels = resvg::tiny_skia::Pixmap::decode_png(&media::rasterize(&svg).unwrap()).unwrap();
    (pixels, (scene.actors[0].x, scene.actors[0].y))
}
fn at(p: &resvg::tiny_skia::Pixmap, o: (f64, f64), dx: f64, dy: f64) -> [u8; 3] {
    let c = p
        .pixel((o.0 + dx).floor() as u32, (o.1 + dy).floor() as u32)
        .unwrap();
    [c.red(), c.green(), c.blue()]
}

#[test]
fn drawn_contain_cover_stretch_and_off_pixels() {
    let script = motion_script::parse(DEVICE).unwrap();
    // Before the first cue: the rig's own drawing, overlay visible.
    let (p, o) = raster(&script, 0.5);
    assert_eq!(at(&p, o, 0.0, 0.0), [0, 0, 255]);
    assert_eq!(at(&p, o, 0.0, -30.0), [250, 250, 250]);
    // Contain: a 2:1 image in a 1:2 slot is 40x20 centred; black bands.
    let (p, o) = raster(&script, 1.5);
    assert_eq!(at(&p, o, -15.0, 0.0), [255, 0, 0]);
    assert_eq!(at(&p, o, 15.0, 0.0), [0, 255, 0]);
    assert_eq!(at(&p, o, -15.0, -30.0), [0, 0, 0]);
    assert_eq!(at(&p, o, 15.0, 30.0), [0, 0, 0]);
    // Cover (the default): the centre quarter of the image fills the whole
    // glass with no bands, so only the blend between red and green shows.
    let (p, o) = raster(&script, 2.5);
    for dy in [-35.0, 0.0, 35.0] {
        let (left, right) = (at(&p, o, -15.0, dy), at(&p, o, 15.0, dy));
        assert!(
            left[0] > left[1] && right[1] > right[0],
            "{left:?} {right:?}"
        );
        assert!(left[0] < 255 && right[1] < 255, "cropped to the centre");
    }
    // Stretch: whole image over the whole glass.
    let (p, o) = raster(&script, 3.5);
    assert_eq!(at(&p, o, -15.0, -35.0), [255, 0, 0]);
    assert_eq!(at(&p, o, 15.0, -35.0), [0, 255, 0]);
    // Off: dark glass, overlay still hidden.
    let (p, o) = raster(&script, 4.5);
    let off = [
        SCREEN_OFF_COLOR[0],
        SCREEN_OFF_COLOR[1],
        SCREEN_OFF_COLOR[2],
    ];
    assert_eq!(at(&p, o, 0.0, 0.0), off);
    assert_eq!(at(&p, o, 0.0, -30.0), off);
}

#[test]
fn missing_screen_images_fail_export_preflight() {
    let script = motion_script::parse(DEVICE).unwrap();
    assert!(rig_texture_preflight(&script, Some(&images())).is_ok());
    let error = rig_texture_preflight(&script, Some(&BTreeMap::new())).unwrap_err();
    assert!(error
        .to_string()
        .contains("missing screen image resource: wide"));
}

/// Veikko holds the catalog phone; a looping, trimmed recording plays on it.
fn veikko_with_phone() -> Script {
    let project: serde_json::Value = serde_json::from_str(include_str!(
        "../../../examples/stick-studio/characters/veikko.kilsat"
    ))
    .unwrap();
    let mut script = motion_script::parse(project["source"].as_str().unwrap()).unwrap();
    let catalog = HandPropCatalog::decode(include_str!(
        "../../../examples/stick-studio/hand-props.json"
    ))
    .unwrap();
    let phone = catalog.items.iter().find(|i| i.id == "puhelin").unwrap();
    let rig = script
        .characters
        .get_mut("VEIKKO")
        .unwrap()
        .rig
        .as_mut()
        .unwrap();
    let placement = GripPlacement {
        x: 0.0,
        y: 4.0,
        rotation_degrees: -10.0,
        scale: 0.8,
    };
    attach_hand_prop(rig, HandSide::Right, phone, placement).unwrap();
    script.scenes[0].composition.screens = vec![motion_script::ScreenCue {
        actor: "VEIKKO".into(),
        slot: "phone".into(),
        at: 1.0,
        source: motion_script::ScreenSource::Video {
            asset: "assets/screens/oma-sovellus-demo.mov".into(),
            fps: 30.0,
            duration: 2.0,
        },
        fit: TextureFit::Cover,
        trim_start: 0.5,
        trim_end: None,
        speed: 1.0,
        looping: true,
        taps: vec![1.2],
    }];
    script.validate().unwrap();
    script
}

#[test]
fn phone_video_frame_follows_the_scene_clock_in_preview_and_export() {
    let script = veikko_with_phone();
    let validated = motion_scene::ValidatedScript::new(Arc::new(script.clone())).unwrap();
    let screen = |time: f64| {
        let scene = validated.evaluate(0, time).unwrap();
        let actor = &scene.actors[0];
        let glass = actor
            .rigid_parts
            .iter()
            .find(|p| p.id == "heldprop_r_puhelin_screen")
            .unwrap()
            .clone();
        let message = actor
            .rigid_parts
            .iter()
            .find(|p| p.id == "heldprop_r_puhelin_message")
            .unwrap()
            .color[3];
        let reflection = actor
            .rigid_parts
            .iter()
            .find(|p| p.id == "heldprop_r_puhelin_reflection")
            .unwrap()
            .color[3];
        (glass, message, reflection)
    };
    // Drawn UI before the cue.
    let (glass, message, _) = screen(0.5);
    assert!(glass.texture.is_none() && message == 255);
    // At 2.6 s: source 0.5 + 1.6 loops over the 1.5 s span to 0.6 s → frame 18.
    let (glass, message, reflection) = screen(2.6);
    let texture = glass.texture.unwrap();
    assert_eq!(
        texture.asset,
        "assets/screens/oma-sovellus-demo.mov#frame=18"
    );
    assert_eq!(texture.fit, TextureFit::Cover);
    assert_eq!(message, 0, "drawn UI hides under the recording");
    assert_eq!(
        reflection, 255,
        "the glass reflection stays over the picture"
    );
    // Preview (sample clock) and export (frame seconds) pick the same frame.
    let fps = script.project.fps;
    for frame in 0..(5 * fps as u64) {
        let seconds = frame_to_seconds(frame, fps).unwrap();
        let sample = frame_to_sample(frame, fps).unwrap();
        assert_eq!(
            format!("{:?}", validated.evaluate(0, seconds).unwrap()),
            format!("{:?}", validated.evaluate_sample(0, sample).unwrap())
        );
    }
    let composition = &script.scenes[0].composition;
    assert_eq!(
        composition.video_frames_at(2.6),
        vec![("assets/screens/oma-sovellus-demo.mov", 18)]
    );
    // The tap marker at source 1.2 s repeats with every loop.
    let taps = composition.screens[0].tap_times(5.0);
    for (got, want) in taps.iter().zip([1.7, 3.2, 4.7]) {
        assert!((got - want).abs() < 1e-9, "{taps:?}");
    }
    assert_eq!(taps.len(), 3);
    // Export without decoded frames explains what is missing.
    let error = rig_texture_preflight(&script, Some(&BTreeMap::new())).unwrap_err();
    assert!(error.to_string().contains("has no decoded frames"));
    assert_eq!(
        composition.screen_content("VEIKKO", "phone", 0.2),
        ScreenContent::Drawn
    );
}
