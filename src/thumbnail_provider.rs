use crate::imaging::{thumbnail_animation, thumbnail_mips};
use crate::module::ComLock;
use crate::vtf::read_vtf;
use crate::winstream::WinStream;
use std::cell::OnceCell;
use std::io::{Seek, SeekFrom};
use windows::{
	Win32::Foundation::{ERROR_ALREADY_INITIALIZED, E_FAIL},
	Win32::Graphics::Gdi::{CreateDIBSection, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS},
	Win32::System::Com::IStream,
	Win32::UI::Shell::PropertiesSystem::{IInitializeWithStream, IInitializeWithStream_Impl},
	Win32::UI::Shell::{IThumbnailProvider, IThumbnailProvider_Impl, WTSAT_ARGB, WTS_ALPHATYPE}
};
use windows_core::{implement, Ref, GUID};

pub(crate) const CLSID: GUID = GUID::from_u128(0xd74f9231_f833_4078_8aa2_1d5de684a16e);

#[implement(IThumbnailProvider, IInitializeWithStream)]
pub(crate) struct ThumbnailProvider {
	_lock: ComLock,
	stream: OnceCell<IStream>
}

impl ThumbnailProvider {
	pub(crate) fn new() -> Self {
		Self {
			_lock: ComLock::new(),
			stream: OnceCell::new()
		}
	}
}

impl IThumbnailProvider_Impl for ThumbnailProvider_Impl {
	#[allow(non_snake_case)]
	fn GetThumbnail(
		&self,
		_cx: u32,
		phbmp: *mut windows::Win32::Graphics::Gdi::HBITMAP,
		pdwalpha: *mut WTS_ALPHATYPE
	) -> windows_core::Result<()> {
		let istream = self.stream.get().ok_or(E_FAIL)?;
		let mut stream = WinStream::from(istream);
		stream.seek(SeekFrom::Start(0)).map_err(|_| E_FAIL)?;

		let vtf = read_vtf(stream)
			.map_err(|_| E_FAIL)?;
		
		let treat_as_square = true;
		let thumbnail = match vtf.frame_count { 
			1 => thumbnail_mips(&vtf, treat_as_square),
			_ => thumbnail_animation(&vtf, treat_as_square)
		}
			.map_err(|_| E_FAIL)?;
		
		// if let Some((
		// 	inner_x_start,
		// 	inner_y_start,
		// 	inner_x_end,
		// 	inner_y_end,
		// )) = find_inner_bounds(&thumbnail) {
		// 	thumbnail = thumbnail
		// 		.view(
		// 			inner_x_start,
		// 			inner_y_start,
		// 			inner_x_end - inner_x_start,
		// 			inner_y_end - inner_y_start
		// 		)
		// 		.to_image()
		// }

		unsafe {
			if !pdwalpha.is_null() {
				*pdwalpha = WTSAT_ARGB;
			}

			// Define the DIB Header for a 32-bit BGRA bitmap
			let bmi = BITMAPINFO {
				bmiHeader: BITMAPINFOHEADER {
					biSize: size_of::<BITMAPINFOHEADER>() as u32,
					biWidth: thumbnail.width() as i32,
					biHeight: -(thumbnail.height() as i32), // Top-down bitmap (negative height)
					biPlanes: 1,
					biBitCount: 32,
					biCompression: BI_RGB.0, // BI_RGB
					..Default::default()
				},
				..Default::default()
			};

			let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
			let hbitmap = CreateDIBSection(
				None,
				&bmi,
				DIB_RGB_COLORS,
				&mut bits,
				None,
				0,
			)?;

			assert!(!bits.is_null());
			
			let pixel_ptr = bits as *mut u32;
			
			let (chunks, _) = thumbnail.as_chunks::<4>();
			for (i, &[r, g, b, a]) in chunks.iter().enumerate() {
				*pixel_ptr.add(i) = u32::from_le_bytes([b, g, r, a]);
			}

			// let total_pixels = (thumbnail.width() * thumbnail.height()) as usize;
			// for i in 0..total_pixels {
			// 	// 0xFFFF0000 is Blue in Little-Endian BGRA (Alpha: FF, Red: 00, Green: 00, Blue: FF)
			// 	// Note: Windows Gdi colors are often 0xAARRGGBB in memory
			// 	*pixel_ptr.add(i) = 0xFFFF0000;
			// }

			*phbmp = hbitmap;
		}
		Ok(())
	}
}

impl IInitializeWithStream_Impl for ThumbnailProvider_Impl {
	#[allow(non_snake_case)]
	fn Initialize(&self, pstream: Ref<IStream>, _grfmode: u32) -> windows_core::Result<()> {
		self.stream
			.set(pstream.ok()?.clone())
			.map_err(|_| ERROR_ALREADY_INITIALIZED)?;

		Ok(())
	}
}

// pub(crate) fn log(msg: &str) {
// 	let mut f = OpenOptions::new()
// 		.create(true)
// 		.append(true)
// 		.open("C:\\Temp\\thumbnail.log")
// 		.unwrap();
// 	writeln!(f, "{}", msg).unwrap();
// }