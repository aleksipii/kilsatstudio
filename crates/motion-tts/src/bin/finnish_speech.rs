//! Explicit local-inference probe. No network or automatic installation.
use motion_tts::{PiperProvider, SpeechProvider, SpeechRequest, VoiceSettings};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

fn bounded_file(path: &Path, limit: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > limit {
        return Err("input must be a bounded regular non-symlink file".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("input exceeds limit".into());
    }
    Ok(bytes)
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Finnish speech probe: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let mut config: Option<PathBuf> = None;
    let mut text: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut speed = 1.0f32;
    let mut allowed = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--piper-config" => {
                config = Some(args.next().ok_or("missing Piper config path")?.into())
            }
            "--text-file" => text = Some(args.next().ok_or("missing UTF-8 text path")?.into()),
            "--report" => output = Some(args.next().ok_or("missing new report path")?.into()),
            "--speed" => speed = args.next().ok_or("missing speed")?.parse()?,
            "--allow-local-inference" => allowed = true,
            "--help" | "-h" => {
                println!("finnish_speech --piper-config JSON --text-file UTF8 --report NEW_JSON --allow-local-inference [--speed 0.5..2]\nRuns configured local Piper only; no downloads, no production/rights acceptance.");
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg}").into()),
        }
    }
    if !allowed {
        return Err("explicit --allow-local-inference is required".into());
    }
    let output = output.ok_or("--report is required")?;
    if output.exists() {
        return Err("report must be a new file; existing evidence is never overwritten".into());
    }
    let text_bytes = bounded_file(&text.ok_or("--text-file is required")?, 16_000)?;
    let text = String::from_utf8(text_bytes.clone())?;
    let provider = PiperProvider::from_config_file(&config.ok_or("--piper-config is required")?)?;
    let provenance = provider.provenance()?;
    let result = provider.synthesize(SpeechRequest {
        text,
        voice: VoiceSettings {
            preset: provider.config.preset.clone(),
            language: "fi-fi".into(),
            speed,
            style: None,
        },
    })?;
    let wav = bounded_file(&result.audio_path, 32 * 1024 * 1024)?;
    let mut reader = hound::WavReader::new(std::io::Cursor::new(&wav))?;
    let mut peak = 0.0f64;
    let mut squares = 0.0f64;
    for sample in reader.samples::<i16>() {
        let value = f64::from(sample?) / 32768.0;
        peak = peak.max(value.abs());
        squares += value * value;
    }
    let elapsed = started.elapsed().as_secs_f64();
    let duration = result.duration_seconds();
    let report = serde_json::json!({
        "schema_version": 1, "provider": "piper", "language": "fi-fi", "voice_preset": provider.config.preset,
        "audio_path": result.audio_path, "output_sha256": format!("{:x}",Sha256::digest(&wav)),
        "transcript_sha256": format!("{:x}",Sha256::digest(&text_bytes)),
        "audio": { "sample_rate": result.sample_rate, "channels":1, "bits_per_sample":16,
            "sample_count": result.sample_count, "duration_seconds":duration, "nonzero_signal":peak>0.0,
            "peak":peak, "rms":(squares/result.sample_count as f64).sqrt() },
        "elapsed_seconds":elapsed, "real_time_factor":elapsed/duration, "cached":false,
        "provenance": {"kind":"recorded_local_model", "provider":"piper", "assets":provenance},
        "production_acceptance":"not_granted", "perceptual_quality":"unmeasured", "rights_review":"not_granted"
    });
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    file.write_all(serde_json::to_string_pretty(&report)?.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}
