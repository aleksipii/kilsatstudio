use crate::{invalid, Error};
use base64::Engine;
use image::{ImageFormat, ImageReader};
use std::{
    fs::File,
    io::{Cursor, Read},
    path::Path,
    sync::Arc,
};

pub const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_IMAGE_SIDE: u32 = 8192;
pub const MAX_IMAGE_PIXELS: u64 = 16 * 1024 * 1024;
#[derive(Debug, Clone)]
pub struct ImageResource {
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
    png: Arc<[u8]>,
}
impl ImageResource {
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
    pub fn png(&self) -> &[u8] {
        &self.png
    }
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, Error> {
        check_dimensions(width, height)?;
        let expected = u64::from(width) * u64::from(height) * 4;
        if rgba.len() as u64 != expected {
            return invalid("RGBA buffer does not match image dimensions");
        }
        let image = image::RgbaImage::from_raw(width, height, rgba)
            .ok_or_else(|| Error::Validation("invalid RGBA buffer".into()))?;
        let mut png = Cursor::new(Vec::new());
        image.write_to(&mut png, ImageFormat::Png)?;
        let png = png.into_inner();
        if png.len() as u64 > MAX_IMAGE_BYTES {
            return invalid("canonical PNG exceeds 32 MiB");
        }
        Ok(Self {
            width,
            height,
            rgba: image.into_raw().into(),
            png: png.into(),
        })
    }

    pub fn data_uri(&self) -> String {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&self.png)
        )
    }
}
/// Decode only PNG/JPEG bytes from an explicitly resolved local regular file.
/// Header dimensions are checked before full decode. Decoder allocation limits are
/// best effort; strict dimensions, encoded bytes and RGBA pixel budgets are enforced.
pub fn load_image(path: &Path) -> Result<ImageResource, Error> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return invalid("image path must be a regular file");
    }
    let mut bytes = Vec::new();
    file.take(MAX_IMAGE_BYTES + 1).read_to_end(&mut bytes)?;
    decode_image(&bytes)
}
fn decode_image(bytes: &[u8]) -> Result<ImageResource, Error> {
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return invalid("encoded image exceeds 32 MiB");
    }
    let format = image::guess_format(bytes)?;
    if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
        return invalid("only PNG and JPEG image assets are supported");
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    limits.max_alloc = Some(128 * 1024 * 1024);
    let mut header = ImageReader::with_format(Cursor::new(bytes), format);
    header.limits(limits.clone());
    let (width, height) = header.into_dimensions()?;
    check_dimensions(width, height)?;
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let image = reader.decode()?.to_rgba8();
    ImageResource::from_rgba(width, height, image.into_raw())
}
fn check_dimensions(width: u32, height: u32) -> Result<(), Error> {
    if width == 0
        || height == 0
        || width > MAX_IMAGE_SIDE
        || height > MAX_IMAGE_SIDE
        || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS
    {
        return invalid("image exceeds 8192 side or 16 megapixel decoded budget");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_png_preserves_transparent_rgba_and_rejects_other_formats() {
        let image = image::RgbaImage::from_raw(2, 1, vec![10, 20, 30, 0, 40, 50, 60, 128]).unwrap();
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let decoded = decode_image(bytes.get_ref()).unwrap();
        assert_eq!(decoded.rgba(), image.into_raw());
        assert_eq!((decoded.width, decoded.height), (2, 1));
        assert!(decoded
            .data_uri()
            .starts_with("data:image/png;base64,iVBOR"));
        assert_eq!(decode_image(&decoded.png).unwrap().rgba, decoded.rgba);
        assert!(decode_image(b"GIF89a").is_err());
        assert!(decode_image(b"not an image").is_err());
    }
    #[test]
    fn clones_share_immutable_pixel_and_png_storage() {
        let image = ImageResource::from_rgba(1, 1, vec![1, 2, 3, 4]).unwrap();
        let copy = image.clone();
        assert!(Arc::ptr_eq(&image.rgba, &copy.rgba));
        assert!(Arc::ptr_eq(&image.png, &copy.png));
    }
    #[test]
    fn rgba_constructor_enforces_shape_and_pixel_limits() {
        assert!(ImageResource::from_rgba(0, 1, vec![]).is_err());
        assert!(ImageResource::from_rgba(8192, 8192, vec![]).is_err());
        assert!(ImageResource::from_rgba(1, 1, vec![0; 3]).is_err());
        assert_eq!(
            ImageResource::from_rgba(1, 1, vec![1, 2, 3, 4])
                .unwrap()
                .rgba(),
            &[1, 2, 3, 4]
        );
    }
    #[test]
    fn jpeg_decodes_and_truncation_fails() {
        let image = image::RgbImage::from_pixel(2, 2, image::Rgb([40, 60, 80]));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Jpeg).unwrap();
        assert_eq!(decode_image(bytes.get_ref()).unwrap().rgba.len(), 16);
        assert!(decode_image(&bytes.get_ref()[..20]).is_err());
    }
    #[test]
    fn header_dimension_and_encoded_size_budgets_fail_before_decode() {
        // Valid PNG container with a header claiming a dimension beyond our bound.
        let image = image::RgbaImage::new(1, 1);
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let mut bytes = bytes.into_inner();
        bytes[16..20].copy_from_slice(&9000u32.to_be_bytes());
        assert!(decode_image(&bytes).is_err());
        assert!(decode_image(&vec![0; (MAX_IMAGE_BYTES + 1) as usize]).is_err());
    }
}
