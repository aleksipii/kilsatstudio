//! KS-003d: in the G2 scene with screen cues, jumping straight to any frame
//! gives exactly the evaluated scene (and SVG) that playing frame by frame
//! from 0 gives, on the frame-seconds clock and the 24 kHz sample clock.
//!
//! The shipped `scene.kilsat` predates catalog screen slots, so the test
//! re-attaches the catalog phone with the scene's own placement (which copies
//! the `phone` slot) and adds cues: off → still image → a trimmed, looping,
//! sped-up video with brightness and glow → a trimmed video that holds its
//! last frame.
use motion_rig::props::{
    attach_hand_prop, detach_hand_prop, GripPlacement, HandPropCatalog, HandSide,
};
use motion_rig::ScreenContent;
use motion_scene::time::{frame_to_sample, frame_to_seconds};
use motion_scene::{frame_count, EvaluatedScene, ValidatedScript};
use motion_script::{ScreenCue, ScreenSource, Script};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

const SCENE_ID: &str = "g2_min";
/// `PHONE_PLACEMENT` in `examples/g2-minimal-scene/build_scene.py`.
const PHONE_PLACEMENT: GripPlacement = GripPlacement {
    x: 0.0,
    y: 0.0,
    rotation_degrees: 180.0,
    scale: 1.0,
};
const GLASS: &str = "heldprop_r_puhelin_screen";
const LOOPING: &str = "assets/screens/app-demo.mov";
const HOLDING: &str = "assets/screens/outro.mp4";

fn cue(at: f64, source: ScreenSource) -> ScreenCue {
    ScreenCue {
        actor: "VEIKKO".into(),
        slot: "phone".into(),
        at,
        source,
        fit: motion_rig::TextureFit::Cover,
        trim_start: 0.0,
        trim_end: None,
        speed: 1.0,
        looping: false,
        taps: Vec::new(),
        brightness: 1.0,
        glow: 0.0,
        glow_radius: None,
    }
}

fn load() -> (Script, motion_art::Document) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let project = motion_project::load(&root.join("examples/g2-minimal-scene/scene.kilsat"))
        .expect("scene.kilsat loads");
    let mut script = motion_script::parse(&project.source).expect("screenplay parses");
    let catalog = HandPropCatalog::decode(
        &std::fs::read_to_string(root.join("examples/stick-studio/hand-props.json")).unwrap(),
    )
    .unwrap();
    let phone = catalog.items.iter().find(|i| i.id == "puhelin").unwrap();
    let rig = script
        .characters
        .get_mut("VEIKKO")
        .unwrap()
        .rig
        .as_mut()
        .unwrap();
    detach_hand_prop(rig, HandSide::Right).unwrap();
    attach_hand_prop(rig, HandSide::Right, phone, PHONE_PLACEMENT).unwrap();
    assert!(rig.screens.iter().any(|s| s.id == "phone"));
    let looping = ScreenCue {
        trim_start: 0.4,
        trim_end: Some(1.3),
        speed: 1.5,
        looping: true,
        brightness: 1.2,
        glow: 0.5,
        ..cue(
            1.25,
            ScreenSource::Video {
                asset: LOOPING.into(),
                fps: 30.0,
                duration: 2.0,
            },
        )
    };
    let holding = ScreenCue {
        trim_start: 0.2,
        fit: motion_rig::TextureFit::Contain,
        ..cue(
            3.0,
            ScreenSource::Video {
                asset: HOLDING.into(),
                fps: 25.0,
                duration: 0.5,
            },
        )
    };
    script.scenes[0].composition.screens = vec![
        cue(0.25, ScreenSource::Off),
        cue(
            0.75,
            ScreenSource::Image {
                asset: "screens/still".into(),
            },
        ),
        looping,
        holding,
    ];
    script.validate().expect("cues validate");
    (script, project.art)
}

fn bits(scene: &EvaluatedScene) -> String {
    format!("{scene:?}")
}

fn glass(scene: &EvaluatedScene) -> &motion_rig::EvaluatedPart {
    scene
        .actors
        .iter()
        .find(|a| a.id == "VEIKKO")
        .unwrap()
        .rigid_parts
        .iter()
        .find(|p| p.id == GLASS)
        .unwrap()
}

#[test]
fn screen_cues_seek_equals_frame_by_frame_on_both_clocks() {
    let (script, art) = load();
    let fps = script.project.fps;
    let frames = frame_count(script.scenes[0].duration, fps, 10_000).unwrap();
    assert_eq!(frames, 96);
    let composition = &script.scenes[0].composition;
    // A tiny decoded image for every still and video frame the scene shows.
    let pixel = motion_art::ImageResource::from_rgba(1, 1, vec![40, 160, 220, 255]).unwrap();
    let mut images = BTreeMap::from([("screens/still".to_string(), pixel.clone())]);
    for f in 0..frames {
        for (asset, frame) in composition.video_frames_at(frame_to_seconds(f, fps).unwrap()) {
            images.insert(
                motion_script::video_frame_asset(asset, frame),
                pixel.clone(),
            );
        }
    }
    let svg = |scene: &EvaluatedScene| {
        motion_render::render_svg_with_art(scene, 480, 270, &[], SCENE_ID, &art, &images).unwrap()
    };

    // Playback: one long-lived evaluator stepping every frame in order.
    let new = || ValidatedScript::new(Arc::new(script.clone())).unwrap();
    let player = new();
    let played: Vec<EvaluatedScene> = (0..frames)
        .map(|f| {
            player
                .evaluate(0, frame_to_seconds(f, fps).unwrap())
                .unwrap()
        })
        .collect();

    // The cues are really exercised: drawn → off → still → looping video → held frame.
    let asset = |f: usize| glass(&played[f]).texture.as_ref().map(|t| t.asset.clone());
    assert_eq!(asset(0), None);
    assert_eq!(glass(&played[0]).color, glass(&played[5]).color, "drawn");
    assert_eq!(glass(&played[12]).color, motion_rig::SCREEN_OFF_COLOR);
    assert_eq!(asset(12), None, "off");
    assert_eq!(asset(24).as_deref(), Some("screens/still"));
    let looped: Vec<String> = (30..72).filter_map(asset).collect();
    assert!(looped
        .iter()
        .all(|a| a.starts_with(&format!("{LOOPING}#frame="))));
    // 0.9 s of source at 1.5x loops every 0.6 s: the frame numbers wrap.
    let numbers: Vec<u64> = looped
        .iter()
        .map(|a| a.rsplit('=').next().unwrap().parse().unwrap())
        .collect();
    assert!(numbers.iter().all(|n| (12..39).contains(n)), "{numbers:?}");
    assert!(
        numbers.windows(2).any(|w| w[1] < w[0]),
        "wraps: {numbers:?}"
    );
    let lit = glass(&played[40]).texture.as_ref().unwrap();
    assert_eq!(lit.brightness, 1.2);
    assert!(lit.glow.is_some());
    // The held cue ends on its last frame (12 of 0..=12) and stays there.
    let last = format!("{HOLDING}#frame=12");
    assert_eq!(asset(95).as_deref(), Some(last.as_str()));
    assert_eq!(asset(90), asset(95));
    assert!(glass(&played[95]).texture.as_ref().unwrap().glow.is_none());
    assert!(matches!(
        composition.screen_content("VEIKKO", "phone", 0.3),
        ScreenContent::Off
    ));

    let played_bits: Vec<String> = played.iter().map(bits).collect();
    let played_svg: Vec<String> = played.iter().map(svg).collect();
    // Seeks: scattered order and backwards scrubbing on a fresh evaluator,
    // plus a brand new evaluator per frame, on both clocks.
    let mut order: Vec<u64> = (0..frames).rev().collect();
    order.rotate_left(frames as usize / 3);
    let seeker = new();
    for f in order {
        let seconds = frame_to_seconds(f, fps).unwrap();
        let sample = frame_to_sample(f, fps).unwrap();
        let want = &played_bits[f as usize];
        let by_seconds = seeker.evaluate(0, seconds).unwrap();
        assert_eq!(&bits(&by_seconds), want, "seconds clock, frame {f}");
        let by_sample = seeker.evaluate_sample(0, sample).unwrap();
        assert_eq!(&bits(&by_sample), want, "sample clock, frame {f}");
        assert_eq!(
            &bits(&new().evaluate_sample(0, sample).unwrap()),
            want,
            "fresh evaluator, frame {f}"
        );
        assert_eq!(
            &bits(&motion_scene::evaluate(&script, 0, seconds).unwrap()),
            want
        );
        assert_eq!(
            svg(&by_sample),
            played_svg[f as usize],
            "rendered frame {f}"
        );
    }
}
