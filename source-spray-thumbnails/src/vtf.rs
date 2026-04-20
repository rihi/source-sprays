use crate::vtf::VtfError::{InvalidImageDimensionForImageFormat, InvalidSignature, NoHighResImageData, UnsupportedImageFormat, UnsupportedVersion};
use std::io::{Read, Seek, SeekFrom};
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

pub(crate) enum ImageFormat {
	DXT1,
	DXT5
}

impl ImageFormat {
	pub(crate) fn required_memory(
		&self,
		width: usize,
		height: usize,
	) -> Result<usize, ()> {
		if width % 4 != 0 || height % 4 != 0 {
			return Err(());
		}
		Ok(match self {
			ImageFormat::DXT1 => (width * height) / 2,
			ImageFormat::DXT5 => width * height
		})
	}
}

impl TryFrom<u32> for ImageFormat {
	type Error = ();

	fn try_from(value: u32) -> Result<Self, Self::Error> {
		match value { 
			13 => Ok(ImageFormat::DXT1),
			15 => Ok(ImageFormat::DXT5),
			_ => Err(())
		}
	}
}

pub(crate) struct VtfData {
	pub(crate) width: u16,
	pub(crate) height: u16,
	pub(crate) frame_count: u16,
	pub(crate) first_frame_index: u16,
	pub(crate) high_res_image_format: ImageFormat,
	pub(crate) mipmap_count: u8,
	pub(crate) used_mips: Option<Vec<u8>>,
	pub(crate) images: Vec<Vec<Vec<u8>>>,
}

pub(crate) fn read_vtf<R: Read + Seek>(mut reader: R) -> Result<VtfData, VtfError> {
	let mut header = [0u8; 16];
	reader.read_exact(&mut header)?;
	
	if &header[0..4] != b"VTF\0" {
		return Err(InvalidSignature { signature: header[0..4].try_into().unwrap() });
	}

	let version_major = u32::from_le_bytes(header[4..8].try_into().unwrap());
	let version_minor = u32::from_le_bytes(header[8..12].try_into().unwrap());
	let header_size = u32::from_le_bytes(header[12..16].try_into().unwrap());
	
	let expected_header_size = match (version_major, version_minor) { 
		(7, 0..=1) => 64,
		(7, 2) => 80,
		(7, 3..=5) => 80,
		_ => return Err(UnsupportedVersion { major: version_major, minor: version_minor })
	};
	
	let mut rest = vec![0u8; (expected_header_size - 16) as usize];
	reader.read_exact(&mut rest)?;

	let width = u16::from_le_bytes(rest[0..2].try_into().unwrap());
	let height = u16::from_le_bytes(rest[2..4].try_into().unwrap());
	
	let frame_count = u16::from_le_bytes(rest[8..10].try_into().unwrap());
	let first_frame_index = u16::from_le_bytes(rest[10..12].try_into().unwrap());

	let high_res_image_format = u32::from_le_bytes(rest[36..40].try_into().unwrap());
	let high_res_image_format = ImageFormat::try_from(high_res_image_format)
		.map_err(|_| UnsupportedImageFormat { format: high_res_image_format })?; 
	let mipmap_count = rest[40];
	
	let (metadata_offset, high_res_offset) = if version_minor >= 3 {
		let num_resources = u32::from_le_bytes(rest[52..56].try_into().unwrap());

		let resources: Vec<_> = (0..num_resources)
			.map(|_| {
				let mut res_entry = [0u8; 8];
				reader.read_exact(&mut res_entry)?;
				let tag = u32::from_le_bytes(res_entry[0..4].try_into().unwrap());
				let offset = u32::from_le_bytes(res_entry[4..8].try_into().unwrap());
				Ok((tag, offset))
			})
			.collect::<Result<_, VtfError>>()?;
		
		let metadata_offset = resources.iter()
			.find(|(tag, _)| *tag == u32::from_le_bytes(*b"SDT\x00"))
			.map(|(_, offset)| *offset as u64);
		let (_, high_res_offset) = resources.iter()
			.find(|(tag, _)| *tag == 0x30)
			.ok_or(NoHighResImageData)?;
		
		(metadata_offset, *high_res_offset as u64)
	} else {
		(None, header_size as u64) // could probably be expected_header_size
	};
	
	let used_mips = match metadata_offset {
		None => None,
		Some(metadata_offset) => {
			reader.seek(SeekFrom::Start(metadata_offset))?;

			let mut size_bytes = [0u8; 4];
			reader.read_exact(&mut size_bytes)?;
			
			let size = u32::from_le_bytes(size_bytes);
			if size < 1 {
				None
			} else {
				let mut data = vec![0u8; size as usize];
				reader.read_exact(&mut data)?;

				let version = data[0];

				match version {
					0 => {
						let count = data[1] as usize;
						let indices = &data[2..(2 + count)];
						Some(indices.to_vec())
					},
					_ => None,
				}
			}
		},
	};

	reader.seek(SeekFrom::Start(high_res_offset))?;

	let mut images = (0..mipmap_count)
		.rev()
		.map(|mip| (0..frame_count)
			.map(|_| {
				let w = (width >> mip).max(4);
				let h = (height >> mip).max(4);

				let size = high_res_image_format.required_memory(w as usize, h as usize)
					.map_err(|_| InvalidImageDimensionForImageFormat)?;
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
		used_mips,
		images,
	})
}