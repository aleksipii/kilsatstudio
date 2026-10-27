//! G2 minimal scene (KS-003c): two stick figures on the bus-stop background, a
//! wide shot and a hard cut to a close-up. Checks the render contract, depth
//! order, framing rules and the SVG reference hashes recorded in
//! `docs/evidence/g2-render/README.md`.
//!
//! The hand-held phone is not part of this scene yet: it waits for A05's
//! `grip` binding (KS-003a). See `docs/agents/A08-renderointi-kamera.md`.
use motion_render::contract::{depth_violations, paint_order, DepthBand, PaintSource};
use motion_render::framing::{check_framing, FramingRule, Hand, Subject};
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

const WIDE_SVG_SHA256: &str = "02bf7cab570d725a4ada591d8c6c20b73cb210b2c43ba96a38a0ca35b05f2004";
const CLOSE_SVG_SHA256: &str = "64bddae7203a1b062959678091f865e1733223a090832e8d59f940371f258449";

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
    // Face first (primary, size rule), then the raised hand that will hold the phone.
    vec![
        Subject::Head("VEIKKO".into()),
        Subject::Hand("VEIKKO".into(), Hand::Right),
    ]
}

#[test]
fn wide_shot_frames_both_figures_and_the_raised_hand() {
    let (script, _) = load();
    let scene = motion_scene::evaluate(&script, 0, WIDE_TIME).unwrap();
    assert_eq!(scene.camera.zoom, 1.0);
    let mut subjects = subjects_both();
    subjects.push(Subject::Hand("VEIKKO".into(), Hand::Right));
    assert_eq!(check_framing(&scene, &subjects, FramingRule::WIDE), vec![]);
}

#[test]
fn close_up_frames_face_and_raised_hand() {
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
