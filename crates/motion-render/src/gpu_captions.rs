//! Native bounded glyph contours. Shaping is CPU-side; pixels are composed by GPU.
//! Uses the same font, wrap, baseline and letterbox as the SVG caption path.
use crate::{
    captions::{self, CaptionCue},
    scene_commands::{
        Command, Error, SceneCommands, MAX_COMMANDS, MAX_FRAGMENT_WORK, MAX_GPU_POINTS,
    },
};
use motion_scene::EvaluatedScene;
use motion_script::CaptionStyle;

const MAX_GLYPHS: usize = 512;
const MAX_CUES: usize = 128;
const FLATNESS: f64 = 0.15;
type Point = [f32; 2];
fn invalid(message: impl Into<String>) -> Error {
    crate::Error::Caption(message.into()).into()
}
fn parse_color(value: &str) -> Result<[f32; 4], Error> {
    if value.len() != 7
        || !value.starts_with('#')
        || !value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
    {
        return Err(invalid("GPU caption colour must be #rrggbb"));
    }
    Ok([
        u8::from_str_radix(&value[1..3], 16).unwrap() as f32 / 255.0,
        u8::from_str_radix(&value[3..5], 16).unwrap() as f32 / 255.0,
        u8::from_str_radix(&value[5..7], 16).unwrap() as f32 / 255.0,
        1.0,
    ])
}

/// Append native caption glyphs before calling `crossfade` on complete frames.
/// Disabled/silent captions add no commands. Failure is atomic. Existing actor
/// labels still need the separate explicit omission acknowledgement.
pub fn append_captions(
    commands: &mut SceneCommands,
    scene: &EvaluatedScene,
    style: &CaptionStyle,
    cues: &[CaptionCue],
) -> Result<(), Error> {
    crate::validate_scene(scene, commands.width, commands.height)?;
    if !style.enabled {
        return Ok(());
    }
    for (value, lo, hi) in [
        (style.font_size, 4.0, 1024.0),
        (style.outline_width, 0.0, 64.0),
        (style.bottom_margin, 0.0, 0.5),
        (style.max_width, 0.1, 1.0),
        (style.line_height, 0.5, 4.0),
    ] {
        if !value.is_finite() || !(lo..=hi).contains(&value) {
            return Err(invalid("invalid GPU caption style range"));
        }
    }
    if cues.len() > MAX_CUES
        || cues.iter().any(|c| {
            !c.start.is_finite() || !c.end.is_finite() || c.start < 0.0 || c.end <= c.start
        })
    {
        return Err(invalid("invalid or excessive GPU caption intervals"));
    }
    if commands.commands.iter().any(|c| c.0[4][0] == 14.0) {
        return Err(Error::UnsupportedFeature(
            "append captions to each scene before complete-frame crossfade",
        ));
    }
    let mut active: Vec<_> = cues
        .iter()
        .filter(|c| c.start <= scene.time && scene.time < c.end)
        .collect();
    active.sort_by(|a, b| a.start.total_cmp(&b.start));
    if active.is_empty() {
        return Ok(());
    }
    let (sw, sh) = (f64::from(scene.width), f64::from(scene.height));
    let scale = (f64::from(commands.width) / sw).min(f64::from(commands.height) / sh);
    let (ox, oy) = (
        (f64::from(commands.width) - sw * scale) * 0.5,
        (f64::from(commands.height) - sh * scale) * 0.5,
    );
    let expected = [
        ox as f32,
        oy as f32,
        (sw * scale) as f32,
        (sh * scale) as f32,
    ];
    if expected != commands.canvas || commands.opacity != scene.opacity as f32 {
        return Err(invalid(
            "caption scene differs from command canvas or opacity",
        ));
    }
    let font = captions::caption_font_data();
    let face = ttf_parser::Face::parse(font, 0)
        .map_err(|_| invalid("bundled GPU caption font is invalid"))?;
    let shaping = rustybuzz::Face::from_slice(font, 0)
        .ok_or_else(|| invalid("bundled caption shaping face is invalid"))?;
    let unit = style.font_size * scale / f64::from(face.units_per_em());
    let descent = -f64::from(face.descender()) / f64::from(face.units_per_em()) * style.font_size;
    let pitch = style.font_size * style.line_height;
    let mut staged = Staged::default();
    let mut bottom = sh * (1.0 - style.bottom_margin);
    let outline = parse_color(&style.outline)?;
    let mut glyph_count = 0usize;
    for cue in active.iter().rev() {
        let lines = captions::wrap_caption(style, &cue.text, style.max_width * sw)?;
        if lines.is_empty() {
            continue;
        }
        let top = bottom - lines.len() as f64 * pitch;
        if top < 0.0 {
            return Err(invalid("captions do not fit vertically inside the frame"));
        }
        let fill = parse_color(style.fill_for(&cue.speaker))?;
        for (index, line) in lines.iter().enumerate() {
            let baseline = bottom - descent - (lines.len() - 1 - index) as f64 * pitch;
            let mut buffer = rustybuzz::UnicodeBuffer::new();
            buffer.push_str(line);
            buffer.guess_segment_properties();
            // All bundled-font caption scripts are horizontal; reject unsupported
            // vertical/RTL layout rather than silently draw an incorrect caption.
            if buffer.direction() != rustybuzz::Direction::LeftToRight {
                return Err(Error::UnsupportedFeature(
                    "GPU caption layout currently requires left-to-right text",
                ));
            }
            let shaped = rustybuzz::shape(&shaping, &[], buffer);
            glyph_count = glyph_count.checked_add(shaped.len()).ok_or(Error::Budget)?;
            if glyph_count > MAX_GLYPHS {
                return Err(Error::Budget);
            }
            let total: i64 = shaped
                .glyph_positions()
                .iter()
                .map(|p| i64::from(p.x_advance))
                .sum();
            let mut x = ox + sw * scale * 0.5 - total as f64 * unit * 0.5;
            let y = oy + baseline * scale;
            let mut glyphs = Vec::new();
            for (info, position) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
                let mut builder = Contours::new(
                    x + f64::from(position.x_offset) * unit,
                    y - f64::from(position.y_offset) * unit,
                    unit,
                );
                let glyph = u16::try_from(info.glyph_id)
                    .map_err(|_| invalid("caption glyph id exceeds font range"))?;
                let _ = face.outline_glyph(ttf_parser::GlyphId(glyph), &mut builder);
                builder.finish();
                if builder.overflow {
                    return Err(Error::Budget);
                }
                if !builder.contours.is_empty() {
                    glyphs.push(staged.store_glyph(builder.contours)?);
                }
                x += f64::from(position.x_advance) * unit;
            }
            // SVG paint-order=stroke applies to a complete line, not alternating
            // glyph outline/fill pairs, which could paint over tightly kerned text.
            if style.outline_width > 0.0 {
                for glyph in &glyphs {
                    staged.paint_glyph(
                        glyph,
                        style.outline_width * scale,
                        outline,
                        commands.width,
                        commands.height,
                    )?;
                }
            }
            for glyph in &glyphs {
                staged.paint_glyph(glyph, 0.0, fill, commands.width, commands.height)?;
            }
        }
        bottom = top;
    }
    if staged.commands.is_empty() {
        return Ok(());
    }
    if let Some(background) = &style.background {
        let color = parse_color(background)?;
        let top = (bottom - style.font_size * 0.25 - style.outline_width).max(0.0);
        staged.commands.insert(
            0,
            Command([
                [(1.0 / scale) as f32, 0.0, 0.0, (1.0 / scale) as f32],
                [
                    -(ox / scale) as f32,
                    -(oy / scale) as f32,
                    (0.75 / scale) as f32,
                    0.0,
                ],
                [
                    (sw / 2.0) as f32,
                    ((sh + top) / 2.0) as f32,
                    (sw / 2.0) as f32,
                    ((sh - top) / 2.0) as f32,
                ],
                color,
                [1.0, 0.0, 0.0, 0.0],
            ]),
        );
    }
    let base_work = crate::scene_commands::command_work(&commands.commands)
        .checked_add(staged.commands.len() as u64)
        .and_then(|n| n.checked_mul(u64::from(commands.width) * u64::from(commands.height)))
        .ok_or(Error::Budget)?;
    let contour_work = commands
        .caption_work
        .checked_add(staged.work)
        .ok_or(Error::Budget)?;
    if commands.points.len() + staged.points.len() > MAX_GPU_POINTS
        || commands.commands.len() + staged.commands.len() > MAX_COMMANDS
        || base_work
            .checked_add(contour_work)
            .is_none_or(|n| n > MAX_FRAGMENT_WORK)
    {
        return Err(Error::Budget);
    }
    let offset = commands.points.len() as f32;
    for mut command in staged.commands {
        if command.0[4][0] == 25.0 {
            command.0[1][0] += offset;
        }
        commands.commands.push(command);
    }
    commands.points.extend(staged.points);
    commands.caption_work = contour_work;
    Ok(())
}

#[derive(Default)]
struct Staged {
    points: Vec<Point>,
    commands: Vec<Command>,
    work: u64,
}
struct Glyph {
    offset: usize,
    count: usize,
    vertices: usize,
    bounds: [f32; 4],
}
impl Staged {
    fn store_glyph(&mut self, contours: Vec<Vec<Point>>) -> Result<Glyph, Error> {
        let count = contours.len();
        let offset = self.points.len();
        let mut vertices = 0usize;
        let mut bounds = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        for contour in contours {
            vertices += contour.len();
            self.points.push([contour.len() as f32, 0.0]);
            for point in contour {
                bounds[0] = bounds[0].min(point[0]);
                bounds[1] = bounds[1].min(point[1]);
                bounds[2] = bounds[2].max(point[0]);
                bounds[3] = bounds[3].max(point[1]);
                self.points.push(point);
            }
        }
        if self.points.len() > MAX_GPU_POINTS || !bounds.iter().all(|v| v.is_finite()) {
            return Err(Error::Budget);
        }
        Ok(Glyph {
            offset,
            count,
            vertices,
            bounds,
        })
    }
    fn paint_glyph(
        &mut self,
        glyph: &Glyph,
        radius: f64,
        color: [f32; 4],
        width: u32,
        height: u32,
    ) -> Result<(), Error> {
        let pad = radius + 1.0;
        let [x0, y0, x1, y1] = glyph.bounds.map(f64::from);
        let area = ((x1 + pad).min(f64::from(width)) - (x0 - pad).max(0.0))
            .max(0.0)
            .ceil()
            * ((y1 + pad).min(f64::from(height)) - (y0 - pad).max(0.0))
                .max(0.0)
                .ceil();
        self.work = self
            .work
            .checked_add(
                (area as u64)
                    .checked_mul((glyph.vertices + glyph.count) as u64)
                    .ok_or(Error::Budget)?,
            )
            .ok_or(Error::Budget)?;
        if self.commands.len() >= MAX_COMMANDS || self.work > MAX_FRAGMENT_WORK {
            return Err(Error::Budget);
        }
        self.commands.push(Command([
            glyph.bounds,
            [glyph.offset as f32, glyph.count as f32, 0.0, 0.0],
            [0.0; 4],
            color,
            [25.0, radius as f32, 0.0, 0.0],
        ]));
        Ok(())
    }
}

struct Contours {
    x: f64,
    y: f64,
    scale: f64,
    contours: Vec<Vec<Point>>,
    current: Vec<Point>,
    total: usize,
    overflow: bool,
}
impl Contours {
    fn new(x: f64, y: f64, scale: f64) -> Self {
        Self {
            x,
            y,
            scale,
            contours: Vec::new(),
            current: Vec::new(),
            total: 0,
            overflow: false,
        }
    }
    fn transform(&self, x: f32, y: f32) -> [f64; 2] {
        [
            self.x + f64::from(x) * self.scale,
            self.y - f64::from(y) * self.scale,
        ]
    }
    fn push(&mut self, p: [f64; 2]) {
        if self.total >= MAX_GPU_POINTS || !p.iter().all(|v| v.is_finite()) {
            self.overflow = true;
            return;
        }
        let p = p.map(|v| v as f32);
        if self.current.last() != Some(&p) {
            self.current.push(p);
            self.total += 1;
        }
    }
    fn finish(&mut self) {
        if self.current.first() == self.current.last() && self.current.len() > 1 {
            self.current.pop();
        }
        if self.current.len() >= 3 {
            self.contours.push(std::mem::take(&mut self.current));
        } else {
            self.current.clear();
        }
    }
    fn last(&self) -> [f64; 2] {
        self.current
            .last()
            .copied()
            .unwrap_or([0.0; 2])
            .map(f64::from)
    }
    fn quad(&mut self, a: [f64; 2], b: [f64; 2], c: [f64; 2], depth: u8) {
        if flat(a, c, b) <= FLATNESS {
            self.push(c);
            return;
        }
        if depth >= 8 {
            self.overflow = true;
            return;
        }
        let ab = mid(a, b);
        let bc = mid(b, c);
        let centre = mid(ab, bc);
        self.quad(a, ab, centre, depth + 1);
        self.quad(centre, bc, c, depth + 1);
    }
    fn cubic(&mut self, a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2], depth: u8) {
        if flat(a, d, b).max(flat(a, d, c)) <= FLATNESS {
            self.push(d);
            return;
        }
        if depth >= 8 {
            self.overflow = true;
            return;
        }
        let ab = mid(a, b);
        let bc = mid(b, c);
        let cd = mid(c, d);
        let abc = mid(ab, bc);
        let bcd = mid(bc, cd);
        let centre = mid(abc, bcd);
        self.cubic(a, ab, abc, centre, depth + 1);
        self.cubic(centre, bcd, cd, d, depth + 1);
    }
}
fn mid(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
}
fn flat(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let length = dx.hypot(dy);
    if length < 1e-10 {
        (p[0] - a[0]).hypot(p[1] - a[1])
    } else {
        ((p[0] - a[0]) * dy - (p[1] - a[1]) * dx).abs() / length
    }
}
impl ttf_parser::OutlineBuilder for Contours {
    fn move_to(&mut self, x: f32, y: f32) {
        self.finish();
        self.push(self.transform(x, y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.push(self.transform(x, y));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.quad(self.last(), self.transform(x1, y1), self.transform(x, y), 0);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.cubic(
            self.last(),
            self.transform(x1, y1),
            self.transform(x2, y2),
            self.transform(x, y),
            0,
        );
    }
    fn close(&mut self) {
        self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scene() -> EvaluatedScene {
        let script=motion_script::parse("project: {width: 320,height: 180}\ncharacters: {}\nscenes: [{id: s,duration: 2,events: []}]\n").unwrap();
        motion_scene::evaluate(&script, 0, 0.5).unwrap()
    }
    fn cue(text: &str) -> CaptionCue {
        CaptionCue {
            speaker: "A".into(),
            text: text.into(),
            start: 0.0,
            end: 1.0,
            estimated: false,
        }
    }
    #[test]
    fn glyph_holes_outlines_half_open_timing_and_crossfade_offsets_are_native() {
        let scene = scene();
        let style = CaptionStyle {
            font_size: 28.0,
            outline_width: 2.0,
            ..Default::default()
        };
        let mut commands = crate::scene_commands::build(
            &scene,
            320,
            180,
            &[],
            crate::scene_commands::Labels::OmitAcknowledged,
        )
        .unwrap();
        append_captions(&mut commands, &scene, &style, &[cue("OBO AV")]).unwrap();
        let glyphs: Vec<_> = commands
            .commands
            .iter()
            .filter(|c| c.0[4][0] == 25.0)
            .collect();
        assert!(
            glyphs.iter().any(|g| g.0[1][1] >= 2.0),
            "O/B must retain multiple contours, including holes"
        );
        assert!(glyphs.iter().any(|g| g.0[4][1] > 0.0));
        assert!(
            commands.pixels.is_empty(),
            "no rendered caption image is uploaded"
        );
        let offset = commands.points.len() as f32;
        let mut second = crate::scene_commands::build(
            &scene,
            320,
            180,
            &[],
            crate::scene_commands::Labels::OmitAcknowledged,
        )
        .unwrap();
        append_captions(&mut second, &scene, &style, &[cue("O")]).unwrap();
        let before = second
            .commands
            .iter()
            .find(|c| c.0[4][0] == 25.0)
            .unwrap()
            .0[1][0];
        let first_count = commands.commands.len();
        let mixed = crate::scene_commands::crossfade(commands, second, 0.5).unwrap();
        let shifted = mixed.commands[first_count + 1..]
            .iter()
            .find(|c| c.0[4][0] == 25.0)
            .unwrap();
        assert_eq!(shifted.0[1][0], before + offset);
        let mut late = scene.clone();
        late.time = 1.0;
        let mut no_caption = crate::scene_commands::build(
            &late,
            320,
            180,
            &[],
            crate::scene_commands::Labels::OmitAcknowledged,
        )
        .unwrap();
        append_captions(&mut no_caption, &late, &style, &[cue("O")]).unwrap();
        assert_eq!(no_caption.command_count(), 0);
    }
    #[test]
    fn invalid_glyphs_styles_and_work_limits_leave_commands_unchanged() {
        let scene = scene();
        let mut commands = crate::scene_commands::build(
            &scene,
            320,
            180,
            &[],
            crate::scene_commands::Labels::OmitAcknowledged,
        )
        .unwrap();
        let style = CaptionStyle {
            font_size: 28.0,
            ..Default::default()
        };
        assert!(append_captions(&mut commands, &scene, &style, &[cue("😀")]).is_err());
        assert!(commands.points.is_empty() && commands.commands.is_empty());
        let style = CaptionStyle {
            font_size: f64::NAN,
            ..style
        };
        assert!(append_captions(&mut commands, &scene, &style, &[cue("A")]).is_err());
        assert_eq!(commands.command_count(), 0);
    }
}
