//! Stateless, bounded sequence timing on the shared audio sample clock.
use crate::Error;
use motion_script::{Script, Transition};

pub const SEQUENCE_SAMPLE_RATE: u32 = 24_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneSpan {
    pub scene_index: usize,
    pub scene_id: String,
    pub start_sample: u64,
    pub duration_samples: u64,
    pub outgoing_overlap: u64,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneContribution {
    pub scene_index: usize,
    pub local_sample: u64,
    pub weight: f64,
}
#[derive(Debug, Clone)]
pub struct SequenceTimeline {
    spans: Vec<SceneSpan>,
    total_samples: u64,
}
fn invalid(message: &str) -> Error {
    Error::InvalidSequence(message.into())
}
fn samples(seconds: f64) -> Result<u64, Error> {
    let value = (seconds * f64::from(SEQUENCE_SAMPLE_RATE)).round();
    if !seconds.is_finite() || seconds <= 0.0 || value < 1.0 || value >= u64::MAX as f64 {
        return Err(invalid("duration must round to a representable positive sample count"));
    }
    Ok(value as u64)
}
impl SequenceTimeline {
    pub fn compile(script: &Script) -> Result<Self, Error> {
        if script.scenes.is_empty() || script.scenes.len() > 1024 {
            return Err(invalid("expected 1..1024 scenes"));
        }
        script.validate()?;
        let mut spans = Vec::with_capacity(script.scenes.len());
        let mut start = 0_u64;
        let mut total = 0_u64;
        for (index, scene) in script.scenes.iter().enumerate() {
            let duration = samples(scene.duration)?;
            let overlap = match scene.composition.transition {
                Transition::Cut => 0,
                Transition::Crossfade { duration } => samples(duration)?,
            };
            total = start.checked_add(duration).ok_or_else(|| invalid("sequence duration overflow"))?;
            spans.push(SceneSpan {
                scene_index: index,
                scene_id: scene.id.clone(),
                start_sample: start,
                duration_samples: duration,
                outgoing_overlap: overlap,
            });
            start = total.checked_sub(overlap).ok_or_else(|| invalid("crossfade duration overflow"))?;
        }
        Ok(Self { spans, total_samples: total })
    }
    pub fn spans(&self) -> &[SceneSpan] { &self.spans }
    pub fn total_samples(&self) -> u64 { self.total_samples }
    /// Evaluate [0, total_samples). At the first overlap sample both scenes
    /// are returned and the incoming weight is exactly zero.
    pub fn sample(&self, global_sample: u64) -> Result<Vec<SceneContribution>, Error> {
        if global_sample >= self.total_samples { return Err(Error::InvalidTime); }
        let mut contributions = Vec::with_capacity(2);
        for (index, span) in self.spans.iter().enumerate() {
            if global_sample < span.start_sample || global_sample - span.start_sample >= span.duration_samples { continue; }
            let local = global_sample - span.start_sample;
            let mut weight = 1.0;
            if index > 0 {
                let overlap = self.spans[index - 1].outgoing_overlap;
                if local < overlap { weight = local as f64 / overlap as f64; }
            }
            if span.outgoing_overlap > 0 {
                let fade_start = span.duration_samples - span.outgoing_overlap;
                if local >= fade_start { weight = 1.0 - (local - fade_start) as f64 / span.outgoing_overlap as f64; }
            }
            contributions.push(SceneContribution { scene_index: span.scene_index, local_sample: local, weight });
        }
        Ok(contributions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn script(durations: &[f64]) -> Script {
        Script { project: Default::default(), characters: Default::default(), scenes: durations.iter().enumerate().map(|(i, duration)| motion_script::Scene { composition: Default::default(), background: None, id: format!("s{i}"), duration: *duration, events: vec![] }).collect() }
    }
    #[test]
    fn cut_boundaries_and_legacy_defaults() {
        let value = motion_script::parse("characters: {}\nscenes: [{id: a, duration: 1, events: []}, {id: b, duration: 2, events: []}]").unwrap();
        assert_eq!(value.scenes[0].composition.transition, Transition::Cut);
        let timeline = SequenceTimeline::compile(&value).unwrap();
        assert_eq!(timeline.total_samples(), 72_000);
        assert_eq!(timeline.sample(23_999).unwrap()[0].scene_index, 0);
        assert_eq!(timeline.sample(24_000).unwrap(), vec![SceneContribution { scene_index: 1, local_sample: 0, weight: 1.0 }]);
        assert!(timeline.sample(72_000).is_err());
    }
    #[test]
    fn crossfade_weights_and_random_seeks_are_exact() {
        let mut value = script(&[2.0, 2.0, 1.0]);
        value.scenes[0].composition.transition = Transition::Crossfade { duration: 1.0 };
        let timeline = SequenceTimeline::compile(&value).unwrap();
        assert_eq!(timeline.total_samples(), 96_000);
        assert_eq!(timeline.spans()[1].start_sample, 24_000);
        for (sample, outgoing, incoming) in [(36_000, 0.5, 0.5), (24_000, 1.0, 0.0), (42_000, 0.25, 0.75), (30_000, 0.75, 0.25)] {
            let result = timeline.sample(sample).unwrap();
            assert_eq!(result.len(), 2);
            assert_eq!(result[0].weight, outgoing);
            assert_eq!(result[1].weight, incoming);
            assert_eq!(result[1].local_sample, sample - 24_000);
        }
        assert_eq!(timeline.sample(48_000).unwrap().len(), 1);
    }
    #[test]
    fn sample_rounding_and_adjacent_crossfades_never_have_three_scenes() {
        let mut value = script(&[0.000125, 0.000125, 0.000125]);
        value.scenes[0].composition.transition = Transition::Crossfade { duration: 0.000063 };
        value.scenes[1].composition.transition = Transition::Crossfade { duration: 0.000042 };
        let timeline = SequenceTimeline::compile(&value).unwrap();
        assert_eq!(timeline.total_samples(), 6);
        assert_eq!(timeline.spans()[0].outgoing_overlap, 2);
        for sample in 0..timeline.total_samples() {
            let contributions = timeline.sample(sample).unwrap();
            assert!((1..=2).contains(&contributions.len()));
            assert_eq!(contributions.iter().map(|c| c.weight).sum::<f64>(), 1.0);
        }
        assert_eq!(timeline.sample(3).unwrap()[0].scene_index, 1);
    }
    #[test]
    fn sequence_budget_and_overflow_are_checked() {
        assert!(SequenceTimeline::compile(&script(&[])).is_err());
        assert!(SequenceTimeline::compile(&script(&[1.0; 1025])).is_err());
        let tiny = script(&[0.000001]);
        assert!(tiny.validate().is_ok());
        assert!(SequenceTimeline::compile(&tiny).is_err());
        assert!(SequenceTimeline::compile(&script(&[f64::MAX])).is_err());
        assert!(SequenceTimeline::compile(&script(&[5e14, 5e14])).is_err());
    }
}
