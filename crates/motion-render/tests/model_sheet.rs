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
    let script: Value = serde_json::from_str(project["source"].as_str().unwrap()).unwrap();
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
            },
        )
        .unwrap()
        .0
}

/// SHA-256 of Pasma's sheet in every style. These hashes are the reference
/// images: any change to sheet layout, rig evaluation or a preset changes them.
/// Regenerate with `MODEL_SHEET_PRINT=1 cargo test -p motion-render --test model_sheet`
/// after checking the rendered SVGs (`MODEL_SHEET_OUT=<dir>` writes them).
const PASMA_SHEETS: [(&str, &str); 5] = [
    (
        "default",
        "29be6b9cf4bd4c0479fcd3536eaee855d5c2e8d4b576abd60723a2b6e018e45c",
    ),
    (
        "stick",
        "86bab2b286138a6b39dbc98037dbb2716bbe83fe65fba23da0e8cf92a983a0af",
    ),
    (
        "rubber_hose",
        "c91e9eced4e9a78d51bada9b8c9053c6079ccfbfc0a43b1b4cea80bab6d5870e",
    ),
    (
        "flat_tv",
        "e5b1a693a6c4335319ea2c08e42159b564817470c0063813286ebba149248b09",
    ),
    (
        "geometric",
        "7489b25cd3e7a377f83b4dc48c377a354596040b0c755f04a45f3f209cbc5a10",
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
