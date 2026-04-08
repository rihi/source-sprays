use crate::spray_util::load_image;
use crate::vtf::write_vtf;
use crate::watching::{compile_spray_def, delete_spray_def, is_spray_def, path_vtf_for_def};
use bpaf::Bpaf;
use color_eyre::eyre;
use color_eyre::eyre::WrapErr;
use notify::event::{ModifyKind, RenameMode};
use notify::{EventKind, RecursiveMode, Watcher};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::mpsc::channel;
use walkdir::WalkDir;

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version)]
pub enum Cli {
	#[bpaf(command)]
	/// Compile an entire directory into VTF sprays
	Compile {
		/// If the directory should be watched and auto recompiled on changes
		#[bpaf(switch)]
		watch_changes: bool,
		
		/// If a whole directory tree should be searched for sprays to compile
		#[bpaf(switch, fallback(false), display_fallback)]
		recursive: bool,

		/// The resolution of the smallest mipmap that should be required.
		/// If left on 'Infer', uses 32 if at least one mip specified, else 'none'. 
		#[bpaf(argument("value"), fallback(LMipResOption::Infer), debug_fallback)]
		lowest_mip_resolution: LMipResOption,

		/// Maximum allowed file size in KiB
		#[bpaf(argument("value"), fallback(512), display_fallback)]
		size_limit: u32,

		/// Which spray to compile. Directory when using recursive
		#[bpaf(positional("INPUT_PATH"))]
		input_path: PathBuf,

		/// Where to write the output. Directory when using recursive
		#[bpaf(positional("OUTPUT_PATH"))]
		output_path: PathBuf,
	},
	#[bpaf(command)]
	/// Process individual images into a VTF spray
	CompileManual {
		#[bpaf(external(mip), many)]
		mips: Vec<Mip>,

		/// The resolution of the smallest mipmap that should be required.
		/// If left on 'Infer', uses 32 if at least one mip specified, else 'none'. 
		#[bpaf(argument("value"), fallback(LMipResOption::Infer), debug_fallback)]
		lowest_mip_resolution: LMipResOption,

		/// Maximum allowed file size in KiB
		#[bpaf(argument("value"), fallback(512), display_fallback)]
		size_limit: u32,

		/// Where to write the vtf file
		#[bpaf(positional("VTF"))]
		output_file: PathBuf,

		/// Path to image file. Multiple in case of animation.
		#[bpaf(positional("IMAGE"), some("At least one input file is required"))]
		input_files: Vec<PathBuf>,
	},
}

#[derive(Debug, Copy, Clone, Bpaf)]
// #[bpaf(enum)]
pub enum LMipResOption {
	Infer,
	None,
	Resolution(u32),
}

impl FromStr for LMipResOption {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_ascii_lowercase().as_str() { 
			"infer" => Ok(LMipResOption::Infer),
			"none" => Ok(LMipResOption::None),
			_ => s.parse::<u32>()
				.map(|res| LMipResOption::Resolution(res))
				.map_err(|_| format!("Expected 'infer', 'none' or number. Got {}", s).to_string())
		}
	}
}

impl LMipResOption {
	pub fn infer(&self, infer_none: bool) -> Option<u32> {
		match self {
			LMipResOption::Infer => if infer_none { None } else { Some(32) },
			LMipResOption::None => None,
			LMipResOption::Resolution(res) => Some(*res),
		}
	}
}


#[derive(Debug, Clone, Bpaf)]
#[bpaf(adjacent)]
pub struct Mip {
	/// The MIP level
	#[bpaf(long("mip"), argument("N"))]
	level: NonZero<usize>,
	/// The image file path for this mip level. Multiple in case of animation.
	#[bpaf(positional("IMAGE"), some("At least one input file is required"))]
	input_files: Vec<PathBuf>,
}

pub fn run() -> eyre::Result<()> {
	let cli = cli().run();

	match cli {
		Cli::CompileManual {
			mips,
			lowest_mip_resolution,
			size_limit,
			output_file,
			input_files,
		} => {
			for (i, mip) in mips.iter().enumerate().skip(1) {
				if mip.input_files.len() != input_files.len() {
					eprintln!("Numbers of frames in mip {} must match number of frames of mip 0", i);
					std::process::exit(1);
				}
			}

			let max_mip = mips.iter()
				.map(|m| m.level.get())
				.max()
				.unwrap_or(0);
			
			let mut paths = vec![None; max_mip + 1];
			paths[0] = Some(input_files);
			for mip in mips {
				let level = mip.level.get();
				if paths[level].replace(mip.input_files).is_some() {
					eprintln!("Mip {} is specified multiple times. Must be unique", mip.level);
					std::process::exit(1);
				}
			}

			let images: Vec<_> = paths.into_iter()
				.map(|mip_paths| mip_paths
					.map(|mip| mip.into_iter()
						.map(|path| load_image(&path).map(|img| img.to_rgba8()))
						.collect::<Result<Vec<_>, _>>())
					.transpose())
				.collect::<Result<_, _>>()?;
			
			let file = File::create(&output_file)
				.wrap_err("Failed to create vtf file")?;

			let mut writer = BufWriter::new(file);
			
			write_vtf(
				&mut writer,
				&images.iter()
					.map(|frames| frames.as_deref())
					.collect::<Vec<_>>(),
				lowest_mip_resolution.infer(images.len() == 1),
				size_limit,
			)
				.wrap_err("Failed to write vtf file")?;
			
			writer.flush()
				.wrap_err("Error while flushing file writer")?;
			
			Ok(())
		}

		Cli::Compile {
			watch_changes,
			recursive,
			lowest_mip_resolution,
			size_limit,
			input_path,
			output_path,
		} => {
			if recursive {
				if !input_path.is_dir() {
					eprintln!("input_path is not a directory, required for recursive");
					std::process::exit(1);
				}
				if !output_path.is_dir() {
					eprintln!("output_path is not a directory, required for recursive");
					std::process::exit(1);
				}
			}
			
			let process_whole = || {
				if recursive {
					for entry in WalkDir::new(&input_path) {
						let entry = match entry {
							Ok(entry) => entry,
							Err(e) => {
								eprintln!("Warning: Skipping entry: {:?}", e);
								continue
							}
						};

						if is_spray_def(entry.path()) {
							let vtf_path = path_vtf_for_def(entry.path(), &input_path, &output_path);
							compile_spray_def(entry.path(), &vtf_path, lowest_mip_resolution, size_limit);
						}
					}
				} else {
					compile_spray_def(&input_path, &output_path, lowest_mip_resolution, size_limit);
				}
			};

			if !watch_changes {
				process_whole();
				return Ok(());
			}

			let (tx, rx) = channel();
			let mut watcher = notify::recommended_watcher(tx)
				.wrap_err("Failed to create recommended_watcher")?;

			let recursive_mode = if recursive { RecursiveMode::Recursive } else { RecursiveMode::NonRecursive };
			watcher.watch(&input_path, recursive_mode)
				.wrap_err("Error on watching directory")?;
			
			let compile_potential_def = |path: &Path| {
				if !is_spray_def(path) {
					return
				}
				
				let vtf_path = if recursive {
					&path_vtf_for_def(path, &input_path, &output_path)
				} else {
					if path != input_path {
						return
					}
					&output_path
				};
				compile_spray_def(path, vtf_path, lowest_mip_resolution, size_limit);
			};
			let on_file_new_data = |path: &Path| {
				compile_potential_def(path);
				if let Some(parent) = path.parent() {
					compile_potential_def(parent);
				}
			};
			let on_file_gone = |path: &Path| {
				if is_spray_def(&path) && (recursive || input_path == *path) {
					delete_spray_def(path, &input_path, &output_path);
				}
				if let Some(parent) = path.parent() {
					compile_potential_def(parent);
				}
				// if recursive {
				// 	let Ok(path_rel) = path.strip_prefix(&input_path) else { continue; };
				// 	let path_out = output_path.join(path_rel);
				// 	println!("Info: Removing directory {}", path_out.display());
				// 	let _ = std::fs::remove_dir_all(path_out);
				// }
			};
			
			process_whole();
			
			for res in rx {
				let event = match res {
					Ok(event) => event,
					Err(e) => { 
						println!("watch error: {:?}", e);
						continue;
					},
				};
				
				if event.need_rescan() {
					process_whole();
					continue
				}
				
				println!("-----------");
				println!("{:?}", &event);
				match event.kind {
					EventKind::Create(_) => for path in event.paths {
						on_file_new_data(&path);
					},
					EventKind::Modify(kind) => match kind {
						ModifyKind::Name(mode) => match mode {
							RenameMode::Any
							| RenameMode::Other
							| RenameMode::To => event.paths.iter().for_each(|path| on_file_new_data(path)),
							RenameMode::From => event.paths.iter().for_each(|path| on_file_gone(path)),
							RenameMode::Both => {
								event.paths[0..1].iter().for_each(|path| on_file_gone(path));
								event.paths[1..2].iter().for_each(|path| on_file_new_data(path));
							}
						}
						ModifyKind::Any 
						| ModifyKind::Data(_)
						| ModifyKind::Metadata(_) => event.paths.iter()
							.filter(|path| path.is_file())
							.for_each(|path| on_file_new_data(path)),
						ModifyKind::Other => {}
					},
					EventKind::Remove(_) => event.paths[0..1].iter().for_each(|path| on_file_gone(path)),
					_ => {}
				}
			}
			
			Ok(())
		}
	}
}

#[test]
fn test_cli() {
	cli().check_invariants(false);
}