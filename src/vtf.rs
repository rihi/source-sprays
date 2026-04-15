use crate::image_util::{compress, has_transparency, resize};
use color_eyre::eyre;
use color_eyre::eyre::{OptionExt, WrapErr};
use image::RgbaImage;
use std::cmp::max;
use std::io::{Read, Seek, SeekFrom, Write};
use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum VtfError {
	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),

	#[error("Invalid VTF signature: {signature:?}")]
	InvalidSignature { signature: [u8; 4] },

	#[error("Unsupported VTF version {major}.{minor}")]
	UnsupportedVersion { major: u32, minor: u32 },

	#[error("Unsupported image format: {format}")]
	UnsupportedImageFormat { format: u32 },

	#[error("Invalid image dimensions for image format")]
	InvalidImageDimensionForImageFormat,

	#[error("No High-res image data resource")]
	NoHighResImageData
}

#[derive(Copy, Clone, Debug)]
pub enum TextureFormat {
	Bc1,
	Bc3,
}

impl TextureFormat {
	pub(crate) fn required_memory(
		&self,
		width: usize,
		height: usize,
	) -> Result<usize, ()> {
		if width % 4 != 0 || height % 4 != 0 {
			return Err(());
		}
		Ok(match self {
			TextureFormat::Bc1 => (width * height) / 2,
			TextureFormat::Bc3 => width * height
		})
	}
}

impl TryFrom<u32> for TextureFormat {
	type Error = ();

	fn try_from(value: u32) -> Result<Self, Self::Error> {
		match value {
			13 => Ok(TextureFormat::Bc1),
			15 => Ok(TextureFormat::Bc3),
			_ => Err(())
		}
	}
}

pub(crate) struct VtfData {
	pub(crate) width: u16,
	pub(crate) height: u16,
	pub(crate) frame_count: u16,
	pub(crate) first_frame_index: u16,
	pub(crate) high_res_image_format: TextureFormat,
	pub(crate) mipmap_count: u8,
	pub(crate) images: Vec<Vec<Vec<u8>>>,
}


pub(crate) fn read_vtf(mut reader: impl Read + Seek) -> Result<VtfData, VtfError> {
	let mut header = [0u8; 16];
	reader.read_exact(&mut header)?;

	if &header[0..4] != b"VTF\0" {
		return Err(VtfError::InvalidSignature { signature: header[0..4].try_into().unwrap() });
	}

	let version_major = u32::from_le_bytes(header[4..8].try_into().unwrap());
	let version_minor = u32::from_le_bytes(header[8..12].try_into().unwrap());
	let header_size = u32::from_le_bytes(header[12..16].try_into().unwrap());

	let expected_header_size = match (version_major, version_minor) {
		(7, 0..=1) => 64,
		(7, 2) => 80,
		(7, 3..=5) => 80,
		_ => return Err(VtfError::UnsupportedVersion { major: version_major, minor: version_minor })
	};

	let mut rest = vec![0u8; (expected_header_size - 16) as usize];
	reader.read_exact(&mut rest)?;

	let width = u16::from_le_bytes(rest[0..2].try_into().unwrap());
	let height = u16::from_le_bytes(rest[2..4].try_into().unwrap());

	let frame_count = u16::from_le_bytes(rest[8..10].try_into().unwrap());
	let first_frame_index = u16::from_le_bytes(rest[10..12].try_into().unwrap());

	let high_res_image_format = u32::from_le_bytes(rest[36..40].try_into().unwrap());
	let high_res_image_format = TextureFormat::try_from(high_res_image_format)
		.map_err(|_| VtfError::UnsupportedImageFormat { format: high_res_image_format })?;
	let mipmap_count = rest[40];

	let high_res_offset = if version_minor >= 3 {
		let num_resources = u32::from_le_bytes(rest[52..56].try_into().unwrap());
		let (_, offset) = (0..num_resources)
			.map(|_| {
				let mut res_entry = [0u8; 8];
				reader.read_exact(&mut res_entry)?;
				let tag = u32::from_le_bytes(res_entry[0..4].try_into().unwrap());
				let offset = u32::from_le_bytes(res_entry[4..8].try_into().unwrap());
				Ok((tag, offset))
			})
			.find(|res: &Result<_, VtfError>| matches!(res, Err(_) | Ok((0x30, _))))
			.ok_or_else(|| VtfError::NoHighResImageData)??;
		offset as u64
	} else {
		header_size as u64 // could probably be expected_header_size
	};

	reader.seek(SeekFrom::Start(high_res_offset))?;

	let mut images = (0..mipmap_count)
		.rev()
		.map(|mip| (0..frame_count)
			.map(|_| {
				let w = (width >> mip).max(1);
				let h = (height >> mip).max(1);

				let size = high_res_image_format.required_memory(w as usize, h as usize)
					.map_err(|_| VtfError::InvalidImageDimensionForImageFormat)?;
				let mut vec = vec![0u8; size];
				reader.read_exact(&mut vec)?;
				Ok(vec)
			})
			.collect::<Result<Vec<_>, VtfError>>())
		.collect::<Result<Vec<_>, _>>()?;

	images.reverse();

	Ok(VtfData {
		width,
		height,
		frame_count,
		first_frame_index,
		high_res_image_format,
		mipmap_count,
		images,
	})
}

pub fn write_vtf(
	mut dest: impl Write,
	images: &[Option<&[RgbaImage]>], 
	lowest_mip_resolution: Option<u32>,
	size_limit: u64,
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
	let compression_denominator = if is_transparent { 1 } else { 2 };

	let ((l_res_lower, l_res_greater), mip_count) = find_lresolution(
		frame_count,
		minimum_mip_count,
		lowest_mip_resolution,
		max(main_first_frame.width(), main_first_frame.height()),
		compression_denominator,
		size_limit,
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

fn find_lresolution(
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
			let pixel_count = r * r * 4u32.pow(m);
			match r * 2u32.pow(m) >= desired_resolution {
				false => (false, pixel_count, u32::MAX - m),
				true => (true, u32::MAX - pixel_count, u32::MAX - m),
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
			let pixel_count = res0 * r * 4u32.pow(m);
			match r * 2u32.pow(m) >= desired_resolution {
				false => (false, pixel_count, u32::MAX - m),
				true => (true, u32::MAX - pixel_count, u32::MAX - m),
			}
		})?;

	Some(if res0 < res1 {
		((res0, res1), mip_count1)
	} else {
		((res1, res0), mip_count1)
	})
}