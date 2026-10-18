//! Explicit scene-geometry experiment. Raw GPU pixels and independent SVG source
//! are evidence, not an asserted CPU/GPU pixel-equivalence result.
use motion_render::{gpu::Backend, gpu_scene, scene_commands::Labels};
use std::{path::PathBuf, sync::atomic::AtomicBool};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 8 || args[1] != "--omit-labels-acknowledged" {
        return Err("usage: gpu_scene_probe --metal|--vulkan|--gl --omit-labels-acknowledged SCRIPT SCENE_INDEX SECONDS WIDTH HEIGHT NEW_OUTPUT_DIRECTORY".into());
    }
    let backend = match args[0].as_str() {
        "--metal" => Backend::Metal,
        "--vulkan" => Backend::Vulkan,
        "--gl" => Backend::Gl,
        _ => return Err("explicit backend required".into()),
    };
    let source_path = PathBuf::from(&args[2]);
    if std::fs::metadata(&source_path)?.len() > 8 * 1024 * 1024 {
        return Err("script exceeds the 8 MiB probe input limit".into());
    }
    // `take` also handles a file that grows after metadata inspection.
    let mut source = String::new();
    use std::io::Read;
    std::fs::File::open(&source_path)?.take(8 * 1024 * 1024 + 1).read_to_string(&mut source)?;
    if source.len() > 8 * 1024 * 1024 { return Err("script exceeds input limit".into()); }
    let script = motion_script::parse(&source)?;
    script.validate()?;
    if script.scenes.iter().any(|scene| scene.composition.transition != motion_script::Transition::Cut) {
        return Err(motion_render::scene_commands::reject_artwork_or_sequence().into());
    }
    let scene_index: usize = args[3].parse()?;
    let seconds: f64 = args[4].parse()?;
    let width: u32 = args[5].parse()?;
    let height: u32 = args[6].parse()?;
    let output = PathBuf::from(&args[7]);
    if output.exists() { return Err("output already exists".into()); }
    let scene = motion_scene::evaluate(&script, scene_index, seconds)?;
    let frame = gpu_scene::render_scene(&scene, width, height, &[], Labels::OmitAcknowledged, backend, &AtomicBool::new(false))?;
    // CPU SVG uses the identical evaluated state, but includes font labels. No
    // raster comparison, acceleration or physical Mac acceptance is claimed.
    let svg = motion_render::render_svg_with_mouths(&scene, width, height, &[])?;
    std::fs::create_dir(&output)?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        std::fs::write(output.join("scene.rgba"), &frame.rgba)?;
        std::fs::write(output.join("reference.svg"), svg)?;
        std::fs::write(output.join("source.yaml"), source)?;
        std::fs::write(output.join("report.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version":1,"experiment":"evaluated-geometry-v1","adapter":frame.adapter_name,
            "backend":frame.backend,"device_type":frame.device_type,"width":frame.width,"height":frame.height,
            "scene_index":scene_index,"scene_seconds":seconds,"format":"rgba8unorm-top-left",
            "row_padding_removed":true,"labels":"omitted-with-explicit-acknowledgement",
            "mouths":"rest-no-audio-preparation","artwork":"not-loaded-script-only",
            "cpu_reference":"SVG-with-labels-same-evaluated-scene","pixel_parity":"not-measured",
            "validation_errors":frame.validation_errors,"full_scene_parity":false,
            "implementation_commit":std::env::var("KILSAT_IMPLEMENTATION_COMMIT").ok(),
        }))?)?;
        Ok(())
    })();
    if result.is_err() { let _ = std::fs::remove_dir_all(&output); }
    result
}
