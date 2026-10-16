use super::*;
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(super) fn rasterize(svg: &str) -> Result<Vec<u8>, Error> {
    rasterize_scaled(svg, None)
}
pub(super) fn rasterize_scaled(svg: &str, max_side: Option<u32>) -> Result<Vec<u8>, Error> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
        .map_err(|e| invalid(format!("SVG rasterization: {e}")))?;
    let size = tree.size().to_int_size();
    if u64::from(size.width()) * u64::from(size.height()) > 16_000_000 {
        return Err(invalid("raster frame exceeds 16 million pixel budget"));
    }
    let scale = max_side.map_or(1.0, |limit| {
        (limit as f32 / size.width().max(size.height()) as f32).min(1.0)
    });
    let width = (size.width() as f32 * scale).round().max(1.0) as u32;
    let height = (size.height() as f32 * scale).round().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| invalid("unable to allocate raster frame"))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(
            width as f32 / size.width() as f32,
            height as f32 / size.height() as f32,
        ),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|e| invalid(format!("PNG encoding: {e}")))
}

/// Encode retained PNG/WAV interchange files using a caller-selected local FFmpeg.
/// Never searches PATH, downloads an encoder, or invokes a shell. Runs off the UI thread.
pub fn export_mp4_scene(
    script: &Script,
    scene_index: usize,
    rendered: &RenderedScene,
    destination: &Path,
    ffmpeg: &Path,
    cancel: &CancellationToken,
    mut progress: impl FnMut(Progress),
) -> Result<Report, Error> {
    export_mp4_inner(
        script,
        scene_index,
        rendered,
        None,
        destination,
        ffmpeg,
        cancel,
        &mut progress,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn export_mp4_scene_with_art(
    script: &Script,
    scene_index: usize,
    rendered: &RenderedScene,
    art: &ArtContext,
    destination: &Path,
    ffmpeg: &Path,
    cancel: &CancellationToken,
    mut progress: impl FnMut(Progress),
) -> Result<Report, Error> {
    export_mp4_inner(
        script,
        scene_index,
        rendered,
        Some(art),
        destination,
        ffmpeg,
        cancel,
        &mut progress,
    )
}

#[allow(clippy::too_many_arguments)]
fn export_mp4_inner(
    script: &Script,
    scene_index: usize,
    rendered: &RenderedScene,
    art: Option<&ArtContext>,
    destination: &Path,
    ffmpeg: &Path,
    cancel: &CancellationToken,
    progress: &mut impl FnMut(Progress),
) -> Result<Report, Error> {
    check(cancel)?;
    validate_encoder(script, ffmpeg)?;
    let report = export_sequence(
        script,
        scene_index,
        rendered,
        destination,
        cancel,
        progress,
        true,
        art,
    )?;
    let result = encode(script, destination, ffmpeg, cancel).map(|()| report);
    if result.is_err() {
        fs::remove_dir_all(destination).map_err(|e| {
            invalid(format!(
                "video export failed; partial-output cleanup failed: {e}"
            ))
        })?;
    }
    result
}

pub(super) fn validate_encoder(script: &Script, ffmpeg: &Path) -> Result<(), Error> {
    if !ffmpeg.is_absolute() || !ffmpeg.is_file() {
        return Err(invalid(
            "FFmpeg must be an explicit absolute path to a local executable",
        ));
    }
    // H.264 yuv420p needs even dimensions. Reject rather than silently resize poses.
    if !script.project.width.is_multiple_of(2) || !script.project.height.is_multiple_of(2) {
        return Err(invalid("MP4 requires even project width and height"));
    }
    Ok(())
}
fn encode(
    script: &Script,
    destination: &Path,
    ffmpeg: &Path,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    encode_with_timeout(
        script,
        destination,
        ffmpeg,
        cancel,
        Duration::from_secs(300),
    )
}

fn encode_with_timeout(
    script: &Script,
    destination: &Path,
    ffmpeg: &Path,
    cancel: &CancellationToken,
    timeout: Duration,
) -> Result<(), Error> {
    encode_files(
        script,
        destination,
        ffmpeg,
        cancel,
        timeout,
        "frame_%06d.png",
        "scene.wav",
        "scene.mp4",
        "manifest.json",
    )
}
#[allow(clippy::too_many_arguments)]
pub(super) fn encode_named(
    script: &Script,
    destination: &Path,
    ffmpeg: &Path,
    cancel: &CancellationToken,
    frames: &str,
    audio: &str,
    video_name: &str,
    manifest_name: &str,
) -> Result<(), Error> {
    encode_files(
        script,
        destination,
        ffmpeg,
        cancel,
        Duration::from_secs(300),
        frames,
        audio,
        video_name,
        manifest_name,
    )
}
#[allow(clippy::too_many_arguments)]
fn encode_files(
    script: &Script,
    destination: &Path,
    ffmpeg: &Path,
    cancel: &CancellationToken,
    timeout: Duration,
    frames: &str,
    audio: &str,
    video_name: &str,
    manifest_name: &str,
) -> Result<(), Error> {
    check(cancel)?;
    let directory = fs::canonicalize(destination)?;
    let used = fs::read_dir(&directory)?.try_fold(0u64, |sum, entry| {
        let length = entry?.metadata()?.len();
        sum.checked_add(length)
            .ok_or_else(|| std::io::Error::other("export byte overflow"))
    })?;
    let remaining = MAX_BYTES.saturating_sub(used);
    if remaining < 1024 {
        return Err(invalid("insufficient remaining video byte budget"));
    }
    // Closed stdio prevents prompts and unbounded stderr pipe accumulation.
    let mut child = Command::new(ffmpeg)
        .args([
            "-nostdin",
            "-v",
            "error",
            "-n",
            "-protocol_whitelist",
            "file,pipe",
            "-framerate",
        ])
        .arg(script.project.fps.to_string())
        .args(["-start_number", "0", "-i"])
        .arg(directory.join(frames))
        .arg("-i")
        .arg(directory.join(audio))
        .args([
            "-map",
            "0:v:0",
            "-map",
            "1:a:0",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
            "-fs",
        ])
        .arg(remaining.to_string())
        .arg(directory.join(video_name))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let started = Instant::now();
    let status = loop {
        let stop = if cancel.is_cancelled() {
            Some(Error::Cancelled)
        } else if started.elapsed() > timeout {
            Some(invalid("FFmpeg exceeded five minute timeout"))
        } else {
            None
        };
        if let Some(error) = stop {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
    };
    check(cancel)?;
    if !status.success() {
        return Err(invalid(format!(
            "FFmpeg encoding failed ({status}); check local H.264/AAC support"
        )));
    }
    let video = directory.join(video_name);
    let length = fs::metadata(&video)?.len();
    if length == 0 || length >= remaining {
        return Err(invalid(
            "FFmpeg output is empty or reached the video byte budget",
        ));
    }
    let path = directory.join(manifest_name);
    let mut manifest: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
    manifest["video_encoded"] = true.into();
    manifest["video_file"] = video_name.into();
    manifest["video_codec"] = "h264".into();
    manifest["audio_codec"] = "aac".into();
    let bytes = bounded_json(&manifest, 8 * 1024 * 1024)?;
    if used + length + bytes.len() as u64 > MAX_BYTES {
        return Err(invalid("video export exceeds byte budget"));
    }
    fs::write(path, bytes)?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn encoder_timeout_and_cancellation_kill_and_reap_process() {
        let (script, rendered) = crate::tests::fixture();
        let destination = crate::tests::destination();
        export_png_scene(
            &script,
            0,
            &rendered,
            &destination,
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap();
        let encoder = crate::tests::destination();
        fs::write(&encoder, b"#!/bin/sh\nwhile :; do :; done\n").unwrap();
        fs::set_permissions(&encoder, fs::Permissions::from_mode(0o700)).unwrap();
        let start = Instant::now();
        assert!(encode_with_timeout(
            &script,
            &destination,
            &encoder,
            &CancellationToken::new(),
            Duration::from_millis(50)
        )
        .is_err());
        assert!(start.elapsed() < Duration::from_secs(5));
        let token = CancellationToken::new();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(50));
                token.cancel();
            });
            assert!(matches!(
                encode_with_timeout(
                    &script,
                    &destination,
                    &encoder,
                    &token,
                    Duration::from_secs(5)
                ),
                Err(Error::Cancelled)
            ));
        });
        fs::remove_file(encoder).unwrap();
        fs::remove_dir_all(destination).unwrap();
    }
}
