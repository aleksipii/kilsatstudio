//! Storyboard sheet: one panel per shot, drawn with the export renderer.
//!
//! The output is a new directory with `storyboard.html` (a printable Finnish
//! sheet), `storyboard.json` (the same shots as data) and `panels/*.png`. Each
//! panel is the shot's panel moment (`motion_script::ShotInfo::panel`) rendered
//! exactly as the preview and the export show it, captions included.
use super::*;
use motion_project::{ProductionDocument, ShotStatus};
use motion_script::{ShotInfo, ShotSize};

/// Most panels on one sheet.
pub const MAX_STORYBOARD_PANELS: usize = 512;

#[derive(Debug, Clone, Serialize)]
pub struct StoryboardReport {
    pub shots: usize,
    pub output_directory: PathBuf,
}

/// Escape text for HTML element content and attribute values.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Seconds with a decimal comma, as Finnish text writes them.
fn seconds(value: f64) -> String {
    format!("{value:.1}").replace('.', ",")
}

fn size_label(size: ShotSize) -> &'static str {
    match size {
        ShotSize::Wide => "Laaja kuva",
        ShotSize::Medium => "Keskikuva",
        ShotSize::CloseUp => "Lähikuva",
    }
}

fn angle_label(angle: &str) -> &str {
    match angle {
        "front" => "etukuva",
        "reverse" => "vastakuva",
        "high" => "yläkulma",
        "low" => "alakulma",
        other => other,
    }
}

/// Finnish emotion name; an intensity after the name (`sad 0.4`) is kept.
fn emotion_label(value: &str) -> String {
    let (name, rest) = value.split_once(' ').unwrap_or((value, ""));
    let name = match name {
        "neutral" => "neutraali",
        "happy" => "iloinen",
        "sad" => "surullinen",
        "surprised" => "yllättynyt",
        "angry" => "vihainen",
        other => other,
    };
    if rest.is_empty() {
        name.to_string()
    } else {
        format!("{name} {}", rest.replace('.', ","))
    }
}

fn action_label(kind: &str) -> &str {
    match kind {
        "gesture" => "ele",
        "emotion" => "tunne",
        "move" => "liikkuu",
        "pose" => "asento",
        "face" => "kääntyy",
        "hold" => "ottaa",
        other => other,
    }
}

/// File name of a shot's panel: the id with every character outside
/// `[A-Za-z0-9_-]` replaced, plus the shot's index so names never collide.
fn panel_name(index: usize, shot: &ShotInfo) -> String {
    let id: String = shot
        .id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("{index:04}-{id}.png")
}

fn status_class(status: ShotStatus) -> &'static str {
    match status {
        ShotStatus::Done => "done",
        ShotStatus::Retake => "retake",
        ShotStatus::Storyboard | ShotStatus::Animatic => "early",
        _ => "work",
    }
}

fn sheet(title: &str, shots: &[ShotInfo], production: &ProductionDocument, total: f64) -> String {
    let mut html = String::new();
    let tracked = production.track(shots);
    let summary = production.summary(shots);
    let counts: Vec<String> = summary
        .iter()
        .map(|(status, n)| format!("{} {n}", status.label()))
        .collect();
    html.push_str(&format!(
        "<!doctype html>\n<html lang=\"fi\"><head><meta charset=\"utf-8\"><title>{t} – kuvakäsikirjoitus</title>\n<style>\n\
@page {{ size: A4 landscape; margin: 12mm; }}\n\
body {{ font: 13px/1.4 -apple-system, 'Helvetica Neue', sans-serif; color: #1d1d1f; margin: 24px; }}\n\
h1 {{ font-size: 20px; margin: 0 0 4px; }} .meta {{ color: #555; margin-bottom: 16px; }}\n\
.grid {{ display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; }}\n\
.panel {{ border: 1px solid #ccc; border-radius: 6px; padding: 8px; break-inside: avoid; }}\n\
.panel img {{ width: 100%; display: block; border: 1px solid #222; background: #000; }}\n\
.head {{ display: flex; justify-content: space-between; font-weight: 600; margin: 6px 0 2px; }}\n\
.camera {{ color: #555; font-size: 12px; }} .line {{ margin: 4px 0; }} .line b {{ font-variant: small-caps; }}\n\
.actions {{ color: #444; font-size: 12px; margin: 4px 0; }}\n\
.status {{ display: inline-block; border-radius: 4px; padding: 1px 6px; font-size: 11px; background: #e8e8ed; }}\n\
.status.done {{ background: #d3f2d6; }} .status.retake {{ background: #f9d0cc; }} .status.work {{ background: #fff0c2; }}\n\
.note {{ font-style: italic; margin-top: 4px; }} .moved {{ color: #b3261e; font-size: 12px; }}\n\
</style></head><body>\n<h1>{t}</h1>\n<div class=\"meta\">{n} otosta · {len} s · {counts}</div>\n<div class=\"grid\">\n",
        t = escape(title),
        n = shots.len(),
        len = seconds(total),
        counts = escape(&counts.join(" · ")),
    ));
    for (index, tracked) in tracked.iter().enumerate() {
        let shot = tracked.shot;
        let status = tracked.record.map_or(ShotStatus::Storyboard, |r| r.status);
        html.push_str(&format!(
            "<div class=\"panel\"><img src=\"panels/{img}\" alt=\"{id}\">\n<div class=\"head\"><span>{scene} · {number:03}</span><span>{dur} s</span></div>\n<div class=\"camera\">{size}, {angle} · {from}–{to} s</div>\n",
            img = panel_name(index, shot),
            id = escape(&shot.id),
            scene = escape(&shot.scene_id),
            number = shot.number,
            dur = seconds(shot.duration()),
            size = size_label(shot.size),
            angle = escape(angle_label(&shot.angle)),
            from = seconds(shot.start),
            to = seconds(shot.end),
        ));
        for line in &shot.lines {
            let emotion = line.emotion.as_deref().map_or(String::new(), |e| {
                format!(" ({})", escape(&emotion_label(e)))
            });
            html.push_str(&format!(
                "<div class=\"line\"><b>{}</b>{emotion}: “{}”</div>\n",
                escape(&line.actor),
                escape(&line.text)
            ));
        }
        if !shot.actions.is_empty() {
            let actions: Vec<String> = shot
                .actions
                .iter()
                .map(|a| {
                    let value = if a.kind == "emotion" {
                        emotion_label(&a.value)
                    } else {
                        a.value.clone()
                    };
                    format!(
                        "{} {} {}",
                        escape(&a.actor),
                        action_label(a.kind),
                        escape(&value)
                    )
                })
                .collect();
            html.push_str(&format!(
                "<div class=\"actions\">{}</div>\n",
                actions.join(" · ")
            ));
        }
        html.push_str(&format!(
            "<span class=\"status {}\">{}</span>\n",
            status_class(status),
            status.label()
        ));
        if tracked.moved {
            html.push_str(
                "<div class=\"moved\">Otos on siirtynyt merkinnän jälkeen – tarkista tila.</div>\n",
            );
        }
        if let Some(record) = tracked.record.filter(|r| !r.note.is_empty()) {
            html.push_str(&format!(
                "<div class=\"note\">{}</div>\n",
                escape(&record.note)
            ));
        }
        html.push_str("</div>\n");
    }
    html.push_str("</div>\n</body></html>\n");
    html
}

fn shots_json(shots: &[ShotInfo], production: &ProductionDocument) -> serde_json::Value {
    let tracked = production.track(shots);
    serde_json::json!({
        "schema_version": 1,
        "shots": tracked.iter().enumerate().map(|(index, t)| {
            let s = t.shot;
            serde_json::json!({
                "id": s.id, "scene": s.scene_id, "number": s.number,
                "start": s.start, "end": s.end, "panel": s.panel,
                "size": s.size.name(), "zoom": s.zoom, "angle": s.angle,
                "image": format!("panels/{}", panel_name(index, s)),
                "lines": s.lines.iter().map(|l| serde_json::json!({
                    "at": l.at, "actor": l.actor, "text": l.text, "emotion": l.emotion,
                })).collect::<Vec<_>>(),
                "actions": s.actions.iter().map(|a| serde_json::json!({
                    "at": a.at, "actor": a.actor, "kind": a.kind, "value": a.value,
                })).collect::<Vec<_>>(),
                "status": t.record.map_or("storyboard", |r| r.status.name()),
                "note": t.record.map_or("", |r| r.note.as_str()),
                "moved": t.moved,
            })
        }).collect::<Vec<_>>(),
    })
}

/// Write the storyboard of a prepared sequence into the new directory
/// `destination`. `panel_side` (1–1024) is the longest side of each panel.
#[allow(clippy::too_many_arguments)]
pub fn export_storyboard(
    preview: &SequencePreview,
    destination: &Path,
    title: &str,
    production: &ProductionDocument,
    panel_side: u32,
    captions: CaptionMode,
    cancel: &CancellationToken,
    mut progress: impl FnMut(Progress),
) -> Result<StoryboardReport, Error> {
    check(cancel)?;
    let script = preview.script();
    let shots = motion_script::shot_list(script).map_err(|e| invalid(e.to_string()))?;
    if shots.is_empty() || shots.len() > MAX_STORYBOARD_PANELS {
        return Err(invalid(format!(
            "a storyboard holds 1..{MAX_STORYBOARD_PANELS} shots; this sequence has {}",
            shots.len()
        )));
    }
    production.validate().map_err(|e| invalid(e.to_string()))?;
    if destination.exists() {
        return Err(invalid("destination already exists"));
    }
    let timeline = preview.timeline();
    let to_sample = |seconds: f64| {
        motion_scene::time::seconds_to_sample(seconds).map_err(|e| invalid(e.to_string()))
    };
    let mut samples = Vec::with_capacity(shots.len());
    for shot in &shots {
        let span = timeline
            .spans()
            .get(shot.scene_index)
            .ok_or_else(|| invalid("shot outside the sequence"))?;
        let last = span.start_sample + span.duration_samples.saturating_sub(1);
        samples.push((span.start_sample + to_sample(shot.panel)?).min(last));
    }
    fs::create_dir(destination)?;
    let result = (|| {
        let panels = destination.join("panels");
        fs::create_dir(&panels)?;
        let total = shots.len() as u64;
        for (index, (shot, sample)) in shots.iter().zip(&samples).enumerate() {
            check(cancel)?;
            let frame = preview.render_frame_captions(*sample, panel_side, cancel, captions)?;
            let size = resvg::tiny_skia::IntSize::from_wh(frame.width, frame.height)
                .ok_or_else(|| invalid("empty panel"))?;
            let pixmap = resvg::tiny_skia::Pixmap::from_vec(frame.rgba, size)
                .ok_or_else(|| invalid("panel pixels do not match its size"))?;
            let png = pixmap.encode_png().map_err(|e| invalid(e.to_string()))?;
            fs::write(panels.join(panel_name(index, shot)), png)?;
            progress(Progress {
                completed_frames: index as u64 + 1,
                total_frames: total,
            });
        }
        let length = timeline.total_samples() as f64 / f64::from(SAMPLE_RATE);
        fs::write(
            destination.join("storyboard.html"),
            sheet(title, &shots, production, length),
        )?;
        fs::write(
            destination.join("storyboard.json"),
            serde_json::to_vec_pretty(&shots_json(&shots, production))?,
        )?;
        Ok(StoryboardReport {
            shots: shots.len(),
            output_directory: destination.to_path_buf(),
        })
    })();
    discard_partial(destination, result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn preview(source: &str) -> SequencePreview {
        let script = motion_script::parse(source).unwrap();
        let rendered: Vec<_> = script
            .scenes
            .iter()
            .map(|s| motion_voice::export::render_scene(&s.id, s.duration, &[]).unwrap())
            .collect();
        SequencePreview::new(
            Arc::new(script),
            Arc::new(rendered),
            None,
            &CancellationToken::new(),
        )
        .unwrap()
    }
    fn destination() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "kilsat-storyboard-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
    const SOURCE: &str = "project: {width: 320, height: 180, fps: 24}\ncharacters: {}\nscenes: [{id: kahvila, duration: 4, events: [], composition: {background_color: '#ff0000', background_keys: [{at: 2, color: '#0000ff'}], camera: [{at: 0, x: 0, y: 0, zoom: 1}, {at: 1.999, x: 0, y: 0, zoom: 1}, {at: 2, x: 0, y: 0, zoom: 2.4}]}}, {id: 'toimisto <2>', duration: 1, events: []}]\n";

    #[test]
    fn writes_a_panel_per_shot_with_status_and_escaped_text() {
        let preview = preview(SOURCE);
        let shots = motion_script::shot_list(preview.script()).unwrap();
        let mut production = ProductionDocument::default();
        production
            .set(
                &shots[1],
                ShotStatus::Retake,
                "Kamera <liian> lähellä & nopea",
            )
            .unwrap();
        let out = destination();
        let mut calls = 0;
        let report = export_storyboard(
            &preview,
            &out,
            "Kahvitauko",
            &production,
            160,
            CaptionMode::Follow,
            &CancellationToken::new(),
            |_| calls += 1,
        )
        .unwrap();
        assert_eq!((report.shots, calls), (3, 3));
        let html = fs::read_to_string(out.join("storyboard.html")).unwrap();
        assert!(html.contains("3 otosta"));
        assert!(html.contains("kahvila · 020"));
        assert!(html.contains("Lähikuva, etukuva"));
        assert!(html.contains("Kamera &lt;liian&gt; lähellä &amp; nopea"));
        assert!(html.contains("toimisto &lt;2&gt; · 010"));
        assert!(html.contains("class=\"status retake\">Korjattava"));
        assert!(!html.contains("<2>"));
        assert_eq!(emotion_label("sad 0.4"), "surullinen 0,4");
        assert_eq!(emotion_label("happy"), "iloinen");
        let json: serde_json::Value =
            serde_json::from_slice(&fs::read(out.join("storyboard.json")).unwrap()).unwrap();
        assert_eq!(json["shots"][1]["status"], "retake");
        assert_eq!(json["shots"][0]["size"], "wide");
        // Panels: shot 010 is red (its middle, 1 s), shot 020 blue (3 s).
        let pixel = |name: &str| {
            let png = fs::read(out.join(name)).unwrap();
            let pixmap = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
            assert_eq!(pixmap.width(), 160);
            let p = pixmap.pixel(80, 45).unwrap();
            (p.red(), p.green(), p.blue())
        };
        let first = json["shots"][0]["image"].as_str().unwrap();
        let second = json["shots"][1]["image"].as_str().unwrap();
        let third = json["shots"][2]["image"].as_str().unwrap();
        assert_eq!(third, "panels/0002-toimisto--2--010.png");
        assert!(matches!(pixel(first), (r, _, b) if r > 200 && b < 50));
        assert!(matches!(pixel(second), (r, _, b) if b > 200 && r < 50));
        fs::remove_dir_all(out).unwrap();
    }

    #[test]
    fn existing_destinations_and_cancelled_exports_leave_nothing_behind() {
        let preview = preview(SOURCE);
        let out = destination();
        fs::create_dir(&out).unwrap();
        let production = ProductionDocument::default();
        let run = |out: &Path, cancel: &CancellationToken| {
            export_storyboard(
                &preview,
                out,
                "x",
                &production,
                64,
                CaptionMode::Follow,
                cancel,
                |_| {},
            )
        };
        assert!(run(&out, &CancellationToken::new()).is_err());
        assert!(out.is_dir());
        fs::remove_dir(&out).unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(run(&out, &cancel), Err(Error::Cancelled)));
        assert!(!out.exists());
    }
}
