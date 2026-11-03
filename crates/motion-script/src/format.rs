//! Canonical, readable MotionScript YAML.
//!
//! One formatter serves the editor, `to_source`, the `format_script` tool and
//! the example generators, so every screenplay written by Koeta Studio looks
//! the same:
//!
//! * block maps and sequences with a two-space indent;
//! * short maps and lists whose values are all scalars (keyframes, points,
//!   colours) on one line in flow form, e.g. `- {at: 0.3, rotation: -0.1}`;
//! * scalars quoted exactly as `serde_yaml` quotes them (YAML 1.2 core schema).
//!
//! Comments above top-level sections (and the file header) are kept; comments
//! inside a section cannot be carried through a data round trip and are
//! counted in [`Formatted::dropped_comments`] so the caller can say so.
//!
//! Every result is parsed again and compared with the input value. Formatting
//! that would change the data is refused instead of returned.

use crate::Error;
use serde_yaml::Value;
use std::collections::HashMap;

/// Longest one-line flow map or list, not counting indentation.
pub const FLOW_WIDTH: usize = 96;
const INDENT: &str = "  ";

/// A formatted screenplay and what happened to its comments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Formatted {
    pub text: String,
    /// Header and top-level section comment lines written to `text`.
    pub kept_comments: usize,
    /// Comment lines that sat inside a section or after a value and are gone.
    pub dropped_comments: usize,
}

/// Format a YAML value without comments.
pub fn format_value(value: &Value) -> Result<String, Error> {
    let text = emit(value, &Comments::default())?;
    verify(&text, value)?;
    Ok(text)
}

/// Reformat MotionScript text. Only YAML syntax is required: an invalid or
/// unfinished screenplay can still be tidied before it is checked.
pub fn format_source(source: &str) -> Result<Formatted, Error> {
    let value: Value = serde_yaml::from_str(source)?;
    format_with_comments(&value, source)
}

/// Format `value`, carrying the header and top-level comments of `original`
/// to the sections with the same keys.
pub(crate) fn format_with_comments(value: &Value, original: &str) -> Result<Formatted, Error> {
    let mut comments = Comments::scan(original);
    let used = comments.retain_keys(value);
    let text = emit(value, &comments)?;
    verify(&text, value)?;
    Ok(Formatted {
        text,
        kept_comments: used,
        dropped_comments: comments.total - used,
    })
}

fn verify(text: &str, value: &Value) -> Result<(), Error> {
    let again: Value = serde_yaml::from_str(text)?;
    if &again != value {
        return Err(Error::Unsupported(
            "YAML formatting would change the data; the text was left unchanged".into(),
        ));
    }
    Ok(())
}

fn emit(value: &Value, comments: &Comments) -> Result<String, Error> {
    let mut lines = Vec::new();
    lines.extend(comments.header.iter().cloned());
    match value {
        Value::Mapping(map) if !map.is_empty() => {
            // One blank line after the header and around multi-line or
            // commented sections; consecutive one-line settings stay together.
            let mut previous_multiline = !comments.header.is_empty();
            for (key, item) in map {
                let section = key.as_str().and_then(|k| comments.sections.get(k));
                let mut entry = Vec::new();
                map_entry(&mut entry, key_text(key)?, item, 0)?;
                let multiline = entry.len() > 1 || section.is_some();
                if !lines.is_empty() && (multiline || previous_multiline) {
                    lines.push(String::new());
                }
                if let Some(section) = section {
                    lines.extend(section.iter().cloned());
                }
                lines.extend(entry);
                previous_multiline = multiline;
            }
        }
        Value::Sequence(items) if !items.is_empty() => {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            block(&mut lines, value, 0)?;
        }
        _ => {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(inline(value)?.unwrap_or_default());
        }
    }
    if !comments.trailer.is_empty() {
        lines.push(String::new());
        lines.extend(comments.trailer.iter().cloned());
    }
    let mut text = lines.join("\n");
    text.push('\n');
    Ok(text)
}

fn pad(depth: usize) -> String {
    INDENT.repeat(depth)
}

fn block(out: &mut Vec<String>, value: &Value, depth: usize) -> Result<(), Error> {
    match value {
        Value::Mapping(map) => {
            for (key, item) in map {
                map_entry(out, key_text(key)?, item, depth)?;
            }
        }
        Value::Sequence(items) => {
            for item in items {
                sequence_item(out, item, depth)?;
            }
        }
        Value::Tagged(tagged) => {
            out.push(format!("{}{}", pad(depth), tagged.tag));
            block(out, &tagged.value, depth)?;
        }
        _ => out.push(format!(
            "{}{}",
            pad(depth),
            inline(value)?.unwrap_or_default()
        )),
    }
    Ok(())
}

fn map_entry(out: &mut Vec<String>, key: String, item: &Value, depth: usize) -> Result<(), Error> {
    let head = format!("{}{key}:", pad(depth));
    if let Some(text) = inline(item)? {
        out.push(format!("{head} {text}"));
    } else if let Value::Tagged(tagged) = item {
        out.push(format!("{head} {}", tagged.tag));
        block(out, &tagged.value, depth + 1)?;
    } else {
        out.push(head);
        block(out, item, depth + 1)?;
    }
    Ok(())
}

fn sequence_item(out: &mut Vec<String>, item: &Value, depth: usize) -> Result<(), Error> {
    let dash = format!("{}- ", pad(depth));
    if let Some(text) = inline(item)? {
        out.push(format!("{dash}{text}"));
    } else if let Value::Tagged(tagged) = item {
        out.push(format!("{dash}{}", tagged.tag));
        block(out, &tagged.value, depth + 1)?;
    } else {
        // The first line of a nested block shares the dash line.
        let mut nested = Vec::new();
        block(&mut nested, item, depth + 1)?;
        let child = pad(depth + 1);
        for (index, line) in nested.into_iter().enumerate() {
            if index == 0 {
                out.push(format!(
                    "{dash}{}",
                    line.strip_prefix(&child).unwrap_or(&line)
                ));
            } else {
                out.push(line);
            }
        }
    }
    Ok(())
}

/// One-line text for scalars, empty collections and short scalar collections.
fn inline(value: &Value) -> Result<Option<String>, Error> {
    Ok(match value {
        Value::Mapping(map) if map.is_empty() => Some("{}".into()),
        Value::Sequence(items) if items.is_empty() => Some("[]".into()),
        Value::Mapping(map) => {
            let mut parts = Vec::with_capacity(map.len());
            for (key, item) in map {
                match (flow_scalar(key)?, flow_scalar(item)?) {
                    (Some(k), Some(v)) => parts.push(format!("{k}: {v}")),
                    _ => return Ok(None),
                }
            }
            fit(format!("{{{}}}", parts.join(", ")))
        }
        Value::Sequence(items) => {
            let mut parts = Vec::with_capacity(items.len());
            for item in items {
                if let Some(text) = flow_scalar(item)? {
                    parts.push(text);
                } else if let Value::Sequence(inner) = item {
                    let mut row = Vec::with_capacity(inner.len());
                    for cell in inner {
                        match flow_scalar(cell)? {
                            Some(text) => row.push(text),
                            None => return Ok(None),
                        }
                    }
                    parts.push(format!("[{}]", row.join(", ")));
                } else {
                    return Ok(None);
                }
            }
            fit(format!("[{}]", parts.join(", ")))
        }
        Value::Tagged(tagged) => match &tagged.value {
            Value::Mapping(_) | Value::Sequence(_) => None,
            inner => Some(format!("{} {}", tagged.tag, scalar(inner)?)),
        },
        _ => Some(scalar(value)?),
    })
}

fn fit(text: String) -> Option<String> {
    (text.chars().count() <= FLOW_WIDTH).then_some(text)
}

/// A scalar that is also safe inside `{...}` or `[...]`.
fn flow_scalar(value: &Value) -> Result<Option<String>, Error> {
    if matches!(
        value,
        Value::Mapping(_) | Value::Sequence(_) | Value::Tagged(_)
    ) {
        return Ok(None);
    }
    let text = scalar(value)?;
    let quoted = text.starts_with('\'') || text.starts_with('"');
    let safe = quoted || !text.contains([',', '[', ']', '{', '}', '#', ':']);
    Ok(safe.then_some(text))
}

fn key_text(key: &Value) -> Result<String, Error> {
    match key {
        Value::Mapping(_) | Value::Sequence(_) | Value::Tagged(_) => Err(Error::Unsupported(
            "YAML map keys must be plain values".into(),
        )),
        _ => scalar(key),
    }
}

/// A single-line scalar, quoted as serde_yaml would quote it.
fn scalar(value: &Value) -> Result<String, Error> {
    let text = serde_yaml::to_string(value)?;
    let text = text.strip_suffix('\n').unwrap_or(&text);
    if text.contains('\n') {
        // serde_yaml uses block scalars for multi-line strings; a JSON string
        // is a valid YAML double-quoted scalar and stays on one line.
        if let Value::String(s) = value {
            return serde_json::to_string(s)
                .map_err(|e| Error::Unsupported(format!("YAML string: {e}")));
        }
        return Err(Error::Unsupported("multi-line YAML scalar".into()));
    }
    Ok(text.to_owned())
}

/// Comments found in the original text.
#[derive(Debug, Default)]
struct Comments {
    header: Vec<String>,
    sections: HashMap<String, Vec<String>>,
    trailer: Vec<String>,
    total: usize,
}

impl Comments {
    fn scan(source: &str) -> Self {
        let mut found = Self::default();
        let mut pending: Vec<String> = Vec::new();
        let mut seen_key = false;
        let mut header_end = 0;
        // Indentation of a line that opened a `|` or `>` block scalar.
        let mut literal: Option<usize> = None;
        for line in source.lines() {
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            if let Some(open) = literal {
                if trimmed.is_empty() || indent > open {
                    continue;
                }
                literal = None;
            }
            if trimmed.is_empty() {
                // Before the first key, a blank line ends the file header.
                if !seen_key && !pending.is_empty() {
                    header_end = pending.len();
                }
                continue;
            }
            if trimmed == "---" || trimmed == "..." {
                continue;
            }
            if trimmed.starts_with('#') {
                found.total += 1;
                if indent == 0 {
                    pending.push(line.trim_end().to_owned());
                }
                // An indented comment is inside a section and is dropped.
                continue;
            }
            if inline_comment(trimmed) {
                found.total += 1;
            }
            if opens_block_scalar(trimmed) {
                literal = Some(indent);
            }
            if indent == 0 && !trimmed.starts_with('-') {
                let mut comments = std::mem::take(&mut pending);
                if !seen_key {
                    found.header = comments.drain(..header_end).collect();
                    seen_key = true;
                }
                if let Some(key) = top_key(trimmed) {
                    if !comments.is_empty() {
                        found.sections.entry(key).or_default().extend(comments);
                    }
                }
            } else if !pending.is_empty() {
                // A column-0 comment followed by nested content belongs to
                // the section's body and cannot be kept.
                pending.clear();
            }
        }
        found.trailer = pending;
        found
    }

    /// Drop section comments whose key is missing; return the kept line count.
    fn retain_keys(&mut self, value: &Value) -> usize {
        let keys: Vec<&str> = match value {
            Value::Mapping(map) => map.keys().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        self.sections.retain(|key, _| keys.contains(&key.as_str()));
        self.header.len() + self.trailer.len() + self.sections.values().map(Vec::len).sum::<usize>()
    }
}

fn top_key(line: &str) -> Option<String> {
    if let Ok(Value::Mapping(map)) = serde_yaml::from_str::<Value>(line) {
        if map.len() == 1 {
            return map.keys().next().and_then(Value::as_str).map(str::to_owned);
        }
    }
    let (key, _) = line.split_once(':')?;
    Some(key.trim().trim_matches(['\'', '"']).to_owned())
}

fn opens_block_scalar(line: &str) -> bool {
    let line = line.trim_end();
    ["|", "|-", "|+", ">", ">-", ">+"].iter().any(|marker| {
        line.ends_with(&format!(": {marker}")) || line.ends_with(&format!("- {marker}"))
    })
}

/// Whether a content line ends in a `# comment` outside quotes.
fn inline_comment(line: &str) -> bool {
    let mut quote: Option<char> = None;
    let mut previous = ' ';
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '\'' || c == '"' => quote = Some(c),
            None if c == '#' && previous.is_whitespace() => return true,
            None => {}
        }
        previous = c;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(text: &str) -> Value {
        serde_yaml::from_str(text).unwrap()
    }

    #[test]
    fn short_scalar_collections_stay_on_one_line() {
        let text = format_value(&value(
            r##"{"scene": {"camera": [{"at": 0, "x": 1.5, "y": -2}], "points": [[0, 1], [2, 3]], "color": "#3B3350"}}"##,
        ))
        .unwrap();
        assert_eq!(
            text,
            "scene:\n  camera:\n    - {at: 0, x: 1.5, y: -2}\n  points: [[0, 1], [2, 3]]\n  color: '#3B3350'\n"
        );
    }

    #[test]
    fn long_or_nested_values_use_block_form() {
        let long: Vec<u32> = (0..60).collect();
        let source =
            serde_json::json!({"a": long, "b": {"c": {"d": 1}}, "e": [{"f": [1, {"g": 2}]}]});
        let text = format_value(&serde_yaml::to_value(&source).unwrap()).unwrap();
        assert!(text.contains("a:\n  - 0\n  - 1\n"), "{text}");
        assert!(text.contains("b:\n  c: {d: 1}\n"), "{text}");
        assert!(
            text.contains("e:\n  - f:\n      - 1\n      - {g: 2}\n"),
            "{text}"
        );
    }

    #[test]
    fn tricky_strings_round_trip() {
        for s in [
            "a, b",
            "x: y",
            "#hash",
            "on",
            "y",
            "null",
            "true",
            "24",
            "1e3",
            "- dash",
            "multi\nline",
            "trailing ",
            " leading",
            "quote ' and \"",
            "{brace}",
            "[list]",
            "ääkköset – ok",
            "",
            "tab\there",
            "back\\slash",
            "*star",
            "&amp",
            "!bang",
            "%pct",
            "@at",
            "`tick`",
            "a #b",
        ] {
            let source = serde_json::json!({"k": s, "list": [s, 1], "map": {"v": s}, s: 1});
            let original = serde_yaml::to_value(&source).unwrap();
            let text = format_value(&original).unwrap_or_else(|e| panic!("{s:?}: {e}"));
            assert_eq!(value(&text), original, "{s:?}\n{text}");
        }
    }

    #[test]
    fn formatting_is_idempotent() {
        let once = format_source("b: [1, 2]\na:\n- x: 1\n  y: {z: [1, 2]}\n")
            .unwrap()
            .text;
        assert_eq!(format_source(&once).unwrap().text, once);
    }

    #[test]
    fn header_and_section_comments_are_kept_inner_ones_counted() {
        let source = "# Header\n\n# Project settings\nproject: {title: A}\n# Scenes\nscenes:\n  # inner\n  - id: s # trailing\n";
        let formatted = format_source(source).unwrap();
        assert_eq!(
            formatted.text,
            "# Header\n\n# Project settings\nproject: {title: A}\n\n# Scenes\nscenes:\n  - {id: s}\n"
        );
        assert_eq!(
            (formatted.kept_comments, formatted.dropped_comments),
            (3, 2)
        );
    }

    #[test]
    fn block_scalar_content_is_not_a_comment() {
        let formatted =
            format_source("text: |\n  line # not a comment\n  # neither\nnext: 1\n").unwrap();
        assert_eq!(formatted.dropped_comments, 0);
        assert_eq!(
            formatted.text,
            "text: \"line # not a comment\\n# neither\\n\"\nnext: 1\n"
        );
    }

    #[test]
    fn syntax_errors_are_reported_and_tags_survive() {
        assert!(matches!(format_source("a: [1,"), Err(Error::Syntax(_))));
        let tagged =
            value("a: !Move {x: 1}\nb: !Pose\n  arm:\n    - 1\n    - {deep: [1, {k: 2}]}\n");
        let text = format_value(&tagged).unwrap();
        assert_eq!(value(&text), tagged);
    }
}
