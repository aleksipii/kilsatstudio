//! Bounded off-thread measured-audio directing. Provider duration metadata is not
//! trusted: timing comes only from a decoded, immutable project-local WAV copy.
use motion_script::auto_direction::{self, DirectionResult};
use motion_tts::{CancellationToken, SpeechProvider, SpeechRequest, VoiceSettings};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};
const MAX_WAV_BYTES: u64 = 8 * 1024 * 1024;
const MAX_JOB_TIME: Duration = Duration::from_secs(300);
static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

pub struct Request {
    pub generation: u64,
    pub source: String,
    pub dialogue: String,
    pub root: PathBuf,
    pub provider: Arc<dyn SpeechProvider>,
    /// One-based physical input line numbers, including blank lines. Paths may
    /// be absolute or project relative; imports take precedence over synthesis.
    pub imported_audio: BTreeMap<usize, PathBuf>,
    pub cancel: CancellationToken,
}
#[derive(Debug, Clone)]
pub struct VerifiedAudio {
    pub source_line: usize,
    pub actor: String,
    pub relative_path: String,
    pub sample_count: u64,
    pub wav_sha256: String,
}
pub struct Ready {
    pub original_source: String,
    pub result: DirectionResult,
    pub audio: Vec<VerifiedAudio>,
    /// Speech preparation and directing only, not rendering/export elapsed time.
    pub elapsed: Duration,
}
#[allow(clippy::large_enum_variant)]
pub enum Update {
    Progress {
        generation: u64,
        completed: usize,
        total: usize,
        message: String,
    },
    Finished {
        generation: u64,
        result: Result<Ready, String>,
    },
}
pub struct Worker {
    sender: mpsc::SyncSender<Request>,
    updates: mpsc::Receiver<Update>,
}
impl Worker {
    pub fn new() -> Self {
        let (sender, requests) = mpsc::sync_channel::<Request>(1);
        let (progress, updates) = mpsc::sync_channel(8);
        thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                let generation = request.generation;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    prepare(request, MAX_JOB_TIME, |completed, total, message| {
                        let _ = progress.try_send(Update::Progress {
                            generation,
                            completed,
                            total,
                            message,
                        });
                    })
                }))
                .unwrap_or_else(|_| {
                    Err(
                        "Automatic direction worker failed; project contents were not changed"
                            .into(),
                    )
                });
                if progress
                    .send(Update::Finished { generation, result })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self { sender, updates }
    }
    pub fn submit(&self, request: Request) -> Result<(), String> {
        self.sender
            .try_send(request)
            .map_err(|e| format!("Automatic direction queue: {e}"))
    }
    pub fn poll(&self) -> Option<Update> {
        self.updates.try_recv().ok()
    }
}

struct Deadline {
    stop: Option<mpsc::SyncSender<()>>,
    join: Option<thread::JoinHandle<()>>,
}
impl Deadline {
    fn start(limit: Duration, cancel: CancellationToken) -> Self {
        let (tx, rx) = mpsc::sync_channel(1);
        let join = thread::spawn(move || {
            if matches!(rx.recv_timeout(limit), Err(mpsc::RecvTimeoutError::Timeout)) {
                cancel.cancel();
            }
        });
        Self {
            stop: Some(tx),
            join: Some(join),
        }
    }
}
impl Drop for Deadline {
    fn drop(&mut self) {
        if let Some(tx) = self.stop.take() {
            let _ = tx.try_send(());
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
fn check(cancel: &CancellationToken, started: Instant, limit: Duration) -> Result<(), String> {
    if started.elapsed() >= limit {
        return Err("Automatic direction preparation deadline exceeded".into());
    }
    cancel.check().map_err(|e| e.to_string())
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn decoded_snapshot(
    path: &Path,
    cancel: &CancellationToken,
    started: Instant,
    limit: Duration,
) -> Result<(Vec<u8>, u64), String> {
    check(cancel, started, limit)?;
    let metadata =
        fs::symlink_metadata(path).map_err(|e| format!("Speech WAV {}: {e}", path.display()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_WAV_BYTES {
        return Err("Speech WAV must be a bounded regular file, not a symlink".into());
    }
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_WAV_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_WAV_BYTES {
        return Err("Speech WAV exceeds8MiB".into());
    }
    let mut reader = hound::WavReader::new(Cursor::new(bytes.as_slice()))
        .map_err(|e| format!("Invalid speech WAV: {e}"))?;
    let spec = reader.spec();
    let frames = u64::from(reader.duration());
    if spec.channels != 1
        || spec.sample_rate != 24_000
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
        || frames == 0
        || frames > auto_direction::MAX_DURATION_SAMPLES
    {
        return Err("Speech must be nonempty24kHz mono PCM16 WAV, at most60seconds; convert before importing".into());
    }
    let mut decoded = 0u64;
    for sample in reader.samples::<i16>() {
        sample.map_err(|e| format!("Truncated/corrupt speech PCM: {e}"))?;
        decoded += 1;
        if decoded > frames {
            return Err("WAV contains excess PCM".into());
        }
        if decoded % 8192 == 0 {
            check(cancel, started, limit)?;
        }
    }
    if decoded != frames {
        return Err("WAV header length differs from decoded PCM".into());
    }
    check(cancel, started, limit)?;
    Ok((bytes, frames))
}
fn safe_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(format!(
            "{} must be a regular project directory, not a symlink",
            path.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|e| e.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn publish(
    root: &Path,
    bytes: &[u8],
    digest: &str,
    cancel: &CancellationToken,
    started: Instant,
    limit: Duration,
) -> Result<String, String> {
    let assets = root.join("assets");
    safe_directory(&assets)?;
    let directory = assets.join("auto-direction");
    safe_directory(&directory)?;
    let name = format!("{digest}.wav");
    let destination = directory.join(&name);
    let verify_existing = || -> Result<(), String> {
        let (existing, _) = decoded_snapshot(&destination, cancel, started, limit)?;
        if hash(&existing) != digest {
            return Err("Existing speech snapshot is corrupt; no file was overwritten".into());
        }
        Ok(())
    };
    if destination.try_exists().map_err(|e| e.to_string())? {
        verify_existing()?;
    } else {
        let temporary = Temporary(directory.join(format!(
            ".pending-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        )));
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary.0)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        check(cancel, started, limit)?;
        match fs::hard_link(&temporary.0, &destination) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => verify_existing()?,
            Err(error) => {
                return Err(format!(
                    "Atomic speech snapshot publication failed: {error}"
                ))
            }
        }
    }
    Ok(format!("assets/auto-direction/{name}"))
}
fn prepare(
    request: Request,
    limit: Duration,
    mut progress: impl FnMut(usize, usize, String),
) -> Result<Ready, String> {
    let started = Instant::now();
    if limit.is_zero() || limit > MAX_JOB_TIME {
        return Err("Automatic direction deadline must be within1..300seconds".into());
    }
    check(&request.cancel, started, limit)?;
    if request.source.len() > 100_000 {
        return Err("Screenplay exceeds100KB".into());
    }
    let script = motion_script::parse(&request.source).map_err(|e| e.to_string())?;
    let lines = auto_direction::parse_dialogue(&request.dialogue, &script)?;
    if !request.root.is_absolute() || !request.root.is_dir() {
        return Err(
            "Save/select an existing absolute project folder before preparing speech".into(),
        );
    }
    let root = fs::canonicalize(&request.root).map_err(|e| e.to_string())?;
    let line_ids: BTreeSet<_> = lines.iter().map(|line| line.source_line).collect();
    if request
        .imported_audio
        .keys()
        .any(|line| !line_ids.contains(line))
    {
        return Err("Imported WAV map contains an unknown physical dialogue line".into());
    }
    let _deadline = Deadline::start(limit, request.cancel.clone());
    let mut audio = Vec::with_capacity(lines.len());
    let mut counts = Vec::with_capacity(lines.len());
    let mut paths = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        check(&request.cancel, started, limit)?;
        let path = if let Some(path) = request.imported_audio.get(&line.source_line) {
            if path.is_absolute() {
                path.clone()
            } else {
                root.join(path)
            }
        } else {
            let voice = script.characters[&line.actor]
                .voice
                .as_ref()
                .ok_or_else(|| {
                    format!(
                        "Line{}({}): select a voice or import a WAV",
                        line.source_line, line.actor
                    )
                })?;
            let result = request
                .provider
                .synthesize_cancellable(
                    SpeechRequest {
                        text: line.text.clone(),
                        voice: VoiceSettings {
                            preset: voice.preset.clone(),
                            language: voice.language.clone(),
                            speed: voice.speed,
                            style: line.emotion.clone(),
                        },
                    },
                    &request.cancel,
                )
                .map_err(|e| format!("Line{}({}): {e}", line.source_line, line.actor))?;
            result.audio_path
        };
        let (bytes, frames) = decoded_snapshot(&path, &request.cancel, started, limit)?;
        counts.push(frames);
        // Reject excessive actual speech before publishing another snapshot.
        auto_direction::time_lines(&lines[..=index], Some(&counts))?;
        let digest = hash(&bytes);
        let relative = publish(&root, &bytes, &digest, &request.cancel, started, limit)?;
        audio.push(VerifiedAudio {
            source_line: line.source_line,
            actor: line.actor.clone(),
            relative_path: relative.clone(),
            sample_count: frames,
            wav_sha256: digest,
        });
        paths.push(relative);
        progress(
            index + 1,
            lines.len(),
            format!("Measured dialogue{} of{}", index + 1, lines.len()),
        );
    }
    check(&request.cancel, started, limit)?;
    let mut result =
        auto_direction::direct_with_audio(&request.source, &request.dialogue, &counts, &paths)?;
    check(&request.cancel, started, limit)?;
    result
        .diagnostics
        .retain(|message| !message.starts_with("Timing uses caller-supplied"));
    result.diagnostics.insert(0,"Desktop worker verified complete decoded PCM16/mono24kHz WAV lengths and immutable SHA256-bound snapshots. This verifies timing inputs, not phoneme/lip-sync, voice rights or artistic quality. Undo does not remove the retained audio files.".into());
    Ok(Ready {
        original_source: request.source,
        result,
        audio,
        elapsed: started.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use motion_tts::{SpeechError, SpeechResult};
    const SOURCE: &str = include_str!("../../../examples/hello-emma.yaml");
    struct Project(PathBuf);
    impl Project {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "direction-test-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Project {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn wav(frames: u32) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(
                &mut cursor,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 24000,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for index in 0..frames {
                writer
                    .write_sample(if index % 2 == 0 { 1000i16 } else { -1000 })
                    .unwrap();
            }
            writer.finalize().unwrap();
        }
        cursor.into_inner()
    }
    struct MetadataLies {
        path: PathBuf,
        calls: Arc<AtomicU64>,
    }
    impl SpeechProvider for MetadataLies {
        fn synthesize(&self, _: SpeechRequest) -> Result<SpeechResult, SpeechError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(SpeechResult {
                audio_path: self.path.clone(),
                sample_rate: 1,
                sample_count: u64::MAX,
                cache_key: "not-trusted".into(),
                phonemes: String::new(),
                cached: false,
            })
        }
    }
    fn request(project: &Project, path: PathBuf) -> (Request, Arc<AtomicU64>) {
        let calls = Arc::new(AtomicU64::new(0));
        (
            Request {
                generation: 17,
                source: SOURCE.into(),
                dialogue: "EMMA: Hei!\n\nALEX: Moi.".into(),
                root: project.0.clone(),
                provider: Arc::new(MetadataLies {
                    path,
                    calls: calls.clone(),
                }),
                imported_audio: BTreeMap::new(),
                cancel: CancellationToken::new(),
            },
            calls,
        )
    }
    #[test]
    fn actual_pcm_overrides_provider_metadata_and_snapshot_survives_original_changes() {
        let project = Project::new();
        let path = project.0.join("voice.wav");
        let bytes = wav(12000);
        fs::write(&path, &bytes).unwrap();
        let (request, calls) = request(&project, path.clone());
        let ready = prepare(request, MAX_JOB_TIME, |_, _, _| {}).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        assert_eq!(ready.audio.len(), 2);
        assert!(ready.audio.iter().all(|a| a.sample_count == 12000));
        assert_eq!(ready.audio[1].source_line, 3);
        assert_eq!(ready.original_source, SOURCE);
        fs::write(path, wav(20)).unwrap();
        let snapshot = project.0.join(&ready.audio[0].relative_path);
        assert_eq!(fs::read(snapshot).unwrap(), bytes);
        let script = motion_script::parse(&ready.result.source).unwrap();
        let speech: Vec<_> = script.scenes[ready.result.first_scene..]
            .iter()
            .flat_map(|s| &s.events)
            .filter_map(|e| {
                if let motion_script::Event::Speak { audio, .. } = e {
                    audio.clone()
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(speech.len(), 2);
        assert!(speech
            .iter()
            .all(|p| p.starts_with("assets/auto-direction/")));
    }
    #[test]
    fn imports_precede_provider_and_unknown_line_maps_fail_before_inference() {
        let project = Project::new();
        let path = project.0.join("voice.wav");
        fs::write(&path, wav(2400)).unwrap();
        let (mut request, calls) = request(&project, project.0.join("must-not-run.wav"));
        request.imported_audio = BTreeMap::from([(1, path.clone()), (3, path.clone())]);
        assert!(prepare(request, MAX_JOB_TIME, |_, _, _| {}).is_ok());
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        let (mut request, calls) = self::request(&project, path);
        request.imported_audio.insert(2, "unknown.wav".into());
        assert!(prepare(request, MAX_JOB_TIME, |_, _, _| {}).is_err());
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn truncated_and_oversized_wav_headers_are_rejected_before_large_allocation() {
        let project = Project::new();
        let path = project.0.join("bad.wav");
        let mut bytes = wav(2400);
        bytes.pop();
        fs::write(&path, bytes).unwrap();
        let (request, _) = request(&project, path.clone());
        assert!(prepare(request, MAX_JOB_TIME, |_, _, _| {}).is_err());
        let mut bytes = wav(2400);
        let offset = bytes.windows(4).position(|chunk| chunk == b"data").unwrap() + 4;
        bytes[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        fs::write(&path, bytes).unwrap();
        let (request, _) = self::request(&project, path);
        assert!(prepare(request, MAX_JOB_TIME, |_, _, _| {}).is_err());
    }
    #[test]
    fn cancelled_and_zero_deadline_requests_never_start_provider() {
        let project = Project::new();
        let (request, calls) = request(&project, "never.wav".into());
        request.cancel.cancel();
        assert!(prepare(request, MAX_JOB_TIME, |_, _, _| {}).is_err());
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        let (request, calls) = self::request(&project, "never.wav".into());
        assert!(prepare(request, Duration::ZERO, |_, _, _| {}).is_err());
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }
}
