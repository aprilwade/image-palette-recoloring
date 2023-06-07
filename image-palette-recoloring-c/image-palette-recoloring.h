#ifndef INC_IMAGE_PALETTE_RECOLORING_H
#define INC_IMAGE_PALETTE_RECOLORING_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct image_weights image_weights;

/// Compute the RGBXY weights for an image.
/// This is the most expensive part of the process, so save the return to avoid
/// unnecessarily recomputing it where possible.
///
/// This computation shouldn't be able to fail.
image_weights* create_image_weights(uint32_t width, uint32_t height, const uint8_t *rgb_bytes);
void free_image_weights(image_weights *weights);

/// Compute an initial decomposition palette for an image.
/// The returned palette is a list of RGB values. The total byte size of the
/// buffer returned is 3 times the value stored into out_palette_color_count.
///
/// The return palette will never be smaller than 4 colors.
uint8_t *compute_palette(
    uint32_t img_width,
    uint32_t img_heigh,
    const uint8_t *rgb_img_bytes,
    uint8_t min_palette_size,
    double error_bound,
    uint8_t *out_palette_color_count
);
void free_computed_palette(uint8_t *palette_bytes, uint8_t palette_color_count);


typedef struct decomposed_image decomposed_image;

/// Decompose an image into channels based on a provided palette.
///
/// The provided palette can be created using the `compute_palette` function or
/// entirely custom. Note that the absolute minimum size for a decomposition
/// palette is 4 colors. If a smaller palette is provided, the function will
/// return NULL.
///
/// If the provided palette contains "redundant" colors, then the function will
/// return NULL. The decomposition process relies on creating a 3d convex-hull
/// of the palette and if any of the colors provided lie within the hull rather
/// are vertices of the hull, they are redundant.
decomposed_image *create_decomposed_image(
    const image_weights *weights,
    uint8_t palette_color_count,
    const uint8_t *palette_bytes
);
void free_decomposed_image(decomposed_image *img);

/// Returns the width of the original image in pixels.
uint32_t get_decomposed_image_width(const decomposed_image *img);
/// Returns the height of the original image in pixels.
uint32_t get_decomposed_image_height(const decomposed_image *img);
/// Returns the number of colors in the palette used to decompose the image.
uint8_t get_decomposed_image_num_channels(const decomposed_image *img);

/// Extract one channel of a decomposed image as grayscale image.
///
/// On success, this function returns 1. (At this time, the function cannot
/// fail.)
///
/// `palette_bytes` is assumed to contain 1 color (ie 3 bytes) for each color
/// in the decomposition palette.
///
/// `output_buf` should contain at least 3 bytes per pixel in the original
/// image. The function doesn't perform any bounds checking of `output_buf`,
/// but you can use `get_decomposed_image_width` and
/// `get_decomposed_image_height` to recover the original image size yourself.
uint8_t reconstruct_image(
    const decomposed_image *img,
    const uint8_t *palette_bytes,
    uint8_t *output_buf
);

/// Extract one channel of a decomposed image as grayscale image.
///
/// On success, this function returns 1.
///
/// If `channel` is out-of-bounds (ie greater-than-or-equal to the number of
/// colors in the decomposition palette), then the function will return 0 to
/// indicate failure. In this case, nothing is written `output_buf`.
///
/// `output_buf` should contain at least 1 byte per pixel in the original
/// image. The function doesn't perform any bounds checking of `output_buf`,
/// but you can use `get_decomposed_image_width` and
/// `get_decomposed_image_height` to recover the original image size yourself.
uint8_t grayscale_image_channel(
    const decomposed_image *img,
    uint8_t channel,
    uint8_t *output_buf
);

#ifdef __cplusplus
}
#endif

#endif
