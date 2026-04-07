use crate::spray_util::{convert_file, convert_folder};
use std::path::{Path, PathBuf};
use crate::cli::LMipResOption;

pub fn rebase_path(
	path: &Path,
	base: &Path,
	new_base: &Path,
) -> Option<PathBuf> {
	path.strip_prefix(base)
		.ok()
		.map(|rel| new_base.join(rel))
}

pub fn path_vtf_for_def(
	def_path: &Path,
	src_path: &Path,
	dst_path: &Path,
) -> PathBuf {
	let mut vtf_path = rebase_path(def_path, src_path, dst_path).unwrap();
	if def_path.is_dir() {
		vtf_path.add_extension("vtf");
	} else {
		vtf_path.set_extension("vtf");
	}
	vtf_path
}

pub fn is_spray_def(
	path: &Path
) -> bool {
	let Some(filename) = path.file_name() else {
		return false;
	};
	return filename.to_string_lossy().contains(".spray");
}

pub fn compile_spray_def(
	def_path: &Path,
	vtf_path: &Path,
	lowest_mip_resolution: LMipResOption,
	size_limit: u32,
) {
	if def_path.is_file() {
		println!("Info: Compiling file spray def {} to {}", def_path.display(), vtf_path.display());
		if let Err(e) = convert_file(def_path, &vtf_path, lowest_mip_resolution.infer(true), size_limit) {
			eprintln!("Error: Failed convert file def to vtf: {}\n{:?}", def_path.display(), e);
		}
	}
	if def_path.is_dir() {
		println!("Info: Compiling dir spray def {} to {}", def_path.display(), vtf_path.display());
		if let Err(e) = convert_folder(def_path, &vtf_path, lowest_mip_resolution, size_limit) {
			eprintln!("Error: Failed convert dir def to vtf: {}\n{:?}", vtf_path.display(), e);
		}
	}
}

pub fn delete_spray_def(
	def_path: &Path,
	src_path: &Path,
	dst_path: &Path,
) {
	let rebased = rebase_path(def_path, src_path, dst_path).unwrap();
	let vtf_file = rebased.with_extension("vtf");
	let vtf_dir = rebased.with_added_extension("vtf");
	
	println!("Info: Removing spray {} at {}", def_path.display(), vtf_file.display());
	let _ = std::fs::remove_file(vtf_file);
	println!("Info: Removing spray {} at {}", def_path.display(), vtf_dir.display());
	let _ = std::fs::remove_file(vtf_dir);
}