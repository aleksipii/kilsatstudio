//! Screen-video import and preview decoding, never on the egui event loop.
//!
//! Two bounded lanes (one thread each, small `sync_channel` queues): imports
//! copy a recording into `<project>/assets/screens/` and probe its frame rate
//! and duration with ffprobe; decodes run `motion_export::screen_video` into
//! the 540 px proxy cache. [`ScreenVideos`] is the UI-side state machine: it
//! schedules what the selected scene needs, reports progress, cancels stale
//! work and hands finished caches to the preview's `ArtContext`, so preview
//! frames come from the same scene clock and frame numbering as export.
use motion_export::screen_video::{
    decode_screen_video, probe_color, DecodedScreenVideo, ScreenVideoRequest,
    MAX_SCREEN_VIDEO_BYTES, SCREEN_PROXY_WIDTH,
};
use motion_script::{MAX_SCREEN_FPS, MAX_SCREEN_VIDEO_SECONDS, SCREEN_VIDEO_EXTENSIONS};
use motion_tts::CancellationToken;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Project folder for imported screen recordings.
pub const SCREENS_DIR: &str = "assets/screens";
/// Proxy frame cache beside the project (content-addressed, safe to delete).
pub const PREVIEW_CACHE_DIR: &str = "screen-video-cache/preview";
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const DECODE_TIMEOUT: Duration = Duration::from_secs(600);
const QUEUE: usize = 4;

/// The ffprobe beside the configured FFmpeg (never a PATH search).
pub fn ffprobe_for(ffmpeg: &Path) -> PathBuf {
    ffmpeg.with_file_name("ffprobe")
}

/// A local tool must be an absolute path to an existing file.
pub fn tool_problem(ffmpeg: &str) -> Option<String> {
    let ffmpeg = Path::new(ffmpeg.trim());
    if ffmpeg.as_os_str().is_empty() {
        return Some(
            "FFmpeg puuttuu: anna paikallisen ffmpeg-ohjelman koko polku (esim. /opt/homebrew/bin/ffmpeg). Video näkyy esikatselussa vasta purun jälkeen.".into(),
        );
    }
    if !ffmpeg.is_absolute() || !ffmpeg.is_file() {
        return Some(format!(
            "FFmpeg puuttuu: {} ei ole olemassa oleva ohjelma (koko polku).",
            ffmpeg.display()
        ));
    }
    let ffprobe = ffprobe_for(ffmpeg);
    (!ffprobe.is_file()).then(|| {
        format!(
            "ffprobe puuttuu: odotettiin samaan kansioon kuin FFmpeg ({}).",
            ffprobe.display()
        )
    })
}

/// Checks a chosen recording before anything is copied.
pub fn validate_import_source(path: &Path) -> Result<u64, String> {
    if !path.is_absolute() {
        return Err("Anna videotiedoston koko polku.".into());
    }
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if !extension.is_some_and(|e| SCREEN_VIDEO_EXTENSIONS.contains(&e.as_str())) {
        return Err("Vain .mp4-, .mov- ja .m4v-videot kelpaavat.".into());
    }
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("Videota ei löydy: {e}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Valitse tavallinen videotiedosto, ei kansiota tai symbolista linkkiä.".into());
    }
    if metadata.len() == 0 || metadata.len() > MAX_SCREEN_VIDEO_BYTES {
        return Err("Video on tyhjä tai suurempi kuin 4 GiB.".into());
    }
    Ok(metadata.len())
}

/// ASCII file name for an imported recording: `<stem>.<ext>`.
pub fn import_file_name(source: &Path) -> String {
    let stem: String = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .take(64)
        .collect();
    let stem = stem.trim_matches('-');
    let stem = if stem.is_empty() { "video" } else { stem };
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "mp4".into());
    format!("{stem}.{extension}")
}

/// A recording copied into the project and probed.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    /// Project-relative cue asset, `assets/screens/<file>`.
    pub asset: String,
    pub fps: f64,
    pub duration: f64,
}

fn real_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "{} ei saa olla symbolinen linkki tai tiedosto.",
            path.display()
        ));
    }
    Ok(())
}

/// Copy `source` into `<root>/assets/screens/` (never overwriting) and probe
/// it. A recording already in that folder is reused without copying. A copy
/// whose probe fails is removed again. Blocking: run on the import lane.
pub fn import_video(
    root: &Path,
    source: &Path,
    ffprobe: &Path,
    cancel: &CancellationToken,
) -> Result<Imported, String> {
    let length = validate_import_source(source)?;
    let root = fs::canonicalize(root)
        .map_err(|e| format!("Projektikansiota {} ei löydy: {e}", root.display()))?;
    real_directory(&root)?;
    let screens = root.join(SCREENS_DIR);
    if let Ok(existing) = fs::canonicalize(source) {
        if existing.parent() == Some(screens.as_path()) {
            let name = existing
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or("Videon nimi ei kelpaa.")?;
            let (fps, duration) = probe_video(ffprobe, &existing, cancel)?;
            return Ok(Imported {
                asset: format!("{SCREENS_DIR}/{name}"),
                fps,
                duration,
            });
        }
    }
    for directory in [root.join("assets"), screens.clone()] {
        match fs::create_dir(&directory) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                return Err(format!(
                    "Kansiota {} ei voitu luoda: {e}",
                    directory.display()
                ))
            }
        }
        real_directory(&directory)?;
    }
    let name = import_file_name(source);
    let (stem, extension) = name.rsplit_once('.').unwrap_or((&name, "mp4"));
    let temp = screens.join(format!(".import-{}.tmp", std::process::id()));
    let copied = (|| -> Result<(), String> {
        let mut input = fs::File::open(source).map_err(|e| e.to_string())?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        let mut buffer = vec![0u8; 1024 * 1024];
        let mut written = 0u64;
        loop {
            if cancel.is_cancelled() {
                return Err("Tuonti peruttiin.".into());
            }
            let read = input.read(&mut buffer).map_err(|e| e.to_string())?;
            if read == 0 {
                break;
            }
            written += read as u64;
            if written > MAX_SCREEN_VIDEO_BYTES {
                return Err("Video kasvoi tuonnin aikana yli 4 GiB:n.".into());
            }
            std::io::Write::write_all(&mut output, &buffer[..read]).map_err(|e| e.to_string())?;
        }
        output.sync_all().map_err(|e| e.to_string())?;
        if written != length {
            return Err("Video muuttui tuonnin aikana; yritä uudelleen.".into());
        }
        Ok(())
    })();
    if let Err(error) = copied {
        let _ = fs::remove_file(&temp);
        return Err(format!("Videon kopiointi epäonnistui: {error}"));
    }
    // Hard-link to the first free name, so an existing recording is never replaced.
    let mut chosen = None;
    for n in 1..=999 {
        let file = if n == 1 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem}-{n}.{extension}")
        };
        match fs::hard_link(&temp, screens.join(&file)) {
            Ok(()) => {
                chosen = Some(file);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                let _ = fs::remove_file(&temp);
                return Err(format!("Videota ei voitu tallentaa projektiin: {e}"));
            }
        }
    }
    let _ = fs::remove_file(&temp);
    let file = chosen.ok_or("Kansiossa assets/screens on liikaa samannimisiä videoita.")?;
    let destination = screens.join(&file);
    match probe_video(ffprobe, &destination, cancel) {
        Ok((fps, duration)) => Ok(Imported {
            asset: format!("{SCREENS_DIR}/{file}"),
            fps,
            duration,
        }),
        Err(error) => {
            let _ = fs::remove_file(&destination);
            Err(error)
        }
    }
}

fn rate(value: &serde_json::Value) -> Option<f64> {
    let text = value.as_str()?;
    let (n, d) = text.split_once('/').unwrap_or((text, "1"));
    let (n, d) = (n.trim().parse::<f64>().ok()?, d.trim().parse::<f64>().ok()?);
    (d > 0.0 && n > 0.0).then_some(n / d)
}
fn seconds(value: &serde_json::Value) -> Option<f64> {
    value
        .as_str()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .or_else(|| value.as_f64())
        .filter(|d| d.is_finite() && *d > 0.0)
}

/// `(fps, duration)` from `ffprobe -of json` output, rounded for the screenplay
/// (fps to 0.001, duration down to whole milliseconds so no frame is invented).
pub fn parse_probe(json: &[u8]) -> Result<(f64, f64), String> {
    let value: serde_json::Value =
        serde_json::from_slice(json).map_err(|e| format!("ffprobe-tulos ei kelpaa: {e}"))?;
    let stream = &value["streams"][0];
    if stream.is_null() {
        return Err("Tiedostossa ei ole videoraitaa.".into());
    }
    let fps = rate(&stream["avg_frame_rate"])
        .or_else(|| rate(&stream["r_frame_rate"]))
        .map(|f| (f * 1000.0).round() / 1000.0)
        .filter(|f| f.is_finite() && *f > 0.0 && *f <= MAX_SCREEN_FPS)
        .ok_or_else(|| format!("Videon kuvanopeus puuttuu tai ylittää {MAX_SCREEN_FPS} fps."))?;
    let duration = seconds(&stream["duration"])
        .or_else(|| seconds(&value["format"]["duration"]))
        .map(|d| (d * 1000.0).floor() / 1000.0)
        .filter(|d| *d > 0.0)
        .ok_or("Videon kesto puuttuu.")?;
    if duration > MAX_SCREEN_VIDEO_SECONDS {
        return Err(format!(
            "Video on pidempi kuin {MAX_SCREEN_VIDEO_SECONDS} sekuntia."
        ));
    }
    Ok((fps, duration))
}

/// Probe frame rate and duration with ffprobe (timeout, cancellable).
pub fn probe_video(
    ffprobe: &Path,
    source: &Path,
    cancel: &CancellationToken,
) -> Result<(f64, f64), String> {
    if !ffprobe.is_absolute() || !ffprobe.is_file() {
        return Err(format!(
            "ffprobe puuttuu: {} (sama kansio kuin FFmpeg).",
            ffprobe.display()
        ));
    }
    let mut child = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=avg_frame_rate,r_frame_rate,duration:format=duration",
            "-of",
            "json",
        ])
        .arg(source)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("ffprobea ei voitu käynnistää: {e}"))?;
    let reader = child.stdout.take().map(|stdout| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout.take(1024 * 1024).read_to_end(&mut bytes);
            bytes
        })
    });
    let started = Instant::now();
    let status = loop {
        let stop = if cancel.is_cancelled() {
            Some("Tuonti peruttiin.".to_owned())
        } else if started.elapsed() > PROBE_TIMEOUT {
            Some("ffprobe ei vastannut 30 sekunnissa.".to_owned())
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
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
    };
    let output = reader.and_then(|r| r.join().ok()).unwrap_or_default();
    if !status.success() {
        return Err("ffprobe ei tunnistanut videota.".into());
    }
    parse_probe(&output)
}

/// One decoded proxy is identified by the cue's asset, fps and duration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VideoKey {
    pub asset: String,
    fps: u64,
    duration: u64,
}
impl VideoKey {
    pub fn new(asset: &str, fps: f64, duration: f64) -> Self {
        Self {
            asset: asset.to_owned(),
            fps: fps.to_bits(),
            duration: duration.to_bits(),
        }
    }
    pub fn fps(&self) -> f64 {
        f64::from_bits(self.fps)
    }
    pub fn duration(&self) -> f64 {
        f64::from_bits(self.duration)
    }
    /// Video keys of every video cue in `scene`.
    pub fn of_scene(scene: &motion_script::Scene) -> Vec<Self> {
        let mut keys: Vec<Self> = scene
            .composition
            .screens
            .iter()
            .filter_map(|cue| match &cue.source {
                motion_script::ScreenSource::Video {
                    asset,
                    fps,
                    duration,
                } => Some(Self::new(asset, *fps, *duration)),
                _ => None,
            })
            .collect();
        keys.sort();
        keys.dedup();
        keys
    }
}

/// Project-relative `asset` resolved inside `root`, or an error.
pub fn resolve_in_project(root: &Path, asset: &str) -> Result<PathBuf, String> {
    if !Path::new(asset)
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err(format!("{asset}: polun pitää olla projektin sisällä."));
    }
    let root = fs::canonicalize(root).map_err(|e| format!("Projektikansio: {e}"))?;
    let path =
        fs::canonicalize(root.join(asset)).map_err(|e| format!("Videota {asset} ei löydy: {e}"))?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err(format!(
            "{asset}: videon pitää olla tiedosto projektikansion sisällä."
        ));
    }
    Ok(path)
}

struct DecodeJob {
    key: VideoKey,
    root: PathBuf,
    ffmpeg: PathBuf,
    cancel: CancellationToken,
    progress: Arc<AtomicU64>,
}
struct ImportJob {
    id: u64,
    root: PathBuf,
    source: PathBuf,
    ffprobe: PathBuf,
    cancel: CancellationToken,
}
enum Done {
    Imported(u64, Result<Imported, String>),
    Decoded(VideoKey, Result<DecodedScreenVideo, motion_export::Error>),
}

/// PNG frames written so far in `cache_root`'s in-progress entries.
fn partial_frames(cache_root: &Path) -> u64 {
    fs::read_dir(cache_root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".partial"))
        .flat_map(|e| fs::read_dir(e.path()).into_iter().flatten().flatten())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".png"))
        .count() as u64
}

fn decode(job: &DecodeJob) -> Result<DecodedScreenVideo, motion_export::Error> {
    let invalid = motion_export::Error::Invalid;
    let source = resolve_in_project(&job.root, &job.key.asset).map_err(invalid)?;
    let color = probe_color(&ffprobe_for(&job.ffmpeg), &source, PROBE_TIMEOUT)?;
    let request = ScreenVideoRequest {
        source,
        fps: job.key.fps(),
        duration: job.key.duration(),
        max_width: Some(SCREEN_PROXY_WIDTH),
    };
    let cache_root = job.root.join(PREVIEW_CACHE_DIR);
    let done = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|scope| {
        // Progress = frames in the in-progress entry (the lane decodes one at a time).
        scope.spawn(|| {
            while !done.load(Ordering::Relaxed) {
                job.progress
                    .store(partial_frames(&cache_root), Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(200));
            }
        });
        let result = decode_screen_video(
            &job.ffmpeg,
            color,
            &request,
            &cache_root,
            &job.cancel,
            DECODE_TIMEOUT,
        );
        done.store(true, Ordering::Relaxed);
        result
    })
}

/// Two bounded background lanes; dropping it lets both threads exit.
pub struct Worker {
    imports: mpsc::SyncSender<ImportJob>,
    decodes: mpsc::SyncSender<DecodeJob>,
    done: mpsc::Receiver<Done>,
}
impl Worker {
    pub fn new() -> Self {
        let (imports, import_rx) = mpsc::sync_channel::<ImportJob>(QUEUE);
        let (decodes, decode_rx) = mpsc::sync_channel::<DecodeJob>(QUEUE);
        let (sender, done) = mpsc::sync_channel(2 * QUEUE + 2);
        let import_sender = sender.clone();
        std::thread::spawn(move || {
            while let Ok(job) = import_rx.recv() {
                let result = import_video(&job.root, &job.source, &job.ffprobe, &job.cancel);
                if import_sender.send(Done::Imported(job.id, result)).is_err() {
                    break;
                }
            }
        });
        std::thread::spawn(move || {
            while let Ok(job) = decode_rx.recv() {
                let result = if job.cancel.is_cancelled() {
                    Err(motion_export::Error::Cancelled)
                } else {
                    decode(&job)
                };
                if sender.send(Done::Decoded(job.key, result)).is_err() {
                    break;
                }
            }
        });
        Self {
            imports,
            decodes,
            done,
        }
    }
}

/// Decode state of one video for the preview.
#[derive(Clone)]
pub enum DecodeState {
    Decoding {
        cancel: CancellationToken,
        progress: Arc<AtomicU64>,
        frames: u64,
    },
    Ready(DecodedScreenVideo),
    Failed(String),
}

/// An import in flight: the cue it should become, once probed.
pub struct PendingImport {
    pub id: u64,
    pub cancel: CancellationToken,
}

/// UI-side state machine for screen-video imports and preview decodes.
pub struct ScreenVideos {
    worker: Worker,
    pub entries: BTreeMap<VideoKey, DecodeState>,
    /// Why nothing can be decoded right now (missing FFmpeg/ffprobe).
    pub blocked: Option<String>,
    /// Bumped whenever a decode finishes, so the preview rebuilds its snapshot.
    pub revision: u64,
    pub import: Option<PendingImport>,
    next_import: u64,
    root: PathBuf,
}
impl Default for ScreenVideos {
    fn default() -> Self {
        Self {
            worker: Worker::new(),
            entries: BTreeMap::new(),
            blocked: None,
            revision: 0,
            import: None,
            next_import: 0,
            root: PathBuf::new(),
        }
    }
}
impl ScreenVideos {
    /// Schedule decodes for `wanted` and cancel decodes no longer wanted.
    /// Never blocks: a full queue is retried on a later frame.
    pub fn sync(&mut self, wanted: &[VideoKey], root: &Path, ffmpeg: &str) {
        // Assets are project-relative: another project root invalidates everything.
        if self.root != root {
            self.cancel_all();
            self.root = root.to_path_buf();
        }
        let stale: Vec<VideoKey> = self
            .entries
            .iter()
            .filter(|(key, state)| {
                !wanted.contains(key) && matches!(state, DecodeState::Decoding { .. })
            })
            .map(|(key, _)| key.clone())
            .collect();
        for key in stale {
            if let Some(DecodeState::Decoding { cancel, .. }) = self.entries.remove(&key) {
                cancel.cancel();
            }
        }
        let missing: Vec<&VideoKey> = wanted
            .iter()
            .filter(|k| !self.entries.contains_key(*k))
            .collect();
        if missing.is_empty() {
            self.blocked = None;
            return;
        }
        self.blocked = tool_problem(ffmpeg);
        if self.blocked.is_some() {
            return;
        }
        for key in missing {
            let job = DecodeJob {
                key: key.clone(),
                root: root.to_path_buf(),
                ffmpeg: PathBuf::from(ffmpeg.trim()),
                cancel: CancellationToken::new(),
                progress: Arc::new(AtomicU64::new(0)),
            };
            let state = DecodeState::Decoding {
                cancel: job.cancel.clone(),
                progress: job.progress.clone(),
                frames: motion_script::ScreenSource::video_frames(key.fps(), key.duration()),
            };
            if self.worker.decodes.try_send(job).is_err() {
                break;
            }
            self.entries.insert(key.clone(), state);
        }
    }

    /// Start an import; returns its id, or an error if one is already running.
    pub fn start_import(
        &mut self,
        root: &Path,
        source: &Path,
        ffmpeg: &str,
    ) -> Result<u64, String> {
        if self.import.is_some() {
            return Err("Edellinen videon tuonti on vielä kesken.".into());
        }
        if let Some(problem) = tool_problem(ffmpeg) {
            return Err(problem);
        }
        if !source.is_absolute() {
            return Err("Anna videotiedoston koko polku.".into());
        }
        self.next_import += 1;
        let cancel = CancellationToken::new();
        let job = ImportJob {
            id: self.next_import,
            root: root.to_path_buf(),
            source: source.to_path_buf(),
            ffprobe: ffprobe_for(Path::new(ffmpeg.trim())),
            cancel: cancel.clone(),
        };
        self.worker
            .imports
            .try_send(job)
            .map_err(|_| "Tuontijono on täynnä; yritä hetken päästä.".to_owned())?;
        self.import = Some(PendingImport {
            id: self.next_import,
            cancel,
        });
        Ok(self.next_import)
    }

    pub fn cancel_import(&mut self) {
        if let Some(import) = self.import.take() {
            import.cancel.cancel();
        }
    }

    /// Cancel every running decode (for example when the project changes).
    pub fn cancel_all(&mut self) {
        for state in self.entries.values() {
            if let DecodeState::Decoding { cancel, .. } = state {
                cancel.cancel();
            }
        }
        self.entries.clear();
        self.revision = self.revision.wrapping_add(1);
    }

    /// Forget failures so they are tried again on the next `sync`.
    pub fn retry_failed(&mut self) {
        self.entries
            .retain(|_, state| !matches!(state, DecodeState::Failed(_)));
    }

    /// Drain finished work. Returns a finished import, if one completed.
    pub fn poll(&mut self) -> Option<Result<Imported, String>> {
        let mut imported = None;
        while let Ok(done) = self.worker.done.try_recv() {
            match done {
                Done::Imported(id, result) => {
                    if self.import.as_ref().is_some_and(|i| i.id == id) {
                        self.import = None;
                        imported = Some(result);
                    }
                }
                Done::Decoded(key, result) => {
                    // Only a still-wanted, still-running entry takes the result.
                    if !matches!(self.entries.get(&key), Some(DecodeState::Decoding { .. })) {
                        continue;
                    }
                    match result {
                        Ok(decoded) => {
                            self.entries.insert(key, DecodeState::Ready(decoded));
                            self.revision = self.revision.wrapping_add(1);
                        }
                        Err(motion_export::Error::Cancelled) => {
                            self.entries.remove(&key);
                        }
                        Err(error) => {
                            self.entries
                                .insert(key, DecodeState::Failed(error.to_string()));
                        }
                    }
                }
            }
        }
        imported
    }

    pub fn busy(&self) -> bool {
        self.import.is_some()
            || self
                .entries
                .values()
                .any(|s| matches!(s, DecodeState::Decoding { .. }))
    }

    /// Decoded proxies for `wanted`, keyed by cue asset for `ArtContext`.
    pub fn decoded(&self, wanted: &[VideoKey]) -> BTreeMap<String, DecodedScreenVideo> {
        wanted
            .iter()
            .filter_map(|key| match self.entries.get(key) {
                Some(DecodeState::Ready(decoded)) => Some((key.asset.clone(), decoded.clone())),
                _ => None,
            })
            .collect()
    }

    /// One status line per wanted video that is not ready, in Finnish.
    pub fn status(&self, wanted: &[VideoKey]) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(blocked) = &self.blocked {
            lines.push(blocked.clone());
        }
        for key in wanted {
            match self.entries.get(key) {
                Some(DecodeState::Decoding {
                    progress, frames, ..
                }) => lines.push(format!(
                    "Puretaan esikatseluun: {} ({}/{} ruutua)…",
                    key.asset,
                    progress.load(Ordering::Relaxed).min(*frames),
                    frames
                )),
                Some(DecodeState::Failed(error)) => {
                    lines.push(format!("Videon {} purku epäonnistui: {error}", key.asset))
                }
                Some(DecodeState::Ready(_)) => {}
                None if self.blocked.is_none() => {
                    lines.push(format!("Odottaa purkua: {}", key.asset))
                }
                None => {}
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kilsat-screen-video-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::canonicalize(dir).unwrap()
    }

    #[cfg(unix)]
    fn script(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// Fake tool folder: ffprobe prints a 30 fps, 2.5 s stream; ffmpeg runs `ffmpeg_body`.
    #[cfg(unix)]
    fn fake_tools(dir: &Path, ffmpeg_body: &str) -> PathBuf {
        let bin = dir.join("bin");
        fs::create_dir_all(&bin).unwrap();
        script(
            &bin.join("ffprobe"),
            r#"echo '{"streams":[{"avg_frame_rate":"30/1","r_frame_rate":"30/1","duration":"2.5004"}],"format":{"duration":"2.6"}}'"#,
        );
        script(&bin.join("ffmpeg"), ffmpeg_body);
        bin.join("ffmpeg")
    }

    fn wait(videos: &mut ScreenVideos, until: impl Fn(&ScreenVideos) -> bool) {
        let started = Instant::now();
        while !until(videos) {
            assert!(started.elapsed() < Duration::from_secs(20), "timed out");
            let _ = videos.poll();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn import_source_validation_rejects_bad_paths() {
        let dir = temp("validate");
        assert!(validate_import_source(Path::new("relative.mp4")).is_err());
        let text = dir.join("notes.txt");
        fs::write(&text, b"x").unwrap();
        assert!(validate_import_source(&text).unwrap_err().contains(".mp4"));
        let empty = dir.join("empty.mov");
        fs::write(&empty, b"").unwrap();
        assert!(validate_import_source(&empty)
            .unwrap_err()
            .contains("tyhjä"));
        let folder = dir.join("folder.mp4");
        fs::create_dir(&folder).unwrap();
        assert!(validate_import_source(&folder).is_err());
        assert!(validate_import_source(&dir.join("missing.mp4")).is_err());
        let ok = dir.join("Demo Clip.MOV");
        fs::write(&ok, b"data").unwrap();
        assert_eq!(validate_import_source(&ok), Ok(4));
        #[cfg(unix)]
        {
            let link = dir.join("link.mp4");
            std::os::unix::fs::symlink(&ok, &link).unwrap();
            assert!(validate_import_source(&link).is_err());
        }
        assert_eq!(import_file_name(&ok), "demo-clip.mov");
        assert_eq!(import_file_name(Path::new("/x/äö.m4v")), "video.m4v");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn probe_output_is_parsed_and_bounded() {
        let ntsc = br#"{"streams":[{"avg_frame_rate":"30000/1001","duration":"12.5"}]}"#;
        assert_eq!(parse_probe(ntsc), Ok((29.97, 12.5)));
        let fallback = br#"{"streams":[{"avg_frame_rate":"0/0","r_frame_rate":"60/1"}],"format":{"duration":"3.0009"}}"#;
        assert_eq!(parse_probe(fallback), Ok((60.0, 3.0)));
        assert!(parse_probe(br#"{"streams":[]}"#).is_err());
        assert!(
            parse_probe(br#"{"streams":[{"avg_frame_rate":"500/1","duration":"1"}]}"#).is_err()
        );
        assert!(
            parse_probe(br#"{"streams":[{"avg_frame_rate":"30/1","duration":"7200"}]}"#).is_err()
        );
        assert!(parse_probe(b"not json").is_err());
    }

    #[test]
    fn project_paths_must_stay_inside_the_root() {
        let dir = temp("resolve");
        fs::create_dir_all(dir.join("assets/screens")).unwrap();
        fs::write(dir.join("assets/screens/a.mp4"), b"v").unwrap();
        assert!(resolve_in_project(&dir, "assets/screens/a.mp4").is_ok());
        assert!(resolve_in_project(&dir, "../a.mp4").is_err());
        assert!(resolve_in_project(&dir, "/etc/hosts").is_err());
        assert!(resolve_in_project(&dir, "assets/screens/missing.mp4").is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn import_copies_into_screens_folder_without_overwriting_and_probes() {
        let dir = temp("import");
        let project = dir.join("project");
        fs::create_dir(&project).unwrap();
        let ffmpeg = fake_tools(&dir, "exit 1");
        let ffprobe = ffprobe_for(&ffmpeg);
        let source = dir.join("My Demo.mp4");
        fs::write(&source, b"recording").unwrap();
        let cancel = CancellationToken::new();
        let first = import_video(&project, &source, &ffprobe, &cancel).unwrap();
        assert_eq!(
            first,
            Imported {
                asset: "assets/screens/my-demo.mp4".into(),
                fps: 30.0,
                duration: 2.5
            }
        );
        let second = import_video(&project, &source, &ffprobe, &cancel).unwrap();
        assert_eq!(second.asset, "assets/screens/my-demo-2.mp4");
        assert_eq!(
            fs::read(project.join(&first.asset)).unwrap(),
            b"recording".to_vec()
        );
        // Re-importing a file already in assets/screens reuses it.
        let again = import_video(&project, &project.join(&first.asset), &ffprobe, &cancel).unwrap();
        assert_eq!(again.asset, first.asset);
        // A failed probe leaves no copied file behind.
        script(&ffprobe, "exit 1");
        assert!(import_video(&project, &source, &ffprobe, &cancel).is_err());
        assert!(!project.join("assets/screens/my-demo-3.mp4").exists());
        // Missing ffprobe is reported, not searched for on PATH.
        assert!(
            import_video(&project, &source, &dir.join("nope/ffprobe"), &cancel)
                .unwrap_err()
                .contains("ffprobe")
        );
        let leftovers: Vec<_> = fs::read_dir(project.join(SCREENS_DIR))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_ffmpeg_blocks_decoding_without_jobs() {
        let dir = temp("missing");
        let mut videos = ScreenVideos::default();
        let wanted = [VideoKey::new("assets/screens/a.mp4", 30.0, 2.0)];
        videos.sync(&wanted, &dir, "");
        assert!(videos
            .blocked
            .as_deref()
            .unwrap()
            .contains("FFmpeg puuttuu"));
        assert!(videos.entries.is_empty() && !videos.busy());
        videos.sync(&wanted, &dir, "/definitely/missing/ffmpeg");
        assert!(videos
            .blocked
            .as_deref()
            .unwrap()
            .contains("FFmpeg puuttuu"));
        assert!(videos.entries.is_empty());
        assert_eq!(videos.status(&wanted).len(), 1);
        assert!(videos.decoded(&wanted).is_empty());
        assert!(videos
            .start_import(&dir, &dir.join("a.mp4"), "")
            .unwrap_err()
            .contains("FFmpeg"));
        // Nothing wanted: no warning.
        videos.sync(&[], &dir, "");
        assert!(videos.blocked.is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn decode_state_machine_fails_cancels_and_succeeds_with_fake_ffmpeg() {
        let dir = temp("decode");
        fs::create_dir_all(dir.join(SCREENS_DIR)).unwrap();
        fs::write(dir.join("assets/screens/a.mp4"), b"video-a").unwrap();
        let key = VideoKey::new("assets/screens/a.mp4", 10.0, 0.3);
        let wanted = [key.clone()];

        // A failing FFmpeg ends in Failed, with a status line; retry clears it.
        let failing = fake_tools(&dir, "exit 1");
        let ffmpeg = failing.to_string_lossy().into_owned();
        let mut videos = ScreenVideos::default();
        videos.sync(&wanted, &dir, &ffmpeg);
        assert!(videos.blocked.is_none() && videos.busy());
        assert!(videos.status(&wanted)[0].contains("Puretaan"));
        wait(&mut videos, |v| !v.busy());
        assert!(matches!(videos.entries[&key], DecodeState::Failed(_)));
        assert!(videos.status(&wanted)[0].contains("epäonnistui"));
        let revision = videos.revision;
        videos.sync(&wanted, &dir, &ffmpeg);
        assert!(!videos.busy(), "a failure is not retried automatically");
        videos.retry_failed();
        assert!(videos.entries.is_empty());

        // A slow FFmpeg is cancelled when the cue is no longer wanted.
        script(&failing, "sleep 30");
        videos.sync(&wanted, &dir, &ffmpeg);
        assert!(videos.busy());
        std::thread::sleep(Duration::from_millis(100));
        videos.sync(&[], &dir, &ffmpeg);
        assert!(videos.entries.is_empty() && !videos.busy());

        // A fake FFmpeg that writes PNG frames makes the proxy ready.
        let png = dir.join("frame.png");
        fs::write(&png, tiny_png()).unwrap();
        script(
            &failing,
            &format!(
                "for last; do :; done\nout=$(dirname \"$last\")\nfor i in 000000 000001 000002; do cp '{}' \"$out/$i.png\"; done",
                png.display()
            ),
        );
        let started = Instant::now();
        loop {
            videos.sync(&wanted, &dir, &ffmpeg);
            let _ = videos.poll();
            if matches!(videos.entries.get(&key), Some(DecodeState::Ready(_))) {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(40),
                "decode timed out"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(videos.revision > revision);
        let decoded = videos.decoded(&wanted);
        assert_eq!(decoded["assets/screens/a.mp4"].frames, 3);
        assert!(videos.status(&wanted).is_empty());
        assert!(dir.join(PREVIEW_CACHE_DIR).is_dir());
        let _ = fs::remove_dir_all(dir);
    }

    /// Real FFmpeg: import a synthetic clip, then decode its preview proxy.
    /// Run with `KILSAT_TEST_FFMPEG=/opt/homebrew/bin/ffmpeg cargo test -p
    /// studio-desktop real_ffmpeg_import_and_preview_decode -- --ignored`.
    #[test]
    #[ignore]
    fn real_ffmpeg_import_and_preview_decode() {
        let ffmpeg =
            PathBuf::from(std::env::var("KILSAT_TEST_FFMPEG").expect("KILSAT_TEST_FFMPEG"));
        let dir = temp("real");
        let project = dir.join("project");
        fs::create_dir(&project).unwrap();
        let clip = dir.join("clip.mp4");
        let status = Command::new(&ffmpeg)
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=320x240:rate=24:duration=1",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&clip)
            .status()
            .unwrap();
        assert!(status.success());
        let mut videos = ScreenVideos::default();
        let ffmpeg = ffmpeg.to_string_lossy().into_owned();
        videos.start_import(&project, &clip, &ffmpeg).unwrap();
        let imported = loop {
            if let Some(result) = videos.poll() {
                break result.unwrap();
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(
            (imported.asset.as_str(), imported.fps),
            ("assets/screens/clip.mp4", 24.0)
        );
        assert!(
            (imported.duration - 1.0).abs() < 0.05,
            "{}",
            imported.duration
        );
        let wanted = [VideoKey::new(
            &imported.asset,
            imported.fps,
            imported.duration,
        )];
        videos.sync(&wanted, &project, &ffmpeg);
        wait(&mut videos, |v| !v.busy());
        let decoded = videos.decoded(&wanted);
        let video = &decoded[&imported.asset];
        assert!(video.frames >= 23, "{}", video.frames);
        let (width, _) = motion_export::screen_video::png_size(&video.frame_path(0)).unwrap();
        assert!(width <= SCREEN_PROXY_WIDTH);
        let _ = fs::remove_dir_all(dir);
    }

    /// A 1×1 RGB PNG.
    fn tiny_png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xdd, 0x8d,
            0xb0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ]
    }
}
