use crate::image_util::decompress;
use crate::vtf::VtfData;
use image::imageops::FilterType;
use image::{GenericImageView, RgbaImage};
use std::cmp::max;
use std::iter::zip;
use std::ops::Deref;

pub(crate) fn stretch_square(img: &RgbaImage) -> RgbaImage {
	let size = img.width().max(img.height());
	image::imageops::resize(
		img,
		size,
		size,
		FilterType::Triangle, // good quality + fast
	)
}

pub(crate) fn find_inner_bounds(img: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
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

pub(crate) fn thumbnail_animation(
	vtf: &VtfData,
	treat_as_square: bool,
) -> RgbaImage {
	let base = vtf.first_frame_index as usize;
	let count = vtf.frame_count.min(3) as usize;

	let mut images: Vec<_> = (0..count)
		.map(|frame| {
			decompress(
				vtf.width as u32,
				vtf.height as u32,
				vtf.high_res_image_format,
				&vtf.images[0][(base + frame) % vtf.frame_count as usize],
			)
		})
		.collect();

	for (frame, img) in images.iter_mut().enumerate() {
		let progress = frame as f64 / (count - 1) as f64;
		for pixel in img.pixels_mut() {
			pixel[3] = (pixel[3] as f64 * (1.0 - 0.5 * progress)) as u8;
		}
	}

	if treat_as_square {
		for img in &mut images {
			*img = stretch_square(img);
		}
	}

	let frame_size = max(vtf.width, vtf.height) as u32;
	let (
		inner_x_start,
		_inner_y_start,
		inner_x_end,
		_inner_y_end,
	) = find_inner_bounds(&images[0])
		.unwrap_or((0, 0, frame_size, frame_size));

	let positions: Vec<_> = images.iter()
		.enumerate()
		.map(|(frame, _)| (
			((inner_x_end - inner_x_start) / 4) * frame as u32,
			0,
		))
		.collect();

	let images_cropped: Vec<_> = zip(positions, &images)
		.map(|(pos, img)| {
			let (x_pos, y_pos) = pos;
			let (
				inner_x_start,
				inner_y_start,
				inner_x_end,
				inner_y_end,
			) = find_inner_bounds(img).unwrap_or((0, 0, 0, 0));
			(
				(x_pos + inner_x_start, y_pos + inner_y_start),
				img.view(
					inner_x_start,
					inner_y_start,
					inner_x_end - inner_x_start,
					inner_y_end - inner_y_start
				)
			)
		})
		.collect();

	let (
		canvas_x_start,
		canvas_y_start,
		canvas_x_end,
		canvas_y_end,
	) = images_cropped.iter()
		.fold((u32::MAX, u32::MAX, 0, 0), |region, (pos, img)| {
			let (x_start, y_start, x_end, y_end) = region;
			let &(x_pos, y_pos) = pos;
			(
				x_start.min(x_pos),
				y_start.min(y_pos),
				x_end.max(x_pos + img.width()),
				y_end.max(y_pos + img.height()),
			)
		});

	let mut canvas = RgbaImage::new(
		canvas_x_end - canvas_x_start,
		canvas_y_end - canvas_y_start,
	);

	for (pos, img) in images_cropped.iter().rev() {
		let &(x_pos, y_pos) = pos;

		let mut shadow = RgbaImage::new(img.width() + 20, img.height() + 20);
		image::imageops::overlay(&mut shadow, img.deref(), 10, 10);

		for pixel in shadow.pixels_mut() {
			pixel[0] = 0;
			pixel[1] = 0;
			pixel[2] = 0;
			pixel[3] = pixel[3] / 2;
		}
		let shadow = image::imageops::fast_blur(&shadow, 5.0);
		let shadow_offset = (frame_size / 64) as i64;

		let x_paste = (x_pos - canvas_x_start) as i64;
		let y_paste = (y_pos - canvas_y_start) as i64;

		image::imageops::overlay(
			&mut canvas,
			&shadow,
			x_paste + shadow_offset,
			y_paste,
		);

		image::imageops::overlay(
			&mut canvas,
			img.deref(),
			x_paste,
			y_paste,
		)
	}

	canvas
}

pub(crate) fn thumbnail_mips(
	vtf: &VtfData,
	treat_as_square: bool,
) -> RgbaImage {
	let count = vtf.mipmap_count.min(3) as u32;

	let mut images: Vec<_> = (0..count)
		.map(|mip| decompress(
			(vtf.width >> mip).max(1) as u32,
			(vtf.height >> mip).max(1) as u32,
			vtf.high_res_image_format,
			&vtf.images[mip as usize][0],
		))
		.collect();

	if treat_as_square {
		for img in &mut images {
			*img = stretch_square(img);
		}
	}

	let frame_size = max(vtf.width, vtf.height) as u32;
	let (
		inner_x_start,
		inner_y_start,
		inner_x_end,
		inner_y_end,
	) = find_inner_bounds(&images[0])
		.unwrap_or((0, 0, frame_size, frame_size));

	let closed_form_position = |mip: i32, start: f64, end: f64| {
		(start + 2.0 * 0.50 * (end - start)) * (1.0 - 2f64.powi(-mip))
	};

	let positions: Vec<_> = images.iter()
		.enumerate()
		.map(|(mip, _)| (
			closed_form_position(mip as i32, inner_x_start as f64, inner_x_end as f64) as u32,
			closed_form_position(mip as i32, inner_y_start as f64, inner_y_end as f64) as u32,
		))
		.collect();

	let images_cropped: Vec<_> = zip(positions, &images)
		.map(|(pos, img)| {
			let (x_pos, y_pos) = pos;
			let (
				inner_x_start,
				inner_y_start,
				inner_x_end,
				inner_y_end,
			) = find_inner_bounds(img).unwrap_or((0, 0, 0, 0));
			(
				(x_pos + inner_x_start, y_pos + inner_y_start),
				img.view(
					inner_x_start,
					inner_y_start,
					inner_x_end - inner_x_start,
					inner_y_end - inner_y_start
				)
			)
		})
		.collect();

	let (
		canvas_x_start,
		canvas_y_start,
		canvas_x_end,
		canvas_y_end,
	) = images_cropped.iter()
		.fold((u32::MAX, u32::MAX, 0, 0), |region, (pos, img)| {
			let (x_start, y_start, x_end, y_end) = region;
			let &(x_pos, y_pos) = pos;
			(
				x_start.min(x_pos),
				y_start.min(y_pos),
				x_end.max(x_pos + img.width()),
				y_end.max(y_pos + img.height()),
			)
		});

	let mut canvas = RgbaImage::new(
		canvas_x_end - canvas_x_start,
		canvas_y_end - canvas_y_start,
	);

	for (pos, img) in images_cropped.iter() {
		let &(x_pos, y_pos) = pos;
		let mut shadow = RgbaImage::new(img.width() + 20, img.height() + 20);
		image::imageops::overlay(&mut shadow, img.deref(), 10, 10);

		for pixel in shadow.pixels_mut() {
			pixel[0] = 0;
			pixel[1] = 0;
			pixel[2] = 0;
			pixel[3] = pixel[3] / 2;
		}
		let shadow = image::imageops::fast_blur(&shadow, 5.0);
		let shadow_offset = (frame_size / 64) as i64;

		let x_paste = (x_pos - canvas_x_start) as i64;
		let y_paste = (y_pos - canvas_y_start) as i64;

		image::imageops::overlay(
			&mut canvas,
			&shadow,
			x_paste - shadow_offset,
			y_paste - shadow_offset,
		);

		image::imageops::overlay(
			&mut canvas,
			img.deref(),
			x_paste,
			y_paste,
		)
	}

	canvas
}