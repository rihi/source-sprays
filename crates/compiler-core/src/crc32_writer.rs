use crc32fast::Hasher;
use std::io::Write;

pub struct Crc32Writer<W: Write> {
	inner: W,
	hasher: Hasher,
}

impl<W: Write> Crc32Writer<W> {
	pub fn new(inner: W) -> Self {
		Self {
			inner,
			hasher: Hasher::new(),
		}
	}

	pub fn finalize(&self) -> u32 {
		self.hasher.clone().finalize()
	}
}

impl<W: Write> Write for Crc32Writer<W> {
	fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
		self.hasher.update(buf);
		self.inner.write(buf)
	}

	fn flush(&mut self) -> std::io::Result<()> {
		self.inner.flush()
	}
}