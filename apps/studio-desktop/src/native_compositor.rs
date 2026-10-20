//! CPU composition lane: one active raster, one replaceable desired frame/result.
use motion_export::{ArtContext, ScenePreview, SequencePreviewFrame};
use motion_script::Script;
use motion_tts::CancellationToken;
use motion_voice::export::SceneManifest;
use std::sync::{Arc, Condvar, Mutex};

#[derive(Clone)]
pub struct Request {
    /// Increment for every script, scene, art, resource or speech snapshot change.
    pub generation: u64,
    pub scene_index: usize,
    pub sample_frame: u64,
    pub script: Arc<Script>,
    /// User-level caption choice; a change is a new snapshot (new generation).
    pub captions: motion_script::CaptionMode,
    pub art: Arc<ArtContext>,
    pub manifest: Option<Arc<SceneManifest>>,
    /// Optional already evaluated mouths for this sample (including selected-line playback).
    pub mouths: Option<Vec<(String, motion_lipsync::Viseme)>>,
}
pub struct Update {
    pub generation: u64,
    pub scene_index: usize,
    pub sample_frame: u64,
    pub result: Result<SequencePreviewFrame, String>,
}
#[derive(Default)]
struct State {
    desired: Option<Request>,
    active: Option<(u64, CancellationToken)>,
    update: Option<Update>,
    stopped: bool,
}
pub struct Worker {
    shared: Arc<(Mutex<State>, Condvar)>,
}
impl Worker {
    pub fn new() -> Self {
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let background = shared.clone();
        std::thread::spawn(move || run(background));
        Self { shared }
    }
    /// Replaces the pending frame, never blocks on raster work or spawns a thread.
    pub fn request(&self, request: Request) -> Result<(), String> {
        let (lock, wake) = &*self.shared;
        let mut state = lock.lock().map_err(|_| "Preview worker lock failed")?;
        if state.stopped {
            return Err("Preview worker stopped".into());
        }
        if let Some((generation, cancel)) = &state.active {
            if *generation != request.generation {
                cancel.cancel();
            }
        }
        // A result from another snapshot must never be displayed as current.
        if state
            .update
            .as_ref()
            .is_some_and(|u| u.generation != request.generation)
        {
            state.update = None;
        }
        state.desired = Some(request);
        wake.notify_one();
        Ok(())
    }
    pub fn poll(&self) -> Option<Update> {
        self.shared.0.lock().ok()?.update.take()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared.0.lock() {
            state.stopped = true;
            state.desired = None;
            if let Some((_, token)) = &state.active {
                token.cancel();
            }
            self.shared.1.notify_one();
        }
        // Never join a raster on the UI thread. Cancellation is checked between
        // bounded evaluation/composition/raster stages, then the thread exits.
    }
}
fn same_snapshot(a: &Request, b: &Request) -> bool {
    a.generation == b.generation
        && a.scene_index == b.scene_index
        && a.captions == b.captions
        && Arc::ptr_eq(&a.script, &b.script)
        && Arc::ptr_eq(&a.art, &b.art)
        && match (&a.manifest, &b.manifest) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
}
fn run(shared: Arc<(Mutex<State>, Condvar)>) {
    let mut cached: Option<(Request, Result<ScenePreview, String>)> = None;
    loop {
        let (request, cancel) = {
            let (lock, wake) = &*shared;
            let Ok(mut state) = lock.lock() else {
                return;
            };
            while state.desired.is_none() && !state.stopped {
                let Ok(next) = wake.wait(state) else {
                    return;
                };
                state = next;
            }
            if state.stopped {
                return;
            }
            let Some(request) = state.desired.take() else {
                continue;
            };
            let cancel = CancellationToken::new();
            state.active = Some((request.generation, cancel.clone()));
            (request, cancel)
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if cached
                .as_ref()
                .is_none_or(|(old, _)| !same_snapshot(old, &request))
            {
                let snapshot = ScenePreview::new_with_captions(
                    request.script.clone(),
                    request.scene_index,
                    request.art.clone(),
                    request.manifest.clone(),
                    request.captions,
                )
                .map_err(|e| e.to_string());
                cached = Some((request.clone(), snapshot));
            }
            match &cached.as_ref().expect("snapshot was initialized").1 {
                Ok(preview) => preview
                    .render_with_mouths(
                        request.sample_frame,
                        1024,
                        request.mouths.as_deref(),
                        &cancel,
                    )
                    .map_err(|e| e.to_string()),
                Err(error) => Err(error.clone()),
            }
        }))
        .unwrap_or_else(|_| {
            Err("Preview compositor panicked; revise inputs before retrying".into())
        });
        let Ok(mut state) = shared.0.lock() else {
            return;
        };
        state.active = None;
        if state.stopped {
            return;
        }
        if !cancel.is_cancelled()
            && state
                .desired
                .as_ref()
                .is_none_or(|new| same_snapshot(new, &request))
        {
            state.update = Some(Update {
                generation: request.generation,
                scene_index: request.scene_index,
                sample_frame: request.sample_frame,
                result,
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request(generation: u64) -> Request {
        Request { generation, scene_index:0, sample_frame:0,
            script:Arc::new(motion_script::parse("project: {fps: 4, width: 64, height: 36}\ncharacters: {}\nscenes:\n  - id: intro\n    duration: 1.0\n    events: []\n").unwrap()),
            captions: motion_script::CaptionMode::default(),
            art: Arc::new(ArtContext::default()), manifest:None, mouths:None }
    }
    #[test]
    fn bounded_pending_request_is_replaced_and_stale_result_cleared() {
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let worker = Worker {
            shared: shared.clone(),
        };
        worker.request(request(1)).unwrap();
        let cancel = CancellationToken::new();
        {
            let mut state = shared.0.lock().unwrap();
            state.active = Some((1, cancel.clone()));
            state.update = Some(Update {
                generation: 1,
                scene_index: 0,
                sample_frame: 0,
                result: Err("old".into()),
            });
        }
        for generation in 2..100 {
            worker.request(request(generation)).unwrap();
        }
        let state = shared.0.lock().unwrap();
        assert_eq!(state.desired.as_ref().unwrap().generation, 99);
        assert!(state.update.is_none());
        assert!(cancel.is_cancelled());
        drop(state);
        drop(worker);
        assert!(shared.0.lock().unwrap().stopped);
    }
    #[test]
    fn snapshot_identity_includes_art_resources_and_manifest() {
        let a = request(1);
        let mut b = a.clone();
        assert!(same_snapshot(&a, &b));
        b.sample_frame = 42;
        assert!(same_snapshot(&a, &b));
        b.art = Arc::new(ArtContext::default());
        assert!(!same_snapshot(&a, &b));
        b = a.clone();
        b.scene_index = 1;
        assert!(!same_snapshot(&a, &b));
    }
}
