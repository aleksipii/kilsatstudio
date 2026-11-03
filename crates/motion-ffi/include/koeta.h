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
void koeta_string_free(char *string);

#ifdef __cplusplus
}
#endif

#endif
