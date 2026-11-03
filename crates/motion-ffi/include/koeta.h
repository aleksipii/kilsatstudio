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

// Opens MotionScript YAML or a .kilsat JSON project. NULL on failure.
KoetaDocument *koeta_document_open(const uint8_t *bytes, size_t len, char **error);
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
void koeta_string_free(char *string);

#ifdef __cplusplus
}
#endif

#endif
