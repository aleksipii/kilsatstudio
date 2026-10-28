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
    "8ba99dc39ed851d076f69370c83a35237b738aaa6a062ae1bba75366dcbd8f03";
/// Regression hashes for the G2 SVG export at 1920x1080 (`sequence_digest` over the
/// 96 per-frame SHA-256 values). SVG text is platform independent; CI checks it on
/// macOS and Ubuntu. This file is the only copy: the test, the `g2_export` command and the
/// `Export QA` workflow all read these values. A PR that changes the G2 scene
/// updates them here, and A10 reviews that change.
pub const G2_SVG_SEQUENCE_SHA256: &str =
    "1de95277c7718eb151cbf3ef57043814df2e44dd7f685aea0ba617838fe8e481";
/// SVG frames 24 (wide) and 72 (close-up), the A08 reference frames.
pub const G2_SVG_F024_SHA256: &str =
    "5f8f2ffc832fa87bca76c7009a432fa9a9770710ae463188cf65c89f47507c8f";
pub const G2_SVG_F072_SHA256: &str =
    "08394cfaf48232eafa2dc03b9828f4814302f278381c82e897892e77f1efe08e";
/// PNG frames from resvg/tiny-skia. Equal to the A08 export hashes in
/// `docs/evidence/g2-render/README.md`. Asserted on Apple Silicon macOS only:
/// floating-point rasterization is not claimed to be identical on other targets.
pub const G2_PNG_WIDE_SHA256: &str =
    "8bee3aa180d1041408a9e04e4d5fe2ef31215f2f47df040db8b7389a52edc658";
pub const G2_PNG_CLOSE_SHA256: &str =
    "85c0e33ec5cf848f0c21bcf17c245eb929da4623d776b7868eb575a6bece7f23";

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
            screen_videos: Default::default(),
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
