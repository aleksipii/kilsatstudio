//! Speaker-coloured captions on the shared SVG path.
//!
//! Text is laid out here with the metrics of the one bundled caption font
//! (`assets/fonts/Anton-Regular.ttf`, SIL OFL 1.1, see the repository NOTICE) so
//! word wrapping is deterministic and independent of the machine. The resulting
//! `<text>` elements are rasterized by resvg with a font database that contains
//! only that font (see `motion-export`), so preview, SVG, PNG and video export
//! agree. Characters the font lacks are reported as errors, never dropped.
use crate::{escape, validate_scene, Error};
use motion_scene::EvaluatedScene;
use motion_script::CaptionStyle;
use std::fmt::Write;
use std::sync::OnceLock;

/// SVG `font-family` used for captions; the only family in the font database.
pub const CAPTION_FONT_FAMILY: &str = "Anton";
/// Maximum caption text length in bytes (before wrapping).
pub const MAX_CAPTION_BYTES: usize = 1024;
const MAX_LINES: usize = 12;
const FONT_DATA: &[u8] = include_bytes!("../assets/fonts/Anton-Regular.ttf");

/// The bundled caption font file (OpenType/TrueType bytes).
pub fn caption_font_data() -> &'static [u8] {
    FONT_DATA
}

fn face() -> &'static ttf_parser::Face<'static> {
    static FACE: OnceLock<ttf_parser::Face<'static>> = OnceLock::new();
    FACE.get_or_init(|| ttf_parser::Face::parse(FONT_DATA, 0).expect("bundled font parses"))
}

/// One caption interval derived from a `speak` event.
#[derive(Debug, Clone, PartialEq)]
pub struct CaptionCue {
    pub speaker: String,
    pub text: String,
    /// Scene-local seconds, half-open `[start, end)`.
    pub start: f64,
    pub end: f64,
    /// True when `end` comes from a length estimate rather than prepared audio.
    pub estimated: bool,
}

fn caption_error(message: impl Into<String>) -> Error {
    Error::Caption(message.into())
}

/// Text as drawn: optionally upper-cased, whitespace collapsed to single spaces.
/// Fails on control characters, over-long text and glyphs missing from the font.
pub fn caption_display_text(style: &CaptionStyle, text: &str) -> Result<String, Error> {
    if text.len() > MAX_CAPTION_BYTES {
        return Err(caption_error(format!(
            "caption text exceeds {MAX_CAPTION_BYTES} bytes"
        )));
    }
    let prepared: String = if style.uppercase {
        text.to_uppercase()
    } else {
        text.to_owned()
    };
    let mut missing = Vec::new();
    for c in prepared.chars() {
        if c.is_whitespace() {
            continue;
        }
        if (c.is_control() || face().glyph_index(c).is_none()) && !missing.contains(&c) {
            missing.push(c);
        }
    }
    if !missing.is_empty() {
        let list: Vec<String> = missing
            .iter()
            .map(|c| format!("{c:?} (U+{:04X})", u32::from(*c)))
            .collect();
        return Err(caption_error(format!(
            "caption text has characters missing from the bundled caption font: {}",
            list.join(", ")
        )));
    }
    Ok(prepared.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Advance width of `text` at `font_size` using the bundled font's metrics
/// (no kerning, which makes wrapping slightly conservative).
pub fn text_width(text: &str, font_size: f64) -> f64 {
    let face = face();
    let scale = font_size / f64::from(face.units_per_em());
    text.chars()
        .map(|c| {
            face.glyph_index(c)
                .and_then(|g| face.glyph_hor_advance(g))
                .unwrap_or(0) as f64
                * scale
        })
        .sum()
}

/// Deterministic greedy word wrap. A single word wider than `max_width` is an
/// error: it is never split or clipped.
pub fn wrap_caption(
    style: &CaptionStyle,
    text: &str,
    max_width: f64,
) -> Result<Vec<String>, Error> {
    let display = caption_display_text(style, text)?;
    let mut lines: Vec<String> = Vec::new();
    for word in display.split(' ').filter(|w| !w.is_empty()) {
        if text_width(word, style.font_size) > max_width {
            return Err(caption_error(format!(
                "caption word {word:?} is wider than the maximum line width; reduce font_size or increase max_width"
            )));
        }
        match lines.last_mut() {
            Some(line) if text_width(&format!("{line} {word}"), style.font_size) <= max_width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    if lines.len() > MAX_LINES {
        return Err(caption_error(format!(
            "caption wraps to more than {MAX_LINES} lines"
        )));
    }
    Ok(lines)
}

/// Check one caption can be laid out for a canvas of `scene_width` x `scene_height`.
pub fn validate_caption(
    style: &CaptionStyle,
    text: &str,
    scene_width: u32,
    scene_height: u32,
) -> Result<(), Error> {
    let lines = wrap_caption(style, text, style.max_width * f64::from(scene_width))?;
    let height = lines.len() as f64 * style.font_size * style.line_height;
    if height > f64::from(scene_height) * (1.0 - style.bottom_margin) {
        return Err(caption_error(
            "caption does not fit vertically inside the frame",
        ));
    }
    Ok(())
}

/// Append every caption active at `scene.time` just before the closing `</svg>`,
/// in the same letterboxed canvas transform and opacity as the scene. Overlapping
/// cues stack upward with the latest cue lowest. `width`/`height` are the output
/// size given to the scene renderer. Does nothing when captions are disabled.
pub fn append_captions(
    svg: &mut String,
    scene: &EvaluatedScene,
    width: u32,
    height: u32,
    style: &CaptionStyle,
    cues: &[CaptionCue],
) -> Result<(), Error> {
    validate_scene(scene, width, height)?;
    if !style.enabled {
        return Ok(());
    }
    let mut active: Vec<&CaptionCue> = cues
        .iter()
        .filter(|cue| cue.start <= scene.time && scene.time < cue.end)
        .collect();
    active.sort_by(|a, b| a.start.total_cmp(&b.start));
    if active.is_empty() {
        return Ok(());
    }
    let (sw, sh) = (f64::from(scene.width), f64::from(scene.height));
    let scale = (f64::from(width) / sw).min(f64::from(height) / sh);
    let ox = (f64::from(width) - sw * scale) * 0.5;
    let oy = (f64::from(height) - sh * scale) * 0.5;
    let face = face();
    let descent = -f64::from(face.descender()) / f64::from(face.units_per_em()) * style.font_size;
    let pitch = style.font_size * style.line_height;
    let mut body = String::new();
    let mut bottom = sh * (1.0 - style.bottom_margin);
    for cue in active.iter().rev() {
        let lines = wrap_caption(style, &cue.text, style.max_width * sw)?;
        if lines.is_empty() {
            continue;
        }
        let top = bottom - lines.len() as f64 * pitch;
        if top < 0.0 {
            return Err(caption_error(
                "captions do not fit vertically inside the frame",
            ));
        }
        let fill = style.fill_for(&cue.speaker);
        for (index, line) in lines.iter().enumerate() {
            let from_bottom = (lines.len() - 1 - index) as f64;
            let baseline = bottom - descent - from_bottom * pitch;
            write!(
                body,
                "<text x=\"{:.3}\" y=\"{baseline:.3}\" text-anchor=\"middle\" font-family=\"{CAPTION_FONT_FAMILY}\" font-size=\"{:.3}\" fill=\"{fill}\"",
                sw * 0.5,
                style.font_size
            )
            .unwrap();
            if style.outline_width > 0.0 {
                // paint-order stroke puts the outline under the fill, so only the
                // outer half of the stroke is visible: double the visible width.
                write!(
                    body,
                    " stroke=\"{}\" stroke-width=\"{:.3}\" stroke-linejoin=\"round\" paint-order=\"stroke\"",
                    style.outline,
                    style.outline_width * 2.0
                )
                .unwrap();
            }
            write!(body, ">{}</text>", escape(line)).unwrap();
        }
        bottom = top;
    }
    if let Some(background) = &style.background {
        // A screen-space band keeps text off detailed props and blankets during
        // camera pushes. Stack it once behind all simultaneous cues, and leave
        // silent shots unchanged. Padding includes the outer glyph outline.
        let padding = style.font_size * 0.25 + style.outline_width;
        let top = (bottom - padding).max(0.0);
        body.insert_str(
            0,
            &format!(
                "<rect data-caption-background=\"true\" x=\"0\" y=\"{top:.3}\" width=\"{sw:.3}\" height=\"{:.3}\" fill=\"{}\"/>",
                sh - top,
                escape(background)
            ),
        );
    }
    let close = svg
        .rfind("</svg>")
        .ok_or_else(|| caption_error("SVG has no closing tag"))?;
    let group = format!(
        "<g id=\"captions\" transform=\"translate({ox:.6} {oy:.6}) scale({scale:.6})\" opacity=\"{:.6}\">{body}</g>",
        scene.opacity
    );
    svg.insert_str(close, &group);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style() -> CaptionStyle {
        CaptionStyle::default()
    }

    #[test]
    fn caption_backing_is_opt_in_and_only_drawn_behind_active_text() {
        let script = motion_script::parse(
            "project: {width: 640, height: 360}\ncharacters: {C: {asset: c}}\nscenes: [{id: s, duration: 2, events: []}]\n",
        )
        .unwrap();
        let scene = motion_scene::evaluate(&script, 0, 0.5).unwrap();
        let empty = "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>";
        let cue = CaptionCue {
            speaker: "C".into(),
            text: "Hello".into(),
            start: 0.0,
            end: 1.0,
            estimated: false,
        };
        let mut style = style();
        style.font_size = 28.0;
        let mut legacy = empty.to_string();
        append_captions(
            &mut legacy,
            &scene,
            640,
            360,
            &style,
            std::slice::from_ref(&cue),
        )
        .unwrap();
        assert!(!legacy.contains("data-caption-background"));
        style.background = Some("#15121f".into());
        let mut active = empty.to_string();
        append_captions(
            &mut active,
            &scene,
            640,
            360,
            &style,
            std::slice::from_ref(&cue),
        )
        .unwrap();
        assert!(
            active.find("data-caption-background").unwrap() < active.find("<text").unwrap()
        );
        assert!(active.contains("width=\"640.000\""));
        for cues in [Vec::new(), vec![CaptionCue { start: 1.0, ..cue }]] {
            let mut silent = empty.to_string();
            append_captions(&mut silent, &scene, 640, 360, &style, &cues).unwrap();
            assert_eq!(silent, empty);
        }
    }

    #[test]
    fn bundled_font_has_ascii_and_reports_missing_glyphs() {
        let face = face();
        for c in "ABCXYZ abcxyz 0123456789 .,!?'\"-:;()".chars() {
            assert!(face.glyph_index(c).is_some(), "missing {c:?}");
        }
        let error = caption_display_text(&style(), "hello \u{4f60}\u{597d}")
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("U+4F60") && error.contains("U+597D"),
            "{error}"
        );
        assert!(caption_display_text(&style(), "bell\u{7}").is_err());
    }

    #[test]
    fn uppercase_and_whitespace_collapse() {
        assert_eq!(
            caption_display_text(&style(), "  hi \n  there ").unwrap(),
            "HI THERE"
        );
        let mut s = style();
        s.uppercase = false;
        assert_eq!(caption_display_text(&s, "Hi  there").unwrap(), "Hi there");
    }

    #[test]
    fn wrapping_is_greedy_deterministic_and_bounded() {
        let s = style();
        let text = "hey stan what are you doing are you watching it";
        let wide = wrap_caption(&s, text, 100_000.0).unwrap();
        assert_eq!(wide.len(), 1);
        let max = 700.0;
        let lines = wrap_caption(&s, text, max).unwrap();
        assert!(lines.len() > 1);
        assert_eq!(lines, wrap_caption(&s, text, max).unwrap());
        for line in &lines {
            assert!(text_width(line, s.font_size) <= max, "{line}");
        }
        assert_eq!(lines.join(" "), wide[0]);
        // Greedy: the first word of the next line would not have fit.
        for pair in lines.windows(2) {
            let next = pair[1].split(' ').next().unwrap();
            assert!(text_width(&format!("{} {next}", pair[0]), s.font_size) > max);
        }
        let error = wrap_caption(&s, "supercalifragilistic", 100.0).unwrap_err();
        assert!(error.to_string().contains("wider than"));
    }
}
