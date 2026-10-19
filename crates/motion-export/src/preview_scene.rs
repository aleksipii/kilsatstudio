//! Immutable selected-scene preview, using the SVG composition/raster path of export.
use super::*;
use std::sync::Arc;

/// Construct off the UI thread after source, art, resources or speech changes.
/// This snapshot retains no PCM and never starts synthesis or loads files.
pub struct ScenePreview {
    script: Arc<Script>,
    index: usize,
    art: Arc<ArtContext>,
    manifest: Option<Arc<SceneManifest>>,
    validated: motion_scene::ValidatedScript,
    total_samples: u64,
}
impl ScenePreview {
    pub fn new(
        script: Arc<Script>,
        index: usize,
        art: Arc<ArtContext>,
        manifest: Option<Arc<SceneManifest>>,
    ) -> Result<Self, Error> {
        bounded_json(script.as_ref(), MAX_SOURCE)?;
        let events = script.scenes.iter().map(|s| s.events.len()).sum::<usize>();
        if events > 10_000
            || script.characters.len() > 256
            || script.scenes.len() > 10_000
            || events.saturating_mul(script.characters.len() + 1) > 1_000_000
        {
            return Err(invalid("preview script work budget exceeded"));
        }
        if live_clip_frame_work(&script, index).saturating_add(rigid_frame_work(&script))
            > 1_000_000
        {
            return Err(invalid("preview live clip sample work budget exceeded"));
        }
        let validated = motion_scene::ValidatedScript::new(script.clone())
            .map_err(|e| invalid(e.to_string()))?;
        let scene = script
            .scenes
            .get(index)
            .ok_or_else(|| invalid("missing preview scene"))?;
        let total_samples = (scene.duration * f64::from(SAMPLE_RATE)).round() as u64;
        if total_samples == 0 || scene.duration > motion_voice::export::MAX_SCENE_SECONDS {
            return Err(invalid("invalid preview duration"));
        }
        if u64::from(script.project.width) * u64::from(script.project.height) > 16_000_000 {
            return Err(invalid("preview source canvas exceeds 16 million pixels"));
        }
        art.document
            .validate()
            .map_err(|e| invalid(e.to_string()))?;
        let group_bytes = group_work(&art, &script, 1, false)?;
        let mut geometry = 0usize;
        for layer in &art.document.layers {
            let owner = script
                .scenes
                .iter()
                .find(|s| s.id == layer.scene_id)
                .ok_or_else(|| invalid("unknown artwork scene"))?;
            if layer.active.is_some_and(|r| r.end > owner.duration) {
                return Err(invalid("artwork visibility exceeds scene duration"));
            }
            if let Some(shape) = &layer.clip_shape {
                shape
                    .validate_scene_duration(owner.duration)
                    .map_err(|error| invalid(error.to_string()))?;
            }
            geometry += match &layer.content {
                motion_art::Content::Stroke { points, .. } => points.len(),
                _ => 1,
            };
            if let Some(motion_art::ClipShape::Polygon { points, .. }) = &layer.clip_shape {
                geometry += points.len();
            }
            if let Some(motion_art::ClipShape::Painted { strokes, .. }) = &layer.clip_shape {
                geometry += strokes
                    .iter()
                    .map(motion_art::MaskStroke::render_cost)
                    .sum::<usize>()
                    + 1;
            }
            if let Some(shape @ (motion_art::ClipShape::SoftEllipse { .. } | motion_art::ClipShape::Image { .. })) = &layer.clip_shape {
                geometry += (usize::BITS - shape.key_count().max(1).leading_zeros()) as usize + 8;
            }
            if layer.visible {
                for asset_id in layer.image_assets() {
                    if !art.images.contains_key(asset_id) {
                        return Err(invalid("missing artwork image resource"));
                    }
                }
            }
        }
        let bytes = art
            .images
            .values()
            .map(|i| i.png().len() as u64)
            .sum::<u64>();
        let pixels = art
            .images
            .values()
            .map(|i| u64::from(i.width()) * u64::from(i.height()))
            .sum::<u64>();
        if geometry > 4_000_000
            || bytes > 32 * 1024 * 1024
            || pixels > 16_000_000
            || bytes
                .saturating_mul(4)
                .saturating_add(pixels.saturating_mul(8))
                .saturating_add(1024 * 1024 * 24)
                .saturating_add(group_bytes)
                > MAX_BYTES
        {
            return Err(invalid("preview artwork working-set budget exceeded"));
        }
        if let Some(m) = manifest.as_deref() {
            bounded_json(m, 8 * MAX_SOURCE)?;
            if m.schema_version != 1
                || m.scene_id != scene.id
                || m.sample_rate != SAMPLE_RATE
                || m.channels != 1
                || m.total_frames != total_samples
                || m.dialogue.len() > MAX_DIALOGUE
            {
                return Err(invalid(
                    "preview manifest identity or sample clock mismatch",
                ));
            }
            let mut tracks: BTreeMap<&str, Vec<(u64, u64)>> = BTreeMap::new();
            let mut intervals = 0usize;
            for clip in &m.dialogue {
                let end = clip
                    .start_frame
                    .checked_add(clip.duration_frames)
                    .ok_or_else(|| invalid("dialogue end overflow"))?;
                if !script.characters.contains_key(&clip.actor)
                    || !clip.gain.is_finite()
                    || end > total_samples
                    || clip
                        .trim_start_frame
                        .checked_add(clip.duration_frames)
                        .is_none()
                    || (!clip.visemes.is_empty() && clip.alignment_source.is_none())
                {
                    return Err(invalid("invalid preview dialogue bounds or provenance"));
                }
                if let Some(
                    AlignmentSource::Forced { tool, version }
                    | AlignmentSource::Estimated { tool, version },
                ) = &clip.alignment_source
                {
                    if tool.trim().is_empty()
                        || version.trim().is_empty()
                        || tool.len() > 1024
                        || version.len() > 1024
                    {
                        return Err(invalid("invalid preview alignment provenance"));
                    }
                }
                intervals += clip.visemes.len();
                if intervals > 100_000 {
                    return Err(invalid("preview viseme budget exceeded"));
                }
                let mut previous = clip.start_frame;
                for v in &clip.visemes {
                    if v.start_frame < previous || v.start_frame >= v.end_frame || v.end_frame > end
                    {
                        return Err(invalid("invalid preview viseme interval"));
                    }
                    previous = v.end_frame;
                }
                if clip.duration_frames > 0 {
                    tracks
                        .entry(&clip.actor)
                        .or_default()
                        .push((clip.start_frame, end));
                }
            }
            for track in tracks.values_mut() {
                track.sort_unstable();
                if track.windows(2).any(|w| w[1].0 < w[0].1) {
                    return Err(invalid("overlapping preview dialogue for actor"));
                }
            }
        }
        Ok(Self {
            script,
            index,
            art,
            manifest,
            validated,
            total_samples,
        })
    }

    pub fn render(
        &self,
        sample_frame: u64,
        max_side: u32,
        cancel: &CancellationToken,
    ) -> Result<SequencePreviewFrame, Error> {
        self.render_with_mouths(sample_frame, max_side, None, cancel)
    }

    /// Use an already evaluated scene-clock mouth snapshot (for selected-line
    /// playback) without constructing audio or a fabricated scene manifest.
    /// None retains the optional validated manifest; Some replaces it completely.
    pub fn render_with_mouths(
        &self,
        sample_frame: u64,
        max_side: u32,
        mouth_override: Option<&[(String, Viseme)]>,
        cancel: &CancellationToken,
    ) -> Result<SequencePreviewFrame, Error> {
        check(cancel)?;
        if !(1..=1024).contains(&max_side) || sample_frame > self.total_samples {
            return Err(invalid("preview sample or output size out of bounds"));
        }
        let mut pose = self
            .validated
            .evaluate(self.index, sample_frame as f64 / f64::from(SAMPLE_RATE))
            .map_err(|e| invalid(e.to_string()))?;
        let mouths = match mouth_override {
            Some(override_mouths) => {
                if override_mouths.len() > 256 {
                    return Err(invalid("preview mouth override exceeds actor budget"));
                }
                let mut actors = std::collections::BTreeSet::new();
                for (actor, _) in override_mouths {
                    if !self.script.characters.contains_key(actor) || !actors.insert(actor.as_str())
                    {
                        return Err(invalid(
                            "preview mouth override requires unique known actors",
                        ));
                    }
                }
                override_mouths
                    .iter()
                    .map(|(actor, viseme)| (actor.as_str(), *viseme))
                    .collect()
            }
            None => self
                .manifest
                .as_deref()
                .map(|m| mouths_at(m, sample_frame))
                .unwrap_or_default(),
        };
        motion_scene::apply_speech(&mut pose, &mouths);
        let svg = motion_render::render_svg_with_art(
            &pose,
            pose.width,
            pose.height,
            &mouths,
            &self.script.scenes[self.index].id,
            &self.art.document,
            &self.art.images,
        )
        .map_err(|e| invalid(e.to_string()))?;
        check(cancel)?;
        let png = media::rasterize_scaled(&svg, Some(max_side))?;
        check(cancel)?;
        let pixmap =
            resvg::tiny_skia::Pixmap::decode_png(&png).map_err(|e| invalid(e.to_string()))?;
        Ok(SequencePreviewFrame {
            sample_frame,
            width: pixmap.width(),
            height: pixmap.height(),
            rgba: pixmap.take(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_preview_matches_export_pixels_and_speech_clock() {
        let (script, rendered) = crate::tests::fixture();
        let preview = ScenePreview::new(
            Arc::new(script.clone()),
            0,
            Arc::new(ArtContext::default()),
            Some(Arc::new(rendered.manifest.clone())),
        )
        .unwrap();
        let cancel = CancellationToken::new();
        for frame in 0..4 {
            let (sample, _, png) = encode_frame(
                &script,
                0,
                &rendered,
                frame,
                true,
                &cancel,
                Some(&ArtContext::default()),
            )
            .unwrap();
            let exported = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
            let result = preview.render(sample, 1024, &cancel).unwrap();
            assert_eq!(
                (result.width, result.height),
                (exported.width(), exported.height())
            );
            assert_eq!(result.rgba, exported.data());
        }
        let scaled = preview.render(6000, 128, &cancel).unwrap();
        assert_eq!((scaled.width, scaled.height), (128, 72));
        assert!(preview.render(0, 1025, &cancel).is_err());
        assert!(preview.render(24001, 128, &cancel).is_err());
        cancel.cancel();
        assert!(matches!(
            preview.render(0, 128, &cancel),
            Err(Error::Cancelled)
        ));
    }
    #[test]
    fn selected_line_mouth_override_matches_export_without_scene_audio() {
        let (script, rendered) = crate::tests::fixture();
        let preview = ScenePreview::new(
            Arc::new(script.clone()),
            0,
            Arc::new(ArtContext::default()),
            None,
        )
        .unwrap();
        let cancel = CancellationToken::new();
        for frame in 0..4 {
            let (sample, _, png) = encode_frame(
                &script,
                0,
                &rendered,
                frame,
                true,
                &cancel,
                Some(&ArtContext::default()),
            )
            .unwrap();
            let mouths: Vec<_> = mouths_at(&rendered.manifest, sample)
                .into_iter()
                .map(|(actor, viseme)| (actor.to_owned(), viseme))
                .collect();
            let expected = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
            assert_eq!(
                preview
                    .render_with_mouths(sample, 1024, Some(&mouths), &cancel)
                    .unwrap()
                    .rgba,
                expected.data()
            );
        }
        assert!(preview
            .render_with_mouths(0, 64, Some(&[("unknown".into(), Viseme::A)]), &cancel)
            .is_err());
        assert!(preview
            .render_with_mouths(
                0,
                64,
                Some(&[("EMMA".into(), Viseme::A), ("EMMA".into(), Viseme::O)]),
                &cancel
            )
            .is_err());
    }
    #[test]
    fn advanced_mask_blending_and_visibility_match_export() {
        use motion_art::{
            BlendMode, ClipShape, Color, Content, Document, Layer, Plane, Rect, TimeRange,
        };
        let (script, rendered) = crate::tests::fixture();
        let rect = Rect {
            x: 2.0,
            y: 2.0,
            width: 100.0,
            height: 80.0,
        };
        let layer = Layer {
            id: "masked".into(),
            scene_id: "intro".into(),
            name: "masked".into(),
            visible: true,
            plane: Plane::Foreground,
            active: Some(TimeRange {
                start: 0.25,
                end: 0.75,
            }),
            opacity: 0.65,
            attachment: None,
            clip: None,
            clip_shape: Some(ClipShape::Ellipse { rect }),
            blend: BlendMode::Multiply,
            content: Content::Rectangle {
                rect,
                color: Color {
                    r: 240,
                    g: 80,
                    b: 20,
                    a: 255,
                },
            },
        };
        let art = ArtContext {
            document: Document {
                layers: vec![layer],
                ..Default::default()
            },
            images: BTreeMap::new(),
        };
        let preview = ScenePreview::new(
            Arc::new(script.clone()),
            0,
            Arc::new(art.clone()),
            Some(Arc::new(rendered.manifest.clone())),
        )
        .unwrap();
        let cancel = CancellationToken::new();
        for frame in 0..4 {
            let (sample, _, png) =
                encode_frame(&script, 0, &rendered, frame, true, &cancel, Some(&art)).unwrap();
            let exported = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
            assert_eq!(
                preview.render(sample, 1024, &cancel).unwrap().rgba,
                exported.data()
            );
        }
        let off = preview.render(0, 1024, &cancel).unwrap();
        let on = preview.render(6000, 1024, &cancel).unwrap();
        let pixel = |f: &SequencePreviewFrame| {
            f.rgba[(30 * f.width as usize + 30) * 4..(30 * f.width as usize + 30) * 4 + 4].to_vec()
        };
        assert_ne!(pixel(&off), pixel(&on));
    }
    #[test]
    fn manifest_identity_and_intervals_are_checked_without_audio() {
        let (script, rendered) = crate::tests::fixture();
        let script = Arc::new(script);
        let art = Arc::new(ArtContext::default());
        let mut manifest = rendered.manifest.clone();
        manifest.scene_id = "stale".into();
        assert!(
            ScenePreview::new(script.clone(), 0, art.clone(), Some(Arc::new(manifest))).is_err()
        );
        let mut manifest = rendered.manifest.clone();
        manifest.dialogue[0].visemes[0].end_frame = 24_001;
        assert!(
            ScenePreview::new(script.clone(), 0, art.clone(), Some(Arc::new(manifest))).is_err()
        );
        let mut manifest = rendered.manifest.clone();
        manifest.dialogue[0].alignment_source = None;
        assert!(
            ScenePreview::new(script.clone(), 0, art.clone(), Some(Arc::new(manifest))).is_err()
        );
        let mut manifest = rendered.manifest;
        manifest.dialogue.push(manifest.dialogue[0].clone());
        assert!(ScenePreview::new(script, 0, art, Some(Arc::new(manifest))).is_err());
    }
}
