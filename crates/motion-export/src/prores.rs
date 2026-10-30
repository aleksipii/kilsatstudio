//! Apple ProRes delivery through a caller-selected local FFmpeg (`prores_ks`).
//!
//! The PNG frames and WAV are produced by the normal sequence export on the shared
//! 24 kHz clock; this module only encodes them. FFmpeg is never searched for on
//! PATH, downloaded or run through a shell.
use super::*;
use crate::color::ColorTag;
use std::{
    process::{Command, Stdio},
    time::Duration,
};

/// `prores_ks` profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProResProfile {
    /// ProRes 422 HQ, 10-bit 4:2:2, no alpha. Requires an even frame width.
    Hq,
    /// ProRes 4444, 10-bit 4:4:4 with a 16-bit alpha plane.
    P4444,
}

impl ProResProfile {
    fn profile_number(self) -> &'static str {
        match self {
            ProResProfile::Hq => "3",
            ProResProfile::P4444 => "4",
        }
    }
    fn pixel_format(self) -> &'static str {
        match self {
            ProResProfile::Hq => "yuv422p10le",
            ProResProfile::P4444 => "yuva444p10le",
        }
    }
    /// Name written to manifests.
    pub fn name(self) -> &'static str {
        match self {
            ProResProfile::Hq => "prores_422_hq",
            ProResProfile::P4444 => "prores_4444",
        }
    }
}

/// Explicit ProRes encode settings. ProRes files are large, so the movie has its
/// own byte cap instead of sharing the 256 MiB interchange-frame budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProResOptions {
    pub profile: ProResProfile,
    pub color: ColorTag,
    /// Hard cap for the `.mov`; FFmpeg stops at this size and the export fails.
    pub max_video_bytes: u64,
    /// Wall-clock limit for the FFmpeg process.
    pub timeout: Duration,
}

impl ProResOptions {
    pub const MIN_VIDEO_BYTES: u64 = 1024 * 1024;
    pub const MAX_VIDEO_BYTES: u64 = 16 * 1024 * 1024 * 1024;

    pub fn new(profile: ProResProfile) -> Self {
        Self {
            profile,
            color: ColorTag::default(),
            max_video_bytes: 2 * 1024 * 1024 * 1024,
            timeout: Duration::from_secs(600),
        }
    }
    fn validate(&self, width: u32, height: u32) -> Result<(), Error> {
        if !(Self::MIN_VIDEO_BYTES..=Self::MAX_VIDEO_BYTES).contains(&self.max_video_bytes) {
            return Err(invalid("ProRes video cap must be between 1 MiB and 16 GiB"));
        }
        if self.timeout < Duration::from_secs(1) || self.timeout > Duration::from_secs(4 * 3600) {
            return Err(invalid(
                "ProRes timeout must be between 1 second and 4 hours",
            ));
        }
        if width == 0 || height == 0 {
            return Err(invalid("ProRes requires a non-empty frame"));
        }
        // 4:2:2 halves horizontal chroma; reject rather than silently resize.
        if self.profile == ProResProfile::Hq && !width.is_multiple_of(2) {
            return Err(invalid("ProRes 422 HQ requires an even frame width"));
        }
        Ok(())
    }
}

/// The movie written by [`encode_prores_frames`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProResFrames<'a> {
    /// FFmpeg image-sequence pattern relative to the directory, e.g. `frame-%06d.png`.
    pub pattern: &'a str,
    pub start_number: u64,
    pub fps: u32,
    /// Optional WAV relative to the directory, encoded as 16-bit PCM.
    pub audio: Option<&'a str>,
    /// Output file name relative to the directory; must end in `.mov`.
    pub output: &'a str,
}

fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.contains(['/', '\\', '\0'])
        && name != "."
        && name != ".."
}

fn check_ffmpeg(ffmpeg: &Path) -> Result<(), Error> {
    if !ffmpeg.is_absolute() || !ffmpeg.is_file() {
        return Err(Error::EncoderUnavailable {
            path: ffmpeg.to_path_buf(),
        });
    }
    Ok(())
}

/// Encode an existing numbered PNG sequence (opaque or with alpha) in `directory`
/// to ProRes with explicit colour tags. Blocking; run it off the UI thread.
/// The output file must not exist; it is removed again if encoding fails.
pub fn encode_prores_frames(
    directory: &Path,
    frames: &ProResFrames<'_>,
    width: u32,
    height: u32,
    options: &ProResOptions,
    ffmpeg: &Path,
    cancel: &CancellationToken,
) -> Result<PathBuf, Error> {
    check(cancel)?;
    check_ffmpeg(ffmpeg)?;
    options.validate(width, height)?;
    if frames.fps == 0 || frames.fps > 240 {
        return Err(invalid("ProRes frame rate must be 1..=240 fps"));
    }
    if !plain_name(frames.pattern)
        || !plain_name(frames.output)
        || !frames.output.ends_with(".mov")
        || frames.audio.is_some_and(|a| !plain_name(a))
    {
        return Err(invalid(
            "ProRes frames, audio and .mov output must be plain names inside the export directory",
        ));
    }
    let directory = fs::canonicalize(directory)?;
    let output = directory.join(frames.output);
    if output.exists() {
        return Err(invalid("ProRes output already exists"));
    }
    let mut command = Command::new(ffmpeg);
    command
        .args([
            "-nostdin",
            "-v",
            "error",
            "-n",
            "-protocol_whitelist",
            "file,pipe",
            "-framerate",
        ])
        .arg(frames.fps.to_string())
        .arg("-start_number")
        .arg(frames.start_number.to_string())
        .arg("-i")
        .arg(directory.join(frames.pattern));
    if let Some(audio) = frames.audio {
        command.arg("-i").arg(directory.join(audio));
    }
    command.args(["-map", "0:v:0"]);
    if frames.audio.is_some() {
        command.args(["-map", "1:a:0", "-c:a", "pcm_s16le"]);
    }
    command
        .args([
            "-c:v",
            "prores_ks",
            "-profile:v",
            options.profile.profile_number(),
            "-vendor",
            "apl0",
            "-pix_fmt",
            options.profile.pixel_format(),
        ])
        .args(options.color.ffmpeg_args());
    if options.profile == ProResProfile::P4444 {
        command.args(["-alpha_bits", "16"]);
    }
    command
        .arg("-fs")
        .arg(options.max_video_bytes.to_string())
        .arg(&output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = command.spawn()?;
    let result = media::wait_encoder(child, cancel, options.timeout).and_then(|status| {
        check(cancel)?;
        if !status.success() {
            return Err(invalid(format!(
                "FFmpeg ProRes encoding failed ({status}); check that this FFmpeg build includes prores_ks"
            )));
        }
        let length = fs::metadata(&output)?.len();
        if length == 0 || length >= options.max_video_bytes {
            return Err(invalid(
                "ProRes output is empty or reached the configured video byte cap",
            ));
        }
        Ok(())
    });
    match result {
        Ok(()) => Ok(output),
        Err(error) => {
            if output.exists() {
                let _ = fs::remove_file(&output);
            }
            Err(error)
        }
    }
}

/// Export the project sequence as PNG/WAV (same frames as the MP4 path) and encode
/// `sequence.mov` with ProRes. `sequence.json` records codec and colour tags.
/// Creates `destination`; removes only its own directory on failure.
#[allow(clippy::too_many_arguments)]
pub fn export_prores_sequence(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    destination: &Path,
    ffmpeg: &Path,
    options: &ProResOptions,
    cancel: &CancellationToken,
    progress: impl FnMut(Progress),
    captions: CaptionMode,
) -> Result<Report, Error> {
    check(cancel)?;
    check_ffmpeg(ffmpeg)?;
    options.validate(script.project.width, script.project.height)?;
    let report = sequence::export_png_sequence_captions(
        script,
        rendered,
        art,
        destination,
        cancel,
        progress,
        captions,
    )?;
    let result = (|| {
        encode_prores_frames(
            destination,
            &ProResFrames {
                pattern: "frame-%06d.png",
                start_number: 0,
                fps: script.project.fps,
                audio: Some("audio.wav"),
                output: "sequence.mov",
            },
            script.project.width,
            script.project.height,
            options,
            ffmpeg,
            cancel,
        )?;
        let path = destination.join("sequence.json");
        let mut manifest: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
        manifest["video_encoded"] = true.into();
        manifest["video_file"] = "sequence.mov".into();
        manifest["video_codec"] = options.profile.name().into();
        manifest["audio_codec"] = "pcm_s16le".into();
        manifest["color"] = serde_json::json!({
            "primaries": "bt709",
            "transfer": options.color.transfer(),
            "matrix": "bt709",
            "range": "tv",
            "png": "sRGB+gAMA",
            "pipeline": "none (no ACES/OCIO)",
        });
        let bytes = bounded_json(&manifest, 8 * 1024 * 1024)?;
        fs::write(path, bytes)?;
        Ok(report)
    })();
    discard_partial(destination, result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_ffmpeg_and_invalid_options_fail_before_output() {
        let (mut script, rendered) = crate::tests::fixture();
        let dest = crate::tests::destination();
        let cancel = CancellationToken::new();
        let options = ProResOptions::new(ProResProfile::Hq);
        let missing = Path::new("/nonexistent/kilsat/ffmpeg");
        let error = export_prores_sequence(
            &script,
            std::slice::from_ref(&rendered),
            None,
            &dest,
            missing,
            &options,
            &cancel,
            |_| {},
            CaptionMode::Follow,
        )
        .unwrap_err();
        assert!(matches!(error, Error::EncoderUnavailable { .. }), "{error}");
        assert!(error.to_string().contains("FFmpeg"));
        assert!(!dest.exists());

        let relative = Path::new("ffmpeg");
        assert!(matches!(
            encode_prores_frames(
                Path::new("."),
                &ProResFrames {
                    pattern: "frame-%06d.png",
                    start_number: 0,
                    fps: 24,
                    audio: None,
                    output: "out.mov",
                },
                8,
                8,
                &options,
                relative,
                &cancel,
            ),
            Err(Error::EncoderUnavailable { .. })
        ));

        // A real file stands in for FFmpeg; option validation runs before spawning.
        let fake = std::env::current_exe().unwrap();
        script.project.width = 7;
        assert!(export_prores_sequence(
            &script,
            std::slice::from_ref(&rendered),
            None,
            &dest,
            &fake,
            &options,
            &cancel,
            |_| {},
            CaptionMode::Follow,
        )
        .unwrap_err()
        .to_string()
        .contains("even frame width"));
        let mut tiny = options.clone();
        tiny.max_video_bytes = 10;
        assert!(tiny.validate(8, 8).is_err());
        assert!(!dest.exists());
        cancel.cancel();
        assert!(matches!(
            export_prores_sequence(
                &script,
                std::slice::from_ref(&rendered),
                None,
                &dest,
                &fake,
                &ProResOptions::new(ProResProfile::P4444),
                &cancel,
                |_| {},
                CaptionMode::Follow,
            ),
            Err(Error::Cancelled)
        ));
        assert!(!dest.exists());
    }

    #[test]
    fn output_names_cannot_escape_the_export_directory() {
        let fake = std::env::current_exe().unwrap();
        let options = ProResOptions::new(ProResProfile::P4444);
        for (pattern, output, audio) in [
            ("../frame-%06d.png", "a.mov", None),
            ("frame-%06d.png", "../a.mov", None),
            ("frame-%06d.png", "a.mp4", None),
            ("frame-%06d.png", "a.mov", Some("/tmp/a.wav")),
        ] {
            let error = encode_prores_frames(
                Path::new("."),
                &ProResFrames {
                    pattern,
                    start_number: 0,
                    fps: 24,
                    audio,
                    output,
                },
                8,
                8,
                &options,
                &fake,
                &CancellationToken::new(),
            )
            .unwrap_err();
            assert!(error.to_string().contains("plain names"), "{error}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn cancellation_kills_encoder_and_removes_partial_movie() {
        use std::os::unix::fs::PermissionsExt;
        let directory = crate::tests::destination();
        fs::create_dir(&directory).unwrap();
        let encoder = crate::tests::destination();
        // Writes a partial movie (last argument), then spins until killed.
        fs::write(
            &encoder,
            b"#!/bin/sh\nfor last; do :; done\nprintf x > \"$last\"\nwhile :; do :; done\n",
        )
        .unwrap();
        fs::set_permissions(&encoder, fs::Permissions::from_mode(0o700)).unwrap();
        let token = CancellationToken::new();
        let result = std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(100));
                token.cancel();
            });
            encode_prores_frames(
                &directory,
                &ProResFrames {
                    pattern: "frame-%06d.png",
                    start_number: 0,
                    fps: 24,
                    audio: None,
                    output: "out.mov",
                },
                8,
                8,
                &ProResOptions::new(ProResProfile::Hq),
                &encoder,
                &token,
            )
        });
        assert!(matches!(result, Err(Error::Cancelled)));
        assert!(!directory.join("out.mov").exists());
        fs::remove_file(encoder).unwrap();
        fs::remove_dir_all(directory).unwrap();
    }

    fn real_ffmpeg() -> Option<PathBuf> {
        std::env::var_os("REAL_FFMPEG").map(PathBuf::from)
    }

    fn ffprobe_json(ffmpeg: &Path, movie: &Path) -> serde_json::Value {
        let ffprobe = ffmpeg.with_file_name("ffprobe");
        let out = Command::new(ffprobe)
            .args([
                "-v",
                "error",
                "-count_frames",
                "-show_entries",
                "stream=codec_name,profile,pix_fmt,width,height,nb_read_frames,color_primaries,color_transfer,color_space,color_range,sample_rate",
                "-of",
                "json",
            ])
            .arg(movie)
            .output()
            .unwrap();
        assert!(out.status.success());
        serde_json::from_slice(&out.stdout).unwrap()
    }

    #[test]
    #[ignore = "requires REAL_FFMPEG (absolute path; ffprobe next to it)"]
    fn real_prores_hq_sequence_is_tagged_and_frame_exact() {
        let ffmpeg = real_ffmpeg().expect("REAL_FFMPEG");
        let (script, rendered) = crate::otio::tests::two_scene_fixture(true);
        let dest = crate::tests::destination();
        let mut options = ProResOptions::new(ProResProfile::Hq);
        options.color = ColorTag::Rec709;
        let report = export_prores_sequence(
            &script,
            &rendered,
            None,
            &dest,
            &ffmpeg,
            &options,
            &CancellationToken::new(),
            |_| {},
            CaptionMode::Follow,
        )
        .unwrap();
        let probe = ffprobe_json(&ffmpeg, &dest.join("sequence.mov"));
        eprintln!("ffprobe ProRes HQ: {probe}");
        let video = &probe["streams"][0];
        assert_eq!(video["codec_name"], "prores");
        assert_eq!(video["profile"], "HQ");
        assert_eq!(video["pix_fmt"], "yuv422p10le");
        assert_eq!(video["color_primaries"], "bt709");
        assert_eq!(video["color_transfer"], "bt709");
        assert_eq!(video["color_space"], "bt709");
        assert_eq!(video["color_range"], "tv");
        assert_eq!(
            video["nb_read_frames"]
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap(),
            report.frame_count
        );
        assert_eq!(probe["streams"][1]["codec_name"], "pcm_s16le");
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(dest.join("sequence.json")).unwrap()).unwrap();
        assert_eq!(manifest["video_codec"], "prores_422_hq");
        assert_eq!(manifest["color"]["transfer"], "bt709");
        fs::remove_dir_all(dest).unwrap();
    }

    #[test]
    #[ignore = "requires REAL_FFMPEG (absolute path; ffprobe next to it)"]
    fn real_prores_4444_keeps_transparent_alpha() {
        let ffmpeg = real_ffmpeg().expect("REAL_FFMPEG");
        let directory = crate::tests::destination();
        fs::create_dir(&directory).unwrap();
        // Left half transparent, right half opaque red, for three frames.
        for frame in 0..3 {
            let mut pixmap = resvg::tiny_skia::Pixmap::new(16, 8).unwrap();
            for (index, pixel) in pixmap.pixels_mut().iter_mut().enumerate() {
                if index % 16 >= 8 {
                    *pixel = resvg::tiny_skia::ColorU8::from_rgba(255, 0, 0, 255).premultiply();
                }
            }
            let png = crate::color::tag_png_srgb(pixmap.encode_png().unwrap()).unwrap();
            fs::write(directory.join(format!("cut-{frame:04}.png")), png).unwrap();
        }
        let movie = encode_prores_frames(
            &directory,
            &ProResFrames {
                pattern: "cut-%04d.png",
                start_number: 0,
                fps: 24,
                audio: None,
                output: "cutout.mov",
            },
            16,
            8,
            &ProResOptions::new(ProResProfile::P4444),
            &ffmpeg,
            &CancellationToken::new(),
        )
        .unwrap();
        let probe = ffprobe_json(&ffmpeg, &movie);
        eprintln!("ffprobe ProRes 4444: {probe}");
        let video = &probe["streams"][0];
        assert_eq!(video["codec_name"], "prores");
        assert_eq!(video["profile"], "4444");
        // ProRes 4444 is a 12-bit format; the decoder reports its native depth.
        assert_eq!(video["pix_fmt"], "yuva444p12le");
        assert_eq!(video["color_transfer"], "iec61966-2-1");
        assert_eq!(video["nb_read_frames"], "3");
        let out = Command::new(&ffmpeg)
            .args(["-v", "error", "-i"])
            .arg(&movie)
            .args([
                "-frames:v",
                "1",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(out.status.success());
        let rgba = out.stdout;
        assert_eq!(rgba.len(), 16 * 8 * 4);
        let alpha = |x: usize| rgba[(3 * 16 + x) * 4 + 3];
        assert_eq!(alpha(2), 0, "transparent half keeps zero alpha");
        assert_eq!(alpha(13), 255, "opaque half keeps full alpha");
        let red = rgba[(3 * 16 + 13) * 4];
        assert!(
            red > 240,
            "opaque half stays red after Rec.709 round trip: {red}"
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
