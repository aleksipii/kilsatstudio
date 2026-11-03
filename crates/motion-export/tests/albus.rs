//! Albus, the guitar-playing cat (examples/albus): the rig validates and gives a
//! complete model sheet, every expression changes the face, and both scenes
//! (moves and expressions) evaluate and render through the shared pipeline.
//!
//! `ALBUS_OUT=<dir>` also writes the model sheet and frames as PNG for review.
use motion_art::style::CharacterStyle;
use motion_render::model_sheet::{render_model_sheet, ModelSheetOptions};
use motion_rig::{FaceView, RigDefinition};
use motion_scene::{EvaluatedScene, ValidatedScript};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

const FPS: u32 = 24;

fn example(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/albus")
        .join(relative)
}

fn rig() -> RigDefinition {
    let project = motion_project::load(&example("characters/albus.kilsat")).unwrap();
    let script = motion_script::parse(&project.source).expect("Albus character parses");
    script.characters["ALBUS"]
        .rig
        .clone()
        .expect("Albus has a rig")
}

fn demo() -> (ValidatedScript, motion_art::Document) {
    let project = motion_project::load(&example("albus.kilsat")).unwrap();
    let script = motion_script::parse(&project.source).expect("Albus script compiles");
    (
        ValidatedScript::new(Arc::new(script)).expect("Albus script validates"),
        project.art,
    )
}

fn at(script: &ValidatedScript, scene: usize, seconds: f64) -> EvaluatedScene {
    let frame = (seconds * f64::from(FPS)).round() as u64;
    let time = motion_scene::time::frame_to_seconds(frame, FPS).unwrap();
    script.evaluate(scene, time).unwrap()
}

fn out_dir() -> Option<PathBuf> {
    std::env::var_os("ALBUS_OUT").map(PathBuf::from)
}

#[test]
fn albus_has_a_complete_model_sheet() {
    let r = rig();
    r.prepare().unwrap_or_else(|e| panic!("{e}"));
    let options = ModelSheetOptions {
        style: CharacterStyle::Default,
        images: None,
    };
    let sheet = render_model_sheet(&r, &options).unwrap();
    assert_eq!(
        sheet,
        render_model_sheet(&r, &options).unwrap(),
        "byte-stable"
    );
    assert_eq!(
        sheet.expressions,
        ["neutral", "happy", "sad", "surprised", "angry", "blink"]
    );
    assert_eq!(sheet.mouths, motion_rig::VISEME_NAMES);
    if let Some(dir) = out_dir() {
        let png = motion_export::rasterize_svg_png(&sheet.svg).unwrap();
        std::fs::write(dir.join("albus-model-sheet.png"), png).unwrap();
    }
}

#[test]
fn every_expression_changes_the_face_and_ears_stay_from_behind() {
    let prepared = rig().prepare().unwrap();
    let pose = |emotion, view| {
        prepared
            .evaluate_acting(
                &[],
                motion_rig::ActingInput {
                    emotion,
                    blink: false,
                    view,
                    hands: Default::default(),
                },
            )
            .unwrap()
            .0
    };
    let neutral = pose("neutral", FaceView::Front);
    for emotion in ["happy", "sad", "surprised", "angry"] {
        assert_ne!(pose(emotion, FaceView::Front), neutral, "{emotion}");
    }
    let part = |parts: &[motion_rig::EvaluatedPart], id: &str| {
        parts.iter().find(|p| p.id == id).unwrap().clone()
    };
    // Angry flattens the ears and bares the fangs.
    let angry = pose("angry", FaceView::Front);
    assert_ne!(part(&angry, "ear_l").points, part(&neutral, "ear_l").points);
    assert_eq!(part(&neutral, "fangs").color[3], 0);
    assert!(part(&angry, "fangs").color[3] > 0);
    // From behind the face hides but the ears and headband remain.
    let back = pose("neutral", FaceView::Back);
    assert_eq!(part(&back, "eye_l").color[3], 0);
    assert_eq!(part(&back, "whisker_l0").color[3], 0);
    assert!(part(&back, "ear_l").color[3] > 0);
    assert!(part(&back, "headband").color[3] > 0);
}

#[test]
fn albus_scenes_play_and_render() {
    let (script, art) = demo();
    let images = BTreeMap::new();
    let albus = |scene: &EvaluatedScene| {
        scene
            .actors
            .iter()
            .find(|a| a.id == "ALBUS")
            .unwrap()
            .clone()
    };
    // The foot goes up on the amp in the middle of that move.
    let rest = albus(&at(&script, 0, 0.0));
    let up = albus(&at(&script, 0, 12.0));
    let shoe_y = |a: &motion_scene::ActorPose| {
        let shoe = a.rigid_parts.iter().find(|p| p.id == "shoe_r").unwrap();
        shoe.points.iter().map(|p| p.y).sum::<f64>() / shoe.points.len() as f64
    };
    assert!(
        shoe_y(&up) < shoe_y(&rest) - 15.0,
        "foot raised onto the amp"
    );
    // While strumming, the right paw (screen left) stays over the guitar body
    // between keys; interpolated negative angles must not hit a joint limit.
    let centre = |a: &motion_scene::ActorPose, id: &str| {
        let p = a.rigid_parts.iter().find(|p| p.id == id).unwrap();
        let n = p.points.len() as f64;
        (
            p.points.iter().map(|q| q.x).sum::<f64>() / n,
            p.points.iter().map(|q| q.y).sum::<f64>() / n,
        )
    };
    for seconds in [1.0, 1.3, 2.2, 3.0] {
        let playing = albus(&at(&script, 0, seconds));
        let (px, py) = centre(&playing, "hand_l");
        let (gx, gy) = centre(&playing, "guitar_body");
        let reach = (px - gx).hypot(py - gy);
        let size = playing.scale.x.abs() * 30.0;
        assert!(
            reach < size,
            "{seconds}s: strumming paw {reach:.1} from the guitar"
        );
    }
    let moves = [
        ("soitto", 1.3),
        ("soolo", 5.6),
        ("headbang", 8.4),
        ("jalka", 12.0),
        ("hyppy", 14.3),
        ("vilkutus", 15.6),
        ("nuolaisu", 17.6),
        ("venyttely", 20.6),
        ("kumarrus", 23.0),
    ];
    for (name, seconds) in moves {
        let scene = at(&script, 0, seconds);
        let svg =
            motion_render::render_svg_with_art(&scene, 1920, 1080, &[], "liikkeet", &art, &images)
                .unwrap();
        assert!(svg.contains("<svg"));
        if let Some(dir) = out_dir() {
            let png = motion_export::rasterize_svg_png(&svg).unwrap();
            std::fs::write(dir.join(format!("liike-{name}.png")), png).unwrap();
        }
    }
    let faces = [
        ("neutraali", 1.5),
        ("iloinen", 3.0),
        ("surullinen", 5.0),
        ("yllattynyt", 7.0),
        ("vihainen", 9.0),
        ("puolisivu", 11.0),
        ("sivu", 13.0),
        ("takaa", 14.0),
    ];
    for (name, seconds) in faces {
        let scene = at(&script, 1, seconds);
        let svg =
            motion_render::render_svg_with_art(&scene, 1920, 1080, &[], "ilmeet", &art, &images)
                .unwrap();
        if let Some(dir) = out_dir() {
            let png = motion_export::rasterize_svg_png(&svg).unwrap();
            std::fs::write(dir.join(format!("ilme-{name}.png")), png).unwrap();
        }
    }
}
