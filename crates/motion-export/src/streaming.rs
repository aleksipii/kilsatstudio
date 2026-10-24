//! Explicit bounded raw-frame streaming. Legacy interchange-export limits remain unchanged.
use super::*;
use motion_scene::SequenceTimeline;
use std::{
    fs::File,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

/// Explicit streaming profile, never a silent resolution or frame-rate reduction.
#[derive(Clone, Debug)]
pub struct StreamingProfile {
    /// Export-only limit. Whole-pipeline timing must also include speech/preparation.
    pub deadline: Duration,
    pub max_output_bytes: u64,
    review_size: Option<(u32, u32)>,
}
impl StreamingProfile {
    pub fn full_hd_24() -> Self {
        Self {
            deadline: Duration::from_secs(300),
            max_output_bytes: 512 * 1024 * 1024,
            review_size: None,
        }
    }
    /// Explicit reduced-size review; authored coordinates and frame rate stay unchanged.
    pub fn with_review_size(mut self, width: u32, height: u32) -> Result<Self, Error> {
        if width == 0
            || height == 0
            || width > 1920
            || height > 1080
            || !width.is_multiple_of(2)
            || !height.is_multiple_of(2)
        {
            return Err(invalid(
                "review size must be positive even dimensions within 1920x1080",
            ));
        }
        self.review_size = Some((width, height));
        Ok(self)
    }
    fn dimensions(&self, script: &Script) -> (u32, u32) {
        self.review_size
            .unwrap_or((script.project.width, script.project.height))
    }
    fn validate(&self, script: &Script) -> Result<(), Error> {
        if self.deadline < Duration::from_secs(1)
            || self.deadline > Duration::from_secs(3600)
            || !(32 * 1024 * 1024..=1024 * 1024 * 1024).contains(&self.max_output_bytes)
        {
            return Err(invalid(
                "streaming profile requires a 1..3600s deadline and 32..1024MiB output cap",
            ));
        }
        if script.project.width == 0
            || script.project.height == 0
            || script.project.width > 1920
            || script.project.height > 1080
            || script.project.fps == 0
            || script.project.fps > 24
        {
            return Err(invalid("streaming profile supports at most 1920x1080 at 24fps; dimensions/fps are never changed automatically"));
        }
        if let Some((width, height)) = self.review_size {
            if width > script.project.width
                || height > script.project.height
                || u64::from(width) * u64::from(script.project.height)
                    != u64::from(height) * u64::from(script.project.width)
            {
                return Err(invalid("explicit review size must preserve the authored aspect ratio without upscaling"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct StreamingReport {
    pub frame_count: u64,
    pub output_directory: PathBuf,
    pub total_samples: u64,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub source_width: u32,
    pub source_height: u32,
    pub review: bool,
    pub elapsed_seconds: f64,
    pub video_bytes: u64,
}

fn preflight(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    profile: &StreamingProfile,
    captions: CaptionMode,
) -> Result<(SequenceTimeline, u64, u64), Error> {
    profile.validate(script)?;
    // Preview preflight validates every scene/audio/art asset and bounds per-frame
    // geometry. Streaming has a separate cumulative frame/time/output budget.
    let (timeline, frames, preview_working) =
        sequence::preflight(script, rendered, art, false, captions)?;
    let source_samples = timeline.spans().iter().try_fold(0u64, |n, span| {
        n.checked_add(span.duration_samples)
            .ok_or_else(|| invalid("source duration overflow"))
    })?;
    if frames == 0
        || frames > 1440
        || timeline.total_samples() > 60 * u64::from(SAMPLE_RATE)
        || source_samples > 120 * u64::from(SAMPLE_RATE)
        || script.scenes.len() > 128
    {
        return Err(invalid(
            "streaming profile permits 1..1440 frames, 60s output and 120s scene-source audio",
        ));
    }
    let pixels = u64::from(script.project.width) * u64::from(script.project.height);
    let working = preview_working
        .checked_add(
            pixels
                .saturating_sub(pixels.min(1024 * 1024))
                .saturating_mul(24),
        )
        .ok_or_else(|| invalid("streaming working-set overflow"))?;
    if working > MAX_BYTES {
        return Err(invalid("full-size streaming working set exceeds 256MiB"));
    }
    Ok((timeline, frames, working))
}

fn enforce(
    cancel: &CancellationToken,
    started: Instant,
    profile: &StreamingProfile,
    reason: &Arc<Mutex<Option<String>>>,
) -> Result<(), Error> {
    check(cancel)?;
    if let Some(reason) = reason.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Err(invalid(reason));
    }
    if started.elapsed() >= profile.deadline {
        return Err(invalid("streaming export deadline exceeded"));
    }
    Ok(())
}

struct Directory {
    stage: PathBuf,
    destination: PathBuf,
    reserved: bool,
    complete: bool,
}
impl Directory {
    fn new(destination: &Path) -> Result<Self, Error> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        if destination.exists() {
            return Err(invalid("destination already exists"));
        }
        let name = destination
            .file_name()
            .filter(|n| !n.is_empty())
            .ok_or_else(|| invalid("output directory needs a new name"))?;
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = fs::canonicalize(parent)?;
        if !parent.is_dir() {
            return Err(invalid("output parent must be an existing directory"));
        }
        let destination = parent.join(name);
        let stage = parent.join(format!(
            ".kilsat-stream-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&stage)?;
        Ok(Self {
            stage,
            destination,
            reserved: false,
            complete: false,
        })
    }
    fn publish(&mut self) -> Result<(), Error> {
        // Atomic no-overwrite reservation avoids rename replacing a raced empty
        // directory. All bytes are complete before this short publication stage.
        fs::create_dir(&self.destination)?;
        self.reserved = true;
        for name in ["audio.wav", "sequence.mp4", "sequence.json"] {
            fs::rename(self.stage.join(name), self.destination.join(name))?;
        }
        self.complete = true;
        Ok(())
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.stage);
        if self.reserved && !self.complete {
            let _ = fs::remove_dir_all(&self.destination);
        }
    }
}

struct Encoder {
    child: Arc<Mutex<Child>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Drop for Encoder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        {
            let mut child = self.child.lock().unwrap_or_else(|e| e.into_inner());
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn directory_bytes(path: &Path) -> std::io::Result<u64> {
    fs::read_dir(path)?.try_fold(0u64, |n, entry| {
        let m = entry?.metadata()?;
        if !m.is_file() {
            return Err(std::io::Error::other("unexpected streaming output entry"));
        }
        n.checked_add(m.len())
            .ok_or_else(|| std::io::Error::other("streaming disk-size overflow"))
    })
}
fn bounded_text(path: &Path) -> String {
    use std::io::Read;
    let mut text = String::new();
    if let Ok(file) = File::open(path) {
        let _ = file.take(4096).read_to_string(&mut text);
    }
    text
}

/// Render one selected scene into the same streaming sequence format.
#[allow(clippy::too_many_arguments)]
pub fn export_mp4_scene_streaming(
    script: &Script,
    index: usize,
    rendered: &RenderedScene,
    art: Option<&ArtContext>,
    destination: &Path,
    ffmpeg: &Path,
    profile: &StreamingProfile,
    cancel: &CancellationToken,
    progress: impl FnMut(Progress),
    captions: CaptionMode,
) -> Result<StreamingReport, Error> {
    let selected = script
        .scenes
        .get(index)
        .ok_or_else(|| invalid("missing scene index"))?;
    let mut single = script.clone();
    let mut scene = selected.clone();
    scene.composition.transition = motion_script::Transition::Cut;
    single.scenes = vec![scene];
    let mut selected_art = art.cloned();
    if let Some(a) = &mut selected_art {
        a.document
            .layers
            .retain(|layer| layer.scene_id == selected.id);
        a.document
            .groups
            .retain(|group| group.scene_id == selected.id);
        a.document
            .lights
            .retain(|light| light.scene_id == selected.id);
    }
    export_mp4_sequence_streaming(
        &single,
        std::slice::from_ref(rendered),
        selected_art.as_ref(),
        destination,
        ffmpeg,
        profile,
        cancel,
        progress,
        captions,
    )
}

/// Blocking worker-only export with one composed frame in flight. No PNG sequence
/// is retained. Exact shared scene/crossfade/caption evaluation drives every frame.
#[allow(clippy::too_many_arguments)]
pub fn export_mp4_sequence_streaming(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    destination: &Path,
    ffmpeg: &Path,
    profile: &StreamingProfile,
    cancel: &CancellationToken,
    mut progress: impl FnMut(Progress),
    captions: CaptionMode,
) -> Result<StreamingReport, Error> {
    let started = Instant::now();
    let (width, height) = profile.dimensions(script);
    check(cancel)?;
    media::validate_encoder(script, ffmpeg)?;
    let (timeline, total, working) = preflight(script, rendered, art, profile, captions)?;
    let _budget = motion_jobs::process_budget()
        .reserve("streaming full-size export working set", working)
        .map_err(|e| invalid(e.to_string()))?;
    let validated = motion_scene::ValidatedScript::new(Arc::new(script.clone()))
        .map_err(|e| invalid(e.to_string()))?;
    let reason = Arc::new(Mutex::new(None));
    enforce(cancel, started, profile, &reason)?;
    let audio = motion_voice::sequence::mix_sequence(&timeline, rendered, cancel)
        .map_err(|e| invalid(e.to_string()))?;
    enforce(cancel, started, profile, &reason)?;
    let mut directory = Directory::new(destination)?;
    let audio_path = directory.stage.join("audio.wav");
    motion_audio::write_wav(&audio_path, &audio).map_err(|e| invalid(e.to_string()))?;
    drop(audio);
    enforce(cancel, started, profile, &reason)?;
    let video = directory.stage.join("sequence.mp4");
    let errors = directory.stage.join("encoder.stderr");
    let progress_file = directory.stage.join("encoder.progress");
    let mut command = Command::new(ffmpeg);
    command
        .args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostats",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
        ])
        .arg(format!("{width}x{height}"))
        .args([
            "-framerate",
            &script.project.fps.to_string(),
            "-i",
            "pipe:0",
            "-i",
        ])
        .arg(&audio_path)
        .args([
            "-map",
            "0:v:0",
            "-map",
            "1:a:0",
            "-frames:v",
            &total.to_string(),
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "18",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-movflags",
            "+faststart",
            "-fs",
        ])
        .arg((profile.max_output_bytes - 16 * 1024 * 1024).to_string())
        .arg("-progress")
        .arg(&progress_file)
        .arg("-n")
        .arg(&video)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::from(File::create(&errors)?));
    let mut child = command
        .spawn()
        .map_err(|e| invalid(format!("start streaming FFmpeg: {e}")))?;
    let mut input = child
        .stdin
        .take()
        .ok_or_else(|| invalid("encoder stdin unavailable"))?;
    let child = Arc::new(Mutex::new(child));
    let stop = Arc::new(AtomicBool::new(false));
    let watch_child = Arc::clone(&child);
    let watch_stop = Arc::clone(&stop);
    let watch_reason = Arc::clone(&reason);
    let watch_cancel = cancel.clone();
    let watch_profile = profile.clone();
    let watch_dir = directory.stage.clone();
    let watch = thread::spawn(move || {
        while !watch_stop.load(Ordering::Acquire) {
            let failure = if watch_cancel.is_cancelled() {
                Some("streaming export cancelled".to_string())
            } else if started.elapsed() >= watch_profile.deadline {
                Some("streaming export deadline exceeded".into())
            } else if directory_bytes(&watch_dir)
                .map_or(true, |n| n > watch_profile.max_output_bytes)
                || fs::metadata(watch_dir.join("encoder.stderr"))
                    .is_ok_and(|m| m.len() > 1024 * 1024)
                || fs::metadata(watch_dir.join("encoder.progress"))
                    .is_ok_and(|m| m.len() > 1024 * 1024)
            {
                Some("streaming output/log byte budget exceeded".into())
            } else {
                None
            };
            if let Some(failure) = failure {
                *watch_reason.lock().unwrap_or_else(|e| e.into_inner()) = Some(failure);
                let mut child = watch_child.lock().unwrap_or_else(|e| e.into_inner());
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
    });
    let encoder = Encoder {
        child,
        stop,
        thread: Some(watch),
    };
    for frame in 0..total {
        enforce(cancel, started, profile, &reason)?;
        let sample = frame * u64::from(SAMPLE_RATE) / u64::from(script.project.fps);
        let (image, _) = sequence::compose(
            script,
            rendered,
            art,
            &timeline,
            sample,
            profile.review_size.map(|(w, h)| w.max(h)),
            cancel,
            Some(&validated),
            captions,
        )?;
        if image.width() != width || image.height() != height {
            return Err(invalid(
                "raster dimensions differ from explicit output profile",
            ));
        }
        enforce(cancel, started, profile, &reason)?;
        if let Err(error) = input.write_all(image.data()) {
            enforce(cancel, started, profile, &reason)?;
            return Err(invalid(format!(
                "streaming encoder write: {error}; {}",
                bounded_text(&errors)
            )));
        }
        progress(Progress {
            completed_frames: frame + 1,
            total_frames: total,
        });
    }
    drop(input);
    let status = loop {
        enforce(cancel, started, profile, &reason)?;
        let status = encoder
            .child
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_wait()?;
        if let Some(status) = status {
            break status;
        }
        thread::sleep(Duration::from_millis(10));
    };
    enforce(cancel, started, profile, &reason)?;
    if !status.success() {
        return Err(invalid(format!(
            "streaming encoder failed: {status}; {}",
            bounded_text(&errors)
        )));
    }
    encoder.stop.store(true, Ordering::Release);
    let progress_text = fs::read_to_string(&progress_file)?;
    let encoded_frames = progress_text
        .lines()
        .filter_map(|l| l.strip_prefix("frame="))
        .filter_map(|v| v.trim().parse::<u64>().ok())
        .last();
    if !progress_text.lines().any(|l| l == "progress=end") || encoded_frames != Some(total) {
        return Err(invalid(
            "encoder did not confirm every requested frame; output may have reached its size limit",
        ));
    }
    let video_bytes = fs::metadata(&video)?.len();
    if video_bytes == 0 || directory_bytes(&directory.stage)? > profile.max_output_bytes {
        return Err(invalid("empty or oversized streaming output"));
    }
    drop(encoder);
    fs::remove_file(errors)?;
    fs::remove_file(progress_file)?;
    let manifest = serde_json::json!({"schema_version":1,"mode":"bounded-streaming-rgba","sample_rate":SAMPLE_RATE,
        "fps":script.project.fps,"width":width,"height":height,"source_width":script.project.width,"source_height":script.project.height,"review":profile.review_size.is_some(),
        "total_samples":timeline.total_samples(),"frame_count":total,"audio_file":"audio.wav","video_file":"sequence.mp4",
        "captions":captions,"scenes":timeline.spans().iter().map(|s|serde_json::json!({"scene_index":s.scene_index,"scene_id":script.scenes[s.scene_index].id,
            "start_sample":s.start_sample,"duration_samples":s.duration_samples,"outgoing_overlap":s.outgoing_overlap})).collect::<Vec<_>>()});
    fs::write(
        directory.stage.join("sequence.json"),
        bounded_json(&manifest, 1024 * 1024)?,
    )?;
    enforce(cancel, started, profile, &reason)?;
    if directory_bytes(&directory.stage)? > profile.max_output_bytes {
        return Err(invalid("streaming final byte budget exceeded"));
    }
    directory.publish()?;
    Ok(StreamingReport {
        frame_count: total,
        output_directory: destination.to_path_buf(),
        total_samples: timeline.total_samples(),
        width,
        height,
        fps: script.project.fps,
        source_width: script.project.width,
        source_height: script.project.height,
        review: profile.review_size.is_some(),
        elapsed_seconds: started.elapsed().as_secs_f64(),
        video_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(seconds: f64, width: u32, height: u32, fps: u32) -> (Script, Vec<RenderedScene>) {
        let source=format!("project: {{width: {width}, height: {height}, fps: {fps}}}\ncharacters: {{}}\nscenes: [{{id: s, duration: {seconds}, events: []}}]\n");
        let script = motion_script::parse(&source).unwrap();
        let rendered = motion_voice::export::render_scene("s", seconds, &[]).unwrap();
        (script, vec![rendered])
    }
    fn destination() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "kilsat-stream-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
    #[test]
    fn explicit_profile_accepts_full_hd_without_weakening_legacy_work_guards() {
        let (script, rendered) = fixture(30.0, 1920, 1080, 24);
        let profile = StreamingProfile::full_hd_24();
        let (_, frames, working) =
            preflight(&script, &rendered, None, &profile, CaptionMode::Follow).unwrap();
        assert_eq!(frames, 720);
        assert!(working < MAX_BYTES);
        assert!(sequence::preflight(&script, &rendered, None, true, CaptionMode::Follow).is_err());
        let (mut script, rendered) = fixture(60.0, 1920, 1080, 24);
        assert_eq!(
            preflight(&script, &rendered, None, &profile, CaptionMode::Follow)
                .unwrap()
                .1,
            1440
        );
        script.project.fps = 25;
        assert!(preflight(&script, &rendered, None, &profile, CaptionMode::Follow).is_err());
    }
    #[test]
    fn review_is_explicit_aspect_preserving_and_deadlines_and_caps_are_bounded() {
        let (script, rendered) = fixture(0.25, 1920, 1080, 24);
        let profile = StreamingProfile::full_hd_24()
            .with_review_size(960, 540)
            .unwrap();
        assert!(preflight(&script, &rendered, None, &profile, CaptionMode::Follow).is_ok());
        let mut bad = StreamingProfile::full_hd_24()
            .with_review_size(640, 480)
            .unwrap();
        assert!(preflight(&script, &rendered, None, &bad, CaptionMode::Follow).is_err());
        bad.deadline = Duration::ZERO;
        assert!(bad.validate(&script).is_err());
        bad = StreamingProfile::full_hd_24();
        bad.max_output_bytes = 16 * 1024 * 1024;
        assert!(bad.validate(&script).is_err());
        assert!(StreamingProfile::full_hd_24()
            .with_review_size(961, 540)
            .is_err());
    }
    #[test]
    fn direct_rasterization_matches_retained_png_pixels() {
        let svg="<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"64\" height=\"32\"><rect width=\"64\" height=\"32\" fill=\"#1234ab\"/><circle cx=\"23\" cy=\"16\" r=\"9\" fill=\"#fedcba\" opacity=\"0.6\"/></svg>";
        let png = media::rasterize_scaled(svg, None).unwrap();
        let retained = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
        let direct = media::rasterize_scaled_pixmap(svg, None).unwrap();
        assert_eq!(retained.data(), direct.data());
    }
    #[cfg(unix)]
    #[test]
    fn streaming_deadline_kills_a_blocked_encoder_and_cleans_staging() {
        use std::os::unix::fs::PermissionsExt;
        let root = destination();
        fs::create_dir(&root).unwrap();
        let encoder = root.join("blocked-encoder");
        fs::write(&encoder, "#!/bin/sh\nexec sleep 20\n").unwrap();
        fs::set_permissions(&encoder, fs::Permissions::from_mode(0o755)).unwrap();
        let out = root.join("out");
        let (script, rendered) = fixture(1.0, 640, 360, 24);
        let mut profile = StreamingProfile::full_hd_24();
        profile.deadline = Duration::from_secs(1);
        let started = Instant::now();
        let result = export_mp4_sequence_streaming(
            &script,
            &rendered,
            None,
            &out,
            &encoder,
            &profile,
            &CancellationToken::new(),
            |_| {},
            CaptionMode::Follow,
        );
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(!out.exists());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "requires an explicitly installed FFmpeg; run in media CI"]
    fn real_ffmpeg_streaming_confirms_every_frame_and_has_no_png_sequence() {
        let ffmpeg = PathBuf::from(
            std::env::var_os("KILSAT_TEST_FFMPEG").unwrap_or_else(|| "/usr/bin/ffmpeg".into()),
        );
        let (script, rendered) = fixture(0.25, 64, 64, 8);
        let out = destination();
        let report = export_mp4_sequence_streaming(
            &script,
            &rendered,
            None,
            &out,
            &ffmpeg,
            &StreamingProfile::full_hd_24(),
            &CancellationToken::new(),
            |_| {},
            CaptionMode::Follow,
        )
        .unwrap();
        assert_eq!(report.frame_count, 2);
        assert!(report.video_bytes > 0);
        assert!(!report.review);
        let names: Vec<_> = fs::read_dir(&out)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 3);
        assert!(out.join("audio.wav").is_file());
        assert!(out.join("sequence.mp4").is_file());
        assert!(!out.join("frame-000000.png").exists());
        let decoded = Command::new(ffmpeg)
            .args(["-v", "error", "-i"])
            .arg(out.join("sequence.mp4"))
            .args(["-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"])
            .output()
            .unwrap();
        assert!(decoded.status.success());
        assert_eq!(decoded.stdout.len(), 2 * 64 * 64 * 4);
        fs::remove_dir_all(out).unwrap();
    }
}
