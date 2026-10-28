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
        brightness: 1.0,
        glow: 0.0,
        glow_radius: None,
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
    assert!(
        error.to_string().contains("missing decoded frame 15"),
        "{error}"
    );
    // The first frames alone are not enough: the first later frame the
    // export shows is named.
    let mut partial = BTreeMap::new();
    let image = motion_art::ImageResource::from_rgba(1, 1, vec![0, 0, 0, 255]).unwrap();
    let asset = "assets/screens/oma-sovellus-demo.mov";
    for frame in 0..30 {
        partial.insert(
            motion_script::video_frame_asset(asset, frame),
            image.clone(),
        );
    }
    let error = rig_texture_preflight(&script, Some(&partial)).unwrap_err();
    assert!(
        error.to_string().contains("missing decoded frame 30"),
        "{error}"
    );
    // With every frame the export needs, preflight passes.
    let mut complete = BTreeMap::new();
    for frame in 0..60 {
        complete.insert(
            motion_script::video_frame_asset(asset, frame),
            image.clone(),
        );
    }
    assert!(rig_texture_preflight(&script, Some(&complete)).is_ok());
    assert_eq!(
        composition.screen_content("VEIKKO", "phone", 0.2),
        ScreenContent::Drawn
    );
}

/// Grey picture cues on the DEVICE glass, rendered over a black scene.
fn lit_script(cues: Vec<motion_script::ScreenCue>) -> Script {
    let mut script = motion_script::parse(DEVICE).unwrap();
    script.scenes[0].composition.screens = cues;
    script.validate().unwrap();
    script
}
fn lit_cue(at: f64, asset: &str) -> motion_script::ScreenCue {
    motion_script::ScreenCue {
        actor: "DEVICE".into(),
        slot: "phone".into(),
        at,
        source: motion_script::ScreenSource::Image {
            asset: asset.into(),
        },
        fit: TextureFit::Stretch,
        trim_start: 0.0,
        trim_end: None,
        speed: 1.0,
        looping: false,
        taps: vec![],
        brightness: 1.0,
        glow: 0.0,
        glow_radius: None,
    }
}
fn dark_raster(script: &Script, time: f64) -> (resvg::tiny_skia::Pixmap, (f64, f64)) {
    let mut scene = motion_scene::evaluate(script, 0, time).unwrap();
    scene.background_color = "#000000".into();
    let images = BTreeMap::from([
        (
            "grey".to_string(),
            motion_art::ImageResource::from_rgba(2, 1, [100, 100, 100, 255].repeat(2)).unwrap(),
        ),
        (
            "black".to_string(),
            motion_art::ImageResource::from_rgba(2, 1, [0, 0, 0, 255].repeat(2)).unwrap(),
        ),
    ]);
    let svg = motion_render::render_svg_with_art(
        &scene,
        256,
        256,
        &[],
        "intro",
        &motion_art::Document::default(),
        &images,
    )
    .unwrap();
    let pixels = resvg::tiny_skia::Pixmap::decode_png(&media::rasterize(&svg).unwrap()).unwrap();
    (pixels, (scene.actors[0].x, scene.actors[0].y))
}

#[test]
fn brightness_scales_the_picture_and_glow_lights_a_dark_scene() {
    let script = lit_script(vec![
        lit_cue(0.5, "grey"),
        motion_script::ScreenCue {
            brightness: 2.0,
            ..lit_cue(1.0, "grey")
        },
        motion_script::ScreenCue {
            brightness: 0.5,
            ..lit_cue(1.5, "grey")
        },
        motion_script::ScreenCue {
            glow: 1.0,
            glow_radius: Some(0.5),
            ..lit_cue(2.0, "grey")
        },
        motion_script::ScreenCue {
            glow: 1.0,
            brightness: 2.0,
            glow_radius: Some(0.5),
            ..lit_cue(2.5, "grey")
        },
        motion_script::ScreenCue {
            glow: 1.0,
            ..lit_cue(3.0, "black")
        },
        {
            let mut off = lit_cue(4.0, "grey");
            off.source = motion_script::ScreenSource::Off;
            off
        },
    ]);
    let near =
        |v: [u8; 3], want: u8, tolerance: u8| v.iter().all(|c| c.abs_diff(want) <= tolerance);
    // Brightness 1: the picture as is, and nothing outside the 40x80 glass.
    let (p, o) = dark_raster(&script, 0.75);
    assert_eq!(at(&p, o, 0.0, 0.0), [100, 100, 100]);
    assert_eq!(at(&p, o, 25.0, 0.0), [0, 0, 0]);
    // Brightness 2 doubles the picture; 0.5 halves it.
    let (p, o) = dark_raster(&script, 1.25);
    let centre = at(&p, o, 0.0, 0.0);
    assert!(near(centre, 200, 1), "{centre:?}");
    assert_eq!(
        at(&p, o, 25.0, 0.0),
        [0, 0, 0],
        "brightness alone does not glow"
    );
    let (p, o) = dark_raster(&script, 1.75);
    let centre = at(&p, o, 0.0, 0.0);
    assert!(near(centre, 50, 1), "{centre:?}");
    // Glow: a neutral light outside the glass that fades with distance and
    // ends at the ellipse (half-axes 20 + 40 and 40 + 40); the picture itself
    // is unchanged because the glow is drawn behind the device.
    let (p, o) = dark_raster(&script, 2.25);
    assert_eq!(at(&p, o, 0.0, 0.0), [100, 100, 100]);
    let close = at(&p, o, 25.0, 0.0);
    let far = at(&p, o, 45.0, 0.0);
    assert!(
        close[0] > 30 && close[0] == close[1] && close[1] == close[2],
        "{close:?}"
    );
    assert!(far[0] > 0 && far[0] < close[0], "{close:?} {far:?}");
    assert!(at(&p, o, 0.0, -50.0)[0] > 0, "above the glass");
    assert_eq!(at(&p, o, 65.0, 0.0), [0, 0, 0], "beyond the glow");
    // A brighter screen glows more.
    let (p, o) = dark_raster(&script, 2.75);
    assert!(at(&p, o, 25.0, 0.0)[0] > close[0]);
    // A black picture and an off screen never glow.
    let (p, o) = dark_raster(&script, 3.5);
    assert_eq!(at(&p, o, 25.0, 0.0), [0, 0, 0]);
    let (p, o) = dark_raster(&script, 4.5);
    assert_eq!(at(&p, o, 25.0, 0.0), [0, 0, 0]);
    let off = [
        SCREEN_OFF_COLOR[0],
        SCREEN_OFF_COLOR[1],
        SCREEN_OFF_COLOR[2],
    ];
    assert_eq!(at(&p, o, 0.0, 0.0), off);
}

#[test]
fn export_streams_decoded_video_frames_from_the_cache() {
    let script = veikko_with_phone();
    let asset = "assets/screens/oma-sovellus-demo.mov";
    let directory = std::env::temp_dir().join(format!(
        "kilsat-screen-stream-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    // Frame n is a 2x2 image whose red channel encodes n.
    for frame in 0..60u8 {
        let image =
            motion_art::ImageResource::from_rgba(2, 2, [frame, 0, 0, 255].repeat(4)).unwrap();
        std::fs::write(directory.join(format!("{frame:06}.png")), image.png()).unwrap();
    }
    let decoded = screen_video::DecodedScreenVideo {
        directory: directory.clone(),
        frames: 60,
        color: screen_video::ScreenVideoColor::Bt709,
        source_bytes: 0,
        recipe: screen_video::SCREEN_VIDEO_RECIPE,
    };
    let art = ArtContext {
        screen_videos: BTreeMap::from([(asset.to_string(), decoded)]),
        ..Default::default()
    };
    // No frame is held in memory up front, yet preflight passes: every frame
    // the export shows exists in the cache.
    assert!(art.images.is_empty());
    art_preflight(&script, Some(&art)).unwrap();
    assert_eq!(art.screen_frame_bytes().unwrap(), 16);
    // A frame loads only for the instant that shows it.
    let scene = &script.scenes[0];
    assert!(matches!(
        art.images_at(scene, 0.5).unwrap(),
        std::borrow::Cow::Borrowed(_)
    ));
    let images = art.images_at(scene, 2.6).unwrap();
    assert_eq!(images.len(), 1);
    let id = motion_script::video_frame_asset(asset, 18);
    let decoded_frame =
        motion_art::ImageResource::from_rgba(2, 2, [18, 0, 0, 255].repeat(4)).unwrap();
    assert_eq!(images[&id].png(), decoded_frame.png());
    // A missing cache file fails preflight before any frame is encoded.
    std::fs::remove_file(directory.join("000030.png")).unwrap();
    let error = art_preflight(&script, Some(&art)).unwrap_err();
    assert!(
        error.to_string().contains("missing decoded frame 30"),
        "{error}"
    );
    std::fs::remove_dir_all(&directory).unwrap();
}
