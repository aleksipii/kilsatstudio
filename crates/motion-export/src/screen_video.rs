//! Screen video decoding into a content-addressed frame cache.
//!
//! A `ScreenSource::Video` recording (MP4/MOV, for example an iOS simulator
//! capture from `xcrun simctl io booted recordVideo demo.mov`) is decoded by a
//! caller-selected FFmpeg into numbered sRGB PNG frames at the cue's frame
//! rate. Decoding is a blocking call meant for a background job, never the UI
//! thread. Preview decodes a reduced-width proxy; export decodes full size.
//! Both use the same frame numbering, so the scene clock picks the same frame
//! number in either.
//!
//! Colour: Display P3 tagged recordings (common from iPhones) are converted to
//! BT.709/sRGB primaries during decoding, so they do not look washed out next
//! to sRGB artwork. HDR (BT.2020/HLG/PQ) recordings are rejected rather than
//! shown with wrong colours; record SDR or tone-map them first.
use crate::{check, invalid, Error};
use motion_tts::CancellationToken;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Increment when decoding output changes, so stale cache entries are ignored.
pub const SCREEN_VIDEO_RECIPE: u32 = 1;
/// Largest accepted source file.
pub const MAX_SCREEN_VIDEO_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// Most frames decoded for one source.
pub const MAX_SCREEN_VIDEO_FRAMES: u64 = 20_000;
/// Widest decoded frame.
pub const MAX_SCREEN_VIDEO_WIDTH: u32 = 4096;
/// Tallest decoded frame.
pub const MAX_SCREEN_VIDEO_HEIGHT: u32 = 4096;
/// Most bytes one decoded cache entry may write; decoding stops beyond it.
pub const MAX_SCREEN_CACHE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// Width of preview proxies.
pub const SCREEN_PROXY_WIDTH: u32 = 540;
const MANIFEST: &str = "manifest.json";

/// Source colour as tagged in the recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenVideoColor {
    /// BT.709/sRGB primaries, or untagged (treated as BT.709).
    Bt709,
    /// Display P3 (SMPTE 432, D65) primaries: converted to BT.709.
    DisplayP3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScreenVideoRequest {
    pub source: PathBuf,
    pub fps: f64,
    /// Seconds to decode (the cue's `duration`).
    pub duration: f64,
    /// `Some(width)` decodes a proxy no wider than `width`; `None` full size.
    pub max_width: Option<u32>,
}

/// A decoded frame cache entry.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DecodedScreenVideo {
    #[serde(skip)]
    pub directory: PathBuf,
    pub frames: u64,
    pub color: ScreenVideoColor,
    pub source_bytes: u64,
    pub recipe: u32,
}
impl DecodedScreenVideo {
    /// Path of frame `frame`; frames past the end hold the last one.
    pub fn frame_path(&self, frame: u64) -> PathBuf {
        let frame = frame.min(self.frames.saturating_sub(1));
        self.directory.join(format!("{frame:06}.png"))
    }
    /// Decode frame `frame` as the image resource keyed by
    /// `motion_script::video_frame_asset(asset, frame)`.
    pub fn load_frame(
        &self,
        asset: &str,
        frame: u64,
    ) -> Result<(String, motion_art::ImageResource), Error> {
        let image = motion_art::load_image(&self.frame_path(frame))
            .map_err(|e| invalid(format!("screen video frame {frame} of {asset}: {e}")))?;
        Ok((motion_script::video_frame_asset(asset, frame), image))
    }
}

/// Width and height from a PNG header, without decoding the image.
pub fn png_size(path: &Path) -> Result<(u32, u32), Error> {
    let mut header = [0u8; 24];
    fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut header))
        .map_err(|e| invalid(format!("screen video frame {}: {e}", path.display())))?;
    if &header[..8] != b"\x89PNG\r\n\x1a\n" || &header[12..16] != b"IHDR" {
        return Err(invalid(format!(
            "screen video frame {} is not a PNG",
            path.display()
        )));
    }
    let width = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
    let height = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
    if width == 0
        || height == 0
        || width > MAX_SCREEN_VIDEO_WIDTH
        || height > MAX_SCREEN_VIDEO_HEIGHT
    {
        return Err(invalid(format!(
            "screen video frame {} has unsupported size {width}x{height}",
            path.display()
        )));
    }
    Ok((width, height))
}

fn validate(request: &ScreenVideoRequest) -> Result<u64, Error> {
    let frames = motion_script::ScreenSource::video_frames(request.fps, request.duration);
    if !request.fps.is_finite()
        || request.fps <= 0.0
        || request.fps > motion_script::MAX_SCREEN_FPS
        || !request.duration.is_finite()
        || request.duration <= 0.0
        || frames > MAX_SCREEN_VIDEO_FRAMES
        || request
            .max_width
            .is_some_and(|w| !(16..=MAX_SCREEN_VIDEO_WIDTH).contains(&w))
    {
        return Err(invalid(format!(
            "screen video needs fps within (0, {}], at most {MAX_SCREEN_VIDEO_FRAMES} frames and a proxy width within 16..={MAX_SCREEN_VIDEO_WIDTH}",
            motion_script::MAX_SCREEN_FPS
        )));
    }
    Ok(frames)
}

/// Cache key: FNV-1a over the recipe, the source bytes and every setting that
/// changes the decoded frames.
pub fn cache_key(request: &ScreenVideoRequest) -> Result<(String, u64), Error> {
    let mut file = fs::File::open(&request.source)?;
    let length = file.metadata()?.len();
    if length == 0 || length > MAX_SCREEN_VIDEO_BYTES {
        return Err(invalid("screen video source is empty or larger than 4 GiB"));
    }
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    feed(&SCREEN_VIDEO_RECIPE.to_le_bytes());
    feed(&request.fps.to_bits().to_le_bytes());
    feed(&request.duration.to_bits().to_le_bytes());
    feed(&request.max_width.unwrap_or(0).to_le_bytes());
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        feed(&buffer[..read]);
    }
    Ok((format!("{hash:016x}"), length))
}

/// Read the recording's colour tags with `ffprobe`.
pub fn probe_color(
    ffprobe: &Path,
    source: &Path,
    timeout: Duration,
) -> Result<ScreenVideoColor, Error> {
    executable(ffprobe)?;
    let output = run(
        Command::new(ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=color_primaries,color_transfer",
                "-of",
                "json",
            ])
            .arg(source),
        None,
        None,
        timeout,
        true,
    )?;
    let value: serde_json::Value = serde_json::from_slice(&output)?;
    let stream = &value["streams"][0];
    color_from_tags(
        stream["color_primaries"].as_str(),
        stream["color_transfer"].as_str(),
    )
}

/// Map ffprobe colour tags to a supported source colour.
pub fn color_from_tags(
    primaries: Option<&str>,
    transfer: Option<&str>,
) -> Result<ScreenVideoColor, Error> {
    if matches!(transfer, Some("arib-std-b67" | "smpte2084")) || matches!(primaries, Some("bt2020"))
    {
        return Err(Error::Invalid(
            "HDR screen recordings (BT.2020, HLG or PQ) are not supported; record SDR or tone-map to BT.709 first".into(),
        ));
    }
    Ok(match primaries {
        Some("smpte432") => ScreenVideoColor::DisplayP3,
        // DCI-P3 has another white point and transfer; converting it as
        // Display P3 would shift colours.
        Some("smpte431") => {
            return Err(Error::Invalid(
                "DCI-P3 (SMPTE 431) screen recordings are not supported; convert to Display P3 or BT.709 first".into(),
            ))
        }
        _ => ScreenVideoColor::Bt709,
    })
}

/// FFmpeg arguments decoding `request` into `output/%06d.png`.
pub fn decode_args(
    request: &ScreenVideoRequest,
    color: ScreenVideoColor,
    frames: u64,
    output: &Path,
) -> Vec<OsString> {
    let mut filters = Vec::new();
    if color == ScreenVideoColor::DisplayP3 {
        // Convert P3 primaries to BT.709 before scaling or RGB conversion.
        filters.push(
            "colorspace=all=bt709:iprimaries=smpte432:itrc=bt709:ispace=bt709:fast=0".to_owned(),
        );
    }
    filters.push(format!("fps={}", request.fps));
    // Always bound both dimensions (aspect kept, even sizes for chroma).
    let width = request.max_width.unwrap_or(MAX_SCREEN_VIDEO_WIDTH);
    filters.push(format!(
        "scale='min({width},iw)':'min({MAX_SCREEN_VIDEO_HEIGHT},ih)':force_original_aspect_ratio=decrease:force_divisible_by=2:flags=lanczos"
    ));
    filters.push("format=rgb24".into());
    let mut args: Vec<OsString> = [
        "-nostdin",
        "-v",
        "error",
        "-n",
        "-protocol_whitelist",
        "file",
        "-i",
    ]
    .iter()
    .map(Into::into)
    .collect();
    args.push(request.source.clone().into());
    args.extend(
        [
            "-map".to_owned(),
            "0:v:0".to_owned(),
            "-an".to_owned(),
            "-vf".to_owned(),
            filters.join(","),
            "-frames:v".to_owned(),
            frames.to_string(),
            "-start_number".to_owned(),
            "0".to_owned(),
        ]
        .map(OsString::from),
    );
    args.push(output.join("%06d.png").into());
    args
}

/// Decode `request` into `cache_root/<key>`, reusing a complete entry.
/// Blocking, cancellable and bounded by `timeout`; run it off the UI thread.
pub fn decode_screen_video(
    ffmpeg: &Path,
    color: ScreenVideoColor,
    request: &ScreenVideoRequest,
    cache_root: &Path,
    cancel: &CancellationToken,
    timeout: Duration,
) -> Result<DecodedScreenVideo, Error> {
    executable(ffmpeg)?;
    let expected = validate(request)?;
    check(cancel)?;
    let (key, source_bytes) = cache_key(request)?;
    let directory = cache_root.join(&key);
    if let Some(decoded) = cached(&directory, source_bytes) {
        return Ok(decoded);
    }
    fs::create_dir_all(cache_root)?;
    // Decode beside the entry and rename, so readers never see partial caches.
    let partial = cache_root.join(format!("{key}.partial"));
    if partial.exists() {
        fs::remove_dir_all(&partial)?;
    }
    fs::create_dir(&partial)?;
    let result = (|| {
        run(
            Command::new(ffmpeg).args(decode_args(request, color, expected, &partial)),
            Some(cancel),
            Some((&partial, MAX_SCREEN_CACHE_BYTES)),
            timeout,
            false,
        )?;
        let frames = (0..)
            .take_while(|i| partial.join(format!("{i:06}.png")).is_file())
            .count() as u64;
        if frames == 0 {
            return Err(invalid("FFmpeg decoded no screen video frames"));
        }
        let decoded = DecodedScreenVideo {
            directory: directory.clone(),
            frames,
            color,
            source_bytes,
            recipe: SCREEN_VIDEO_RECIPE,
        };
        fs::write(partial.join(MANIFEST), serde_json::to_vec(&decoded)?)?;
        if directory.exists() {
            fs::remove_dir_all(&directory)?;
        }
        fs::rename(&partial, &directory)?;
        Ok(decoded)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&partial);
    }
    result
}

/// A complete cache entry, or `None` when missing, stale or corrupt.
fn cached(directory: &Path, source_bytes: u64) -> Option<DecodedScreenVideo> {
    let bytes = fs::read(directory.join(MANIFEST)).ok()?;
    let mut decoded: DecodedScreenVideo = serde_json::from_slice(&bytes).ok()?;
    decoded.directory = directory.to_path_buf();
    let complete = decoded.frames > 0
        && decoded.recipe == SCREEN_VIDEO_RECIPE
        && decoded.source_bytes == source_bytes
        && (0..decoded.frames).all(|i| decoded.frame_path(i).is_file());
    complete.then_some(decoded)
}

fn executable(path: &Path) -> Result<(), Error> {
    if !path.is_absolute() || !path.is_file() {
        return Err(Error::EncoderUnavailable {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

/// Bytes of the regular files directly inside `directory`.
fn directory_bytes(directory: &Path) -> u64 {
    fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .fold(0, u64::saturating_add)
}

/// Run a child with closed stdin, killing it on cancel, timeout or when
/// `budget` (a directory and its byte limit) is exceeded. Returns stdout when
/// `capture` is set (bounded to 1 MiB).
fn run(
    command: &mut Command,
    cancel: Option<&CancellationToken>,
    budget: Option<(&Path, u64)>,
    timeout: Duration,
    capture: bool,
) -> Result<Vec<u8>, Error> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(if capture {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stderr(Stdio::null())
        .spawn()?;
    let reader = child.stdout.take().map(|stdout| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout.take(1024 * 1024).read_to_end(&mut bytes);
            bytes
        })
    });
    let started = Instant::now();
    let mut polls = 0u32;
    let status = loop {
        polls = polls.wrapping_add(1);
        let over_budget =
            || budget.is_some_and(|(directory, limit)| directory_bytes(directory) > limit);
        let stop = if cancel.is_some_and(CancellationToken::is_cancelled) {
            Some(Error::Cancelled)
        } else if started.elapsed() > timeout {
            Some(invalid("screen video tool exceeded its timeout"))
        } else if polls.is_multiple_of(10) && over_budget() {
            Some(invalid(
                "screen video decode exceeded its 4 GiB cache budget",
            ))
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
    let output = reader.and_then(|r| r.join().ok()).unwrap_or_default();
    if budget.is_some_and(|(directory, limit)| directory_bytes(directory) > limit) {
        return Err(invalid(
            "screen video decode exceeded its 4 GiB cache budget",
        ));
    }
    if !status.success() {
        return Err(invalid(format!("screen video tool failed ({status})")));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(dir: &Path) -> ScreenVideoRequest {
        let source = dir.join("demo.mov");
        fs::write(&source, b"not really a movie").unwrap();
        ScreenVideoRequest {
            source,
            fps: 30.0,
            duration: 2.0,
            max_width: Some(SCREEN_PROXY_WIDTH),
        }
    }
    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kilsat-screen-video-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn cache_key_tracks_bytes_and_settings() {
        let dir = temp("key");
        let base = request(&dir);
        let (key, length) = cache_key(&base).unwrap();
        assert_eq!(length, 18);
        assert_eq!(cache_key(&base).unwrap().0, key, "stable");
        let full = ScreenVideoRequest {
            max_width: None,
            ..base.clone()
        };
        assert_ne!(cache_key(&full).unwrap().0, key, "proxy and full differ");
        let faster = ScreenVideoRequest {
            fps: 60.0,
            ..base.clone()
        };
        assert_ne!(cache_key(&faster).unwrap().0, key);
        fs::write(&base.source, b"another recording!").unwrap();
        assert_ne!(cache_key(&base).unwrap().0, key, "content addressed");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn colour_tags_convert_p3_and_reject_hdr() {
        assert_eq!(
            color_from_tags(None, None).unwrap(),
            ScreenVideoColor::Bt709
        );
        assert_eq!(
            color_from_tags(Some("smpte432"), Some("bt709")).unwrap(),
            ScreenVideoColor::DisplayP3
        );
        assert!(color_from_tags(Some("bt2020"), Some("arib-std-b67")).is_err());
        assert!(color_from_tags(Some("bt709"), Some("smpte2084")).is_err());
        assert!(
            color_from_tags(Some("smpte431"), Some("bt709")).is_err(),
            "DCI-P3"
        );
    }

    #[test]
    fn decode_arguments_convert_colour_scale_proxies_and_bound_frames() {
        let dir = temp("args");
        let r = request(&dir);
        let args: Vec<String> = decode_args(&r, ScreenVideoColor::DisplayP3, 60, &dir)
            .into_iter()
            .map(|a| a.into_string().unwrap())
            .collect();
        let filters = &args[args.iter().position(|a| a == "-vf").unwrap() + 1];
        assert!(filters.starts_with("colorspace=all=bt709:iprimaries=smpte432"));
        assert!(filters.contains("fps=30") && filters.contains("min(540,iw)"));
        assert!(filters.ends_with("format=rgb24"));
        assert_eq!(
            args[args.iter().position(|a| a == "-frames:v").unwrap() + 1],
            "60"
        );
        let full = ScreenVideoRequest {
            max_width: None,
            ..r.clone()
        };
        let args: Vec<String> = decode_args(&full, ScreenVideoColor::Bt709, 60, &dir)
            .into_iter()
            .map(|a| a.into_string().unwrap())
            .collect();
        assert!(!args.iter().any(|a| a.contains("colorspace")));
        assert!(args
            .iter()
            .any(|a| a.contains("min(4096,iw)") && a.contains("min(4096,ih)")));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn invalid_requests_and_missing_tools_fail_before_decoding() {
        let dir = temp("invalid");
        let mut r = request(&dir);
        let cancel = CancellationToken::default();
        let relative = Path::new("ffmpeg");
        assert!(matches!(
            decode_screen_video(
                relative,
                ScreenVideoColor::Bt709,
                &r,
                &dir,
                &cancel,
                Duration::from_secs(1)
            ),
            Err(Error::EncoderUnavailable { .. })
        ));
        r.fps = 0.0;
        assert!(validate(&r).is_err());
        r.fps = 30.0;
        r.max_width = Some(8);
        assert!(validate(&r).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn decoding_stops_when_the_cache_budget_is_exceeded() {
        let dir = temp("budget");
        fs::write(dir.join("000000.png"), vec![0u8; 2048]).unwrap();
        let error = run(
            Command::new("/bin/sleep").arg("5"),
            None,
            Some((&dir, 1024)),
            Duration::from_secs(10),
            false,
        )
        .unwrap_err();
        assert!(error.to_string().contains("cache budget"), "{error}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn stale_or_incomplete_cache_entries_are_ignored() {
        let dir = temp("cache");
        let entry = dir.join("abc");
        fs::create_dir_all(&entry).unwrap();
        let decoded = DecodedScreenVideo {
            directory: entry.clone(),
            frames: 2,
            color: ScreenVideoColor::Bt709,
            source_bytes: 10,
            recipe: SCREEN_VIDEO_RECIPE,
        };
        fs::write(entry.join(MANIFEST), serde_json::to_vec(&decoded).unwrap()).unwrap();
        fs::write(entry.join("000000.png"), b"x").unwrap();
        assert!(cached(&entry, 10).is_none(), "frame 1 missing");
        fs::write(entry.join("000001.png"), b"x").unwrap();
        assert_eq!(cached(&entry, 10).unwrap().frames, 2);
        assert!(cached(&entry, 11).is_none(), "source changed size");
        // Frames past the end hold the last frame.
        assert!(decoded.frame_path(99).ends_with("000001.png"));
        let _ = fs::remove_dir_all(dir);
    }

    /// Needs a real FFmpeg/ffprobe: `KILSAT_TEST_FFMPEG=/abs/ffmpeg
    /// KILSAT_TEST_FFPROBE=/abs/ffprobe cargo test -p motion-export
    /// real_ffmpeg_screen_video -- --ignored`.
    #[test]
    #[ignore]
    fn real_ffmpeg_screen_video_decodes_proxy_and_converts_p3() {
        let ffmpeg =
            PathBuf::from(std::env::var("KILSAT_TEST_FFMPEG").expect("KILSAT_TEST_FFMPEG"));
        let ffprobe =
            PathBuf::from(std::env::var("KILSAT_TEST_FFPROBE").expect("KILSAT_TEST_FFPROBE"));
        let dir = temp("real");
        let make = |name: &str, primaries: &str| {
            let path = dir.join(name);
            let status = Command::new(&ffmpeg)
                .args(["-nostdin", "-v", "error", "-y", "-f", "lavfi", "-i"])
                .arg("color=c=0xC86464:s=320x640:r=30:d=1")
                // Tag the frames themselves; recent FFmpeg takes encoder
                // colour metadata from the frames, not only from flags.
                .arg("-vf")
                .arg(format!(
                    "setparams=color_primaries={primaries}:color_trc=bt709:colorspace=bt709:range=tv"
                ))
                .args(["-c:v", "libx264", "-pix_fmt", "yuv444p", "-qp", "0"])
                .args([
                    "-color_primaries",
                    primaries,
                    "-color_trc",
                    "bt709",
                    "-colorspace",
                    "bt709",
                    "-movflags",
                    "+write_colr",
                ])
                // Write the primaries into the H.264 VUI itself (12 = SMPTE
                // 432 / Display P3, 1 = BT.709): encoder flags alone are not
                // reliably carried into the stream.
                .args(["-bsf:v"])
                .arg(format!(
                    "h264_metadata=colour_primaries={}:transfer_characteristics=1:matrix_coefficients=1",
                    if primaries == "smpte432" { 12 } else { 1 }
                ))
                .arg(&path)
                .status()
                .unwrap();
            assert!(status.success());
            path
        };
        let red_of = |source: PathBuf, max_width| {
            // Show the raw tags in the log, so a detection failure is diagnosable.
            let raw = Command::new(&ffprobe)
                .args(["-v", "error", "-select_streams", "v:0", "-show_entries"])
                .arg("stream=codec_name,color_primaries,color_transfer,color_space,color_range")
                .args(["-of", "json"])
                .arg(&source)
                .output()
                .unwrap();
            eprintln!(
                "{}: {}",
                source.display(),
                String::from_utf8_lossy(&raw.stdout)
            );
            let color = probe_color(&ffprobe, &source, Duration::from_secs(30)).unwrap();
            let r = ScreenVideoRequest {
                source,
                fps: 30.0,
                duration: 1.0,
                max_width,
            };
            let decoded = decode_screen_video(
                &ffmpeg,
                color,
                &r,
                &dir.join("cache"),
                &CancellationToken::default(),
                Duration::from_secs(120),
            )
            .unwrap();
            assert_eq!(decoded.frames, 30);
            let (id, image) = decoded.load_frame("demo.mov", 3).unwrap();
            assert_eq!(id, "demo.mov#frame=3");
            // Reuse: a second decode returns the cached entry.
            let again = decode_screen_video(
                &ffmpeg,
                color,
                &r,
                &dir.join("cache"),
                &CancellationToken::default(),
                Duration::from_secs(120),
            )
            .unwrap();
            assert_eq!(again, decoded);
            (color, image.width(), image.rgba()[0])
        };
        let (color, width, srgb_red) = red_of(make("srgb.mov", "bt709"), None);
        assert_eq!((color, width), (ScreenVideoColor::Bt709, 320));
        let (color, width, p3_red) = red_of(make("p3.mov", "smpte432"), Some(160));
        assert_eq!((color, width), (ScreenVideoColor::DisplayP3, 160));
        // The same code values mean a more saturated red in P3: in sRGB the
        // red channel must rise.
        assert!(
            p3_red > srgb_red + 5,
            "P3 red {p3_red} should exceed the sRGB red {srgb_red}"
        );
        let _ = fs::remove_dir_all(dir);
    }
}
