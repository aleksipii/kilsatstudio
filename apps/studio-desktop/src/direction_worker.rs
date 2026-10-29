//! Bounded off-thread measured-audio directing. Provider duration metadata is not
//! trusted: timing comes only from a decoded, immutable project-local WAV copy.
use crate::project_history::MAX_SOURCE_BYTES;
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
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
const MAX_WAV_BYTES: u64 = 8 * 1024 * 1024;
const MAX_JOB_TIME: Duration = Duration::from_secs(300);
/// A whole story may have up to 200 lines; synthesis stays bounded and cancellable.
const MAX_STORY_JOB_TIME: Duration = Duration::from_secs(30 * 60);
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
    /// Story text (`kilsat_story: 1`). When present, every line is synthesized
    /// with its character's voice and the story is recompiled on the measured
    /// lengths; `source`, `dialogue` and `imported_audio` are not used.
    pub story: Option<String>,
    /// The open project's own characters and sets, offered to the story.
    pub story_project: crate::story_import::ProjectContent,
}
/// A story compiled on measured speech, ready to replace the project.
pub struct StoryReady {
    pub built: crate::story_import::Built,
    pub elapsed: Duration,
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
    StoryFinished {
        generation: u64,
        result: Result<StoryReady, String>,
    },
}
pub struct Worker {
    sender: mpsc::SyncSender<Request>,
    updates: mpsc::Receiver<Update>,
    pending: Arc<Mutex<Vec<CancellationToken>>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Ok(pending) = self.pending.lock() {
            for cancel in pending.iter() {
                cancel.cancel();
            }
        }
    }
}
impl Worker {
    pub fn new() -> Self {
        let (sender, requests) = mpsc::sync_channel::<Request>(1);
        let (progress, updates) = mpsc::sync_channel(8);
        let pending = Arc::new(Mutex::new(Vec::<CancellationToken>::new()));
        let worker_pending = Arc::clone(&pending);
        thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                let generation = request.generation;
                let report = |completed, total, message| {
                    let _ = progress.try_send(Update::Progress {
                        generation,
                        completed,
                        total,
                        message,
                    });
                };
                let crashed = "Puheohjauksen valmistelija keskeytyi; käsikirjoitusta ei muutettu";
                let update = if request.story.is_some() {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        prepare_story(request, MAX_STORY_JOB_TIME, report)
                    }))
                    .unwrap_or_else(|_| Err(crashed.into()));
                    Update::StoryFinished { generation, result }
                } else {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        prepare(request, MAX_JOB_TIME, report)
                    }))
                    .unwrap_or_else(|_| Err(crashed.into()));
                    Update::Finished { generation, result }
                };
                if let Ok(mut pending) = worker_pending.lock() {
                    if !pending.is_empty() {
                        pending.remove(0);
                    }
                }
                if progress.send(update).is_err() {
                    break;
                }
            }
        });
        Self {
            sender,
            updates,
            pending,
        }
    }
    pub fn submit(&self, request: Request) -> Result<(), String> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "Puheohjauksen tila ei ole saatavilla")?;
        pending.push(request.cancel.clone());
        match self.sender.try_send(request) {
            Ok(()) => Ok(()),
            Err(error) => {
                pending.pop();
                Err(format!("Puheohjauksen työjono: {error}"))
            }
        }
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
        return Err("Puheohjauksen valmistelun aikaraja ylittyi".into());
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
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| format!("Puheen WAV-tiedosto {}: {e}", path.display()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_WAV_BYTES {
        return Err(
            "Puheen WAVin on oltava tavallinen, enintään 8 MiB:n tiedosto. Symbolinen linkki ei kelpaa"
                .into(),
        );
    }
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_WAV_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_WAV_BYTES {
        return Err("Puheen WAV ylittää 8 MiB:n rajan".into());
    }
    let mut reader = hound::WavReader::new(Cursor::new(bytes.as_slice()))
        .map_err(|e| format!("Virheellinen puheen WAV-tiedosto: {e}"))?;
    let spec = reader.spec();
    let frames = u64::from(reader.duration());
    if spec.channels != 1
        || spec.sample_rate != 24_000
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
        || frames == 0
        || frames > auto_direction::MAX_DURATION_SAMPLES
    {
        return Err("Puheen WAVin on sisällettävä 24 kHz:n mono PCM16 -ääntä, enintään 60 sekuntia. Muunna äänite ennen tuontia".into());
    }
    let mut decoded = 0u64;
    for sample in reader.samples::<i16>() {
        sample.map_err(|e| format!("Puheen PCM-data on katkennut tai vioittunut: {e}"))?;
        decoded += 1;
        if decoded > frames {
            return Err("WAV sisältää ylimääräistä PCM-dataa".into());
        }
        if decoded.is_multiple_of(8192) {
            check(cancel, started, limit)?;
        }
    }
    if decoded != frames {
        return Err("WAV-otsakkeen pituus poikkeaa puretun PCM-datan pituudesta".into());
    }
    check(cancel, started, limit)?;
    Ok((bytes, frames))
}
fn safe_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(format!(
            "{}: projektikansion on oltava tavallinen kansio, ei symbolinen linkki",
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
            return Err("Aiempi puheen äänikopio on vioittunut; tiedostoja ei korvattu".into());
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
            Err(error) => return Err(format!("Äänikopion viimeistely epäonnistui: {error}")),
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
        return Err("Puheohjauksen aikarajan on oltava 1–300 sekuntia".into());
    }
    check(&request.cancel, started, limit)?;
    if request.source.len() > MAX_SOURCE_BYTES {
        return Err("Käsikirjoitus ylittää 256 KiB:n rajan".into());
    }
    let script = motion_script::parse(&request.source).map_err(|e| e.to_string())?;
    let lines = auto_direction::parse_dialogue(&request.dialogue, &script)?;
    if !request.root.is_absolute() || !request.root.is_dir() {
        return Err(
            "Tallenna tai valitse olemassa olevan projektikansion koko polku ennen puheen valmistelua".into(),
        );
    }
    let root = fs::canonicalize(&request.root).map_err(|e| e.to_string())?;
    let line_ids: BTreeSet<_> = lines.iter().map(|line| line.source_line).collect();
    if request
        .imported_audio
        .keys()
        .any(|line| !line_ids.contains(line))
    {
        return Err(
            "WAV-tuontien luettelossa on tuntematon alkuperäisen vuorosanarivin numero".into(),
        );
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
                        "Rivi {} ({}): valitse puheääni tai tuo WAV-äänite",
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
                .map_err(|e| format!("Rivi {} ({}): {e}", line.source_line, line.actor))?;
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
            format!("Mitattu repliikki {}/{}", index + 1, lines.len()),
        );
    }
    check(&request.cancel, started, limit)?;
    let mut result =
        auto_direction::direct_with_audio(&request.source, &request.dialogue, &counts, &paths)?;
    check(&request.cancel, started, limit)?;
    result
        .diagnostics
        .retain(|message| !message.starts_with("Timing uses caller-supplied"));
    result.diagnostics.insert(0,"Puheohjaus tarkisti kokonaan purettujen 24 kHz:n mono PCM16 WAVien pituudet ja muuttumattomat SHA-256-tiivisteisiin sidotut äänikopiot. Tarkistus koskee ajoituksen syötteitä. Foneemien ja huulisynkan laatua, äänten käyttöoikeuksia tai taiteellista laatua ei arvioitu. Kumoa ei poista säilytettyjä äänitiedostoja.".into());
    Ok(Ready {
        original_source: request.source,
        result,
        audio,
        elapsed: started.elapsed(),
    })
}

fn prepare_story(
    request: Request,
    limit: Duration,
    mut progress: impl FnMut(usize, usize, String),
) -> Result<StoryReady, String> {
    let started = Instant::now();
    if limit.is_zero() || limit > MAX_STORY_JOB_TIME {
        return Err("Tarinan puheen valmistelun aikarajan on oltava enintään 30 minuuttia".into());
    }
    let text = request.story.as_deref().unwrap_or_default();
    let issues = |issues: Vec<motion_script::story::StoryIssue>| {
        let first: Vec<String> = issues.iter().take(5).map(ToString::to_string).collect();
        format!("Käsikirjoitusta ei hyväksytty: {}", first.join("; "))
    };
    let draft =
        crate::story_import::build_with(text, None, &request.story_project).map_err(issues)?;
    if !request.root.is_absolute() || !request.root.is_dir() {
        return Err(
            "Anna olemassa olevan projektikansion koko polku ennen puheen valmistelua".into(),
        );
    }
    let root = fs::canonicalize(&request.root).map_err(|e| e.to_string())?;
    let _deadline = Deadline::start(limit, request.cancel.clone());
    let lines = &draft.compilation.lines;
    let mut measured = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        check(&request.cancel, started, limit)?;
        let voice = draft.compilation.script.characters[&line.actor]
            .voice
            .as_ref()
            .ok_or_else(|| format!("{}: hahmolla ei ole puheääntä", line.actor))?;
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
            .map_err(|e| format!("Repliikki {} ({}): {e}", index + 1, line.actor))?;
        let (bytes, frames) =
            decoded_snapshot(&result.audio_path, &request.cancel, started, limit)?;
        let seconds = frames as f64 / auto_direction::SAMPLE_RATE as f64;
        if seconds > motion_script::story::MAX_LINE_SECONDS {
            return Err(format!(
                "Repliikki {} ({}) kestää {seconds:.1} s; jaa se lyhyemmiksi repliikeiksi (enintään {} s)",
                index + 1,
                line.actor,
                motion_script::story::MAX_LINE_SECONDS
            ));
        }
        let digest = hash(&bytes);
        let relative = publish(&root, &bytes, &digest, &request.cancel, started, limit)?;
        measured.push(motion_script::story::MeasuredLine {
            seconds,
            audio: Some(relative),
        });
        progress(
            index + 1,
            lines.len(),
            format!("Puhe {}/{}: {}", index + 1, lines.len(), line.actor),
        );
    }
    check(&request.cancel, started, limit)?;
    let built = crate::story_import::build_with(text, Some(&measured), &request.story_project)
        .map_err(issues)?;
    Ok(StoryReady {
        built,
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
    /// Speech whose length depends on the text, so timing must follow it.
    struct TextLength {
        dir: PathBuf,
        voices: Arc<Mutex<Vec<String>>>,
    }
    impl SpeechProvider for TextLength {
        fn synthesize(&self, request: SpeechRequest) -> Result<SpeechResult, SpeechError> {
            self.voices
                .lock()
                .unwrap()
                .push(request.voice.preset.clone());
            let frames = 24_000 + 2_400 * request.text.chars().count() as u32;
            let path = self.dir.join(format!("{frames}.wav"));
            fs::write(&path, wav(frames)).unwrap();
            Ok(SpeechResult {
                audio_path: path,
                sample_rate: 24_000,
                sample_count: u64::from(frames),
                cache_key: "k".into(),
                phonemes: String::new(),
                cached: false,
            })
        }
    }
    #[test]
    fn story_is_retimed_on_measured_speech_with_one_wav_per_line() {
        let project = Project::new();
        let scratch = Project::new();
        let voices = Arc::new(Mutex::new(Vec::new()));
        let story = include_str!("../../../examples/story/kahvitauko.yaml");
        let mut request = request(&project, PathBuf::new()).0;
        request.provider = Arc::new(TextLength {
            dir: scratch.0.clone(),
            voices: voices.clone(),
        });
        request.story = Some(story.into());
        let mut steps = 0;
        let ready = prepare_story(request, MAX_STORY_JOB_TIME, |_, _, _| steps += 1).unwrap();
        let compilation = &ready.built.compilation;
        assert_eq!(
            compilation.timing,
            motion_script::story::StoryTiming::Supplied
        );
        assert_eq!(steps, compilation.lines.len());
        assert_eq!(voices.lock().unwrap().len(), compilation.lines.len());
        assert!(voices
            .lock()
            .unwrap()
            .iter()
            .all(|v| v.starts_with("piper_")));
        for line in &compilation.lines {
            let expected = 1.0 + 0.1 * line.text.chars().count() as f64;
            assert!((line.duration - expected).abs() < 1e-9);
            let audio = line.audio.as_deref().unwrap();
            assert!(project.0.join(audio).is_file());
        }
        for pair in compilation.lines.windows(2) {
            if pair[0].scene == pair[1].scene {
                assert!(pair[0].at + pair[0].duration <= pair[1].at);
            }
        }
        let script = motion_script::parse(&ready.built.document.source).unwrap();
        let with_audio = script
            .scenes
            .iter()
            .flat_map(|scene| &scene.events)
            .filter(|e| matches!(e, motion_script::Event::Speak { audio: Some(_), .. }))
            .count();
        assert_eq!(with_audio, compilation.lines.len());
    }
    #[test]
    fn story_speech_needs_a_project_folder_and_a_valid_story() {
        let project = Project::new();
        let (mut request, calls) = request(&project, PathBuf::new());
        request.story = Some("kilsat_story: 1\n".into());
        assert!(prepare_story(request, MAX_STORY_JOB_TIME, |_, _, _| {}).is_err());
        let (mut request, _) = request_for(&project);
        request.root = PathBuf::from("relative");
        assert!(prepare_story(request, MAX_STORY_JOB_TIME, |_, _, _| {}).is_err());
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }
    fn request_for(project: &Project) -> (Request, Arc<AtomicU64>) {
        let (mut request, calls) = request(project, PathBuf::new());
        request.story = Some(include_str!("../../../examples/story/kahvitauko.yaml").into());
        (request, calls)
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
                story: None,
                story_project: Default::default(),
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
    #[test]
    fn dropping_worker_cancels_an_active_cooperative_provider() {
        struct WaitForCancel(Arc<std::sync::atomic::AtomicBool>);
        impl SpeechProvider for WaitForCancel {
            fn synthesize(&self, _: SpeechRequest) -> Result<SpeechResult, SpeechError> {
                Err(SpeechError("requires cancellation API".into()))
            }
            fn synthesize_cancellable(
                &self,
                _: SpeechRequest,
                cancel: &CancellationToken,
            ) -> Result<SpeechResult, SpeechError> {
                self.0.store(true, Ordering::Release);
                let deadline = Instant::now() + Duration::from_secs(2);
                while !cancel.is_cancelled() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(1));
                }
                Err(SpeechError("stopped".into()))
            }
        }
        let project = Project::new();
        let (mut request, _) = request(&project, "unused.wav".into());
        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
        request.provider = Arc::new(WaitForCancel(Arc::clone(&started)));
        let cancel = request.cancel.clone();
        let worker = Worker::new();
        worker.submit(request).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !started.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(started.load(Ordering::Acquire));
        drop(worker);
        assert!(cancel.is_cancelled());
    }
}
