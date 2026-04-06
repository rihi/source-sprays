use crate::vtf::TextureFormat;
use image::{GenericImageView, RgbaImage};
use texpresso::{Algorithm, Params, COLOUR_WEIGHTS_PERCEPTUAL};

pub fn has_transparency(img: &RgbaImage) -> bool {
	img.pixels().any(|p| p[3] < 255)
}

pub fn calculate_dimensions(
	img: &impl GenericImageView,
	target_width: u32,
	target_height: u32
) -> (u32, u32) {
	let (width, height) = img.dimensions();
	if width > height {
		let new_w = target_width;
		let new_h = (target_height as f32 * (height as f32 / width as f32)) as u32;
		(new_w, new_h)
	} else {
		let new_w = (target_width as f32 * (width as f32 / height as f32)) as u32;
		let new_h = target_height;
		(new_w, new_h)
	}
}

pub fn resize(img: &RgbaImage, width: u32, height: u32) -> RgbaImage {
	let (new_w, new_h) = calculate_dimensions(img, width, height);
	let resized_img = image::imageops::resize(img, new_w, new_h, image::imageops::FilterType::Lanczos3);

	let mut canvas = RgbaImage::new(width, height);
	let offset_x = (width - new_w) / 2;
	let offset_y = (height - new_h) / 2;

	image::imageops::overlay(&mut canvas, &resized_img, offset_x as i64, offset_y as i64);
	canvas
}

pub fn compress(
	img: &RgbaImage,
	format: TextureFormat,
) -> Box<[u8]> {
	let format = match format {
		TextureFormat::Bc1 => texpresso::Format::Bc1,
		TextureFormat::Bc3 => texpresso::Format::Bc3,
	};
	
	let width = img.width() as usize;
	let height = img.height() as usize;
	
	let mut output = vec![0u8; format.compressed_size(width, height)]
		.into_boxed_slice();
	
	format.compress(
		img,
		img.width() as usize,
		img.height() as usize,
		Params {
			algorithm: Algorithm::IterativeClusterFit,
			weights: COLOUR_WEIGHTS_PERCEPTUAL,
			weigh_colour_by_alpha: true
		},
		&mut output,
	);
	
	output
}