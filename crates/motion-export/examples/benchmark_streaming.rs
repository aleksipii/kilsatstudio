//! Time the bounded streaming MP4 export of a real `.kilsat` project.
//!
//! Every scene gets synthetic silent audio (no speech synthesis), so this
//! measures scene evaluation, artwork composition and encoding only. Usage:
//!
//! ```sh
//! cargo run --release -p motion-export --example benchmark_streaming -- \
//!     PROJECT.kilsat NEW_OUTPUT_DIRECTORY /opt/homebrew/bin/ffmpeg
//! ```
use std::{collections::BTreeMap, error::Error, path::Path, time::Instant};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err(
            "usage: benchmark_streaming PROJECT.kilsat NEW_OUTPUT_DIRECTORY ABSOLUTE_FFMPEG".into(),
        );
    }
    let input = Path::new(&args[0]);
    let load_started = Instant::now();
    let project = motion_project::load(input)?;
    let mut images = BTreeMap::new();
    for layer in &project.art.layers {
        for asset_id in layer.image_assets() {
            if images.contains_key(asset_id) {
                continue;
            }
            let asset = project
                .assets
                .iter()
                .find(|a| a.id == asset_id)
                .ok_or("missing image asset")?;
            let path = motion_project::resolve_asset(input, asset)?;
            images.insert(asset_id.to_string(), motion_art::load_image(&path)?);
        }
    }
    let art = motion_export::ArtContext {
        document: project.art,
        images,
        screen_videos: Default::default(),
    };
    let script = motion_script::parse(&project.source)?;
    let timeline = motion_scene::SequenceTimeline::compile(&script)?;
    let rendered: Vec<_> = script
        .scenes
        .iter()
        .zip(timeline.spans())
        .map(|(scene, span)| motion_voice::export::RenderedScene {
            audio: motion_audio::AudioBuffer {
                sample_rate: 24_000,
                channels: 1,
                samples: vec![0.; span.duration_samples as usize],
            },
            manifest: motion_voice::export::SceneManifest {
                schema_version: 1,
                scene_id: scene.id.clone(),
                sample_rate: 24_000,
                channels: 1,
                total_frames: span.duration_samples,
                dialogue: vec![],
                cues: vec![],
            },
        })
        .collect();
    let load_seconds = load_started.elapsed().as_secs_f64();
    let started = Instant::now();
    let report = motion_export::export_mp4_sequence_streaming(
        &script,
        &rendered,
        Some(&art),
        Path::new(&args[1]),
        Path::new(&args[2]),
        &motion_export::StreamingProfile::full_hd_24(),
        &motion_tts::CancellationToken::new(),
        |_| {},
        motion_export::CaptionMode::Follow,
    )?;
    let elapsed = started.elapsed().as_secs_f64();
    let video_seconds = report.frame_count as f64 / f64::from(report.fps);
    println!(
        "{}",
        serde_json::json!({
            "project": args[0],
            "scenes": script.scenes.len(),
            "frames": report.frame_count,
            "width": report.width,
            "height": report.height,
            "fps": report.fps,
            "video_seconds": video_seconds,
            "load_seconds": load_seconds,
            "export_seconds": elapsed,
            "frames_per_second": report.frame_count as f64 / elapsed,
            "realtime_factor": elapsed / video_seconds,
            "video_bytes": report.video_bytes,
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "audio": "synthetic silence; no speech synthesis",
        })
    );
    Ok(())
}
