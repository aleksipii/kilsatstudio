//! Export an existing WAV/scene manifest with poses; never downloads or runs TTS.
use std::{error::Error, fs, io::Read, path::Path};
fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, Box<dyn Error>> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > limit {
        return Err("input file exceeds byte budget".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("input file exceeds byte budget".into());
    }
    Ok(bytes)
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(5..=7).contains(&args.len()) {
        return Err(
            "usage: render_scene SCRIPT SCENE_INDEX WAV SCENE_MANIFEST NEW_OUTPUT_DIRECTORY [svg|png|mp4] [ABSOLUTE_FFMPEG]".into(),
        );
    }
    let input = Path::new(&args[0]);
    let (source, art) = if input.extension().is_some_and(|e| e == "kilsat") {
        let project = motion_project::load(input)?;
        let mut images = std::collections::BTreeMap::new();
        let mut encoded_bytes = 0usize;
        let mut decoded_pixels = 0u64;
        for layer in &project.art.layers {
            for asset_id in layer.image_assets() {
                if !images.contains_key(asset_id) {
                    let asset = project
                        .assets
                        .iter()
                        .find(|a| a.id == asset_id)
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
                    images.insert(asset_id.to_string(), image);
                }
            }
        }
        (
            project.source,
            motion_export::ArtContext {
                document: project.art,
                images,
                screen_videos: Default::default(),
            },
        )
    } else {
        (
            String::from_utf8(read_bounded(input, 1024 * 1024)?)?,
            motion_export::ArtContext::default(),
        )
    };
    let script = motion_script::parse(&source)?;
    let manifest: motion_voice::export::SceneManifest =
        serde_json::from_slice(&read_bounded(Path::new(&args[3]), 8 * 1024 * 1024)?)?;
    // Bound WAV bytes before the PCM reader allocates. This check does not prove
    // that supplied waveform content corresponds to the supplied alignment.
    if fs::metadata(&args[2])?.len() > 128 * 1024 * 1024 {
        return Err("WAV exceeds 128 MiB input budget".into());
    }
    let audio = motion_audio::read_wav(&args[2])?;
    let rendered = motion_voice::export::RenderedScene { audio, manifest };
    let index = args[1].parse()?;
    let destination = Path::new(&args[4]);
    let cancel = motion_tts::CancellationToken::new();
    let report = match args.get(5).map(String::as_str).unwrap_or("svg") {
        "svg" if args.len() <= 6 => motion_export::export_scene_with_art(
            &script,
            index,
            &rendered,
            &art,
            destination,
            &cancel,
            |_| {},
        ),
        "png" if args.len() <= 6 => motion_export::export_png_scene_with_art(
            &script,
            index,
            &rendered,
            &art,
            destination,
            &cancel,
            |_| {},
        ),
        "mp4" if args.len() == 7 => motion_export::export_mp4_scene_with_art(
            &script,
            index,
            &rendered,
            &art,
            destination,
            Path::new(&args[6]),
            &cancel,
            |_| {},
        ),
        _ => {
            return Err(
                "format must be svg/png, or mp4 with an explicit absolute FFmpeg path".into(),
            )
        }
    }?;
    println!("Exported {} frames, scene.wav, scene.json and manifest.json to {}. Supplied WAV/alignment pairing is not certified.", report.frame_count, report.output_directory.display());
    Ok(())
}
