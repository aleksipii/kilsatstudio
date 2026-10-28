//! G2 minimal scene (KS-003c): two stick figures on the bus-stop background, a
//! catalog phone in VEIKKO's right hand, a wide shot and a hard cut to a
//! close-up. Checks the phone binding, the render contract, depth order, framing
//! rules and the SVG reference hashes recorded in
//! `docs/evidence/g2-render/README.md`.
use motion_render::contract::{depth_violations, paint_order, DepthBand, PaintSource};
use motion_render::framing::{check_framing, FramingRule, Hand, Subject};
use motion_rig::props::{
    attach_hand_prop, detach_hand_prop, GripPlacement, HandPropCatalog, HandSide,
};
use sha2::{Digest, Sha256};
use std::path::Path;

const SCENE_ID: &str = "g2_min";
/// Frame times at 24 fps: frame 24 (wide) and frame 72 (close-up). The camera
/// cut keys are at 1.99 s and 2.0 s, so no 24 fps frame samples the transition.
const WIDE_TIME: f64 = 1.0;
const CLOSE_TIME: f64 = 3.0;
/// Reference renders are half the 1920x1080 scene size.
const OUT_W: u32 = 960;
const OUT_H: u32 = 540;

/// `PHONE_PLACEMENT` in `examples/g2-minimal-scene/build_scene.py`: the raised
/// forearm points up and back, so a half turn stands the phone upright.
const PHONE_PLACEMENT: GripPlacement = GripPlacement {
    x: 0.0,
    y: 0.0,
    rotation_degrees: 180.0,
    scale: 1.0,
};
const PHONE_PREFIX: &str = "heldprop_r_puhelin_";

const WIDE_SVG_SHA256: &str = "6f6b2cd6ca918ac5c8969a25ba19f7de76d58952876bea58c154b561a7e4dde7";
const CLOSE_SVG_SHA256: &str = "5de41ad65eee7b6c9c5a690173b0984edc696605526097bd640c9bfac742c0f6";

fn load() -> (motion_script::Script, motion_art::Document) {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/g2-minimal-scene/scene.kilsat");
    let project = motion_project::load(&path).expect("scene.kilsat loads");
    let script = motion_script::parse(&project.source).expect("screenplay parses");
    (script, project.art)
}

fn render(script: &motion_script::Script, art: &motion_art::Document, time: f64) -> String {
    let scene = motion_scene::evaluate(script, 0, time).unwrap();
    motion_render::render_svg_with_art(
        &scene,
        OUT_W,
        OUT_H,
        &[],
        SCENE_ID,
        art,
        &Default::default(),
    )
    .unwrap()
}

fn sha256(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn subjects_both() -> Vec<Subject> {
    vec![
        Subject::Actor("VEIKKO".into()),
        Subject::Actor("KIPINA".into()),
    ]
}

fn close_subjects() -> Vec<Subject> {
    // Face first (primary, size rule), then the raised hand and the phone it holds.
    vec![
        Subject::Head("VEIKKO".into()),
        Subject::Hand("VEIKKO".into(), Hand::Right),
        Subject::HeldProp("VEIKKO".into(), Hand::Right),
    ]
}

/// Numbers equal within `tolerance`, everything else exactly. The generator and
/// `grip_parts` use the same formula, but trigonometry may differ in the last bit
/// between platform math libraries.
fn assert_close(actual: &serde_json::Value, expected: &serde_json::Value, path: &str) {
    use serde_json::Value;
    match (actual, expected) {
        (Value::Number(a), Value::Number(e)) => {
            let (a, e) = (a.as_f64().unwrap(), e.as_f64().unwrap());
            assert!((a - e).abs() <= 1e-9, "{path}: {a} != {e}");
        }
        (Value::Array(a), Value::Array(e)) => {
            assert_eq!(a.len(), e.len(), "{path}: length");
            for (i, (a, e)) in a.iter().zip(e).enumerate() {
                assert_close(a, e, &format!("{path}[{i}]"));
            }
        }
        (Value::Object(a), Value::Object(e)) => {
            let keys = |o: &serde_json::Map<String, Value>| {
                o.iter()
                    .filter(|(_, v)| !v.is_null())
                    .map(|(k, _)| k.clone())
                    .collect::<Vec<_>>()
            };
            assert_eq!(keys(a), keys(e), "{path}: fields");
            for (k, e) in e.iter().filter(|(_, v)| !v.is_null()) {
                assert_close(&a[k], e, &format!("{path}.{k}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn phone_is_the_catalog_item_attached_to_veikkos_right_hand() {
    let (script, _) = load();
    let rig = script.characters["VEIKKO"].rig.clone().expect("VEIKKO rig");
    let phone: Vec<_> = rig
        .parts
        .iter()
        .filter(|p| p.id.starts_with("heldprop_"))
        .collect();
    assert!(!phone.is_empty(), "the phone is in the scene");
    assert!(phone
        .iter()
        .all(|p| p.id.starts_with(PHONE_PREFIX) && p.joint == "hand_r"));
    assert!(script.characters["KIPINA"]
        .rig
        .as_ref()
        .unwrap()
        .parts
        .iter()
        .all(|p| !p.id.starts_with("heldprop_")));

    // Rebuild the binding with A05's function from the shipped catalog.
    let catalog_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/stick-studio/hand-props.json");
    let catalog = HandPropCatalog::decode(&std::fs::read_to_string(catalog_path).unwrap()).unwrap();
    let item = catalog.items.iter().find(|i| i.id == "puhelin").unwrap();
    let mut expected = rig.clone();
    detach_hand_prop(&mut expected, HandSide::Right).unwrap();
    attach_hand_prop(&mut expected, HandSide::Right, item, PHONE_PLACEMENT).unwrap();
    assert_close(
        &serde_json::to_value(&rig.parts).unwrap(),
        &serde_json::to_value(&expected.parts).unwrap(),
        "VEIKKO.parts",
    );
}

#[test]
fn phone_is_drawn_over_the_hand_that_holds_it() {
    let (script, _) = load();
    for time in [WIDE_TIME, CLOSE_TIME] {
        let scene = motion_scene::evaluate(&script, 0, time).unwrap();
        let veikko = scene.actors.iter().find(|a| a.id == "VEIKKO").unwrap();
        let ids: Vec<&str> = veikko.rigid_parts.iter().map(|p| p.id.as_str()).collect();
        let index = |id: &str| ids.iter().position(|p| *p == id).unwrap();
        let first_phone = ids
            .iter()
            .position(|p| p.starts_with(PHONE_PREFIX))
            .unwrap();
        // The phone paints after the arm and hand, and its fingers after its case.
        for part in ["upper_arm_r", "forearm_r", "hand_r", "face"] {
            assert!(index(part) < first_phone, "{part} paints before the phone");
        }
        assert!(ids[first_phone..]
            .iter()
            .all(|p| p.starts_with(PHONE_PREFIX)));
        assert!(index("heldprop_r_puhelin_case") < index("heldprop_r_puhelin_finger_0"));
        assert!(index("heldprop_r_puhelin_case") < index("heldprop_r_puhelin_thumb"));
    }
}

#[test]
fn wide_shot_frames_both_figures_and_the_raised_hand() {
    let (script, _) = load();
    let scene = motion_scene::evaluate(&script, 0, WIDE_TIME).unwrap();
    assert_eq!(scene.camera.zoom, 1.0);
    let mut subjects = subjects_both();
    subjects.push(Subject::Hand("VEIKKO".into(), Hand::Right));
    subjects.push(Subject::HeldProp("VEIKKO".into(), Hand::Right));
    assert_eq!(check_framing(&scene, &subjects, FramingRule::WIDE), vec![]);
}

#[test]
fn close_up_frames_face_raised_hand_and_phone() {
    let (script, _) = load();
    let scene = motion_scene::evaluate(&script, 0, CLOSE_TIME).unwrap();
    assert!(scene.camera.zoom > 2.0);
    assert_eq!(
        check_framing(&scene, &close_subjects(), FramingRule::CLOSE_UP),
        vec![]
    );
    // The close-up is a different shot: the wide rule for both figures fails.
    assert!(!check_framing(&scene, &subjects_both(), FramingRule::WIDE).is_empty());
}

#[test]
fn phone_stays_in_frame_on_every_frame_of_both_shots() {
    let (script, _) = load();
    let phone = [Subject::HeldProp("VEIKKO".into(), Hand::Right)];
    for frame in 0..96 {
        let scene = motion_scene::evaluate(&script, 0, frame as f64 / 24.0).unwrap();
        let rule = if frame < 48 {
            FramingRule::WIDE
        } else {
            FramingRule::CLOSE_UP
        };
        // The phone is not the primary subject; only the margin rule applies.
        let violations: Vec<_> = check_framing(&scene, &phone, rule)
            .into_iter()
            .filter(|v| !matches!(v, motion_render::framing::FramingViolation::TooSmall { .. }))
            .collect();
        assert_eq!(violations, vec![], "frame {frame}");
    }
}

#[test]
fn camera_cut_has_no_intermediate_frame_at_24_fps() {
    let (script, _) = load();
    let zooms: Vec<f64> = (0..96)
        .map(|frame| {
            motion_scene::evaluate(&script, 0, frame as f64 / 24.0)
                .unwrap()
                .camera
                .zoom
        })
        .collect();
    let close = zooms[95];
    assert!(zooms[..48].iter().all(|&z| z == 1.0));
    assert!(zooms[48..].iter().all(|&z| z == close));
}

#[test]
fn depth_order_is_banded_and_deterministic() {
    let (script, art) = load();
    let mut reference = None;
    for time in [CLOSE_TIME, 0.0, WIDE_TIME, 2.5, WIDE_TIME] {
        let scene = motion_scene::evaluate(&script, 0, time).unwrap();
        let order = paint_order(&scene, Some((SCENE_ID, &art))).unwrap();
        assert!(depth_violations(&order).is_empty());
        let bands: Vec<_> = order.iter().map(|i| i.band).collect();
        assert!(bands.windows(2).all(|w| w[0] <= w[1]));
        let actors: Vec<_> = order
            .iter()
            .filter_map(|i| match &i.source {
                PaintSource::Actor(id) => Some(id.as_str()),
                _ => None,
            })
            .collect();
        // Equal z: character ID order.
        assert_eq!(actors, ["KIPINA", "VEIKKO"]);
        assert_eq!(bands.last(), Some(&DepthBand::Foreground));
        match &reference {
            None => reference = Some(order),
            Some(r) => assert_eq!(&order, r),
        }
    }
}

#[test]
fn reference_svgs_are_deterministic_and_match_recorded_hashes() {
    let (script, art) = load();
    let wide = render(&script, &art, WIDE_TIME);
    let close = render(&script, &art, CLOSE_TIME);
    // Seeking backwards and re-rendering is byte-identical (no renderer clock).
    assert_eq!(render(&script, &art, CLOSE_TIME), close);
    assert_eq!(render(&script, &art, WIDE_TIME), wide);
    if let Ok(dir) = std::env::var("KILSAT_G2_RENDER_OUT") {
        std::fs::write(Path::new(&dir).join("g2-wide.svg"), &wide).unwrap();
        std::fs::write(Path::new(&dir).join("g2-close.svg"), &close).unwrap();
    }
    assert_eq!(sha256(&wide), WIDE_SVG_SHA256, "wide SVG hash changed");
    assert_eq!(
        sha256(&close),
        CLOSE_SVG_SHA256,
        "close-up SVG hash changed"
    );
}

#[test]
fn stick_studio_backgrounds_and_g2_scene_follow_background_style() {
    use motion_art::style::{check_background, BackgroundStyle};
    // Stick-studio scenes place characters at actor scale 1.6 on 1920x1080.
    let style = BackgroundStyle::stick_studio(1920.0, 1080.0, 1.6);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    for name in ["toimisto", "pysakki", "hautausmaa", "kahvila"] {
        let path = root.join(format!("stick-studio/backgrounds/{name}.kilsat"));
        let project = motion_project::load(&path).unwrap();
        assert!(project.art.layers.len() > 10, "{name} has its layers");
        assert_eq!(
            check_background(&project.art, "tausta", &style),
            vec![],
            "{name}"
        );
    }
    let (_, art) = load();
    assert_eq!(check_background(&art, SCENE_ID, &style), vec![]);
    // At actor scale 1 the cemetery tree trunk (28 px) is heavier than 2 x 9 px limbs.
    let unscaled = BackgroundStyle::stick_studio(1920.0, 1080.0, 1.0);
    let cemetery =
        motion_project::load(&root.join("stick-studio/backgrounds/hautausmaa.kilsat")).unwrap();
    assert!(!check_background(&cemetery.art, "tausta", &unscaled).is_empty());
}
