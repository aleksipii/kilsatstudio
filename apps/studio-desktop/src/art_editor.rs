//! Artwork edits expose one transaction to the shared project history.
use eframe::egui;
use motion_art::{
    BlendMode, ClipShape, Color, Content, Document, Layer, MaskKey, MaskStroke, Plane, Point, Rect,
    TimeRange,
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
    MaskPaint,
    MaskErase,
}
struct MaskGesture {
    layer: Layer,
    scene: String,
    tool: Tool,
    camera: (f64, f64, f64, f64),
    viewport: egui::Rect,
    scene_size: (u32, u32),
    points: Vec<Point>,
    radius: f64,
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
    group_draft: Option<motion_art::Group>,
    move_parent: String,
    move_index: usize,
    mask_radius: f64,
    mask_gesture: Option<MaskGesture>,
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
        feather: 0.0,
    }
    .validate()
    .map_err(|e| e.to_string())?;
    Ok(points)
}
fn parse_mask_keys(text: &str, duration: f64) -> Result<Vec<MaskKey>, String> {
    if text.len() > 24_576 {
        return Err("Mask keys exceed 24 KiB".into());
    }
    let mut keys = Vec::new();
    for row in text.split_whitespace() {
        if keys.len() >= motion_art::MAX_MASK_KEYS {
            return Err("Use at most 128 mask keys".into());
        }
        let values: Vec<f64> = row
            .split(',')
            .map(|value| value.parse().map_err(|_| "Invalid mask key number"))
            .collect::<Result<_, _>>()?;
        if values.len() != 6 {
            return Err("Use time,x,y,width,height,feather for each key".into());
        }
        keys.push(MaskKey {
            time: values[0],
            rect: Rect {
                x: values[1],
                y: values[2],
                width: values[3],
                height: values[4],
            },
            feather: values[5],
        });
    }
    ClipShape::SoftEllipse {
        rect: Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        feather: 0.0,
        keys: keys.clone(),
    }
    .validate()
    .map_err(|error| error.to_string())?;
    if !duration.is_finite()
        || duration <= 0.0
        || keys.last().is_some_and(|key| key.time > duration)
    {
        return Err("Mask keys must fit within the scene duration".into());
    }
    Ok(keys)
}
fn compositing_controls(ui: &mut egui::Ui, layer: &mut Layer, duration: f64, images: &[&str]) {
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
        Some(ClipShape::SoftEllipse { .. }) => 4,
        Some(ClipShape::Image { .. }) => 5,
        Some(ClipShape::Painted { .. }) => 6,
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
        .selected_text(
            [
                "None",
                "Rectangle",
                "Ellipse",
                "Polygon",
                "Soft / animated ellipse",
                "Image (alpha / luminance)",
                "Painted (vector brush / eraser)",
            ][kind],
        )
        .show_ui(ui, |ui| {
            for (value, label) in [
                "None",
                "Rectangle",
                "Ellipse",
                "Polygon",
                "Soft / animated ellipse",
                "Image (alpha / luminance)",
                "Painted (vector brush / eraser)",
            ]
            .into_iter()
            .enumerate()
            {
                if value == 5 && images.is_empty() {
                    ui.add_enabled(
                        false,
                        egui::Label::new("Image mask (import an image first)"),
                    );
                    continue;
                }
                ui.selectable_value(&mut kind, value, label);
            }
        });
    if kind != previous {
        let rect = layer
            .clip
            .or(match &layer.clip_shape {
                Some(
                    ClipShape::Ellipse { rect }
                    | ClipShape::SoftEllipse { rect, .. }
                    | ClipShape::Image { rect, .. },
                ) => Some(*rect),
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
            6 => {
                layer.clip_shape = Some(ClipShape::Painted {
                    base_visible: true,
                    strokes: Vec::new(),
                })
            }
            1 => layer.clip = Some(rect),
            2 => layer.clip_shape = Some(ClipShape::Ellipse { rect }),
            5 => {
                layer.clip_shape = Some(ClipShape::Image {
                    asset_id: images[0].to_string(),
                    rect,
                    channel: motion_art::MaskChannel::Alpha,
                })
            }
            4 => {
                layer.clip_shape = Some(ClipShape::SoftEllipse {
                    rect,
                    feather: 0.25,
                    keys: Vec::new(),
                })
            }
            3 => {
                layer.clip_shape = Some(ClipShape::Polygon {
                    feather: 0.0,
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
        Some(
            ClipShape::Ellipse { rect }
            | ClipShape::SoftEllipse { rect, .. }
            | ClipShape::Image { rect, .. },
        ) => Some(rect),
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
    if let Some(ClipShape::Painted {
        base_visible,
        strokes,
    }) = &mut layer.clip_shape
    {
        ui.checkbox(
            base_visible,
            "Initially show all content (unchecked: hide all)",
        );
        ui.label(format!("{} ordered brush / eraser strokes", strokes.len()));
        ui.weak("Choose Mask brush or Mask eraser, then drag or click on the canvas. Brush reveals; eraser hides. Each completed stroke is one Undo step. Changing the mask type replaces its strokes. CPU preview/export only.");
    }
    if let Some(ClipShape::Image {
        asset_id, channel, ..
    }) = &mut layer.clip_shape
    {
        egui::ComboBox::from_label("Mask image")
            .selected_text(asset_id.as_str())
            .show_ui(ui, |ui| {
                for id in images {
                    ui.selectable_value(asset_id, id.to_string(), *id);
                }
            });
        egui::ComboBox::from_label("Mask channel")
            .selected_text(format!("{channel:?}"))
            .show_ui(ui, |ui| {
                for value in [
                    motion_art::MaskChannel::Alpha,
                    motion_art::MaskChannel::Luminance,
                ] {
                    ui.selectable_value(channel, value, format!("{value:?}"));
                }
            });
        ui.weak("The image is stretched over X/Y/W/H. Alpha: opaque pixels show content. Luminance: bright pixels show content. CPU preview/export only.");
    }
    if let Some(ClipShape::Polygon { points, feather }) = &mut layer.clip_shape {
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
        ui.add(
            egui::Slider::new(feather, 0.0..=motion_art::MAX_POLYGON_FEATHER)
                .logarithmic(true)
                .text("Feather (blur σ, scene units)"),
        );
        ui.weak("0 = hard edge. Positive values blur the boundary; the edge keeps about half coverage. CPU preview/export only; the experimental GPU path reports it as unsupported.");
    }
    if let Some(ClipShape::SoftEllipse { feather, keys, .. }) = &mut layer.clip_shape {
        ui.add(egui::Slider::new(feather, 0.0..=1.0).text("Feather (inward)"));
        ui.weak("0 = hard edge, 1 = fade from center. The base shape is used when no keys exist.");
        ui.label("Animation keys: time,x,y,width,height,feather (scene seconds). Linear interpolation; first/last key holds.");
        let id = ui.id().with(("soft-mask-keys", &layer.id));
        let error_id = id.with("error");
        let initial = keys
            .iter()
            .map(|key| {
                format!(
                    "{},{},{},{},{},{}",
                    key.time, key.rect.x, key.rect.y, key.rect.width, key.rect.height, key.feather
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut draft = ui
            .data_mut(|data| data.get_temp::<String>(id))
            .unwrap_or(initial);
        ui.add(
            egui::TextEdit::multiline(&mut draft)
                .desired_rows(4)
                .char_limit(24_576),
        );
        if ui.button("Apply mask animation keys").clicked() {
            match parse_mask_keys(&draft, duration) {
                Ok(next) => {
                    *keys = next;
                    ui.data_mut(|data| data.remove::<String>(error_id));
                }
                Err(error) => ui.data_mut(|data| data.insert_temp(error_id, error)),
            }
        }
        if let Some(error) = ui.data_mut(|data| data.get_temp::<String>(error_id)) {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        ui.data_mut(|data| data.insert_temp(id, draft));
    }
    if layer.clip_shape.is_some() || layer.blend != BlendMode::Normal {
        ui.weak("Inspect this mask/blend in the shared scene preview, Whole project preview or PNG/MP4 export.");
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
            self.mask_gesture = None;
            self.selected = None;
            self.group_draft = None;
            self.move_parent.clear();
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
    pub(crate) fn commit(&mut self, document: &mut Document, mut candidate: Document) {
        if !candidate.groups.is_empty() || !candidate.order.is_empty() {
            for layer in &candidate.layers {
                if !document.layers.iter().any(|old| old.id == layer.id) {
                    let unit = motion_art::ArtUnit::Layer(layer.id.clone());
                    if !candidate.order.contains(&unit)
                        && !candidate.groups.iter().any(|g| g.children.contains(&unit))
                    {
                        candidate.order.push(unit);
                    }
                }
            }
        }
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
   ui.horizontal_wrapped(|ui| { for (tool,name) in [(Tool::Select,"Select"),(Tool::Pen,"Pen"),(Tool::Rectangle,"Rectangle"),(Tool::Ellipse,"Ellipse"),(Tool::MaskPaint,"Mask brush"),(Tool::MaskErase,"Mask eraser")] {if ui.selectable_value(&mut self.tool,tool,name).changed(){self.points.clear();self.mask_gesture=None;}} });
   if matches!(self.tool, Tool::MaskPaint | Tool::MaskErase) {
    if self.mask_radius <= 0.0 { self.mask_radius = 12.0; }
    ui.add(egui::Slider::new(&mut self.mask_radius, 0.5..=1000.0).logarithmic(true).text("Mask brush radius (scene units)"));
    ui.weak("Select a layer and explicitly choose its Painted clip mask first. Escape cancels the current stroke. Fixed radius; pressure is not recorded.");
   }
   ui.weak("Drag on canvas to draw. Background layers render behind characters; foreground layers render above them. Save as .kilsat to retain artwork.");
   self.group_controls(ui, document, scene_id);
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
    let image_ids: Vec<&str> = assets.iter().filter(|a| a.kind == motion_project::AssetKind::Image).map(|a| a.id.as_str()).collect();
    compositing_controls(ui, layer, scene.duration, &image_ids);
    ui.horizontal(|ui| {if ui.add_enabled(index>0 && next.groups.is_empty() && next.order.is_empty(),egui::Button::new("Lower")).clicked() { next.layers.swap(index,index-1); }
   if ui.add_enabled(index+1<next.layers.len() && next.groups.is_empty() && next.order.is_empty(),egui::Button::new("Raise")).clicked(){ next.layers.swap(index,index+1); }
   if ui.button("Delete layer").clicked(){let id=next.layers[index].id.clone(); if let Err(error)=next.apply(motion_art::Command::Remove{id}) {self.error=Some(error.to_string());} self.selected=None;}});
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
    fn group_controls(&mut self, ui: &mut egui::Ui, document: &mut Document, scene_id: &str) {
        use motion_art::{ArtUnit, Command, Group};
        ui.collapsing("Isolated groups / hierarchy", |ui| {
            if ui.button("Create foreground group").clicked() {
                let id = (1..).map(|n| format!("group-{n}")).find(|id| !document.layers.iter().any(|l| &l.id == id) && !document.groups.iter().any(|g| &g.id == id)).unwrap();
                let mut candidate = document.clone();
                let group = Group { id: id.clone(), scene_id: scene_id.into(), plane: Plane::Foreground, name: "Group".into(), visible: true, opacity: 1.0, blend: BlendMode::Normal, children: Vec::new() };
                match candidate.apply(Command::CreateGroup(group.clone())) {
                    Ok(()) => { self.commit(document, candidate); self.selected = Some(id); self.group_draft = Some(group); }
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
            for plane in [Plane::Background, Plane::Foreground] {
                if let Ok(steps) = document.composition(scene_id, plane, 0.0) {
                    let mut depth: usize = 0;
                    for step in steps {
                        match step {
                            motion_art::CompositionStep::EnterGroup(group) => {
                                if ui.selectable_label(self.selected.as_deref() == Some(&group.id), format!("{}▣ {}", "  ".repeat(depth), group.name)).clicked() {
                                    self.selected = Some(group.id.clone()); self.group_draft = Some(group.clone());
                                }
                                depth += 1;
                            }
                            motion_art::CompositionStep::LeaveGroup => depth = depth.saturating_sub(1),
                            motion_art::CompositionStep::Layer(layer) => {
                                if ui.selectable_label(self.selected.as_deref() == Some(&layer.id), format!("{}{}", "  ".repeat(depth), layer.name)).clicked() { self.selected = Some(layer.id.clone()); self.group_draft = None; }
                            }
                        }
                    }
                }
            }
            // Hidden groups remain selectable so visibility can always be restored.
            for group in document.groups.iter().filter(|g| g.scene_id == scene_id && !g.visible) {
                if ui.selectable_label(self.selected.as_deref() == Some(&group.id), format!("Hidden group: {}", group.name)).clicked() { self.selected = Some(group.id.clone()); self.group_draft = Some(group.clone()); }
            }
            let mut command = None;
            if let Some(draft) = &mut self.group_draft {
                ui.text_edit_singleline(&mut draft.name);
                ui.checkbox(&mut draft.visible, "Visible subtree");
                ui.add(egui::Slider::new(&mut draft.opacity, 0.0..=1.0).text("Isolated opacity"));
                egui::ComboBox::from_label("Group plane").selected_text(format!("{:?}", draft.plane)).show_ui(ui, |ui| {
                    ui.selectable_value(&mut draft.plane, Plane::Background, "Background");
                    ui.selectable_value(&mut draft.plane, Plane::Foreground, "Foreground");
                });
                egui::ComboBox::from_label("Group blend").selected_text(format!("{:?}", draft.blend)).show_ui(ui, |ui| {
                    for mode in [BlendMode::Normal, BlendMode::Multiply, BlendMode::Screen] { ui.selectable_value(&mut draft.blend, mode, format!("{mode:?}")); }
                });
                if ui.button("Apply group properties").clicked() { command = Some(Command::ReplaceGroup(draft.clone())); }
                ui.weak("Ungroup keeps all children in order; removing opacity or blend can change appearance.");
                if ui.button("Ungroup, keep children").clicked() { command = Some(Command::Ungroup { id: draft.id.clone() }); }
            }
            if let Some(id) = &self.selected {
                let unit = if document.groups.iter().any(|g| &g.id == id) { ArtUnit::Group(id.clone()) } else { ArtUnit::Layer(id.clone()) };
                ui.label("Move selected item: parent ID (empty = root), sibling index");
                ui.text_edit_singleline(&mut self.move_parent);
                ui.add(egui::DragValue::new(&mut self.move_index).range(0..=motion_art::MAX_LAYERS));
                if ui.button("Apply hierarchy move").clicked() {
                    command = Some(Command::Move { unit, parent: (!self.move_parent.is_empty()).then(|| self.move_parent.clone()), index: self.move_index });
                }
            }
            if let Some(command) = command {
                let ungroup = matches!(&command, Command::Ungroup { .. });
                let mut candidate = document.clone();
                match candidate.apply(command) {
                    Ok(()) => { self.commit(document, candidate); if ungroup { self.selected = None; self.group_draft = None; } }
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        });
    }
    fn id(&self, document: &Document) -> String {
        (1..)
            .map(|i| format!("art-{i}"))
            .find(|id| !document.layers.iter().any(|l| &l.id == id))
            .unwrap()
    }
    fn finish_mask_gesture(&mut self, document: &mut Document) {
        let Some(draft) = self.mask_gesture.take() else {
            return;
        };
        let Some(index) = document
            .layers
            .iter()
            .position(|layer| layer == &draft.layer)
        else {
            return;
        };
        if draft.points.is_empty() {
            return;
        }
        let mut next = document.clone();
        let Some(ClipShape::Painted { strokes, .. }) = &mut next.layers[index].clip_shape else {
            return;
        };
        strokes.push(MaskStroke {
            points: draft.points,
            radius: draft.radius,
            erase: draft.tool == Tool::MaskErase,
        });
        self.commit(document, next);
    }
    fn mask_gesture(
        &mut self,
        response: &egui::Response,
        rect: egui::Rect,
        scene: &motion_scene::EvaluatedScene,
        scene_id: &str,
        document: &mut Document,
    ) {
        let camera = (
            scene.camera.x,
            scene.camera.y,
            scene.camera.zoom,
            scene.time,
        );
        let selected = document
            .layers
            .iter()
            .find(|layer| Some(&layer.id) == self.selected.as_ref() && layer.scene_id == scene_id);
        let cancelled = response
            .ctx
            .input(|input| input.key_pressed(egui::Key::Escape));
        let incompatible = self.mask_gesture.as_ref().is_some_and(|draft| {
            selected != Some(&draft.layer)
                || draft.scene != scene_id
                || draft.tool != self.tool
                || draft.camera != camera
                || draft.viewport != rect
                || draft.scene_size != (scene.width, scene.height)
                || draft.radius != self.mask_radius
        });
        if cancelled || incompatible || !response.enabled() {
            self.mask_gesture = None;
            return;
        }
        let Some(layer) = selected else {
            self.mask_gesture = None;
            return;
        };
        if !matches!(layer.clip_shape, Some(ClipShape::Painted { .. })) {
            self.mask_gesture = None;
            return;
        }
        let (pressed, down, released, position) = response.ctx.input(|input| {
            (
                input.pointer.primary_pressed(),
                input.pointer.primary_down(),
                input.pointer.primary_released(),
                input.pointer.interact_pos(),
            )
        });
        if pressed && response.hovered() {
            self.mask_gesture = Some(MaskGesture {
                layer: layer.clone(),
                scene: scene_id.into(),
                tool: self.tool,
                camera,
                viewport: rect,
                scene_size: (scene.width, scene.height),
                points: Vec::new(),
                radius: self.mask_radius,
            });
        }
        let scale = (rect.width() / scene.width as f32).min(rect.height() / scene.height as f32);
        if !scale.is_finite()
            || scale <= 0.0
            || !scene.camera.zoom.is_finite()
            || scene.camera.zoom <= 0.0
        {
            self.mask_gesture = None;
            return;
        }
        let origin =
            rect.center() - egui::vec2(scene.width as f32, scene.height as f32) * scale * 0.5;
        let offset = match &layer.attachment {
            Some(id) => match scene.actors.iter().find(|actor| &actor.id == id) {
                Some(actor) => (actor.x, actor.y),
                None => {
                    self.mask_gesture = None;
                    return;
                }
            },
            None => (0.0, 0.0),
        };
        if let Some(position) = position {
            if let Some(draft) = self.mask_gesture.as_mut().filter(|_| down || released) {
                let x = (position.x - origin.x) as f64 / scale as f64;
                let y = (position.y - origin.y) as f64 / scale as f64;
                let point = Point {
                    x: (x - scene.width as f64 / 2.0) / scene.camera.zoom
                        + scene.width as f64 / 2.0
                        + scene.camera.x
                        - offset.0,
                    y: (y - scene.height as f64 / 2.0) / scene.camera.zoom
                        + scene.height as f64 / 2.0
                        + scene.camera.y
                        - offset.1,
                };
                if draft.points.last() != Some(&point) {
                    if draft.points.len() >= motion_art::MAX_MASK_STROKE_POINTS {
                        self.mask_gesture = None;
                        self.error =
                            Some("Mask stroke exceeds 2048 points; draw shorter strokes".into());
                        return;
                    }
                    draft.points.push(point);
                }
            }
            if response.hovered() || self.mask_gesture.is_some() {
                let painter = response
                    .ctx
                    .layer_painter(response.layer_id)
                    .with_clip_rect(rect);
                let color = if self.tool == Tool::MaskErase {
                    egui::Color32::LIGHT_RED
                } else {
                    egui::Color32::LIGHT_GREEN
                };
                painter.circle_stroke(
                    position,
                    (self.mask_radius * scene.camera.zoom) as f32 * scale,
                    egui::Stroke::new(1.5, color),
                );
                if let Some(draft) = &self.mask_gesture {
                    for point in &draft.points {
                        let (x, y) = scene.camera.project(
                            point.x + offset.0,
                            point.y + offset.1,
                            scene.width,
                            scene.height,
                        );
                        painter.circle_filled(
                            origin + egui::vec2(x as f32, y as f32) * scale,
                            2.0,
                            color,
                        );
                    }
                }
            }
        }
        if released {
            self.finish_mask_gesture(document);
        } else if !down {
            // Focus loss / pointer cancellation must not leave a draft for a later gesture.
            self.mask_gesture = None;
        }
    }
    pub fn gesture(
        &mut self,
        response: &egui::Response,
        rect: egui::Rect,
        scene: &motion_scene::EvaluatedScene,
        scene_id: &str,
        document: &mut Document,
    ) {
        if matches!(self.tool, Tool::MaskPaint | Tool::MaskErase) {
            self.mask_gesture(response, rect, scene, scene_id, document);
            return;
        }
        self.mask_gesture = None;
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
                Tool::Select | Tool::MaskPaint | Tool::MaskErase => return,
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
    // Grouped scenes must use the shared compositor, never a flattened painter fallback.
    if !document.order.is_empty() || document.groups.iter().any(|g| g.scene_id == scene_id) {
        return;
    }

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
    fn mask_document() -> Document {
        Document {
            layers: vec![Layer {
                id: "mask".into(),
                scene_id: "scene".into(),
                name: "Mask".into(),
                visible: true,
                plane: Plane::Foreground,
                active: None,
                opacity: 1.0,
                attachment: None,
                clip: None,
                clip_shape: Some(ClipShape::Painted {
                    base_visible: false,
                    strokes: Vec::new(),
                }),
                blend: BlendMode::Normal,
                content: Content::Rectangle {
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 300.0,
                        height: 200.0,
                    },
                    color: Color {
                        r: 255,
                        g: 0,
                        b: 0,
                        a: 255,
                    },
                },
            }],
            ..Document::default()
        }
    }
    fn mask_scene() -> motion_scene::EvaluatedScene {
        motion_scene::EvaluatedScene {
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
        }
    }
    fn mask_frame(
        ctx: &egui::Context,
        editor: &mut Editor,
        document: &mut Document,
        scene: &motion_scene::EvaluatedScene,
        events: Vec<egui::Event>,
    ) {
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
                        ui.interact(rect, egui::Id::new("mask-gesture"), egui::Sense::drag());
                    editor.gesture(&response, rect, scene, "scene", document);
                });
            },
        );
    }
    fn mask_button(x: f32, y: f32, pressed: bool) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(egui::pos2(x, y)),
            egui::Event::PointerButton {
                pos: egui::pos2(x, y),
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            },
        ]
    }
    #[test]
    fn painted_mask_gestures_are_detached_and_single_dabs_commit_once() {
        let ctx = egui::Context::default();
        let mut editor = Editor {
            tool: Tool::MaskPaint,
            selected: Some("mask".into()),
            mask_radius: 7.0,
            ..Default::default()
        };
        let mut document = mask_document();
        let before = document.clone();
        let scene = mask_scene();
        mask_frame(&ctx, &mut editor, &mut document, &scene, vec![]);
        mask_frame(
            &ctx,
            &mut editor,
            &mut document,
            &scene,
            mask_button(60.0, 60.0, true),
        );
        assert_eq!(document, before);
        assert_eq!(editor.revision(), 0);
        mask_frame(
            &ctx,
            &mut editor,
            &mut document,
            &scene,
            mask_button(60.0, 60.0, false),
        );
        let Some(ClipShape::Painted { strokes, .. }) = &document.layers[0].clip_shape else {
            panic!("painted mask")
        };
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0].points, vec![Point { x: 125.0, y: 90.0 }]);
        assert_eq!(strokes[0].radius, 7.0);
        assert!(!strokes[0].erase);
        assert_eq!(editor.revision(), 1);
        assert_eq!(editor.take_transaction_before(), Some(before));
        let painted = document.clone();
        editor.tool = Tool::MaskErase;
        mask_frame(
            &ctx,
            &mut editor,
            &mut document,
            &scene,
            mask_button(80.0, 80.0, true),
        );
        mask_frame(
            &ctx,
            &mut editor,
            &mut document,
            &scene,
            vec![egui::Event::PointerMoved(egui::pos2(100.0, 100.0))],
        );
        assert_eq!(document, painted);
        mask_frame(
            &ctx,
            &mut editor,
            &mut document,
            &scene,
            mask_button(110.0, 110.0, false),
        );
        let Some(ClipShape::Painted { strokes, .. }) = &document.layers[0].clip_shape else {
            panic!("painted mask")
        };
        assert_eq!(strokes.len(), 2);
        assert!(strokes[1].erase);
        assert!(strokes[1].points.len() >= 2);
        assert_eq!(editor.take_transaction_before(), Some(painted));
        assert_eq!(editor.revision(), 2);
    }
    #[test]
    fn painted_dab_uses_attachment_local_coordinates() {
        let ctx = egui::Context::default();
        let mut editor = Editor {
            tool: Tool::MaskPaint,
            selected: Some("mask".into()),
            mask_radius: 7.0,
            ..Default::default()
        };
        let mut document = mask_document();
        document.layers[0].attachment = Some("actor".into());
        let mut scene = mask_scene();
        scene.actors.push(motion_scene::ActorPose {
            id: "actor".into(),
            x: 40.0,
            y: 30.0,
            emotion: "neutral".into(),
            rotation: 0.0,
            scale: motion_scene::Vec2::new(1.0, 1.0),
            wave: 0.0,
            skeleton: [motion_scene::Vec2::new(0.0, 0.0); 7],
            rigid_parts: Vec::new(),
        });
        mask_frame(&ctx, &mut editor, &mut document, &scene, vec![]);
        mask_frame(
            &ctx,
            &mut editor,
            &mut document,
            &scene,
            mask_button(60.0, 60.0, true),
        );
        mask_frame(
            &ctx,
            &mut editor,
            &mut document,
            &scene,
            mask_button(60.0, 60.0, false),
        );
        let Some(ClipShape::Painted { strokes, .. }) = &document.layers[0].clip_shape else {
            panic!("painted mask")
        };
        assert_eq!(strokes[0].points, vec![Point { x: 85.0, y: 60.0 }]);
    }
    #[test]
    fn painted_drafts_cancel_on_selection_scene_camera_and_document_changes() {
        for changed in 0..4 {
            let ctx = egui::Context::default();
            let mut editor = Editor {
                tool: Tool::MaskPaint,
                selected: Some("mask".into()),
                mask_radius: 7.0,
                ..Default::default()
            };
            let mut document = mask_document();
            let mut scene = mask_scene();
            mask_frame(&ctx, &mut editor, &mut document, &scene, vec![]);
            mask_frame(
                &ctx,
                &mut editor,
                &mut document,
                &scene,
                mask_button(60.0, 60.0, true),
            );
            assert!(editor.mask_gesture.is_some());
            match changed {
                0 => editor.selected = None,
                1 => scene.time = 0.5,
                2 => scene.camera.zoom = 1.0,
                _ => document.layers[0].attachment = Some("actor".into()),
            }
            let unchanged = document.clone();
            mask_frame(
                &ctx,
                &mut editor,
                &mut document,
                &scene,
                mask_button(80.0, 80.0, false),
            );
            assert_eq!(document, unchanged);
            assert!(editor.mask_gesture.is_none());
            assert!(editor.take_transaction_before().is_none());
        }
    }
    #[test]
    fn painted_stroke_rejection_does_not_replace_other_masks_or_commit_invalid_data() {
        let mut editor = Editor::default();
        let mut document = mask_document();
        let before = document.clone();
        editor.mask_gesture = Some(MaskGesture {
            layer: document.layers[0].clone(),
            scene: "scene".into(),
            tool: Tool::MaskPaint,
            camera: (0.0, 0.0, 1.0, 0.0),
            viewport: egui::Rect::NOTHING,
            scene_size: (300, 200),
            points: vec![Point {
                x: f64::NAN,
                y: 0.0,
            }],
            radius: 5.0,
        });
        editor.finish_mask_gesture(&mut document);
        assert_eq!(document, before);
        assert!(editor.take_transaction_before().is_none());
        assert!(editor.error.is_some());
        document.layers[0].clip_shape = Some(ClipShape::Ellipse {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            },
        });
        let unchanged = document.clone();
        editor.mask_gesture = Some(MaskGesture {
            layer: document.layers[0].clone(),
            scene: "scene".into(),
            tool: Tool::MaskErase,
            camera: (0.0, 0.0, 1.0, 0.0),
            viewport: egui::Rect::NOTHING,
            scene_size: (300, 200),
            points: vec![Point { x: 5.0, y: 5.0 }],
            radius: 5.0,
        });
        editor.finish_mask_gesture(&mut document);
        assert_eq!(document, unchanged);
        assert!(editor.take_transaction_before().is_none());
    }
    #[test]
    fn grouped_transactions_have_one_before_snapshot_and_invalid_apply_is_atomic() {
        let mut editor = Editor::default();
        let mut document = Document::default();
        let before = document.clone();
        let mut candidate = document.clone();
        candidate
            .apply(motion_art::Command::CreateGroup(motion_art::Group {
                id: "group".into(),
                scene_id: "s".into(),
                plane: Plane::Foreground,
                name: "Group".into(),
                visible: true,
                opacity: 0.5,
                blend: BlendMode::Multiply,
                children: Vec::new(),
            }))
            .unwrap();
        editor.commit(&mut document, candidate);
        assert_eq!(editor.take_transaction_before(), Some(before));
        let committed = document.clone();
        let revision = editor.revision();
        let mut invalid = committed.clone();
        invalid.groups[0].opacity = f64::NAN;
        editor.commit(&mut document, invalid);
        assert_eq!(document, committed);
        assert_eq!(editor.revision(), revision);
        assert!(editor.take_transaction_before().is_none());
        assert!(editor.error.is_some());
    }
    #[test]
    fn soft_mask_key_drafts_validate_before_changing_document_history() {
        let keys = parse_mask_keys("0,0,0,20,20,0\n2,10,5,40,30,1", 2.0).unwrap();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[1].feather, 1.0);
        for invalid in [
            "0,0,0,20,20,NaN",
            "0,0,0,20,20,1.1",
            "3,0,0,20,20,0",
            "1,0,0,20,20,0 1,0,0,20,20,1",
            "0,0,0,0,20,0",
            "0,0,0,20,20",
        ] {
            assert!(parse_mask_keys(invalid, 2.0).is_err(), "{invalid}");
        }
        assert!(parse_mask_keys("", 2.0).unwrap().is_empty());
        let mut document = Document::default();
        let mut editor = Editor::default();
        let candidate = Document {
            layers: vec![Layer {
                id: "soft".into(),
                scene_id: "s".into(),
                name: "Soft mask".into(),
                visible: true,
                plane: Plane::Foreground,
                active: None,
                opacity: 1.0,
                attachment: None,
                clip: None,
                blend: BlendMode::Normal,
                clip_shape: Some(ClipShape::SoftEllipse {
                    rect: keys[0].rect,
                    feather: 0.0,
                    keys,
                }),
                content: Content::Rectangle {
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 100.0,
                        height: 100.0,
                    },
                    color: Color {
                        r: 255,
                        g: 0,
                        b: 0,
                        a: 255,
                    },
                },
            }],
            ..Document::default()
        };
        editor.commit(&mut document, candidate.clone());
        assert_eq!(document, candidate);
        assert_eq!(editor.take_transaction_before(), Some(Document::default()));
        let mut invalid = candidate.clone();
        let Some(ClipShape::SoftEllipse { keys, .. }) = &mut invalid.layers[0].clip_shape else {
            panic!("soft mask")
        };
        keys[1].time = 0.0;
        editor.commit(&mut document, invalid);
        assert_eq!(document, candidate);
        assert!(editor.take_transaction_before().is_none());
        assert!(editor.error.is_some());
    }
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
                ..Document::default()
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
            ..Document::default()
        };
        let before = document.clone();
        let mut editor = Editor::default();
        let mut next = before.clone();
        next.layers[0].clip_shape = Some(ClipShape::Polygon {
            points,
            feather: 0.0,
        });
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
