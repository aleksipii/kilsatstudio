//! Explicit invocation of the user's existing local executable. No downloads.
use motion_lipsync::rhubarb::{align, Recognizer};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 5 {
        return Err("usage: rhubarb_align /absolute/rhubarb audio.wav transcript.txt output.json (English pocketSphinx)".into());
    }
    let mut text = String::new();
    fs::File::open(&args[3])?
        .take(65537)
        .read_to_string(&mut text)?;
    if text.len() > 65536 {
        return Err("transcript exceeds 64 KiB".into());
    }
    if Path::new(&args[4]).exists() {
        return Err("output already exists".into());
    }
    let result = align(
        Path::new(&args[1]),
        Path::new(&args[2]),
        &text,
        Recognizer::PocketSphinx,
        || false,
    )?;
    let bytes = serde_json::to_vec_pretty(&result)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[4])?
        .write_all(&bytes)?;
    println!("Wrote estimated mouth cues; this is not forced alignment or perceptual acceptance.");
    Ok(())
}
