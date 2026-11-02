//! Storyboard sheet (and optionally an animatic) of a `.kilsat` project.
//!
//! usage: storyboard PROJECT.kilsat NEW_OUTPUT_DIRECTORY [ABSOLUTE_FFMPEG]
//!
//! Speech is not synthesized here: the sequence is silent. The desktop app's
//! export uses the prepared speech and audio cues instead.
use motion_export::{ArtContext, CaptionMode, SequencePreview, StreamingProfile};
use motion_tts::CancellationToken;
use std::{collections::BTreeMap, error::Error, path::Path, sync::Arc};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(2..=3).contains(&args.len()) {
        return Err(
            "usage: storyboard PROJECT.kilsat NEW_OUTPUT_DIRECTORY [ABSOLUTE_FFMPEG]".into(),
        );
    }
    let project = Path::new(&args[0]);
    let document = motion_project::load(project)?;
    let script = motion_script::parse(&document.source)?;
    let mut art = ArtContext {
        document: document.art.clone(),
        images: BTreeMap::new(),
        screen_videos: Default::default(),
    };
    for asset in &document.assets {
        if asset.kind == motion_project::AssetKind::Image {
            let path = motion_project::resolve_asset(project, asset)?;
            art.images
                .insert(asset.id.clone(), motion_art::load_image(&path)?);
        }
    }
    let rendered = script
        .scenes
        .iter()
        .map(|s| motion_voice::export::render_scene(&s.id, s.duration, &[]))
        .collect::<Result<Vec<_>, _>>()?;
    let cancel = CancellationToken::new();
    let script = Arc::new(script);
    let rendered = Arc::new(rendered);
    let preview = SequencePreview::new(
        Arc::clone(&script),
        Arc::clone(&rendered),
        Some(Arc::new(art.clone())),
        &cancel,
    )?;
    let output = Path::new(&args[1]);
    let report = motion_export::export_storyboard(
        &preview,
        &output.join("storyboard"),
        &script.project.title,
        &document.production,
        640,
        CaptionMode::Follow,
        &cancel,
        |_| {},
    )?;
    println!(
        "{} panels in {}",
        report.shots,
        report.output_directory.display()
    );
    if let Some(ffmpeg) = args.get(2) {
        let report = motion_export::export_animatic_streaming(
            &script,
            &rendered,
            Some(&art),
            &output.join("animatic"),
            Path::new(ffmpeg),
            &StreamingProfile::animatic(&script),
            &cancel,
            |_| {},
            CaptionMode::Follow,
        )?;
        println!(
            "animatic: {} frames, {}x{}, {:.1} s to encode",
            report.frame_count, report.width, report.height, report.elapsed_seconds
        );
    }
    Ok(())
}
