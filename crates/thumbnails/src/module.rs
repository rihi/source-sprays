use std::sync::atomic::{AtomicU32, Ordering};

pub(crate) static GLOBAL_REF_COUNT: AtomicU32 = AtomicU32::new(0);

pub(crate) struct ComLock {
	_private: ()
}

impl ComLock {
	pub(crate) fn new() -> Self {
		GLOBAL_REF_COUNT.fetch_add(1, Ordering::SeqCst);
		Self { _private: () }
	}
}

impl Drop for ComLock {
	fn drop(&mut self) {
		GLOBAL_REF_COUNT.fetch_sub(1, Ordering::SeqCst);
	}
}