//! One persistent, bounded worker for complete scene preparation.
use motion_lipsync::Alignment;
use motion_script::Script;
use motion_tts::{CancellationToken, KokoroConfig};
use motion_voice::scene_audio::{prepare_scene_with_cancellation, PreparedScene, SceneProgress};
use std::{collections::BTreeMap, path::PathBuf, sync::mpsc};

pub struct Request {
    pub script: Script,
    pub index: usize,
    pub root: PathBuf,
    pub config: KokoroConfig,
    pub alignments: String,
    pub cancel: CancellationToken,
}
pub struct Ready {
    pub prepared: PreparedScene,
    pub peaks: Vec<motion_audio::WaveformPeak>,
}
pub enum Update {
    Progress(SceneProgress),
    Finished(Result<Ready, String>),
}
pub struct Worker {
    tx: mpsc::SyncSender<Request>,
    rx: mpsc::Receiver<Update>,
}
impl Worker {
    pub fn new() -> Self {
        let (tx, requests) = mpsc::sync_channel::<Request>(1);
        let (updates, rx) = mpsc::sync_channel(4);
        std::thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    request.cancel.check().map_err(|e| e.to_string())?;
                    let alignments = if request.alignments.trim().is_empty() {
                        BTreeMap::new()
                    } else {
                        serde_json::from_str::<BTreeMap<u64, Alignment>>(&request.alignments)
                            .map_err(|e| format!("Scene alignment JSON: {e}"))?
                    };
                    let prepared = prepare_scene_with_cancellation(
                        &request.script,
                        request.index,
                        &request.root,
                        &motion_tts::KokoroProvider {
                            config: request.config,
                        },
                        &alignments,
                        &request.cancel,
                        |progress| {
                            let _ = updates.try_send(Update::Progress(progress));
                        },
                    )
                    .map_err(|e| e.to_string())?;
                    request.cancel.check().map_err(|e| e.to_string())?;
                    let audio = &prepared.rendered.audio;
                    let peaks = motion_audio::waveform_peaks(audio, (audio.frames() / 700).max(1))
                        .map_err(|e| e.to_string())?;
                    request.cancel.check().map_err(|e| e.to_string())?;
                    Ok(Ready { prepared, peaks })
                }))
                .unwrap_or_else(|_| {
                    Err("Scene preparation worker panicked; retry after checking inputs".into())
                });
                if updates.send(Update::Finished(result)).is_err() {
                    break;
                }
            }
        });
        Self { tx, rx }
    }
    pub fn submit(&self, request: Request) -> Result<(), String> {
        self.tx
            .try_send(request)
            .map_err(|e| format!("Scene preparation queue: {e}"))
    }
    pub fn poll(&self) -> Option<Update> {
        self.rx.try_recv().ok()
    }
}
