//! Night-room reference sequence helper.
//!
//! ```sh
//! cargo run -p motion-export --example night_room_reference                  # timing report
//! cargo run -p motion-export --example night_room_reference -- write         # refresh files
//! cargo run -p motion-export --example night_room_reference -- report <wav-dir>
//! cargo run -p motion-export --example night_room_reference -- timing [<wav-dir>]
//! cargo run -p motion-export --example night_room_reference -- frames <out-dir> [<wav-dir>]
//! cargo run -p motion-export --example night_room_reference -- export <out-dir> [<wav-dir>] [--ffmpeg <path>]
//! ```
//!
//! `report` prints each shot's start/end, the cut points on the 24 kHz sequence
//! clock, caption windows and mouth-variant switch counts. `timing` prints the
//! comic-timing measurements (docs/lipsync-timing-evaluation.md). `write` regenerates
//! `night-room.kilsat`, the phone-screen PNGs and the placeholder `audio/*.wav`
//! fixtures. `frames` writes one PNG per shot plus `contact-sheet.png` with every
//! feature on. `export` writes the PNG sequence (and `sequence.mp4` when an explicit
//! FFmpeg is given). A `<wav-dir>` holding `audio/*.wav` (for example real Kokoro
//! speech from `scripts/night_room_real_speech.py`) replaces the placeholder tones
//! and re-times the shot list to the WAV lengths.
#[path = "../tests/support/mod.rs"]
mod support;

use motion_export::render_sequence_frame;
use motion_tts::CancellationToken;
use resvg::tiny_skia::{IntSize, Pixmap, PixmapPaint, Transform};
use std::path::{Path, PathBuf};

fn usage() -> ! {
    eprintln!("usage: night_room_reference [report [wav-dir] | timing [wav-dir] | write | frames <out> [wav-dir] | export <out> [wav-dir] [--ffmpeg path]]");
    std::process::exit(2);
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut ffmpeg = None;
    if let Some(i) = args.iter().position(|a| a == "--ffmpeg") {
        if i + 1 >= args.len() {
            usage();
        }
        ffmpeg = Some(PathBuf::from(args.remove(i + 1)));
        args.remove(i);
    }
    let path = |i: usize| args.get(i).map(PathBuf::from);
    match args.first().map(String::as_str) {
        None => report(None),
        Some("report") => report(path(1).as_deref()),
        Some("timing") => {
            let real = path(1);
            let (expansion, rendered) = support::build(real.as_deref());
            let dir = support::audio_source_dir(real.as_deref());
            let timings = support::comic_timing(&expansion, &rendered, &dir);
            print!("{}", support::comic_timing_report(&timings));
        }
        Some("write") => {
            support::write_all(&support::example_dir());
            println!("wrote {}", support::example_dir().display());
        }
        Some("frames") => frames(&path(1).unwrap_or_else(|| usage()), path(2).as_deref()),
        Some("export") => export(
            &path(1).unwrap_or_else(|| usage()),
            path(2).as_deref(),
            ffmpeg.as_deref(),
        ),
        Some(_) => usage(),
    }
}

fn report(real: Option<&Path>) {
    let (expansion, rendered) = support::build(real);
    print!("{}", support::timing_report(&expansion));
    println!();
    print!("{}", support::dialogue_report(&expansion, &rendered));
}

fn pixmap(rgba: Vec<u8>, w: u32, h: u32) -> Pixmap {
    Pixmap::from_vec(rgba, IntSize::from_wh(w, h).unwrap()).unwrap()
}

fn frames(out: &Path, real: Option<&Path>) {
    let (expansion, rendered) = support::build(real);
    let document = support::project_document(&expansion);
    let art = support::art_context(&document);
    std::fs::create_dir_all(out).unwrap();
    // Preview side in pixels (default the project width; up to 1024 for zoomed review).
    let side: u32 = std::env::var("KILSAT_NIGHT_ROOM_SIDE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(640);
    let mut sheet = Pixmap::new(side * 2, side * 9 / 8).unwrap();
    for (index, sample) in support::representative_samples(&expansion)
        .into_iter()
        .enumerate()
    {
        let frame = render_sequence_frame(
            &expansion.script,
            &rendered,
            Some(&art),
            sample,
            side,
            &CancellationToken::new(),
        )
        .unwrap();
        let image = pixmap(frame.rgba, frame.width, frame.height);
        let id = &expansion.plan[index].scene_id;
        image
            .save_png(out.join(format!("{index}_{id}.png")))
            .unwrap();
        sheet.draw_pixmap(
            (index as i32 % 2) * image.width() as i32,
            (index as i32 / 2) * image.height() as i32,
            image.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        println!(
            "{index} {id}: sample {sample} ({:.3} s)",
            sample as f64 / 24_000.0
        );
    }
    sheet.save_png(out.join("contact-sheet.png")).unwrap();
    println!("wrote frames and contact-sheet.png to {}", out.display());
}

fn export(out: &Path, real: Option<&Path>, ffmpeg: Option<&Path>) {
    let (expansion, rendered) = support::build(real);
    let document = support::project_document(&expansion);
    let art = support::art_context(&document);
    let cancel = CancellationToken::new();
    let started = std::time::Instant::now();
    let report = match ffmpeg {
        Some(ffmpeg) => motion_export::export_mp4_sequence(
            &expansion.script,
            &rendered,
            Some(&art),
            out,
            ffmpeg,
            &cancel,
            |_| {},
        ),
        None => motion_export::export_png_sequence(
            &expansion.script,
            &rendered,
            Some(&art),
            out,
            &cancel,
            |_| {},
        ),
    }
    .unwrap_or_else(|e| {
        eprintln!("export failed: {e}");
        std::process::exit(1);
    });
    println!(
        "exported {report:?} to {} in {:.1} s",
        out.display(),
        started.elapsed().as_secs_f64()
    );
}
