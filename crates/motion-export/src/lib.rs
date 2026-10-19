//! Bounded, cancellable SVG/audio interchange export on the shared sample clock.
//! SVG, PNG and explicitly configured local FFmpeg video share the same scene clock.
#[cfg(test)]
mod artist_blending_tests;
#[cfg(test)]
mod clip_library_tests;
#[cfg(test)]
mod group_composition_tests;
#[cfg(test)]
mod live_clip_tests;
mod media;
mod preview_scene;
#[cfg(test)]
mod soft_mask_tests;
pub use preview_scene::ScenePreview;
mod sequence;
pub use media::{export_mp4_scene, export_mp4_scene_with_art};
use motion_lipsync::{AlignmentSource, Viseme};
use motion_script::Script;
use motion_tts::CancellationToken;
use motion_voice::export::{RenderedScene, SceneManifest, SAMPLE_RATE};
pub use sequence::{
    export_mp4_sequence, export_png_sequence, export_svg_sequence, render_sequence_frame,
    SequencePreview, SequencePreviewFrame,
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default)]
pub struct ArtContext {
    pub document: motion_art::Document,
    pub images: BTreeMap<String, motion_art::ImageResource>,
}

pub const MAX_FRAMES: u64 = 10_000;
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_SOURCE: usize = 1024 * 1024;
const MAX_DIALOGUE: usize = 10_000;

fn rigid_frame_work(script: &Script) -> u64 {
    script
        .characters
        .values()
        .filter_map(|character| character.rig.as_ref())
        .flat_map(|rig| &rig.parts)
        .fold(0u64, |work, part| {
            work.saturating_add(part.points.len() as u64 * 4)
        })
}

// Isolated group targets are additional raster work even if currently hidden.
// Budget original canvas dimensions before frame allocation or output creation.
fn group_work(art: &ArtContext, script: &Script, frames: u64, overlap: bool) -> Result<u64, Error> {
    let mut per_scene = BTreeMap::<&str, u64>::new();
    for group in &art.document.groups {
        if !script.scenes.iter().any(|scene| scene.id == group.scene_id) {
            return Err(invalid("unknown artwork group scene"));
        }
        *per_scene.entry(&group.scene_id).or_default() += 1;
    }
    let groups = per_scene.values().copied().max().unwrap_or(0);
    if groups == 0 {
        return Ok(0);
    }
    let pixels = u64::from(script.project.width) * u64::from(script.project.height);
    let simultaneous = if overlap { 2 } else { 1 };
    let visits = pixels.saturating_mul(groups).saturating_mul(simultaneous);
    let bytes = pixels
        .saturating_mul(4 * motion_art::MAX_GROUP_DEPTH as u64)
        .saturating_mul(simultaneous);
    if visits > 64 * 1024 * 1024
        || visits.saturating_mul(frames) > 1_000_000_000
        || bytes > 128 * 1024 * 1024
    {
        return Err(invalid(
            "artwork isolated-group raster work/memory budget exceeded",
        ));
    }
    Ok(bytes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub completed_frames: u64,
    pub total_frames: u64,
}
#[derive(Debug)]
pub struct Report {
    pub frame_count: u64,
    pub output_directory: PathBuf,
}
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("scene export cancelled")]
    Cancelled,
    #[error("invalid scene export: {0}")]
    Invalid(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}
fn check(cancel: &CancellationToken) -> Result<(), Error> {
    if cancel.is_cancelled() {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

/// Select already-trimmed, scene-relative half-open viseme intervals. Validation
/// happens before export; gaps, pre-roll and clip ends always evaluate to Rest.
fn mouths_at(manifest: &SceneManifest, sample: u64) -> Vec<(&str, Viseme)> {
    manifest
        .dialogue
        .iter()
        .filter_map(|clip| {
            if sample < clip.start_frame || sample >= clip.start_frame + clip.duration_frames {
                return None;
            }
            let mouth = clip
                .visemes
                .iter()
                .find(|v| v.start_frame <= sample && sample < v.end_frame)
                .map(|v| v.viseme)
                .unwrap_or(Viseme::Rest);
            Some((clip.actor.as_str(), mouth))
        })
        .collect()
}

// Conservative live-layer work per frame. Count all instances regardless of their
// current window/weight so seeks and later frames cannot bypass preflight. Every
// actor scans the instance list; each layer also visits seven joints, searches
// referenced tracks and blends their sampled local transforms.
pub(crate) fn live_clip_frame_work(script: &Script, scene_index: usize) -> u64 {
    let Some(scene) = script.scenes.get(scene_index) else {
        return u64::MAX;
    };
    let instances = &scene.composition.clip_instances;
    let scans = (instances.len() as u64).saturating_mul(script.characters.len() as u64);
    instances.iter().fold(scans, |work, instance| {
        let tracks = script
            .animation_clips
            .get(&instance.clip)
            .map_or(u64::MAX, |clip| {
                clip.bone_tracks.values().fold(7u64, |cost, keys| {
                    let search = u64::from(usize::BITS - keys.len().max(1).leading_zeros());
                    cost.saturating_add(search.saturating_add(4))
                })
            });
        let weight_search = if instance.weight_keys.is_empty() {
            0
        } else {
            u64::from(usize::BITS - instance.weight_keys.len().leading_zeros()).saturating_add(4)
        };
        let fade_work =
            u64::from(instance.fade_in > 0.0) * 2 + u64::from(instance.fade_out > 0.0) * 2;
        // Charge all seven possible joints, even when masked or dormant.
        let additive_work = match instance.blend_mode {
            motion_script::ClipBlendMode::Override => 0,
            motion_script::ClipBlendMode::Additive => 56,
        };
        // Group accumulation visits every joint and deterministically sorts
        // contributors by ID. Charge dormant/zero contributors as well; the
        // 128-row schema ceiling bounds comparison and rotation-reference work.
        let normalized_work = if instance.normalized_group.is_some() {
            112u64.saturating_add(7 * instances.len() as u64)
        } else {
            0
        };
        work.saturating_add(tracks)
            .saturating_add(weight_search)
            .saturating_add(fade_work)
            .saturating_add(7 * instance.joint_mask.len() as u64)
            .saturating_add(u64::from(instance.looping))
            .saturating_add(additive_work)
            .saturating_add(normalized_work)
    })
}

fn validate(script: &Script, index: usize, rendered: &RenderedScene) -> Result<u64, Error> {
    validate_mode(script, index, rendered, true)
}
fn validate_mode(
    script: &Script,
    index: usize,
    rendered: &RenderedScene,
    export: bool,
) -> Result<u64, Error> {
    let events = script
        .scenes
        .iter()
        .fold(0usize, |n, scene| n.saturating_add(scene.events.len()));
    if events > 10_000 || script.characters.len() > 256 || script.scenes.len() > 10_000 {
        return Err(invalid("actor, scene or event count budget exceeded"));
    }
    bounded_json(script, MAX_SOURCE)?;
    script.validate().map_err(|e| invalid(e.to_string()))?;
    let scene = script
        .scenes
        .get(index)
        .ok_or_else(|| invalid("missing scene index"))?;
    let count = if export {
        motion_scene::frame_count(scene.duration, script.project.fps, MAX_FRAMES)
            .map_err(|e| invalid(e.to_string()))?
    } else {
        1
    };
    if events > 10_000
        || script.characters.len() > 256
        || count.saturating_mul(script.characters.len() as u64) > 100_000
        || count
            .saturating_mul(events as u64)
            .saturating_mul(script.characters.len() as u64 + 1)
            > 1_000_000
    {
        return Err(invalid("frame, actor or event work budget exceeded"));
    }
    let keys = scene
        .composition
        .actor_tracks
        .values()
        .map(Vec::len)
        .sum::<usize>()
        + scene.composition.camera.len()
        + scene
            .composition
            .bone_tracks
            .values()
            .flat_map(|bones| bones.values())
            .map(Vec::len)
            .sum::<usize>();
    let rigid_work = rigid_frame_work(script);
    if count.saturating_mul(
        (keys as u64)
            .saturating_add(live_clip_frame_work(script, index))
            .saturating_add(rigid_work),
    ) > 1_000_000
    {
        return Err(invalid(
            "keyframe/live-clip evaluation exceeds one million work units",
        ));
    }
    let expected_samples = (scene.duration * SAMPLE_RATE as f64).round() as u64;
    let m = &rendered.manifest;
    if scene.duration > motion_voice::export::MAX_SCENE_SECONDS
        || expected_samples == 0
        || m.schema_version != 1
        || m.scene_id != scene.id
        || m.sample_rate != SAMPLE_RATE
        || m.channels != 1
        || m.total_frames != expected_samples
        || rendered.audio.sample_rate != SAMPLE_RATE
        || rendered.audio.channels != 1
        || rendered.audio.frames() as u64 != expected_samples
    {
        return Err(invalid(
            "scene identity, duration, schema or 24 kHz mono audio mismatch",
        ));
    }
    rendered
        .audio
        .validate()
        .map_err(|e| invalid(e.to_string()))?;
    if m.dialogue.len() > MAX_DIALOGUE {
        return Err(invalid("too many dialogue clips"));
    }
    let mut tracks: BTreeMap<&str, Vec<(u64, u64)>> = BTreeMap::new();
    let mut interval_count = 0usize;
    for clip in &m.dialogue {
        let end = clip
            .start_frame
            .checked_add(clip.duration_frames)
            .ok_or_else(|| invalid("clip end overflow"))?;
        if !script.characters.contains_key(&clip.actor)
            || !clip.gain.is_finite()
            || end > expected_samples
            || clip
                .trim_start_frame
                .checked_add(clip.duration_frames)
                .is_none()
        {
            return Err(invalid("unknown actor, gain, clip bounds or trim overflow"));
        }
        if !clip.visemes.is_empty() && clip.alignment_source.is_none() {
            return Err(invalid("visemes require alignment provenance"));
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
                return Err(invalid(
                    "alignment provenance requires bounded tool and version",
                ));
            }
        }
        interval_count = interval_count
            .checked_add(clip.visemes.len())
            .ok_or_else(|| invalid("viseme count overflow"))?;
        if interval_count > 100_000 {
            return Err(invalid("too many viseme intervals"));
        }
        let mut previous = clip.start_frame;
        for v in &clip.visemes {
            if v.start_frame < previous || v.start_frame >= v.end_frame || v.end_frame > end {
                return Err(invalid(
                    "visemes must be ordered half-open intervals inside their clip",
                ));
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
    if count.saturating_mul((interval_count + m.dialogue.len()) as u64) > 1_000_000 {
        return Err(invalid("dialogue/viseme frame work budget exceeded"));
    }
    for track in tracks.values_mut() {
        track.sort_unstable();
        if track.windows(2).any(|p| p[1].0 < p[0].1) {
            return Err(invalid("overlapping dialogue for the same actor"));
        }
    }
    let first = motion_scene::evaluate(script, index, 0.0).map_err(|e| invalid(e.to_string()))?;
    motion_render::render_svg(&first, first.width, first.height)
        .map_err(|e| invalid(e.to_string()))?;
    Ok(count)
}

#[derive(Serialize)]
struct Frame {
    frame: u64,
    sample_frame: u64,
    time_seconds: f64,
    file: String,
}

/// Creates a new directory only; removes only its own partial directory on error.
/// The caller must run this blocking operation off the UI thread.
pub fn export_scene(
    script: &Script,
    scene_index: usize,
    rendered: &RenderedScene,
    destination: &Path,
    cancel: &CancellationToken,
    mut progress: impl FnMut(Progress),
) -> Result<Report, Error> {
    export_sequence(
        script,
        scene_index,
        rendered,
        destination,
        cancel,
        &mut progress,
        false,
        None,
    )
}

/// Rasterize the same evaluated SVG poses to PNG without external processes.
pub fn export_png_scene(
    script: &Script,
    scene_index: usize,
    rendered: &RenderedScene,
    destination: &Path,
    cancel: &CancellationToken,
    mut progress: impl FnMut(Progress),
) -> Result<Report, Error> {
    export_sequence(
        script,
        scene_index,
        rendered,
        destination,
        cancel,
        &mut progress,
        true,
        None,
    )
}

/// Export foreground artwork using the shared frame scheduler and sample clock.
pub fn export_scene_with_art(
    script: &Script,
    scene_index: usize,
    rendered: &RenderedScene,
    art: &ArtContext,
    destination: &Path,
    cancel: &CancellationToken,
    mut progress: impl FnMut(Progress),
) -> Result<Report, Error> {
    export_sequence(
        script,
        scene_index,
        rendered,
        destination,
        cancel,
        &mut progress,
        false,
        Some(art),
    )
}
pub fn export_png_scene_with_art(
    script: &Script,
    scene_index: usize,
    rendered: &RenderedScene,
    art: &ArtContext,
    destination: &Path,
    cancel: &CancellationToken,
    mut progress: impl FnMut(Progress),
) -> Result<Report, Error> {
    export_sequence(
        script,
        scene_index,
        rendered,
        destination,
        cancel,
        &mut progress,
        true,
        Some(art),
    )
}

// Conservative raster working-set estimate: pixmap, encoder buffers and retained
// result. Large frames automatically reduce concurrency; no frame-sized thread fanout.
fn frame_jobs(script: &Script, png: bool) -> motion_jobs::Config {
    let bytes = if png {
        u64::from(script.project.width) * u64::from(script.project.height) * 16 + 1024 * 1024
    } else {
        4 * 1024 * 1024
    };
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(4)
        .min((320 * 1024 * 1024 / bytes).max(1) as usize);
    motion_jobs::Config {
        workers,
        batch_size: workers,
    }
}

// A single validated animation snapshot is shared by the bounded frame workers.
// Callers perform source/work-budget preflight before cloning the immutable script.
struct PreparedFrames<'a> {
    script: &'a Script,
    validated: motion_scene::ValidatedScript,
}
impl<'a> PreparedFrames<'a> {
    fn new(script: &'a Script) -> Result<Self, Error> {
        let validated = motion_scene::ValidatedScript::new(std::sync::Arc::new(script.clone()))
            .map_err(|e| invalid(e.to_string()))?;
        Ok(Self { script, validated })
    }
    fn encode_frame(
        &self,
        scene_index: usize,
        rendered: &RenderedScene,
        i: u64,
        png: bool,
        cancel: &CancellationToken,
        art: Option<&ArtContext>,
    ) -> Result<(u64, f64, Vec<u8>), Error> {
        let script = self.script;
        check(cancel)?;
        let sample = i
            .checked_mul(SAMPLE_RATE as u64)
            .ok_or_else(|| invalid("frame clock overflow"))?
            / script.project.fps as u64;
        let time = sample as f64 / SAMPLE_RATE as f64;
        let mut pose = self
            .validated
            .evaluate(scene_index, time)
            .map_err(|e| invalid(e.to_string()))?;
        let mouths = mouths_at(&rendered.manifest, sample);
        motion_scene::apply_speech(&mut pose, &mouths);
        let svg = match art {
            Some(art) => motion_render::render_svg_with_art(
                &pose,
                pose.width,
                pose.height,
                &mouths,
                &script.scenes[scene_index].id,
                &art.document,
                &art.images,
            ),
            None => motion_render::render_svg_with_mouths(&pose, pose.width, pose.height, &mouths),
        }
        .map_err(|e| invalid(e.to_string()))?;
        check(cancel)?;
        let encoded = if png {
            media::rasterize(&svg)?
        } else {
            svg.into_bytes()
        };
        check(cancel)?;
        Ok((sample, time, encoded))
    }
}

// Preserve the compact fixture helper used across export test modules.
#[cfg(test)]
fn encode_frame(
    script: &Script,
    scene_index: usize,
    rendered: &RenderedScene,
    i: u64,
    png: bool,
    cancel: &CancellationToken,
    art: Option<&ArtContext>,
) -> Result<(u64, f64, Vec<u8>), Error> {
    check(cancel)?;
    bounded_json(script, MAX_SOURCE)?;
    PreparedFrames::new(script)?.encode_frame(scene_index, rendered, i, png, cancel, art)
}

#[allow(clippy::too_many_arguments)]
fn export_sequence(
    script: &Script,
    scene_index: usize,
    rendered: &RenderedScene,
    destination: &Path,
    cancel: &CancellationToken,
    progress: &mut impl FnMut(Progress),
    png: bool,
    art: Option<&ArtContext>,
) -> Result<Report, Error> {
    check(cancel)?;
    let total = validate(script, scene_index, rendered)?;
    let prepared = PreparedFrames::new(script)?;
    if let Some(art) = art {
        art.document
            .validate()
            .map_err(|e| invalid(e.to_string()))?;
        for layer in art
            .document
            .layers
            .iter()
            .filter(|layer| layer.scene_id == script.scenes[scene_index].id)
        {
            if layer
                .active
                .is_some_and(|range| range.end > script.scenes[scene_index].duration)
            {
                return Err(invalid(
                    "artwork visibility interval exceeds scene duration",
                ));
            }
            if let Some(shape) = &layer.clip_shape {
                shape
                    .validate_scene_duration(script.scenes[scene_index].duration)
                    .map_err(|error| invalid(error.to_string()))?;
            }
            // Preflight future images too: frame zero may precede their start time.
            if layer.visible {
                for asset_id in layer.image_assets() {
                    if !art.images.contains_key(asset_id) {
                        return Err(invalid(format!(
                            "missing artwork image resource: {asset_id}"
                        )));
                    }
                }
            }
        }
        let group_bytes = group_work(art, script, total, false)?;
        let geometry = art
            .document
            .layers
            .iter()
            .filter(|l| l.scene_id == script.scenes[scene_index].id)
            .map(|l| {
                let content = match &l.content {
                    motion_art::Content::Stroke { points, .. } => points.len() as u64,
                    _ => 1,
                };
                content
                    + match &l.clip_shape {
                        Some(motion_art::ClipShape::Polygon { points, .. }) => points.len() as u64,
                        Some(motion_art::ClipShape::Painted { strokes, .. }) => {
                            strokes
                                .iter()
                                .map(|stroke| stroke.render_cost() as u64)
                                .sum::<u64>()
                                + 1
                        }
                        Some(motion_art::ClipShape::SoftEllipse { keys, .. }) => {
                            u64::from(usize::BITS - keys.len().max(1).leading_zeros()) + 8
                        }
                        _ => 0,
                    }
            })
            .sum::<u64>();
        if geometry.saturating_mul(total) > 4_000_000 {
            return Err(invalid(
                "artwork exceeds four million layer/point frame visits",
            ));
        }
        let image_bytes = art.images.values().try_fold(0usize, |n, i| {
            n.checked_add(i.png().len())
                .ok_or_else(|| invalid("image byte overflow"))
        })?;
        let pixels = art.images.values().try_fold(0u64, |n, i| {
            n.checked_add(u64::from(i.width()) * u64::from(i.height()))
                .ok_or_else(|| invalid("image pixel overflow"))
        })?;
        if image_bytes > 32 * 1024 * 1024 || pixels > 16_000_000 {
            return Err(invalid(
                "artwork exceeds 32 MiB encoded / 16 million decoded pixel budget",
            ));
        }
        if group_bytes > 0 {
            let raster_bytes = if png {
                u64::from(script.project.width) * u64::from(script.project.height) * 24
            } else {
                0
            };
            let working = group_bytes
                .saturating_add(raster_bytes)
                .saturating_add(image_bytes as u64 * 4)
                .saturating_add(pixels.saturating_mul(8))
                .saturating_add(rendered.audio.samples.len() as u64 * 4)
                .saturating_add(16 * 1024 * 1024);
            if working > MAX_BYTES {
                return Err(invalid("grouped scene working set exceeds 256 MiB"));
            }
        }
        let pose = prepared
            .validated
            .evaluate(scene_index, 0.0)
            .map_err(|e| invalid(e.to_string()))?;
        motion_render::render_svg_with_art(
            &pose,
            pose.width,
            pose.height,
            &[],
            &script.scenes[scene_index].id,
            &art.document,
            &art.images,
        )
        .map_err(|e| invalid(e.to_string()))?;
    }
    check(cancel)?;
    if png && u64::from(script.project.width) * u64::from(script.project.height) > 16_000_000 {
        return Err(invalid("raster frame exceeds 16 million pixel budget"));
    }
    if png
        && u64::from(script.project.width)
            .saturating_mul(u64::from(script.project.height))
            .saturating_mul(total)
            > 1_000_000_000
    {
        return Err(invalid(
            "raster sequence exceeds one billion pixel work budget",
        ));
    }
    let scene_json = bounded_json(&rendered.manifest, 8 * 1024 * 1024)?;
    let source = bounded_json(script, MAX_SOURCE)?;
    let mut bytes = (rendered.audio.frames() as u64)
        .checked_mul(2)
        .and_then(|v| v.checked_add(44 + scene_json.len() as u64))
        .ok_or_else(|| invalid("byte budget overflow"))?;
    if bytes > MAX_BYTES {
        return Err(invalid("output exceeds 256 MiB byte budget"));
    }
    fs::create_dir(destination)?;
    let result = (|| {
        progress(Progress {
            completed_frames: 0,
            total_frames: total,
        });
        let mut frames = Vec::with_capacity(total as usize);
        let mut config = frame_jobs(script, png);
        if art.is_some_and(|art| !art.document.groups.is_empty()) {
            config.workers = 1;
            config.batch_size = 1;
        }
        motion_jobs::ordered_map(
            total as usize,
            config,
            || cancel.is_cancelled(),
            |i| prepared.encode_frame(scene_index, rendered, i as u64, png, cancel, art),
            |i, (sample, time, encoded)| {
                check(cancel)?;
                let i = i as u64;
                bytes = bytes
                    .checked_add(encoded.len() as u64)
                    .ok_or_else(|| invalid("byte budget overflow"))?;
                if bytes > MAX_BYTES {
                    return Err(invalid("output exceeds 256 MiB byte budget"));
                }
                let extension = if png { "png" } else { "svg" };
                let file = format!("frame_{i:06}.{extension}");
                write_new(&destination.join(&file), &encoded)?;
                frames.push(Frame {
                    frame: i,
                    sample_frame: sample,
                    time_seconds: time,
                    file,
                });
                progress(Progress {
                    completed_frames: i + 1,
                    total_frames: total,
                });
                Ok(())
            },
        )
        .map_err(|error| match error {
            motion_jobs::JobError::Cancelled => Error::Cancelled,
            motion_jobs::JobError::Task(error) => error,
            motion_jobs::JobError::Panic => invalid("frame worker panicked"),
            motion_jobs::JobError::InvalidConfig => invalid("invalid frame worker budget"),
        })?;
        check(cancel)?;
        let manifest = bounded_json(
            &serde_json::json!({
                "schema_version":1,"format":if png {"png_audio_sequence"} else {"svg_audio_sequence"},"scene_id":rendered.manifest.scene_id,
                "fps":script.project.fps,"sample_rate":SAMPLE_RATE,"frame_count":total,
                "width":script.project.width,"height":script.project.height,"frames":frames,
                "audio_file":"scene.wav","audio_manifest_file":"scene.json",
                "source_project":serde_json::from_slice::<serde_json::Value>(&source)?,
                "artwork":art.map(|a| &a.document),
                "artwork_composition":"foreground_document_order_translation_only_attachments",
                "clock":"floor(frame * 24000 / fps)","video_encoded":false
            }),
            8 * 1024 * 1024,
        )?;
        bytes = bytes
            .checked_add(manifest.len() as u64)
            .ok_or_else(|| invalid("manifest byte overflow"))?;
        if bytes > MAX_BYTES {
            return Err(invalid("output exceeds 256 MiB byte budget"));
        }
        write_new(&destination.join("scene.json"), &scene_json)?;
        // Directory is private to this invocation; reserve the WAV filename first.
        write_new(&destination.join("scene.wav"), &[])?;
        motion_audio::write_wav(destination.join("scene.wav"), &rendered.audio)
            .map_err(|e| invalid(e.to_string()))?;
        check(cancel)?;
        write_new(&destination.join("manifest.json"), &manifest)?;
        Ok(Report {
            frame_count: total,
            output_directory: destination.to_path_buf(),
        })
    })();
    if result.is_err() {
        fs::remove_dir_all(destination)
            .map_err(|e| invalid(format!("export failed; partial-output cleanup failed: {e}")))?;
    }
    result
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    Ok(())
}

/// Serialize with a hard cap during writing, rather than allocating an arbitrary
/// source/manifest and checking its size afterwards.
fn bounded_json(value: &impl Serialize, limit: usize) -> Result<Vec<u8>, Error> {
    struct Limited {
        bytes: Vec<u8>,
        limit: usize,
    }
    impl Write for Limited {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
                return Err(std::io::Error::other(
                    "JSON input/output byte budget exceeded",
                ));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Limited {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut writer, value)?;
    Ok(writer.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use motion_audio::AudioBuffer;
    use motion_voice::export::{DialogueManifest, SampleViseme};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    pub(super) fn destination() -> PathBuf {
        std::env::temp_dir().join(format!(
            "kilsat-export-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
    pub(super) fn fixture() -> (Script, RenderedScene) {
        let script = motion_script::parse("project: {fps: 4, width: 640, height: 360}\ncharacters:\n  EMMA: {asset: characters/emma}\n  ALEX: {asset: characters/alex}\nscenes:\n  - id: intro\n    duration: 1.0\n    events: []\n").unwrap();
        let clip = DialogueManifest {
            actor: "EMMA".into(),
            start_frame: 6000,
            duration_frames: 12000,
            trim_start_frame: 2400,
            gain: 1.0,
            alignment_source: Some(AlignmentSource::Manual),
            visemes: vec![
                SampleViseme {
                    start_frame: 6000,
                    end_frame: 12000,
                    viseme: Viseme::A,
                },
                SampleViseme {
                    start_frame: 12000,
                    end_frame: 18000,
                    viseme: Viseme::O,
                },
            ],
        };
        let rendered = RenderedScene {
            audio: AudioBuffer {
                sample_rate: 24000,
                channels: 1,
                samples: vec![0.0; 24000],
            },
            manifest: SceneManifest {
                schema_version: 1,
                scene_id: "intro".into(),
                sample_rate: 24000,
                channels: 1,
                total_frames: 24000,
                dialogue: vec![clip],
            },
        };
        (script, rendered)
    }
    #[test]
    fn parallel_sequences_match_serial_frame_bytes_and_progress() {
        let (mut script, rendered) = fixture();
        script.project.fps = 13;
        for png in [false, true] {
            let dest = destination();
            let token = CancellationToken::new();
            let mut completed = Vec::new();
            export_sequence(
                &script,
                0,
                &rendered,
                &dest,
                &token,
                &mut |p| completed.push(p.completed_frames),
                png,
                None,
            )
            .unwrap();
            assert_eq!(completed, (0..=13).collect::<Vec<_>>());
            for i in 0..13 {
                let (_, _, expected) =
                    encode_frame(&script, 0, &rendered, i, png, &token, None).unwrap();
                let extension = if png { "png" } else { "svg" };
                assert_eq!(
                    fs::read(dest.join(format!("frame_{i:06}.{extension}"))).unwrap(),
                    expected
                );
            }
            fs::remove_dir_all(dest).unwrap();
        }
    }

    #[test]
    fn raster_working_set_reduces_concurrency_for_large_frames() {
        let (mut script, _) = fixture();
        script.project.width = 4000;
        script.project.height = 4000;
        let config = frame_jobs(&script, true);
        assert_eq!(config.workers, 1);
        assert_eq!(config.batch_size, 1);
    }

    #[test]
    fn timed_background_keys_reach_svg_and_decoded_png_on_the_sample_clock() {
        let (mut script, rendered) = fixture();
        script.scenes[0].composition.background_color = "#0000ff".into();
        script.scenes[0].composition.background_keys = vec![
            motion_script::BackgroundKey {
                at: 0.25,
                color: "#ff0000".into(),
            },
            motion_script::BackgroundKey {
                at: 0.5,
                color: "#00ff00".into(),
            },
        ];
        let colors = ["#0000ff", "#ff0000", "#00ff00", "#00ff00"];
        let pixels = [
            [0, 0, 255, 255],
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 255, 0, 255],
        ];
        let svg_dir = destination();
        let png_dir = destination();
        export_scene(
            &script,
            0,
            &rendered,
            &svg_dir,
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap();
        export_png_scene(
            &script,
            0,
            &rendered,
            &png_dir,
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap();
        for index in 0..4 {
            let svg = fs::read_to_string(svg_dir.join(format!("frame_{index:06}.svg"))).unwrap();
            assert!(svg.contains(&format!("fill=\"{}\"", colors[index])));
            let image =
                resvg::tiny_skia::Pixmap::load_png(png_dir.join(format!("frame_{index:06}.png")))
                    .unwrap();
            assert_eq!(&image.data()[..4], &pixels[index]);
        }
        fs::remove_dir_all(svg_dir).unwrap();
        fs::remove_dir_all(png_dir).unwrap();
    }

    #[test]
    fn png_decodes_dimensions_and_cancellation_cleans_output() {
        let (mut s, r) = fixture();
        s.project.width = 1920;
        s.project.height = 1080;
        let dest = destination();
        export_png_scene(&s, 0, &r, &dest, &CancellationToken::new(), |_| {}).unwrap();
        let image = resvg::tiny_skia::Pixmap::load_png(dest.join("frame_000001.png")).unwrap();
        assert_eq!((image.width(), image.height()), (1920, 1080));
        assert!(image
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] > 0));
        let m: serde_json::Value =
            serde_json::from_slice(&fs::read(dest.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(m["frames"][1]["sample_frame"], 6000);
        assert_eq!(m["format"], "png_audio_sequence");
        fs::remove_dir_all(&dest).unwrap();
        let token = CancellationToken::new();
        assert!(matches!(
            export_png_scene(&s, 0, &r, &dest, &token, |p| {
                if p.completed_frames == 1 {
                    token.cancel();
                }
            }),
            Err(Error::Cancelled)
        ));
        assert!(!dest.exists());
    }

    #[test]
    fn raster_budget_and_encoder_path_fail_without_output() {
        let (mut s, r) = fixture();
        let dest = destination();
        assert!(export_mp4_scene(
            &s,
            0,
            &r,
            &dest,
            Path::new("ffmpeg"),
            &CancellationToken::new(),
            |_| {}
        )
        .is_err());
        s.project.width = 8192;
        s.project.height = 8192;
        assert!(export_png_scene(&s, 0, &r, &dest, &CancellationToken::new(), |_| {}).is_err());
        assert!(!dest.exists());
    }

    #[cfg(unix)]
    #[test]
    fn failed_encoder_removes_owned_frames_but_preserves_existing_output() {
        use std::os::unix::fs::PermissionsExt;
        let (s, r) = fixture();
        let encoder = destination();
        fs::write(&encoder, b"#!/bin/sh\nexit 7\n").unwrap();
        fs::set_permissions(&encoder, fs::Permissions::from_mode(0o700)).unwrap();
        let dest = destination();
        assert!(export_mp4_scene(
            &s,
            0,
            &r,
            &dest,
            &encoder,
            &CancellationToken::new(),
            |_| {}
        )
        .is_err());
        assert!(!dest.exists());
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("keep"), b"original").unwrap();
        assert!(export_mp4_scene(
            &s,
            0,
            &r,
            &dest,
            &encoder,
            &CancellationToken::new(),
            |_| {}
        )
        .is_err());
        assert_eq!(fs::read(dest.join("keep")).unwrap(), b"original");
        fs::remove_dir_all(dest).unwrap();
        fs::remove_file(encoder).unwrap();
    }

    #[test]
    #[ignore = "requires explicit KILSAT_TEST_FFMPEG and optional KILSAT_TEST_FFPROBE local executables"]
    fn real_ffmpeg_encodes_h264_aac() {
        let encoder =
            PathBuf::from(std::env::var_os("KILSAT_TEST_FFMPEG").expect("set KILSAT_TEST_FFMPEG"));
        for seconds in [1.0, 0.9] {
            let (mut s, mut r) = fixture();
            s.scenes[0].duration = seconds;
            r.manifest.total_frames = (seconds * SAMPLE_RATE as f64).round() as u64;
            r.audio.samples.truncate(r.manifest.total_frames as usize);
            let dest = destination();
            let report = export_mp4_scene(
                &s,
                0,
                &r,
                &dest,
                &encoder,
                &CancellationToken::new(),
                |_| {},
            )
            .unwrap();
            assert!(fs::metadata(dest.join("scene.mp4")).unwrap().len() > 0);
            if let Some(probe) = std::env::var_os("KILSAT_TEST_FFPROBE") {
                let output = std::process::Command::new(probe)
                    .args([
                        "-v",
                        "error",
                        "-show_entries",
                        "stream=codec_name,width,height,duration,nb_frames",
                        "-of",
                        "json",
                    ])
                    .arg(dest.join("scene.mp4"))
                    .output()
                    .unwrap();
                assert!(output.status.success());
                let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                let streams = value["streams"].as_array().unwrap();
                let video = streams.iter().find(|v| v["codec_name"] == "h264").unwrap();
                assert_eq!(video["width"], 640);
                assert_eq!(video["height"], 360);
                assert_eq!(
                    video["nb_frames"].as_str().unwrap().parse::<u64>().unwrap(),
                    report.frame_count
                );
                let video_seconds = video["duration"].as_str().unwrap().parse::<f64>().unwrap();
                assert!(
                    (video_seconds - report.frame_count as f64 / s.project.fps as f64).abs()
                        < 0.001
                );
                let audio = streams.iter().find(|v| v["codec_name"] == "aac").unwrap();
                let audio_seconds = audio["duration"].as_str().unwrap().parse::<f64>().unwrap();
                assert!((audio_seconds - seconds).abs() < 0.05);
            }
            fs::remove_dir_all(dest).unwrap();
        }
    }

    #[test]
    fn sample_clock_visemes_silence_and_trim_are_scene_relative() {
        let (s, r) = fixture();
        assert!(mouths_at(&r.manifest, 5999).is_empty());
        assert_eq!(mouths_at(&r.manifest, 6000), vec![("EMMA", Viseme::A)]);
        assert_eq!(mouths_at(&r.manifest, 11999), vec![("EMMA", Viseme::A)]);
        assert_eq!(mouths_at(&r.manifest, 12000), vec![("EMMA", Viseme::O)]);
        assert!(mouths_at(&r.manifest, 18000).is_empty());
        let dest = destination();
        let mut progress = vec![];
        let report = export_scene(&s, 0, &r, &dest, &CancellationToken::new(), |p| {
            progress.push(p)
        })
        .unwrap();
        assert_eq!(report.frame_count, 4);
        assert_eq!(progress.last().unwrap().completed_frames, 4);
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(dest.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["frames"][1]["sample_frame"], 6000);
        assert_eq!(manifest["frames"][1]["time_seconds"], 0.25);
        let open = fs::read_to_string(dest.join("frame_000001.svg")).unwrap();
        assert!(open.contains("width=\"14\" height=\"14\""));
        assert_eq!(
            motion_audio::read_wav(dest.join("scene.wav"))
                .unwrap()
                .frames(),
            24000
        );
        assert!(dest.join("scene.json").is_file());
        fs::remove_dir_all(dest).unwrap();
    }
    #[test]
    fn cancellation_before_and_during_export_cleans_only_owned_output() {
        let (s, r) = fixture();
        let dest = destination();
        let token = CancellationToken::new();
        token.cancel();
        assert!(matches!(
            export_scene(&s, 0, &r, &dest, &token, |_| {}),
            Err(Error::Cancelled)
        ));
        assert!(!dest.exists());
        let token = CancellationToken::new();
        assert!(matches!(
            export_scene(&s, 0, &r, &dest, &token, |p| if p.completed_frames == 1 {
                token.cancel();
            }),
            Err(Error::Cancelled)
        ));
        assert!(!dest.exists());
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("keep"), b"original").unwrap();
        assert!(export_scene(&s, 0, &r, &dest, &CancellationToken::new(), |_| {}).is_err());
        assert_eq!(fs::read(dest.join("keep")).unwrap(), b"original");
        fs::remove_dir_all(dest).unwrap();
    }
    #[test]
    fn malformed_identity_duration_and_intervals_fail_before_output() {
        for case in 0..10 {
            let (s, mut r) = fixture();
            match case {
                0 => r.manifest.scene_id = "wrong".into(),
                1 => r.manifest.schema_version = 2,
                2 => {
                    r.audio.samples.pop();
                }
                3 => r.manifest.dialogue[0].actor = "UNKNOWN".into(),
                4 => r.manifest.dialogue[0].gain = f32::NAN,
                5 => r.manifest.dialogue[0].visemes[0].start_frame = 5999,
                6 => r.manifest.dialogue[0].visemes[1].end_frame = 18001,
                7 => r.manifest.dialogue[0].alignment_source = None,
                8 => r.manifest.dialogue[0].visemes[1].start_frame = 11999,
                _ => r.manifest.dialogue[0].trim_start_frame = u64::MAX,
            }
            let dest = destination();
            assert!(
                export_scene(&s, 0, &r, &dest, &CancellationToken::new(), |_| {}).is_err(),
                "case {case}"
            );
            assert!(!dest.exists());
        }
    }
    #[test]
    fn inactive_live_layers_cannot_bypass_export_work_preflight() {
        let (mut script, mut rendered) = fixture();
        script.project.fps = 120;
        script.scenes[0].duration = 10.0;
        rendered.audio.samples.resize(240_000, 0.0);
        rendered.manifest.total_frames = 240_000;
        let clip = motion_script::AnimationClip {
            duration: 1.0,
            bone_tracks: [
                "hip",
                "chest",
                "head",
                "left_hand",
                "right_hand",
                "left_foot",
                "right_foot",
            ]
            .iter()
            .map(|bone| (bone.to_string(), vec![motion_script::ActorKey::default()]))
            .collect(),
        };
        script.animation_clips.insert("rest".into(), clip);
        script.scenes[0].composition.clip_instances = (0..32)
            .map(|i| motion_script::ClipInstance {
                id: format!("layer{i}"),
                clip: "rest".into(),
                actor: "EMMA".into(),
                at: 9.0,
                duration: 1.0,
                weight: 0.0,
                fade_in: 0.0,
                fade_out: 0.0,
                weight_keys: Vec::new(),
                looping: false,
                joint_mask: Vec::new(),
                blend_mode: motion_script::ClipBlendMode::Override,
                normalized_group: None,
            })
            .collect();
        script.validate().unwrap();
        assert!(bounded_json(&script, MAX_SOURCE).is_ok());
        let dest = destination();
        let result = export_scene(
            &script,
            0,
            &rendered,
            &dest,
            &CancellationToken::new(),
            |_| {},
        );
        assert!(
            matches!(result, Err(Error::Invalid(message)) if message.contains("keyframe/live-clip"))
        );
        assert!(!dest.exists());
        // The same otherwise-valid scene remains within budget without live layers.
        script.scenes[0].composition.clip_instances.clear();
        assert_eq!(validate(&script, 0, &rendered).unwrap(), 1200);
    }

    fn dormant_clip_fixture(count: usize) -> (Script, RenderedScene) {
        let (mut script, mut rendered) = fixture();
        script.project.fps = 240;
        script.scenes[0].duration = 37.5;
        rendered.audio.samples.resize(900_000, 0.0);
        rendered.manifest.total_frames = 900_000;
        script.animation_clips.insert(
            "rest".into(),
            motion_script::AnimationClip {
                duration: 1.0,
                bone_tracks: BTreeMap::from([(
                    "head".into(),
                    vec![motion_script::ActorKey::default()],
                )]),
            },
        );
        script.scenes[0].composition.clip_instances = vec![motion_script::ClipInstance {
            id: "later".into(),
            clip: "rest".into(),
            actor: "EMMA".into(),
            at: 36.5,
            duration: 1.0,
            weight: 0.0,
            fade_in: 0.0,
            fade_out: 0.0,
            weight_keys: Vec::new(),
            looping: false,
            joint_mask: Vec::new(),
            blend_mode: motion_script::ClipBlendMode::Override,
            normalized_group: None,
        }];
        let prototype = script.scenes[0].composition.clip_instances[0].clone();
        for index in 1..count {
            let mut row = prototype.clone();
            row.id = format!("later{index}");
            script.scenes[0].composition.clip_instances.push(row);
        }
        (script, rendered)
    }

    #[test]
    fn dormant_zero_weight_envelopes_cannot_bypass_export_work_budget() {
        let (mut script, rendered) = dormant_clip_fixture(4);
        // The exact same scene fits without envelope search/fade work.
        assert_eq!(validate(&script, 0, &rendered).unwrap(), 9000);
        for row in &mut script.scenes[0].composition.clip_instances {
            row.fade_in = 0.1;
            row.fade_out = 0.1;
            row.weight_keys = (0..256)
                .map(|index| motion_script::InstanceWeightKey {
                    time: index as f64 / 256.0,
                    weight: 1.0,
                    easing: motion_script::ActorEasing::Linear,
                })
                .collect();
        }
        script.validate().unwrap();
        let output = destination();
        let result = export_scene(
            &script,
            0,
            &rendered,
            &output,
            &CancellationToken::new(),
            |_| {},
        );
        assert!(
            matches!(result, Err(Error::Invalid(message)) if message.contains("keyframe/live-clip"))
        );
        assert!(!output.exists());
    }

    #[test]
    fn dormant_additive_layers_are_preflighted_before_output_creation() {
        let (mut script, rendered) = dormant_clip_fixture(2);
        assert_eq!(validate(&script, 0, &rendered).unwrap(), 9000);
        for row in &mut script.scenes[0].composition.clip_instances {
            row.blend_mode = motion_script::ClipBlendMode::Additive;
        }
        script.validate().unwrap();
        let output = destination();
        let result = export_scene(
            &script,
            0,
            &rendered,
            &output,
            &CancellationToken::new(),
            |_| {},
        );
        assert!(
            matches!(result, Err(Error::Invalid(message)) if message.contains("keyframe/live-clip"))
        );
        assert!(!output.exists());
    }

    #[test]
    fn overlap_rejected_for_same_actor_but_other_actor_allowed() {
        let (s, mut r) = fixture();
        let mut extra: DialogueManifest =
            serde_json::from_value(serde_json::to_value(&r.manifest.dialogue[0]).unwrap()).unwrap();
        extra.actor = "ALEX".into();
        r.manifest.dialogue.push(extra);
        assert_eq!(validate(&s, 0, &r).unwrap(), 4);
        r.manifest.dialogue[1].actor = "EMMA".into();
        assert!(validate(&s, 0, &r).is_err());
        r.manifest.dialogue.pop();
        r.manifest.dialogue[0].visemes.clear();
        assert_eq!(mouths_at(&r.manifest, 6000), vec![("EMMA", Viseme::Rest)]);
    }
    #[test]
    fn non_divisor_fps_uses_integer_sample_floor() {
        let (mut s, r) = fixture();
        s.project.fps = 29;
        let dest = destination();
        export_scene(&s, 0, &r, &dest, &CancellationToken::new(), |_| {}).unwrap();
        let m: serde_json::Value =
            serde_json::from_slice(&fs::read(dest.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(m["frames"][1]["sample_frame"], 827);
        assert_eq!(m["frames"][1]["time_seconds"], 827.0 / 24000.0);
        fs::remove_dir_all(dest).unwrap();
    }
    #[test]
    fn rounded_audio_eof_is_silent_for_final_visual_frame() {
        let (mut script, mut rendered) = fixture();
        script.project.fps = 24;
        script.scenes[0].duration = 0.04167;
        rendered.audio.samples.truncate(1000);
        rendered.manifest.total_frames = 1000;
        let clip = &mut rendered.manifest.dialogue[0];
        clip.start_frame = 0;
        clip.duration_frames = 1000;
        clip.trim_start_frame = 0;
        clip.visemes = vec![SampleViseme {
            start_frame: 0,
            end_frame: 1000,
            viseme: Viseme::A,
        }];
        let dest = destination();
        let report = export_scene(
            &script,
            0,
            &rendered,
            &dest,
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap();
        assert_eq!(report.frame_count, 2);
        assert!(fs::read_to_string(dest.join("frame_000000.svg"))
            .unwrap()
            .contains("width=\"14\" height=\"14\""));
        assert!(!fs::read_to_string(dest.join("frame_000001.svg"))
            .unwrap()
            .contains("width=\"14\" height=\"14\""));
        fs::remove_dir_all(dest).unwrap();
    }
    #[test]
    fn bounded_project_serialization_stops_before_output() {
        let (mut script, rendered) = fixture();
        script.project.title = "x".repeat(MAX_SOURCE + 1);
        let dest = destination();
        assert!(export_scene(
            &script,
            0,
            &rendered,
            &dest,
            &CancellationToken::new(),
            |_| {}
        )
        .is_err());
        assert!(!dest.exists());
    }
}

#[cfg(test)]
mod artwork_tests {
    use super::*;
    use motion_art::{Color, Content, Document, ImageResource, Layer, Point, Rect};
    fn layer(id: &str, content: Content) -> Layer {
        Layer {
            id: id.into(),
            scene_id: "intro".into(),
            name: id.into(),
            visible: true,
            opacity: 1.0,
            plane: motion_art::Plane::Foreground,
            active: None,
            attachment: None,
            clip_shape: None,
            blend: motion_art::BlendMode::Normal,
            clip: None,
            content,
        }
    }
    #[test]
    fn timed_background_images_and_foreground_obey_half_open_frame_intervals() {
        let (script, rendered) = super::tests::fixture();
        let mut background = layer(
            "background",
            Content::Image {
                asset_id: "red".into(),
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 640.0,
                    height: 360.0,
                },
            },
        );
        background.plane = motion_art::Plane::Background;
        background.active = Some(motion_art::TimeRange {
            start: 0.25,
            end: 0.75,
        });
        let mut foreground = layer(
            "foreground",
            Content::Rectangle {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 32.0,
                    height: 32.0,
                },
                color: Color {
                    r: 0,
                    g: 255,
                    b: 0,
                    a: 255,
                },
            },
        );
        foreground.active = Some(motion_art::TimeRange {
            start: 0.5,
            end: 1.0,
        });
        // Put foreground first to prove plane order takes precedence over document order.
        let art = ArtContext {
            document: Document {
                layers: vec![foreground, background],
                ..Default::default()
            },
            images: BTreeMap::from([(
                "red".into(),
                ImageResource::from_rgba(1, 1, vec![255, 0, 0, 255]).unwrap(),
            )]),
        };
        let destination = super::tests::destination();
        export_png_scene_with_art(
            &script,
            0,
            &rendered,
            &art,
            &destination,
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap();
        let base = [24, 31, 44, 255];
        let expected_background = [base, [255, 0, 0, 255], [255, 0, 0, 255], base];
        let expected_corner = [base, [255, 0, 0, 255], [0, 255, 0, 255], [0, 255, 0, 255]];
        for index in 0..4 {
            let image = resvg::tiny_skia::Pixmap::load_png(
                destination.join(format!("frame_{index:06}.png")),
            )
            .unwrap();
            assert_eq!(
                {
                    let p = image.pixel(500, 10).unwrap();
                    [p.red(), p.green(), p.blue(), p.alpha()]
                },
                expected_background[index]
            );
            assert_eq!(
                {
                    let p = image.pixel(10, 10).unwrap();
                    [p.red(), p.green(), p.blue(), p.alpha()]
                },
                expected_corner[index]
            );
            if index == 1 || index == 2 {
                // The procedural actor must remain visible above the full-canvas image.
                assert!(image
                    .data()
                    .as_chunks::<4>()
                    .0
                    .contains(&[245, 200, 162, 255]));
            }
        }
        fs::remove_dir_all(destination).unwrap();
    }

    #[test]
    fn future_background_resources_and_duration_fail_before_creating_output() {
        let (script, rendered) = super::tests::fixture();
        let mut image = layer(
            "future",
            Content::Image {
                asset_id: "missing".into(),
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
            },
        );
        image.plane = motion_art::Plane::Background;
        image.active = Some(motion_art::TimeRange {
            start: 0.5,
            end: 1.0,
        });
        let mut art = ArtContext {
            document: Document {
                layers: vec![image],
                ..Default::default()
            },
            images: BTreeMap::new(),
        };
        let destination = super::tests::destination();
        let error = export_scene_with_art(
            &script,
            0,
            &rendered,
            &art,
            &destination,
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap_err();
        assert!(error.to_string().contains("missing artwork image"));
        assert!(!destination.exists());
        art.images.insert(
            "missing".into(),
            ImageResource::from_rgba(1, 1, vec![255, 0, 0, 255]).unwrap(),
        );
        art.document.layers[0].active.as_mut().unwrap().end = 1.1;
        let error = export_png_scene_with_art(
            &script,
            0,
            &rendered,
            &art,
            &destination,
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap_err();
        assert!(error.to_string().contains("exceeds scene duration"));
        assert!(!destination.exists());
    }

    #[test]
    fn embedded_png_mask_and_stroke_survive_real_sequence_rasterization() {
        let (script, rendered) = super::tests::fixture();
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 32.0,
            height: 32.0,
        };
        let mut image = layer(
            "image",
            Content::Image {
                asset_id: "red".into(),
                rect,
            },
        );
        image.clip = Some(Rect {
            width: 16.0,
            ..rect
        });
        let stroke = layer(
            "stroke",
            Content::Stroke {
                points: vec![Point { x: 40.0, y: 4.0 }, Point { x: 55.0, y: 4.0 }],
                color: Color {
                    r: 0,
                    g: 255,
                    b: 0,
                    a: 255,
                },
                width: 4.0,
            },
        );
        let art = ArtContext {
            document: Document {
                layers: vec![image, stroke],
                ..Default::default()
            },
            images: BTreeMap::from([(
                "red".into(),
                ImageResource::from_rgba(1, 1, vec![255, 0, 0, 255]).unwrap(),
            )]),
        };
        let destination = super::tests::destination();
        export_scene_with_art(
            &script,
            0,
            &rendered,
            &art,
            &destination,
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap();
        let svg = fs::read_to_string(destination.join("frame_000000.svg")).unwrap();
        assert!(svg.contains("data:image/png;base64,"));
        assert!(svg.contains("clip-path=\"url(#art0)\""));
        assert!(svg.contains("<polyline"));
        fs::remove_dir_all(&destination).unwrap();
        export_png_scene_with_art(
            &script,
            0,
            &rendered,
            &art,
            &destination,
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap();
        let png = fs::read(destination.join("frame_000000.png")).unwrap();
        let pix = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
        assert_eq!(pix.pixel(4, 4).unwrap().red(), 255);
        assert_ne!(pix.pixel(24, 4).unwrap().red(), 255);
        assert_eq!(pix.pixel(45, 4).unwrap().green(), 255);
        fs::remove_dir_all(destination).unwrap();
    }
    #[test]
    #[ignore = "requires explicit local KILSAT_TEST_FFMPEG"]
    fn embedded_art_is_retained_in_real_mp4_pipeline() {
        let (script, rendered) = super::tests::fixture();
        let art = ArtContext {
            document: Document {
                layers: vec![layer(
                    "image",
                    Content::Image {
                        asset_id: "red".into(),
                        rect: Rect {
                            x: 0.0,
                            y: 0.0,
                            width: 32.0,
                            height: 32.0,
                        },
                    },
                )],
                ..Default::default()
            },
            images: BTreeMap::from([(
                "red".into(),
                ImageResource::from_rgba(1, 1, vec![255, 0, 0, 255]).unwrap(),
            )]),
        };
        let destination = super::tests::destination();
        let ffmpeg = PathBuf::from(std::env::var_os("KILSAT_TEST_FFMPEG").expect("set FFmpeg"));
        export_mp4_scene_with_art(
            &script,
            0,
            &rendered,
            &art,
            &destination,
            &ffmpeg,
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap();
        assert!(fs::metadata(destination.join("scene.mp4")).unwrap().len() > 0);
        let png = fs::read(destination.join("frame_000000.png")).unwrap();
        assert_eq!(
            resvg::tiny_skia::Pixmap::decode_png(&png)
                .unwrap()
                .pixel(4, 4)
                .unwrap()
                .red(),
            255
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(destination.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["video_encoded"], true);
        assert_eq!(manifest["artwork"]["layers"][0]["id"], "image");
        fs::remove_dir_all(destination).unwrap();
    }
    #[test]
    fn missing_resource_fails_before_creating_output() {
        let (script, rendered) = super::tests::fixture();
        let art = ArtContext {
            document: Document {
                layers: vec![layer(
                    "missing",
                    Content::Image {
                        asset_id: "absent".into(),
                        rect: Rect {
                            x: 0.0,
                            y: 0.0,
                            width: 1.0,
                            height: 1.0,
                        },
                    },
                )],
                ..Default::default()
            },
            images: BTreeMap::new(),
        };
        let destination = super::tests::destination();
        assert!(export_scene_with_art(
            &script,
            0,
            &rendered,
            &art,
            &destination,
            &CancellationToken::new(),
            |_| {}
        )
        .is_err());
        assert!(!destination.exists());
    }
}
