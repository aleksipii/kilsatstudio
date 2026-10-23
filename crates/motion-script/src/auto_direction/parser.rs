//! Strict, line-oriented dialogue input with explicit emotion annotations.
use super::{IntentLine, MAX_LINES, MAX_TEXT_BYTES};
use crate::Script;
use std::collections::BTreeSet;

fn line_error(line: usize, message: impl std::fmt::Display) -> String {
    format!("Line {line}: {message}")
}

/// Parse `CHARACTER: text` or `CHARACTER[emotion]: text` without inventing intent.
/// Character names and emotion values are exact, case-sensitive identifiers.
/// Blank lines are ignored; source line numbers retain their original positions.
pub fn parse_dialogue(text: &str, script: &Script) -> Result<Vec<IntentLine>, String> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(line_error(
            1,
            format!("dialogue exceeds the {MAX_TEXT_BYTES}-byte input limit"),
        ));
    }
    let mut lines = Vec::new();
    let mut speakers = BTreeSet::new();
    for (index, source) in text.lines().enumerate() {
        let source_line = index + 1;
        if source.trim().is_empty() {
            continue;
        }
        if lines.len() >= MAX_LINES {
            return Err(line_error(
                source_line,
                format!("at most {MAX_LINES} dialogue lines are supported"),
            ));
        }
        let (header, dialogue) = source.split_once(':').ok_or_else(|| {
            line_error(
                source_line,
                "use CHARACTER: text or CHARACTER[emotion]: text",
            )
        })?;
        let header = header.trim();
        let (actor, emotion) = if let Some((actor, annotation)) = header.split_once('[') {
            let emotion = annotation
                .strip_suffix(']')
                .ok_or_else(|| line_error(source_line, "emotion annotation must end with ]"))?;
            if emotion.contains(['[', ']']) {
                return Err(line_error(source_line, "malformed emotion annotation"));
            }
            let emotion = emotion.trim();
            crate::validate_emotion(emotion).map_err(|error| line_error(source_line, error))?;
            (actor.trim(), Some(emotion.to_owned()))
        } else {
            if header.contains(']') {
                return Err(line_error(source_line, "malformed emotion annotation"));
            }
            (header, None)
        };
        if actor.is_empty() || !script.characters.contains_key(actor) {
            return Err(line_error(
                source_line,
                format!("unknown character {actor:?}"),
            ));
        }
        let dialogue = dialogue.trim();
        if dialogue.is_empty() {
            return Err(line_error(source_line, "dialogue text is empty"));
        }
        speakers.insert(actor);
        if speakers.len() > 2 {
            return Err(line_error(
                source_line,
                "this direction profile supports at most two speaking characters",
            ));
        }
        lines.push(IntentLine {
            actor: actor.into(),
            text: dialogue.into(),
            emotion,
            source_line,
        });
    }
    if lines.is_empty() {
        return Err(line_error(1, "write at least one dialogue line"));
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script() -> Script {
        crate::parse(
            "characters: {A: {asset: a}, B: {asset: b}, C: {asset: c}}\nscenes: [{id: existing, duration: 2, events: []}]\n",
        )
        .unwrap()
    }

    #[test]
    fn preserves_unicode_colons_explicit_intent_and_source_lines() {
        let input = "\n A [ happy ]: Hei: tämä on animaatio! 🎬\r\n\nB: That is ironic.\n";
        let lines = parse_dialogue(input, &script()).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].actor, "A");
        assert_eq!(lines[0].text, "Hei: tämä on animaatio! 🎬");
        assert_eq!(lines[0].emotion.as_deref(), Some("happy"));
        assert_eq!(lines[0].source_line, 2);
        assert_eq!(lines[1].source_line, 4);
        assert_eq!(lines[1].emotion, None);
    }

    #[test]
    fn accepts_every_existing_emotion_and_rejects_unsupported_annotations() {
        for emotion in ["neutral", "happy", "sad", "surprised", "angry"] {
            let lines = parse_dialogue(&format!("A[{emotion}]: Hello"), &script()).unwrap();
            assert_eq!(lines[0].emotion.as_deref(), Some(emotion));
        }
        for header in [
            "A[ironic]",
            "A[]",
            "A[happy",
            "A]",
            "A[[happy]]",
            "A[happy] extra",
        ] {
            let error = parse_dialogue(&format!("\n{header}: Hello"), &script()).unwrap_err();
            assert!(error.starts_with("Line 2:"), "{header}: {error}");
        }
    }

    #[test]
    fn rejects_missing_unknown_empty_and_third_speakers_with_source_lines() {
        for input in ["A Hello", "UNKNOWN: Hello", ": Hello", "A:  "] {
            assert!(parse_dialogue(&format!("\n{input}"), &script())
                .unwrap_err()
                .starts_with("Line 2:"));
        }
        assert!(parse_dialogue("\n\n", &script()).is_err());
        let error = parse_dialogue("A: One\n\nB: Two\nC: Three", &script()).unwrap_err();
        assert!(error.starts_with("Line 4:"));
        assert!(error.contains("two speaking characters"));
    }

    #[test]
    fn enforces_exact_byte_and_line_budgets() {
        let maximum = format!("A: {}", "x".repeat(MAX_TEXT_BYTES - 3));
        assert!(parse_dialogue(&maximum, &script()).is_ok());
        assert!(parse_dialogue(&format!("{maximum}x"), &script()).is_err());
        // A multi-byte character is charged by bytes, rather than display width.
        let unicode_over = format!("A: {}ä", "x".repeat(MAX_TEXT_BYTES - 4));
        assert_eq!(unicode_over.len(), MAX_TEXT_BYTES + 1);
        assert!(parse_dialogue(&unicode_over, &script()).is_err());
        let maximum_lines = "A: Hello\n".repeat(MAX_LINES);
        assert_eq!(
            parse_dialogue(&maximum_lines, &script()).unwrap().len(),
            MAX_LINES
        );
        let error = parse_dialogue(&format!("{maximum_lines}\nB: Extra"), &script()).unwrap_err();
        assert!(error.starts_with(&format!("Line {}:", MAX_LINES + 2)));
    }
}
