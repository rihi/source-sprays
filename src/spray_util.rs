use crate::vtf::write_vtf;
use color_eyre::eyre;
use color_eyre::eyre::{eyre, WrapErr};
use image::{DynamicImage, ImageReader, RgbaImage};
use itertools::Itertools;
use lazy_regex::regex_captures;
use std::collections::HashMap;
use std::fs;
use std::fs::{File, FileTimes};
use std::io::{BufWriter, Write};
use std::os::windows::fs::FileTimesExt;
use std::path::{Path, PathBuf};
use crate::cli::LMipResOption;

pub fn convert_generic(
	images: &[Option<&[RgbaImage]>],
	input_path: &Path,
	output_file: &Path,
	lowest_mip_resolution: Option<u32>,
	size_limit: u32,
) -> eyre::Result<()> {
	if let Some(parent) = output_file.parent() {
		std::fs::create_dir_all(parent)
			.wrap_err("Failed to create parent directories for output")?;
	}
	
	let file = File::create(output_file)
		.wrap_err("Failed to create file")?;
	
	let mut writer = BufWriter::new(&file);
	
	write_vtf(
		&mut writer,
		images,
		lowest_mip_resolution,
		size_limit,
	)?;
	
	writer.flush()
		.wrap_err("Error while flushing file writer")?;
	
	let metadata = fs::metadata(input_path)
		.wrap_err("Failed to copy metadata information for created file")?;
	let creation_time = metadata.created()
		.wrap_err("Failed to copy metadata information for created file")?;

	file.set_times(FileTimes::new()
		.set_created(creation_time)
		.set_modified(creation_time)
		.set_accessed(creation_time))
		.wrap_err("Failed to copy metadata information for created file")?;
	
	Ok(())
}

pub fn convert_file(
	input_file: &Path,
	output_file: &Path,
	lowest_mip_resolution: Option<u32>,
	size_limit: u32,
) -> eyre::Result<()> {
	let image = load_image(input_file)?
		.into_rgba8();
	
	convert_generic(
		&[Some(&vec![image])],
		input_file,
		output_file,
		lowest_mip_resolution,
		size_limit,
	)
}

pub fn convert_folder(
	input_dir: &Path,
	output_file: &Path,
	lowest_mip_resolution: LMipResOption,
	size_limit: u32,
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
		if let Some((_, _, mip, _, frame)) = capture && mip.is_empty() != frame.is_empty() {
			let parse_number = |str: &str| Some(str)
				.filter(|s| !s.is_empty())
				.map(|s| s.parse::<u32>())
				.transpose()
				.map(|r| r.unwrap_or(0));

			let Ok(mip) = parse_number(mip) else {
				eprintln!("File {} doesn't have valid mip number", path.display());
				continue
			};
			let Ok(frame) = parse_number(frame) else {
				eprintln!("File {} doesn't have valid mip number", path.display());
				continue
			};
			image_paths.insert((mip, frame), path);
		}
	}

	if !image_paths.contains_key(&(0, 0)) {
		return Err(eyre!("Spray definition is missing mip0 frame0"));
	}

	let max_mip = image_paths.keys().map(|&(m, _)| m).max().unwrap_or(0);
	let max_frame = image_paths.keys().map(|&(_, f)| f).max().unwrap_or(0);
	
	let images: Vec<_> = (0..=max_mip)
		.map(|mip| {
			let (paths, missing): (Vec<_>, Vec<_>) = (0..=max_frame)
				.map(|f| image_paths.get(&(mip, f)).ok_or(f))
				.partition_result();

			if paths.is_empty() && !missing.is_empty() {
				return Ok(None);
			}

			if !missing.is_empty() {
				return Err(eyre!("Mip {} is missing frames {:?}", mip, missing));
			}

			let loaded: Vec<_> = paths.into_iter()
				.map(|p| load_image(p).map(|img| img.into_rgba8()))
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
		size_limit
	)
}

pub fn load_image(file: &Path) -> eyre::Result<DynamicImage> {
	let img = ImageReader::open(file)
		.wrap_err_with(|| format!("Failed to load image {}", file.display()))?
		.decode()
		.wrap_err_with(|| format!("Failed to decode image {}", file.display()))?;
	Ok(img)
}