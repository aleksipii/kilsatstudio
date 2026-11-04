//! P08 model sheet, silhouette and style preset tests against the shipped stick
//! characters (docs/model-sheet.md).
use motion_art::style::CharacterStyle;
use motion_render::model_sheet::{
    apply_character_style, render_model_sheet, rest_pose, silhouette_check, ModelSheetOptions,
};
use motion_rig::{ActingInput, FaceView, RigDefinition, Transform2, Vec2};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;

const CHARACTERS: [(&str, &str); 6] = [
    ("noutaja", "NOUTAJA"),
    ("veikko", "VEIKKO"),
    ("pasma", "PASMA"),
    ("kipina", "KIPINA"),
    ("kiire", "KIIRE"),
    ("sisu", "SISU"),
];

fn rig(file: &str, id: &str) -> RigDefinition {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../examples/stick-studio/characters/{file}.kilsat"
    ));
    let project: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let script: Value = serde_yaml::from_str(project["source"].as_str().unwrap()).unwrap();
    serde_json::from_value(script["characters"][id]["rig"].clone()).unwrap()
}

fn sha256(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn sheet(rig: &RigDefinition, style: CharacterStyle) -> motion_render::model_sheet::ModelSheet {
    render_model_sheet(
        rig,
        &ModelSheetOptions {
            style,
            images: None,
        },
    )
    .unwrap_or_else(|e| panic!("{} {}: {e}", rig.id, style.name()))
}

fn posed(r: &RigDefinition, offsets: &[(&str, f64)]) -> Vec<motion_rig::EvaluatedPart> {
    let prepared = r.prepare().unwrap();
    let offsets: Vec<_> = offsets
        .iter()
        .map(|(joint, rotation)| {
            (
                prepared.joint_index(joint).unwrap(),
                Transform2 {
                    translation: Vec2::new(0.0, 0.0),
                    rotation: *rotation,
                    scale: Vec2::new(1.0, 1.0),
                },
            )
        })
        .collect();
    prepared
        .evaluate_acting(
            &offsets,
            ActingInput {
                emotion: "neutral",
                blink: false,
                view: FaceView::Front,
                hands: Default::default(),
            },
        )
        .unwrap()
        .0
}

/// SHA-256 of Pasma's sheet in every style. These hashes are the reference
/// images: any change to sheet layout, rig evaluation or a preset changes them.
/// Regenerate with `MODEL_SHEET_PRINT=1 cargo test -p motion-render --test model_sheet`
/// after checking the rendered SVGs (`MODEL_SHEET_OUT=<dir>` writes them).
const PASMA_SHEETS: [(&str, &str); 6] = [
    (
        "default",
        "8e4e44e70d7c7133a5201fe1b9e8eba906c901eda4f91f67b63d83c7a5ff38da",
    ),
    (
        "stick",
        "c8d59c92af52ec23e1ef2893fb136d2b9eb117882993a0449dac3a46a286c149",
    ),
    (
        "rubber_hose",
        "4ae07b05528945ab4f376c2de60dc122874293159e517879c1580c87f7449da0",
    ),
    (
        "flat_tv",
        "bdf9f54af30b512f95e26d8be9b8e22c8215b974f662d628c9a8b76cbaf4f390",
    ),
    (
        "geometric",
        "fdaefa60758f5eeea8b6e0454e2acf92b9cdc4884b7e183e0cfb2230b73c96c7",
    ),
    (
        "cutout",
        "2e6221f32b7353412bb82804ff34adb3a938b156881fbf25b683cf6e1ffb4133",
    ),
];

#[test]
fn every_character_renders_in_every_style_deterministically() {
    let out = std::env::var("MODEL_SHEET_OUT").ok();
    for (file, id) in CHARACTERS {
        let r = rig(file, id);
        for style in CharacterStyle::ALL {
            let a = sheet(&r, style);
            let b = sheet(&r, style);
            assert_eq!(a, b, "{id} {} is not byte-stable", style.name());
            assert!(a.svg.starts_with("<svg ") && a.svg.ends_with("</svg>\n"));
            // Six expressions (five emotions and the blink) and all 13 visemes.
            assert_eq!(
                a.expressions,
                ["neutral", "happy", "sad", "surprised", "angry", "blink"],
                "{id}"
            );
            assert_eq!(a.mouths, motion_rig::VISEME_NAMES, "{id}");
            let p = a.proportions.expect("head joint found");
            assert!((3.0..5.0).contains(&p.heads), "{id} {p:?}");
            assert!(!a.palette.is_empty() && a.palette.iter().all(|c| c[3] > 0));
            assert_eq!(a.silhouette.limbs.len(), 4, "{id}");
            if let Some(dir) = &out {
                std::fs::write(format!("{dir}/{file}-{}.svg", style.name()), &a.svg).unwrap();
            }
        }
    }
}

#[test]
fn pasma_sheets_match_reference_hashes() {
    let r = rig("pasma", "PASMA");
    let print = std::env::var("MODEL_SHEET_PRINT").is_ok();
    for (name, expected) in PASMA_SHEETS {
        let style = CharacterStyle::from_name(name).unwrap();
        let hash = sha256(&sheet(&r, style).svg);
        if print {
            println!("(\"{name}\", \"{hash}\"),");
        } else {
            assert_eq!(hash, expected, "{name} model sheet changed");
        }
    }
}

#[test]
fn default_style_leaves_rigs_unchanged_and_presets_stay_valid() {
    for (file, id) in CHARACTERS {
        let r = rig(file, id);
        assert_eq!(apply_character_style(&r, CharacterStyle::Default), r);
        for style in &CharacterStyle::ALL[1..] {
            let styled = apply_character_style(&r, *style);
            styled
                .prepare()
                .unwrap_or_else(|e| panic!("{id} {}: {e}", style.name()));
            assert_ne!(styled, r, "{id} {} changed nothing", style.name());
            // Structure is kept: the styled rig animates like the original.
            assert_eq!(styled.joints, r.joints);
            assert_eq!(
                styled.parts.iter().map(|p| &p.id).collect::<Vec<_>>(),
                r.parts.iter().map(|p| &p.id).collect::<Vec<_>>()
            );
            assert_eq!(styled.visemes, r.visemes);
            assert_eq!(styled.expressions, r.expressions);
        }
    }
}

#[test]
fn preset_settings_reach_the_rig() {
    let r = rig("pasma", "PASMA");
    let width = |rig: &RigDefinition, id: &str| {
        let part = rig.parts.iter().find(|p| p.id == id).unwrap();
        let xs = part.points.iter().map(|p| p[0]);
        xs.clone().fold(f64::MIN, f64::max) - xs.fold(f64::MAX, f64::min)
    };
    // Rubber hose: thicker limbs, a silhouette outline on every part.
    let hose = apply_character_style(&r, CharacterStyle::RubberHose);
    assert!(width(&hose, "shin_l") > 1.4 * width(&r, "shin_l"));
    assert!(hose.parts.iter().all(|p| p
        .outline
        .is_some_and(|o| o.mode == motion_rig::OutlineMode::Silhouette)));
    // Rounding is capped by the 64-vertex limit of a rounded polygon.
    let shin = hose.parts.iter().find(|p| p.id == "shin_l").unwrap();
    assert_eq!(shin.smooth, 4);
    // Stick and geometric: outlines off; geometric keeps sharp corners.
    for style in [CharacterStyle::Stick, CharacterStyle::Geometric] {
        let s = apply_character_style(&r, style);
        assert!(s.parts.iter().all(|p| p.outline.is_none()));
    }
    // Cutout: every piece keeps its own heavy outline (behind mode), so
    // overlapping pieces read as layered paper; nothing squashes.
    let cutout = apply_character_style(&r, CharacterStyle::Cutout);
    assert!(cutout.parts.iter().all(|p| p
        .outline
        .is_some_and(|o| o.mode == motion_rig::OutlineMode::Behind && o.width == 3.5)));
    assert_eq!(CharacterStyle::Cutout.preset().squash_stretch, 0.0);
    let geo = apply_character_style(&r, CharacterStyle::Geometric);
    assert!(geo
        .parts
        .iter()
        .filter(|p| !p.points.is_empty())
        .all(|p| p.smooth == 0));
    // The palette is remapped: limb ink becomes the preset's ink.
    let ink = CharacterStyle::Geometric.preset().palette.unwrap().ink;
    assert_eq!(
        geo.parts.iter().find(|p| p.id == "thigh_l").unwrap().color,
        [ink.r, ink.g, ink.b, 255]
    );
}

#[test]
fn silhouette_warns_when_an_arm_folds_onto_the_body() {
    for (file, id) in CHARACTERS {
        let r = rig(file, id);
        let (_, parts) = rest_pose(&r, CharacterStyle::Default).unwrap();
        let report = silhouette_check(&r, &parts);
        assert_eq!(report.warnings().count(), 0, "{id} rest pose: {report:?}");
    }
    let r = rig("pasma", "PASMA");
    let folded = posed(&r, &[("upper_arm_r", 0.5), ("forearm_r", 2.5)]);
    let report = silhouette_check(&r, &folded);
    let warned: Vec<_> = report.warnings().map(|l| l.limb).collect();
    assert_eq!(warned, ["arm_r"], "{report:?}");
    let arm = &report.limbs[1];
    assert!(
        arm.visible_fraction < 0.5 && arm.contour_gain < 0.2,
        "{arm:?}"
    );
    // The check is deterministic.
    assert_eq!(silhouette_check(&r, &folded), report);
}

#[test]
fn silhouette_of_a_rig_without_stick_limbs_is_empty() {
    let r: RigDefinition = serde_json::from_value(serde_json::json!({
        "version": 1, "id": "blob",
        "joints": [{"id": "root", "parent": null}],
        "parts": [{"id": "body", "joint": "root", "points": [[0, 0], [10, 0], [5, 10]], "color": [0, 0, 0, 255]}]
    }))
    .unwrap();
    let (_, parts) = rest_pose(&r, CharacterStyle::Default).unwrap();
    assert!(silhouette_check(&r, &parts).limbs.is_empty());
    // Without a head joint the sheet still renders, with no ruler or face rows.
    let s = sheet(&r, CharacterStyle::FlatTv);
    assert!(s.proportions.is_none() && s.expressions.is_empty() && s.mouths.is_empty());
}

fn in_view(r: &RigDefinition, view: FaceView) -> Vec<motion_rig::EvaluatedPart> {
    let prepared = r.prepare().unwrap();
    prepared
        .evaluate_acting(
            &[],
            ActingInput {
                emotion: "neutral",
                blink: false,
                view,
                hands: Default::default(),
            },
        )
        .unwrap()
        .0
}

fn centre_x(parts: &[motion_rig::EvaluatedPart], id: &str) -> (f64, u8) {
    let part = parts.iter().find(|p| p.id == id).unwrap();
    let x = part.points.iter().map(|p| p.x).sum::<f64>() / part.points.len() as f64;
    (x, part.color[3])
}

#[test]
fn face_accessories_follow_the_eyes_and_hide_from_behind() {
    for (file, id, accessory, eye) in [
        ("sisu", "SISU", "glasses_l", "pupil_l"),
        ("sisu", "SISU", "glasses_r", "pupil_r"),
        ("kipina", "KIPINA", "goggle_l", "pupil_l"),
    ] {
        let r = rig(file, id);
        for view in [FaceView::Front, FaceView::ThreeQuarter, FaceView::Side] {
            let parts = in_view(&r, view);
            let (lens, alpha) = centre_x(&parts, accessory);
            let (pupil, _) = centre_x(&parts, eye);
            assert!(alpha > 0, "{id} {accessory} visible in {view:?}");
            // The lens stays over its eye as the head turns.
            assert!(
                (lens - pupil).abs() < 2.0,
                "{id} {accessory} {view:?}: {lens} vs {pupil}"
            );
        }
        let back = in_view(&r, FaceView::Back);
        assert_eq!(
            centre_x(&back, accessory).1,
            0,
            "{id} {accessory} hidden from behind"
        );
    }
    // Front-only clothing disappears from behind; back drawings replace it.
    let hidden = [
        ("pasma", "PASMA", "tie"),
        ("kiire", "KIIRE", "vest_zip"),
        ("veikko", "VEIKKO", "cap_brim"),
        ("noutaja", "NOUTAJA", "face"),
    ];
    for (file, id, part) in hidden {
        let back = in_view(&rig(file, id), FaceView::Back);
        assert_eq!(centre_x(&back, part).1, 0, "{id} {part}");
    }
    // Sisu's bun sits behind the face as she turns toward +x.
    let sisu = rig("sisu", "SISU");
    let bun = |view| centre_x(&in_view(&sisu, view), "bun").0;
    assert!(bun(FaceView::Front) > 0.0);
    assert!(bun(FaceView::ThreeQuarter) < 0.0 && bun(FaceView::Side) < bun(FaceView::ThreeQuarter));
}
