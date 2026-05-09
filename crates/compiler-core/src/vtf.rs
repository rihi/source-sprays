use crate::image_util::{has_transparency, resize};
use color_eyre::eyre;
use color_eyre::eyre::{eyre, OptionExt, WrapErr};
use image::metadata::Cicp;
use image::{ConvertColorOptions, RgbaImage};
use itertools::Itertools;
use source_spray_common::imaging::compress;
use source_spray_common::vtf::TextureFormat;
use std::cmp::min;
use std::io::Write;

pub fn write_optimal_vtf(
	mut dest: impl Write,
	images: &[Option<&[RgbaImage]>],
	lowest_mip_resolution: Option<u32>,
	desired_resolution: Option<u32>,
	size_limit: u64,
) -> eyre::Result<()> {
	let main_frames = images.iter()
		.find_map(|frames| frames.as_deref())
		.unwrap();
	
	let max_images_resolution = images.iter()
		.enumerate()
		.filter_map(|(mip, frames)| frames.map(|f| (mip, f)))
		.flat_map(|(mip, frames)| frames.iter()
			.flat_map(|img| [img.width(), img.height()])
			.map(move |res| res * 2u32.pow(mip as u32)))
		.max()
		.unwrap();

	let is_transparent = images.iter()
		.flatten()
		.copied()
		.flatten()
		.any(|img| has_transparency(img));
	
	let texture_format = if is_transparent { TextureFormat::DXT5 } else { TextureFormat::DXT1 };
	let frame_count = main_frames.len() as u32;
	let minimum_mip_count = images.len() as u32 - 1;
	let compression_denominator = if is_transparent { 1 } else { 2 };

	let ((l_res_lower, l_res_greater), mip_count) = find_lresolution(
		frame_count,
		minimum_mip_count,
		lowest_mip_resolution,
		min(max_images_resolution, desired_resolution.unwrap_or(u32::MAX)),
		compression_denominator,
		size_limit,
	)
		.ok_or_eyre("No possible resolution for given parameters")?;

	let main_first_frame = &main_frames[0];
	let (target_l_width, target_l_height) = if main_first_frame.width() <= main_first_frame.height() {
		(l_res_greater, l_res_lower)
	} else {
		(l_res_lower, l_res_greater)
	};

	let target_width = target_l_width << mip_count;
	let target_height = target_l_height << mip_count;
	
	let filled_images = (0..=mip_count as usize)
		.scan(None, |prev, i| {
			if let Some(&img) = images.get(i).flatten_ref() {
				*prev = Some(img);
			}
			Some(match prev {
				Some(frames) => frames,
				None => main_frames,
			})
		})
		.collect::<Vec<_>>();
	
	let mip_indices: Vec<_> = images.iter()
		.enumerate()
		.filter_map(move |(i, img)| img.map(move |_| i))
		.map(u8::try_from)
		.try_collect()
		.wrap_err("Mip index doesn't fit into u8")?;

	if mip_indices.len() > 32 - 4 - 2 {
		return Err(eyre!("More than 26 mips"));
	}
	
	write_vtf(
		&mut dest,
		&filled_images,
		&mip_indices,
		target_width,
		target_height,
		frame_count,
		texture_format,
	)?;
	
	Ok(())
}

pub fn write_vtf(
	mut dest: impl Write,
	images: &[&[RgbaImage]],
	used_mips: &[u8],
	m0_width: u32,
	m0_height: u32,
	frame_count: u32,
	texture_format: TextureFormat,
) -> std::io::Result<()> {
	let mip_count = images.len() as u32 - 1;
	
	write_vtf_header(&mut dest, m0_width, m0_height, texture_format, mip_count, frame_count)?;
	write_vtf_sdt_resource(&mut dest, used_mips)?;
	write_vtf_high_res_data(&mut dest, images, texture_format, m0_width, m0_height)?;
	
	Ok(())
}

pub fn write_vtf_header(
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
		TextureFormat::DXT1 => 13u32,
		TextureFormat::DXT5 => 15u32,
	};

	let mut flags = 0x0000u32;
	flags |= 0x0004; // clamp s
	flags |= 0x0008; // clamp t
	flags |= 0x0040; // srgb
	if mip_count == 0 {
		flags |= 0x0100; // no mip map
	}
	flags |= 0x0200; // no level of detail
	flags |= match texture_format {
		TextureFormat::DXT1 => 0x1000, // 1 bit alpha
		TextureFormat::DXT5 => 0x2000, // 8 bit alpha
	};

	let first_frame = 0u16;
	let reflectivity = [1.0f32, 1.0f32, 1.0f32];
	let bumpmap_scale = 1.0f32;
	let depth = 1u16;

	dest.write_all(b"VTF\0")?;
	dest.write_all(&7u32.to_le_bytes())?;
	dest.write_all(&4u32.to_le_bytes())?;
	dest.write_all(&96u32.to_le_bytes())?; // header size
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
	dest.write_all(&2u32.to_le_bytes())?; // num resources
	dest.write_all(&[0, 0, 0, 0, 0, 0, 0, 0])?;

	dest.write_all(b"SDT\x00")?;
	dest.write_all(&96u32.to_le_bytes())?;
	
	dest.write_all(b"\x30\x00\x00\x00")?;
	dest.write_all(&128u32.to_le_bytes())?;
	
	Ok(())
}

pub fn write_vtf_sdt_resource(
	mut dest: impl Write,
	mip_indices: &[u8],
) -> std::io::Result<()> {
	if mip_indices.len() > 32 - 4 - 2 {
		panic!("More than 26 mips");
	}

	let mut mip_indices_data = [0u8; 32];
	mip_indices_data[0..4].copy_from_slice(&(32u32 - 4).to_le_bytes());
	mip_indices_data[4] = 0; // version
	mip_indices_data[5] = mip_indices.len() as u8;
	mip_indices_data[6..(6 + mip_indices.len())].copy_from_slice(&mip_indices);

	dest.write_all(&mip_indices_data)?;
	
	Ok(())
}

pub fn write_vtf_high_res_data(
	mut dest: impl Write,
	images: &[&[RgbaImage]],
	texture_format: TextureFormat,
	m0_width: u32,
	m0_height: u32
) -> std::io::Result<()> {
	for (mip_level, &frames) in images.into_iter().enumerate().rev() {
		let mip_level = mip_level as u32;
		let w = m0_width >> mip_level;
		let h = m0_height >> mip_level;

		for frame in frames {
			let mut resized = resize(frame, w, h);
			resized.apply_color_space(Cicp::SRGB, ConvertColorOptions::default())
				.expect("Failed to convert to srgb color space");
			let compressed = compress(&resized, texture_format);

			dest.write_all(&compressed)?;
		}
	}
	
	Ok(())
}

pub fn file_size(
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

	let vtf_header_size = 80;
	let vtf_resource_dict_size = 2 * 8;
	let vtf_sdt_resource_size = 32;
	let vtf_highres_resource_size = (f * mip_factor * w * h) / cr;

	vtf_header_size + vtf_resource_dict_size + vtf_sdt_resource_size + vtf_highres_resource_size
}

fn mip_counts(
	width: u32,
	height: u32,
	frame_count: u32,
	compression_denominator: u32,
	size_limit: u64
) -> impl Iterator<Item=u32> {
	(0..)
		.take_while(move |&mip_count| {
			let file_size = file_size(width, height, frame_count, mip_count, compression_denominator);
			file_size <= size_limit
		})
}

pub fn find_lresolution(
	frame_count: u32,
	minimum_mip_count: u32,
	maximum_l_resolution: Option<u32>,
	desired_resolution: u32,
	compression_denominator: u32,
	size_limit: u64,
) -> Option<((u32, u32), u32)> {
	let maximum_l_resolution = maximum_l_resolution.unwrap_or(u32::MAX);
	let (res0, _mip_count0) = (0..(maximum_l_resolution / 4))
		.map(|res| (res + 1) * 4)
		.map_while(|res| {
			let mut iter = mip_counts(res, res, frame_count, compression_denominator, size_limit)
				.filter(|&mip_count| mip_count >= minimum_mip_count)
				.map(move |mip_count| (res, mip_count))
				.peekable();

			iter.peek()?;
			Some(iter)
		})
		.flatten()
		.max_by_key(|&(r, m)| {
			let resolution = r * 2u32.pow(m);
			match resolution >= desired_resolution {
				false => (false, resolution, u32::MAX - m),
				true => (true, u32::MAX - resolution, u32::MAX - m),
			}
		})?;
	
	let (res1, mip_count1) = (0..(maximum_l_resolution / 4))
		.map(|res| (res + 1) * 4)
		.map_while(|res| {
			let mut iter = mip_counts(res0, res, frame_count, compression_denominator, size_limit)
				.filter(|&mip_count| mip_count >= minimum_mip_count)
				.map(move |mip_count| (res, mip_count))
				.peekable();
			
			iter.peek()?;
			Some(iter)
		})
		.flatten()
		.max_by_key(|&(r, m)| {
			let resolution0 = res0 * 2u32.pow(m);
			let resolution1 = r * 2u32.pow(m);
			match resolution0 >= desired_resolution && resolution1 >= desired_resolution {
				false => (false, resolution1, u32::MAX - m),
				true => (true, u32::MAX - resolution1, u32::MAX - m),
			}
		})?;

	Some(if res0 < res1 {
		((res0, res1), mip_count1)
	} else {
		((res1, res0), mip_count1)
	})
}