//! Export sampled scene poses, not rendered frames, using the preview evaluator.
use std::{env, error::Error, fs, io::Write, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: evaluate_scene SCRIPT SCENE_INDEX OUTPUT".into());
    }
    let script = motion_script::parse(&fs::read_to_string(&args[0])?)?;
    let scene_index: usize = args[1].parse()?;
    let scene = script
        .scenes
        .get(scene_index)
        .ok_or("scene index does not exist")?;
    let frame_count = motion_scene::frame_count(scene.duration, script.project.fps, 100_000)?;
    // Bound both work and output size, including high character-count scripts.
    if frame_count as f64 * script.characters.len() as f64 > 1_000_000.0 {
        return Err("pose export exceeds budget of 100000 frames or 1000000 actor poses".into());
    }
    // Refuse to replace an existing artifact; incomplete output is removed on failure.
    let output_path = Path::new(&args[2]);
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)?;
    let result = (|| -> Result<(), Box<dyn Error>> {
        write!(
            output,
            "{{\"fps\":{},\"scene\":{},\"frames\":[",
            script.project.fps,
            serde_json::to_string(&scene.id)?
        )?;
        for frame in 0..frame_count {
            let time = frame as f64 / script.project.fps as f64;
            let evaluated = motion_scene::evaluate(&script, scene_index, time)?;
            let actors: Vec<_> = evaluated.actors.iter().map(|actor| serde_json::json!({
                "id": actor.id, "x": actor.x, "y": actor.y, "emotion": actor.emotion, "wave": actor.wave
            })).collect();
            if frame != 0 {
                write!(output, ",")?;
            }
            serde_json::to_writer(
                &mut output,
                &serde_json::json!({
                    "frame": frame, "time_seconds": time,
                    "width": evaluated.width, "height": evaluated.height, "actors": actors
                }),
            )?;
        }
        writeln!(output, "]}}")?;
        output.flush()?;
        Ok(())
    })();
    if result.is_err() {
        drop(output);
        let _ = fs::remove_file(output_path);
    }
    result?;
    println!(
        "Exported {} sampled pose frames to {} (not rendered images)",
        frame_count,
        output_path.display()
    );
    Ok(())
}
