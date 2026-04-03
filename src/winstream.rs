use std::io::{Read, Seek, SeekFrom};
use windows::Win32::System::Com::{IStream, STREAM_SEEK_CUR, STREAM_SEEK_END, STREAM_SEEK_SET};

pub(crate) struct WinStream<'a> {
	stream: &'a IStream,
}

impl<'a> From<&'a IStream> for WinStream<'a> {
	fn from(stream: &'a IStream) -> Self {
		Self { stream }
	}
}

impl Read for WinStream<'_> {
	fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
		let mut bytes_read = 0u32;
		unsafe {
			self.stream.Read(
				buf.as_mut_ptr() as _,
				buf.len() as u32,
				Some(&mut bytes_read),
			)
		}
			.ok()
			.map_err(|err| std::io::Error::other(format!("IStream::Read failed: {}", err.code().0)))?;
		
		Ok(bytes_read as usize)
	}
}

impl Seek for WinStream<'_> {
	fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
		let mut new_pos = 0;
		unsafe {
			self.stream.Seek(
				match pos {
					SeekFrom::Start(seek) => seek as i64,
					SeekFrom::End(seek) => seek,
					SeekFrom::Current(seek) => seek
				},
				match pos {
					SeekFrom::Start(_) => STREAM_SEEK_SET,
					SeekFrom::End(_) => STREAM_SEEK_END,
					SeekFrom::Current(_) => STREAM_SEEK_CUR,
				},
				Some(&mut new_pos)
			)?;
		}

		Ok(new_pos)
	}
}