//! Hand-run, release-profile wall-clock benchmarks with fixed deterministic fixtures
//! (silent synthetic audio, built-in vector characters; no TTS, no downloads, no GUI).
//!
//! ```sh
//! cargo test --release -p motion-export --test benchmark -- --ignored --nocapture --test-threads=1
//! ```
//!
//! `KILSAT_BENCH_RUNS` (default 7) sets measured runs per benchmark; one extra warm-up
//! run is executed and discarded. Results are printed as one `BENCH` line per benchmark
//! (milliseconds per run). These are single-machine observations: they say nothing about
//! other hardware, GPU paths, input latency or display presentation.
use motion_export::export_png_sequence;
use motion_tts::CancellationToken;
use motion_voice::export::{RenderedScene, SceneManifest};
use std::{path::PathBuf, time::Instant};

const SCRIPT: &str = "project: {fps: 24, width: 1280, height: 720}
characters:
  EMMA: {asset: characters/emma}
  ALEX: {asset: characters/alex}
scenes:
  - id: one
    duration: 1.0
    events:
      - {type: move, at: 0.0, actor: EMMA, x: 400, y: 500, duration: 0.5}
      - {type: move, at: 0.0, actor: ALEX, x: 900, y: 500, duration: 0.5}
      - {type: emotion, at: 0.25, actor: EMMA, value: happy}
      - {type: gesture, at: 0.25, actor: EMMA, value: wave, duration: 0.5}
  - id: two
    duration: 1.0
    events:
      - {type: move, at: 0.0, actor: EMMA, x: 600, y: 520, duration: 1.0}
      - {type: emotion, at: 0.5, actor: ALEX, value: surprised}
";

fn runs() -> usize {
    std::env::var("KILSAT_BENCH_RUNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|n| *n >= 1)
        .unwrap_or(7)
}

fn silent(id: &str, frames: u64) -> RenderedScene {
    RenderedScene {
        audio: motion_audio::AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: vec![0.0; frames as usize],
        },
        manifest: SceneManifest {
            schema_version: 1,
            scene_id: id.into(),
            sample_rate: 24000,
            channels: 1,
            total_frames: frames,
            dialogue: vec![],
            cues: vec![],
        },
    }
}

/// Run `work` once to warm up, then `runs()` measured times; print a BENCH line.
fn bench(name: &str, detail: &str, mut work: impl FnMut()) {
    work();
    let mut ms: Vec<f64> = (0..runs())
        .map(|_| {
            let start = Instant::now();
            work();
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    ms.sort_by(|a, b| a.total_cmp(b));
    let median = if ms.len() % 2 == 1 {
        ms[ms.len() / 2]
    } else {
        (ms[ms.len() / 2 - 1] + ms[ms.len() / 2]) / 2.0
    };
    println!(
        "BENCH {name} | {detail} | runs={} | median_ms={median:.2} | min_ms={:.2} | max_ms={:.2} | profile={} | os={} arch={}",
        ms.len(),
        ms[0],
        ms[ms.len() - 1],
        if cfg!(debug_assertions) { "debug" } else { "release" },
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
}

#[test]
#[ignore = "hand-run wall-clock benchmark; use --release"]
fn bench_scene_evaluation() {
    let script = motion_script::parse(SCRIPT).unwrap();
    // 2 scenes x 100 times spread over [0, 1.0): 200 evaluations per run.
    bench("scene_evaluation", "200 evaluations per run", || {
        for scene in 0..2 {
            for i in 0..100 {
                let scene = motion_scene::evaluate(&script, scene, i as f64 / 100.0).unwrap();
                std::hint::black_box(&scene);
            }
        }
    });
}

#[test]
#[ignore = "hand-run wall-clock benchmark; use --release"]
fn bench_svg_scene_render_to_png_1280x720() {
    let script = motion_script::parse(SCRIPT).unwrap();
    // 24 frames of scene one, each: evaluate -> SVG -> resvg raster -> PNG encode.
    bench(
        "svg_scene_to_png_1280x720",
        "24 frames per run (evaluate+SVG+raster+PNG encode)",
        || {
            for frame in 0..24 {
                let scene = motion_scene::evaluate(&script, 0, frame as f64 / 24.0).unwrap();
                let svg = motion_render::render_svg(&scene, 1280, 720).unwrap();
                let tree =
                    resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
                let mut pixmap = resvg::tiny_skia::Pixmap::new(1280, 720).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut pixmap.as_mut(),
                );
                let png = pixmap.encode_png().unwrap();
                assert!(png.len() > 100);
                std::hint::black_box(&png);
            }
        },
    );
}

#[test]
#[ignore = "hand-run wall-clock benchmark; use --release"]
fn bench_sequence_export_png_48_frames_1280x720() {
    let script = motion_script::parse(SCRIPT).unwrap();
    let rendered = [silent("one", 24000), silent("two", 24000)];
    let root: PathBuf = std::env::temp_dir().join(format!("kilsat-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let mut n = 0;
    bench(
        "sequence_export_png",
        "2 scenes x 1 s, 24 fps = 48 PNG frames + audio.wav, 1280x720, to local temp dir",
        || {
            n += 1;
            let report = export_png_sequence(
                &script,
                &rendered,
                None,
                &root.join(format!("run{n}")),
                &CancellationToken::new(),
                |_| {},
            )
            .unwrap();
            assert_eq!(report.frame_count, 48);
        },
    );
    std::fs::remove_dir_all(&root).unwrap();
}
