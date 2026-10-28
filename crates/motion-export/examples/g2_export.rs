//! Reproducible export of the G2 minimal scene on the strict model-free path.
//!
//! ```sh
//! cargo run --locked --release -p motion-export --example g2_export -- OUT_DIR png
//! cargo run --locked --release -p motion-export --example g2_export -- OUT_DIR mp4 --ffmpeg /ABS/ffmpeg
//! cargo run --locked --release -p motion-export --example g2_export -- OUT_DIR svg --runs 7
//! ```
//!
//! `OUT_DIR` must not exist. The export uses the same frame clock, evaluator and
//! renderer as the desktop preview. Audio is prepared with `ModelPolicy::Strict`
//! (no synthesis, recognizer, Python or downloads); the scene is silent.
//!
//! Prints one JSON summary on stdout: frame count, audio length, per-frame SHA-256
//! digest and wall-clock timings. `--runs N` first exports N extra times into
//! temporary sibling directories (removed afterwards) and reports their timings;
//! one discarded warm-up run precedes them. Timings describe only this machine.
#[path = "../tests/support/g2.rs"]
mod g2;

use motion_tts::CancellationToken;
use std::{
    error::Error,
    path::{Path, PathBuf},
    time::Instant,
};

const USAGE: &str = "usage: g2_export NEW_OUTPUT_DIR [svg|png|mp4] [--ffmpeg ABSOLUTE_PATH] [--runs N] [--scene PATH]";

struct Args {
    out: PathBuf,
    format: String,
    ffmpeg: Option<PathBuf>,
    runs: usize,
    scene: PathBuf,
}

fn parse_args() -> Result<Args, Box<dyn Error>> {
    let mut raw = std::env::args().skip(1);
    let out = PathBuf::from(raw.next().ok_or(USAGE)?);
    let mut args = Args {
        out,
        format: "svg".into(),
        ffmpeg: None,
        runs: 0,
        scene: g2::scene_path(),
    };
    while let Some(arg) = raw.next() {
        match arg.as_str() {
            "svg" | "png" | "mp4" => args.format = arg,
            "--ffmpeg" => args.ffmpeg = Some(PathBuf::from(raw.next().ok_or(USAGE)?)),
            "--runs" => {
                args.runs = raw.next().ok_or(USAGE)?.parse()?;
                if args.runs > 100 {
                    return Err("--runs is limited to 100".into());
                }
            }
            "--scene" => args.scene = PathBuf::from(raw.next().ok_or(USAGE)?),
            _ => return Err(USAGE.into()),
        }
    }
    if (args.format == "mp4") != args.ffmpeg.is_some() {
        return Err("mp4 requires --ffmpeg ABSOLUTE_PATH; svg/png do not take it".into());
    }
    Ok(args)
}

fn export(
    scene: &g2::G2,
    format: &str,
    ffmpeg: Option<&Path>,
    out: &Path,
) -> Result<motion_export::Report, motion_export::Error> {
    let cancel = CancellationToken::new();
    match format {
        "svg" => motion_export::export_scene_with_art(
            &scene.script,
            0,
            &scene.rendered,
            &scene.art,
            out,
            &cancel,
            |_| {},
        ),
        "png" => motion_export::export_png_scene_with_art(
            &scene.script,
            0,
            &scene.rendered,
            &scene.art,
            out,
            &cancel,
            |_| {},
        ),
        _ => motion_export::export_mp4_scene_with_art(
            &scene.script,
            0,
            &scene.rendered,
            &scene.art,
            out,
            ffmpeg.expect("checked in parse_args"),
            &cancel,
            |_| {},
        ),
    }
}

fn timed(
    scene: &g2::G2,
    args: &Args,
    out: &Path,
) -> Result<(motion_export::Report, f64), Box<dyn Error>> {
    let start = Instant::now();
    let report = export(scene, &args.format, args.ffmpeg.as_deref(), out)?;
    Ok((report, start.elapsed().as_secs_f64() * 1000.0))
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = parse_args()?;
    if args.out.exists() {
        return Err(format!("{} already exists", args.out.display()).into());
    }
    let load_start = Instant::now();
    let scene = g2::load(&args.scene)?;
    let load_ms = load_start.elapsed().as_secs_f64() * 1000.0;

    // Optional measured runs into temporary siblings; one warm-up run is discarded.
    let mut run_ms = Vec::new();
    let total_runs = if args.runs == 0 { 0 } else { args.runs + 1 };
    for run in 0..total_runs {
        let mut dir = args.out.clone().into_os_string();
        dir.push(format!(".bench-{run}"));
        let dir = PathBuf::from(dir);
        let (_, ms) = timed(&scene, &args, &dir)?;
        std::fs::remove_dir_all(&dir)?;
        if run > 0 {
            run_ms.push(ms);
        }
    }
    let (report, export_ms) = timed(&scene, &args, &args.out)?;

    let project = &scene.script.project;
    let duration = scene.script.scenes[0].duration;
    let expected_frames =
        motion_scene::frame_count(duration, project.fps, motion_export::MAX_FRAMES)?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(args.out.join("manifest.json"))?)?;
    let mut frame_hashes = Vec::new();
    for frame in manifest["frames"]
        .as_array()
        .ok_or("manifest has no frames")?
    {
        let file = frame["file"].as_str().ok_or("frame without file")?;
        frame_hashes.push(g2::sha256_hex(&std::fs::read(args.out.join(file))?));
    }
    let wav = motion_audio::read_wav(args.out.join("scene.wav"))?;
    let audio_samples = wav.frames() as u64;
    run_ms.sort_by(f64::total_cmp);
    let median = |v: &[f64]| match v.len() {
        0 => None,
        n if n % 2 == 1 => Some(v[n / 2]),
        n => Some((v[n / 2 - 1] + v[n / 2]) / 2.0),
    };
    let sequence_sha256 = g2::sequence_digest(&frame_hashes);
    // The SVG reference applies only to the recorded G2 scene; PNG bytes are not
    // claimed across CPUs, so they have no reference here.
    let reference = (args.format == "svg" && scene.source_sha256 == g2::G2_SCENE_SHA256)
        .then_some(g2::G2_SVG_SEQUENCE_SHA256);
    let summary = serde_json::json!({
        "scene": args.scene.display().to_string(),
        "scene_sha256": scene.source_sha256,
        "model_policy_refused": scene.refused_steps,
        "format": args.format,
        "width": project.width,
        "height": project.height,
        "fps": project.fps,
        "duration_seconds": duration,
        "frame_count": report.frame_count,
        "expected_frame_count": expected_frames,
        "video_seconds": report.frame_count as f64 / f64::from(project.fps),
        "audio_samples": audio_samples,
        "audio_sample_rate": wav.sample_rate,
        "audio_seconds": audio_samples as f64 / f64::from(wav.sample_rate),
        "frame_sha256_first": frame_hashes.first(),
        "frame_sha256_last": frame_hashes.last(),
        "sequence_sha256": sequence_sha256,
        "reference_sequence_sha256": reference,
        "sequence_matches_reference": reference.map(|r| r == sequence_sha256),
        "video_file": (args.format == "mp4").then_some("scene.mp4"),
        "timing": {
            "load_ms": load_ms,
            "export_ms": export_ms,
            "export_ms_per_frame": export_ms / report.frame_count as f64,
            "measured_runs": run_ms.len(),
            "run_ms_median": median(&run_ms),
            "run_ms_min": run_ms.first(),
            "run_ms_max": run_ms.last(),
        },
        "conditions": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "available_parallelism": std::thread::available_parallelism().map_or(1, usize::from),
            "commit": std::env::var("KILSAT_COMMIT").ok(),
        },
    });
    println!("{}", serde_json::to_string_pretty(&summary)?);
    if report.frame_count != expected_frames || manifest["frame_count"] != expected_frames {
        return Err("frame count does not match frame_count(duration, fps)".into());
    }
    if reference.is_some_and(|r| r != sequence_sha256) {
        return Err("SVG sequence differs from G2_SVG_SEQUENCE_SHA256".into());
    }
    Ok(())
}
