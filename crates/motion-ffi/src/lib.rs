//! C interface to the shared animation pipeline, used by the native iPad app.
//!
//! The iPad app never re-implements parsing, evaluation or rendering: it opens a
//! MotionScript YAML file or a `.kilsat` project here and asks for frames, which
//! come from `motion_export::ScenePreview`, the same deterministic SVG/raster path
//! and frame clock as export on the Mac. The C declarations live in
//! `include/koeta.h`; keep them in step with this file.
//!
//! A document opened from a file path also loads the project's image assets and
//! can prepare each scene's audio from imported WAV files with the strict
//! model-free path (`motion_voice::model_free`): no speech is ever synthesized,
//! and a line without its audio file is reported, never filled in.
//!
//! Ownership: every pointer returned here is freed with the matching
//! `koeta_*_free` function. Calls on one document may come from any single
//! thread at a time or from several threads (the preview cache is locked).
use motion_export::{ArtContext, ScenePreview};
use motion_script::{Event, Script};
use motion_voice::model_free::{prepare_scene_model_free, MissingAudioReason, ModelFreeError};
use std::{
    collections::BTreeMap,
    ffi::{c_char, CStr, CString},
    io::Read,
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    ptr,
    sync::{Arc, Mutex},
};

/// Largest accepted input file. The desktop source bound is far below this.
const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
/// Frame budget per scene, matching the order of the export budgets.
const MAX_FRAMES: u64 = 1_000_000;
/// Longest side of a rendered preview frame (the `ScenePreview` limit).
pub const MAX_SIDE: u32 = 1024;
/// Decoded image assets kept for one document (the desktop budget).
const MAX_IMAGE_BYTES: usize = 128 * 1024 * 1024;
/// Longest scene whose audio is prepared for playback: 10 minutes of 24 kHz mono
/// f32 is about 58 MB, which every iPad that runs iPadOS 16 can hold.
pub const MAX_AUDIO_SECONDS: f64 = 600.0;
/// Shared dialogue sample clock (`motion_audio::clock`).
pub const SAMPLE_RATE: u32 = motion_scene::time::SEQUENCE_SAMPLE_RATE;

/// An opened screenplay or project and its lazily built per-scene previews.
pub struct Document {
    script: Arc<Script>,
    art: Arc<ArtContext>,
    /// Directory that relative asset and audio paths resolve against.
    root: Option<PathBuf>,
    previews: Mutex<BTreeMap<usize, Arc<ScenePreview>>>,
}

impl Document {
    /// Open MotionScript YAML or a `.kilsat` JSON project from its bytes. Such a
    /// document has no project directory: image assets are not loaded and scene
    /// audio is unavailable. Use [`Self::open_path`] for both.
    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        Self::open_in(bytes, None)
    }

    /// Open a file and resolve its image assets and imported audio against the
    /// directory that contains it, as the Mac app and export do.
    pub fn open_path(path: &Path) -> Result<Self, String> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .and_then(|file| {
                file.take(MAX_INPUT_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)
            })
            .map_err(|e| format!("Tiedostoa ei voitu lukea: {e}"))?;
        Self::open_in(&bytes, Some(path))
    }

    fn open_in(bytes: &[u8], path: Option<&Path>) -> Result<Self, String> {
        if bytes.len() > MAX_INPUT_BYTES {
            return Err("Tiedosto on liian suuri (yli 64 MiB).".into());
        }
        let text =
            std::str::from_utf8(bytes).map_err(|_| "Tiedosto ei ole UTF-8-tekstiä.".to_string())?;
        let (source, art, images) = if text.trim_start().starts_with('{') {
            let project = motion_project::decode(bytes).map_err(|e| format!("Projekti: {e}"))?;
            let images = match path {
                Some(path) => load_images(path, &project.assets)?,
                None => BTreeMap::new(),
            };
            (project.source, project.art, images)
        } else {
            (
                text.to_owned(),
                motion_art::Document::default(),
                BTreeMap::new(),
            )
        };
        let script = motion_script::parse(&source).map_err(|e| e.to_string())?;
        if script.scenes.is_empty() {
            return Err("Käsikirjoituksessa ei ole yhtään kohtausta.".into());
        }
        let root = match path.map(project_dir) {
            Some(dir) => Some(
                dir.canonicalize()
                    .map_err(|e| format!("Projektin kansiota ei voitu lukea: {e}"))?,
            ),
            None => None,
        };
        Ok(Self {
            script: Arc::new(script),
            art: Arc::new(ArtContext {
                document: art,
                images,
                screen_videos: BTreeMap::new(),
            }),
            root,
            previews: Mutex::new(BTreeMap::new()),
        })
    }

    /// Mixed audio of `scene` as 24 kHz mono samples on the export clock, or
    /// `None` when the scene has no dialogue and no audio cues.
    ///
    /// Every line must name an imported WAV; nothing is synthesized. Mouths are
    /// taken from the same scene manifest as export (text-estimated timing
    /// unless an alignment exists), so later frames of this scene show them.
    /// Blocking: decodes WAV files, so call it off the main thread.
    pub fn scene_audio(&self, scene: usize) -> Result<Option<Vec<f32>>, String> {
        let entry = self.script.scenes.get(scene).ok_or("Kohtausta ei ole.")?;
        let has_dialogue = entry
            .events
            .iter()
            .any(|e| matches!(e, Event::Speak { .. }));
        if !has_dialogue && entry.composition.audio_cues.is_empty() {
            return Ok(None);
        }
        if !(0.0..=MAX_AUDIO_SECONDS).contains(&entry.duration) {
            return Err(format!(
                "Kohtaus on yli {} minuuttia pitkä, joten sen ääntä ei toisteta iPadilla.",
                MAX_AUDIO_SECONDS / 60.0
            ));
        }
        let root = self
            .root
            .as_deref()
            .ok_or("Ääntä ei voi ladata, koska projektin kansiota ei tunneta.")?;
        let prepared = prepare_scene_model_free(
            &self.script,
            scene,
            root,
            &BTreeMap::new(),
            &motion_tts::CancellationToken::new(),
            |_| {},
        )
        .map_err(|e| audio_error(e, root))?;
        let audio = prepared.rendered.audio;
        if audio.sample_rate != SAMPLE_RATE || audio.channels != 1 {
            return Err("Kohtauksen ääni ei ole 24 kHz:n monoääntä.".into());
        }
        let preview = ScenePreview::new(
            self.script.clone(),
            scene,
            self.art.clone(),
            Some(Arc::new(prepared.rendered.manifest)),
        )
        .map_err(|e| e.to_string())?;
        self.previews
            .lock()
            .map_err(|_| "Esikatselu on lukossa.")?
            .insert(scene, Arc::new(preview));
        Ok(Some(audio.samples))
    }

    /// Frame that shows `sample` on the shared clock.
    pub fn frame_at_sample(&self, sample: u64) -> Result<u64, String> {
        motion_scene::time::sample_to_frame(sample, self.fps()).map_err(|e| e.to_string())
    }

    /// First sample of `frame` on the shared clock.
    pub fn frame_start_sample(&self, frame: u64) -> Result<u64, String> {
        motion_scene::time::frame_to_sample(frame, self.fps()).map_err(|e| e.to_string())
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

fn project_dir(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

/// Shown when the app may read the project file but not the files next to it.
const FOLDER_HINT: &str =
    "Jos projekti on avattu yksittäisenä tiedostona, avaa sen kansio (Avaa kansio…), jotta sovellus saa luvan projektin kuviin ja ääniin.";

/// Decode every image asset of a project, as the desktop does, within one budget.
fn load_images(
    path: &Path,
    assets: &[motion_project::AssetReference],
) -> Result<BTreeMap<String, motion_art::ImageResource>, String> {
    let mut images = BTreeMap::new();
    let mut bytes = 0usize;
    for asset in assets
        .iter()
        .filter(|a| a.kind == motion_project::AssetKind::Image)
    {
        let image = motion_project::resolve_asset(path, asset)
            .map_err(|e| e.to_string())
            .and_then(|file| motion_art::load_image(&file).map_err(|e| e.to_string()))
            .and_then(|image| {
                image
                    .reserve_in(motion_jobs::process_budget())
                    .map_err(|e| e.to_string())
            })
            .map_err(|e| format!("Kuvaa {} ei voitu ladata: {e}. {FOLDER_HINT}", asset.path))?;
        bytes += image.rgba().len() + image.png().len();
        if bytes > MAX_IMAGE_BYTES {
            return Err("Projektin kuvat vievät yli 128 MiB muistia.".into());
        }
        images.insert(asset.id.clone(), image);
    }
    Ok(images)
}

fn audio_error(error: ModelFreeError, root: &Path) -> String {
    match error {
        ModelFreeError::MissingImportedAudio(lines) => {
            let mut message = String::from(
                "Kohtauksen ääntä ei toisteta. iPad ei syntetisoi puhetta, ja näiltä repliikeiltä puuttuu äänitiedosto:",
            );
            for line in lines.iter().take(5) {
                match &line.reason {
                    MissingAudioReason::NotSpecified => {
                        message += &format!(
                            " {} (repliikki {}) ilman audio-kenttää;",
                            line.actor, line.dialogue_id
                        )
                    }
                    MissingAudioReason::FileMissing(file) => {
                        let shown = file.strip_prefix(root).unwrap_or(file);
                        message += &format!(
                            " {} (repliikki {}): {} puuttuu;",
                            line.actor,
                            line.dialogue_id,
                            shown.display()
                        )
                    }
                }
            }
            if lines.len() > 5 {
                message += &format!(" ja {} muuta;", lines.len() - 5);
            }
            message.pop();
            message += ".";
            if lines
                .iter()
                .any(|l| matches!(l.reason, MissingAudioReason::FileMissing(_)))
            {
                message += " ";
                message += FOLDER_HINT;
            }
            message
        }
        ModelFreeError::Preparation(error) => {
            format!("Kohtauksen ääntä ei voitu valmistella: {}", error.0)
        }
    }
}

// The iPad app renders on one queue while it prepares audio on another.
const _: fn() = || {
    fn shareable<T: Send + Sync>() {}
    shareable::<Document>();
};

/// One rendered RGBA8 frame, rows top to bottom, no padding.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// C view of a scene's mixed audio; free with `koeta_audio_free`.
/// `samples` is null and `len` 0 when the scene has no audio.
#[repr(C)]
pub struct KoetaAudio {
    pub samples: *mut f32,
    pub len: usize,
    pub sample_rate: u32,
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

/// Open a file by path; image assets and imported audio resolve next to it.
/// Returns null and sets `*error` on failure.
///
/// # Safety
/// `path` must be null or a NUL-terminated UTF-8 string; `error` null or writable.
#[no_mangle]
pub unsafe extern "C" fn koeta_document_open_path(
    path: *const c_char,
    error: *mut *mut c_char,
) -> *mut Document {
    guarded(error, ptr::null_mut(), || {
        if path.is_null() {
            return Err("Tiedostopolku puuttuu.".into());
        }
        // SAFETY: the caller passes a NUL-terminated string.
        let path = unsafe { CStr::from_ptr(path) }
            .to_str()
            .map_err(|_| "Tiedostopolku ei ole UTF-8-tekstiä.".to_string())?;
        Ok(Box::into_raw(Box::new(Document::open_path(Path::new(
            path,
        ))?)))
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

/// Prepare the mixed audio of `scene` into `*out` (see `Document::scene_audio`).
/// Returns false and sets `*error` when the audio cannot be prepared; a scene
/// without audio succeeds with an empty `*out`. Blocking.
///
/// # Safety
/// `document` must be null or a live document; `out` must be writable;
/// `error` null or writable. Free a filled buffer with `koeta_audio_free`.
#[no_mangle]
pub unsafe extern "C" fn koeta_scene_audio(
    document: *const Document,
    scene: u32,
    out: *mut KoetaAudio,
    error: *mut *mut c_char,
) -> bool {
    guarded(error, false, || {
        if out.is_null() {
            return Err("Äänelle ei annettu kohdetta.".into());
        }
        // SAFETY: forwarded caller guarantee.
        let samples = unsafe { self::document(document) }?.scene_audio(scene as usize)?;
        let (pointer, len) = match samples {
            Some(samples) => {
                let mut samples = samples.into_boxed_slice();
                let len = samples.len();
                let pointer = samples.as_mut_ptr();
                std::mem::forget(samples);
                (pointer, len)
            }
            None => (ptr::null_mut(), 0),
        };
        // SAFETY: `out` is non-null and writable per the caller contract.
        unsafe {
            *out = KoetaAudio {
                samples: pointer,
                len,
                sample_rate: SAMPLE_RATE,
            }
        };
        Ok(true)
    })
}

/// # Safety
/// `audio` must be null or filled by `koeta_scene_audio`, freed only once.
#[no_mangle]
pub unsafe extern "C" fn koeta_audio_free(audio: *mut KoetaAudio) {
    // SAFETY: the caller passes null or a buffer filled by `koeta_scene_audio`.
    let Some(audio) = (unsafe { audio.as_mut() }) else {
        return;
    };
    if !audio.samples.is_null() {
        // SAFETY: rebuilds exactly the boxed slice leaked in `koeta_scene_audio`.
        drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(audio.samples, audio.len)) });
    }
    audio.samples = ptr::null_mut();
    audio.len = 0;
}

/// Frame that shows `sample` on the shared clock; 0 for a null document.
///
/// # Safety
/// `document` must be null or a live document.
#[no_mangle]
pub unsafe extern "C" fn koeta_document_frame_at_sample(
    document: *const Document,
    sample: u64,
) -> u64 {
    // SAFETY: forwarded caller guarantee.
    unsafe { self::document(document) }
        .and_then(|d| d.frame_at_sample(sample))
        .unwrap_or(0)
}

/// First sample of `frame` on the shared clock; 0 for a null document or overflow.
///
/// # Safety
/// `document` must be null or a live document.
#[no_mangle]
pub unsafe extern "C" fn koeta_document_frame_start_sample(
    document: *const Document,
    frame: u64,
) -> u64 {
    // SAFETY: forwarded caller guarantee.
    unsafe { self::document(document) }
        .and_then(|d| d.frame_start_sample(frame))
        .unwrap_or(0)
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

    /// Temporary project directory, removed on drop.
    struct Dir(PathBuf);
    impl Dir {
        fn new(tag: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "koeta-ffi-{tag}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            ));
            std::fs::create_dir_all(path.join("audio")).unwrap();
            Self(path)
        }
        fn write(&self, name: &str, text: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, text).unwrap();
            path
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Procedural test tone after leading silence, not speech.
    fn write_tone(path: &Path, frames: usize, hz: f32) {
        let samples = (0..frames)
            .map(|i| {
                if i < 1200 {
                    0.0
                } else {
                    0.3 * (i as f32 * hz * std::f32::consts::TAU / 24_000.0).sin()
                }
            })
            .collect();
        let buffer = motion_audio::AudioBuffer {
            sample_rate: SAMPLE_RATE,
            channels: 1,
            samples,
        };
        motion_audio::write_wav(path, &buffer).unwrap();
    }

    const DIALOGUE: &str = "characters:
  AINO: {asset: characters/aino, voice: {provider: piper, preset: piper_fi_fixture, language: fi-fi, speed: 1.0}}
  EMMA: {asset: characters/emma, voice: {provider: kokoro, preset: af_heart, language: en-us, speed: 1.0}}
scenes:
  - id: kahvila
    duration: 2.0
    events:
      - {type: speak, at: 0.25, actor: AINO, text: \"Hyvää yötä, Emma.\", audio: audio/aino.wav}
      - {type: speak, at: 1.0, actor: EMMA, text: \"Good night.\", audio: audio/emma.wav}
  - id: hiljaisuus
    duration: 1.0
    events: []
";

    #[test]
    fn scene_audio_is_the_export_mix_and_drives_the_mouths() {
        let dir = Dir::new("audio");
        write_tone(&dir.0.join("audio/aino.wav"), 12_000, 180.0);
        write_tone(&dir.0.join("audio/emma.wav"), 12_000, 220.0);
        let path = dir.write("dialogue.yaml", DIALOGUE);
        let document = Document::open_path(&path).unwrap();
        let frames = document.frame_count(0).unwrap();
        let before: Vec<_> = (0..frames)
            .map(|f| document.render(0, f, 96).unwrap().rgba)
            .collect();

        let samples = document.scene_audio(0).unwrap().unwrap();
        assert_eq!(samples.len(), 2 * SAMPLE_RATE as usize);
        // Silent until AINO's tone starts 0.25 s + 1200 samples into the scene.
        assert!(samples[..6000 + 1200].iter().all(|s| *s == 0.0));
        assert!(samples[6000 + 1200..18_000].iter().any(|s| *s != 0.0));
        // Exactly the export mixer's output for the same scene.
        let script = motion_script::parse(DIALOGUE).unwrap();
        let export = prepare_scene_model_free(
            &script,
            0,
            &dir.0.canonicalize().unwrap(),
            &BTreeMap::new(),
            &motion_tts::CancellationToken::new(),
            |_| {},
        )
        .unwrap();
        assert_eq!(samples, export.rendered.audio.samples);
        // The prepared manifest now animates the mouths in rendered frames.
        let after: Vec<_> = (0..frames)
            .map(|f| document.render(0, f, 96).unwrap().rgba)
            .collect();
        assert_eq!(before[0], after[0]);
        assert!(before.iter().zip(&after).any(|(b, a)| b != a));
        // A scene without dialogue or cues has no audio track.
        assert_eq!(document.scene_audio(1).unwrap(), None);
    }

    #[test]
    fn missing_dialogue_audio_is_reported_and_never_synthesized() {
        let dir = Dir::new("missing");
        write_tone(&dir.0.join("audio/aino.wav"), 12_000, 180.0);
        let path = dir.write("dialogue.yaml", DIALOGUE);
        let error = Document::open_path(&path)
            .unwrap()
            .scene_audio(0)
            .unwrap_err();
        assert!(error.contains("ei syntetisoi"), "{error}");
        assert!(
            error.contains("EMMA") && error.contains("audio/emma.wav"),
            "{error}"
        );
        assert!(!error.contains(&dir.0.display().to_string()), "{error}");

        let no_field = DIALOGUE.replace(", audio: audio/emma.wav", "");
        let path = dir.write("no-field.yaml", &no_field);
        let error = Document::open_path(&path)
            .unwrap()
            .scene_audio(0)
            .unwrap_err();
        assert!(error.contains("audio-kenttää"), "{error}");
        assert!(!error.contains("Avaa kansio"), "{error}");
        // Bytes alone have no project directory to resolve audio against.
        let error = Document::open(DIALOGUE.as_bytes())
            .unwrap()
            .scene_audio(0)
            .unwrap_err();
        assert!(error.contains("kansiota"), "{error}");
    }

    #[test]
    fn opening_by_path_loads_image_assets_or_explains_missing_access() {
        let season = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/kilsat-season");
        let path = season.join("ep02.kilsat");
        let document = Document::open_path(&path).unwrap();
        assert_eq!(document.art.images.len(), 1);
        let frame = document.render(0, 0, 160).unwrap();
        assert_eq!(
            frame.rgba.len(),
            frame.width as usize * frame.height as usize * 4
        );
        // The same project without its assets folder next to it.
        let dir = Dir::new("images");
        let copy = dir.0.join("ep02.kilsat");
        std::fs::copy(&path, &copy).unwrap();
        let error = Document::open_path(&copy).err().unwrap();
        assert!(error.contains("assets/kilsat-example.png"), "{error}");
        assert!(error.contains("Avaa kansio"), "{error}");
    }

    #[test]
    fn clock_conversions_round_trip_on_the_shared_sample_clock() {
        let document = Document::open(YAML.as_bytes()).unwrap();
        let fps = document.fps();
        for frame in [0, 1, 2, 23, 24, 1000] {
            let sample = document.frame_start_sample(frame).unwrap();
            assert_eq!(sample, frame * SAMPLE_RATE as u64 / fps as u64);
            assert_eq!(document.frame_at_sample(sample).unwrap(), frame);
            assert_eq!(document.frame_at_sample(sample + 1).unwrap(), frame);
        }
    }

    #[test]
    fn c_audio_interface_round_trips() {
        let dir = Dir::new("c-audio");
        write_tone(&dir.0.join("audio/aino.wav"), 12_000, 180.0);
        write_tone(&dir.0.join("audio/emma.wav"), 12_000, 220.0);
        let path = dir.write("dialogue.yaml", DIALOGUE);
        let path = CString::new(path.to_str().unwrap()).unwrap();
        unsafe {
            let mut error = ptr::null_mut();
            let document = koeta_document_open_path(path.as_ptr(), &mut error);
            assert!(!document.is_null() && error.is_null());
            let mut audio = KoetaAudio {
                samples: ptr::null_mut(),
                len: 0,
                sample_rate: 0,
            };
            assert!(koeta_scene_audio(document, 0, &mut audio, &mut error));
            assert_eq!((audio.len, audio.sample_rate), (48_000, SAMPLE_RATE));
            assert!(!audio.samples.is_null());
            koeta_audio_free(&mut audio);
            assert!(audio.samples.is_null() && audio.len == 0);
            assert!(koeta_scene_audio(document, 1, &mut audio, &mut error));
            assert!(audio.samples.is_null() && audio.len == 0);
            assert!(!koeta_scene_audio(document, 9, &mut audio, &mut error));
            assert!(!error.is_null());
            koeta_string_free(error);
            let frame = koeta_document_frame_at_sample(document, 24_000);
            assert_eq!(koeta_document_frame_start_sample(document, frame), 24_000);
            koeta_document_free(document);

            let mut error = ptr::null_mut();
            assert!(koeta_document_open_path(ptr::null(), &mut error).is_null());
            koeta_string_free(error);
            assert_eq!(koeta_document_frame_at_sample(ptr::null(), 5), 0);
        }
    }
}
