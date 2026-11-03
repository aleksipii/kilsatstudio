//! Format MotionScript YAML in the canonical layout (`motion_script::format`).
//!
//! ```sh
//! cargo run -q -p motion-script --example format_script -- FILE...         # rewrite in place
//! cargo run -q -p motion-script --example format_script -- --check FILE... # exit 1 if any differ
//! cargo run -q -p motion-script --example format_script -- --stdin         # YAML or JSON in, YAML out
//! cargo run -q -p motion-script --example format_script -- --to-json       # YAML in, compact JSON out
//! ```
//!
//! A `.kilsat` project is changed only in its `source` string; every other
//! byte of the project file stays as it was. Other files are formatted whole.
use motion_script::format::{format_source, Formatted};
use std::{
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
};

const MAX_BYTES: u64 = 16 * 1024 * 1024;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("format_script: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode> {
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help") => {
            eprintln!("usage: format_script [--check] FILE... | --stdin | --to-json");
            Ok(ExitCode::from(2))
        }
        Some("--stdin") => {
            let formatted = format_source(&stdin()?)?;
            report("<stdin>", &formatted);
            io::stdout().write_all(formatted.text.as_bytes())?;
            Ok(ExitCode::SUCCESS)
        }
        Some("--to-json") => {
            let value: serde_yaml::Value = serde_yaml::from_str(&stdin()?)?;
            serde_json::to_writer(io::stdout(), &value)?;
            Ok(ExitCode::SUCCESS)
        }
        Some("--check") => {
            let mut unformatted = 0;
            for path in &args[1..] {
                let (before, after) = rewrite(Path::new(path))?;
                if before != after {
                    println!("not formatted: {path}");
                    unformatted += 1;
                }
            }
            Ok(if unformatted == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        Some(_) => {
            for path in args {
                let (before, after) = rewrite(Path::new(path))?;
                if before != after {
                    std::fs::write(path, after)?;
                    println!("formatted: {path}");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn stdin() -> Result<String> {
    let mut text = String::new();
    io::stdin().take(MAX_BYTES + 1).read_to_string(&mut text)?;
    if text.len() as u64 > MAX_BYTES {
        return Err("input exceeds 16 MiB".into());
    }
    Ok(text)
}

/// The file's current text and its formatted text.
fn rewrite(path: &Path) -> Result<(String, String)> {
    if std::fs::metadata(path)?.len() > MAX_BYTES {
        return Err(format!("{}: file exceeds 16 MiB", path.display()).into());
    }
    let text = std::fs::read_to_string(path)?;
    let name = path.display().to_string();
    if path.extension().is_some_and(|e| e == "kilsat") {
        let project: serde_json::Value = serde_json::from_str(&text)?;
        let source = project
            .get("source")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("{name}: project has no source string"))?;
        let formatted = format_source(source).map_err(|e| format!("{name}: {e}"))?;
        report(&name, &formatted);
        let old = serde_json::to_string(source)?;
        let new = serde_json::to_string(&formatted.text)?;
        if text.matches(&old).count() != 1 {
            return Err(format!("{name}: cannot locate the source string uniquely").into());
        }
        let after = text.replacen(&old, &new, 1);
        Ok((text, after))
    } else {
        let formatted = format_source(&text).map_err(|e| format!("{name}: {e}"))?;
        report(&name, &formatted);
        Ok((text, formatted.text))
    }
}

fn report(name: &str, formatted: &Formatted) {
    if formatted.dropped_comments > 0 {
        eprintln!(
            "{name}: {} comment line(s) inside sections were removed",
            formatted.dropped_comments
        );
    }
}
