use crate::module::{ComLock, GLOBAL_REF_COUNT};
use crate::thumbnail_provider::ThumbnailProvider;
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::CLASS_E_NOAGGREGATION;
use windows::Win32::System::Com::IClassFactory;
use windows::Win32::System::Com::IClassFactory_Impl;
use windows_core::{implement, IUnknown, Interface, BOOL, GUID};

#[implement(IClassFactory)]
pub(crate) struct MyClassFactory {
	_lock: ComLock,
}

impl MyClassFactory {
	pub(crate) fn new() -> MyClassFactory {
		Self { _lock: ComLock::new() }
	} 
}

impl IClassFactory_Impl for MyClassFactory_Impl {
	#[allow(non_snake_case)]
	fn CreateInstance(
		&self,
		outer: windows_core::Ref<'_, windows_core::IUnknown>,
		riid: *const GUID,
		ppv: *mut *mut core::ffi::c_void,
	) -> windows_core::Result<()> {
		unsafe {
			if outer.is_some() {
				return CLASS_E_NOAGGREGATION.ok();
			}
			let thumbnail_provider: IUnknown = ThumbnailProvider::new().into();
			return thumbnail_provider.query(riid, ppv).ok();
		}
	}

	#[allow(non_snake_case)]
	fn LockServer(
		&self,
		lock: BOOL
	) -> windows_core::Result<()> {
		if lock.as_bool() {
			GLOBAL_REF_COUNT.fetch_add(1, Ordering::SeqCst);
		} else {
			GLOBAL_REF_COUNT.fetch_sub(1, Ordering::SeqCst);
		}
		Ok(())
	}
}