//! Pre-render pronunciation report:
//! `cargo run -p motion-lipsync --example english_report -- "text" [audio_frames_at_24khz]`
use motion_lipsync::english::{estimate_viseme_track, pronunciation_report, Pronunciations};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(text) = args.next() else {
        eprintln!("usage: english_report \"text\" [audio_frames_at_24khz]");
        std::process::exit(2);
    };
    let frames = args.next().and_then(|f| f.parse::<u64>().ok());
    let none = Pronunciations::new();
    let report = pronunciation_report(&text, &none);
    for w in &report.words {
        println!(
            "{:<20} {:<9} {}{}",
            w.word,
            format!("{:?}", w.source).to_lowercase(),
            w.phonemes.join(" "),
            if w.low_confidence {
                "   LOW CONFIDENCE"
            } else {
                ""
            }
        );
    }
    let low = report.low_confidence_words();
    if !low.is_empty() {
        println!("add `pronunciations:` overrides for: {}", low.join(", "));
    }
    match estimate_viseme_track(&text, &none, frames) {
        Ok(t) => println!(
            "{} cues over {} samples (estimated, time_scale {:.2})",
            t.cues.len(),
            t.frame_count,
            t.time_scale
        ),
        Err(e) => println!("no track: {e}"),
    }
}
