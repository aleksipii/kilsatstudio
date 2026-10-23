//! Pure 24 kHz scheduling. Supplied lengths are caller data, not verified PCM.
use super::{
    IntentLine, TimedLine, TimingSource, GAP_SAMPLES, LEAD_SAMPLES, MAX_DURATION_SAMPLES,
    MAX_LINES, MAX_TEXT_BYTES, SAMPLE_RATE,
};

/// Reserve one lead-in, each complete line and a fixed gap after every line.
/// Without supplied lengths, count whitespace-separated words at 2.5 words/sec,
/// with a one-second minimum. This heuristic does not recognize phonemes or
/// establish real speech duration in either Finnish or English.
pub fn time_lines(
    lines: &[IntentLine],
    supplied_samples: Option<&[u64]>,
) -> Result<Vec<TimedLine>, String> {
    if lines.is_empty() || lines.len() > MAX_LINES {
        return Err(format!("Timing needs 1–{MAX_LINES} dialogue lines."));
    }
    if supplied_samples.is_some_and(|samples| samples.len() != lines.len()) {
        return Err("Supply exactly one sample count per dialogue line.".into());
    }
    let timing_source = if supplied_samples.is_some() {
        TimingSource::SuppliedSamples
    } else {
        TimingSource::Estimated
    };
    let mut cursor = LEAD_SAMPLES;
    let mut text_bytes = 0usize;
    let mut result = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        text_bytes = text_bytes
            .checked_add(line.text.len())
            .ok_or("Dialogue byte count overflow.")?;
        if text_bytes > MAX_TEXT_BYTES {
            return Err(format!(
                "Dialogue exceeds the {MAX_TEXT_BYTES}-byte timing budget."
            ));
        }
        if line.text.trim().is_empty() {
            return Err(format!("Line {}: dialogue is empty.", line.source_line));
        }
        let duration_samples = match supplied_samples {
            Some(samples) => samples[index],
            None => {
                let words = u64::try_from(line.text.split_whitespace().count())
                    .map_err(|_| "Word count overflow.")?;
                words
                    .checked_mul(SAMPLE_RATE * 2 / 5)
                    .ok_or("Estimated speech duration overflow.")?
                    .max(SAMPLE_RATE)
            }
        };
        if duration_samples == 0 || duration_samples > MAX_DURATION_SAMPLES {
            return Err(format!(
                "Line {}: sample count must be positive and at most {MAX_DURATION_SAMPLES}.",
                line.source_line
            ));
        }
        let end_with_gap = cursor
            .checked_add(duration_samples)
            .and_then(|end| end.checked_add(GAP_SAMPLES))
            .ok_or("Dialogue schedule overflow.")?;
        if end_with_gap > MAX_DURATION_SAMPLES {
            return Err(format!(
                "Line {}: dialogue with lead-in and gaps exceeds 60 seconds; shorten the scene rather than cutting speech.",
                line.source_line
            ));
        }
        result.push(TimedLine {
            line: line.clone(),
            start_sample: cursor,
            duration_samples,
            timing_source,
        });
        cursor = end_with_gap;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str) -> IntentLine {
        IntentLine {
            actor: "EMMA".into(),
            text: text.into(),
            emotion: None,
            source_line: 7,
        }
    }

    #[test]
    fn supplied_samples_preserve_every_sample_and_fixed_gaps() {
        let lines = vec![line("Hello"); MAX_LINES];
        let lengths = vec![12_345; MAX_LINES];
        let timed = time_lines(&lines, Some(&lengths)).unwrap();
        for (index, item) in timed.iter().enumerate() {
            assert_eq!(
                item.start_sample,
                LEAD_SAMPLES + index as u64 * (12_345 + GAP_SAMPLES)
            );
            assert_eq!(item.duration_samples, 12_345);
            assert_eq!(item.timing_source, TimingSource::SuppliedSamples);
            assert_eq!(item.line, lines[index]);
        }
    }

    #[test]
    fn sixty_second_bound_includes_lead_and_final_gap() {
        let lines = [line("Full length")];
        let maximum = MAX_DURATION_SAMPLES - LEAD_SAMPLES - GAP_SAMPLES;
        let timed = time_lines(&lines, Some(&[maximum])).unwrap();
        assert_eq!(
            timed[0].start_sample + timed[0].duration_samples + GAP_SAMPLES,
            MAX_DURATION_SAMPLES
        );
        assert!(time_lines(&lines, Some(&[maximum + 1])).is_err());
        assert!(time_lines(&lines, Some(&[u64::MAX])).is_err());
        assert!(time_lines(&lines, Some(&[0])).is_err());
        assert!(time_lines(&lines, Some(&[MAX_DURATION_SAMPLES])).is_err());
    }

    #[test]
    fn estimated_timing_is_deterministic_and_never_marked_measured() {
        let lines = [line("Hei maailma"), line("One two three four five")];
        let first = time_lines(&lines, None).unwrap();
        assert_eq!(first, time_lines(&lines, None).unwrap());
        assert_eq!(first[0].duration_samples, SAMPLE_RATE);
        assert_eq!(first[1].duration_samples, 2 * SAMPLE_RATE);
        assert_eq!(
            first[1].start_sample,
            LEAD_SAMPLES + SAMPLE_RATE + GAP_SAMPLES
        );
        assert!(first
            .iter()
            .all(|item| item.timing_source == TimingSource::Estimated));
        assert_eq!(lines[0].text, "Hei maailma");
    }

    #[test]
    fn invalid_inputs_fail_without_truncation_or_partial_results() {
        assert!(time_lines(&[], None).is_err());
        assert!(time_lines(&vec![line("Hi"); MAX_LINES + 1], None).is_err());
        assert!(time_lines(&[line(" ")], None)
            .unwrap_err()
            .contains("Line 7"));
        assert!(time_lines(&[line("Hello")], Some(&[])).is_err());
        assert!(time_lines(&[line("Hello")], Some(&[24_000, 24_000])).is_err());
        assert!(time_lines(&[line(&"a".repeat(MAX_TEXT_BYTES + 1))], None).is_err());
        assert!(time_lines(&[line(&"word ".repeat(200))], None).is_err());
    }
}
