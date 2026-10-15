//! Deterministic SVG sequence export; no video encoding or inferred speech timing.
use std::{
    env,
    error::Error,
    fs,
    io::{Read, Write},
    path::Path,
};

type ExportResult<T> = Result<T, Box<dyn Error>>;
const MAX_FRAMES: u64 = 10_000;
const MAX_ACTOR_FRAMES: u64 = 100_000;
const MAX_SOURCE_BYTES: usize = 1024 * 1024;
const MAX_EVENTS: usize = 10_000;
const MAX_EVENT_WORK: u64 = 1_000_000;
const MAX_BYTES: usize = 256 * 1024 * 1024;

fn export(source: &str, scene_index: usize, destination: &Path) -> ExportResult<u64> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err("MotionScript source exceeds 1 MiB input budget".into());
    }
    let script = motion_script::parse(source)?;
    let total_events: usize = script.scenes.iter().map(|scene| scene.events.len()).sum();
    if total_events > MAX_EVENTS {
        return Err("MotionScript exceeds 10000 total events".into());
    }
    let scene = script
        .scenes
        .get(scene_index)
        .ok_or("scene index does not exist")?;
    let frame_count = motion_scene::frame_count(scene.duration, script.project.fps, MAX_FRAMES)?;
    let frames = frame_count as f64;
    if frames * script.characters.len() as f64 > MAX_ACTOR_FRAMES as f64 {
        return Err("SVG export exceeds budget of 100000 actor frames".into());
    }
    // Evaluation validates every scene and scans selected-scene events per actor.
    // Charge all events per frame and actor to conservatively bound that work.
    if frames * total_events as f64 * (script.characters.len() as f64 + 1.0) > MAX_EVENT_WORK as f64
    {
        return Err("SVG export exceeds 1000000 event-frame-actor work budget".into());
    }
    // Validate renderer dimensions and first pose before creating any output.
    let first = motion_scene::evaluate(&script, scene_index, 0.0)?;
    let first_svg = motion_render::render_svg(&first, first.width, first.height)?;
    fs::create_dir(destination)?; // An existing directory is never overwritten.
    let result = (|| -> ExportResult<u64> {
        let mut bytes = 0usize;
        let mut entries = Vec::with_capacity(frame_count as usize);
        for frame in 0..frame_count {
            // Same exclusive scene-end frame clock as evaluate_scene and preview.
            let time_seconds = frame as f64 / script.project.fps as f64;
            let svg = if frame == 0 {
                first_svg.clone()
            } else {
                let pose = motion_scene::evaluate(&script, scene_index, time_seconds)?;
                motion_render::render_svg(&pose, pose.width, pose.height)?
            };
            bytes = bytes
                .checked_add(svg.len())
                .ok_or("SVG byte budget overflow")?;
            if bytes > MAX_BYTES {
                return Err("SVG sequence exceeds 256 MiB output budget".into());
            }
            let filename = format!("frame_{frame:06}.svg");
            write_new(&destination.join(&filename), svg.as_bytes())?;
            entries.push(
                serde_json::json!({"frame": frame,"time_seconds": time_seconds,"file": filename}),
            );
        }
        let manifest = serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": 1, "format": "svg_sequence", "scene_index": scene_index,
            "scene_id": scene.id, "duration_seconds": scene.duration,
            "fps": script.project.fps, "width": script.project.width, "height": script.project.height,
            "frame_count": frame_count, "frames": entries,
            "source_motionscript": source,
            "audio_included": false, "mouth_timing": "neutral; no alignment supplied"
        }))?;
        if bytes
            .checked_add(manifest.len())
            .ok_or("manifest byte budget overflow")?
            > MAX_BYTES
        {
            return Err("SVG sequence and manifest exceed 256 MiB output budget".into());
        }
        write_new(&destination.join("manifest.json"), &manifest)?;
        Ok(frame_count)
    })();
    if result.is_err() {
        // Only a directory created successfully by this invocation is removed.
        fs::remove_dir_all(destination).map_err(|error| {
            format!("export failed and partial output could not be removed: {error}")
        })?;
    }
    result
}

fn write_new(path: &Path, contents: &[u8]) -> ExportResult<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(contents)?;
    file.flush()?;
    Ok(())
}

fn read_source(path: &Path) -> ExportResult<String> {
    let mut source = String::new();
    fs::File::open(path)?
        .take(MAX_SOURCE_BYTES as u64 + 1)
        .read_to_string(&mut source)?;
    if source.len() > MAX_SOURCE_BYTES {
        return Err("MotionScript source exceeds 1 MiB input budget".into());
    }
    Ok(source)
}

fn main() -> ExportResult<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: render_frames SCRIPT SCENE_INDEX NEW_OUTPUT_DIRECTORY".into());
    }
    let source = read_source(Path::new(&args[0]))?;
    let count = export(&source, args[1].parse()?, Path::new(&args[2]))?;
    println!(
        "Exported {count} SVG frames and manifest.json to {} (no video or audio)",
        args[2]
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    const SCRIPT: &str = "project: {fps: 2, width: 640, height: 360}\ncharacters:\n  EMMA: {asset: characters/emma}\nscenes:\n  - id: intro\n    duration: 1.0\n    events:\n      - {type: move, at: 0.0, actor: EMMA, x: 100, y: 100, duration: 1.0}\n";
    fn temp() -> std::path::PathBuf {
        env::temp_dir().join(format!(
            "kilsat-svg-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
    #[test]
    fn deterministic_frames_and_manifest_preserve_frame_clock() {
        let a = temp();
        let b = temp();
        assert_eq!(export(SCRIPT, 0, &a).unwrap(), 2);
        assert_eq!(export(SCRIPT, 0, &b).unwrap(), 2);
        for file in ["frame_000000.svg", "frame_000001.svg", "manifest.json"] {
            assert_eq!(
                fs::read(a.join(file)).unwrap(),
                fs::read(b.join(file)).unwrap()
            );
        }
        assert_ne!(
            fs::read(a.join("frame_000000.svg")).unwrap(),
            fs::read(a.join("frame_000001.svg")).unwrap()
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(a.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["frames"][1]["time_seconds"], 0.5);
        assert_eq!(manifest["source_motionscript"], SCRIPT);
        fs::remove_dir_all(a).unwrap();
        fs::remove_dir_all(b).unwrap();
    }
    #[test]
    fn rejects_existing_directory_and_invalid_inputs_without_modification() {
        let path = temp();
        fs::create_dir(&path).unwrap();
        fs::write(path.join("keep"), b"original").unwrap();
        assert!(export(SCRIPT, 0, &path).is_err());
        assert_eq!(fs::read(path.join("keep")).unwrap(), b"original");
        fs::remove_dir_all(&path).unwrap();
        assert!(export(SCRIPT, 9, &path).is_err());
        assert!(!path.exists());
        assert!(export(
            &SCRIPT.replace("duration: 1.0", "duration: 10001.0"),
            0,
            &path
        )
        .is_err());
        assert!(!path.exists());
    }
    #[test]
    fn rejects_oversized_source_before_parsing_or_output() {
        let path = temp();
        let oversized = "!".repeat(MAX_SOURCE_BYTES + 1);
        assert!(export(&oversized, 0, &path)
            .unwrap_err()
            .to_string()
            .contains("1 MiB"));
        assert!(!path.exists());
        let input = temp();
        fs::write(&input, oversized).unwrap();
        assert!(read_source(&input)
            .unwrap_err()
            .to_string()
            .contains("1 MiB"));
        fs::remove_file(input).unwrap();
    }
    #[test]
    fn rejects_event_count_and_multiscene_evaluation_work_without_output() {
        let path = temp();
        let event = "      - {type: emotion, at: 0.0, actor: EMMA, value: happy}\n";
        let header = SCRIPT.split("    events:").next().unwrap();
        let excessive_events = format!("{header}    events:\n{}", event.repeat(MAX_EVENTS + 1));
        assert!(export(&excessive_events, 0, &path)
            .unwrap_err()
            .to_string()
            .contains("total events"));
        assert!(!path.exists());
        // Selected scene has no events: validation of OTHER scenes still costs work.
        let other_scene = format!(
            "{header}    events: []\n  - id: other\n    duration: 1.0\n    events:\n{}",
            event.repeat(100)
        );
        let long_scene = other_scene.replacen("duration: 1.0", "duration: 3000.0", 1);
        assert!(export(&long_scene, 0, &path)
            .unwrap_err()
            .to_string()
            .contains("event-frame-actor"));
        assert!(!path.exists());
    }
    #[test]
    fn floating_duration_does_not_include_scene_endpoint() {
        let path = temp();
        let source = SCRIPT
            .replace("fps: 2", "fps: 25")
            .replace("duration: 1.0", "duration: 0.28");
        assert_eq!(export(&source, 0, &path).unwrap(), 7);
        assert!(!path.join("frame_000007.svg").exists());
        fs::remove_dir_all(path).unwrap();
    }
}
