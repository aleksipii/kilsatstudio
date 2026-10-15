use super::*;
use std::{process::{Command, Stdio}, time::{Duration, Instant}};

pub(super) fn rasterize(svg: &str) -> Result<Vec<u8>, Error> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
        .map_err(|e| invalid(format!("SVG rasterization: {e}")))?;
    let size = tree.size().to_int_size();
    if u64::from(size.width()) * u64::from(size.height()) > 16_000_000 {
        return Err(invalid("raster frame exceeds 16 million pixel budget"));
    }
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| invalid("unable to allocate raster frame"))?;
    resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pixmap.as_mut());
    pixmap.encode_png().map_err(|e| invalid(format!("PNG encoding: {e}")))
}

/// Encode retained PNG/WAV interchange files using a caller-selected local FFmpeg.
/// Never searches PATH, downloads an encoder, or invokes a shell. Runs off the UI thread.
pub fn export_mp4_scene(
    script: &Script, scene_index: usize, rendered: &RenderedScene,
    destination: &Path, ffmpeg: &Path, cancel: &CancellationToken,
    mut progress: impl FnMut(Progress),
) -> Result<Report, Error> {
    check(cancel)?;
    if !ffmpeg.is_absolute() || !ffmpeg.is_file() {
        return Err(invalid("FFmpeg must be an explicit absolute path to a local executable"));
    }
    // H.264 yuv420p needs even dimensions. Reject rather than silently resize poses.
    if script.project.width % 2 != 0 || script.project.height % 2 != 0 {
        return Err(invalid("MP4 requires even project width and height"));
    }
    let report = export_png_scene(script, scene_index, rendered, destination, cancel, &mut progress)?;
    let result = encode(script, destination, ffmpeg, cancel).map(|()| report);
    if result.is_err() {
        fs::remove_dir_all(destination)
            .map_err(|e| invalid(format!("video export failed; partial-output cleanup failed: {e}")))?;
    }
    result
}

fn encode(script: &Script, destination: &Path, ffmpeg: &Path, cancel: &CancellationToken) -> Result<(), Error> {
    check(cancel)?;
    let directory = fs::canonicalize(destination)?;
    let used = fs::read_dir(&directory)?.try_fold(0u64, |sum, entry| {
        let length = entry?.metadata()?.len();
        sum.checked_add(length).ok_or_else(|| std::io::Error::other("export byte overflow"))
    })?;
    let remaining = MAX_BYTES.saturating_sub(used);
    if remaining < 1024 { return Err(invalid("insufficient remaining video byte budget")); }
    // Closed stdio prevents prompts and unbounded stderr pipe accumulation.
    let mut child = Command::new(ffmpeg)
        .args(["-nostdin", "-v", "error", "-n", "-protocol_whitelist", "file,pipe", "-framerate"])
        .arg(script.project.fps.to_string())
        .args(["-start_number", "0", "-i"]).arg(directory.join("frame_%06d.png"))
        .arg("-i").arg(directory.join("scene.wav"))
        .args(["-map", "0:v:0", "-map", "1:a:0", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-movflags", "+faststart", "-fs"])
        .arg(remaining.to_string()).arg(directory.join("scene.mp4"))
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
    let started = Instant::now();
    let status = loop {
        let stop = if cancel.is_cancelled() { Some(Error::Cancelled) }
            else if started.elapsed() > Duration::from_secs(300) { Some(invalid("FFmpeg exceeded five minute timeout")) }
            else { None };
        if let Some(error) = stop { let _ = child.kill(); let _ = child.wait(); return Err(error); }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(error) => { let _ = child.kill(); let _ = child.wait(); return Err(error.into()); }
        }
    };
    check(cancel)?;
    if !status.success() { return Err(invalid(format!("FFmpeg encoding failed ({status}); check local H.264/AAC support"))); }
    let video = directory.join("scene.mp4");
    let length = fs::metadata(&video)?.len();
    if length == 0 || length >= remaining {
        return Err(invalid("FFmpeg output is empty or reached the video byte budget"));
    }
    let path = directory.join("manifest.json");
    let mut manifest: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
    manifest["video_encoded"] = true.into();
    manifest["video_file"] = "scene.mp4".into();
    manifest["video_codec"] = "h264".into();
    manifest["audio_codec"] = "aac".into();
    let bytes = bounded_json(&manifest, 8 * 1024 * 1024)?;
    if used + length + bytes.len() as u64 > MAX_BYTES { return Err(invalid("video export exceeds byte budget")); }
    fs::write(path, bytes)?;
    Ok(())
}
