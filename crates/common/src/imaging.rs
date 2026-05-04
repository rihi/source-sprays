use crate::vtf::TextureFormat;
use image::imageops::FilterType;
use image::RgbaImage;
use texpresso::{Algorithm, Params, COLOUR_WEIGHTS_PERCEPTUAL};

pub fn compress(
	img: &RgbaImage,
	format: TextureFormat,
) -> Box<[u8]> {
	let format = match format {
		TextureFormat::DXT1 => texpresso::Format::Bc1,
		TextureFormat::DXT5 => texpresso::Format::Bc3,
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

pub fn decompress(
	width: u32,
	height: u32,
	format: TextureFormat,
	data: &[u8],
) -> RgbaImage {
	let format = match format {
		TextureFormat::DXT1 => texpresso::Format::Bc1,
		TextureFormat::DXT5 => texpresso::Format::Bc3,
	};

	let mut image = RgbaImage::new(width, height);
	format.decompress(data, width as usize, height as usize, &mut image);
	image
}

pub fn stretch_square(img: &RgbaImage) -> RgbaImage {
	let size = img.width().max(img.height());
	image::imageops::resize(
		img,
		size,
		size,
		FilterType::Triangle, // good quality + fast
	)
}

pub fn find_inner_bounds(img: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
	let (width, height) = img.dimensions();
	let buf = img.as_raw();

	let non_transparent = |x: u32, y: u32| {
		let idx = ((y * width + x) * 4 + 3) as usize;
		buf[idx] != 0
	};
	
	let min_y = (0..height)
		.find(|&y| (0..width).any(|x| non_transparent(x, y)))?;

	let max_y = (min_y..height)
		.rev()
		.find(|&y| (0..width).any(|x| non_transparent(x, y)))? + 1;

	let min_x = (0..width)
		.find(|&x| (min_y..max_y).any(|y| non_transparent(x, y)))?;

	let max_x = (min_x..width)
		.rev()
		.find(|&x| (min_y..max_y).any(|y| non_transparent(x, y)))? + 1;

	Some((min_x, min_y, max_x, max_y))
}