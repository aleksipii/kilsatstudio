//! Koeta Studio product name and logo.

use eframe::egui;
use std::sync::Arc;

pub const APP_NAME: &str = "Koeta Studio";

/// The Koeta logo, downscaled from `assets/brand/Koeta.png`.
const LOGO_PNG: &[u8] = include_bytes!("../assets/koeta-logo-512.png");

/// Decoded logo for the window and Dock icon. `None` only if the embedded PNG is invalid.
pub fn window_icon() -> Option<Arc<egui::IconData>> {
    eframe::icon_data::from_png_bytes(LOGO_PNG)
        .ok()
        .map(Arc::new)
}

/// Logo texture for the toolbar. Decoded and uploaded once per egui context.
pub fn logo_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::new("koeta_brand_logo");
    if let Some(texture) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(id)) {
        return Some(texture);
    }
    let icon = window_icon()?;
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [icon.width as usize, icon.height as usize],
        &icon.rgba,
    );
    let texture = ctx.load_texture("koeta-logo", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    Some(texture)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_logo_decodes_as_square_rgba() {
        let icon = window_icon().expect("embedded logo must be a valid PNG");
        assert_eq!((icon.width, icon.height), (512, 512));
        assert_eq!(icon.rgba.len(), 512 * 512 * 4);
    }

    #[test]
    fn logo_texture_is_cached_per_context() {
        let ctx = egui::Context::default();
        let first = logo_texture(&ctx).expect("logo texture");
        let second = logo_texture(&ctx).expect("cached logo texture");
        assert_eq!(first.id(), second.id());
    }
}
