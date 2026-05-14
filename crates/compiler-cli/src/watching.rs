use crate::cli::LMipResOption;
use color_eyre::eyre;
use color_eyre::eyre::{eyre, Context};
use image::RgbaImage;
use itertools::Itertools;
use lazy_regex::regex_captures;
use source_spray_compiler_core::image_util::load_image;
use source_spray_compiler_core::vtf::write_optimal_vtf;
use std::collections::HashMap;
use std::fs;
use std::fs::{File, FileTimes};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

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
	desired_resolution: Option<u32>,
	size_limit: u64,
) {
	if def_path.is_file() {
		println!("Info: Compiling file spray def {} to {}", def_path.display(), vtf_path.display());
		if let Err(e) = convert_file(
			def_path,
			&vtf_path,
			lowest_mip_resolution.infer(true),
			desired_resolution,
			size_limit,
			true
		) {
			eprintln!("Error: Failed convert file def to vtf: {}\n{:?}", def_path.display(), e);
		}
	}
	if def_path.is_dir() {
		println!("Info: Compiling dir spray def {} to {}", def_path.display(), vtf_path.display());
		if let Err(e) = convert_folder(
			def_path,
			&vtf_path,
			lowest_mip_resolution,
			desired_resolution,
			size_limit,
			true
		) {
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

pub fn convert_generic(
	images: &[Option<&[RgbaImage]>],
	input_path: &Path,
	output_file: &Path,
	lowest_mip_resolution: Option<u32>,
	desired_resolution: Option<u32>,
	size_limit: u64,
	copy_metadata: bool,
) -> eyre::Result<()> {
	if let Some(parent) = output_file.parent() {
		std::fs::create_dir_all(parent)
			.wrap_err("Failed to create parent directories for output")?;
	}

	let file = File::create(output_file)
		.wrap_err("Failed to create file")?;

	let mut writer = BufWriter::new(&file);

	write_optimal_vtf(
		&mut writer,
		images,
		lowest_mip_resolution,
		desired_resolution,
		size_limit,
	)?;

	writer.flush()
		.wrap_err("Error while flushing file writer")?;

	if copy_metadata {
		let metadata = fs::metadata(input_path)
			.wrap_err("Failed to copy metadata information for created file")?;
		let creation_time = metadata.created()
			.wrap_err("Failed to copy metadata information for created file")?;

		let mut times = FileTimes::new()
			.set_modified(creation_time)
			.set_accessed(creation_time);
		#[cfg(windows)]
		{
			use std::os::windows::fs::FileTimesExt;
			times = times.set_created(creation_time);
		}

		file.set_times(times)
			.wrap_err("Failed to copy metadata information for created file")?;
	}

	Ok(())
}

pub fn convert_file(
	input_file: &Path,
	output_file: &Path,
	lowest_mip_resolution: Option<u32>,
	desired_resolution: Option<u32>,
	size_limit: u64,
	copy_metadata: bool,
) -> eyre::Result<()> {
	let image = load_image(input_file)?;

	convert_generic(
		&[Some(&vec![image])],
		input_file,
		output_file,
		lowest_mip_resolution,
		desired_resolution,
		size_limit,
		copy_metadata
	)
}

pub fn convert_folder(
	input_dir: &Path,
	output_file: &Path,
	lowest_mip_resolution: LMipResOption,
	desired_resolution: Option<u32>,
	size_limit: u64,
	copy_metadata: bool,
) -> eyre::Result<()> {
	let mut image_paths: HashMap<(u32, u32), PathBuf> = HashMap::new();

	let entries = std::fs::read_dir(input_dir)
		.wrap_err("Error reading spray directory")?;
	for entry in entries {
		let entry = entry
			.wrap_err("Error reading spray directory")?;

		let path = entry.path();
		if !path.is_file() {
			continue;
		}

		let Some(filename) = path.file_name() else { continue };
		let filename = filename.to_string_lossy();
		let capture = regex_captures!(r"^(mip(\d+))?(frame(\d+))?\..*", &filename);
		if let Some((_, _, mip, _, frame)) = capture && !(mip.is_empty() && frame.is_empty()) {
			let parse_number = |str: &str| Some(str)
				.filter(|s| !s.is_empty())
				.map(|s| s.parse::<u32>())
				.transpose()
				.map(|r| r.unwrap_or(0));

			let Ok(mip) = parse_number(mip) else {
				eprintln!("Warning: File {} doesn't have valid mip number", path.display());
				continue
			};
			let Ok(frame) = parse_number(frame) else {
				eprintln!("Warning: File {} doesn't have valid mip number", path.display());
				continue
			};
			image_paths.insert((mip, frame), path);
		}
	}

	let Some((_min_mip, max_mip)) = image_paths.keys()
		.map(|&(m, _)| m)
		.minmax()
		.into_option() else {
			return Err(eyre!("Spray directory contains no frames/mips")); 
		};
	let (min_frame, max_frame) = image_paths.keys()
		.map(|&(_, f)| f)
		.minmax()
		.into_option()
		.unwrap();

	let images: Vec<_> = (0..=max_mip)
		.map(|mip| {
			let (paths, missing): (Vec<_>, Vec<_>) = (min_frame..=max_frame)
				.map(|f| image_paths.get(&(mip, f)).ok_or(f))
				.partition_result();

			if paths.is_empty() {
				return Ok(None);
			}

			if !missing.is_empty() {
				return Err(eyre!("Mip {} is missing frames {:?}", mip, missing));
			}

			let loaded: Vec<_> = paths.into_iter()
				.map(|p| load_image(p))
				.collect::<Result<_, _>>()?;

			Ok(Some(loaded))
		})
		.collect::<Result<_, _>>()?;

	convert_generic(
		&images.iter()
			.map(|frames| frames.as_deref())
			.collect::<Vec<_>>(),
		input_dir,
		output_file,
		lowest_mip_resolution.infer(images.len() == 1),
		desired_resolution,
		size_limit,
		copy_metadata,
	)
}