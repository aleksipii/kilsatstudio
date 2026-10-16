//! Headless YAML/.kilsat PNG/WAV or MP4 project export from existing local files.
//! MP4 requires an explicit local encoder; no synthesis or downloads.
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
    if !(3..=5).contains(&args.len()) {
        return Err("usage: render_sequence SCRIPT_YAML_OR_KILSAT INPUT_JSON NEW_OUTPUT_DIRECTORY [png|mp4] [ABSOLUTE_FFMPEG]".into());
    }
    let input = Path::new(&args[0]);
    let (source, art) = if input.extension().is_some_and(|e| e == "kilsat") {
        let project = motion_project::load(input)?;
        let mut images = std::collections::BTreeMap::new();
        let mut encoded_bytes = 0usize;
        let mut decoded_pixels = 0u64;
        for layer in &project.art.layers {
            if let motion_art::Content::Image { asset_id, .. } = &layer.content {
                if !images.contains_key(asset_id) {
                    let asset = project
                        .assets
                        .iter()
                        .find(|a| &a.id == asset_id)
                        .ok_or("missing image asset")?;
                    let path = motion_project::resolve_asset(input, asset)?;
                    let image = motion_art::load_image(&path)?;
                    encoded_bytes += image.png().len();
                    decoded_pixels += u64::from(image.width()) * u64::from(image.height());
                    if encoded_bytes > 32 * 1024 * 1024 || decoded_pixels > 16_000_000 {
                        return Err(
                            "artwork exceeds aggregate 32 MiB / 16 million pixel image budget"
                                .into(),
                        );
                    }
                    images.insert(asset_id.clone(), image);
                }
            }
        }
        (
            project.source,
            motion_export::ArtContext {
                document: project.art,
                images,
            },
        )
    } else {
        (
            String::from_utf8(read(input, 1024 * 1024)?)?,
            motion_export::ArtContext::default(),
        )
    };
    let script = motion_script::parse(&source)?;
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
    let cancel = motion_tts::CancellationToken::new();
    let report = match args.get(3).map(String::as_str).unwrap_or("png") {
        "png" if args.len() <= 4 => motion_export::export_png_sequence(
            &script,
            &rendered,
            Some(&art),
            Path::new(&args[2]),
            &cancel,
            |_| {},
        ),
        "mp4" if args.len() == 5 => motion_export::export_mp4_sequence(
            &script,
            &rendered,
            Some(&art),
            Path::new(&args[2]),
            Path::new(&args[4]),
            &cancel,
            |_| {},
        ),
        _ => return Err("format must be png, or mp4 with explicit absolute FFmpeg".into()),
    }?;
    println!(
        "Exported {} PNG frames plus audio.wav and sequence.json",
        report.frame_count
    );
    Ok(())
}
