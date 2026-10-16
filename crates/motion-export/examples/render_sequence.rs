//! Headless PNG/WAV project export from existing local files. No synthesis/downloads.
use serde::Deserialize;
use std::{
    error::Error,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    wav: PathBuf,
    manifest: PathBuf,
}
fn read(path: &Path, limit: u64) -> Result<Vec<u8>, Box<dyn Error>> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > limit {
        return Err("input exceeds byte budget".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("input exceeds byte budget".into());
    }
    Ok(bytes)
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: render_sequence SCRIPT_YAML INPUT_JSON NEW_OUTPUT_DIRECTORY (PNG + WAV only; no MP4/SVG sequence)".into());
    }
    let script =
        motion_script::parse(&String::from_utf8(read(Path::new(&args[0]), 1024 * 1024)?)?)?;
    let timeline = motion_scene::SequenceTimeline::compile(&script)?;
    if timeline.total_samples() > 24_000 * 600
        || timeline
            .spans()
            .iter()
            .map(|s| s.duration_samples)
            .sum::<u64>()
            > 24_000 * 1200
    {
        return Err("sequence exceeds ten minute output / twenty minute source limit".into());
    }
    let inputs: Vec<Input> = serde_json::from_slice(&read(Path::new(&args[1]), 1024 * 1024)?)?;
    if inputs.len() != script.scenes.len() {
        return Err("one {wav,manifest} input required per scene in script order".into());
    }
    let base = Path::new(&args[1]).parent().unwrap_or(Path::new("."));
    let mut manifest_bytes = 0u64;
    let mut aggregate = 0u64;
    for input in &inputs {
        manifest_bytes = manifest_bytes
            .checked_add(fs::metadata(base.join(&input.manifest))?.len())
            .ok_or("manifest byte overflow")?;
        if manifest_bytes > 8 * 1024 * 1024 {
            return Err("aggregate manifests exceed 8 MiB".into());
        }
        aggregate = aggregate
            .checked_add(fs::metadata(base.join(&input.wav))?.len())
            .ok_or("WAV byte overflow")?;
        if aggregate > 128 * 1024 * 1024 {
            return Err("aggregate WAV inputs exceed 128 MiB".into());
        }
    }
    let mut rendered = Vec::new();
    let mut remaining_manifests = 8 * 1024 * 1024;
    aggregate = 0;
    for input in inputs {
        let wav = base.join(input.wav);
        let manifest = base.join(input.manifest);
        aggregate = aggregate
            .checked_add(fs::metadata(&wav)?.len())
            .ok_or("input byte overflow")?;
        if aggregate > 128 * 1024 * 1024 {
            return Err("aggregate WAV input exceeds 128 MiB".into());
        }
        let manifest_data = read(&manifest, remaining_manifests)?;
        remaining_manifests -= manifest_data.len() as u64;
        rendered.push(motion_voice::export::RenderedScene {
            audio: motion_audio::read_wav(wav)?,
            manifest: serde_json::from_slice(&manifest_data)?,
        });
    }
    let report = motion_export::export_png_sequence(
        &script,
        &rendered,
        None,
        Path::new(&args[2]),
        &motion_tts::CancellationToken::new(),
        |_| {},
    )?;
    println!(
        "Exported {} PNG frames plus audio.wav and sequence.json",
        report.frame_count
    );
    Ok(())
}
