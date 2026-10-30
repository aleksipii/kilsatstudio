//! Colour-space labelling for exported media.
//!
//! Kilsat renders 8-bit sRGB-encoded RGBA. There is no ACES or OCIO pipeline and
//! no colour transform between the renderer and the file: these tags only state
//! how the existing code values should be interpreted.

/// How encoded video is labelled. Both use Rec.709/sRGB primaries and the
/// Rec.709 Y'CbCr matrix in limited ("tv") range; they differ only in the
/// transfer characteristic written to the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorTag {
    /// Transfer `iec61966-2-1` (sRGB). Matches the renderer's output exactly.
    #[default]
    Srgb,
    /// Transfer `bt709`. The common broadcast/NLE delivery label; code values are
    /// not re-encoded, so this is a relabel of the same sRGB-encoded pixels.
    Rec709,
}

impl ColorTag {
    /// FFmpeg/ffprobe name of the transfer characteristic.
    pub fn transfer(self) -> &'static str {
        match self {
            ColorTag::Srgb => "iec61966-2-1",
            ColorTag::Rec709 => "bt709",
        }
    }
    /// Short name for manifests.
    pub fn name(self) -> &'static str {
        match self {
            ColorTag::Srgb => "srgb",
            ColorTag::Rec709 => "rec709",
        }
    }
    /// Output options: convert RGB to Y'CbCr with the Rec.709 matrix (FFmpeg's
    /// default for RGB input is BT.601) and write matching container tags.
    ///
    /// `setparams` stamps the tags onto every frame. FFmpeg 9 writes the frame's
    /// own colour properties (from the PNG decoder) and ignores `-color_trc`
    /// alone, which produced `unknown` or `iec61966-2-1` regardless of the choice.
    pub(crate) fn ffmpeg_args(self) -> Vec<&'static str> {
        let filter = match self {
            ColorTag::Srgb => {
                "scale=out_color_matrix=bt709:out_range=tv,\
                 setparams=color_primaries=bt709:color_trc=iec61966-2-1:colorspace=bt709:range=tv"
            }
            ColorTag::Rec709 => {
                "scale=out_color_matrix=bt709:out_range=tv,\
                 setparams=color_primaries=bt709:color_trc=bt709:colorspace=bt709:range=tv"
            }
        };
        vec![
            "-vf",
            filter,
            "-color_primaries",
            "bt709",
            "-color_trc",
            self.transfer(),
            "-colorspace",
            "bt709",
            "-color_range",
            "tv",
        ]
    }
}

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
/// gAMA value for sRGB required by the PNG specification (1/2.2 × 100000).
const SRGB_GAMMA: u32 = 45_455;

/// Insert `sRGB` (perceptual intent) and `gAMA` chunks directly after `IHDR`.
/// Pixel data is untouched. PNGs that already carry colour chunks are returned as-is.
pub(crate) fn tag_png_srgb(png: Vec<u8>) -> Result<Vec<u8>, &'static str> {
    const IHDR_END: usize = 8 + 8 + 13 + 4;
    if png.len() < IHDR_END || &png[..8] != SIGNATURE || &png[12..16] != b"IHDR" {
        return Err("PNG encoder output has no leading IHDR chunk");
    }
    if chunk_types(&png).any(|kind| matches!(&kind, b"sRGB" | b"gAMA" | b"iCCP" | b"cHRM")) {
        return Ok(png);
    }
    let mut out = Vec::with_capacity(png.len() + 29);
    out.extend_from_slice(&png[..IHDR_END]);
    push_chunk(&mut out, b"sRGB", &[0]);
    push_chunk(&mut out, b"gAMA", &SRGB_GAMMA.to_be_bytes());
    out.extend_from_slice(&png[IHDR_END..]);
    Ok(out)
}

fn chunk_types(png: &[u8]) -> impl Iterator<Item = [u8; 4]> + '_ {
    let mut at = 8usize;
    std::iter::from_fn(move || {
        let header = png.get(at..at + 8)?;
        let length = u32::from_be_bytes(header[..4].try_into().ok()?) as usize;
        let kind: [u8; 4] = header[4..8].try_into().ok()?;
        at = at.checked_add(12)?.checked_add(length)?;
        Some(kind)
    })
}

fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// CRC-32 (ISO-HDLC) as used by PNG chunks.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Remove the colour chunks added by [`tag_png_srgb`]; used by regression tests
/// whose reference hashes describe the pixel stream.
#[cfg(test)]
pub(crate) fn strip_png_color_chunks(png: &[u8]) -> Vec<u8> {
    let mut out = png[..8].to_vec();
    let mut at = 8usize;
    while at + 12 <= png.len() {
        let length = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
        let end = at + 12 + length;
        if !matches!(&png[at + 4..at + 8], b"sRGB" | b"gAMA") {
            out.extend_from_slice(&png[at..end]);
        }
        at = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_known_png_chunk() {
        // IEND has a fixed, well-known CRC.
        assert_eq!(crc32(b"IEND"), 0xae42_6082);
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn srgb_and_gamma_follow_ihdr_and_pixels_decode_identically() {
        let mut pixmap = resvg::tiny_skia::Pixmap::new(3, 2).unwrap();
        pixmap.fill(resvg::tiny_skia::Color::from_rgba8(10, 200, 30, 128));
        let plain = pixmap.encode_png().unwrap();
        let tagged = tag_png_srgb(plain.clone()).unwrap();
        let kinds: Vec<[u8; 4]> = chunk_types(&tagged).collect();
        assert_eq!(&kinds[..3], &[*b"IHDR", *b"sRGB", *b"gAMA"]);
        // sRGB chunk: length 1, intent 0; gAMA: 45455.
        assert_eq!(&tagged[33..41], b"\0\0\0\x01sRGB");
        assert_eq!(tagged[41], 0);
        assert_eq!(&tagged[46..54], b"\0\0\0\x04gAMA");
        assert_eq!(
            u32::from_be_bytes(tagged[54..58].try_into().unwrap()),
            45_455
        );
        let decoded = resvg::tiny_skia::Pixmap::decode_png(&tagged).unwrap();
        assert_eq!(decoded.data(), pixmap.data());
        assert_eq!(strip_png_color_chunks(&tagged), plain);
        // Idempotent and rejects non-PNG input.
        assert_eq!(tag_png_srgb(tagged.clone()).unwrap(), tagged);
        assert!(tag_png_srgb(b"not a png".to_vec()).is_err());
    }

    #[test]
    fn video_tags_name_matrix_primaries_transfer_and_range() {
        let args = ColorTag::Rec709.ffmpeg_args();
        assert!(args.windows(2).any(|w| w == ["-color_trc", "bt709"]));
        assert!(args.windows(2).any(|w| w == ["-colorspace", "bt709"]));
        let filter =
            |args: &[&str]| args[args.iter().position(|a| *a == "-vf").unwrap() + 1].to_owned();
        let rec709 = filter(&args);
        assert!(rec709.starts_with("scale=out_color_matrix=bt709:out_range=tv,setparams="));
        assert!(rec709.contains("color_trc=bt709:"));
        let args = ColorTag::Srgb.ffmpeg_args();
        assert!(args.windows(2).any(|w| w == ["-color_trc", "iec61966-2-1"]));
        let srgb = filter(&args);
        assert!(srgb.contains("color_trc=iec61966-2-1:colorspace=bt709:range=tv"));
    }
}
