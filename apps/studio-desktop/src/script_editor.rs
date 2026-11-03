//! MotionScript editor helpers: YAML syntax colours, canonical formatting and
//! jumping to a syntax error. The formatter itself is `motion_script::format`;
//! this module only adapts it to the egui text editor.
use eframe::egui::{
    self,
    cache::{ComputerMut, FrameCache},
    text::{CCursor, LayoutJob},
    text_selection::CCursorRange,
    Color32, FontId, TextFormat,
};

/// Larger sources are shown without colours to keep typing responsive.
pub const HIGHLIGHT_LIMIT: usize = 512 * 1024;

/// Line and column (both 1-based) of a YAML syntax error, if `source` has one.
pub fn syntax_error(source: &str) -> Option<(usize, usize)> {
    match serde_yaml::from_str::<serde_yaml::Value>(source) {
        Err(error) => error.location().map(|l| (l.line(), l.column())),
        Ok(_) => None,
    }
}

/// Character offset of a 1-based line and column, clamped to the text.
pub fn char_index(source: &str, line: usize, column: usize) -> usize {
    let mut index = 0;
    for (number, text) in source.split_inclusive('\n').enumerate() {
        if number + 1 == line {
            let width = text.trim_end_matches(['\n', '\r']).chars().count();
            return index + column.saturating_sub(1).min(width);
        }
        index += text.chars().count();
    }
    source.chars().count()
}

/// Move the editor's cursor to `index` and focus it so it scrolls into view.
pub fn place_cursor(ctx: &egui::Context, id: egui::Id, index: usize) {
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(CCursorRange::one(CCursor::new(index))));
    state.store(ctx, id);
    ctx.memory_mut(|memory| memory.request_focus(id));
}

/// Status text for a format result: what changed and which comments are gone.
pub fn format_status(result: &motion_script::format::Formatted, changed: bool) -> String {
    let base = if changed {
        "MotionScript muotoiltu. Data ei muuttunut; Kumoa palauttaa edellisen tekstin."
    } else {
        "MotionScript oli jo muotoiltu."
    };
    match result.dropped_comments {
        0 => base.to_owned(),
        n => format!(
            "{base} {n} kommenttiriviä osion sisältä poistui; tiedoston alun ja osioiden yläpuolen kommentit säilyivät."
        ),
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum Kind {
    Text,
    Key,
    Str,
    Number,
    Keyword,
    Comment,
    Punct,
}

struct Palette {
    text: Color32,
    key: Color32,
    string: Color32,
    number: Color32,
    keyword: Color32,
    comment: Color32,
    punct: Color32,
}

impl Palette {
    fn new(dark: bool) -> Self {
        if dark {
            Self {
                text: Color32::from_rgb(0xD8, 0xDC, 0xE2),
                key: Color32::from_rgb(0x7F, 0xB8, 0xF0),
                string: Color32::from_rgb(0xA8, 0xD4, 0x8C),
                number: Color32::from_rgb(0xE8, 0xB8, 0x6C),
                keyword: Color32::from_rgb(0xD0, 0x98, 0xE8),
                comment: Color32::from_rgb(0x8A, 0x92, 0x9C),
                punct: Color32::from_rgb(0x9A, 0xA4, 0xB0),
            }
        } else {
            Self {
                text: Color32::from_rgb(0x24, 0x29, 0x2E),
                key: Color32::from_rgb(0x00, 0x55, 0xA8),
                string: Color32::from_rgb(0x2E, 0x6E, 0x1E),
                number: Color32::from_rgb(0xA0, 0x50, 0x00),
                keyword: Color32::from_rgb(0x80, 0x30, 0xA0),
                comment: Color32::from_rgb(0x6A, 0x73, 0x7D),
                punct: Color32::from_rgb(0x57, 0x60, 0x6A),
            }
        }
    }
    fn color(&self, kind: Kind) -> Color32 {
        match kind {
            Kind::Text => self.text,
            Kind::Key => self.key,
            Kind::Str => self.string,
            Kind::Number => self.number,
            Kind::Keyword => self.keyword,
            Kind::Comment => self.comment,
            Kind::Punct => self.punct,
        }
    }
}

/// Split one line of YAML into coloured byte ranges.
fn tokens(line: &str, out: &mut Vec<(usize, usize, Kind)>) {
    let bytes = line.as_bytes();
    let mut i = 0;
    let len = bytes.len();
    let push = |out: &mut Vec<(usize, usize, Kind)>, s: usize, e: usize, k: Kind| {
        if e > s {
            out.push((s, e, k));
        }
    };
    while i < len {
        let c = bytes[i];
        match c {
            b' ' | b'\t' => {
                let s = i;
                while i < len && matches!(bytes[i], b' ' | b'\t') {
                    i += 1;
                }
                push(out, s, i, Kind::Text);
            }
            b'#' if i == 0 || matches!(bytes[i - 1], b' ' | b'\t') => {
                push(out, i, len, Kind::Comment);
                return;
            }
            b'"' | b'\'' => {
                let s = i;
                i += 1;
                while i < len {
                    if c == b'"' && bytes[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == c {
                        // '' is an escaped quote inside single quotes.
                        if c == b'\'' && i + 1 < len && bytes[i + 1] == b'\'' {
                            i += 2;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                let e = i.min(len);
                push(
                    out,
                    s,
                    e,
                    if key_follows(bytes, e) {
                        Kind::Key
                    } else {
                        Kind::Str
                    },
                );
            }
            b'{' | b'}' | b'[' | b']' | b',' | b':' | b'|' | b'>' => {
                push(out, i, i + 1, Kind::Punct);
                i += 1;
            }
            b'-' if i + 1 >= len || bytes[i + 1] == b' ' => {
                push(out, i, i + 1, Kind::Punct);
                i += 1;
            }
            _ => {
                // A plain scalar runs to flow punctuation, ": " or " #".
                let s = i;
                while i < len {
                    let b = bytes[i];
                    if matches!(b, b',' | b'{' | b'}' | b'[' | b']') {
                        break;
                    }
                    if b == b':'
                        && (i + 1 >= len || matches!(bytes[i + 1], b' ' | b',' | b'}' | b']'))
                    {
                        break;
                    }
                    if b == b'#' && matches!(bytes[i - 1], b' ' | b'\t') {
                        break;
                    }
                    i += 1;
                }
                // Trailing spaces belong to the next token.
                let mut e = i;
                while e > s && bytes[e - 1] == b' ' {
                    e -= 1;
                }
                let word = &line[s..e];
                let kind = if key_follows(bytes, i) {
                    Kind::Key
                } else if matches!(word, "true" | "false" | "null" | "~") {
                    Kind::Keyword
                } else if word.parse::<f64>().is_ok() {
                    Kind::Number
                } else {
                    Kind::Text
                };
                push(out, s, e, kind);
                push(out, e, i, Kind::Text);
            }
        }
    }
}

fn key_follows(bytes: &[u8], mut i: usize) -> bool {
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    i < bytes.len()
        && bytes[i] == b':'
        && (i + 1 >= bytes.len() || matches!(bytes[i + 1], b' ' | b',' | b'}' | b']'))
}

/// A coloured layout of MotionScript YAML.
pub fn layout(text: &str, dark: bool, font: FontId) -> LayoutJob {
    let palette = Palette::new(dark);
    let mut job = LayoutJob::default();
    let format = |kind| TextFormat::simple(font.clone(), palette.color(kind));
    if text.len() > HIGHLIGHT_LIMIT {
        job.append(text, 0.0, format(Kind::Text));
        return job;
    }
    let mut spans = Vec::new();
    let mut literal: Option<usize> = None;
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let indent = body.len() - body.trim_start().len();
        spans.clear();
        // Lines inside a `|` / `>` block scalar are text, not YAML structure.
        let in_literal = literal.is_some_and(|open| body.trim().is_empty() || indent > open);
        if in_literal {
            spans.push((0, body.len(), Kind::Str));
        } else {
            literal = None;
            tokens(body, &mut spans);
            let trimmed = body.trim_end();
            if ["|", "|-", "|+", ">", ">-", ">+"].iter().any(|m| {
                trimmed.ends_with(&format!(": {m}")) || trimmed.ends_with(&format!("- {m}"))
            }) {
                literal = Some(indent);
            }
        }
        for &(start, end, kind) in &spans {
            job.append(&body[start..end], 0.0, format(kind));
        }
        if body.len() < line.len() {
            job.append(&line[body.len()..], 0.0, format(Kind::Text));
        }
    }
    job
}

#[derive(Default)]
struct Highlighter;

impl ComputerMut<(&str, bool, &FontId), LayoutJob> for Highlighter {
    fn compute(&mut self, (text, dark, font): (&str, bool, &FontId)) -> LayoutJob {
        layout(text, dark, font.clone())
    }
}

type HighlightCache = FrameCache<LayoutJob, Highlighter>;

/// Layouter for `TextEdit::layouter`; layouts are cached per text and theme.
pub fn layouter(ui: &egui::Ui, text: &str, wrap_width: f32) -> std::sync::Arc<egui::Galley> {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let dark = ui.visuals().dark_mode;
    let mut job = ui.ctx().memory_mut(|memory| {
        memory
            .caches
            .cache::<HighlightCache>()
            .get((text, dark, &font))
    });
    job.wrap.max_width = wrap_width;
    ui.fonts(|fonts| fonts.layout_job(job))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(line: &str) -> Vec<(String, Kind)> {
        let mut spans = Vec::new();
        tokens(line, &mut spans);
        spans
            .into_iter()
            .filter(|(_, _, k)| *k != Kind::Text || !line.is_empty())
            .map(|(s, e, k)| (line[s..e].to_owned(), k))
            .filter(|(t, _)| !t.trim().is_empty())
            .collect()
    }

    #[test]
    fn keys_values_and_comments_are_classified() {
        assert_eq!(
            kinds("  - {at: 0.3, easing: smoothstep, on: true, c: '#fff'} # note"),
            vec![
                ("-".into(), Kind::Punct),
                ("{".into(), Kind::Punct),
                ("at".into(), Kind::Key),
                (":".into(), Kind::Punct),
                ("0.3".into(), Kind::Number),
                (",".into(), Kind::Punct),
                ("easing".into(), Kind::Key),
                (":".into(), Kind::Punct),
                ("smoothstep".into(), Kind::Text),
                (",".into(), Kind::Punct),
                ("on".into(), Kind::Key),
                (":".into(), Kind::Punct),
                ("true".into(), Kind::Keyword),
                (",".into(), Kind::Punct),
                ("c".into(), Kind::Key),
                (":".into(), Kind::Punct),
                ("'#fff'".into(), Kind::Str),
                ("}".into(), Kind::Punct),
                ("# note".into(), Kind::Comment),
            ]
        );
        assert_eq!(
            kinds("title: Kilsat 01 – Ilmainen työmatka"),
            vec![
                ("title".into(), Kind::Key),
                (":".into(), Kind::Punct),
                ("Kilsat 01 – Ilmainen työmatka".into(), Kind::Text),
            ]
        );
        assert_eq!(kinds("# Header"), vec![("# Header".into(), Kind::Comment)]);
        assert_eq!(
            kinds("url: a#b"),
            vec![
                ("url".into(), Kind::Key),
                (":".into(), Kind::Punct),
                ("a#b".into(), Kind::Text)
            ]
        );
    }

    #[test]
    fn layout_keeps_every_character() {
        let text = "# c\nproject: {title: \"ä \\\" x\"}\ntext: |\n  key: not a key\nnext: 1\n";
        let job = layout(text, true, FontId::monospace(12.0));
        assert_eq!(job.text, text);
        let big = "a: 1\n".repeat(HIGHLIGHT_LIMIT / 5 + 1);
        assert_eq!(
            layout(&big, false, FontId::monospace(12.0)).sections.len(),
            1
        );
    }

    #[test]
    fn syntax_errors_map_to_cursor_positions() {
        let source = "a: 1\nb: [1,\n";
        let (line, column) = syntax_error(source).unwrap();
        assert!(line >= 2, "{line}:{column}");
        assert_eq!(char_index("ab\ncäd\n", 2, 2), 4);
        assert_eq!(char_index("ab\n", 9, 1), 3);
        assert_eq!(char_index("ab\ncd", 2, 99), 5);
        assert!(syntax_error("a: 1\n").is_none());
    }
}
