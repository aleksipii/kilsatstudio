//! Single bounded decode worker; results carry an exact project inventory signature.
use eframe::egui;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{mpsc, Arc},
};
type DecodeResult = (
    String,
    Result<BTreeMap<String, motion_art::ImageResource>, String>,
);
pub struct Resources {
    tx: mpsc::SyncSender<(String, PathBuf, Vec<motion_project::AssetReference>)>,
    rx: mpsc::Receiver<DecodeResult>,
    pub images: Arc<BTreeMap<String, motion_art::ImageResource>>,
    pub textures: BTreeMap<String, egui::TextureHandle>,
    key: String,
    pending: bool,
    pub error: Option<String>,
}
impl Default for Resources {
    fn default() -> Self {
        let (tx, jobs) =
            mpsc::sync_channel::<(String, PathBuf, Vec<motion_project::AssetReference>)>(1);
        let (results, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            while let Ok((key, root, assets)) = jobs.recv() {
                let result = (|| {
                    let mut images = BTreeMap::new();
                    let mut bytes = 0usize;
                    let root = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
                    for asset in assets
                        .iter()
                        .filter(|a| a.kind == motion_project::AssetKind::Image)
                    {
                        let path = std::fs::canonicalize(root.join(&asset.path))
                            .map_err(|e| e.to_string())?;
                        if !path.starts_with(&root) {
                            return Err("Image escapes project root".into());
                        }
                        let image = motion_art::load_image(&path)
                            .and_then(|i| i.reserve_in(motion_jobs::process_budget()))
                            .map_err(|e| e.to_string())?;
                        bytes += image.rgba().len() + image.png().len();
                        if bytes > 128 * 1024 * 1024 {
                            return Err("Image resources exceed 128 MiB budget".into());
                        }
                        images.insert(asset.id.clone(), image);
                    }
                    Ok(images)
                })();
                if results.send((key, result)).is_err() {
                    break;
                }
            }
        });
        Self {
            tx,
            rx,
            images: Arc::new(BTreeMap::new()),
            textures: BTreeMap::new(),
            key: String::new(),
            pending: false,
            error: None,
        }
    }
}
impl Resources {
    pub fn reload(&mut self) {
        self.key.clear();
    }
    pub fn sync(
        &mut self,
        ctx: &egui::Context,
        root: &str,
        assets: &[motion_project::AssetReference],
    ) {
        let key = format!("{}:{:?}", root, assets);
        if let Ok((result_key, result)) = self.rx.try_recv() {
            self.pending = false;
            if result_key == key {
                match result {
                    Ok(images) => {
                        self.textures = images
                            .iter()
                            .map(|(id, image)| {
                                (
                                    id.clone(),
                                    ctx.load_texture(
                                        id,
                                        egui::ColorImage::from_rgba_unmultiplied(
                                            [image.width() as usize, image.height() as usize],
                                            image.rgba(),
                                        ),
                                        egui::TextureOptions::LINEAR,
                                    ),
                                )
                            })
                            .collect();
                        self.images = Arc::new(images);
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error),
                }
            }
        }
        if self.key != key && !self.pending {
            self.images = Arc::new(BTreeMap::new());
            self.textures.clear();
            if self
                .tx
                .try_send((key.clone(), root.into(), assets.to_vec()))
                .is_ok()
            {
                self.key = key;
                self.pending = true;
            }
        }
        if self.pending {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_decodes_and_discards_stale_inventory() {
        let root = std::env::temp_dir().join(format!("art-resource-worker-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let image = motion_art::ImageResource::from_rgba(1, 1, vec![12, 23, 34, 255]).unwrap();
        std::fs::write(root.join("pixel.png"), image.png()).unwrap();
        let assets = vec![motion_project::AssetReference {
            id: "pixel".into(),
            path: "pixel.png".into(),
            kind: motion_project::AssetKind::Image,
        }];
        let mut resources = Resources::default();
        let ctx = egui::Context::default();
        resources.sync(&ctx, root.to_str().unwrap(), &assets);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while resources.pending && std::time::Instant::now() < deadline {
            std::thread::yield_now();
            resources.sync(&ctx, root.to_str().unwrap(), &assets);
        }
        assert!(resources.error.is_none());
        assert_eq!(resources.images["pixel"].rgba(), [12, 23, 34, 255]);
        assert!(resources.textures.contains_key("pixel"));
        resources.reload();
        resources.sync(&ctx, root.to_str().unwrap(), &assets);
        resources.sync(&ctx, root.to_str().unwrap(), &[]);
        while (resources.pending || !resources.images.is_empty())
            && std::time::Instant::now() < deadline
        {
            std::thread::yield_now();
            resources.sync(&ctx, root.to_str().unwrap(), &[]);
        }
        assert!(resources.images.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
