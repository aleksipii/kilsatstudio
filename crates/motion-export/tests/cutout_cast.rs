//! The original paper cut-out cast (examples/cutout-cast): the rigs validate and
//! give complete model sheets, and the demo street scene plays on twos with
//! snap turns through the shared scene and render pipeline.
//!
//! `CUTOUT_OUT=<dir>` also writes the model sheets and a few demo frames as PNG
//! for visual review.
use motion_art::style::CharacterStyle;
use motion_render::model_sheet::{
    render_model_sheet, rest_pose, silhouette_check, ModelSheetOptions,
};
use motion_rig::{FaceView, RigDefinition};
use motion_scene::{EvaluatedScene, ValidatedScript};
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

const CAST: [(&str, &str); 3] = [("ilona", "ILONA"), ("topi", "TOPI"), ("rauno", "RAUNO")];
const FPS: u64 = 24;

fn example(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/cutout-cast")
        .join(relative)
}

fn rig(file: &str, id: &str) -> RigDefinition {
    let text = std::fs::read_to_string(example(&format!("characters/{file}.kilsat"))).unwrap();
    let project: Value = serde_json::from_str(&text).unwrap();
    let script: Value = serde_json::from_str(project["source"].as_str().unwrap()).unwrap();
    serde_json::from_value(script["characters"][id]["rig"].clone()).unwrap()
}

fn demo() -> (ValidatedScript, motion_art::Document) {
    let project = motion_project::load(&example("cutout-cast.kilsat")).unwrap();
    let script = motion_script::parse(&project.source).expect("demo script compiles");
    (
        ValidatedScript::new(Arc::new(script)).expect("demo script validates"),
        project.art,
    )
}

fn frame(script: &ValidatedScript, index: u64) -> EvaluatedScene {
    let seconds = motion_scene::time::frame_to_seconds(index, FPS as u32).unwrap();
    script.evaluate(0, seconds).unwrap()
}

fn actor<'a>(scene: &'a EvaluatedScene, id: &str) -> &'a motion_scene::ActorPose {
    scene.actors.iter().find(|a| a.id == id).unwrap()
}

fn out_dir() -> Option<PathBuf> {
    std::env::var_os("CUTOUT_OUT").map(PathBuf::from)
}

#[test]
fn every_cutout_character_has_a_complete_model_sheet() {
    for (file, id) in CAST {
        let r = rig(file, id);
        r.prepare().unwrap_or_else(|e| panic!("{id}: {e}"));
        for style in [CharacterStyle::Default, CharacterStyle::Cutout] {
            let options = ModelSheetOptions {
                style,
                images: None,
            };
            let sheet = render_model_sheet(&r, &options).unwrap();
            assert_eq!(
                sheet,
                render_model_sheet(&r, &options).unwrap(),
                "{id} byte-stable"
            );
            assert_eq!(
                sheet.expressions,
                ["neutral", "happy", "sad", "surprised", "angry", "blink"],
                "{id}"
            );
            assert_eq!(sheet.mouths, motion_rig::VISEME_NAMES, "{id}");
            // Big heads on small bodies: about two heads tall, unlike the
            // 3-5 head stick figures.
            let p = sheet.proportions.expect("head joint found");
            assert!((1.5..2.6).contains(&p.heads), "{id} {p:?}");
            if let Some(dir) = out_dir() {
                let png = motion_export::rasterize_svg_png(&sheet.svg).unwrap();
                std::fs::write(dir.join(format!("{file}-{}.png", style.name())), png).unwrap();
            }
        }
        // The stubby limbs still read in the silhouette at rest.
        let (_, parts) = rest_pose(&r, CharacterStyle::Default).unwrap();
        let report = silhouette_check(&r, &parts);
        assert_eq!(report.limbs.len(), 4, "{id}");
        assert!(report.limbs.iter().all(|l| !l.warning), "{id} {report:?}");
    }
}

#[test]
fn cutout_characters_turn_by_swapping_drawings() {
    let r = rig("topi", "TOPI");
    let prepared = r.prepare().unwrap();
    let pose = |view| {
        prepared
            .evaluate_acting(
                &[],
                motion_rig::ActingInput {
                    emotion: "neutral",
                    blink: false,
                    view,
                },
            )
            .unwrap()
            .0
    };
    let brim_reach = |view| {
        let parts = pose(view);
        let brim = parts.iter().find(|p| p.id == "cap_brim").unwrap();
        (
            brim.color[3],
            brim.points.iter().map(|p| p.x).fold(f64::MIN, f64::max),
        )
    };
    let (front_alpha, front) = brim_reach(FaceView::Front);
    let (_, side) = brim_reach(FaceView::Side);
    assert!(
        front_alpha > 0 && side > front + 20.0,
        "the brim points forward in profile"
    );
    assert_eq!(brim_reach(FaceView::Back).0, 0, "no brim from behind");
}

#[test]
fn demo_plays_on_twos_with_snap_turns_and_renders() {
    let (script, art) = demo();
    // Actors hold every drawing for two frames (the camera stays on ones).
    for f in (1..48).step_by(2) {
        assert_eq!(
            frame(&script, f).actors,
            frame(&script, f - 1).actors,
            "frame {f}"
        );
    }
    // TOPI faces left at 1.0 s (frame 24): the profile drawing appears on that
    // very frame at full width, with no narrowing in-between.
    let before = frame(&script, 22);
    let at = frame(&script, 24);
    assert_eq!(actor(&before, "TOPI").view, FaceView::Front);
    let topi = actor(&at, "TOPI");
    assert_eq!(topi.view, FaceView::Side);
    assert_eq!(
        topi.scale.x.abs(),
        actor(&before, "TOPI").scale.x.abs(),
        "snap turns never narrow the figure"
    );
    assert!(topi.scale.x < 0.0, "facing left mirrors the figure");
    // RAUNO shows his back at 5 s; the face is hidden and the hair patch shows.
    let back = frame(&script, 5 * FPS);
    let rauno = actor(&back, "RAUNO");
    assert_eq!(rauno.view, FaceView::Back);
    let part = |id: &str| rauno.rigid_parts.iter().find(|p| p.id == id).unwrap();
    assert_eq!(part("eye_l").color[3], 0);
    assert_eq!(part("moustache").color[3], 0);
    assert!(part("hair_back_patch").color[3] > 0);

    // ILONA's wave swings the arm out beside her big head (joint limits stop
    // it going straight up behind it), and arms draw over the head, so the
    // raised mitten is above the chin and in front of the face.
    let waving = frame(&script, 44);
    let ilona = actor(&waving, "ILONA");
    let index = |id: &str| ilona.rigid_parts.iter().position(|p| p.id == id).unwrap();
    assert!(index("hand_r") > index("face"), "arms draw over the head");
    let mean_y = |id: &str| {
        let points = &ilona.rigid_parts[index(id)].points;
        points.iter().map(|p| p.y).sum::<f64>() / points.len() as f64
    };
    let chin = ilona.rigid_parts[index("face")]
        .points
        .iter()
        .map(|p| p.y)
        .fold(f64::MIN, f64::max);
    assert!(mean_y("hand_r") < chin, "the waving mitten is raised");

    let images = BTreeMap::new();
    for f in [0, 30, 44, 66, 92, 100, 106, 126, 140, 160] {
        let scene = frame(&script, f);
        let svg =
            motion_render::render_svg_with_art(&scene, 1920, 1080, &[], "katu", &art, &images)
                .unwrap();
        assert!(svg.contains("<svg"));
        if let Some(dir) = out_dir() {
            let png = motion_export::rasterize_svg_png(&svg).unwrap();
            std::fs::write(dir.join(format!("katu-{f:03}.png")), png).unwrap();
        }
    }
}
