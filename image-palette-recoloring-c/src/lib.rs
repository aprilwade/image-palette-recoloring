use libc::c_void;

use std::ptr;
use std::slice;

use image::{ImageBuffer, Rgb};
use image_palette_recoloring::{DecomposedImage, ImageWeights};

// On any platform other than WASM, you should be able to allocate and free a buffer without any
// extra these methods.
#[cfg(target_family = "wasm")]
#[no_mangle]
unsafe extern "C" fn create_image_buffer(size: u32) -> *mut u8 {
    Box::into_raw(vec![0u8; size as usize].into_boxed_slice()).cast()
}

#[cfg(target_family = "wasm")]
#[no_mangle]
unsafe extern "C" fn free_image_buffer(size: u32, buf: *mut u8) {
    let slice_ptr = slice::from_raw_parts_mut(buf, size as usize);
    let _ = Box::from_raw(slice_ptr);
}

#[no_mangle]
unsafe extern "C" fn create_image_weights(width: u32, height: u32, bytes: *const u8)
    -> *const c_void
{
    let bytes_slice = slice::from_raw_parts(bytes, 3 * (width * height) as usize);
    let Some(img) = ImageBuffer::<Rgb<u8>, _>::from_raw(width, height, bytes_slice) else {
        return ptr::null()
    };
    let weights = Box::new(ImageWeights::new(&img));

    Box::into_raw(weights) as *const _
}

#[no_mangle]
unsafe extern "C" fn free_image_weights(ptr: *const c_void)  {
    let _ = Box::from_raw(ptr as *mut c_void as *mut ImageWeights);
}

#[no_mangle]
unsafe extern "C" fn compute_palette(
    img_width: u32,
    img_height: u32,
    img_bytes: *const u8,
    min_palette_size: u8,
    error_bound: f64,
    out_palette_size: *mut u8
) -> *mut [u8; 3]
{
    let bytes_slice = slice::from_raw_parts(img_bytes, (img_width * img_height * 3) as usize);
    let Some(img) = ImageBuffer::<Rgb<u8>, _>::from_raw(img_width, img_height, bytes_slice) else {
        return ptr::null_mut()
    };

    let palette = image_palette_recoloring::compute_palette(
        &img,
        min_palette_size as usize,
        error_bound
    );

    *out_palette_size = palette.len() as u8;
    Box::into_raw(palette.into_boxed_slice()) as *mut [u8; 3]
}

#[no_mangle]
unsafe extern "C" fn free_computed_palette(ptr: *mut [u8; 3], palette_size: u8) {
    let ptr = ptr::slice_from_raw_parts_mut(ptr as *mut Rgb<u8>, palette_size as usize);
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
unsafe extern "C" fn create_decomposed_image(
    image_weights: *const c_void,
    palette_size: u8,
    palette: *const [u8; 3],
) -> *const c_void
{
    let image_weights = &*(image_weights as *const ImageWeights);
    let palette = slice::from_raw_parts(palette as *mut Rgb<u8>, palette_size as usize);

    let decomposed = match DecomposedImage::new(&image_weights, palette) {
        Ok(val) => val,
        Err(_) => return ptr::null(),
    };
    let decomposed = Box::new(decomposed);
    Box::into_raw(decomposed) as *const c_void
}

#[no_mangle]
unsafe extern "C" fn free_decomposed_image(ptr: *const c_void) {
    let _ = Box::from_raw(ptr as *mut c_void as *mut DecomposedImage);
}


#[no_mangle]
unsafe extern "C" fn get_decomposed_image_width(ptr: *const c_void) -> u32 {
    let decomposed_image = &*(ptr as *const DecomposedImage);
    decomposed_image.width()
}

#[no_mangle]
unsafe extern "C" fn get_decomposed_image_height(ptr: *const c_void) -> u32 {
    let decomposed_image = &*(ptr as *const DecomposedImage);
    decomposed_image.height()
}

#[no_mangle]
unsafe extern "C" fn reconstruct_image(
    decomposed_image: *const c_void,
    palette: *const [u8; 3],
    output_buffer: *mut u8,
) -> u8
{
    let decomposed_image = &*(decomposed_image as *const DecomposedImage);
    let num_channels = decomposed_image.num_channels();
    let palette = slice::from_raw_parts(palette as *mut Rgb<u8>, num_channels);
    let Some(reconstructed) = decomposed_image.reconstruct(palette) else {
        return 0;
    };

    let output_slice = slice::from_raw_parts_mut(
        output_buffer,
        (reconstructed.width() * reconstructed.height() * 3) as usize,
    );
    output_slice.copy_from_slice(&reconstructed);
    1
}

#[no_mangle]
unsafe extern "C" fn grayscale_image_channel(
    decomposed_image: *const c_void,
    channel: u8,
    output_buffer: *mut u8,
) -> u8
{
    let decomposed_image = &*(decomposed_image as *const DecomposedImage);
    let Some(reconstructed) = decomposed_image.get_channel_grayscale(channel as usize) else {
        return 0;
    };

    let output_slice = slice::from_raw_parts_mut(
        output_buffer,
        (reconstructed.width() * reconstructed.height()) as usize,
    );
    output_slice.copy_from_slice(&reconstructed);
    1
}

#[no_mangle]
unsafe extern "C" fn get_decomposed_image_num_channels(
    decomposed_image: *const c_void,
) -> u8
{
    let decomposed_image = &*(decomposed_image as *const DecomposedImage);
    decomposed_image.num_channels() as u8
}
