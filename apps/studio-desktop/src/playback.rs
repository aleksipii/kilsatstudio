//! macOS audio preview. The reported clock is Rodio's consumed source position,
//! sampled about every 10 ms, not a measurement of hardware output latency.
//! Mouth evaluation must use this clock rather than elapsed wall time.
use motion_audio::AudioBuffer;
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct PlaybackState {
    pub loaded: bool,
    /// Clip-local sample frame, including the most recent seek offset.
    pub position_frame: u64,
    pub playing: bool,
    pub finished: bool,
    pub error: Option<String>,
}

#[cfg(target_os = "macos")]
pub use macos::Playback;

#[cfg(not(target_os = "macos"))]
pub struct Playback;
#[cfg(not(target_os = "macos"))]
impl Playback {
    pub fn new() -> Result<Self, String> {
        Err("Audio-device preview requires macOS. Use waveform scrubbing here, or run the desktop application on a Mac with an output device.".into())
    }
    pub fn play(&self, _: Arc<AudioBuffer>, _: u64) -> Result<(), String> {
        Self::new().map(|_| ())
    }
    pub fn pause(&self) -> Result<(), String> {
        Self::new().map(|_| ())
    }
    pub fn resume(&self) -> Result<(), String> {
        Self::new().map(|_| ())
    }
    pub fn seek(&self, _: u64) -> Result<(), String> {
        Self::new().map(|_| ())
    }
    pub fn stop(&self) -> Result<(), String> {
        Self::new().map(|_| ())
    }
    pub fn snapshot(&self) -> PlaybackState {
        PlaybackState::default()
    }
}

#[cfg(any(target_os = "macos", test))]
fn source_frame(position: std::time::Duration, rate: u32, base: u64, end: u64) -> u64 {
    let frames = position.as_nanos().saturating_mul(rate as u128) / 1_000_000_000;
    base.saturating_add(frames.min(u64::MAX as u128) as u64)
        .min(end)
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
    use std::{
        sync::{
            mpsc::{self, SyncSender, TrySendError},
            Mutex,
        },
        time::Duration,
    };

    enum Command {
        Play(Arc<AudioBuffer>, u64),
        Pause,
        Resume,
        Seek(u64),
        Stop,
    }
    pub struct Playback {
        tx: SyncSender<Command>,
        state: Arc<Mutex<PlaybackState>>,
    }
    impl Playback {
        /// Starts one worker; the device is opened on Play. Failures appear in snapshot().
        pub fn new() -> Result<Self, String> {
            let (tx, rx) = mpsc::sync_channel(8);
            let state = Arc::new(Mutex::new(PlaybackState::default()));
            let updates = state.clone();
            std::thread::Builder::new().name("audio-preview".into()).spawn(move || {
                // Created lazily on Play; retained until worker shutdown. Retrying
                // Play after a failed open retries the current default device.
                let mut output: Option<(OutputStream, OutputStreamHandle)> = None;
                let mut active: Option<Active> = None;
                let mut error: Option<String> = None;
                loop {
                    match rx.recv_timeout(Duration::from_millis(10)) {
                        Ok(command) => {
                            if output.is_none() && matches!(&command, Command::Play(..)) {
                                match OutputStream::try_default() {
                                    Ok(stream) => output = Some(stream),
                                    Err(e) => {
                                        error = Some(format!("Open default audio output: {e}. Check macOS Sound settings and reconnect the output device, then retry Play."));
                                    }
                                }
                            }
                            if let Some((_, handle)) = &output {
                                match handle_command(command, handle, &mut active) {
                                    Ok(()) => error = None,
                                    Err(e) => {
                                        if let Some(track) = active.take() { track.sink.stop(); }
                                        error = Some(e);
                                    }
                                }
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    let mut snapshot = active.as_ref().map(Active::snapshot).unwrap_or_default();
                    snapshot.error = error.clone();
                    publish(&updates, snapshot);
                }
            }).map_err(|e| format!("Start audio playback worker: {e}"))?;
            Ok(Self { tx, state })
        }
        fn send(&self, command: Command) -> Result<(), String> {
            self.tx.try_send(command).map_err(|e| match e {
                TrySendError::Full(_) => {
                    "Audio controls busy; retry after the pending command completes.".into()
                }
                TrySendError::Disconnected(_) => {
                    "Audio worker stopped; reopen the application after checking the output device."
                        .into()
                }
            })
        }
        pub fn play(&self, audio: Arc<AudioBuffer>, start_frame: u64) -> Result<(), String> {
            self.send(Command::Play(audio, start_frame))
        }
        pub fn pause(&self) -> Result<(), String> {
            self.send(Command::Pause)
        }
        pub fn resume(&self) -> Result<(), String> {
            self.send(Command::Resume)
        }
        pub fn seek(&self, frame: u64) -> Result<(), String> {
            self.send(Command::Seek(frame))
        }
        pub fn stop(&self) -> Result<(), String> {
            self.send(Command::Stop)
        }
        pub fn snapshot(&self) -> PlaybackState {
            self.state.try_lock().map(|s| s.clone()).unwrap_or_default()
        }
    }
    fn publish(state: &Mutex<PlaybackState>, snapshot: PlaybackState) {
        if let Ok(mut state) = state.lock() {
            *state = snapshot;
        }
    }
    struct Active {
        sink: Sink,
        audio: Arc<AudioBuffer>,
        base: u64,
    }
    impl Active {
        fn new(
            handle: &OutputStreamHandle,
            audio: Arc<AudioBuffer>,
            frame: u64,
            paused: bool,
        ) -> Result<Self, String> {
            audio.validate().map_err(|e| e.to_string())?;
            let base = frame.min(audio.frames() as u64);
            let sink = Sink::try_new(handle).map_err(|e| format!("Create audio sink: {e}"))?;
            sink.pause();
            sink.append(PcmSource {
                index: base as usize * audio.channels as usize,
                audio: audio.clone(),
            });
            if !paused {
                sink.play();
            }
            Ok(Self { sink, audio, base })
        }
        fn snapshot(&self) -> PlaybackState {
            let finished = self.sink.empty();
            PlaybackState {
                loaded: true,
                position_frame: if finished {
                    self.audio.frames() as u64
                } else {
                    source_frame(
                        self.sink.get_pos(),
                        self.audio.sample_rate,
                        self.base,
                        self.audio.frames() as u64,
                    )
                },
                playing: !finished && !self.sink.is_paused(),
                finished,
                error: None,
            }
        }
    }
    fn handle_command(
        command: Command,
        handle: &OutputStreamHandle,
        active: &mut Option<Active>,
    ) -> Result<(), String> {
        match command {
            Command::Play(audio, frame) => {
                if let Some(old) = active.take() {
                    old.sink.stop();
                }
                *active = Some(Active::new(handle, audio, frame, false)?);
            }
            Command::Pause => {
                if let Some(track) = active {
                    track.sink.pause();
                }
            }
            Command::Resume => {
                if let Some(track) = active {
                    track.sink.play();
                }
            }
            Command::Seek(frame) => {
                if let Some(old) = active.take() {
                    let paused = old.sink.is_paused();
                    old.sink.stop();
                    // Recreate an exact frame slice. Sink::try_seek waits for the device
                    // callback; rebuilding avoids that wait if an output device stalls.
                    *active = Some(Active::new(handle, old.audio, frame, paused)?);
                }
            }
            Command::Stop => {
                if let Some(old) = active.take() {
                    old.sink.stop();
                }
            }
        }
        Ok(())
    }
    /// Shared immutable PCM, avoiding a full audio copy on each play/seek.
    struct PcmSource {
        audio: Arc<AudioBuffer>,
        index: usize,
    }
    impl Iterator for PcmSource {
        type Item = f32;
        fn next(&mut self) -> Option<f32> {
            let sample = self.audio.samples.get(self.index).copied()?;
            self.index += 1;
            Some(sample)
        }
    }
    impl Source for PcmSource {
        fn current_frame_len(&self) -> Option<usize> {
            None
        }
        fn channels(&self) -> u16 {
            self.audio.channels
        }
        fn sample_rate(&self) -> u32 {
            self.audio.sample_rate
        }
        fn total_duration(&self) -> Option<Duration> {
            Some(Duration::from_secs_f64(
                (self.audio.samples.len() - self.index) as f64
                    / self.audio.channels as f64
                    / self.audio.sample_rate as f64,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn clock_uses_frames_offset_and_clamps_to_end() {
        assert_eq!(
            source_frame(Duration::from_millis(125), 24000, 100, 10000),
            3100
        );
        assert_eq!(
            source_frame(Duration::from_secs(99), 24000, 100, 10000),
            10000
        );
        assert_eq!(source_frame(Duration::ZERO, 24000, 75, 10000), 75);
        assert_eq!(source_frame(Duration::MAX, u32::MAX, u64::MAX, 100), 100);
    }
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn unavailable_device_backend_is_actionable() {
        assert!(Playback::new().err().unwrap().contains("requires macOS"));
    }
}
