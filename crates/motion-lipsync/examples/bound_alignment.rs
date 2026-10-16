//! cargo run -p motion-lipsync --example bound_alignment -- audio.wav measured-import.json 'current transcript' bound.json
//! Reads existing measured phone intervals; never runs or downloads an aligner.
use motion_lipsync::{
    cache::{BoundAlignment, CacheKey},
    import_alignment_json, AlignmentAudio,
};
use sha2::{Digest, Sha256};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 5 {
        return Err(
            "expected audio.wav measured-import.json 'current transcript' bound.json".into(),
        );
    }
    let audio = motion_audio::read_wav(&args[1])?;
    let document = std::fs::read_to_string(&args[2])?;
    let metadata: serde_json::Value = serde_json::from_str(&document)?;
    let provider = metadata["tool"].as_str().ok_or("missing tool")?;
    let version = metadata["tool_version"]
        .as_str()
        .ok_or("missing tool_version")?;
    let identity = format!("{:x}", Sha256::digest(document.as_bytes()));
    let key = CacheKey::new(
        &audio,
        &args[3],
        provider,
        version,
        "measured-import-json-v1",
        &identity,
    )?;
    let alignment = import_alignment_json(
        &document,
        &AlignmentAudio {
            asset_id: key.audio_sha256().into(),
            sample_rate: audio.sample_rate,
            frames: audio.frames() as u64,
        },
    )?;
    let envelope = BoundAlignment::new(key, Some(alignment))?;
    // Refuse accidental overwrite of a measured import or existing output.
    use std::io::Write;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[4])?;
    output.write_all(&serde_json::to_vec_pretty(&envelope)?)?;
    output.sync_all()?;
    eprintln!("Bound existing measured timing to exact PCM and transcript; perceptual review remains required.");
    Ok(())
}
