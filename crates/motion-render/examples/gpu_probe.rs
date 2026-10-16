//! Explicit experimental hardware probe; writes raw RGBA and metadata JSON.
use motion_render::gpu::{render_fixture, Backend};
use std::{path::PathBuf, sync::atomic::AtomicBool};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: gpu_probe --metal|--vulkan|--gl NEW_OUTPUT_DIRECTORY".into());
    }
    let backend = match args[0].as_str() {
        "--metal" => Backend::Metal,
        "--vulkan" => Backend::Vulkan,
        "--gl" => Backend::Gl,
        _ => return Err("explicit backend required".into()),
    };
    let output = PathBuf::from(&args[1]);
    if output.exists() {
        return Err("output already exists".into());
    }
    let frame = render_fixture(backend, 256, 256, true, &AtomicBool::new(false))?;
    std::fs::create_dir(&output)?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        std::fs::write(output.join("fixture.rgba"), &frame.rgba)?;
        std::fs::write(
            output.join("report.json"),
            serde_json::to_vec_pretty(
                &serde_json::json!({"schema_version":1,"fixture":"original-offscreen-v1","adapter":frame.adapter_name,"backend":frame.backend,"device_type":frame.device_type,"width":frame.width,"height":frame.height,"format":"rgba8unorm-linear","row_padding_removed":true,"interior_probes":"passed-tolerance-2","validation_errors":frame.validation_errors,"full_scene_parity":false}),
            )?,
        )?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&output);
    }
    result
}
