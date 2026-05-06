use color_eyre::eyre;
use color_eyre::eyre::WrapErr;
use fast_image_resize::Resizer;
use image::metadata::Cicp;
use image::{ConvertColorOptions, ImageReader, RgbaImage};
use std::cmp::max;
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

pub fn resize(
	img: &RgbaImage,
	dst_width: u32,
	dst_heigh: u32
) -> RgbaImage {
	let greater_res = max(img.width(), img.height());
	let mut canvas = RgbaImage::new(greater_res, greater_res);
	canvas.set_color_space(img.color_space()).unwrap();
	
	image::imageops::overlay(
		&mut canvas,
		img,
		((greater_res - img.width()) / 2) as i64,
		((greater_res - img.height()) / 2) as i64
	);
	
	let mut canvas_resized = RgbaImage::new(dst_width, dst_heigh);
	canvas_resized.set_color_space(canvas.color_space()).unwrap();
	
	let mut resizer = Resizer::new();
	resizer.resize(
		&canvas,
		&mut canvas_resized,
		None
	).unwrap();
	
	canvas_resized
}