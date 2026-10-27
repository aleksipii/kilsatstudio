//! Builds `motion_lipsync::audio_timeline::AudioTimeline` v1 from a prepared scene.
//! Mouth intervals are taken from the scene export manifest, so the timeline carries
//! exactly the cues that export writes, with the same provenance.
use crate::{dialogue_jobs, scene_audio::PreparedScene};
use motion_lipsync::{
    audio_timeline::{
        AudioSourceInfo, AudioTimeline, AudioTimelineClip, MouthTiming, SamplePhoneme,
    },
    english::VisemeCue,
};
use motion_script::Script;
use motion_tts::SpeechError;
use std::{collections::BTreeMap, path::Path};

/// Provenance for a synthesized line, supplied by whoever configured the provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SynthesisOrigin {
    pub provider: String,
    pub model_revision: String,
    pub voice_revision: Option<String>,
    pub cache_key: String,
}

/// `synthesized` maps dialogue IDs of lines without imported audio to their origin; a
/// synthesized line without an entry is an error rather than unlabelled audio.
pub fn scene_audio_timeline(
    script: &Script,
    project_root: &Path,
    prepared: &PreparedScene,
    synthesized: &BTreeMap<u64, SynthesisOrigin>,
) -> Result<AudioTimeline, SpeechError> {
    let jobs: Vec<_> = dialogue_jobs(script, project_root)?
        .into_iter()
        .filter(|job| job.scene == prepared.scene_id)
        .collect();
    let manifest = &prepared.rendered.manifest;
    if jobs.len() != prepared.clips.len() || manifest.dialogue.len() != prepared.clips.len() {
        return Err(SpeechError(
            "prepared scene does not match the screenplay's dialogue".into(),
        ));
    }
    let mut timeline = AudioTimeline::new(prepared.scene_id.clone(), manifest.total_frames);
    for ((job, clip), entry) in jobs.iter().zip(&prepared.clips).zip(&manifest.dialogue) {
        let fail = |what: &str| SpeechError(format!("dialogue {} ({}): {what}", job.id, job.actor));
        if clip.actor != job.actor || entry.start_frame != clip.start_frame {
            return Err(fail("prepared clip order differs from the screenplay"));
        }
        let pcm_sha256 =
            motion_lipsync::rhubarb::pcm_identity(&clip.audio).map_err(|e| fail(&e.to_string()))?;
        let source = match &job.imported_audio {
            Some(path) => AudioSourceInfo::Imported {
                path: path
                    .strip_prefix(project_root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .into_owned(),
                pcm_sha256,
            },
            None => {
                let origin = synthesized
                    .get(&job.id)
                    .ok_or_else(|| fail("synthesized audio has no recorded provider origin"))?;
                AudioSourceInfo::Synthesized {
                    provider: origin.provider.clone(),
                    model_revision: origin.model_revision.clone(),
                    voice: job.request.voice.preset.clone(),
                    voice_revision: origin.voice_revision.clone(),
                    cache_key: origin.cache_key.clone(),
                    pcm_sha256,
                }
            }
        };
        let source_samples = clip.audio.frames() as u64;
        // Manifest cues are scene samples of the visible window; store them source-local.
        let to_local = |scene: u64| scene - clip.start_frame + clip.trim_start_frame;
        let mouth = entry.alignment_source.clone().map(|source| MouthTiming {
            source,
            phonemes: clip
                .alignment
                .iter()
                .flat_map(|a| &a.phonemes)
                .filter_map(|p| {
                    let start = crate::export::sample_boundary(p.start_seconds, source_samples);
                    let end = crate::export::sample_boundary(p.end_seconds, source_samples);
                    (start < end).then(|| SamplePhoneme {
                        phoneme: p.phoneme.clone(),
                        start_sample: start,
                        end_sample: end,
                    })
                })
                .collect(),
            visemes: entry
                .visemes
                .iter()
                .map(|v| VisemeCue {
                    start_sample: to_local(v.start_frame),
                    end_sample: to_local(v.end_frame),
                    viseme: v.viseme,
                })
                .collect(),
        });
        timeline.clips.push(AudioTimelineClip {
            id: job.id,
            actor: job.actor.clone(),
            language: job.request.voice.language.clone(),
            source,
            source_samples,
            start_sample: clip.start_frame,
            trim_start_samples: clip.trim_start_frame,
            duration_samples: entry.duration_frames,
            gain: clip.gain,
            mouth,
        });
    }
    timeline
        .validate()
        .map_err(|e| SpeechError(e.to_string()))?;
    Ok(timeline)
}
