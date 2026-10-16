//! Sequential, bounded project PNG/WAV interchange export. No synthesis or encoder.
use super::*;
use motion_scene::SequenceTimeline;
#[derive(Serialize)]
struct Contribution {
    scene_index: usize,
    local_sample: u64,
    weight: f64,
}
#[derive(Serialize)]
struct Frame {
    frame: u64,
    sample_frame: u64,
    file: String,
    contributions: Vec<Contribution>,
}
#[derive(Serialize)]
struct Span {
    scene_index: usize,
    scene_id: String,
    start_sample: u64,
    duration_samples: u64,
    outgoing_overlap: u64,
}
#[derive(Serialize)]
struct Manifest {
    schema_version: u32,
    sample_rate: u32,
    fps: u32,
    width: u32,
    height: u32,
    total_samples: u64,
    audio_file: String,
    scenes: Vec<Span>,
    frames: Vec<Frame>,
}
fn image(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    index: usize,
    sample: u64,
) -> Result<resvg::tiny_skia::Pixmap, Error> {
    let mut pose = motion_scene::evaluate(script, index, sample as f64 / f64::from(SAMPLE_RATE))
        .map_err(|e| invalid(e.to_string()))?;
    let mouths = mouths_at(&rendered[index].manifest, sample);
    motion_scene::apply_speech(&mut pose, &mouths);
    let svg = match art {
        Some(a) => motion_render::render_svg_with_art(
            &pose,
            pose.width,
            pose.height,
            &mouths,
            &script.scenes[index].id,
            &a.document,
            &a.images,
        ),
        None => motion_render::render_svg_with_mouths(&pose, pose.width, pose.height, &mouths),
    }
    .map_err(|e| invalid(e.to_string()))?;
    resvg::tiny_skia::Pixmap::decode_png(&media::rasterize(&svg)?)
        .map_err(|e| invalid(e.to_string()))
}
/// Blocking sequential PNG/WAV export on the shared sample clock. Complete opaque
/// images are mixed by channel sums, never by repeated source-over. MP4/SVG
/// sequences and shared parallel workers are not implemented by this operation.
pub fn export_png_sequence(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    destination: &Path,
    cancel: &CancellationToken,
    mut progress: impl FnMut(Progress),
) -> Result<Report, Error> {
    check(cancel)?;
    if rendered.len() != script.scenes.len() {
        return Err(invalid("one rendered scene required per scene"));
    }
    let timeline = SequenceTimeline::compile(script).map_err(|e| invalid(e.to_string()))?;
    let total_samples = timeline.total_samples();
    let total = total_samples
        .checked_mul(u64::from(script.project.fps))
        .and_then(|n| n.checked_add(u64::from(SAMPLE_RATE) - 1))
        .ok_or_else(|| invalid("frame count overflow"))?
        / u64::from(SAMPLE_RATE);
    let pixels = u64::from(script.project.width) * u64::from(script.project.height);
    if total == 0
        || total > MAX_FRAMES
        || pixels > 16_000_000
        || pixels
            .checked_mul(total)
            .and_then(|n| n.checked_mul(2))
            .is_none_or(|n| n > 1_000_000_000)
    {
        return Err(invalid("sequence frame or pixel-work budget exceeded"));
    }
    let source_samples = timeline.spans().iter().try_fold(0u64, |n, s| {
        n.checked_add(s.duration_samples)
            .ok_or_else(|| invalid("source audio size overflow"))
    })?;
    let source_bytes = source_samples
        .checked_mul(4)
        .ok_or_else(|| invalid("source audio byte overflow"))?;
    // Conservative working set includes retained source/mixed PCM, two RGBA frames,
    // raster/PNG scratch and retained artwork encoding/decoding copies.
    let mut working = pixels
        .checked_mul(24)
        .and_then(|n| total_samples.checked_mul(4).and_then(|a| n.checked_add(a)))
        .and_then(|n| n.checked_add(source_bytes))
        .ok_or_else(|| invalid("sequence working-set overflow"))?;
    if working > MAX_BYTES {
        return Err(invalid("sequence estimated working set exceeds 256 MiB"));
    }
    for (index, scene) in rendered.iter().enumerate() {
        validate(script, index, scene)?;
    }
    if let Some(a) = art {
        a.document.validate().map_err(|e| invalid(e.to_string()))?;
        let mut geometry = 0u64;
        for layer in &a.document.layers {
            let scene = script
                .scenes
                .iter()
                .find(|s| s.id == layer.scene_id)
                .ok_or_else(|| invalid("unknown artwork scene"))?;
            if layer.active.is_some_and(|r| r.end > scene.duration) {
                return Err(invalid("artwork visibility exceeds scene"));
            }
            geometry = geometry
                .checked_add(match &layer.content {
                    motion_art::Content::Stroke { points, .. } => points.len() as u64,
                    _ => 1,
                })
                .ok_or_else(|| invalid("art geometry overflow"))?;
            if layer.visible {
                if let motion_art::Content::Image { asset_id, .. } = &layer.content {
                    if !a.images.contains_key(asset_id) {
                        return Err(invalid("missing future artwork image"));
                    }
                }
            }
        }
        if geometry.checked_mul(total).is_none_or(|n| n > 4_000_000) {
            return Err(invalid("artwork frame-work budget exceeded"));
        }
        let bytes = a.images.values().try_fold(0usize, |n, i| {
            n.checked_add(i.png().len())
                .ok_or_else(|| invalid("image byte overflow"))
        })?;
        let image_pixels = a.images.values().try_fold(0u64, |n, i| {
            n.checked_add(u64::from(i.width()) * u64::from(i.height()))
                .ok_or_else(|| invalid("image pixel overflow"))
        })?;
        working = working
            .checked_add(
                (bytes as u64)
                    .checked_mul(4)
                    .ok_or_else(|| invalid("art working-set overflow"))?,
            )
            .and_then(|n| image_pixels.checked_mul(8).and_then(|p| n.checked_add(p)))
            .ok_or_else(|| invalid("art working-set overflow"))?;
        if working > MAX_BYTES {
            return Err(invalid("sequence artwork working set exceeds 256 MiB"));
        }
        if bytes > 32 * 1024 * 1024 || image_pixels > 16_000_000 {
            return Err(invalid("aggregate artwork image budget exceeded"));
        }
    }
    for index in 0..rendered.len() {
        check(cancel)?;
        image(script, rendered, art, index, 0)?;
    }
    let audio = motion_voice::sequence::mix_sequence(&timeline, rendered, cancel).map_err(|e| {
        if cancel.is_cancelled() {
            Error::Cancelled
        } else {
            invalid(e.to_string())
        }
    })?;
    check(cancel)?;
    if destination.exists() {
        return Err(invalid("destination already exists"));
    }
    fs::create_dir(destination)?;
    let result = (|| {
        let mut used = 44u64
            .checked_add(
                (audio.samples.len() as u64)
                    .checked_mul(2)
                    .ok_or_else(|| invalid("audio size overflow"))?,
            )
            .ok_or_else(|| invalid("audio size overflow"))?;
        if used > MAX_BYTES {
            return Err(invalid("sequence output byte budget exceeded"));
        }
        motion_audio::write_wav(destination.join("audio.wav"), &audio)
            .map_err(|e| invalid(e.to_string()))?;
        let mut frames = Vec::new();
        for frame in 0..total {
            check(cancel)?;
            let sample = frame
                .checked_mul(u64::from(SAMPLE_RATE))
                .ok_or_else(|| invalid("frame clock overflow"))?
                / u64::from(script.project.fps);
            let contributions = timeline
                .sample(sample)
                .map_err(|e| invalid(e.to_string()))?;
            if contributions.is_empty() || contributions.len() > 2 {
                return Err(invalid("invalid sequence contributions"));
            }
            let mut images = Vec::with_capacity(2);
            let mut entries = Vec::new();
            for c in contributions {
                check(cancel)?;
                let src = image(script, rendered, art, c.scene_index, c.local_sample)?;
                if src.width() != script.project.width
                    || src.height() != script.project.height
                    || src.data().as_chunks::<4>().0.iter().any(|p| p[3] != 255)
                {
                    return Err(invalid(
                        "sequence frames must be complete opaque RGBA images",
                    ));
                }
                images.push(src);
                entries.push(Contribution {
                    scene_index: c.scene_index,
                    local_sample: c.local_sample,
                    weight: c.weight,
                });
            }
            let mut out = images.remove(0);
            if let Some(incoming) = images.first() {
                for (value, next) in out.data_mut().iter_mut().zip(incoming.data()) {
                    *value = (f64::from(*value) * entries[0].weight
                        + f64::from(*next) * entries[1].weight)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                }
            }
            let bytes = out.encode_png().map_err(|e| invalid(e.to_string()))?;
            used = used
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| invalid("output byte overflow"))?;
            if used > MAX_BYTES {
                return Err(invalid("sequence output byte budget exceeded"));
            }
            let file = format!("frame-{frame:06}.png");
            write_new(&destination.join(&file), &bytes)?;
            frames.push(Frame {
                frame,
                sample_frame: sample,
                file,
                contributions: entries,
            });
            progress(Progress {
                completed_frames: frame + 1,
                total_frames: total,
            });
        }
        check(cancel)?;
        let manifest = bounded_json(
            &Manifest {
                schema_version: 1,
                sample_rate: SAMPLE_RATE,
                fps: script.project.fps,
                width: script.project.width,
                height: script.project.height,
                total_samples,
                audio_file: "audio.wav".into(),
                scenes: timeline
                    .spans()
                    .iter()
                    .map(|s| Span {
                        scene_index: s.scene_index,
                        scene_id: s.scene_id.clone(),
                        start_sample: s.start_sample,
                        duration_samples: s.duration_samples,
                        outgoing_overlap: s.outgoing_overlap,
                    })
                    .collect(),
                frames,
            },
            8 * 1024 * 1024,
        )?;
        if used
            .checked_add(manifest.len() as u64)
            .is_none_or(|n| n > MAX_BYTES)
        {
            return Err(invalid("sequence output byte budget exceeded"));
        }
        write_new(&destination.join("sequence.json"), &manifest)?;
        Ok(Report {
            frame_count: total,
            output_directory: destination.to_path_buf(),
        })
    })();
    if result.is_err() {
        fs::remove_dir_all(destination)
            .map_err(|e| invalid(format!("partial-output cleanup failed: {e}")))?;
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(fade: bool) -> (Script, Vec<RenderedScene>) {
        let (mut script, mut first) = crate::tests::fixture();
        script.project.width = 8;
        script.project.height = 8;
        script.characters.clear();
        first.manifest.dialogue.clear();
        script.scenes[0].composition.background_color = "#ff0000".into();
        let mut second = script.scenes[0].clone();
        second.id = "second".into();
        second.composition.background_color = "#0000ff".into();
        if fade {
            script.scenes[0].composition.transition =
                motion_script::Transition::Crossfade { duration: 0.5 };
        }
        script.scenes.push(second);
        let second = RenderedScene {
            audio: first.audio.clone(),
            manifest: SceneManifest {
                scene_id: "second".into(),
                ..first.manifest.clone()
            },
        };
        (script, vec![first, second])
    }
    fn pixel(path: &Path, frame: u64) -> [u8; 4] {
        let png = fs::read(path.join(format!("frame-{frame:06}.png"))).unwrap();
        let pix = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
        pix.data()[0..4].try_into().unwrap()
    }
    #[test]
    fn decoded_cut_crossfade_and_exact_sample_manifest() {
        for fade in [false, true] {
            let (script, rendered) = fixture(fade);
            let dest = crate::tests::destination();
            let report = export_png_sequence(
                &script,
                &rendered,
                None,
                &dest,
                &CancellationToken::new(),
                |_| {},
            )
            .unwrap();
            assert_eq!(pixel(&dest, 0), [255, 0, 0, 255]);
            if fade {
                assert_eq!(report.frame_count, 6);
                assert_eq!(pixel(&dest, 2), [255, 0, 0, 255]);
                assert_eq!(pixel(&dest, 3), [128, 0, 128, 255]);
                assert_eq!(pixel(&dest, 4), [0, 0, 255, 255]);
            } else {
                assert_eq!(report.frame_count, 8);
                assert_eq!(pixel(&dest, 3), [255, 0, 0, 255]);
                assert_eq!(pixel(&dest, 4), [0, 0, 255, 255]);
            }
            let m: serde_json::Value =
                serde_json::from_slice(&fs::read(dest.join("sequence.json")).unwrap()).unwrap();
            assert_eq!(m["frames"][3]["sample_frame"], 18000);
            if fade {
                assert_eq!(m["frames"][3]["contributions"][1]["local_sample"], 6000);
            }
            assert_eq!(
                motion_audio::read_wav(dest.join("audio.wav"))
                    .unwrap()
                    .frames() as u64,
                if fade { 36000 } else { 48000 }
            );
            fs::remove_dir_all(&dest).unwrap();
        }
    }
    #[test]
    fn timed_art_is_composited_before_complete_frame_crossfade() {
        use motion_art::{Color, Content, Layer, Plane, Rect, TimeRange};
        let (script, rendered) = fixture(true);
        let mut art = ArtContext::default();
        art.document.layers.push(Layer {
            id: "timed".into(),
            scene_id: "second".into(),
            name: "timed".into(),
            visible: true,
            plane: Plane::Foreground,
            active: Some(TimeRange {
                start: 0.25,
                end: 0.5,
            }),
            opacity: 1.0,
            attachment: None,
            clip: None,
            content: Content::Rectangle {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 8.0,
                    height: 8.0,
                },
                color: Color {
                    r: 0,
                    g: 255,
                    b: 0,
                    a: 255,
                },
            },
        });
        let dest = crate::tests::destination();
        export_png_sequence(
            &script,
            &rendered,
            Some(&art),
            &dest,
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap();
        assert_eq!(pixel(&dest, 2), [255, 0, 0, 255]);
        assert_eq!(pixel(&dest, 3), [128, 128, 0, 255]);
        assert_eq!(pixel(&dest, 4), [0, 0, 255, 255]);
        fs::remove_dir_all(&dest).unwrap();
        art.document.layers[0].active = Some(TimeRange {
            start: 1.0,
            end: 2.0,
        });
        assert!(export_png_sequence(
            &script,
            &rendered,
            Some(&art),
            &dest,
            &CancellationToken::new(),
            |_| {}
        )
        .is_err());
        assert!(!dest.exists());
    }
    #[test]
    fn cancellation_removes_only_owned_output_and_preserves_existing() {
        let (script, rendered) = fixture(true);
        let dest = crate::tests::destination();
        let token = CancellationToken::new();
        let result =
            export_png_sequence(&script, &rendered, None, &dest, &token, |_| token.cancel());
        assert!(matches!(result, Err(Error::Cancelled)));
        assert!(!dest.exists());
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("keep"), b"keep").unwrap();
        assert!(export_png_sequence(
            &script,
            &rendered,
            None,
            &dest,
            &CancellationToken::new(),
            |_| {}
        )
        .is_err());
        assert!(dest.join("keep").exists());
        fs::remove_dir_all(&dest).unwrap();
    }
    #[test]
    fn future_scene_audio_and_resource_limits_preflight_before_creation() {
        let (mut script, mut rendered) = fixture(false);
        let dest = crate::tests::destination();
        rendered[1].manifest.scene_id = "wrong".into();
        assert!(export_png_sequence(
            &script,
            &rendered,
            None,
            &dest,
            &CancellationToken::new(),
            |_| {}
        )
        .is_err());
        assert!(!dest.exists());
        rendered[1].manifest.scene_id = "second".into();
        script.project.width = 16384;
        script.project.height = 16384;
        assert!(export_png_sequence(
            &script,
            &rendered,
            None,
            &dest,
            &CancellationToken::new(),
            |_| {}
        )
        .is_err());
        assert!(!dest.exists());
    }
}
