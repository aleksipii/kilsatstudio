//! Offline consistency comparison of mouth tracks against real dialogue audio.
//!
//! ```sh
//! cargo run -p motion-lipsync --example lipsync_compare -- \
//!     [--rhubarb /absolute/rhubarb] [--out new-dir] [--floor-dbfs -50] [--relative-db 35] \
//!     <wav> <text> [<wav> <text> ...]
//! ```
//!
//! For each 24 kHz mono WAV and its exact line text it prints the RMS energy envelope
//! (leading/trailing silence, voiced segments), the text-estimated track scaled to the
//! whole clip ("full", the behaviour before the speech-span fix) and to the detected
//! speech span ("span"), and, with `--rhubarb`, Rhubarb cues from PocketSphinx (with
//! the transcript) and Phonetic. Every track is compared with the envelope and the
//! text tracks with each Rhubarb track. Rhubarb is itself an estimate: agreement is a
//! consistency check between estimators, not lip-sync quality. `--out` (must not
//! exist) receives one JSON file per line with all cues and metrics. No network use;
//! Rhubarb runs only when its absolute path is given.
use motion_lipsync::english::{
    estimate_viseme_track, estimate_viseme_track_in_span, Pronunciations,
};
use motion_lipsync::rhubarb::{align, MouthCue, Recognizer};
use motion_lipsync::timing_eval::{
    compare_tracks, energy_envelope, envelope_consistency, EnvelopeConfig, EnvelopeConsistency,
    TrackAgreement,
};
use serde::Serialize;
use std::path::{Path, PathBuf};

const RATE: u32 = 24_000;
const FRAME: u64 = 1000; // one 24 fps animation frame at 24 kHz

#[derive(Serialize)]
struct Track {
    name: String,
    cues: Vec<MouthCue>,
    vs_envelope: EnvelopeConsistency,
}

#[derive(Serialize)]
struct Pair {
    a: String,
    b: String,
    agreement: TrackAgreement,
}

#[derive(Serialize)]
struct Report {
    wav: String,
    text: String,
    frames: u64,
    duration_s: f64,
    envelope: motion_lipsync::timing_eval::Envelope,
    leading_silence_s: f64,
    trailing_silence_s: f64,
    tracks: Vec<Track>,
    pairs: Vec<Pair>,
    note: &'static str,
}

fn usage() -> ! {
    eprintln!("usage: lipsync_compare [--rhubarb /abs/rhubarb] [--out new-dir] [--floor-dbfs N] [--relative-db N] <wav> <text> [<wav> <text> ...]");
    std::process::exit(2);
}

fn s(samples: u64) -> f64 {
    samples as f64 / f64::from(RATE)
}

fn opt(v: Option<f64>) -> String {
    v.map_or("-".into(), |x| format!("{x:+.0}"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut take = |flag: &str| -> Option<PathBuf> {
        let i = args.iter().position(|a| a == flag)?;
        if i + 1 >= args.len() {
            usage();
        }
        let v = args.remove(i + 1);
        args.remove(i);
        Some(PathBuf::from(v))
    };
    let rhubarb = take("--rhubarb");
    let out = take("--out");
    let number = |v: Option<PathBuf>, default: f64| -> f64 {
        v.map_or(default, |p| {
            p.to_str()
                .and_then(|x| x.parse::<f64>().ok())
                .filter(|x| x.is_finite())
                .unwrap_or_else(|| usage())
        })
    };
    let mut config = EnvelopeConfig::DEFAULT_24K;
    config.absolute_floor_dbfs = number(take("--floor-dbfs"), config.absolute_floor_dbfs);
    config.relative_db = number(take("--relative-db"), config.relative_db);
    if args.is_empty() || args.len() % 2 != 0 {
        usage();
    }
    if let Some(out) = &out {
        if out.exists() {
            return Err("--out directory already exists".into());
        }
        std::fs::create_dir_all(out)?;
    }
    let overrides = Pronunciations::default();
    println!(
        "envelope: 20 ms RMS window, 10 ms hop, threshold max({} dBFS, peak - {} dB), gaps < 100 ms bridged, runs < 30 ms dropped",
        config.absolute_floor_dbfs, config.relative_db
    );
    println!(
        "Rhubarb is an estimate, not ground truth; agreement below is consistency, not quality.\n"
    );
    for pair in args.chunks(2) {
        let (wav, text) = (Path::new(&pair[0]), pair[1].as_str());
        let audio = motion_audio::read_wav(wav)?;
        if audio.sample_rate != RATE || audio.channels != 1 {
            return Err(format!("{}: expected 24 kHz mono", wav.display()).into());
        }
        let frames = audio.frames() as u64;
        let envelope = energy_envelope(&audio.samples, config);
        let mut named: Vec<(String, Vec<MouthCue>)> = Vec::new();
        named.push((
            "text_full".into(),
            estimate_viseme_track(text, &overrides, Some(frames))?.to_mouth_cues(),
        ));
        if let Some(span) = envelope.speech_span() {
            named.push((
                "text_span".into(),
                estimate_viseme_track_in_span(text, &overrides, frames, span.start, span.end)?
                    .to_mouth_cues(),
            ));
        }
        if let Some(exe) = &rhubarb {
            for (name, recognizer) in [
                ("rhubarb_pocketsphinx", Recognizer::PocketSphinx),
                ("rhubarb_phonetic", Recognizer::Phonetic),
            ] {
                let result = align(exe, wav, text, recognizer, || false)?;
                named.push((name.into(), result.cues));
            }
        }
        let tracks: Vec<Track> = named
            .iter()
            .map(|(name, cues)| Track {
                name: name.clone(),
                cues: cues.clone(),
                vs_envelope: envelope_consistency(cues, &envelope, RATE, FRAME),
            })
            .collect();
        let mut pairs = Vec::new();
        for (i, (a, ca)) in named.iter().enumerate() {
            for (b, cb) in &named[i + 1..] {
                if a.starts_with("text") && b.starts_with("text") {
                    continue;
                }
                pairs.push(Pair {
                    a: a.clone(),
                    b: b.clone(),
                    agreement: compare_tracks(ca, cb, frames),
                });
            }
        }
        println!("== {} ({:.3} s): {text}", wav.display(), s(frames));
        println!(
            "   peak {:.1} dBFS, threshold {:.1} dBFS, leading silence {:.3} s, trailing silence {:.3} s",
            envelope.peak_dbfs.unwrap_or(f64::NEG_INFINITY),
            envelope.threshold_dbfs,
            s(envelope.leading_silence()),
            s(envelope.trailing_silence())
        );
        let segs: Vec<String> = envelope
            .voiced
            .iter()
            .map(|v| format!("{:.3}-{:.3}", s(v.start), s(v.end)))
            .collect();
        println!("   voiced segments (s): {}", segs.join(", "));
        println!("   track                 onset_ms offset_ms seg_on_abs seg_off_abs open_in_silence(runs/>=1f/s) closed_in_voiced(runs/>=1f/s)");
        for t in &tracks {
            let c = &t.vs_envelope;
            println!(
                "   {:<21} {:>8} {:>9} {:>10} {:>11}   {:>3}/{:>3}/{:.3}{:>18}/{:>3}/{:.3}",
                t.name,
                opt(c.utterance_onset_error_ms),
                opt(c.utterance_offset_error_ms),
                c.segment_onset_mean_abs_ms
                    .map_or("-".into(), |x| format!("{x:.0}")),
                c.segment_offset_mean_abs_ms
                    .map_or("-".into(), |x| format!("{x:.0}")),
                c.open_during_silence.runs,
                c.open_during_silence.runs_at_least_one_frame,
                s(c.open_during_silence.samples),
                c.closed_during_voiced.runs,
                c.closed_during_voiced.runs_at_least_one_frame,
                s(c.closed_during_voiced.samples),
            );
        }
        println!("   pair                                       shape  active  open/closed");
        for p in &pairs {
            println!(
                "   {:<42} {:.3}  {:>6}  {:.3}",
                format!("{} vs {}", p.a, p.b),
                p.agreement.shape_agreement,
                p.agreement
                    .active_shape_agreement
                    .map_or("-".into(), |x| format!("{x:.3}")),
                p.agreement.open_closed_agreement
            );
        }
        println!();
        if let Some(out) = &out {
            let report = Report {
                wav: wav.display().to_string(),
                text: text.to_owned(),
                frames,
                duration_s: s(frames),
                leading_silence_s: s(envelope.leading_silence()),
                trailing_silence_s: s(envelope.trailing_silence()),
                envelope,
                tracks,
                pairs,
                note: "Envelope and Rhubarb are estimates; these are consistency metrics, not perceptual lip-sync quality.",
            };
            let stem = wav.file_stem().and_then(|x| x.to_str()).unwrap_or("line");
            std::fs::write(
                out.join(format!("{stem}.json")),
                serde_json::to_vec_pretty(&report)?,
            )?;
        }
    }
    Ok(())
}
