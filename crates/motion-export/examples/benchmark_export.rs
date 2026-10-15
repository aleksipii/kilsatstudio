//! Reproducible local export measurement, with synthetic silent audio (no TTS).
use std::{error::Error, path::Path, time::Instant};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || !matches!(args[0].as_str(), "svg" | "png") {
        return Err("usage: benchmark_export svg|png NEW_OUTPUT_DIRECTORY".into());
    }
    let script = motion_script::parse("project: {fps: 24, width: 640, height: 360}\ncharacters:\n  EMMA: {asset: characters/emma}\nscenes:\n  - id: benchmark\n    duration: 2\n    events: []\n")?;
    let rendered = motion_voice::export::RenderedScene {
        audio: motion_audio::AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: vec![0.; 48000],
        },
        manifest: motion_voice::export::SceneManifest {
            schema_version: 1,
            scene_id: "benchmark".into(),
            sample_rate: 24000,
            channels: 1,
            total_frames: 48000,
            dialogue: vec![],
        },
    };
    let start = Instant::now();
    let export = if args[0] == "png" {
        motion_export::export_png_scene
    } else {
        motion_export::export_scene
    };
    let report = export(
        &script,
        0,
        &rendered,
        Path::new(&args[1]),
        &motion_tts::CancellationToken::new(),
        |_| {},
    )?;
    println!(
        "{}",
        serde_json::json!({"format":args[0],"frames":report.frame_count,
        "width":640,"height":360,"pixels":report.frame_count*640*360,
        "elapsed_seconds":start.elapsed().as_secs_f64(),"os":std::env::consts::OS,
        "arch":std::env::consts::ARCH,"audio":"synthetic silence; no speech synthesis"})
    );
    Ok(())
}
