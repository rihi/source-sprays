use crate::image_util::{compress, has_transparency, resize};
use color_eyre::eyre;
use color_eyre::eyre::{OptionExt, WrapErr};
use image::RgbaImage;
use std::cmp::max;
use std::io::Write;

#[derive(Copy, Clone, Debug)]
pub enum TextureFormat {
	Bc1,
	Bc3,
}

pub fn write_vtf(
	mut dest: impl Write,
	images: &[Option<&[RgbaImage]>], 
	lowest_mip_resolution: Option<u32>,
	size_limit: u32,
) -> eyre::Result<()> {
	let is_transparent = images.iter()
		.flatten()
		.copied()
		.flatten()
		.any(|img| has_transparency(img));

	let main_images = images[0].unwrap();
	let main_first_frame = &main_images[0];
	
	let texture_format = if is_transparent { TextureFormat::Bc3 } else { TextureFormat::Bc1 };
	let frame_count = main_images.len() as u32;
	let minimum_mip_count = images.len() as u32 - 1;
	let maximum_mip_count = match lowest_mip_resolution {
		None => Some(minimum_mip_count),
		Some(_) => None
	};
	let compression_denominator = if is_transparent { 1 } else { 2 };

	let ((l_res_lower, l_res_greater), mip_count) = find_lresolution(
		frame_count,
		minimum_mip_count,
		maximum_mip_count,
		lowest_mip_resolution.unwrap_or(max(main_first_frame.width(), main_first_frame.height())),
		compression_denominator,
		(size_limit * 1024) as u64,
	)
		.ok_or_eyre("No possible resolution for given parameters")?;

	let (target_l_width, target_l_height) = if main_first_frame.width() <= main_first_frame.height() {
		(l_res_greater, l_res_lower)
	} else {
		(l_res_lower, l_res_greater)
	};

	let target_width = target_l_width << mip_count;
	let target_height = target_l_height << mip_count;
	
	let images = (0..=mip_count as usize)
		.scan(None, |prev, i| {
			if let Some(&img) = images.get(i).flatten_ref() {
				*prev = Some(img);
			}
			*prev
		})
		.collect::<Vec<_>>();
	
	write_vtf_header(&mut dest, target_width, target_height, texture_format, mip_count, frame_count)
		.wrap_err("Failed to write vtf header")?;
	
	for (mip_level, frames) in images.into_iter().enumerate().rev() {
		let mip_level = mip_level as u32;
		let w = target_l_width << (mip_count - mip_level);
		let h = target_l_height << (mip_count - mip_level);
		
		for frame in frames {
			let resized = resize(frame, w, h);
			// let compressed = compress(&resized, texture_format, nvcompress_file)
			// 	.wrap_err("Failed to compress image")?;
			let compressed = compress(&resized, texture_format);

			dest.write_all(&compressed)
				.wrap_err("Failed to write compressed image")?;
		}
	}
	
	Ok(())
}

fn write_vtf_header(
    mut dest: impl Write,
    width: u32,
    height: u32,
    texture_format: TextureFormat,
    mip_count: u32,
    frame_count: u32,
) -> std::io::Result<()> {
	if width % 4 != 0 { panic!("width not multiple of 4"); }
	if height % 4 != 0 { panic!("height not multiple of 4"); }

	let img_fmt_id = match texture_format {
		TextureFormat::Bc1 => 13u32,
		TextureFormat::Bc3 => 15u32,
	};

	let mut flags = 0x0000u32;
	flags |= 0x0004; // clamp s
	flags |= 0x0008; // clamp t
	flags |= 0x0040; // srgb
	if mip_count == 0 {
		flags |= 0x0100; // no mip map
		flags |= 0x0200; // no level of detail
	}
	flags |= match texture_format {
		TextureFormat::Bc1 => 0x1000, // 1 bit alpha
		TextureFormat::Bc3 => 0x2000, // 8 bit alpha
	};

	let first_frame = 0u16;
	let reflectivity = [1.0f32, 1.0f32, 1.0f32];
	let bumpmap_scale = 1.0f32;
	let depth = 1u16;

	dest.write_all(b"VTF\0")?;
	dest.write_all(&7u32.to_le_bytes())?;
	dest.write_all(&4u32.to_le_bytes())?;
	dest.write_all(&88u32.to_le_bytes())?;
	dest.write_all(&(width as u16).to_le_bytes())?;
	dest.write_all(&(height as u16).to_le_bytes())?;
	dest.write_all(&flags.to_le_bytes())?;
	dest.write_all(&(frame_count as u16).to_le_bytes())?;
	dest.write_all(&first_frame.to_le_bytes())?;
	dest.write_all(&[0, 0, 0, 0])?;
	for r in reflectivity.iter() {
		dest.write_all(&r.to_le_bytes())?;
	}
	dest.write_all(&[0, 0, 0, 0])?;
	dest.write_all(&bumpmap_scale.to_le_bytes())?;
	dest.write_all(&img_fmt_id.to_le_bytes())?;
	dest.write_all(&((mip_count + 1) as u8).to_le_bytes())?;
	dest.write_all(&0xFFFFFFFFu32.to_le_bytes())?; // -1 Low res format
	dest.write_all(&0u8.to_le_bytes())?; // Low res width
	dest.write_all(&0u8.to_le_bytes())?; // Low res height
	dest.write_all(&depth.to_le_bytes())?;
	dest.write_all(&[0, 0, 0])?;
	dest.write_all(&1u32.to_le_bytes())?;
	dest.write_all(&[0, 0, 0, 0, 0, 0, 0, 0])?;

	let rsrc_offset = 88u32;
	dest.write_all(b"\x30\x00\x00\x00")?;
	dest.write_all(&rsrc_offset.to_le_bytes())?;
	
	Ok(())
}

fn file_size(
	width: u32,
	height: u32,
	frame_count: u32,
	mip_count: u32,
	compression_denominator: u32
) -> u64 {
	// Calculate the geometric sum for mipmaps: (4^(n+1) - 1) / 3
	let mip_factor = (4u64.pow(mip_count + 1) - 1) / 3;

	let w = width as u64;
	let h = height as u64;
	let f = frame_count as u64;
	let cr = compression_denominator as u64;

	88 + (f * mip_factor * w * h) / cr
}

fn max_mip_count(
	width: u32,
	height: u32,
	frame_count: u32,
	compression_denominator: u32,
	size_limit: u64
) -> Option<u32> {
	(0..)
		.take_while(|&mip_count| {
			let file_size = file_size(width, height, frame_count, mip_count, compression_denominator);
			file_size <= size_limit
		})
		.last()
}

fn find_lresolution(
	frame_count: u32,
	minimum_mip_count: u32,
	maximum_mip_count: Option<u32>,
	maximum_l_resolution: u32,
	compression_denominator: u32,
	size_limit: u64,
) -> Option<((u32, u32), u32)> {
	let maximum_mip_count = maximum_mip_count.unwrap_or(u32::MAX);
	
	let (res0, _) = (0..(maximum_l_resolution / 4))
		.map(|res| (res + 1) * 4)
		.filter_map(|res| {
			let mip_count = max_mip_count(res, res, frame_count, compression_denominator, size_limit)?;
			Some((res, mip_count))
		})
		.filter(|&(_, m)| m >= minimum_mip_count)
		.map(|(r, m)| (r, m.min(maximum_mip_count)))
		.max_by_key(|&(r, m)| (r * r * 4u32.pow(m), m as i32 * -1))?;
	
	let (res1, mip_count1) = (0..(maximum_l_resolution / 4))
		.map(|res| (res + 1) * 4)
		.filter_map(|res| {
			let mip_count = max_mip_count(res0, res, frame_count, compression_denominator, size_limit)?;
			Some((res, mip_count))
		})
		.filter(|&(_, m)| m >= minimum_mip_count)
		.map(|(r, m)| (r, m.min(maximum_mip_count)))
		.max_by_key(|&(r, m)| (res0 * r * 4u32.pow(m), m))?;

	Some(if res0 < res1 {
		((res0, res1), mip_count1)
	} else {
		((res1, res0), mip_count1)
	})
}