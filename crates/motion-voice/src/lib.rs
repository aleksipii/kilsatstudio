//! Bounded single-worker orchestration. UI submits and polls; synthesis stays off its thread.
use motion_script::{Event, Script};
use motion_tts::{SpeechError, SpeechProvider, SpeechRequest, SpeechResult, VoiceSettings};
use std::sync::{mpsc, Arc};

#[derive(Clone, Debug)]
pub struct DialogueJob {
    pub id: u64,
    pub scene: String,
    pub actor: String,
    pub at: f64,
    pub request: SpeechRequest,
    /// Imported audio is the explicit offline fallback, never fabricated speech.
    pub imported_audio: Option<std::path::PathBuf>,
}
#[derive(Debug)]
pub enum VoiceUpdate {
    Started(u64),
    Finished {
        id: u64,
        result: Result<SpeechResult, SpeechError>,
    },
}
pub struct VoiceWorker {
    sender: mpsc::SyncSender<DialogueJob>,
    updates: mpsc::Receiver<VoiceUpdate>,
}
impl VoiceWorker {
    pub fn new(provider: Arc<dyn SpeechProvider>, capacity: usize) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<DialogueJob>(capacity.clamp(1, 64));
        // Bound both queues; a paused UI cannot accumulate unbounded completions.
        let (progress, updates) = mpsc::sync_channel(capacity.clamp(1, 64) * 2);
        std::thread::spawn(move || {
            while let Ok(job) = receiver.recv() {
                if progress.send(VoiceUpdate::Started(job.id)).is_err() {
                    break;
                }
                let result = match &job.imported_audio {
                    Some(path) => imported(path),
                    None => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        provider.synthesize(job.request)
                    }))
                    .unwrap_or_else(|_| {
                        Err(SpeechError("speech provider panicked; job failed".into()))
                    }),
                };
                if progress
                    .send(VoiceUpdate::Finished { id: job.id, result })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self { sender, updates }
    }
    // Return ownership of queued work so callers can retry without cloning its text.
    #[allow(clippy::result_large_err)]
    pub fn submit(&self, job: DialogueJob) -> Result<(), mpsc::TrySendError<DialogueJob>> {
        self.sender.try_send(job)
    }
    pub fn poll(&self) -> Option<VoiceUpdate> {
        self.updates.try_recv().ok()
    }
}
fn imported(path: &std::path::Path) -> Result<SpeechResult, SpeechError> {
    // Same 24kHz mono PCM16 contract as synthesized assets.
    let mut reader = hound_reader(path)?;
    let count = reader.duration() as u64;
    for sample in reader.samples::<i16>() {
        sample.map_err(|e| SpeechError(e.to_string()))?;
    }
    Ok(SpeechResult {
        audio_path: path.into(),
        sample_rate: 24000,
        sample_count: count,
        cache_key: String::new(),
        phonemes: String::new(),
        cached: false,
    })
}
fn hound_reader(
    path: &std::path::Path,
) -> Result<hound::WavReader<std::io::BufReader<std::fs::File>>, SpeechError> {
    let r = hound::WavReader::open(path).map_err(|e| SpeechError(format!("import audio: {e}")))?;
    let s = r.spec();
    if s.channels != 1
        || s.sample_rate != 24000
        || s.bits_per_sample != 16
        || s.sample_format != hound::SampleFormat::Int
        || r.duration() == 0
    {
        return Err(SpeechError(
            "import requires nonempty 24kHz mono PCM16 WAV; convert before import".into(),
        ));
    }
    Ok(r)
}
/// Deterministic IDs follow scene/event order; imported paths resolve against project root.
pub fn dialogue_jobs(
    script: &Script,
    root: &std::path::Path,
) -> Result<Vec<DialogueJob>, SpeechError> {
    script.validate().map_err(|e| SpeechError(e.to_string()))?;
    let mut jobs = Vec::new();
    for scene in &script.scenes {
        for event in &scene.events {
            let Event::Speak {
                at,
                actor,
                text,
                emotion,
                audio,
            } = event;
            let voice = script.characters[actor].voice.as_ref();
            let settings = voice
                .map(|v| VoiceSettings {
                    preset: v.preset.clone(),
                    language: v.language.clone(),
                    speed: v.speed,
                    style: emotion.clone(),
                })
                .unwrap_or_default();
            jobs.push(DialogueJob {
                id: jobs.len() as u64,
                scene: scene.id.clone(),
                actor: actor.clone(),
                at: *at,
                request: SpeechRequest {
                    text: text.clone(),
                    voice: settings,
                },
                imported_audio: audio.as_ref().map(|p| root.join(p)),
            });
        }
    }
    Ok(jobs)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiles_character_voice_and_timeline() {
        let script = motion_script::parse(include_str!("../../../examples/dialogue.yaml")).unwrap();
        let jobs = dialogue_jobs(&script, std::path::Path::new(".")).unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].request.voice.preset, "af_heart");
        assert_eq!(jobs[1].at, 5.0);
        assert_eq!(jobs[0].request.voice.style.as_deref(), Some("happy"));
    }
}

pub mod timeline;
