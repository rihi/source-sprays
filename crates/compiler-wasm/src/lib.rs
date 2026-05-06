use image::RgbaImage;
use source_spray_common::vtf::TextureFormat;
use source_spray_compiler_core::vtf::{find_lresolution, write_vtf};
use wasm_bindgen::prelude::*;


#[wasm_bindgen]
pub enum WasmTextureFormat {
    DXT1,
    DXT5,
}

impl From<WasmTextureFormat> for TextureFormat {
    fn from(value: WasmTextureFormat) -> Self {
        match value {
            WasmTextureFormat::DXT1 => TextureFormat::DXT1,
            WasmTextureFormat::DXT5 => TextureFormat::DXT5,
        }
    }
}

#[wasm_bindgen]
pub fn file_size(
    width: u32,
    height: u32,
    frame_count: u32,
    mip_count: u32,
    texture_format: WasmTextureFormat,
) -> u64 {
    let compression_denominator = match texture_format.into() {
        TextureFormat::DXT1 => 2,
        TextureFormat::DXT5 => 1,
    };
    source_spray_compiler_core::vtf::file_size(width, height, frame_count, mip_count, compression_denominator)
}

#[wasm_bindgen]
pub struct OptimalParameters {
    pub mn_res_lower: u32,
    pub mn_res_greater: u32,
    pub mip_count: u32,
}

#[wasm_bindgen]
pub fn find_optimal_parameters(
    frame_count: u32,
    min_mip_count: u32,
    lowest_mip_res: Option<u32>,
    desired_res: u32,
    texture_format: WasmTextureFormat,
    size_limit: u32,
) -> Result<Option<OptimalParameters>, JsError> {
    if frame_count == 0 {
        return Err(JsError::new("frame_count must be greater than 0"));
    }
    
    let compression_denominator = match texture_format.into() {
        TextureFormat::DXT1 => 2,
        TextureFormat::DXT5 => 1,
    };
    
    Ok(
        find_lresolution(
            frame_count,
            min_mip_count,
            lowest_mip_res,
            desired_res,
            compression_denominator,
            u64::from(size_limit),
        )
        .map(
            |((mn_res_lower, mn_res_greater), mip_count)| OptimalParameters {
                mn_res_lower,
                mn_res_greater,
                mip_count,
            },
        )
    )
}

#[wasm_bindgen]
pub struct WasmImages {
    images: Vec<RgbaImage>,
}

#[wasm_bindgen]
impl WasmImages {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmImages {
        WasmImages { images: Vec::new() }
    }

    pub fn push_image(&mut self, width: u32, height: u32, rgba: &[u8]) -> Result<(), JsError> {
        self.images.push(RgbaImage::from_raw(width, height, rgba.to_vec())
            .ok_or(JsError::new(&format!("Couldn't create rgba image from width: {width}, height: {height}, length: {}", rgba.len())))?);
        Ok(())
    }
}

#[wasm_bindgen]
pub fn export_vtf(
    images: &WasmImages,
    used_mips: Vec<u8>,
    mn_width: u32,
    mn_height: u32,
    mip_count: u32,
    frame_count: u32,
    texture_format: WasmTextureFormat,
) -> Result<Vec<u8>, JsError> {
    let images = &images.images;
    let texture_format = texture_format.into();
    
    if frame_count == 0 {
        return Err(JsError::new("frame_count must be greater than 0"));
    }
    if used_mips.iter().any(|&mip| u32::from(mip) > mip_count) {
        return Err(JsError::new("used_mips cannot contain entries greater than mip_count"));
    }
    if mn_width == 0 || mn_height == 0 {
        return Err(JsError::new("mn_width and mn_height must be greater than 0"));
    }
    if mn_width % 4 != 0 || mn_height % 4 != 0 {
        return Err(JsError::new("mn_width and mn_height must be multiples of 4"));
    }
    let expected_image_count = (frame_count * (mip_count + 1)) as usize;
    let image_count = images.len();
    if expected_image_count != image_count {
        return Err(JsError::new(&format!("Expected {expected_image_count} images for frame_count={frame_count} and mip_count={mip_count}, got {image_count}")));
    }
    
    let images: Vec<_> = images
        .chunks_exact(frame_count as usize)
        .collect();

    let m0_width = mn_width << mip_count;
    let m0_height = mn_height << mip_count;

    let mut dest = Vec::new();

    write_vtf(
        &mut dest,
        &images,
        &used_mips,
        m0_width,
        m0_height,
        frame_count,
        texture_format,
    )
        .map_err(|e| JsError::new(&format!("Failed to write vtf: {e:#}")))?;

    Ok(dest)
}
