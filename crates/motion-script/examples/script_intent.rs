//! Convert a bounded screenplay text into `ScriptIntent v1` JSON.
//!
//! ```sh
//! cargo run -p motion-script --example script_intent -- \
//!     examples/automation/pilot-fi.txt --language fi-FI --seed 31 --scene-id varikko
//! ```
//!
//! Writes JSON to stdout (or `--output PATH`). Errors are printed with line and
//! column and the process exits with status 1. This is intake only: no
//! MotionScript, audio or render is produced.

use motion_script::intent::{script_intent_from_text, IntakeOptions, IntentLanguage};
use std::process::ExitCode;

const USAGE: &str = "usage: script_intent INPUT --language fi-FI|en-US|en-GB --seed N --scene-id ID [--output PATH]";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut input, mut language, mut seed, mut scene, mut output) = (None, None, None, None, None);
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .ok_or_else(|| format!("{arg} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--language" => {
                let v = value()?;
                language =
                    Some(IntentLanguage::parse(&v).ok_or(format!("unsupported language {v}"))?);
            }
            "--seed" => {
                seed = Some(
                    value()?
                        .parse::<u32>()
                        .map_err(|e| format!("--seed: {e}"))?,
                )
            }
            "--scene-id" => scene = Some(value()?),
            "--output" => output = Some(value()?),
            _ if input.is_none() && !arg.starts_with("--") => input = Some(arg),
            _ => return Err(format!("unexpected argument {arg}\n{USAGE}")),
        }
    }
    let missing = |what: &str| format!("missing {what}\n{USAGE}");
    let input = input.ok_or_else(|| missing("INPUT"))?;
    let options = IntakeOptions {
        language: language.ok_or_else(|| missing("--language"))?,
        seed: seed.ok_or_else(|| missing("--seed"))?,
        scene_id: scene.ok_or_else(|| missing("--scene-id"))?,
    };
    let bytes = std::fs::read(&input).map_err(|e| format!("{input}: {e}"))?;
    let intake = script_intent_from_text(&bytes, &options).map_err(|failure| {
        let text = String::from_utf8_lossy(&bytes);
        failure
            .errors
            .iter()
            .map(|e| match e.span {
                Some(span) => {
                    let (line, col) = span.line_column(&text);
                    format!("{input}:{line}:{col}: {:?}: {}", e.kind, e.message)
                }
                None => format!("{input}: {:?}: {}", e.kind, e.message),
            })
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let json = intake.intent.to_json_pretty();
    match output {
        Some(path) => std::fs::write(&path, json).map_err(|e| format!("{path}: {e}"))?,
        None => print!("{json}"),
    }
    for u in &intake.intent.unresolved {
        let (line, col) = u.source_span.line_column(&String::from_utf8_lossy(&bytes));
        eprintln!(
            "{input}:{line}:{col}: needs review ({:?}, {}): {}",
            u.code, u.rule_id, u.source_text
        );
    }
    Ok(())
}
