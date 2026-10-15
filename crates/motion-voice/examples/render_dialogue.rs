//! Headless dialogue synthesis and scene WAV export, using the same frame placement API as preview.
use motion_tts::{KokoroConfig, KokoroProvider, SpeechProvider};
use std::{path::PathBuf, time::Duration};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 7 {
        return Err("usage: render_dialogue SCRIPT ASSET_DIR PYTHON RUNTIME_SCRIPT OUTPUT_DIR MODEL_AND_RUNTIME_REVISION".into());
    }
    let root = PathBuf::from(&args[1]);
    let script = motion_script::parse(&std::fs::read_to_string(&root)?)?;
    let assets = PathBuf::from(&args[2]).canonicalize()?;
    let output = PathBuf::from(&args[5]);
    std::fs::create_dir_all(&output)?;
    let output = output.canonicalize()?;
    let provider = KokoroProvider {
        config: KokoroConfig {
            python: PathBuf::from(&args[3]),
            runtime_script: PathBuf::from(&args[4]).canonicalize()?,
            model_config: assets.join("config.json"),
            model_weights: assets.join("kokoro-v1_0.pth"),
            voices_directory: assets.join("voices"),
            cache_directory: output.join("cache"),
            model_version: args[6].clone(),
            timeout: Duration::from_secs(180),
        },
    };
    let jobs =
        motion_voice::dialogue_jobs(&script, root.parent().unwrap_or(std::path::Path::new(".")))?;
    for (index, scene) in script.scenes.iter().enumerate() {
        let mut clips = Vec::new();
        for job in jobs.iter().filter(|j| j.scene == scene.id) {
            eprintln!("Generating {} / {}", scene.id, job.actor);
            let result = if let Some(path) = &job.imported_audio {
                // Imported speech bypasses inference; prepare validates format.
                motion_tts::SpeechResult {
                    audio_path: path.clone(),
                    sample_rate: 24000,
                    sample_count: 0,
                    cache_key: String::new(),
                    phonemes: String::new(),
                    cached: false,
                }
            } else {
                provider.synthesize(job.request.clone())?
            };
            let clip = motion_voice::timeline::DialogueClip::prepare(job, &result, None)?;
            eprintln!("{}: {:.3}s", job.actor, clip.audio.duration_seconds());
            clips.push(clip);
        }
        let rendered = motion_voice::export::render_scene(&scene.id, scene.duration, &clips)?;
        motion_audio::write_wav(output.join(format!("scene-{index}.wav")), &rendered.audio)?;
        std::fs::write(
            output.join(format!("scene-{index}.json")),
            serde_json::to_vec_pretty(&rendered.manifest)?,
        )?;
    }
    Ok(())
}
