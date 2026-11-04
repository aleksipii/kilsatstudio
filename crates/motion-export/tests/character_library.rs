//! Character library (examples/stick-studio/library/characters): every manifest
//! points at a project whose character parses, validates as a rig and gives a
//! complete, byte-stable model sheet; every expression changes the face and the
//! four views evaluate.
//!
//! `CHARACTER_LIBRARY_OUT=<dir>` also writes each model sheet as PNG for review.
use motion_art::style::CharacterStyle;
use motion_render::model_sheet::{render_model_sheet, ModelSheetOptions};
use motion_rig::FaceView;
use std::path::PathBuf;

fn pack() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/stick-studio")
}

fn manifests() -> Vec<serde_json::Value> {
    let mut paths: Vec<_> = std::fs::read_dir(pack().join("library/characters"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|p| serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap())
        .collect()
}

#[test]
fn every_library_character_has_a_working_rig() {
    let all = manifests();
    assert!(all.len() >= 40, "{} characters", all.len());
    let out = std::env::var_os("CHARACTER_LIBRARY_OUT").map(PathBuf::from);
    let options = ModelSheetOptions {
        style: CharacterStyle::Default,
        images: None,
    };
    for m in &all {
        let id = m["id"].as_str().unwrap();
        let path = pack().join(m["geometry"]["path"].as_str().unwrap());
        let character = m["character"]["id"].as_str().unwrap();
        // Check the rig on its own first: its errors name the failing part.
        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let source: serde_json::Value =
            serde_yaml::from_str(document["source"].as_str().unwrap()).unwrap();
        let raw: motion_rig::RigDefinition =
            serde_json::from_value(source["characters"][character]["rig"].clone())
                .unwrap_or_else(|e| panic!("{id}: {e}"));
        raw.prepare().unwrap_or_else(|e| panic!("{id}: {e}"));
        let project = motion_project::load(&path).unwrap_or_else(|e| panic!("{id}: {e}"));
        let script = motion_script::parse(&project.source).unwrap_or_else(|e| panic!("{id}: {e}"));
        motion_scene::ValidatedScript::new(std::sync::Arc::new(script.clone()))
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        let declaration = &script.characters[character];
        assert_eq!(
            declaration.acting.as_ref().map(|_| ()).is_some(),
            m["character"]["acting"].is_string(),
            "{id}"
        );
        let rig = declaration.rig.clone().expect("custom rig");
        let prepared = rig.prepare().unwrap_or_else(|e| panic!("{id}: {e}"));

        let sheet = render_model_sheet(&rig, &options).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(sheet, render_model_sheet(&rig, &options).unwrap(), "{id}");
        assert_eq!(
            sheet.expressions,
            ["neutral", "happy", "sad", "surprised", "angry", "blink"],
            "{id}"
        );
        assert_eq!(sheet.mouths, motion_rig::VISEME_NAMES, "{id}");

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
                .unwrap_or_else(|e| panic!("{id}: {e}"))
                .0
        };
        let neutral = pose("neutral", FaceView::Front);
        for emotion in ["happy", "sad", "surprised", "angry"] {
            assert_ne!(pose(emotion, FaceView::Front), neutral, "{id} {emotion}");
        }
        for view in [FaceView::ThreeQuarter, FaceView::Side, FaceView::Back] {
            assert_ne!(pose("neutral", view), neutral, "{id} {view:?}");
        }
        if let Some(dir) = &out {
            let png = motion_export::rasterize_svg_png(&sheet.svg).unwrap();
            std::fs::write(dir.join(format!("{id}.png")), png).unwrap();
        }
    }
}
