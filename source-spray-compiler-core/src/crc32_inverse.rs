pub fn crc32_patch(current_crc: u32, target_crc: u32) -> [u8; 4] {
	let mut res = current_crc ^ 0xFFFFFFFF;
	for _ in 0..32 {
		if res & 1 != 0 {
			res = (res >> 1) ^ 0xEDB88320;
		} else {
			res >>= 1;
		}
	}
	res ^= target_crc ^ 0xFFFFFFFF;
	
	for _ in 0..32 {
		if res & 0x80000000 != 0 {
			res = (res << 1) ^ (0xEDB88320 << 1) ^ 1;
		} else {
			res <<= 1;
		}
	}
	res.to_le_bytes()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crc32fast::Hasher;

	#[test]
	fn test_crc32_patch() {
		let data = b"Hello, world!";
		let current_crc = crc32fast::hash(data);
		let target_crc = 0x12345678;

		let patch = crc32_patch(current_crc, target_crc);
		
		let mut hasher = Hasher::new();
		hasher.update(data);
		hasher.update(&patch);
		let final_crc = hasher.finalize();
		
		assert_eq!(final_crc, target_crc, "CRC32 patch failed: expected {:08x}, got {:08x}", target_crc, final_crc);
		
		// Test with random data
		for i in 0u32..100 {
			let data = format!("Random data {}", i);
			let current_crc = crc32fast::hash(data.as_bytes());
			let target_crc = i.wrapping_mul(0x1337BEEF);
			let patch = crc32_patch(current_crc, target_crc);
			
			let mut hasher = Hasher::new();
			hasher.update(data.as_bytes());
			hasher.update(&patch);
			assert_eq!(hasher.finalize(), target_crc);
		}
	}
}