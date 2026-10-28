//! The G2 minimal scene (`examples/g2-minimal-scene/scene.kilsat`, A08) prepared for
//! export on the strict model-free path. Shared by `tests/g2_export.rs` and the
//! `g2_export` example.
//!
//! Audio comes from `motion_voice::model_free::prepare_scene_model_free`, which uses
//! `DisabledSpeechProvider` and accepts only imported WAVs. The scene has no
//! dialogue, so the result is 4 s of silence on the 24 kHz scene clock. Nothing here
//! starts a model, recognizer, Python process or download.
#![allow(dead_code)]
use motion_export::ArtContext;
use motion_project::{ModelBackedStep, ModelPolicy};
use motion_script::Script;
use motion_tts::CancellationToken;
use motion_voice::export::RenderedScene;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Path, PathBuf};

pub const SCENE_ID: &str = "g2_min";

/// SHA-256 of the `.kilsat` file the hashes below were recorded from.
pub const G2_SCENE_SHA256: &str =
    "bf58fc78afc7de8fc95d5ab040570b16009986338272ddfae7ed841f0982e175";
/// Regression hashes for the G2 SVG export at 1920x1080 (`sequence_digest` over the
/// 96 per-frame SHA-256 values). SVG text is platform independent; CI checks it on
/// macOS and Ubuntu. This file is the only copy: the test, the `g2_export` command and the
/// `Export QA` workflow all read these values. A PR that changes the G2 scene
/// updates them here, and A10 reviews that change.
pub const G2_SVG_SEQUENCE_SHA256: &str =
    "584232d5dc3afc85d30d2247b29587be2a554e38d3cccf62d6e999d055a67e91";
/// SVG frames 24 (wide) and 72 (close-up), the A08 reference frames.
pub const G2_SVG_F024_SHA256: &str =
    "e9d24dd4950a2537020c0975f83090e66ee6a517edd578dce310eed510404915";
pub const G2_SVG_F072_SHA256: &str =
    "5a531245112def209e3e3fad76379ce84702f03a6cc120d26844d6bc1db786a1";
/// PNG frames from resvg/tiny-skia. Equal to the A08 export hashes in
/// `docs/evidence/g2-render/README.md`. Asserted on Apple Silicon macOS only:
/// floating-point rasterization is not claimed to be identical on other targets.
pub const G2_PNG_WIDE_SHA256: &str =
    "85e13144c14393c48e5b78e4e7370d49aefbd66ebbb54bb060f51335814ed45c";
pub const G2_PNG_CLOSE_SHA256: &str =
    "d6d4c9def910e060b46edfd71e76ed6656cead1d4b35902acc3b9263e390dc1c";

pub fn scene_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/g2-minimal-scene/scene.kilsat")
}

pub struct G2 {
    pub script: Script,
    pub art: ArtContext,
    pub rendered: RenderedScene,
    /// Model-backed steps the strict policy refused before loading, read back from
    /// `ModelPolicy::permit`.
    pub refused_steps: Vec<String>,
    /// SHA-256 of the `.kilsat` file bytes that were loaded.
    pub source_sha256: String,
}

/// Load the scene and prepare its audio with `ModelPolicy::Strict`.
pub fn load(path: &Path) -> Result<G2, Box<dyn Error>> {
    let policy = ModelPolicy::Strict;
    // The strict policy must refuse every model-backed step before we start.
    let mut refused_steps = Vec::new();
    for step in [
        ModelBackedStep::SpeechSynthesis,
        ModelBackedStep::RecognizerAlignment,
        ModelBackedStep::ModelDownload,
    ] {
        if policy.permit(step).is_ok() {
            return Err(format!("strict policy permitted {step:?}").into());
        }
        refused_steps.push(format!("{step:?}"));
    }
    let bytes = std::fs::read(path)?;
    let project = motion_project::load(path)?;
    let script = motion_script::parse(&project.source)?;
    if !project
        .art
        .layers
        .iter()
        .all(|l| l.image_assets().next().is_none())
    {
        return Err("the G2 scene is expected to use vector layers only".into());
    }
    let root = path.parent().ok_or("scene path has no parent directory")?;
    let prepared = motion_voice::model_free::prepare_scene_model_free(
        &script,
        0,
        root,
        &BTreeMap::new(),
        &CancellationToken::new(),
        |_| {},
    )?;
    Ok(G2 {
        script,
        art: ArtContext {
            document: project.art,
            images: BTreeMap::new(),
        },
        rendered: prepared.rendered,
        refused_steps,
        source_sha256: sha256_hex(&bytes),
    })
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// One digest over all frames: SHA-256 of the newline-joined per-frame SHA-256 hex
/// strings, in frame order.
pub fn sequence_digest(frame_hashes: &[String]) -> String {
    let mut joined = frame_hashes.join("\n");
    joined.push('\n');
    sha256_hex(joined.as_bytes())
}
