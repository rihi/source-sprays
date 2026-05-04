use color_eyre::eyre;
use color_eyre::eyre::WrapErr;
use fast_image_resize::Resizer;
use image::metadata::Cicp;
use image::{ConvertColorOptions, ImageReader, RgbaImage};
use std::path::Path;

pub fn load_image(file: &Path) -> eyre::Result<RgbaImage> {
	let img = ImageReader::open(file)
		.wrap_err_with(|| format!("Failed to load image '{}'", file.display()))?
		.decode()
		.wrap_err_with(|| format!("Failed to decode image '{}'", file.display()))?;
	
	let mut img = img.into_rgba8();
	img.apply_color_space(Cicp::SRGB_LINEAR, ConvertColorOptions::default())
		.wrap_err("Failed to convert to linear colorspace")?;
	
	Ok(img)
}

pub fn has_transparency(img: &RgbaImage) -> bool {
	img.pixels().any(|p| p[3] < 255)
}

pub fn calculate_dimensions(
	src_width: f64,
	src_height: f64,
	dst_width: f64,
	dst_height: f64,
) -> (f64, f64, f64, f64) {
	let src_aspect = src_width / src_height;
	let dst_aspect = dst_width / dst_height;

	let (crop_width, crop_height) = if src_aspect > dst_aspect {
		(src_width, src_width / dst_aspect)
	} else {
		(src_height * dst_aspect, src_height)
	};

	// Center the crop box within the source dimensions
	let left = (crop_width - src_width) * 0.5;
	let top = (crop_height - src_height) * 0.5;

	(left, top, crop_width, crop_height)
}

pub fn resize(img: &RgbaImage, width: u32, height: u32) -> RgbaImage {
	let (left, top, c_width, c_height) = calculate_dimensions(
		img.width() as f64,
		img.height() as f64,
		width as f64,
		height as f64
	);
	
	let mut canvas = RgbaImage::new(c_width as u32, c_height as u32);
	canvas.set_color_space(img.color_space()).unwrap();
	
	image::imageops::overlay(&mut canvas, img, left as i64, top as i64);
	
	let mut canvas_resized = RgbaImage::new(width, height);
	canvas_resized.set_color_space(canvas.color_space()).unwrap();
	
	let mut resizer = Resizer::new();
	resizer.resize(
		&canvas,
		&mut canvas_resized,
		None
	).unwrap();
	
	canvas_resized
}