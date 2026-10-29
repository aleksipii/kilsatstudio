//! Version 1 scheduling language lowered into the existing editable Script.
use crate::{Event, Script};
use serde::Deserialize;
use serde_yaml::Value;

pub const VERSION: u32 = 1;
pub const MAX_SOURCE_BYTES: usize = 1_048_576;
pub const MAX_NODES: usize = 1024;
pub const MAX_DEPTH: usize = 32;

/// Diagnostic category. Each variant is a distinct, stable failure class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticCode {
    /// Malformed YAML, including duplicate keys.
    Syntax,
    /// Missing or unknown field, or a value of the wrong shape.
    Schema,
    /// Unsupported document version.
    Version,
    /// Unknown scene or actor reference.
    Reference,
    /// Non-finite, negative or zero time, interval outside its span or scene,
    /// or precision loss.
    Time,
    /// Unknown schedule operation or event type; reported, never skipped.
    Unsupported,
    /// Rejected by the existing `Script::validate`.
    Validation,
    /// Source size, node count or nesting depth limit exceeded.
    Limit,
}

impl DiagnosticCode {
    /// Stable lowercase name used in messages and documentation.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Syntax => "syntax",
            Self::Schema => "schema",
            Self::Version => "version",
            Self::Reference => "reference",
            Self::Time => "time",
            Self::Unsupported => "unsupported",
            Self::Validation => "validation",
            Self::Limit => "limit",
        }
    }
}

impl std::fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{code} at {path}: {message}")]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    /// YAML field path; sequence indexes are zero based.
    pub path: String,
    pub message: String,
    /// One-based location when provided by the YAML parser.
    pub line: Option<usize>,
    pub column: Option<usize>,
}
fn fail(code: DiagnosticCode, path: &str, message: impl ToString) -> Diagnostic {
    Diagnostic {
        code,
        path: path.into(),
        message: message.to_string(),
        line: None,
        column: None,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    script: Value,
    schedules: Value,
}

/// Events and original source paths indexed by scene then event.
/// Paths are revision-local, not persistent editor IDs.
#[derive(Debug)]
pub struct Compilation {
    pub script: Script,
    pub origins: Vec<Vec<String>>,
}

/// Compile atomically. The caller receives no partial Script on failure.
/// No assets, speech models, network services or random sources are accessed.
pub fn compile(source: &str) -> Result<Script, Diagnostic> {
    compile_with_origins(source).map(|result| result.script)
}

/// Compile with provenance for rule diagnostics and timeline inspection.
pub fn compile_with_origins(source: &str) -> Result<Compilation, Diagnostic> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(fail(DiagnosticCode::Limit, "$", "source exceeds 1 MiB"));
    }
    // Malformed YAML (with its location) is a syntax error; a well-formed document
    // with missing or unknown envelope fields is a schema error.
    let value: Value = serde_yaml::from_str(source).map_err(|error| {
        let mut diagnostic = fail(DiagnosticCode::Syntax, "$", &error);
        if let Some(location) = error.location() {
            diagnostic.line = Some(location.line());
            diagnostic.column = Some(location.column());
        }
        diagnostic
    })?;
    let document: Document =
        serde_yaml::from_value(value).map_err(|error| fail(DiagnosticCode::Schema, "$", error))?;
    if document.version != VERSION {
        return Err(fail(
            DiagnosticCode::Version,
            "$.version",
            "only version 1 is supported",
        ));
    }
    let script_yaml = serde_yaml::to_string(&document.script)
        .map_err(|error| fail(DiagnosticCode::Schema, "$.script", error))?;
    let mut script = crate::parse(&script_yaml).map_err(|error| script_error("$.script", error))?;
    let mut origins: Vec<Vec<String>> = script
        .scenes
        .iter()
        .enumerate()
        .map(|(s, scene)| {
            (0..scene.events.len())
                .map(|e| format!("$.script.scenes[{s}].events[{e}]"))
                .collect()
        })
        .collect();
    let mut count = 0;
    let schedules = document.schedules.as_mapping().ok_or_else(|| {
        fail(
            DiagnosticCode::Schema,
            "$.schedules",
            "expected scene-id mapping",
        )
    })?;
    for (scene_id, node) in schedules {
        let scene_id = scene_id.as_str().ok_or_else(|| {
            fail(
                DiagnosticCode::Schema,
                "$.schedules",
                "scene keys must be strings",
            )
        })?;
        let path = format!("$.schedules[{scene_id:?}]");
        let scene = script
            .scenes
            .iter()
            .position(|scene| scene.id == scene_id)
            .ok_or_else(|| fail(DiagnosticCode::Reference, &path, "unknown scene"))?;
        let mut compiler = Compiler {
            script: &mut script,
            scene,
            count: &mut count,
            origins: &mut origins[scene],
        };
        compiler.node(node, 0.0, &path, 0)?;
    }
    for (scene, origins) in script.scenes.iter_mut().zip(&mut origins) {
        let mut ordered: Vec<_> = std::mem::take(&mut scene.events)
            .into_iter()
            .zip(std::mem::take(origins))
            .collect();
        ordered.sort_by(|a, b| a.0.actor_time().0.total_cmp(&b.0.actor_time().0));
        (scene.events, *origins) = ordered.into_iter().unzip();
    }
    script
        .validate()
        .map_err(|error| script_error("$.script", error))?;
    Ok(Compilation { script, origins })
}

struct Compiler<'a> {
    script: &'a mut Script,
    scene: usize,
    count: &'a mut usize,
    origins: &'a mut Vec<String>,
}
impl Compiler<'_> {
    fn node(
        &mut self,
        node: &Value,
        start: f64,
        path: &str,
        depth: usize,
    ) -> Result<f64, Diagnostic> {
        *self.count += 1;
        if depth > MAX_DEPTH || *self.count > MAX_NODES {
            return Err(fail(
                DiagnosticCode::Limit,
                path,
                "schedule exceeds depth 32 or 1024 nodes",
            ));
        }
        let mapping = node
            .as_mapping()
            .ok_or_else(|| fail(DiagnosticCode::Schema, path, "expected mapping"))?;
        let op = node.get("op").and_then(Value::as_str).ok_or_else(|| {
            fail(
                DiagnosticCode::Schema,
                &format!("{path}.op"),
                "expected operation name",
            )
        })?;
        let allowed: &[&str] = match op {
            "seq" | "par" => &["op", "children"],
            "stagger" => &["op", "children", "delay"],
            "wait" => &["op", "duration"],
            "event" => &["op", "span", "event"],
            _ => {
                return Err(fail(
                    DiagnosticCode::Unsupported,
                    &format!("{path}.op"),
                    format!("unsupported operation {op}"),
                ))
            }
        };
        for key in mapping.keys() {
            if !key.as_str().is_some_and(|key| allowed.contains(&key)) {
                return Err(fail(
                    DiagnosticCode::Schema,
                    path,
                    format!("unknown field {key:?}"),
                ));
            }
        }
        let duration = match op {
            "wait" => number(node, "duration", path, true)?,
            "event" => {
                let span = number(node, "span", path, true)?;
                let event_path = format!("{path}.event");
                let value = node
                    .get("event")
                    .ok_or_else(|| fail(DiagnosticCode::Schema, &event_path, "missing event"))?;
                let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
                if !["move", "gesture", "speak", "emotion"].contains(&kind) {
                    return Err(fail(
                        DiagnosticCode::Unsupported,
                        &format!("{event_path}.type"),
                        "expected move, gesture, speak or emotion",
                    ));
                }
                let mut event: Event = serde_yaml::from_value(value.clone())
                    .map_err(|error| fail(DiagnosticCode::Schema, &event_path, error))?;
                let (offset, actor) = event.actor_time();
                if !self.script.characters.contains_key(actor) {
                    return Err(fail(
                        DiagnosticCode::Reference,
                        &format!("{event_path}.actor"),
                        format!("unknown actor {actor}"),
                    ));
                }
                if !offset.is_finite() || offset < 0.0 || offset >= span {
                    return Err(fail(
                        DiagnosticCode::Time,
                        &format!("{event_path}.at"),
                        "at must be finite and within [0, span)",
                    ));
                }
                if let Event::Move { duration, .. } | Event::Gesture { duration, .. } = &event {
                    if !duration.is_finite()
                        || *duration <= 0.0
                        || offset + duration > span
                        || offset + duration <= offset
                    {
                        return Err(fail(
                            DiagnosticCode::Time,
                            &format!("{event_path}.duration"),
                            "positive representable duration must fit span",
                        ));
                    }
                }
                let at = start + offset;
                if !at.is_finite() || (offset > 0.0 && at <= start) {
                    return Err(fail(
                        DiagnosticCode::Time,
                        &event_path,
                        "event time overflow or precision loss",
                    ));
                }
                if let Event::Move { duration, .. } | Event::Gesture { duration, .. } = &event {
                    if !(at + duration).is_finite()
                        || at + duration <= at
                        || at + duration > start + span
                    {
                        return Err(fail(
                            DiagnosticCode::Time,
                            &format!("{event_path}.duration"),
                            "absolute interval overflows or loses precision",
                        ));
                    }
                }
                match &mut event {
                    Event::Move { at: time, .. }
                    | Event::Gesture { at: time, .. }
                    | Event::Speak { at: time, .. }
                    | Event::Emotion { at: time, .. }
                    | Event::Face { at: time, .. }
                    | Event::Hold { at: time, .. }
                    | Event::Pose { at: time, .. } => *time = at,
                }
                self.script.scenes[self.scene].events.push(event);
                self.origins.push(event_path.clone());
                // Existing validators enforce numeric parameters, supported values,
                // assets, scene boundaries and conflicting move intervals.
                self.script
                    .validate()
                    .map_err(|error| script_error(&event_path, error))?;
                span
            }
            _ => {
                let children_path = format!("{path}.children");
                let children = node
                    .get("children")
                    .and_then(Value::as_sequence)
                    .ok_or_else(|| {
                        fail(
                            DiagnosticCode::Schema,
                            &children_path,
                            "expected children sequence",
                        )
                    })?;
                if children.is_empty() {
                    return Err(fail(
                        DiagnosticCode::Schema,
                        &children_path,
                        "children must not be empty",
                    ));
                }
                let delay = if op == "stagger" {
                    number(node, "delay", path, false)?
                } else {
                    0.0
                };
                let mut end: f64 = start;
                let mut previous_start = start;
                for (index, child) in children.iter().enumerate() {
                    let child_start = if op == "seq" {
                        end
                    } else {
                        start + index as f64 * delay
                    };
                    if !child_start.is_finite()
                        || (index > 0 && delay > 0.0 && child_start <= previous_start)
                    {
                        return Err(fail(
                            DiagnosticCode::Time,
                            path,
                            "start time overflow or precision loss",
                        ));
                    }
                    previous_start = child_start;
                    let span = self.node(
                        child,
                        child_start,
                        &format!("{children_path}[{index}]"),
                        depth + 1,
                    )?;
                    end = end.max(child_start + span);
                }
                end - start
            }
        };
        let end = start + duration;
        if !end.is_finite() || end <= start || end > self.script.scenes[self.scene].duration {
            return Err(fail(
                DiagnosticCode::Time,
                path,
                "interval overflows, loses precision or exceeds scene duration",
            ));
        }
        Ok(duration)
    }
}
fn number(node: &Value, key: &str, path: &str, positive: bool) -> Result<f64, Diagnostic> {
    let value = node.get(key).and_then(Value::as_f64).ok_or_else(|| {
        fail(
            DiagnosticCode::Schema,
            &format!("{path}.{key}"),
            "expected number",
        )
    })?;
    if !value.is_finite() || value < 0.0 || (positive && value == 0.0) {
        return Err(fail(
            DiagnosticCode::Time,
            &format!("{path}.{key}"),
            "expected finite nonnegative time (duration/span must be positive)",
        ));
    }
    Ok(value)
}

fn script_error(path: &str, error: crate::Error) -> Diagnostic {
    let code = match &error {
        crate::Error::Syntax(_) => DiagnosticCode::Schema,
        crate::Error::Unsupported(_) => DiagnosticCode::Unsupported,
        crate::Error::Validation(_) => DiagnosticCode::Validation,
    };
    fail(code, path, error)
}
