use super::*;
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// SVG parse options with a font database holding only the bundled caption font.
/// System fonts are never scanned, so text output is identical on every machine.
/// All generic families resolve to the bundled font; the actor label text is
/// `display="none"` and is not drawn.
pub(crate) fn options() -> &'static resvg::usvg::Options<'static> {
    static OPTIONS: std::sync::OnceLock<resvg::usvg::Options<'static>> = std::sync::OnceLock::new();
    OPTIONS.get_or_init(|| {
        let mut database = resvg::usvg::fontdb::Database::new();
        database.load_font_data(motion_render::caption_font_data().to_vec());
        let family = motion_render::CAPTION_FONT_FAMILY;
        database.set_sans_serif_family(family);
        database.set_serif_family(family);
        database.set_cursive_family(family);
        database.set_fantasy_family(family);
        database.set_monospace_family(family);
        resvg::usvg::Options {
            font_family: family.to_owned(),
            fontdb: std::sync::Arc::new(database),
            ..Default::default()
        }
    })
}

pub(super) fn rasterize(svg: &str) -> Result<Vec<u8>, Error> {
    rasterize_scaled(svg, None)
}
pub(super) fn rasterize_scaled(svg: &str, max_side: Option<u32>) -> Result<Vec<u8>, Error> {
    let png = rasterize_scaled_pixmap(svg, max_side)?
        .encode_png()
        .map_err(|e| invalid(format!("PNG encoding: {e}")))?;
    crate::color::tag_png_srgb(png).map_err(invalid)
}
pub(super) fn rasterize_scaled_pixmap(
    svg: &str,
    max_side: Option<u32>,
) -> Result<resvg::tiny_skia::Pixmap, Error> {
    let tree = resvg::usvg::Tree::from_str(svg, options())
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
    Ok(pixmap)
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
        CaptionMode::Follow,
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
    progress: impl FnMut(Progress),
) -> Result<Report, Error> {
    export_mp4_scene_with_art_captions(
        script,
        scene_index,
        rendered,
        art,
        destination,
        ffmpeg,
        cancel,
        progress,
        CaptionMode::Follow,
    )
}

/// As [`export_mp4_scene_with_art`] with a user-level caption choice.
#[allow(clippy::too_many_arguments)]
pub fn export_mp4_scene_with_art_captions(
    script: &Script,
    scene_index: usize,
    rendered: &RenderedScene,
    art: &ArtContext,
    destination: &Path,
    ffmpeg: &Path,
    cancel: &CancellationToken,
    mut progress: impl FnMut(Progress),
    captions: CaptionMode,
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
        captions,
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
    captions: CaptionMode,
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
        captions,
    )?;
    let result = encode(script, destination, ffmpeg, cancel).map(|()| report);
    discard_partial(destination, result)
}

pub(super) fn validate_encoder(script: &Script, ffmpeg: &Path) -> Result<(), Error> {
    if !ffmpeg.is_absolute() || !ffmpeg.is_file() {
        return Err(Error::EncoderUnavailable {
            path: ffmpeg.to_path_buf(),
        });
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
    let used = directory_bytes(&directory)?;
    let remaining = MAX_BYTES.saturating_sub(used);
    if remaining < 1024 {
        return Err(invalid("insufficient remaining video byte budget"));
    }
    // Closed stdio prevents prompts and unbounded stderr pipe accumulation.
    let child = Command::new(ffmpeg)
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
    let status = wait_encoder(child, cancel, timeout)?;
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

/// Wait for a spawned encoder, killing and reaping it on cancellation or timeout.
pub(super) fn wait_encoder(
    mut child: std::process::Child,
    cancel: &CancellationToken,
    timeout: Duration,
) -> Result<std::process::ExitStatus, Error> {
    let started = Instant::now();
    loop {
        let stop = if cancel.is_cancelled() {
            Some(Error::Cancelled)
        } else if started.elapsed() > timeout {
            Some(invalid(format!(
                "FFmpeg exceeded {} second timeout",
                timeout.as_secs()
            )))
        } else {
            None
        };
        if let Some(error) = stop {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
    }
}

/// Bytes already used in an export directory, for the shared output budget.
pub(super) fn directory_bytes(directory: &Path) -> Result<u64, Error> {
    Ok(fs::read_dir(directory)?.try_fold(0u64, |sum, entry| {
        let length = entry?.metadata()?.len();
        sum.checked_add(length)
            .ok_or_else(|| std::io::Error::other("export byte overflow"))
    })?)
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
