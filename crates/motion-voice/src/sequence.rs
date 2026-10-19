//! Linear sequence crossfades on the shared 24 kHz clock.
//! Finite PCM headroom is preserved, just as in `mix_timeline`; WAV writing clips.
use crate::export::{RenderedScene, SAMPLE_RATE};
use motion_audio::AudioBuffer;
use motion_lipsync::AlignmentSource;
use motion_scene::SequenceTimeline;
use motion_tts::{CancellationToken, SpeechError};
use std::collections::BTreeMap;

pub const MAX_SEQUENCE_SAMPLES: u64 = SAMPLE_RATE as u64 * 600;
const MAX_SOURCE_SAMPLES: u64 = MAX_SEQUENCE_SAMPLES * 2;
const CHUNK: usize = 4096;

fn invalid(message: &str) -> SpeechError {
    SpeechError(format!("sequence audio: {message}"))
}

/// Compose exactly one rendered scene per timeline span, in screenplay order.
/// Validation and work limits precede allocation. Cancellation returns no audio.
/// Actor membership is checked by the screenplay-aware export validator.
pub fn mix_sequence(
    timeline: &SequenceTimeline,
    scenes: &[RenderedScene],
    cancel: &CancellationToken,
) -> Result<AudioBuffer, SpeechError> {
    cancel.check()?;
    if timeline.total_samples() > MAX_SEQUENCE_SAMPLES
        || timeline.spans().len() != scenes.len()
        || scenes.len() > 10_000
    {
        return Err(invalid(
            "duration, scene count or ordered scene set exceeds limits",
        ));
    }
    let mut source_samples = 0u64;
    let mut dialogue_count = 0usize;
    let mut interval_count = 0usize;
    for (span, scene) in timeline.spans().iter().zip(scenes) {
        cancel.check()?;
        source_samples = source_samples
            .checked_add(span.duration_samples)
            .ok_or_else(|| invalid("source length overflow"))?;
        if source_samples > MAX_SOURCE_SAMPLES {
            return Err(invalid("source sample work budget exceeded"));
        }
        let m = &scene.manifest;
        if m.schema_version != 1
            || m.scene_id != span.scene_id
            || m.sample_rate != SAMPLE_RATE
            || m.channels != 1
            || m.total_frames != span.duration_samples
            || scene.audio.sample_rate != SAMPLE_RATE
            || scene.audio.channels != 1
            || scene.audio.samples.len() as u64 != span.duration_samples
        {
            return Err(invalid(
                "ordered scene identity, schema, rate or length mismatch",
            ));
        }
        for chunk in scene.audio.samples.chunks(CHUNK) {
            cancel.check()?;
            if chunk.iter().any(|s| !s.is_finite()) {
                return Err(invalid("non-finite source PCM"));
            }
        }
        dialogue_count = dialogue_count.saturating_add(m.dialogue.len());
        if dialogue_count > 10_000 {
            return Err(invalid("dialogue work budget exceeded"));
        }
        let mut tracks: BTreeMap<&str, Vec<(u64, u64)>> = BTreeMap::new();
        for clip in &m.dialogue {
            cancel.check()?;
            let end = clip
                .start_frame
                .checked_add(clip.duration_frames)
                .ok_or_else(|| invalid("dialogue end overflow"))?;
            if clip.actor.trim().is_empty()
                || !clip.gain.is_finite()
                || end > span.duration_samples
                || clip
                    .trim_start_frame
                    .checked_add(clip.duration_frames)
                    .is_none()
                || (!clip.visemes.is_empty() && clip.alignment_source.is_none())
            {
                return Err(invalid("invalid dialogue bounds, gain or provenance"));
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
                    return Err(invalid("invalid alignment provenance"));
                }
            }
            interval_count = interval_count.saturating_add(clip.visemes.len());
            if interval_count > 100_000 {
                return Err(invalid("viseme work budget exceeded"));
            }
            let mut previous = clip.start_frame;
            for v in &clip.visemes {
                if v.start_frame < previous || v.start_frame >= v.end_frame || v.end_frame > end {
                    return Err(invalid("invalid viseme interval"));
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
            if track.windows(2).any(|p| p[1].0 < p[0].1) {
                return Err(invalid("overlapping dialogue for the same actor"));
            }
        }
    }
    cancel.check()?;
    let length = usize::try_from(timeline.total_samples())
        .map_err(|_| invalid("platform length overflow"))?;
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(length)
        .map_err(|_| invalid("allocation failed"))?;
    samples.resize(length, 0f32);
    for (index, (span, scene)) in timeline.spans().iter().zip(scenes).enumerate() {
        let incoming = index
            .checked_sub(1)
            .map(|i| timeline.spans()[i].outgoing_overlap)
            .unwrap_or(0);
        for (chunk_index, chunk) in scene.audio.samples.chunks(CHUNK).enumerate() {
            cancel.check()?;
            for (offset, sample) in chunk.iter().enumerate() {
                let local = (chunk_index * CHUNK + offset) as u64;
                let mut weight = 1.0;
                if incoming > 0 && local < incoming {
                    weight = local as f64 / incoming as f64;
                }
                let outgoing_start = span.duration_samples - span.outgoing_overlap;
                if span.outgoing_overlap > 0 && local >= outgoing_start {
                    weight = 1.0 - (local - outgoing_start) as f64 / span.outgoing_overlap as f64;
                }
                let target = (span.start_sample + local) as usize;
                let mixed = samples[target] as f64 + *sample as f64 * weight;
                if !mixed.is_finite() || mixed.abs() > f32::MAX as f64 {
                    return Err(invalid("mix overflow"));
                }
                samples[target] = mixed as f32;
            }
        }
    }
    cancel.check()?;
    Ok(AudioBuffer {
        sample_rate: SAMPLE_RATE,
        channels: 1,
        samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use motion_script::{Scene, Script, Transition};

    fn fixture(durations: &[f64], overlap: Option<f64>) -> (SequenceTimeline, Vec<RenderedScene>) {
        let mut script = Script {
            rig_clips: Default::default(),
            project: Default::default(),
            characters: Default::default(),
            animation_clips: Default::default(),
            scenes: durations
                .iter()
                .enumerate()
                .map(|(i, duration)| Scene {
                    id: format!("s{i}"),
                    duration: *duration,
                    events: vec![],
                    composition: Default::default(),
                    background: None,
                })
                .collect(),
        };
        if let Some(duration) = overlap {
            script.scenes[0].composition.transition = Transition::Crossfade { duration };
        }
        let timeline = SequenceTimeline::compile(&script).unwrap();
        let rendered = script
            .scenes
            .iter()
            .enumerate()
            .map(|(index, scene)| {
                let mut result =
                    crate::export::render_scene(&scene.id, scene.duration, &[]).unwrap();
                result
                    .audio
                    .samples
                    .fill(if index == 0 { 0.25 } else { 0.75 });
                result
            })
            .collect();
        (timeline, rendered)
    }
    #[test]
    fn cuts_preserve_fractional_lengths_and_impulses() {
        let (timeline, mut scenes) = fixture(&[1.3 / 24_000.0, 2.7 / 24_000.0], None);
        scenes[0].audio.samples = vec![1.25];
        scenes[1].audio.samples = vec![0.0, -0.5, 0.0];
        let result = mix_sequence(&timeline, &scenes, &CancellationToken::default()).unwrap();
        assert_eq!(result.samples, vec![1.25, 0.0, -0.5, 0.0]);
        assert_eq!(scenes[0].audio.samples, vec![1.25]);
    }
    #[test]
    fn crossfade_start_middle_end_matches_shared_clock() {
        let (timeline, scenes) = fixture(&[0.002, 0.002], Some(0.001));
        let result = mix_sequence(&timeline, &scenes, &CancellationToken::default()).unwrap();
        assert_eq!(result.samples[24], 0.25);
        assert_eq!(result.samples[36], 0.5);
        assert!((result.samples[47] - (0.25 / 24.0 + 0.75 * 23.0 / 24.0)).abs() < 1e-7);
        assert_eq!(result.samples[48], 0.75);
        for (global, actual) in result.samples.iter().enumerate() {
            let expected: f64 = timeline
                .sample(global as u64)
                .unwrap()
                .iter()
                .map(|c| {
                    scenes[c.scene_index].audio.samples[c.local_sample as usize] as f64 * c.weight
                })
                .sum();
            assert!((*actual as f64 - expected).abs() < 1e-7);
        }
    }
    #[test]
    fn crossfade_impulses_use_local_samples() {
        let (timeline, mut scenes) = fixture(&[0.002, 0.002], Some(0.001));
        for scene in &mut scenes {
            scene.audio.samples.fill(0.0);
        }
        scenes[0].audio.samples[36] = 0.5;
        scenes[1].audio.samples[12] = 1.0;
        let result = mix_sequence(&timeline, &scenes, &CancellationToken::default()).unwrap();
        assert_eq!(result.samples[36], 0.75);
        assert_eq!(result.samples.iter().filter(|v| **v != 0.0).count(), 1);
    }
    #[test]
    fn rejects_order_length_nonfinite_and_cancelled_work() {
        let (timeline, mut scenes) = fixture(&[0.001, 0.001], None);
        scenes.swap(0, 1);
        assert!(mix_sequence(&timeline, &scenes, &CancellationToken::default()).is_err());
        scenes.swap(0, 1);
        scenes[0].audio.samples.pop();
        assert!(mix_sequence(&timeline, &scenes, &CancellationToken::default()).is_err());
        scenes[0].audio.samples.push(f32::NAN);
        assert!(mix_sequence(&timeline, &scenes, &CancellationToken::default()).is_err());
        let cancel = CancellationToken::default();
        cancel.cancel();
        assert!(mix_sequence(&timeline, &[], &cancel).is_err());
    }
    #[test]
    fn rejects_budget_before_validating_or_allocating_scene_buffers() {
        let script =
            motion_script::parse("characters: {}\nscenes: [{id: huge, duration: 601, events: []}]")
                .unwrap();
        let timeline = SequenceTimeline::compile(&script).unwrap();
        assert!(mix_sequence(&timeline, &[], &CancellationToken::default())
            .unwrap_err()
            .0
            .contains("limits"));
    }
    #[test]
    fn rejects_invalid_manifest_without_touching_sources() {
        let (timeline, mut scenes) = fixture(&[0.001], None);
        scenes[0].manifest.schema_version = 2;
        assert!(mix_sequence(&timeline, &scenes, &CancellationToken::default()).is_err());
        assert!(scenes[0].audio.samples.iter().all(|s| *s == 0.25));
    }
}
