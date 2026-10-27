//! A10 export checks on the G2 minimal scene (`examples/g2-minimal-scene`, A08):
//! frame count, audio length against video length, the frame clock, regression
//! hashes for the whole exported sequence, and failed exports (missing FFmpeg,
//! cancellation, I/O failure, cleanup failure, full disk).
//!
//! Audio is prepared on the strict model-free path (`tests/support/g2.rs`).
#[path = "support/g2.rs"]
mod g2;

use motion_audio::clock::{frame_to_sample, sample_to_seconds, DIALOGUE_SAMPLE_RATE};
use motion_export::{export_png_scene_with_art, export_scene, export_scene_with_art, Error};
use motion_tts::CancellationToken;
use motion_voice::export::{RenderedScene, SceneManifest};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Regression hashes for the G2 SVG export at 1920x1080 (`sequence_digest` over the
/// 96 per-frame SHA-256 values). SVG text is platform independent; CI checks it on
/// macOS and Ubuntu. Recorded in `docs/evidence/g2-export/README.md`.
const G2_SVG_SEQUENCE_SHA256: &str =
    "584232d5dc3afc85d30d2247b29587be2a554e38d3cccf62d6e999d055a67e91";
/// SVG frames 24 (wide) and 72 (close-up), the A08 reference frames.
const G2_SVG_F024_SHA256: &str = "e9d24dd4950a2537020c0975f83090e66ee6a517edd578dce310eed510404915";
const G2_SVG_F072_SHA256: &str = "5a531245112def209e3e3fad76379ce84702f03a6cc120d26844d6bc1db786a1";
/// PNG frames from resvg/tiny-skia. Equal to the A08 export hashes in
/// `docs/evidence/g2-render/README.md`. Asserted on Apple Silicon macOS only:
/// floating-point rasterization is not claimed to be identical on other targets.
const G2_PNG_WIDE_SHA256: &str = "85e13144c14393c48e5b78e4e7370d49aefbd66ebbb54bb060f51335814ed45c";
const G2_PNG_CLOSE_SHA256: &str =
    "d6d4c9def910e060b46edfd71e76ed6656cead1d4b35902acc3b9263e390dc1c";

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A fresh, not yet existing output path (exports create their own directory).
fn destination(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "kilsat-a10-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn read_manifest(dir: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap()
}

fn frame_hashes(dir: &Path, manifest: &serde_json::Value) -> Vec<String> {
    manifest["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| g2::sha256_hex(&fs::read(dir.join(f["file"].as_str().unwrap())).unwrap()))
        .collect()
}

/// Agreed audio/video tolerance: the audio track is `round(duration * 24000)`
/// samples, and it differs from the video length (`frame_count` frames on the
/// sample clock) by less than one frame period.
fn assert_audio_matches_video(audio_samples: u64, frame_count: u64, fps: u32) {
    let video_samples = frame_to_sample(frame_count, fps).unwrap();
    let frame_period = u64::from(DIALOGUE_SAMPLE_RATE).div_ceil(u64::from(fps));
    assert!(
        audio_samples.abs_diff(video_samples) < frame_period,
        "audio {audio_samples} samples vs video {video_samples} samples at {fps} fps"
    );
}

#[test]
fn g2_export_frame_count_audio_length_clock_and_regression_hashes() {
    let scene = g2::load(&g2::scene_path()).unwrap();
    assert_eq!(scene.script.scenes[0].id, g2::SCENE_ID);
    let fps = scene.script.project.fps;
    let duration = scene.script.scenes[0].duration;
    let dir = destination("g2-svg");
    let mut progress = Vec::new();
    let report = export_scene_with_art(
        &scene.script,
        0,
        &scene.rendered,
        &scene.art,
        &dir,
        &CancellationToken::new(),
        |p| progress.push(p),
    )
    .unwrap();

    // Frame count.
    let expected = motion_scene::frame_count(duration, fps, motion_export::MAX_FRAMES).unwrap();
    assert_eq!((expected, fps, duration), (96, 24, 4.0));
    assert_eq!(report.frame_count, expected);
    let manifest = read_manifest(&dir);
    assert_eq!(manifest["frame_count"], expected);
    assert_eq!(
        manifest["frames"].as_array().unwrap().len() as u64,
        expected
    );
    assert_eq!(progress.last().unwrap().completed_frames, expected);

    // Every frame is evaluated at t = floor(frame * 24000 / fps) / 24000; at 24 fps
    // that is bit-identical to frame / fps.
    for (i, frame) in manifest["frames"].as_array().unwrap().iter().enumerate() {
        let i = i as u64;
        let sample = frame_to_sample(i, fps).unwrap();
        assert_eq!(frame["frame"], i);
        assert_eq!(frame["sample_frame"], sample);
        assert_eq!(sample_to_seconds(sample), i as f64 / f64::from(fps));
        // serde_json's default float parser is not round-trip exact, so the JSON
        // value is compared with a tolerance; the exact time is checked above and
        // in `export_preview_parity_tests`.
        let written = frame["time_seconds"].as_f64().unwrap();
        assert!(
            (written - sample_to_seconds(sample)).abs() < 1e-12,
            "frame {i}"
        );
    }

    // Audio length against video length.
    let wav = motion_audio::read_wav(dir.join("scene.wav")).unwrap();
    assert_eq!((wav.sample_rate, wav.channels), (DIALOGUE_SAMPLE_RATE, 1));
    assert_eq!(wav.frames() as u64, 96_000);
    assert_eq!(manifest["sample_rate"], DIALOGUE_SAMPLE_RATE);
    assert_audio_matches_video(wav.frames() as u64, expected, fps);
    assert_eq!(frame_to_sample(expected, fps).unwrap(), wav.frames() as u64);

    // Regression: every exported frame.
    let hashes = frame_hashes(&dir, &manifest);
    assert_eq!(hashes[24], G2_SVG_F024_SHA256);
    assert_eq!(hashes[72], G2_SVG_F072_SHA256);
    assert_eq!(g2::sequence_digest(&hashes), G2_SVG_SEQUENCE_SHA256);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn g2_png_export_matches_recorded_reference_frames() {
    let scene = g2::load(&g2::scene_path()).unwrap();
    let dir = destination("g2-png");
    // Frames 24 and 72 are the A08 reference frames; export only up to frame 72
    // would change the scene, so export the whole scene and read the two files.
    export_png_scene_with_art(
        &scene.script,
        0,
        &scene.rendered,
        &scene.art,
        &dir,
        &CancellationToken::new(),
        |_| {},
    )
    .unwrap();
    for (frame, expected) in [(24, G2_PNG_WIDE_SHA256), (72, G2_PNG_CLOSE_SHA256)] {
        let png = fs::read(dir.join(format!("frame_{frame:06}.png"))).unwrap();
        // PNG signature and IHDR size: 1920x1080 on every platform.
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 1920);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 1080);
        let actual = g2::sha256_hex(&png);
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            assert_eq!(actual, expected, "frame {frame}");
        } else if actual != expected {
            eprintln!("note: PNG frame {frame} differs from the Apple Silicon reference: {actual}");
        }
    }
    fs::remove_dir_all(dir).unwrap();
}

fn tiny_scene(duration: f64, fps: u32) -> (motion_script::Script, RenderedScene) {
    let script = motion_script::parse(&format!(
        "project: {{fps: {fps}, width: 64, height: 36}}\ncharacters:\n  EMMA: {{asset: characters/emma}}\nscenes:\n  - id: s\n    duration: {duration}\n    events: []\n"
    ))
    .unwrap();
    let samples = (duration * f64::from(DIALOGUE_SAMPLE_RATE)).round() as u64;
    let rendered = RenderedScene {
        audio: motion_audio::AudioBuffer {
            sample_rate: DIALOGUE_SAMPLE_RATE,
            channels: 1,
            samples: vec![0.0; samples as usize],
        },
        manifest: SceneManifest {
            schema_version: 1,
            scene_id: "s".into(),
            sample_rate: DIALOGUE_SAMPLE_RATE,
            channels: 1,
            total_frames: samples,
            dialogue: vec![],
            cues: vec![],
        },
    };
    (script, rendered)
}

#[test]
fn frame_count_and_audio_length_hold_for_divisor_and_non_divisor_frame_rates() {
    for fps in [7, 12, 24, 25, 30, 60] {
        for duration in [0.04, 0.5, 1.0 / 3.0, 1.01, 2.5] {
            let (script, rendered) = tiny_scene(duration, fps);
            let dir = destination("grid");
            let report = export_scene(
                &script,
                0,
                &rendered,
                &dir,
                &CancellationToken::new(),
                |_| {},
            )
            .unwrap_or_else(|e| panic!("{duration} s at {fps} fps: {e}"));
            let count =
                motion_scene::frame_count(duration, fps, motion_export::MAX_FRAMES).unwrap();
            assert_eq!(report.frame_count, count, "{duration} s at {fps} fps");
            // The last frame starts inside the scene; one more frame would not.
            let last = (count - 1) as f64 / f64::from(fps);
            assert!(last < duration && count as f64 / f64::from(fps) >= duration);
            let wav = motion_audio::read_wav(dir.join("scene.wav")).unwrap();
            assert_eq!(wav.frames(), rendered.audio.frames());
            assert_audio_matches_video(wav.frames() as u64, count, fps);
            let manifest = read_manifest(&dir);
            let frames = manifest["frames"].as_array().unwrap();
            assert_eq!(frames.len() as u64, count);
            for (i, frame) in frames.iter().enumerate() {
                assert_eq!(
                    frame["sample_frame"],
                    frame_to_sample(i as u64, fps).unwrap()
                );
            }
            fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn missing_ffmpeg_is_a_typed_error_before_any_output() {
    let scene = g2::load(&g2::scene_path()).unwrap();
    let missing = std::env::temp_dir().join("kilsat-a10-no-such-ffmpeg");
    for ffmpeg in [missing.as_path(), Path::new("ffmpeg")] {
        let dir = destination("no-ffmpeg");
        let error = motion_export::export_mp4_scene_with_art(
            &scene.script,
            0,
            &scene.rendered,
            &scene.art,
            &dir,
            ffmpeg,
            &CancellationToken::new(),
            |_| panic!("no frame may be rendered without an encoder"),
        )
        .unwrap_err();
        match error {
            Error::EncoderUnavailable { path } => assert_eq!(path, ffmpeg),
            other => panic!("expected EncoderUnavailable, got {other:?}"),
        }
        assert!(!dir.exists());
    }
}

#[test]
fn cancellation_during_export_is_typed_and_removes_partial_output() {
    let scene = g2::load(&g2::scene_path()).unwrap();
    let dir = destination("cancel");
    let cancel = CancellationToken::new();
    let mut written = 0;
    let error = export_scene_with_art(
        &scene.script,
        0,
        &scene.rendered,
        &scene.art,
        &dir,
        &cancel,
        |p| {
            written = p.completed_frames;
            if p.completed_frames == 10 {
                assert!(dir.join("frame_000009.svg").is_file());
                cancel.cancel();
            }
        },
    )
    .unwrap_err();
    assert!(matches!(error, Error::Cancelled), "{error:?}");
    assert_eq!(written, 10);
    assert!(!dir.exists());
}

/// A write that fails part way through the sequence (here: the next frame path is
/// already taken, a portable stand-in for a write error) is returned as the I/O
/// error and the export's own directory is removed.
#[test]
fn io_failure_mid_export_is_typed_and_removes_partial_output() {
    let scene = g2::load(&g2::scene_path()).unwrap();
    let dir = destination("io");
    let error = export_scene_with_art(
        &scene.script,
        0,
        &scene.rendered,
        &scene.art,
        &dir,
        &CancellationToken::new(),
        |p| {
            if p.completed_frames == 6 {
                fs::create_dir(dir.join("frame_000006.svg")).unwrap();
            }
        },
    )
    .unwrap_err();
    match &error {
        Error::Io(io) => assert_eq!(io.kind(), std::io::ErrorKind::AlreadyExists),
        other => panic!("expected Io, got {other:?}"),
    }
    assert!(!dir.exists());
}

/// If the partial output cannot be removed, the original error is kept and the
/// leftover directory is named, so a caller can mark or remove it.
#[cfg(unix)]
#[test]
fn failed_cleanup_reports_the_cause_and_the_leftover_directory() {
    use std::os::unix::fs::PermissionsExt;
    let scene = g2::load(&g2::scene_path()).unwrap();
    let dir = destination("cleanup");
    let locked = dir.join("locked");
    let error = export_scene_with_art(
        &scene.script,
        0,
        &scene.rendered,
        &scene.art,
        &dir,
        &CancellationToken::new(),
        |p| {
            if p.completed_frames == 3 {
                fs::create_dir(&locked).unwrap();
                fs::write(locked.join("keep"), b"x").unwrap();
                fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
                fs::create_dir(dir.join("frame_000003.svg")).unwrap();
            }
        },
    )
    .unwrap_err();
    // Permissions are not enforced for root; then cleanup succeeds normally.
    let enforced = fs::remove_file(locked.join("keep")).is_err();
    if enforced {
        match &error {
            Error::PartialOutput {
                path,
                cause,
                cleanup,
            } => {
                assert_eq!(path, &dir);
                assert!(matches!(**cause, Error::Io(_)), "{cause:?}");
                assert_eq!(cleanup.kind(), std::io::ErrorKind::PermissionDenied);
                let text = error.to_string();
                assert!(text.contains("partial output left at"), "{text}");
            }
            other => panic!("expected PartialOutput, got {other:?}"),
        }
        assert!(dir.exists());
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        fs::remove_dir_all(&dir).unwrap();
    } else {
        eprintln!("note: permissions not enforced (root?); PartialOutput path not exercised");
        assert!(matches!(error, Error::Io(_)), "{error:?}");
        assert!(!dir.exists());
    }
}

/// Real full disk. Needs an explicitly provided small writable volume, e.g. a 2 MiB
/// macOS RAM disk (see `docs/evidence/g2-export/README.md`):
/// `KILSAT_TEST_FULL_VOLUME=/Volumes/KILSATFULL cargo test -p motion-export --test g2_export -- --ignored`.
#[test]
#[ignore = "requires KILSAT_TEST_FULL_VOLUME pointing at a small writable volume"]
fn full_disk_is_typed_and_removes_partial_output() {
    let volume = PathBuf::from(
        std::env::var_os("KILSAT_TEST_FULL_VOLUME").expect("set KILSAT_TEST_FULL_VOLUME"),
    );
    let scene = g2::load(&g2::scene_path()).unwrap();
    let dir = volume.join(format!("kilsat-a10-full-{}", std::process::id()));
    let mut completed = 0;
    let error = export_png_scene_with_art(
        &scene.script,
        0,
        &scene.rendered,
        &scene.art,
        &dir,
        &CancellationToken::new(),
        |p| completed = p.completed_frames,
    )
    .unwrap_err();
    match &error {
        Error::Io(io) => assert_eq!(io.kind(), std::io::ErrorKind::StorageFull, "{io:?}"),
        other => panic!("expected Io(StorageFull), got {other:?}"),
    }
    assert!(
        completed > 0 && completed < 96,
        "failed after {completed} frames"
    );
    assert!(!dir.exists(), "partial output must be removed");
}
