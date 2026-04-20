use crate::thumbnail_provider;

use windows::Win32::UI::Shell::{IThumbnailProvider, SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
use windows_core::{Interface, GUID};
use winreg::enums::*;
use winreg::transaction::*;
use winreg::RegKey;

fn guid_to_registry_string(guid: &GUID) -> String {
	format!("{{{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}}}",
		guid.data1,
		guid.data2,
		guid.data3,
		guid.data4[0], guid.data4[1],
		guid.data4[2], guid.data4[3], guid.data4[4], guid.data4[5], guid.data4[6], guid.data4[7]
	)
}

pub(crate) fn register(module_path: &str) -> std::io::Result<()> {
	let tx = Transaction::new()?;

	let hkcu = RegKey::predef(HKEY_CURRENT_USER);

	let clsid_key_path = format!(r"Software\Classes\CLSID\{}", guid_to_registry_string(&thumbnail_provider::CLSID));
	let (clsid, _disp) = hkcu.create_subkey_transacted(clsid_key_path, &tx)?;
	clsid.set_value("", &"Thumbnail Provider for .vtf")?;

	let (inproc, _disp) = clsid.create_subkey_transacted("InprocServer32", &tx)?;
	inproc.set_value("", &module_path)?;
	inproc.set_value("ThreadingModel", &"Apartment")?;

	let (ext_key, _disp) = hkcu.create_subkey_transacted(r"Software\Classes\SystemFileAssociations\.vtf", &tx)?;
	ext_key.set_value("Treatment", &1u32)?;
	
	let (shellex, _disp) = ext_key.create_subkey_transacted(format!(r"ShellEx\{}", guid_to_registry_string(&IThumbnailProvider::IID)), &tx)?;
	shellex.set_value("", &format!("{}", guid_to_registry_string(&thumbnail_provider::CLSID)))?;

	tx.commit()?;
	shell_change_notify();
	Ok(())
}

pub(crate) fn unregister() -> std::io::Result<()> {
	let hkcu = RegKey::predef(HKEY_CURRENT_USER);
	let _ = hkcu.delete_subkey_all(r"Software\Classes\SystemFileAssociations\.vtf")?;
	let _ = hkcu.delete_subkey_all(format!(r"Software\Classes\CLSID\{}", guid_to_registry_string(&thumbnail_provider::CLSID)))?;
	shell_change_notify();
	Ok(())
}

fn shell_change_notify() {
	unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
}