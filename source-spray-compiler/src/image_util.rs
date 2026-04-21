use image::RgbaImage;

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
	image::imageops::resize(&canvas, width, height, image::imageops::FilterType::Lanczos3)
}