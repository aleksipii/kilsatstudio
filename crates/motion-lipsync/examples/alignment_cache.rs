//! cargo run -p motion-lipsync --example alignment_cache -- audio.wav timing.json cache-dir transcript
use motion_lipsync::{
    cache::{AlignmentCache, CacheKey},
    import_alignment_json, AlignmentAudio,
};
use sha2::{Digest, Sha256};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if !(args.len() == 5 || (args.len() == 6 && args[5] == "--manual")) {
        return Err("expected audio.wav timing.json cache-dir transcript [--manual]".into());
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
        &args[4],
        provider,
        version,
        if args.len() == 6 {
            "import-json-v1-manual"
        } else {
            "import-json-v1"
        },
        &identity,
    )?;
    println!("Exact decoded PCM SHA-256: {}", key.audio_sha256());
    let cache = AlignmentCache::new(&args[3]);
    let alignment = match cache.load(&key)? {
        Some(a) => {
            println!("Cache hit (structural validation only)");
            a
        }
        None => {
            let mut a = import_alignment_json(
                &document,
                &AlignmentAudio {
                    asset_id: key.audio_sha256().into(),
                    sample_rate: audio.sample_rate,
                    frames: audio.frames() as u64,
                },
            )?;
            if args.len() == 6 {
                a.source = motion_lipsync::AlignmentSource::Manual;
            }
            cache.store(&key, &a)?;
            println!("Imported and cached external timings");
            a
        }
    };
    println!(
        "{} phone intervals; provenance: {:?}",
        alignment.phonemes.len(),
        alignment.source
    );
    Ok(())
}
