//! Native sequence controls. Expensive work belongs to the bounded worker.
use crate::{
    playback,
    sequence_worker::{self, Task, Update},
};
use eframe::egui;
use motion_tts::CancellationToken;
use std::{sync::Arc, time::Instant};

pub struct Panel {
    worker: sequence_worker::Worker,
    generation: u64,
    source: String,
    prepared: Option<Arc<sequence_worker::Prepared>>,
    cancel: Option<CancellationToken>,
    frame_cancel: Option<CancellationToken>,
    frame_pending: bool,
    worker_pending: bool,
    desired: u64,
    shown: Option<u64>,
    failed_frame: Option<u64>,
    play_started: bool,
    prepare_requested: bool,
    texture: Option<egui::TextureHandle>,
    player: Option<playback::Playback>,
    playing: bool,
    silent: bool,
    tick: Instant,
    fraction: f64,
    busy: bool,
    status: String,
    destination: String,
    ffmpeg: String,
    mp4: bool,
}
impl Default for Panel {
    fn default() -> Self {
        Self {
            worker: sequence_worker::Worker::new(),
            generation: 0,
            source: String::new(),
            prepared: None,
            cancel: None,
            frame_cancel: None,
            frame_pending: false,
            worker_pending: false,
            desired: 0,
            shown: None,
            failed_frame: None,
            play_started: false,
            prepare_requested: false,
            texture: None,
            player: None,
            playing: false,
            silent: false,
            tick: Instant::now(),
            fraction: 0.0,
            busy: false,
            status: "Prepare explicitly to synthesize local project audio.".into(),
            destination: String::new(),
            ffmpeg: String::new(),
            mp4: false,
        }
    }
}
impl Panel {
    pub fn stop_transport(&mut self) {
        self.playing = false;
        if let Some(player) = self.player.take() {
            let _ = player.stop();
        }
    }
    pub fn take_prepare_requested(&mut self) -> bool {
        std::mem::take(&mut self.prepare_requested)
    }
    /// Called only after the explicit Prepare action, so cloning the project is lazy.
    pub fn start_prepare(&mut self, mut request: sequence_worker::PrepareRequest) -> bool {
        request.generation = self.generation;
        self.cancel = Some(request.cancel.clone());
        match self.worker.submit(Task::Prepare(Box::new(request))) {
            Ok(()) => {
                self.worker_pending = true;
                self.stop_transport();
                self.busy = true;
                self.prepared = None;
                self.texture = None;
                self.shown = None;
                self.failed_frame = None;
                true
            }
            Err(error) => {
                self.status = error;
                self.cancel = None;
                false
            }
        }
    }
    pub fn take_play_started(&mut self) -> bool {
        std::mem::take(&mut self.play_started)
    }
    fn raster_sample(&self) -> Option<u64> {
        raster_sample(
            self.desired,
            self.prepared.as_ref()?.timeline.total_samples(),
        )
    }
    fn needs_frame(&self, sample: u64) -> bool {
        !self.busy
            && !self.frame_pending
            && !self.worker_pending
            && self.shown != Some(sample)
            && self.failed_frame != Some(sample)
    }
    fn needs_repaint(&self) -> bool {
        self.busy || self.worker_pending || self.frame_pending || self.playing
    }
    pub fn busy(&self) -> bool {
        self.busy
    }
    pub fn invalidate(&mut self, source: String) {
        if self.source == source {
            return;
        }
        self.source = source;
        self.generation = self.generation.wrapping_add(1);
        if let Some(cancel) = self.cancel.take() {
            cancel.cancel();
        }
        if let Some(cancel) = self.frame_cancel.take() {
            cancel.cancel();
        }
        if let Some(player) = self.player.take() {
            let _ = player.stop();
        }
        self.prepared = None;
        self.texture = None;
        self.shown = None;
        self.failed_frame = None;
        self.playing = false;
        self.busy = false;
        self.frame_pending = false;
        self.desired = 0;
        self.status = "Project changed. Prepare again before playback or export.".into();
    }
    fn seek(&mut self, frame: u64) {
        self.generation = self.generation.wrapping_add(1);
        if let Some(cancel) = self.frame_cancel.take() {
            cancel.cancel();
        }
        self.frame_pending = false;
        self.desired = frame;
        self.texture = None;
        self.shown = None;
        self.failed_frame = None;
        self.tick = Instant::now();
        self.fraction = 0.0;
        if let Some(player) = self.player.take() {
            let _ = player.stop();
        }
        if self.playing && !self.silent {
            if let Some(prepared) = &self.prepared {
                match playback::Playback::new().and_then(|player| {
                    player.play(Arc::clone(&prepared.audio), frame)?;
                    Ok(player)
                }) {
                    Ok(player) => self.player = Some(player),
                    Err(error) => {
                        self.silent = true;
                        self.status = format!("Silent visual transport: {error}");
                    }
                }
            }
        }
    }
    pub fn tick(&mut self, ctx: &egui::Context) {
        while let Some(update) = self.worker.poll() {
            if matches!(
                &update,
                Update::Prepared { .. } | Update::Frame { .. } | Update::Exported { .. }
            ) {
                self.worker_pending = false;
            }
            match update {
                Update::PrepareProgress {
                    generation,
                    completed,
                    total,
                } if generation == self.generation => {
                    self.status = format!("Preparing scene {completed}/{total}")
                }
                Update::Prepared { generation, result } if generation == self.generation => {
                    self.busy = false;
                    self.cancel = None;
                    match result {
                        Ok(prepared) => {
                            self.prepared = Some(prepared);
                            self.status =
                                "Project prepared. Shared 24 kHz sequence clock ready.".into();
                            self.desired = 0;
                        }
                        Err(error) => self.status = error,
                    }
                }
                Update::Frame {
                    generation,
                    sample_frame,
                    result,
                } if generation == self.generation => {
                    self.frame_pending = false;
                    self.frame_cancel = None;
                    if Some(sample_frame) == self.raster_sample() || self.playing {
                        match result {
                            Ok(frame) => {
                                self.texture = Some(ctx.load_texture(
                                    "project-sequence-frame",
                                    egui::ColorImage::from_rgba_unmultiplied(
                                        [frame.width as usize, frame.height as usize],
                                        &frame.rgba,
                                    ),
                                    egui::TextureOptions::LINEAR,
                                ));
                                self.shown = Some(sample_frame);
                            }
                            Err(error) => {
                                self.failed_frame = Some(sample_frame);
                                self.status = error;
                            }
                        }
                    }
                }
                Update::ExportProgress {
                    generation,
                    progress,
                } if generation == self.generation => {
                    self.status = format!(
                        "Exporting {}/{} frames",
                        progress.completed_frames, progress.total_frames
                    )
                }
                Update::Exported { generation, result } if generation == self.generation => {
                    self.busy = false;
                    self.cancel = None;
                    self.status = match result {
                        Ok(report) => format!(
                            "Exported {} frames to {}",
                            report.frame_count,
                            report.output_directory.display()
                        ),
                        Err(error) => error,
                    };
                }
                _ => {}
            }
        }
        if self.playing {
            let end = self
                .prepared
                .as_ref()
                .map(|p| p.timeline.total_samples())
                .unwrap_or(0);
            if !self.silent {
                if let Some(player) = &self.player {
                    let state = player.snapshot();
                    if let Some(error) = state.error {
                        self.silent = true;
                        self.status = format!("Silent visual transport: {error}");
                        self.tick = Instant::now();
                    } else if state.loaded {
                        self.desired = state.position_frame.min(end);
                        self.playing = !state.finished;
                    }
                }
            }
            if self.silent {
                let now = Instant::now();
                let frames = now.duration_since(self.tick).as_secs_f64() * 24000.0 + self.fraction;
                self.tick = now;
                self.fraction = frames.fract();
                self.desired = self.desired.saturating_add(frames as u64).min(end);
                self.playing = self.desired < end;
            }
        }
        if let Some(sample_frame) = self
            .raster_sample()
            .filter(|sample| self.needs_frame(*sample))
        {
            if let Some(prepared) = &self.prepared {
                let cancel = CancellationToken::new();
                if self
                    .worker
                    .submit(Task::Frame {
                        generation: self.generation,
                        sample_frame,
                        prepared: Arc::clone(prepared),
                        cancel: cancel.clone(),
                    })
                    .is_ok()
                {
                    self.worker_pending = true;
                    self.frame_pending = true;
                    self.frame_cancel = Some(cancel);
                }
            }
        }
        if self.needs_repaint() {
            ctx.request_repaint_after(std::time::Duration::from_millis(25));
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, locked: bool, can_prepare: bool) -> bool {
        let mut started = false;
        ui.collapsing("Whole project · sequence", |ui| {
            ui.label("Prepare uses your configured local Kokoro runtime. No automatic download.");
            if ui
                .add_enabled(
                    !locked
                        && !self.busy
                        && !self.worker_pending
                        && !self.frame_pending
                        && can_prepare,
                    egui::Button::new("Prepare whole project"),
                )
                .clicked()
            {
                self.prepare_requested = true;
            }
            if self.busy && ui.button("Cancel sequence task").clicked() {
                if let Some(cancel) = &self.cancel {
                    cancel.cancel();
                }
            }
            ui.label(&self.status);
            if let Some(prepared) = self.prepared.clone() {
                ui.label(format!("Alignment: {:?}", prepared.alignment_statuses));
                let end = prepared.timeline.total_samples();
                crate::waveform(ui, &prepared.peaks, self.desired as f64 / end.max(1) as f64);
                ui.add_enabled_ui(!locked && !self.busy, |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .button(if self.playing { "Pause" } else { "Play" })
                            .clicked()
                        {
                            if self.playing {
                                if let Some(player) = &self.player {
                                    let _ = player.pause();
                                }
                                self.playing = false;
                            } else {
                                if self.desired >= end {
                                    self.seek(0);
                                }
                                self.tick = Instant::now();
                                self.playing = true;
                                self.play_started = true;
                                match playback::Playback::new().and_then(|player| {
                                    player.play(Arc::clone(&prepared.audio), self.desired)?;
                                    Ok(player)
                                }) {
                                    Ok(player) => {
                                        self.player = Some(player);
                                        self.silent = false;
                                    }
                                    Err(error) => {
                                        self.player = None;
                                        self.silent = true;
                                        self.status = format!("Silent visual transport: {error}");
                                    }
                                }
                            }
                        }
                        if ui.button("Restart").clicked() {
                            self.seek(0);
                        }
                    });
                    let mut sample = self.desired;
                    if ui
                        .add(egui::Slider::new(&mut sample, 0..=end).text("global sample / 24 kHz"))
                        .changed()
                    {
                        self.seek(sample);
                    }
                    ui.label(format!(
                        "{:.3} / {:.3} s",
                        self.desired as f64 / 24000.0,
                        end as f64 / 24000.0
                    ));
                    ui.label("New absolute output directory");
                    ui.text_edit_singleline(&mut self.destination);
                    ui.checkbox(&mut self.mp4, "MP4 + WAV (otherwise PNG + WAV)");
                    if self.mp4 {
                        ui.label("Absolute path to local FFmpeg");
                        ui.text_edit_singleline(&mut self.ffmpeg);
                    }
                    if ui
                        .add_enabled(
                            !self.frame_pending && !self.worker_pending,
                            egui::Button::new("Export whole project"),
                        )
                        .clicked()
                    {
                        let destination = std::path::PathBuf::from(self.destination.trim());
                        let ffmpeg = self
                            .mp4
                            .then(|| std::path::PathBuf::from(self.ffmpeg.trim()));
                        if !destination.is_absolute()
                            || ffmpeg.as_ref().is_some_and(|p| !p.is_absolute())
                        {
                            self.status = "Use absolute output and FFmpeg paths.".into();
                        } else {
                            if let Some(player) = &self.player {
                                let _ = player.stop();
                            }
                            self.playing = false;
                            let cancel = CancellationToken::new();
                            match self.worker.submit(Task::Export {
                                generation: self.generation,
                                prepared: Arc::clone(&prepared),
                                destination,
                                ffmpeg,
                                cancel: cancel.clone(),
                            }) {
                                Ok(()) => {
                                    self.worker_pending = true;
                                    self.cancel = Some(cancel);
                                    self.busy = true;
                                    started = true;
                                }
                                Err(error) => self.status = error,
                            }
                        }
                    }
                });
            }
            if let Some(texture) = &self.texture {
                let size = texture.size_vec2();
                let scale = (ui.available_width() / size.x).min(1.0);
                ui.image((texture.id(), size * scale));
            } else if self.prepared.is_some() {
                ui.label("Rendering current position…");
            }
        });
        started
    }
}

fn raster_sample(desired: u64, total: u64) -> Option<u64> {
    total.checked_sub(1).map(|last| desired.min(last))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_changes_cancel_and_reject_old_generation() {
        let mut panel = Panel::default();
        let cancel = CancellationToken::new();
        panel.cancel = Some(cancel.clone());
        panel.busy = true;
        panel.invalidate("new source and art".into());
        assert!(cancel.is_cancelled());
        assert!(!panel.busy());
        assert_eq!(panel.generation, 1);
        panel.invalidate("new source and art".into());
        assert_eq!(panel.generation, 1);
        panel.invalidate("new runtime".into());
        assert_eq!(panel.generation, 2);
    }
    #[test]
    fn eof_uses_last_half_open_frame_without_changing_transport() {
        assert_eq!(raster_sample(24000, 24000), Some(23999));
        assert_eq!(raster_sample(0, 0), None);
        assert_eq!(raster_sample(120, 24000), Some(120));
        let panel = Panel {
            desired: 24000,
            shown: Some(23999),
            ..Panel::default()
        };
        assert!(!panel.needs_frame(raster_sample(panel.desired, 24000).unwrap()));
        assert_eq!(panel.desired, 24000);
    }
    #[test]
    fn failed_frame_is_not_retried_until_seek_or_invalidation() {
        let mut panel = Panel {
            failed_frame: Some(123),
            ..Panel::default()
        };
        assert!(!panel.needs_frame(123));
        assert!(panel.needs_frame(124));
        panel.seek(123);
        assert!(panel.needs_frame(123));
        panel.failed_frame = Some(123);
        panel.invalidate("new art".into());
        assert!(panel.needs_frame(123));
    }
    #[test]
    fn paused_seek_keeps_polling_canceled_outstanding_frame() {
        let mut panel = Panel::default();
        let cancel = CancellationToken::new();
        panel.worker_pending = true;
        panel.frame_pending = true;
        panel.frame_cancel = Some(cancel.clone());
        panel.seek(99);
        assert!(cancel.is_cancelled());
        assert!(panel.needs_repaint());
        assert!(!panel.needs_frame(99));
        panel.worker_pending = false; // terminal result has been polled, even if its generation is stale
        assert!(panel.needs_frame(99));
        assert!(!panel.needs_repaint());
    }
    #[test]
    fn headless_layout_never_prepares() {
        let mut panel = Panel::default();
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!panel.ui(ui, false, false));
            });
        });
        assert!(!panel.busy());
        assert!(panel.prepared.is_none());
        assert!(!panel.take_prepare_requested());
    }
}
