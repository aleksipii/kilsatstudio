//! C interface to the shared animation pipeline, used by the native iPad app.
//!
//! The iPad app never re-implements parsing, evaluation or rendering: it opens a
//! MotionScript YAML file or a `.kilsat` project here and asks for frames, which
//! come from `motion_export::ScenePreview`, the same deterministic SVG/raster path
//! and frame clock as export on the Mac. The C declarations live in
//! `include/koeta.h`; keep them in step with this file.
//!
//! Ownership: every pointer returned here is freed with the matching
//! `koeta_*_free` function. Calls on one document may come from any single
//! thread at a time or from several threads (the preview cache is locked).
use motion_export::{ArtContext, ScenePreview};
use motion_script::Script;
use std::{
    collections::BTreeMap,
    ffi::{c_char, CString},
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
    sync::{Arc, Mutex},
};

/// Largest accepted input file. The desktop source bound is far below this.
const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
/// Frame budget per scene, matching the order of the export budgets.
const MAX_FRAMES: u64 = 1_000_000;
/// Longest side of a rendered preview frame (the `ScenePreview` limit).
pub const MAX_SIDE: u32 = 1024;

/// An opened screenplay or project and its lazily built per-scene previews.
pub struct Document {
    script: Arc<Script>,
    art: Arc<ArtContext>,
    previews: Mutex<BTreeMap<usize, Arc<ScenePreview>>>,
}

impl Document {
    /// Open MotionScript YAML or a `.kilsat` JSON project from its bytes.
    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_INPUT_BYTES {
            return Err("Tiedosto on liian suuri (yli 64 MiB).".into());
        }
        let text =
            std::str::from_utf8(bytes).map_err(|_| "Tiedosto ei ole UTF-8-tekstiä.".to_string())?;
        let (source, art) = if text.trim_start().starts_with('{') {
            let project = motion_project::decode(bytes).map_err(|e| format!("Projekti: {e}"))?;
            (project.source, project.art)
        } else {
            (text.to_owned(), motion_art::Document::default())
        };
        let script = motion_script::parse(&source).map_err(|e| e.to_string())?;
        if script.scenes.is_empty() {
            return Err("Käsikirjoituksessa ei ole yhtään kohtausta.".into());
        }
        Ok(Self {
            script: Arc::new(script),
            art: Arc::new(ArtContext {
                document: art,
                images: BTreeMap::new(),
                screen_videos: BTreeMap::new(),
            }),
            previews: Mutex::new(BTreeMap::new()),
        })
    }

    pub fn title(&self) -> &str {
        &self.script.project.title
    }
    pub fn fps(&self) -> u32 {
        self.script.project.fps
    }
    pub fn scene_count(&self) -> usize {
        self.script.scenes.len()
    }
    pub fn scene_id(&self, scene: usize) -> Option<&str> {
        self.script.scenes.get(scene).map(|s| s.id.as_str())
    }
    /// Frames of `scene` on the shared frame clock (as exported).
    pub fn frame_count(&self, scene: usize) -> Result<u64, String> {
        let scene = self.script.scenes.get(scene).ok_or("Kohtausta ei ole.")?;
        motion_scene::frame_count(scene.duration, self.fps(), MAX_FRAMES).map_err(|e| e.to_string())
    }

    /// Render `frame` of `scene` as RGBA8, longest side at most `max_side`.
    pub fn render(&self, scene: usize, frame: u64, max_side: u32) -> Result<Frame, String> {
        if frame >= self.frame_count(scene)? {
            return Err("Ruutu on kohtauksen ulkopuolella.".into());
        }
        if !(1..=MAX_SIDE).contains(&max_side) {
            return Err(format!("Kuvan koon on oltava 1–{MAX_SIDE} pikseliä."));
        }
        let sample =
            motion_scene::time::frame_to_sample(frame, self.fps()).map_err(|e| e.to_string())?;
        let preview = self.preview(scene)?;
        let rendered = preview
            .render(sample, max_side, &motion_tts::CancellationToken::new())
            .map_err(|e| e.to_string())?;
        Ok(Frame {
            width: rendered.width,
            height: rendered.height,
            rgba: rendered.rgba,
        })
    }

    fn preview(&self, scene: usize) -> Result<Arc<ScenePreview>, String> {
        let mut previews = self.previews.lock().map_err(|_| "Esikatselu on lukossa.")?;
        if let Some(preview) = previews.get(&scene) {
            return Ok(preview.clone());
        }
        let preview = Arc::new(
            ScenePreview::new(self.script.clone(), scene, self.art.clone(), None)
                .map_err(|e| e.to_string())?,
        );
        previews.insert(scene, preview.clone());
        Ok(preview)
    }
}

/// One rendered RGBA8 frame, rows top to bottom, no padding.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// C view of a rendered frame; free with `koeta_frame_free`.
#[repr(C)]
pub struct KoetaFrame {
    pub rgba: *mut u8,
    pub len: usize,
    pub width: u32,
    pub height: u32,
}

fn set_error(error: *mut *mut c_char, message: String) {
    if !error.is_null() {
        let message = CString::new(message.replace('\0', " ")).unwrap_or_default();
        // SAFETY: the caller passes either null or a valid out-pointer.
        unsafe { *error = message.into_raw() };
    }
}

/// Run `body`, turning an error or a panic into `error` and `fallback`.
fn guarded<T>(error: *mut *mut c_char, fallback: T, body: impl FnOnce() -> Result<T, String>) -> T {
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(Ok(value)) => value,
        Ok(Err(message)) => {
            set_error(error, message);
            fallback
        }
        Err(_) => {
            set_error(error, "Sisäinen virhe animaatiomoottorissa.".into());
            fallback
        }
    }
}

/// # Safety
/// `document` must be null or a live pointer from `koeta_document_open`.
unsafe fn document<'a>(document: *const Document) -> Result<&'a Document, String> {
    // SAFETY: guaranteed by the caller as documented above.
    unsafe { document.as_ref() }.ok_or_else(|| "Asiakirjaa ei ole avattu.".into())
}

/// Open YAML or `.kilsat` bytes. Returns null and sets `*error` on failure.
///
/// # Safety
/// `bytes` must point to `len` readable bytes; `error` must be null or writable.
#[no_mangle]
pub unsafe extern "C" fn koeta_document_open(
    bytes: *const u8,
    len: usize,
    error: *mut *mut c_char,
) -> *mut Document {
    guarded(error, ptr::null_mut(), || {
        if bytes.is_null() {
            return Err("Tiedosto puuttuu.".into());
        }
        // SAFETY: the caller guarantees `len` readable bytes at `bytes`.
        let input = unsafe { std::slice::from_raw_parts(bytes, len) };
        Ok(Box::into_raw(Box::new(Document::open(input)?)))
    })
}

/// # Safety
/// `document` must be null or from `koeta_document_open`, freed only once.
#[no_mangle]
pub unsafe extern "C" fn koeta_document_free(document: *mut Document) {
    if !document.is_null() {
        // SAFETY: ownership returns to Rust exactly once, as documented.
        drop(unsafe { Box::from_raw(document) });
    }
}

/// Number of scenes, or 0 for a null document.
///
/// # Safety
/// `document` must be null or a live document.
#[no_mangle]
pub unsafe extern "C" fn koeta_document_scene_count(document: *const Document) -> u32 {
    // SAFETY: forwarded caller guarantee.
    unsafe { self::document(document) }.map_or(0, |d| d.scene_count().min(u32::MAX as usize) as u32)
}

/// Frames per second of the project, or 0 for a null document.
///
/// # Safety
/// `document` must be null or a live document.
#[no_mangle]
pub unsafe extern "C" fn koeta_document_fps(document: *const Document) -> u32 {
    // SAFETY: forwarded caller guarantee.
    unsafe { self::document(document) }.map_or(0, Document::fps)
}

/// Project title as a new C string; free with `koeta_string_free`.
///
/// # Safety
/// `document` must be null or a live document.
#[no_mangle]
pub unsafe extern "C" fn koeta_document_title(document: *const Document) -> *mut c_char {
    // SAFETY: forwarded caller guarantee.
    let title = unsafe { self::document(document) }.map_or("", Document::title);
    CString::new(title.replace('\0', " ")).map_or(ptr::null_mut(), CString::into_raw)
}

/// Scene id as a new C string, or null when out of range.
///
/// # Safety
/// `document` must be null or a live document.
#[no_mangle]
pub unsafe extern "C" fn koeta_document_scene_id(
    document: *const Document,
    scene: u32,
) -> *mut c_char {
    // SAFETY: forwarded caller guarantee.
    unsafe { self::document(document) }
        .ok()
        .and_then(|d| d.scene_id(scene as usize))
        .and_then(|id| CString::new(id).ok())
        .map_or(ptr::null_mut(), CString::into_raw)
}

/// Frame count of a scene; 0 and `*error` on failure.
///
/// # Safety
/// `document` must be null or a live document; `error` null or writable.
#[no_mangle]
pub unsafe extern "C" fn koeta_document_frame_count(
    document: *const Document,
    scene: u32,
    error: *mut *mut c_char,
) -> u64 {
    guarded(error, 0, || {
        // SAFETY: forwarded caller guarantee.
        unsafe { self::document(document) }?.frame_count(scene as usize)
    })
}

/// Render one frame into `*out`. Returns false and sets `*error` on failure.
///
/// # Safety
/// `document` must be null or a live document; `out` must be writable;
/// `error` null or writable. Free a filled frame with `koeta_frame_free`.
#[no_mangle]
pub unsafe extern "C" fn koeta_render_frame(
    document: *const Document,
    scene: u32,
    frame: u64,
    max_side: u32,
    out: *mut KoetaFrame,
    error: *mut *mut c_char,
) -> bool {
    guarded(error, false, || {
        if out.is_null() {
            return Err("Ruudulle ei annettu kohdetta.".into());
        }
        // SAFETY: forwarded caller guarantee.
        let rendered =
            unsafe { self::document(document) }?.render(scene as usize, frame, max_side)?;
        let mut rgba = rendered.rgba.into_boxed_slice();
        let len = rgba.len();
        let pointer = rgba.as_mut_ptr();
        std::mem::forget(rgba);
        // SAFETY: `out` is non-null and writable per the caller contract.
        unsafe {
            *out = KoetaFrame {
                rgba: pointer,
                len,
                width: rendered.width,
                height: rendered.height,
            }
        };
        Ok(true)
    })
}

/// # Safety
/// `frame` must be null or filled by `koeta_render_frame`, freed only once.
#[no_mangle]
pub unsafe extern "C" fn koeta_frame_free(frame: *mut KoetaFrame) {
    // SAFETY: the caller passes null or a frame filled by `koeta_render_frame`.
    let Some(frame) = (unsafe { frame.as_mut() }) else {
        return;
    };
    if !frame.rgba.is_null() {
        // SAFETY: rebuilds exactly the boxed slice leaked in `koeta_render_frame`.
        drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(frame.rgba, frame.len)) });
    }
    frame.rgba = ptr::null_mut();
    frame.len = 0;
}

/// # Safety
/// `string` must be null or a string returned by this library, freed only once.
#[no_mangle]
pub unsafe extern "C" fn koeta_string_free(string: *mut c_char) {
    if !string.is_null() {
        // SAFETY: the string was created by `CString::into_raw` in this library.
        drop(unsafe { CString::from_raw(string) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = include_str!("../../../examples/hello-emma.yaml");

    #[test]
    fn opens_yaml_and_renders_the_same_frame_every_time() {
        let document = Document::open(YAML.as_bytes()).unwrap();
        assert!(document.scene_count() >= 1);
        assert!(document.frame_count(0).unwrap() > 1);
        let first = document.render(0, 1, 256).unwrap();
        assert_eq!(
            first.rgba.len(),
            first.width as usize * first.height as usize * 4
        );
        assert_eq!(first.width.max(first.height), 256);
        assert_eq!(first.rgba, document.render(0, 1, 256).unwrap().rgba);
    }

    #[test]
    fn opens_a_kilsat_project_with_its_artwork() {
        let project = include_bytes!("../../../examples/studio-production-demo.kilsat");
        let document = Document::open(project).unwrap();
        assert!(!document.art.document.layers.is_empty());
        let frame = document.render(0, 0, 320).unwrap();
        assert_eq!((frame.width, frame.height), (320, 180));
    }

    #[test]
    fn invalid_input_and_out_of_range_requests_are_errors() {
        assert!(Document::open(b"scenes: [").is_err());
        assert!(Document::open(&[0xff, 0xfe]).is_err());
        let document = Document::open(YAML.as_bytes()).unwrap();
        let frames = document.frame_count(0).unwrap();
        assert!(document.render(0, frames, 256).is_err());
        assert!(document.render(0, 0, 0).is_err());
        assert!(document.render(0, 0, MAX_SIDE + 1).is_err());
        assert!(document.frame_count(document.scene_count()).is_err());
    }

    #[test]
    fn c_interface_round_trips_and_reports_errors() {
        unsafe {
            let mut error = ptr::null_mut();
            let document = koeta_document_open(YAML.as_ptr(), YAML.len(), &mut error);
            assert!(!document.is_null() && error.is_null());
            assert!(koeta_document_fps(document) > 0);
            let id = koeta_document_scene_id(document, 0);
            assert!(!id.is_null());
            koeta_string_free(id);
            assert!(koeta_document_scene_id(document, u32::MAX).is_null());
            let mut frame = KoetaFrame {
                rgba: ptr::null_mut(),
                len: 0,
                width: 0,
                height: 0,
            };
            assert!(koeta_render_frame(
                document, 0, 0, 128, &mut frame, &mut error
            ));
            assert_eq!(frame.len, frame.width as usize * frame.height as usize * 4);
            koeta_frame_free(&mut frame);
            assert!(frame.rgba.is_null());
            assert!(!koeta_render_frame(
                document, 99, 0, 128, &mut frame, &mut error
            ));
            assert!(!error.is_null());
            koeta_string_free(error);
            koeta_document_free(document);

            let mut error = ptr::null_mut();
            let bad = b"not: [valid";
            assert!(koeta_document_open(bad.as_ptr(), bad.len(), &mut error).is_null());
            assert!(!error.is_null());
            koeta_string_free(error);
            assert_eq!(koeta_document_scene_count(ptr::null()), 0);
        }
    }
}
