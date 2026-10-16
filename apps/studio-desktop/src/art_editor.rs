//! Artwork edits expose one transaction to the shared project history.
use eframe::egui;
use motion_art::{
    BlendMode, ClipShape, Color, Content, Document, Layer, Plane, Point, Rect, TimeRange,
};
use std::collections::BTreeMap;
pub struct SceneContext<'a> {
    pub id: &'a str,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
}
#[derive(Default, PartialEq, Clone, Copy)]
pub enum Tool {
    #[default]
    Select,
    Pen,
    Rectangle,
    Ellipse,
}
#[derive(Default)]
pub struct Editor {
    pub tool: Tool,
    pub selected: Option<String>,
    pub dirty: bool,
    revision: u64,
    points: Vec<Point>,
    gesture_scene: String,
    transaction_before: Option<Document>,
    pub error: Option<String>,
    context: String,
}
fn parse_mask_points(text: &str) -> Result<Vec<Point>, String> {
    if text.len() > 16_384 {
        return Err("Mask vertices exceed 16 KiB".into());
    }
    let mut points = Vec::new();
    for pair in text.split_whitespace() {
        if points.len() >= motion_art::MAX_MASK_POINTS {
            return Err("Use at most 256 vertices".into());
        }
        let (x, y) = pair
            .split_once(',')
            .ok_or("Use whitespace-separated x,y vertices")?;
        points.push(Point {
            x: x.parse().map_err(|_| "Invalid x coordinate")?,
            y: y.parse().map_err(|_| "Invalid y coordinate")?,
        });
    }
    ClipShape::Polygon {
        points: points.clone(),
    }
    .validate()
    .map_err(|e| e.to_string())?;
    Ok(points)
}
fn compositing_controls(ui: &mut egui::Ui, layer: &mut Layer) {
    egui::ComboBox::from_label("Blend")
        .selected_text(format!("{:?}", layer.blend))
        .show_ui(ui, |ui| {
            for mode in [BlendMode::Normal, BlendMode::Multiply, BlendMode::Screen] {
                ui.selectable_value(&mut layer.blend, mode, format!("{mode:?}"));
            }
        });
    let mut kind = match &layer.clip_shape {
        Some(ClipShape::Ellipse { .. }) => 2,
        Some(ClipShape::Polygon { .. }) => 3,
        None => {
            if layer.clip.is_some() {
                1
            } else {
                0
            }
        }
    };
    let previous = kind;
    egui::ComboBox::from_label("Clip mask")
        .selected_text(["None", "Rectangle", "Ellipse", "Polygon"][kind])
        .show_ui(ui, |ui| {
            for (value, label) in ["None", "Rectangle", "Ellipse", "Polygon"]
                .into_iter()
                .enumerate()
            {
                ui.selectable_value(&mut kind, value, label);
            }
        });
    if kind != previous {
        let rect = layer
            .clip
            .or(match &layer.clip_shape {
                Some(ClipShape::Ellipse { rect }) => Some(*rect),
                _ => None,
            })
            .unwrap_or(Rect {
                x: 0.0,
                y: 0.0,
                width: 300.0,
                height: 300.0,
            });
        layer.clip = None;
        layer.clip_shape = None;
        match kind {
            1 => layer.clip = Some(rect),
            2 => layer.clip_shape = Some(ClipShape::Ellipse { rect }),
            3 => {
                layer.clip_shape = Some(ClipShape::Polygon {
                    points: vec![
                        Point {
                            x: rect.x,
                            y: rect.y,
                        },
                        Point {
                            x: rect.x + rect.width,
                            y: rect.y,
                        },
                        Point {
                            x: rect.x + rect.width,
                            y: rect.y + rect.height,
                        },
                        Point {
                            x: rect.x,
                            y: rect.y + rect.height,
                        },
                    ],
                })
            }
            _ => {}
        }
    }
    let rect = match &mut layer.clip_shape {
        Some(ClipShape::Ellipse { rect }) => Some(rect),
        _ => layer.clip.as_mut(),
    };
    if let Some(rect) = rect {
        ui.horizontal(|ui| {
            for (name, value) in [
                ("X", &mut rect.x),
                ("Y", &mut rect.y),
                ("W", &mut rect.width),
                ("H", &mut rect.height),
            ] {
                ui.label(name);
                ui.add(egui::DragValue::new(value).speed(1.0));
            }
        });
    }
    if let Some(ClipShape::Polygon { points }) = &mut layer.clip_shape {
        let id = ui.id().with(("mask-vertices", &layer.id));
        let initial = points
            .iter()
            .map(|p| format!("{},{}", p.x, p.y))
            .collect::<Vec<_>>()
            .join(" ");
        let mut draft = ui
            .data_mut(|d| d.get_temp::<String>(id))
            .unwrap_or(initial.clone());
        ui.label(
            "Vertices: x,y pairs separated by spaces (3–256). Do not repeat the first vertex.",
        );
        ui.add(
            egui::TextEdit::multiline(&mut draft)
                .desired_rows(3)
                .char_limit(16_384),
        );
        if ui.button("Apply polygon vertices").clicked() {
            match parse_mask_points(&draft) {
                Ok(next) => {
                    *points = next;
                }
                Err(error) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
            }
        }
        ui.data_mut(|d| d.insert_temp(id, draft));
    }
    if layer.clip_shape.is_some() || layer.blend != BlendMode::Normal {
        ui.weak("Inspect this mask/blend in Whole project preview or PNG/MP4 export. Selected-scene canvas parity is pending.");
    }
}

impl Editor {
    /// Increments only when document content changes, including history restoration.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn clear_history_for_context(&mut self, context: String) {
        if self.context != context {
            self.transaction_before = None;
            self.points.clear();
            self.selected = None;
            self.context = context;
        }
    }
    pub fn take_transaction_before(&mut self) -> Option<Document> {
        self.transaction_before.take()
    }
    pub fn reset(&mut self) {
        let revision = self.revision.wrapping_add(1);
        *self = Self::default();
        self.revision = revision;
    }
    pub(crate) fn commit(&mut self, document: &mut Document, candidate: Document) {
        if *document == candidate {
            return;
        }
        if let Err(error) = candidate.validate() {
            self.error = Some(error.to_string());
            return;
        }
        self.error = None;
        if self.transaction_before.is_none() {
            self.transaction_before = Some(document.clone());
        }
        *document = candidate;
        self.dirty = true;
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        document: &mut Document,
        scene: SceneContext<'_>,
        assets: &[motion_project::AssetReference],
        actors: &[String],
        textures: &BTreeMap<String, egui::TextureHandle>,
    ) {
        let scene_id = scene.id;
        egui::CollapsingHeader::new("Artwork / layers").show(ui,|ui| {
   ui.horizontal_wrapped(|ui| { for (tool,name) in [(Tool::Select,"Select"),(Tool::Pen,"Pen"),(Tool::Rectangle,"Rectangle"),(Tool::Ellipse,"Ellipse")] {if ui.selectable_value(&mut self.tool,tool,name).changed(){self.points.clear();}} });
   ui.weak("Drag on canvas to draw. Background layers render behind characters; foreground layers render above them. Save as .kilsat to retain artwork.");
   let mut next=document.clone();
   for layer in next.layers.iter_mut().filter(|l|l.scene_id==scene_id) { ui.horizontal(|ui| {ui.checkbox(&mut layer.visible,"");if ui.selectable_label(self.selected.as_deref()==Some(&layer.id),&layer.name).clicked(){self.selected=Some(layer.id.clone());}}); }
   if let Some(index)=next.layers.iter().position(|l|l.scene_id==scene_id && Some(&l.id)==self.selected.as_ref()) {
    let layer=&mut next.layers[index];
    ui.text_edit_singleline(&mut layer.name);
    egui::ComboBox::from_label("Plane").selected_text(match layer.plane {Plane::Background=>"Background",Plane::Foreground=>"Foreground"}).show_ui(ui,|ui|{ui.selectable_value(&mut layer.plane,Plane::Background,"Background");ui.selectable_value(&mut layer.plane,Plane::Foreground,"Foreground");});
    let mut timed=layer.active.is_some();
    if ui.checkbox(&mut timed,"Visible during interval").changed(){layer.active=timed.then_some(TimeRange{start:0.0,end:scene.duration});}
    if let Some(active)=&mut layer.active {ui.horizontal(|ui|{ui.label("Start (s)");ui.add(egui::DragValue::new(&mut active.start).speed(0.1));ui.label("End (s)");ui.add(egui::DragValue::new(&mut active.end).speed(0.1));});ui.weak(format!("Scene: {:.2} s. Start is included; end is excluded.",scene.duration));if !active.start.is_finite() || !active.end.is_finite() || active.start<0.0 || active.end<=active.start || active.end>scene.duration {ui.colored_label(egui::Color32::LIGHT_RED,"Use 0 ≤ start < end ≤ scene duration.");}}

    ui.add(egui::Slider::new(&mut layer.opacity,0.0..=1.0).text("Opacity"));
    if let Content::Image{rect,..}|Content::Rectangle{rect,..}|Content::Ellipse{rect,..}=&mut layer.content {ui.horizontal(|ui|{for(name,value)in[("X",&mut rect.x),("Y",&mut rect.y),("W",&mut rect.width),("H",&mut rect.height)]{ui.label(name);ui.add(egui::DragValue::new(value).speed(1.0));}});rect.width=rect.width.max(1.0);rect.height=rect.height.max(1.0);}
    egui::ComboBox::from_label("Follow actor translation").selected_text(layer.attachment.as_deref().unwrap_or("World")).show_ui(ui,|ui| {ui.selectable_value(&mut layer.attachment,None,"World");for actor in actors {ui.selectable_value(&mut layer.attachment,Some(actor.clone()),actor);}});
    compositing_controls(ui, layer);
    ui.horizontal(|ui| {if ui.button("Lower").clicked() && index>0 {next.layers.swap(index,index-1);}
   if ui.button("Raise").clicked() && index+1<next.layers.len(){next.layers.swap(index,index+1);}
   if ui.button("Delete layer").clicked(){next.layers.remove(index);self.selected=None;}});
   }
   let images: Vec<_> = assets.iter().filter(|a| a.kind == motion_project::AssetKind::Image).collect();
   if !images.is_empty() {
    ui.label(format!("Image assets ({})", images.len()));
    egui::ScrollArea::vertical().id_salt("art-image-inventory").max_height(256.0).show_rows(ui, 76.0, images.len(), |ui, rows| {
     for row in rows {let asset = images[row];
      ui.push_id(&asset.id, |ui| {ui.horizontal(|ui| {
       asset_thumbnail(ui, textures.get(&asset.id));
       ui.vertical(|ui| {ui.add(egui::Label::new(&asset.id).truncate()).on_hover_text(&asset.path);
        ui.vertical(|ui| {for (plane,label) in [(Plane::Foreground,"Place foreground"),(Plane::Background,"Place background")] {if ui.button(label).clicked(){let id=self.id(&next);self.selected=Some(id.clone());next.layers.push(Layer{id,name:asset.id.clone(),scene_id:scene_id.into(),visible:true,plane,active:None,opacity:1.0,attachment:None,clip_shape: None, blend: motion_art::BlendMode::Normal, clip:None,content:Content::Image{asset_id:asset.id.clone(),rect:if plane==Plane::Background {Rect{x:0.0,y:0.0,width:scene.width as f64,height:scene.height as f64}}else{Rect{x:50.0,y:50.0,width:250.0,height:250.0}}}});}}});
       });
      });});
     }
    });
   }
   if next.layers.iter().any(|l| l.scene_id==scene_id && l.active.as_ref().is_some_and(|a|a.end>scene.duration)) {self.error=Some("Artwork interval exceeds scene duration".into());} else {
   self.commit(document,next);
   }
   ui.weak("Undo / Redo uses the shared project history in the toolbar.");
   if self.dirty {ui.label("Artwork has unsaved changes");}
   if let Some(error)=&self.error{ui.colored_label(egui::Color32::LIGHT_RED,error);}
  });
    }
    fn id(&self, document: &Document) -> String {
        (1..)
            .map(|i| format!("art-{i}"))
            .find(|id| !document.layers.iter().any(|l| &l.id == id))
            .unwrap()
    }
    pub fn gesture(
        &mut self,
        response: &egui::Response,
        rect: egui::Rect,
        scene: &motion_scene::EvaluatedScene,
        scene_id: &str,
        document: &mut Document,
    ) {
        if self.tool == Tool::Select {
            return;
        }
        if response.drag_started() {
            self.points.clear();
            self.gesture_scene = scene_id.into();
        }
        if self.gesture_scene != scene_id {
            self.points.clear();
            return;
        }
        if let Some(pos) = response
            .interact_pointer_pos()
            .filter(|_| response.dragged() || response.drag_stopped())
        {
            let scale =
                (rect.width() / scene.width as f32).min(rect.height() / scene.height as f32);
            let origin =
                rect.center() - egui::vec2(scene.width as f32, scene.height as f32) * scale * 0.5;
            let x = (pos.x - origin.x) as f64 / scale as f64;
            let y = (pos.y - origin.y) as f64 / scale as f64;
            let point = Point {
                x: (x - scene.width as f64 / 2.0) / scene.camera.zoom
                    + scene.width as f64 / 2.0
                    + scene.camera.x,
                y: (y - scene.height as f64 / 2.0) / scene.camera.zoom
                    + scene.height as f64 / 2.0
                    + scene.camera.y,
            };
            if self.points.len() < 2048 {
                self.points.push(point);
            }
        }
        if response.drag_stopped() && self.points.len() >= 2 {
            let first = &self.points[0];
            let last = self.points.last().unwrap();
            let bounds = Rect {
                x: first.x.min(last.x),
                y: first.y.min(last.y),
                width: (first.x - last.x).abs().max(1.0),
                height: (first.y - last.y).abs().max(1.0),
            };
            let color = Color {
                r: 90,
                g: 200,
                b: 160,
                a: 255,
            };
            let content = match self.tool {
                Tool::Pen => Content::Stroke {
                    points: self.points.clone(),
                    color,
                    width: 4.0,
                },
                Tool::Rectangle => Content::Rectangle {
                    rect: bounds,
                    color,
                },
                Tool::Ellipse => Content::Ellipse {
                    rect: bounds,
                    color,
                },
                Tool::Select => return,
            };
            let mut next = document.clone();
            let id = self.id(&next);
            self.selected = Some(id.clone());
            next.layers.push(Layer {
                id,
                name: "Drawing".into(),
                scene_id: scene_id.into(),
                visible: true,
                plane: Plane::Foreground,
                active: None,
                opacity: 1.0,
                attachment: None,
                clip_shape: None,
                blend: motion_art::BlendMode::Normal,
                clip: None,
                content,
            });
            self.commit(document, next);
            self.points.clear();
        }
    }
}

/// Display only an already decoded native texture; never read files from a frame.
fn asset_thumbnail(ui: &mut egui::Ui, texture: Option<&egui::TextureHandle>) {
    let bounds = egui::vec2(64.0, 64.0);
    let (rect, response) = ui.allocate_exact_size(bounds, egui::Sense::hover());
    if let Some(texture) = texture {
        let size = thumbnail_size(texture.size(), bounds);
        let image_rect = egui::Rect::from_center_size(rect.center(), size);
        ui.painter().image(
            texture.id(),
            image_rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        response.on_hover_text("Decoded image preview");
    } else {
        ui.painter()
            .rect_filled(rect, 3.0, ui.visuals().faint_bg_color);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No preview",
            egui::FontId::proportional(10.0),
            ui.visuals().weak_text_color(),
        );
        response.on_hover_text("Preview pending or unavailable. Check resource loading errors; this does not confirm the image loaded.");
    }
}

fn thumbnail_size(size: [usize; 2], bounds: egui::Vec2) -> egui::Vec2 {
    let source = egui::vec2(size[0].max(1) as f32, size[1].max(1) as f32);
    source * (bounds.x / source.x).min(bounds.y / source.y)
}

pub fn paint(
    painter: &egui::Painter,
    canvas: egui::Rect,
    scene: &motion_scene::EvaluatedScene,
    scene_id: &str,
    document: &Document,
    resources: (
        &std::collections::BTreeMap<String, egui::TextureHandle>,
        Plane,
    ),
) {
    let (textures, plane) = resources;
    let scale = (canvas.width() / scene.width as f32).min(canvas.height() / scene.height as f32);
    let origin =
        canvas.center() - egui::vec2(scene.width as f32, scene.height as f32) * scale * 0.5;
    for layer in document
        .layers
        .iter()
        .filter(|l| l.is_active(scene.time) && l.scene_id == scene_id && l.plane == plane)
    {
        let offset = if let Some(id) = &layer.attachment {
            let Some(actor) = scene.actors.iter().find(|a| &a.id == id) else {
                continue;
            };
            (actor.x, actor.y)
        } else {
            (0.0, 0.0)
        };
        let point = |x: f64, y: f64| {
            let (x, y) =
                scene
                    .camera
                    .project(x + offset.0, y + offset.1, scene.width, scene.height);
            origin + egui::vec2(x as f32, y as f32) * scale
        };
        let bounds = |r: &Rect| {
            egui::Rect::from_two_pos(point(r.x, r.y), point(r.x + r.width, r.y + r.height))
        };
        let mut painter = painter.clone();
        painter.set_opacity(layer.opacity as f32);
        if let Some(clip) = &layer.clip {
            painter = painter.with_clip_rect(bounds(clip).intersect(canvas));
        }
        let color = |c: &Color| egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
        match &layer.content {
            Content::Image { asset_id, rect } => {
                if let Some(texture) = textures.get(asset_id) {
                    painter.image(
                        texture.id(),
                        bounds(rect),
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    painter.rect_filled(bounds(rect), 0.0, egui::Color32::DARK_RED);
                }
            }
            Content::Rectangle { rect, color: c } => {
                painter.rect_filled(bounds(rect), 0.0, color(c));
            }
            Content::Ellipse { rect, color: c } => {
                let r = bounds(rect);
                painter.add(egui::epaint::EllipseShape::filled(
                    r.center(),
                    r.size() * 0.5,
                    color(c),
                ));
            }
            Content::Stroke {
                points,
                color: c,
                width,
            } => {
                painter.add(egui::Shape::line(
                    points.iter().map(|p| point(p.x, p.y)).collect(),
                    egui::Stroke::new(
                        (*width as f32 * scale * scene.camera.zoom as f32).max(0.1),
                        color(c),
                    ),
                ));
            }
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn thumbnail_preserves_landscape_and_portrait_aspect() {
        assert_eq!(
            thumbnail_size([400, 200], egui::vec2(64.0, 64.0)),
            egui::vec2(64.0, 32.0)
        );
        assert_eq!(
            thumbnail_size([200, 400], egui::vec2(64.0, 64.0)),
            egui::vec2(32.0, 64.0)
        );
    }

    #[test]
    fn asset_inventory_rendering_preserves_document_with_present_or_missing_preview() {
        let context = egui::Context::default();
        let texture = context.load_texture(
            "synthetic-asset",
            egui::ColorImage::new([2, 1], egui::Color32::RED),
            Default::default(),
        );
        let mut textures = BTreeMap::new();
        textures.insert("present".into(), texture);
        let assets: Vec<_> = ["present", "missing"]
            .into_iter()
            .map(|id| motion_project::AssetReference {
                id: id.into(),
                path: format!("{id}.png"),
                kind: motion_project::AssetKind::Image,
            })
            .collect();
        let mut editor = Editor::default();
        let mut document = Document::default();
        let before = document.clone();
        let output = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                // Force the collapsible inventory open without simulating a click.
                ui.memory_mut(|memory| memory.set_everything_is_visible(true));
                editor.ui(
                    ui,
                    &mut document,
                    SceneContext {
                        id: "scene",
                        duration: 2.0,
                        width: 100,
                        height: 100,
                    },
                    &assets,
                    &[],
                    &textures,
                );
            });
        });
        assert_eq!(document, before);
        assert!(!editor.dirty);
        assert!(editor.transaction_before.is_none());
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Mesh(mesh) if mesh.texture_id == textures["present"].id())));
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "No preview")));
    }

    use super::*;
    #[test]
    fn rectangle_gesture_commits_once_in_camera_world_coordinates() {
        let ctx = egui::Context::default();
        let mut editor = Editor {
            tool: Tool::Rectangle,
            ..Default::default()
        };
        let mut document = Document::default();
        let scene = motion_scene::EvaluatedScene {
            time: 0.0,
            camera: motion_scene::CameraPose {
                x: 20.0,
                y: 10.0,
                zoom: 2.0,
            },
            background_color: "#000000".into(),
            opacity: 1.0,
            width: 300,
            height: 200,
            actors: vec![],
        };
        for events in [
            vec![],
            vec![
                egui::Event::PointerMoved(egui::pos2(20.0, 20.0)),
                egui::Event::PointerButton {
                    pos: egui::pos2(20.0, 20.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
            vec![egui::Event::PointerMoved(egui::pos2(60.0, 60.0))],
            vec![
                egui::Event::PointerMoved(egui::pos2(100.0, 100.0)),
                egui::Event::PointerButton {
                    pos: egui::pos2(100.0, 100.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ],
        ] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400.0, 300.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let rect =
                            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 200.0));
                        let response =
                            ui.interact(rect, egui::Id::new("gesture"), egui::Sense::drag());
                        editor.gesture(&response, rect, &scene, "scene", &mut document);
                    });
                },
            );
        }
        assert_eq!(document.layers.len(), 1);
        assert!(editor.transaction_before.is_some());
        let Content::Rectangle { rect, .. } = document.layers[0].content else {
            panic!("rectangle expected")
        };
        assert_eq!(rect.x, 105.0);
        assert_eq!(rect.y, 70.0);
        assert_eq!(rect.width, 40.0);
        assert_eq!(rect.height, 40.0);
    }
    #[test]
    fn timed_artwork_changes_are_atomic_and_expose_the_before_state() {
        let mut editor = Editor::default();
        let mut document = Document::default();
        let layer = Layer {
            id: "timed".into(),
            scene_id: "scene".into(),
            name: "Background".into(),
            visible: true,
            plane: Plane::Background,
            active: Some(TimeRange {
                start: 1.0,
                end: 2.0,
            }),
            opacity: 1.0,
            attachment: None,
            clip_shape: None,
            blend: motion_art::BlendMode::Normal,
            clip: None,
            content: Content::Rectangle {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 20.0,
                    height: 20.0,
                },
                color: Color {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 255,
                },
            },
        };
        editor.commit(
            &mut document,
            Document {
                layers: vec![layer],
            },
        );
        let before = document.clone();
        let mut invalid = before.clone();
        invalid.layers[0].active.as_mut().unwrap().end = 1.0;
        editor.commit(&mut document, invalid);
        assert_eq!(document, before);
        assert!(editor.transaction_before.is_some());
        assert!(editor.error.is_some());
        let mut next = before.clone();
        next.layers[0].plane = Plane::Foreground;
        next.layers[0].active = None;
        editor.commit(&mut document, next);
        // The host consumes the initial transaction before a later edit.
        assert!(editor.take_transaction_before().is_some());
        assert_ne!(document, before);
    }

    #[test]
    fn advanced_polygon_edits_are_detached_validated_and_undoable() {
        let points = parse_mask_points("0,0 20,0 20,20 0,20").unwrap();
        assert!(parse_mask_points("0,0 20,20 0,20 20,0").is_err());
        assert!(parse_mask_points("NaN,0 1,0 0,1").is_err());
        let mut document = Document {
            layers: vec![Layer {
                id: "mask".into(),
                scene_id: "s".into(),
                name: "Mask".into(),
                visible: true,
                plane: Plane::Foreground,
                active: None,
                opacity: 1.0,
                attachment: None,
                clip: None,
                clip_shape: None,
                blend: BlendMode::Normal,
                content: Content::Rectangle {
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 20.0,
                        height: 20.0,
                    },
                    color: Color {
                        r: 1,
                        g: 2,
                        b: 3,
                        a: 255,
                    },
                },
            }],
        };
        let before = document.clone();
        let mut editor = Editor::default();
        let mut next = before.clone();
        next.layers[0].clip_shape = Some(ClipShape::Polygon { points });
        next.layers[0].blend = BlendMode::Screen;
        editor.commit(&mut document, next);
        assert_ne!(document, before);
        let valid = document.clone();
        let mut invalid = valid.clone();
        invalid.layers[0].clip = Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        });
        editor.commit(&mut document, invalid);
        assert_eq!(document, valid);
        // Rejected edits preserve the one before-state delivered to the shared
        // project history; the artwork editor no longer owns an undo stack.
        assert_eq!(editor.take_transaction_before(), Some(before));
        assert!(editor.take_transaction_before().is_none());
        assert!(editor.error.is_some());
    }

    #[test]
    fn native_painter_filters_plane_and_half_open_interval() {
        let mut document = Document::default();
        for (id, plane, r) in [
            ("back", Plane::Background, 13),
            ("front", Plane::Foreground, 27),
        ] {
            document.layers.push(Layer {
                id: id.into(),
                scene_id: "s".into(),
                name: id.into(),
                visible: true,
                plane,
                active: Some(TimeRange {
                    start: 1.0,
                    end: 2.0,
                }),
                opacity: 1.0,
                attachment: None,
                clip_shape: None,
                blend: motion_art::BlendMode::Normal,
                clip: None,
                content: Content::Rectangle {
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 10.0,
                        height: 10.0,
                    },
                    color: Color {
                        r,
                        g: 0,
                        b: 0,
                        a: 255,
                    },
                },
            });
        }
        let ctx = egui::Context::default();
        for (time, plane, expected) in [
            (0.999, Plane::Background, vec![]),
            (1.0, Plane::Background, vec![13]),
            (1.5, Plane::Foreground, vec![27]),
            (2.0, Plane::Background, vec![]),
        ] {
            let scene = motion_scene::EvaluatedScene {
                time,
                camera: motion_scene::CameraPose {
                    x: 0.0,
                    y: 0.0,
                    zoom: 1.0,
                },
                background_color: "#000000".into(),
                opacity: 0.25,
                width: 100,
                height: 100,
                actors: vec![],
            };
            let output = ctx.run(Default::default(), |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Middle,
                    egui::Id::new("paint"),
                ));
                paint(
                    &painter,
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0)),
                    &scene,
                    "s",
                    &document,
                    (&Default::default(), plane),
                );
            });
            let colors: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|s| {
                    if let egui::Shape::Rect(r) = &s.shape {
                        Some(r.fill.r())
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(colors, expected, "time {time}, plane {plane:?}");
        }
    }

    #[test]
    fn artwork_transactions_are_atomic_and_only_keep_the_first_before_state() {
        let mut e = Editor::default();
        let mut d = Document::default();
        for i in 0..30 {
            let mut next = d.clone();
            next.layers.push(Layer {
                id: format!("l{i}"),
                scene_id: "s".into(),
                name: "Layer".into(),
                visible: true,
                plane: Plane::Foreground,
                active: None,
                opacity: 1.0,
                attachment: None,
                clip_shape: None,
                blend: motion_art::BlendMode::Normal,
                clip: None,
                content: Content::Rectangle {
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 10.0,
                        height: 10.0,
                    },
                    color: Color {
                        r: 1,
                        g: 2,
                        b: 3,
                        a: 255,
                    },
                },
            });
            e.commit(&mut d, next);
        }
        assert_eq!(e.transaction_before.as_ref().unwrap(), &Document::default());
        assert_eq!(e.revision(), 30);
        let unchanged = d.clone();
        e.commit(&mut d, unchanged);
        assert_eq!(e.revision(), 30);
        let before = d.clone();
        let mut invalid = d.clone();
        invalid.layers[0].opacity = f64::NAN;
        e.commit(&mut d, invalid);
        assert_eq!(d, before);
        assert!(e.error.is_some());
        assert_eq!(e.revision(), 30);
        e.reset();
        assert_eq!(e.revision(), 31);
    }
}
