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
impl Directory {
    /// Remove the staging directory and any half-published destination. Returns the
    /// first path that could not be removed.
    fn discard(&mut self) -> Result<(), (PathBuf, std::io::Error)> {
        let mut failed = None;
        if self.reserved && !self.complete {
            if let Err(e) = fs::remove_dir_all(&self.destination) {
                failed = Some((self.destination.clone(), e));
            }
        }
        self.reserved = false;
        match fs::remove_dir_all(&self.stage) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                failed.get_or_insert((self.stage.clone(), e));
            }
            _ => {}
        }
        failed.map_or(Ok(()), Err)
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
    check(cancel)?;
    bounded_json(script, MAX_SOURCE)?;
    if let Some(art) = art {
        art.document
            .validate()
            .map_err(|e| invalid(e.to_string()))?;
        bounded_json(&art.document, 8 * MAX_SOURCE)?;
    }
    let (single, selected_art) = frames::single_scene(script, index, art)?;
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
    progress: impl FnMut(Progress),
    captions: CaptionMode,
) -> Result<StreamingReport, Error> {
    let started = Instant::now();
    check(cancel)?;
    media::validate_encoder(script, ffmpeg)?;
    let (timeline, total, working) = preflight(script, rendered, art, profile, captions)?;
    let validated = motion_scene::ValidatedScript::new(Arc::new(script.clone()))
        .map_err(|e| invalid(e.to_string()))?;
    let review = profile.review_size.map(|(w, h)| w.max(h));
    encode(
        Stream {
            script,
            rendered,
            timeline: &timeline,
            total,
            working,
            destination,
            ffmpeg,
            profile,
            cancel,
            captions,
            started,
            mode: "bounded-streaming-rgba",
            extra: serde_json::Value::Null,
        },
        progress,
        |frame| {
            let sample = frame * u64::from(SAMPLE_RATE) / u64::from(script.project.fps);
            let (image, _) = sequence::compose(
                script,
                rendered,
                art,
                &timeline,
                sample,
                review,
                cancel,
                Some(&validated),
                captions,
            )?;
            Ok(Arc::new(image))
        },
    )
}

/// What `encode` streams: the validated sequence and its output settings.
pub(crate) struct Stream<'a> {
    pub script: &'a Script,
    pub rendered: &'a [RenderedScene],
    pub timeline: &'a SequenceTimeline,
    pub total: u64,
    pub working: u64,
    pub destination: &'a Path,
    pub ffmpeg: &'a Path,
    pub profile: &'a StreamingProfile,
    pub cancel: &'a CancellationToken,
    pub captions: CaptionMode,
    pub started: Instant,
    /// `mode` written to `sequence.json`.
    pub mode: &'static str,
    /// Extra manifest fields (an object), or `Null`.
    pub extra: serde_json::Value,
}

/// Mix the audio, then encode `total` frames from `frame` (one in flight)
/// with the sequence audio, publishing `audio.wav`, `sequence.mp4` and
/// `sequence.json` atomically into a new directory.
pub(crate) fn encode(
    stream: Stream<'_>,
    mut progress: impl FnMut(Progress),
    mut frame_image: impl FnMut(u64) -> Result<Arc<resvg::tiny_skia::Pixmap>, Error>,
) -> Result<StreamingReport, Error> {
    let Stream {
        script,
        rendered,
        timeline,
        total,
        working,
        destination,
        ffmpeg,
        profile,
        cancel,
        captions,
        started,
        mode,
        extra,
    } = stream;
    let timeline = timeline.clone();
    let (width, height) = profile.dimensions(script);
    let _budget = motion_jobs::process_budget()
        .reserve("streaming full-size export working set", working)
        .map_err(|e| invalid(e.to_string()))?;
    let reason = Arc::new(Mutex::new(None));
    enforce(cancel, started, profile, &reason)?;
    let audio = motion_voice::sequence::mix_sequence(&timeline, rendered, cancel)
        .map_err(|e| invalid(e.to_string()))?;
    enforce(cancel, started, profile, &reason)?;
    let mut directory = Directory::new(destination)?;
    // Run the staged work in one scope so the encoder is stopped before cleanup,
    // and a failed cleanup is reported instead of being lost in `Drop`.
    let result = (|| -> Result<StreamingReport, Error> {
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
            let image = frame_image(frame)?;
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
            .next_back();
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
        let mut manifest = serde_json::json!({"schema_version":1,"mode":mode,"sample_rate":SAMPLE_RATE,
        "fps":script.project.fps,"width":width,"height":height,"source_width":script.project.width,"source_height":script.project.height,"review":profile.review_size.is_some(),
        "total_samples":timeline.total_samples(),"frame_count":total,"audio_file":"audio.wav","video_file":"sequence.mp4",
        "captions":captions,"scenes":timeline.spans().iter().map(|s|serde_json::json!({"scene_index":s.scene_index,"scene_id":script.scenes[s.scene_index].id,
            "start_sample":s.start_sample,"duration_samples":s.duration_samples,"outgoing_overlap":s.outgoing_overlap})).collect::<Vec<_>>()});
        if let (Some(manifest), serde_json::Value::Object(extra)) =
            (manifest.as_object_mut(), extra)
        {
            manifest.extend(extra);
        }
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
    })();
    match result {
        Ok(report) => Ok(report),
        Err(cause) => match directory.discard() {
            Ok(()) => Err(cause),
            Err((path, cleanup)) => Err(Error::PartialOutput {
                path,
                cause: Box::new(cause),
                cleanup,
            }),
        },
    }
}

/// Longest animatic: ten minutes at 24 fps.
pub const MAX_ANIMATIC_FRAMES: u64 = 24 * 600;

impl StreamingProfile {
    /// Animatic profile: a 15-minute export deadline and a 1 GiB cap, at half
    /// the authored size when both sides halve to even numbers (1920×1080 →
    /// 960×540), otherwise at the authored size.
    pub fn animatic(script: &Script) -> Self {
        let profile = Self {
            deadline: Duration::from_secs(900),
            max_output_bytes: 1024 * 1024 * 1024,
            review_size: None,
        };
        let (w, h) = (script.project.width, script.project.height);
        if w.is_multiple_of(4) && h.is_multiple_of(4) {
            profile
                .clone()
                .with_review_size(w / 2, h / 2)
                .unwrap_or(profile)
        } else {
            profile
        }
    }
}

/// One held animatic panel on the sequence clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnimaticPanel {
    /// Shot id, `<scene id>/<number>`.
    pub shot: String,
    /// First sequence sample of the shot.
    pub start_sample: u64,
    /// Sequence sample drawn and held for the whole shot.
    pub panel_sample: u64,
}

/// Every shot's panel on the sequence clock, in time order.
pub fn animatic_panels(
    script: &Script,
    timeline: &SequenceTimeline,
) -> Result<Vec<AnimaticPanel>, Error> {
    let shots = motion_script::shot_list(script).map_err(|e| invalid(e.to_string()))?;
    let to_sample = |seconds: f64| {
        motion_scene::time::seconds_to_sample(seconds).map_err(|e| invalid(e.to_string()))
    };
    let mut panels = Vec::with_capacity(shots.len());
    for shot in &shots {
        let span = timeline
            .spans()
            .get(shot.scene_index)
            .ok_or_else(|| invalid("shot outside the sequence"))?;
        let last = span.start_sample + span.duration_samples.saturating_sub(1);
        panels.push(AnimaticPanel {
            shot: shot.id.clone(),
            start_sample: (span.start_sample + to_sample(shot.start)?).min(last),
            panel_sample: (span.start_sample + to_sample(shot.panel)?).min(last),
        });
    }
    panels.sort_by_key(|p| p.start_sample);
    Ok(panels)
}

/// The panel shown at sequence sample `sample`: the last one started by then.
fn panel_at(panels: &[AnimaticPanel], sample: u64) -> usize {
    panels
        .partition_point(|p| p.start_sample <= sample)
        .saturating_sub(1)
}

/// Export an animatic: each shot is held on its storyboard panel for the
/// shot's length while the sequence audio plays in real time. Panels use the
/// same composition (art, captions, camera) as the full export, each drawn once.
#[allow(clippy::too_many_arguments)]
pub fn export_animatic_streaming(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    destination: &Path,
    ffmpeg: &Path,
    profile: &StreamingProfile,
    cancel: &CancellationToken,
    progress: impl FnMut(Progress),
    captions: CaptionMode,
) -> Result<StreamingReport, Error> {
    let started = Instant::now();
    check(cancel)?;
    bounded_json(script, MAX_SOURCE)?;
    media::validate_encoder(script, ffmpeg)?;
    profile.validate(script)?;
    let (timeline, frames, preview_working) =
        sequence::preflight(script, rendered, art, false, captions)?;
    if frames == 0
        || frames > MAX_ANIMATIC_FRAMES
        || timeline.total_samples() > 600 * u64::from(SAMPLE_RATE)
    {
        return Err(invalid("an animatic permits 1 frame to 10 minutes"));
    }
    let panels = animatic_panels(script, &timeline)?;
    if panels.is_empty() {
        return Err(invalid("the sequence has no shots"));
    }
    let pixels = u64::from(script.project.width) * u64::from(script.project.height);
    // One held panel besides the frame being composed.
    let working = preview_working
        .checked_add(pixels.saturating_mul(8))
        .ok_or_else(|| invalid("animatic working-set overflow"))?;
    if working > MAX_BYTES {
        return Err(invalid("animatic working set exceeds 256MiB"));
    }
    let validated = motion_scene::ValidatedScript::new(Arc::new(script.clone()))
        .map_err(|e| invalid(e.to_string()))?;
    let review = profile.review_size.map(|(w, h)| w.max(h));
    let mut held: Option<(usize, Arc<resvg::tiny_skia::Pixmap>)> = None;
    encode(
        Stream {
            script,
            rendered,
            timeline: &timeline,
            total: frames,
            working,
            destination,
            ffmpeg,
            profile,
            cancel,
            captions,
            started,
            mode: "animatic-held-panels",
            extra: serde_json::json!({ "panels": panels }),
        },
        progress,
        |frame| {
            let sample = frame * u64::from(SAMPLE_RATE) / u64::from(script.project.fps);
            let index = panel_at(&panels, sample);
            if let Some((shown, image)) = &held {
                if *shown == index {
                    return Ok(Arc::clone(image));
                }
            }
            let (image, _) = sequence::compose(
                script,
                rendered,
                art,
                &timeline,
                panels[index].panel_sample,
                review,
                cancel,
                Some(&validated),
                captions,
            )?;
            let image = Arc::new(image);
            held = Some((index, Arc::clone(&image)));
            Ok(image)
        },
    )
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
    #[cfg(unix)]
    #[test]
    fn failed_streaming_cleanup_reports_the_cause_and_the_leftover_directory() {
        use std::os::unix::fs::PermissionsExt;
        if Command::new("id")
            .arg("-u")
            .output()
            .is_ok_and(|o| o.stdout.starts_with(b"0"))
        {
            eprintln!("skipped: root ignores directory permissions");
            return;
        }
        let root = destination();
        fs::create_dir(&root).unwrap();
        // The export reports canonical paths (`/var` is `/private/var` on macOS).
        let root = fs::canonicalize(root).unwrap();
        // The fake encoder locks the staging directory (parent of its last argument,
        // the video path) and blocks, so the deadline fails the export and the
        // cleanup cannot remove the staged files.
        let encoder = root.join("locking-encoder");
        fs::write(
            &encoder,
            "#!/bin/sh\nfor a; do last=$a; done\nchmod 555 \"$(dirname \"$last\")\"\nexec sleep 20\n",
        )
        .unwrap();
        fs::set_permissions(&encoder, fs::Permissions::from_mode(0o755)).unwrap();
        let out = root.join("out");
        let (script, rendered) = fixture(1.0, 640, 360, 24);
        let mut profile = StreamingProfile::full_hd_24();
        profile.deadline = Duration::from_secs(1);
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
        let stages: Vec<PathBuf> = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".kilsat-stream-")
            })
            .collect();
        for stage in &stages {
            fs::set_permissions(stage, fs::Permissions::from_mode(0o755)).unwrap();
        }
        match result {
            Err(Error::PartialOutput {
                path,
                cause,
                cleanup,
            }) => {
                assert_eq!(stages, vec![path]);
                assert!(cause.to_string().contains("deadline"), "{cause}");
                assert_eq!(cleanup.kind(), std::io::ErrorKind::PermissionDenied);
            }
            other => panic!("expected PartialOutput, got {other:?}"),
        }
        assert!(!out.exists());
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

    #[test]
    fn animatic_panels_follow_the_shots_on_the_sequence_clock() {
        let source = "project: {width: 1920, height: 1080, fps: 24}\ncharacters: {}\nscenes: [{id: a, duration: 4, events: [], composition: {camera: [{at: 0, x: 0, y: 0, zoom: 1}, {at: 1.999, x: 0, y: 0, zoom: 1}, {at: 2, x: 0, y: 0, zoom: 2}]}}, {id: b, duration: 2, events: []}]\n";
        let script = motion_script::parse(source).unwrap();
        let timeline = SequenceTimeline::compile(&script).unwrap();
        let panels = animatic_panels(&script, &timeline).unwrap();
        let shots: Vec<_> = panels.iter().map(|p| p.shot.as_str()).collect();
        assert_eq!(shots, ["a/010", "a/020", "b/010"]);
        let starts: Vec<_> = panels.iter().map(|p| p.start_sample).collect();
        assert_eq!(starts, [0, 48_000, 96_000]);
        // Panels are the middle of these quiet shots, held for the whole shot.
        assert_eq!(panels[0].panel_sample, 24_000);
        assert_eq!(panel_at(&panels, 0), 0);
        assert_eq!(panel_at(&panels, 47_999), 0);
        assert_eq!(panel_at(&panels, 48_000), 1);
        assert_eq!(panel_at(&panels, 143_999), 2);
        // Half-size review profile for Full HD, authored size otherwise.
        assert_eq!(
            StreamingProfile::animatic(&script).dimensions(&script),
            (960, 540)
        );
        let mut odd = script.clone();
        odd.project.width = 1918;
        assert_eq!(
            StreamingProfile::animatic(&odd).dimensions(&odd),
            (1918, 1080)
        );
    }
    #[test]
    fn animatic_rejects_overlong_sequences_before_encoding() {
        let (script, rendered) = fixture(601.0, 640, 360, 24);
        // Never executed: the length check comes before encoding.
        let encoder = std::env::temp_dir().join(format!("kilsat-never-run-{}", std::process::id()));
        fs::write(&encoder, b"").unwrap();
        let error = export_animatic_streaming(
            &script,
            &rendered,
            None,
            &destination(),
            &encoder,
            &StreamingProfile::animatic(&script),
            &CancellationToken::new(),
            |_| {},
            CaptionMode::Follow,
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("10 minutes") || error.contains("audio duration budget"),
            "{error}"
        );
        fs::remove_file(encoder).unwrap();
    }
    #[test]
    #[ignore = "requires an explicitly installed FFmpeg; run in media CI"]
    fn real_ffmpeg_animatic_holds_each_panel_for_its_shot() {
        let ffmpeg = PathBuf::from(
            std::env::var_os("KILSAT_TEST_FFMPEG").unwrap_or_else(|| "/usr/bin/ffmpeg".into()),
        );
        // Scene a turns from red to green at 0.3 s; its panel (0.25 s) is red,
        // so the whole held shot stays red. Scene b is blue.
        let source = "project: {width: 64, height: 64, fps: 8}\ncharacters: {}\nscenes: [{id: a, duration: 0.5, events: [], composition: {background_color: '#ff0000', background_keys: [{at: 0.3, color: '#00ff00'}]}}, {id: b, duration: 0.5, events: [], composition: {background_color: '#0000ff'}}]\n";
        let script = motion_script::parse(source).unwrap();
        let rendered = vec![
            motion_voice::export::render_scene("a", 0.5, &[]).unwrap(),
            motion_voice::export::render_scene("b", 0.5, &[]).unwrap(),
        ];
        let out = destination();
        let report = export_animatic_streaming(
            &script,
            &rendered,
            None,
            &out,
            &ffmpeg,
            &StreamingProfile::animatic(&script),
            &CancellationToken::new(),
            |_| {},
            CaptionMode::Follow,
        )
        .unwrap();
        assert_eq!(report.frame_count, 8);
        assert_eq!((report.width, report.height), (32, 32));
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(out.join("sequence.json")).unwrap()).unwrap();
        assert_eq!(manifest["mode"], "animatic-held-panels");
        assert_eq!(manifest["panels"].as_array().unwrap().len(), 2);
        let decoded = Command::new(ffmpeg)
            .args(["-v", "error", "-i"])
            .arg(out.join("sequence.mp4"))
            .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"])
            .output()
            .unwrap();
        assert!(decoded.status.success());
        let frame = 32 * 32 * 3;
        assert_eq!(decoded.stdout.len(), 8 * frame);
        // Centre pixel of each frame.
        let centre = |f: usize| {
            let at = f * frame + (16 * 32 + 16) * 3;
            [
                decoded.stdout[at],
                decoded.stdout[at + 1],
                decoded.stdout[at + 2],
            ]
        };
        for f in 0..4 {
            let [r, g, b] = centre(f);
            assert!(r > 200 && g < 60 && b < 60, "frame {f}: {:?}", centre(f));
        }
        for f in 4..8 {
            let [r, g, b] = centre(f);
            assert!(b > 200 && r < 60 && g < 60, "frame {f}: {:?}", centre(f));
        }
        fs::remove_dir_all(out).unwrap();
    }
}
