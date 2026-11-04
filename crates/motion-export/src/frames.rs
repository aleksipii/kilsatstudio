//! Full-size export frames and mixed audio for an encoder outside this crate.
//!
//! The FFmpeg exports stream frames from `sequence::compose`; this type exposes
//! the same frames one at a time, with the same mix, so a platform encoder (the
//! iPad app uses AVFoundation) writes a movie that matches the Mac export.
use super::*;
use motion_audio::AudioBuffer;
use motion_scene::SequenceTimeline;
use std::sync::Arc;

/// Longest movie: ten minutes, the sequence audio limit.
pub const MAX_EXPORT_SECONDS: u64 = 600;

/// A validated sequence ready to be encoded frame by frame.
pub struct ExportFrames {
    script: Arc<Script>,
    rendered: Arc<Vec<RenderedScene>>,
    art: Option<Arc<ArtContext>>,
    timeline: SequenceTimeline,
    validated: motion_scene::ValidatedScript,
    frame_count: u64,
}

impl ExportFrames {
    /// Every scene of `script` in order, with its crossfades. `rendered` holds
    /// one prepared scene per screenplay scene.
    pub fn new(
        script: Arc<Script>,
        rendered: Vec<RenderedScene>,
        art: Option<Arc<ArtContext>>,
        cancel: &CancellationToken,
    ) -> Result<Self, Error> {
        check(cancel)?;
        bounded_json(script.as_ref(), MAX_SOURCE)?;
        if let Some(art) = &art {
            art.document
                .validate()
                .map_err(|e| invalid(e.to_string()))?;
        }
        let (timeline, frame_count, _) = sequence::preflight(
            &script,
            &rendered,
            art.as_deref(),
            false,
            CaptionMode::Follow,
        )?;
        if timeline.total_samples() > MAX_EXPORT_SECONDS * u64::from(SAMPLE_RATE) {
            return Err(invalid(format!(
                "export is limited to {} minutes",
                MAX_EXPORT_SECONDS / 60
            )));
        }
        check(cancel)?;
        let validated = motion_scene::ValidatedScript::new(script.clone())
            .map_err(|e| invalid(e.to_string()))?;
        Ok(Self {
            script,
            rendered: Arc::new(rendered),
            art,
            timeline,
            validated,
            frame_count,
        })
    }

    /// One scene on its own, cut in and out, with only that scene's artwork,
    /// as the Mac's single-scene movie export.
    pub fn scene(
        script: &Script,
        index: usize,
        rendered: RenderedScene,
        art: Option<&ArtContext>,
        cancel: &CancellationToken,
    ) -> Result<Self, Error> {
        let (single, art) = single_scene(script, index, art)?;
        Self::new(Arc::new(single), vec![rendered], art.map(Arc::new), cancel)
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
    pub fn fps(&self) -> u32 {
        self.script.project.fps
    }
    pub fn width(&self) -> u32 {
        self.script.project.width
    }
    pub fn height(&self) -> u32 {
        self.script.project.height
    }

    /// Sequence sample shown by `frame`, the rule the FFmpeg exports use.
    pub fn frame_sample(&self, frame: u64) -> u64 {
        frame * u64::from(SAMPLE_RATE) / u64::from(self.fps())
    }

    /// Frame `frame` at the authored size, opaque RGBA8.
    pub fn render(
        &self,
        frame: u64,
        cancel: &CancellationToken,
    ) -> Result<SequencePreviewFrame, Error> {
        check(cancel)?;
        if frame >= self.frame_count {
            return Err(invalid("export frame out of range"));
        }
        let (image, _) = sequence::compose(
            &self.script,
            &self.rendered,
            self.art.as_deref(),
            &self.timeline,
            self.frame_sample(frame),
            None,
            cancel,
            Some(&self.validated),
            CaptionMode::Follow,
        )?;
        let budget = Some(sequence::reserve_frame(image.width(), image.height())?);
        Ok(SequencePreviewFrame {
            sample_frame: self.frame_sample(frame),
            width: image.width(),
            height: image.height(),
            rgba: image.take(),
            budget,
        })
    }

    /// The sequence mix, 24 kHz mono, as the FFmpeg exports write `audio.wav`.
    pub fn mix_audio(&self, cancel: &CancellationToken) -> Result<AudioBuffer, Error> {
        motion_voice::sequence::mix_sequence(&self.timeline, &self.rendered, cancel)
            .map_err(|e| invalid(e.to_string()))
    }
}

/// `script` reduced to scene `index` with a cut transition, and `art` reduced to
/// that scene's layers, groups and lights.
pub(crate) fn single_scene(
    script: &Script,
    index: usize,
    art: Option<&ArtContext>,
) -> Result<(Script, Option<ArtContext>), Error> {
    let selected = script
        .scenes
        .get(index)
        .ok_or_else(|| invalid("missing scene index"))?;
    let mut single = script.clone();
    let mut scene = selected.clone();
    scene.composition.transition = motion_script::Transition::Cut;
    single.scenes = vec![scene];
    let mut selected_art = art.cloned();
    if let Some(a) = &mut selected_art {
        a.document
            .layers
            .retain(|layer| layer.scene_id == selected.id);
        a.document
            .groups
            .retain(|group| group.scene_id == selected.id);
        a.document
            .lights
            .retain(|light| light.scene_id == selected.id);
    }
    Ok((single, selected_art))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"
project: { title: Frames, fps: 12, width: 64, height: 36 }
characters:
  EMMA: { asset: characters/emma, default_pose: idle }
scenes:
  - id: one
    duration: 1.0
    events: []
  - id: two
    duration: 0.5
    events: []
"#;

    fn silent(script: &Script) -> Vec<RenderedScene> {
        script
            .scenes
            .iter()
            .map(|s| motion_voice::export::render_scene(&s.id, s.duration, &[]).unwrap())
            .collect()
    }

    #[test]
    fn sequence_frames_match_the_preview_composition() {
        let script = Arc::new(motion_script::parse(SOURCE).unwrap());
        let rendered = silent(&script);
        let cancel = CancellationToken::new();
        let frames = ExportFrames::new(script.clone(), rendered.clone(), None, &cancel).unwrap();
        assert_eq!(frames.frame_count(), 18);
        assert_eq!(
            (frames.width(), frames.height(), frames.fps()),
            (64, 36, 12)
        );
        let preview = SequencePreview::new(script, Arc::new(rendered), None, &cancel).unwrap();
        for frame in [0, 5, 12, 17] {
            let exported = frames.render(frame, &cancel).unwrap();
            assert_eq!((exported.width, exported.height), (64, 36));
            assert!(exported.rgba.chunks(4).all(|p| p[3] == 255));
            let previewed = preview
                .render_frame(frames.frame_sample(frame), 64, &cancel)
                .unwrap();
            assert_eq!(exported.rgba, previewed.rgba, "frame {frame}");
        }
        assert!(frames.render(18, &cancel).is_err());
        let audio = frames.mix_audio(&cancel).unwrap();
        assert_eq!(audio.samples.len() as u64, 36_000);
    }

    #[test]
    fn single_scene_is_cut_from_the_sequence() {
        let script = motion_script::parse(SOURCE).unwrap();
        let rendered = silent(&script).remove(1);
        let cancel = CancellationToken::new();
        let frames = ExportFrames::scene(&script, 1, rendered, None, &cancel).unwrap();
        assert_eq!(frames.frame_count(), 6);
        assert!(ExportFrames::scene(&script, 2, silent(&script).remove(0), None, &cancel).is_err());
    }

    #[test]
    fn cancellation_stops_rendering() {
        let script = Arc::new(motion_script::parse(SOURCE).unwrap());
        let cancel = CancellationToken::new();
        let frames = ExportFrames::new(script.clone(), silent(&script), None, &cancel).unwrap();
        cancel.cancel();
        assert!(frames.render(0, &cancel).is_err());
        assert!(frames.mix_audio(&cancel).is_err());
    }
}
