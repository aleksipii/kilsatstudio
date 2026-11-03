// C interface to the Koeta Studio animation pipeline (crates/motion-ffi).
// Keep in step with crates/motion-ffi/src/lib.rs.
//
// Every returned pointer is released with its koeta_*_free function. Error
// out-parameters may be NULL; when set they hold a message for the user that
// must be freed with koeta_string_free.
#ifndef KOETA_H
#define KOETA_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct Document KoetaDocument;
typedef struct Export KoetaExport;

typedef struct KoetaFrame {
    uint8_t *rgba; // premultiplied RGBA8, rows top to bottom, width * height * 4 bytes
    size_t len;
    uint32_t width;
    uint32_t height;
} KoetaFrame;

typedef struct KoetaAudio {
    float *samples; // mono samples at sample_rate (24000); NULL and len 0 when the scene has no audio
    size_t len;
    uint32_t sample_rate;
} KoetaAudio;

// Opens MotionScript YAML or a .kilsat JSON project. NULL on failure.
// Without a project directory: no image assets, no scene audio.
KoetaDocument *koeta_document_open(const uint8_t *bytes, size_t len, char **error);
// Opens a file by UTF-8 path. Image assets load now and imported audio resolves
// against the file's directory, so the app must keep read access to it.
KoetaDocument *koeta_document_open_path(const char *path, char **error);
void koeta_document_free(KoetaDocument *document);

uint32_t koeta_document_scene_count(const KoetaDocument *document);
uint32_t koeta_document_fps(const KoetaDocument *document);
char *koeta_document_title(const KoetaDocument *document);
char *koeta_document_scene_id(const KoetaDocument *document, uint32_t scene);
// 0 on failure, with *error set.
uint64_t koeta_document_frame_count(const KoetaDocument *document, uint32_t scene, char **error);

// Renders one frame on the export frame clock; longest side 1...1024 pixels.
bool koeta_render_frame(const KoetaDocument *document, uint32_t scene, uint64_t frame,
                        uint32_t max_side, KoetaFrame *out, char **error);
void koeta_frame_free(KoetaFrame *frame);

// Mixed scene audio from imported WAV files, as export mixes it. Never
// synthesizes speech: a line without its file fails with *error set. Blocking
// (decodes files); afterwards rendered frames of the scene show its mouths.
bool koeta_scene_audio(const KoetaDocument *document, uint32_t scene, KoetaAudio *out,
                       char **error);
void koeta_audio_free(KoetaAudio *audio);

// The shared 24 kHz clock: the frame that shows a sample, and a frame's first sample.
uint64_t koeta_document_frame_at_sample(const KoetaDocument *document, uint64_t sample);
uint64_t koeta_document_frame_start_sample(const KoetaDocument *document, uint64_t frame);

// Editing. Errors name their kind: syntax, unsupported command, or invalid value/reference.
// True when the MotionScript opens; else *line is the 1-based line named (0: none).
bool koeta_check_source(const uint8_t *source, size_t len, uint32_t *line, char **error);
// The open screenplay's MotionScript YAML (a project's screenplay).
char *koeta_document_source(const KoetaDocument *document);
// Opens edited YAML in place of the screenplay of the file at path; a .kilsat
// keeps its artwork and images. Nothing is written. NULL on failure.
KoetaDocument *koeta_document_open_edited(const char *path, const uint8_t *source, size_t len,
                                          char **error);
typedef struct KoetaBytes {
    uint8_t *bytes;
    size_t len;
} KoetaBytes;

// Checks edited YAML and returns the bytes that save it over the file at path:
// a .kilsat keeps everything but its screenplay. Writes nothing (the app writes
// with file coordination). Free with koeta_bytes_free.
bool koeta_saved_bytes(const char *path, const uint8_t *source, size_t len, KoetaBytes *out,
                       char **error);
void koeta_bytes_free(KoetaBytes *bytes);

// Movie export: the same frames and mix as the Mac export, for an encoder in the app.
// scene < 0 exports every scene with its crossfades. Blocking (decodes audio); dialogue
// must come from imported WAV files, as in playback. NULL on failure.
KoetaExport *koeta_export_begin(const KoetaDocument *document, int64_t scene, char **error);
uint64_t koeta_export_frame_count(const KoetaExport *export_);
uint32_t koeta_export_fps(const KoetaExport *export_);
void koeta_export_size(const KoetaExport *export_, uint32_t *width, uint32_t *height);
// Frame at the authored size, opaque RGBA8; free with koeta_frame_free.
bool koeta_export_render(const KoetaExport *export_, uint64_t frame, KoetaFrame *out,
                         char **error);
// 24 kHz mono mix; free with koeta_audio_free. Blocking.
bool koeta_export_audio(const KoetaExport *export_, KoetaAudio *out, char **error);
// Safe from any thread; later render and audio calls fail.
void koeta_export_cancel(const KoetaExport *export_);
// After every other call on it has returned.
void koeta_export_free(KoetaExport *export_);

void koeta_string_free(char *string);

#ifdef __cplusplus
}
#endif

#endif
