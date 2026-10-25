//! Integrates pure direction stages with the existing shot compiler.
use super::{
    acting, director, parser, timing, DirectionResult, TimingSource, GAP_SAMPLES,
    MAX_DURATION_SAMPLES, MAX_LINES, MAX_SOURCE_BYTES, SAMPLE_RATE,
};

/// Append a bounded automatic-direction draft to the existing screenplay.
///
/// Supplied durations are caller-provided 24 kHz sample counts. This function
/// does not inspect PCM, synthesize speech, infer accurate phoneme alignment or
/// approve artistic quality. Without supplied durations the result is explicitly
/// estimated. Authored scene data is retained, although YAML formatting and
/// comments are normalized by serialization. The caller owns Undo and saving.
pub fn direct(
    source: &str,
    dialogue: &str,
    supplied_samples: Option<&[u64]>,
) -> Result<DirectionResult, String> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err("Screenplay exceeds the 256 KiB source limit.".into());
    }
    let script = crate::parse(source).map_err(|error| error.to_string())?;
    let first_scene = script.scenes.len();
    let lines = parser::parse_dialogue(dialogue, &script)?;
    let timed = timing::time_lines(&lines, supplied_samples)?;
    let mut shots = director::build_shots(&script, &timed)?;
    acting::apply_acting(&script, &timed, &mut shots)?;
    if shots.shots.is_empty() || shots.shots.len() > MAX_LINES {
        return Err(format!("Direction requires 1–{MAX_LINES} final shots."));
    }
    // The bounded profile has one final shot per line. A later extension that
    // adds reaction shots must budget them here before invoking expand_shots.
    if shots.shots.iter().any(|shot| shot.reaction.is_some()) {
        return Err("Additional reaction shots are outside this direction profile.".into());
    }
    let expansion = crate::expand_shots(&script, &shots).map_err(|error| error.to_string())?;
    let added = &expansion.script.scenes[first_scene..];
    if added.len() != timed.len() || expansion.plan.len() != added.len() {
        return Err("Direction produced an inconsistent shot/line plan.".into());
    }
    let duration_samples = added.iter().try_fold(0_u64, |total, scene| {
        let samples = scene.duration * SAMPLE_RATE as f64;
        if !samples.is_finite() || samples <= 0.0 || samples > MAX_DURATION_SAMPLES as f64 {
            return Err("Directed scene duration is outside the sample budget.".to_string());
        }
        total
            .checked_add(samples.round() as u64)
            .ok_or_else(|| "Directed duration overflowed the sample clock.".to_string())
    })?;
    if duration_samples > MAX_DURATION_SAMPLES {
        return Err("Directed sequence exceeds the 60-second profile; shorten dialogue or split it into separate sequences.".into());
    }
    let timing_source = if supplied_samples.is_some() {
        TimingSource::SuppliedSamples
    } else {
        TimingSource::Estimated
    };
    if timed.iter().any(|line| line.timing_source != timing_source) {
        return Err("Direction stages returned inconsistent timing provenance.".into());
    }
    // No line may disappear or be shortened when it crosses into ordinary shot
    // scenes. Each stage uses the same sample clock; allow one sample only at
    // the f64-seconds conversion boundary, never a per-line growing tolerance.
    let base_seconds: f64 = script.scenes.iter().map(|scene| scene.duration).sum();
    for (line, plan) in timed.iter().zip(&expansion.plan) {
        let start = plan
            .dialogue_start
            .ok_or("A directed dialogue shot lost its dialogue start.")?;
        let end = plan
            .dialogue_end
            .ok_or("A directed dialogue shot lost its dialogue end.")?;
        let expected_start = line.start_sample as f64;
        let actual_start = (start - base_seconds) * SAMPLE_RATE as f64;
        let actual_duration = (end - start) * SAMPLE_RATE as f64;
        if (actual_start - expected_start).abs() > 1.0
            || (actual_duration - line.duration_samples as f64).abs() > 1.0
        {
            return Err("Shot compilation changed the scheduled dialogue timing.".into());
        }
    }
    let source = crate::to_source(&expansion.script).map_err(|error| error.to_string())?;
    if source.len() > MAX_SOURCE_BYTES {
        return Err("Directed screenplay exceeds the 256 KiB source limit.".into());
    }
    let mut diagnostics = vec![match timing_source {
        TimingSource::Estimated => "Draft timing: dialogue durations are text estimates. Generate or import actual speech, then retime before final export; no audio or lip-sync was generated.".into(),
        TimingSource::SuppliedSamples => "Timing uses caller-supplied 24 kHz sample counts. PCM and alignment have not been verified by the director; no audio or lip-sync was generated.".into(),
    }];
    if duration_samples < 30 * SAMPLE_RATE {
        diagnostics.push("This draft is shorter than the 30–60 second pilot target; silence was not added to inflate its duration.".into());
    }
    diagnostics.push(format!(
        "Deterministic two-character direction draft: {} shots; {} ms inter-line tail. Review framing, acting and timing before export. YAML formatting/comments are normalized; existing scenes remain in the project.",
        added.len(), GAP_SAMPLES * 1000 / SAMPLE_RATE
    ));
    Ok(DirectionResult {
        source,
        first_scene,
        shot_count: added.len(),
        duration_samples,
        timing_source,
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = include_str!("../../../../examples/hello-emma.yaml");
    const DIALOGUE: &str = "EMMA: Hello Alex!\nALEX: Let's make an animation.";

    #[test]
    fn end_to_end_is_deterministic_and_preserves_authored_scenes() {
        let first = direct(SOURCE, DIALOGUE, None).unwrap();
        let second = direct(SOURCE, DIALOGUE, None).unwrap();
        assert_eq!(first.source, second.source);
        assert_eq!(first.diagnostics, second.diagnostics);
        assert_eq!(first.timing_source, TimingSource::Estimated);
        assert_eq!(first.first_scene, 1);
        assert_eq!(first.shot_count, 2);
        let original = crate::parse(SOURCE).unwrap();
        let directed = crate::parse(&first.source).unwrap();
        assert_eq!(directed.scenes.len(), original.scenes.len() + 2);
        assert_eq!(
            serde_yaml::to_value(&directed.scenes[0]).unwrap(),
            serde_yaml::to_value(&original.scenes[0]).unwrap()
        );
        assert!(first
            .diagnostics
            .iter()
            .any(|message| message.contains("estimates")));
        let appended = direct(&first.source, DIALOGUE, None).unwrap();
        assert_eq!(appended.first_scene, 3);
        crate::parse(&appended.source).unwrap();
    }

    #[test]
    fn supplied_samples_remain_explicit_and_budget_is_never_silently_clipped() {
        let result = direct(
            SOURCE,
            DIALOGUE,
            Some(&[29 * SAMPLE_RATE, 29 * SAMPLE_RATE]),
        )
        .unwrap();
        assert_eq!(result.timing_source, TimingSource::SuppliedSamples);
        assert!(result.duration_samples <= MAX_DURATION_SAMPLES);
        assert!(result
            .diagnostics
            .iter()
            .any(|message| message.contains("not been verified")));
        assert!(direct(
            SOURCE,
            DIALOGUE,
            Some(&[30 * SAMPLE_RATE, 30 * SAMPLE_RATE])
        )
        .is_err());
        assert!(direct(SOURCE, DIALOGUE, Some(&[u64::MAX, 1])).is_err());
        assert!(direct(SOURCE, "UNKNOWN: Hello", None).is_err());
    }
}
