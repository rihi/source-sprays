mod module;
mod class_factory;
mod thumbnail_provider;
mod registry;
mod winstream;

use std::ffi::c_void;
use std::sync::atomic::Ordering;

use crate::class_factory::MyClassFactory;
use crate::module::GLOBAL_REF_COUNT;
use crate::registry::{register, unregister};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::System::Com::*,
    Win32::System::SystemServices::DLL_PROCESS_ATTACH
};

static mut DLL_INSTANCE: HINSTANCE = HINSTANCE(std::ptr::null_mut());

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub extern "system" fn DllMain(
    dll_instance: HINSTANCE,
    reason: u32,
    _reserved: *mut c_void,
) -> bool {
    if reason == DLL_PROCESS_ATTACH {
        unsafe {
            DLL_INSTANCE = dll_instance;
        }
    }
    true
}

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    if unsafe { *riid } != IClassFactory::IID {
        return E_UNEXPECTED;
    }
    
    return match unsafe { *rclsid } {
        thumbnail_provider::CLSID => {
            let class_factory: IUnknown = MyClassFactory::new().into();
            unsafe { class_factory.query(riid, ppv) }
        }
        _ => CLASS_E_CLASSNOTAVAILABLE  
    };
}

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if GLOBAL_REF_COUNT.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}


#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    let module_path = match get_module_path(unsafe { DLL_INSTANCE }) {
        Ok(path) => path,
        Err(err) => return err,
    };
    if register(&module_path).is_ok() {
        S_OK
    } else {
        E_FAIL
    }
}

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    if unregister().is_ok() {
        S_OK
    } else {
        E_FAIL
    }
}

fn get_module_path(instance: HINSTANCE) -> core::result::Result<String, HRESULT> {
    let mut path = [0u16; MAX_PATH as usize];
    let path_len = unsafe { GetModuleFileNameW(Some(instance.into()), &mut path) } as usize;
    String::from_utf16(&path[0..path_len]).map_err(|_| E_FAIL)
}