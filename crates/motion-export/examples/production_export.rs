//! Explicit local real-speech preparation and bounded streaming export.
//! No model/voice download, no fabricated speech and no publication approval.
use motion_export::{ArtContext, StreamingProfile};
use motion_script::Event;
use motion_tts::{CancellationToken, KokoroConfig, LocalSpeechProvider, SpeechProvider};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

type Failure = Box<dyn std::error::Error>;
fn fail(message: impl Into<String>) -> Failure {
    std::io::Error::other(message.into()).into()
}
#[derive(Default)]
struct Args {
    project: PathBuf,
    output: PathBuf,
    ffmpeg: PathBuf,
    piper: Option<PathBuf>,
    kokoro: Option<PathBuf>,
    review: Option<(u32, u32)>,
    review_frames: bool,
    retime: bool,
    private_system: bool,
    deadline: u64,
}
fn arguments() -> Result<Args, Failure> {
    let mut args = Args {
        deadline: 300,
        ..Default::default()
    };
    let mut input = std::env::args().skip(1);
    while let Some(key) = input.next() {
        match key.as_str() {
            "--project" => {
                args.project = input
                    .next()
                    .ok_or_else(|| fail("--project needs a value"))?
                    .into()
            }
            "--output" => {
                args.output = input
                    .next()
                    .ok_or_else(|| fail("--output needs a value"))?
                    .into()
            }
            "--ffmpeg" => {
                args.ffmpeg = input
                    .next()
                    .ok_or_else(|| fail("--ffmpeg needs a value"))?
                    .into()
            }
            "--piper-config" => {
                args.piper = Some(
                    input
                        .next()
                        .ok_or_else(|| fail("--piper-config needs a value"))?
                        .into(),
                )
            }
            "--kokoro-config" => {
                args.kokoro = Some(
                    input
                        .next()
                        .ok_or_else(|| fail("--kokoro-config needs a value"))?
                        .into(),
                )
            }
            "--deadline-seconds" => {
                args.deadline = input
                    .next()
                    .ok_or_else(|| fail("--deadline-seconds needs a value"))?
                    .parse()?
            }
            "--review-size" => {
                let size = input
                    .next()
                    .ok_or_else(|| fail("--review-size needs WIDTHxHEIGHT"))?;
                let (w, h) = size
                    .split_once('x')
                    .ok_or_else(|| fail("use WIDTHxHEIGHT"))?;
                args.review = Some((w.parse()?, h.parse()?));
            }
            "--review-frames" => args.review_frames = true,
            "--retime-dialogue" => args.retime = true,
            "--private-system-preview" => args.private_system = true,
            "--help" => {
                println!("production_export --project FILE --ffmpeg ABS --output NEWDIR [--piper-config JSON|--kokoro-config JSON] [--retime-dialogue] [--review-size 960x540] [--review-frames] [--deadline-seconds 300] [--private-system-preview]");
                std::process::exit(0);
            }
            _ => return Err(fail(format!("unknown argument: {key}"))),
        }
    }
    if args.project.as_os_str().is_empty()
        || args.output.as_os_str().is_empty()
        || args.ffmpeg.as_os_str().is_empty()
        || !(1..=3600).contains(&args.deadline)
    {
        return Err(fail(
            "provide --project FILE --ffmpeg ABS --output NEWDIR and a1..3600s deadline",
        ));
    }
    Ok(args)
}
struct Timer {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Timer {
    fn new(started: Instant, seconds: u64, cancel: CancellationToken) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                if started.elapsed() >= Duration::from_secs(seconds) {
                    cancel.cancel();
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for Timer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn check(started: Instant, args: &Args, cancel: &CancellationToken) -> Result<(), Failure> {
    if started.elapsed() >= Duration::from_secs(args.deadline) {
        return Err(fail("whole pipeline deadline exceeded"));
    }
    cancel.check()?;
    Ok(())
}
fn read(path: &Path, max: u64) -> Result<Vec<u8>, Failure> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > max {
        return Err(fail("input must be a bounded regular file"));
    }
    let mut bytes = Vec::new();
    File::open(path)?.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(fail("input byte budget exceeded"));
    }
    Ok(bytes)
}
/// Decode only bounded actual PCM bytes; never allocate a forged RIFF sample count.
fn pcm(path: &Path) -> Result<motion_audio::AudioBuffer, Failure> {
    let bytes = read(path, 8 * 1024 * 1024)?;
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(fail("speech must be RIFF/WAVE PCM16"));
    }
    let riff = u32::from_le_bytes(bytes[4..8].try_into()?) as usize;
    if riff.checked_add(8) != Some(bytes.len()) {
        return Err(fail("speech RIFF size differs from actual bytes"));
    }
    let mut at = 12usize;
    let mut format = false;
    let mut data = None;
    while at < bytes.len() {
        if at + 8 > bytes.len() {
            return Err(fail("truncated WAV chunk"));
        }
        let length = u32::from_le_bytes(bytes[at + 4..at + 8].try_into()?) as usize;
        let end = at
            .checked_add(8)
            .and_then(|n| n.checked_add(length))
            .ok_or_else(|| fail("WAV chunk overflow"))?;
        if end > bytes.len() {
            return Err(fail("truncated WAV data"));
        }
        match &bytes[at..at + 4] {
            b"fmt " => {
                if format || length < 16 {
                    return Err(fail("invalid duplicate/short WAV format"));
                }
                let f = &bytes[at + 8..end];
                if u16::from_le_bytes(f[0..2].try_into()?) != 1
                    || u16::from_le_bytes(f[2..4].try_into()?) != 1
                    || u32::from_le_bytes(f[4..8].try_into()?) != 24_000
                    || u32::from_le_bytes(f[8..12].try_into()?) != 48_000
                    || u16::from_le_bytes(f[12..14].try_into()?) != 2
                    || u16::from_le_bytes(f[14..16].try_into()?) != 16
                {
                    return Err(fail("speech must be24kHz mono PCM16; convert explicitly"));
                }
                format = true;
            }
            b"data" => {
                if data.is_some()
                    || length == 0
                    || !length.is_multiple_of(2)
                    || length > 24_000 * 60 * 2
                {
                    return Err(fail("speech data needs1..60seconds of PCM16"));
                }
                data = Some(&bytes[at + 8..end]);
            }
            _ => {}
        }
        at = end
            .checked_add(length % 2)
            .ok_or_else(|| fail("WAV alignment overflow"))?;
    }
    if !format || at != bytes.len() {
        return Err(fail("missing/invalid WAV format"));
    }
    let samples = data
        .ok_or_else(|| fail("missing WAV data"))?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0)
        .collect();
    Ok(motion_audio::AudioBuffer {
        sample_rate: 24_000,
        channels: 1,
        samples,
    })
}
struct Inputs(PathBuf);
impl Drop for Inputs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct Output {
    path: PathBuf,
    created: bool,
    complete: bool,
}
impl Drop for Output {
    fn drop(&mut self) {
        if self.created && !self.complete {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct KokoroProfile {
    python: PathBuf,
    runtime_script: PathBuf,
    model_config: PathBuf,
    model_weights: PathBuf,
    voices_directory: PathBuf,
    cache_directory: PathBuf,
    model_version: String,
    timeout_seconds: u64,
}
fn provider(args: &Args, parent: &Path) -> Result<LocalSpeechProvider, Failure> {
    let config = if let Some(path) = &args.kokoro {
        let c: KokoroProfile = serde_json::from_slice(&read(path, 1024 * 1024)?)?;
        if !(1..=600).contains(&c.timeout_seconds) {
            return Err(fail("invalid Kokoro timeout"));
        }
        KokoroConfig {
            python: c.python,
            runtime_script: c.runtime_script,
            model_config: c.model_config,
            model_weights: c.model_weights,
            voices_directory: c.voices_directory,
            cache_directory: c.cache_directory,
            model_version: c.model_version,
            timeout: Duration::from_secs(c.timeout_seconds),
        }
    } else {
        KokoroConfig {
            python: PathBuf::new(),
            runtime_script: PathBuf::new(),
            model_config: PathBuf::new(),
            model_weights: PathBuf::new(),
            voices_directory: PathBuf::new(),
            cache_directory: parent.join("system-preview-cache"),
            model_version: "unconfigured-English-provider".into(),
            timeout: Duration::from_secs(180),
        }
    };
    let mut provider = LocalSpeechProvider::new(config);
    if let Some(path) = &args.piper {
        provider = provider.with_piper_config_path(path.clone());
    }
    Ok(provider)
}
fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), Failure> {
    let bytes = serde_json::to_vec_pretty(value)?;
    if bytes.len() > 1024 * 1024 {
        return Err(fail("report exceeds1MiB"));
    }
    use std::io::Write;
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
    f.write_all(&bytes)?;
    f.sync_all()?;
    Ok(())
}
fn run(started: Instant) -> Result<serde_json::Value, Failure> {
    let args = arguments()?;
    let cancel = CancellationToken::new();
    let _timer = Timer::new(started, args.deadline, cancel.clone());
    let project = fs::canonicalize(&args.project)?;
    let root = project
        .parent()
        .ok_or_else(|| fail("project has no directory"))?;
    let parent = fs::canonicalize(
        args.output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    if args.output.exists() {
        return Err(fail("output directory must not exist"));
    }
    let bytes = read(&project, 8 * 1024 * 1024)?;
    let document = if project.extension().is_some_and(|x| x == "kilsat") {
        motion_project::decode(&bytes)?
    } else {
        motion_project::ProjectDocument::new(std::str::from_utf8(&bytes)?)?
    };
    let mut script = motion_script::parse(&document.source)?;
    if args.retime
        && script.scenes.iter().any(|s| {
            s.events
                .iter()
                .filter(|e| matches!(e, Event::Speak { .. }))
                .count()
                > 1
        })
    {
        return Err(fail("--retime-dialogue supports at most one Speak per scene; use measured shot direction for multiple turns"));
    }
    let jobs = motion_voice::dialogue_jobs(&script, root)?;
    if jobs.len() > 128 {
        return Err(fail("production export permits at most128dialogue jobs"));
    }
    let private_system = jobs.iter().any(|j| {
        j.imported_audio.is_none()
            && j.request.voice.language == "fi-fi"
            && !j.request.voice.preset.starts_with("piper_")
    });
    if private_system && !args.private_system {
        return Err(fail("Apple system voices require explicit --private-system-preview and are not publication-approved"));
    }
    if args.kokoro.is_none()
        && jobs
            .iter()
            .any(|j| j.imported_audio.is_none() && j.request.voice.language != "fi-fi")
    {
        return Err(fail(
            "English synthesis requires explicit --kokoro-config JSON or imported WAV",
        ));
    }
    let provider = provider(&args, &parent)?;
    let input_dir = parent.join(format!(
        ".kilsat-pipeline-input-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir(&input_dir)?;
    let inputs = Inputs(input_dir);
    let mut measured = BTreeMap::new();
    let mut source_audio = Vec::new();
    let mut sample_work = 0u64;
    for job in &jobs {
        check(started, &args, &cancel)?;
        let (path, was_imported) = match &job.imported_audio {
            Some(path) => (path.clone(), true),
            None => (
                provider
                    .synthesize_cancellable(job.request.clone(), &cancel)?
                    .audio_path,
                false,
            ),
        };
        let audio = pcm(&path)?;
        sample_work = sample_work
            .checked_add(audio.frames() as u64)
            .ok_or_else(|| fail("speech sample overflow"))?;
        if sample_work > 24_000 * 120 {
            return Err(fail("total speech exceeds120s source budget"));
        }
        let snapshot = inputs.0.join(format!("dialogue-{:04}.wav", job.id));
        motion_audio::write_wav(&snapshot, &audio)?;
        measured.insert(job.id, (snapshot, audio.frames() as u64));
        source_audio.push(serde_json::json!({"dialogue_id":job.id,"scene":job.scene,"actor":job.actor,"language":job.request.voice.language,
            "preset":job.request.voice.preset,"imported":was_imported,"sample_rate":24_000,"sample_count":audio.frames(),
            "pcm_sha256":motion_lipsync::rhubarb::pcm_identity(&audio)?}));
    }
    let speech_seconds = started.elapsed().as_secs_f64();
    let mut id = 0u64;
    let mut scene_durations = Vec::new();
    for scene in &mut script.scenes {
        let before = scene.duration;
        for event in &mut scene.events {
            if let Event::Speak { at, audio, .. } = event {
                let (snapshot, samples) = measured
                    .get(&id)
                    .ok_or_else(|| fail("missing measured dialogue"))?;
                *audio = Some(
                    snapshot
                        .to_str()
                        .ok_or_else(|| fail("speech path is not UTF-8"))?
                        .into(),
                );
                if args.retime {
                    scene.duration = scene.duration.max(*at + *samples as f64 / 24_000.0 + 0.25);
                }
                id += 1;
            }
        }
        scene_durations.push(serde_json::json!({"scene":scene.id,"authored_seconds":before,"prepared_seconds":scene.duration}));
    }
    script.validate()?;
    let timeline = motion_scene::SequenceTimeline::compile(&script)?;
    if timeline.total_samples() > 24_000 * 60 {
        return Err(fail("real speech extends the project beyond60seconds; split or edit dialogue, never truncate"));
    }
    let mut art = ArtContext {
        document: document.art.clone(),
        images: BTreeMap::new(),
        screen_videos: Default::default(),
    };
    for asset in &document.assets {
        if asset.kind == motion_project::AssetKind::Image {
            check(started, &args, &cancel)?;
            let path = motion_project::resolve_asset(&project, asset)?;
            let image = motion_art::load_image(&path)?.reserve_in(motion_jobs::process_budget())?;
            art.images.insert(asset.id.clone(), image);
        }
    }
    // Screen-video cues: decode each recording once into the content-addressed
    // frame cache; export then streams the frames it shows, one at a time.
    let ffprobe = args.ffmpeg.with_file_name("ffprobe");
    let video_root = fs::canonicalize(root)?;
    for cue in script.scenes.iter().flat_map(|s| &s.composition.screens) {
        let motion_script::ScreenSource::Video {
            asset,
            fps,
            duration,
        } = &cue.source
        else {
            continue;
        };
        if art.screen_videos.contains_key(asset) {
            continue;
        }
        check(started, &args, &cancel)?;
        let source = fs::canonicalize(video_root.join(asset))?;
        if !source.starts_with(&video_root) || !source.is_file() {
            return Err(fail(
                "screen video must be a file inside the project directory",
            ));
        }
        let color =
            motion_export::screen_video::probe_color(&ffprobe, &source, Duration::from_secs(30))?;
        let request = motion_export::screen_video::ScreenVideoRequest {
            source,
            fps: *fps,
            duration: *duration,
            // Review renders use the preview proxy; final export decodes full size.
            max_width: args
                .review
                .map(|_| motion_export::screen_video::SCREEN_PROXY_WIDTH),
        };
        let decoded = motion_export::screen_video::decode_screen_video(
            &args.ffmpeg,
            color,
            &request,
            &parent.join("screen-video-cache"),
            &cancel,
            Duration::from_secs(args.deadline),
        )?;
        eprintln!(
            "decoded screen video {asset}: {} frames ({:?})",
            decoded.frames, decoded.color
        );
        art.screen_videos.insert(asset.clone(), decoded);
    }
    let alignments: BTreeMap<u64, motion_lipsync::cache::AlignmentInput> =
        if document.timing.scene.trim().is_empty() {
            BTreeMap::new()
        } else {
            serde_json::from_str(&document.timing.scene)?
        };
    if alignments.keys().any(|id| !measured.contains_key(id)) {
        return Err(fail("project timing names an unknown dialogue"));
    }
    let alignment_cache =
        motion_lipsync::cache::AlignmentCache::new(parent.join("alignment-cache"));
    let mut rendered = Vec::new();
    let mut diagnostics = Vec::new();
    let mut alignment_statuses = BTreeMap::new();
    for index in 0..script.scenes.len() {
        check(started, &args, &cancel)?;
        let scene_inputs: BTreeMap<_, _> = alignments
            .iter()
            .filter(|(id, _)| {
                jobs.iter()
                    .any(|job| job.id == **id && job.scene == script.scenes[index].id)
            })
            .map(|(id, input)| (*id, input.clone()))
            .collect();
        let prepared = motion_voice::scene_audio::prepare_scene_with_alignment_inputs(
            &script,
            index,
            root,
            &provider,
            &scene_inputs,
            &alignment_cache,
            &cancel,
            |_| {},
        )?;
        diagnostics.extend(prepared.prepared.diagnostics.iter().map(|d| d.message()));
        alignment_statuses.extend(
            prepared
                .alignment_statuses
                .into_iter()
                .map(|(id, status)| (id, format!("{status:?}"))),
        );
        rendered.push(prepared.prepared.rendered);
    }
    let preparation_seconds = started.elapsed().as_secs_f64();
    check(started, &args, &cancel)?;
    let remaining = Duration::from_secs(args.deadline).saturating_sub(started.elapsed());
    if remaining < Duration::from_secs(1) {
        return Err(fail("no whole-pipeline deadline remains for export"));
    }
    let mut profile = StreamingProfile::full_hd_24();
    profile.deadline = remaining;
    if let Some((width, height)) = args.review {
        profile = profile.with_review_size(width, height)?;
    }
    let mut output = Output {
        path: args.output.clone(),
        created: false,
        complete: false,
    };
    let streaming = motion_export::export_mp4_sequence_streaming(
        &script,
        &rendered,
        Some(&art),
        &args.output,
        &args.ffmpeg,
        &profile,
        &cancel,
        |p| {
            if p.completed_frames % 120 == 0 {
                eprintln!("rendered {}/{} frames", p.completed_frames, p.total_frames);
            }
        },
        document.caption_mode,
    )?;
    output.created = true;
    if args.review_frames {
        let mut retained = 0usize;
        for (index, span) in timeline.spans().iter().enumerate() {
            check(started, &args, &cancel)?;
            let sample = span.start_sample + span.duration_samples / 2;
            let frame = motion_export::render_sequence_frame(
                &script,
                &rendered,
                Some(&art),
                sample,
                640,
                &cancel,
            )?;
            let pixmap = resvg::tiny_skia::Pixmap::from_vec(
                frame.rgba,
                resvg::tiny_skia::IntSize::from_wh(frame.width, frame.height)
                    .ok_or_else(|| fail("invalid review frame dimensions"))?,
            )
            .ok_or_else(|| fail("invalid review frame"))?;
            let png = pixmap.encode_png()?;
            retained += png.len();
            if retained > 16 * 1024 * 1024 {
                return Err(fail("review PNG budget exceeded"));
            }
            fs::write(args.output.join(format!("review-{index:02}.png")), png)?;
        }
    }
    check(started, &args, &cancel)?;
    let elapsed = started.elapsed().as_secs_f64();
    let report = serde_json::json!({"schema_version":1,"status":"completed","project":project,"source_bytes":bytes.len(),
        "host_os":std::env::consts::OS,"host_arch":std::env::consts::ARCH,"environment_kind":if std::env::var_os("CI").is_some(){"ci"}else{"local_unverified"},
        "deadline_seconds":args.deadline,"elapsed_seconds":elapsed,"within_deadline":elapsed<=args.deadline as f64,"within_five_minutes":elapsed<=300.0,
        "speech_elapsed_seconds":speech_seconds,"preparation_elapsed_seconds":preparation_seconds,"streaming":streaming,
        "review":args.review.is_some(),"private_system_preview_used":private_system,"publication_approved":false,
        "artistic_quality":"unmeasured","lip_sync_quality":"unmeasured","alignment_statuses":alignment_statuses,"dialogue_audio":source_audio,"scene_durations":scene_durations,"diagnostics":diagnostics});
    write_json(&args.output.join("pipeline-report.json"), &report)?;
    check(started, &args, &cancel)?;
    output.complete = true;
    Ok(report)
}
fn main() {
    let started = Instant::now();
    match run(started) {
        Ok(report) => println!("{report}"),
        Err(error) => {
            eprintln!(
                "{}",
                serde_json::json!({"schema_version":1,"status":"failed","elapsed_seconds":started.elapsed().as_secs_f64(),"error":error.to_string()})
            );
            std::process::exit(1);
        }
    }
}
