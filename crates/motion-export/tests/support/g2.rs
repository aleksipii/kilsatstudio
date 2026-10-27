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

pub fn scene_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/g2-minimal-scene/scene.kilsat")
}

pub struct G2 {
    pub script: Script,
    pub art: ArtContext,
    pub rendered: RenderedScene,
    /// SHA-256 of the `.kilsat` file bytes that were loaded.
    pub source_sha256: String,
}

/// Load the scene and prepare its audio with `ModelPolicy::Strict`.
pub fn load(path: &Path) -> Result<G2, Box<dyn Error>> {
    let policy = ModelPolicy::Strict;
    // The strict policy must refuse every model-backed step before we start.
    for step in [
        ModelBackedStep::SpeechSynthesis,
        ModelBackedStep::RecognizerAlignment,
        ModelBackedStep::ModelDownload,
    ] {
        if policy.permit(step).is_ok() {
            return Err(format!("strict policy permitted {step:?}").into());
        }
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
