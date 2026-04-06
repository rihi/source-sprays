use std::borrow::Cow;
use crate::spray_util::{convert_file, convert_folder, load_image};
use crate::vtf::write_vtf;
use bpaf::Bpaf;
use color_eyre::eyre;
use color_eyre::eyre::WrapErr;
use notify::{EventKind, RecursiveMode, Watcher};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use walkdir::WalkDir;

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version)]
pub enum Cli {
	#[bpaf(command)]
	/// Process individual images into a VTF spray
	Compile {
		/// Path to image file for mip level 1.
		#[bpaf(argument("mip1"), many)]
		mip1: Vec<PathBuf>,

		/// Path to image file for mip level 2.
		#[bpaf(argument("mip2"), many)]
		mip2: Vec<PathBuf>,

		/// Path to image file for mip level 3.
		#[bpaf(argument("mip3"), many)]
		mip3: Vec<PathBuf>,

		/// Path to image file for mip level 4.
		#[bpaf(argument("mip4"), many)]
		mip4: Vec<PathBuf>,

		/// Path to image file for mip level 5.
		#[bpaf(argument("mip5"), many)]
		mip5: Vec<PathBuf>,

		/// The resolution of the smallest mipmap that should be required.
		#[bpaf(argument("lowest-mip-resolution"), fallback(32), display_fallback)]
		lowest_mip_resolution: u32,

		/// Maximum allowed file size in KiB
		#[bpaf(argument("size-limit"), fallback(512), display_fallback)]
		size_limit: u32,

		/// Where to write the vtf file
		#[bpaf(positional("OUTPUT"))]
		output: PathBuf,

		/// Path to image file. Multiple in case of animation.
		#[bpaf(positional("INPUT_FILES"), some("At least one input file is required"))]
		input_files: Vec<PathBuf>,
	},
	#[bpaf(command)]
	/// Compile an entire directory into VTF sprays
	CompileDirectory {
		/// If the directory should be watched and auto recompiled on changes
		#[bpaf(switch)]
		watch_changes: bool,

		/// The resolution of the smallest mipmap that should be required.
		#[bpaf(argument("lowest-mip-resolution"), fallback(32), display_fallback)]
		lowest_mip_resolution: u32,

		/// Maximum allowed file size in KiB
		#[bpaf(argument("size-limit"), fallback(512), display_fallback)]
		size_limit: u32,

		/// Which directory to compile
		#[bpaf(positional("INPUT_DIR"))]
		input_dir: PathBuf,

		/// Where to write the output
		#[bpaf(positional("OUTPUT_DIR"))]
		output_dir: PathBuf,
	},
}

pub fn run() -> eyre::Result<()> {
	let cli = cli().run();

	match cli {
		Cli::Compile {
			output,
			input_files,
			mip1,
			mip2,
			mip3,
			mip4,
			mip5,
			lowest_mip_resolution,
			size_limit,
		} => {
			// Helper to translate empty bpaf vecs back into Options for our core logic
			let to_opt = |v: Vec<PathBuf>| if v.is_empty() { None } else { Some(v) };

			let mips = vec![
				Some(input_files.clone()),
				to_opt(mip1),
				to_opt(mip2),
				to_opt(mip3),
				to_opt(mip4),
				to_opt(mip5)
			];

			for (i, mip) in mips.iter().enumerate().skip(1) {
				if let Some(m) = mip {
					if m.len() != input_files.len() {
						eprintln!("Numbers of frames in mip {} must match number of frames of mip 0", i);
						std::process::exit(1);
					}
				}
			}

			// Slice trailing nones
			let mut paths = mips;
			while let Some(None) = paths.last() {
				paths.pop();
			}

			let images: Vec<_> = paths.into_iter()
				.map(|mip_paths| mip_paths
					.map(|mip| mip.into_iter()
						.map(|path| load_image(&path).map(|img| img.to_rgba8()))
						.collect::<Result<Vec<_>, _>>())
					.transpose())
				.collect::<Result<_, _>>()?;
			
			let file = File::create(&output)
				.wrap_err("Failed to create vtf file")?;

			let mut writer = BufWriter::new(file);

			write_vtf(
				&mut writer,
				&images.iter()
					.map(|frames| frames.as_deref())
					.collect::<Vec<_>>(),
				lowest_mip_resolution,
				size_limit,
			)
				.wrap_err("Failed to write vtf file")?;
			
			writer.flush()
				.wrap_err("Error while flushing file writer")?;
			
			Ok(())
		}

		Cli::CompileDirectory {
			input_dir,
			output_dir,
			watch_changes,
			lowest_mip_resolution,
			size_limit,
		} => {
			let mut add_file = |path: &Path| {
				if path.is_file() {
					let path_rel = path.strip_prefix(&input_dir).unwrap();
					let path_out = output_dir.join(path_rel).with_extension("vtf");
					if let Err(e) = convert_file(path, &path_out, lowest_mip_resolution, size_limit) {
						eprintln!("Error: Failed convert image to vtf: {}\n{:?}", path.display(), e);
					}
				}
				if path.is_dir() {
					let path_rel = path.strip_prefix(&input_dir).unwrap();
					let path_out = output_dir.join(path_rel).with_extension("spray.vtf");
					if let Err(e) = convert_folder(path, &path_out, lowest_mip_resolution, size_limit) {
						eprintln!("Error: Failed convert dir to vtf: {}\n{:?}", path.display(), e);
					}
				}
			};

			let mut delete_file = |path: &Path| {
				if let Ok(path_rel) = path.strip_prefix(&input_dir) {
					let path_out = output_dir.join(path_rel);
					let path_out_file = path_out.with_extension("vtf");
					let path_out_dir = path_out.with_extension("spray.vtf");

					println!("Info: Removing files for {}", path_out.display());
					let _ = std::fs::remove_file(path_out_file);
					let _ = std::fs::remove_file(path_out_dir);
				}
			};

			let process_files = |path: &Path, process: &mut dyn FnMut(&Path) -> (), recursive: bool| {
				let iter: &mut dyn Iterator<Item=Cow<Path>> = if recursive {
					&mut WalkDir::new(path)
						.into_iter()
						.filter_map(|entry| match entry {
							Ok(val) => Some(val),
							Err(e) => {
								eprintln!("Warning: Skipping entry: {:?}", e);
								None
							}
						})
						.map(|entry| entry.path().to_path_buf().into())
				} else {
					&mut std::iter::once(path.into())
				};

				for p in iter {
					let Some(filename) = p.file_name() else { continue; };
					if filename.to_string_lossy().contains(".spray") {
						process(p.as_ref());
					}
				}
			};

			process_files(&input_dir, &mut add_file, true);

			if !watch_changes {
				return Ok(());
			}

			let (tx, rx) = channel();
			let mut watcher = notify::recommended_watcher(tx)
				.wrap_err("Failed to create recommended_watcher")?;
			
			watcher.watch(&input_dir, RecursiveMode::Recursive)
				.wrap_err("Error on watching directory")?;

			for res in rx {
				let event = match res {
					Ok(event) => event,
					Err(e) => { 
						println!("watch error: {:?}", e);
						continue;
					},
				};

				println!("-----------");
				for path in event.paths {
					println!("{:?} {:?}", event.kind, path);
					match event.kind {
						EventKind::Create(_) => process_files(&path, &mut add_file, false),
						EventKind::Modify(_) => process_files(&path, &mut add_file, false),
						EventKind::Remove(_) => {
							process_files(&path, &mut delete_file, false);
							
							let Ok(path_rel) = path.strip_prefix(&input_dir) else { continue; };
							let path_out = output_dir.join(path_rel);
							println!("Info: Removing directory {:?}", path_out);
							let _ = std::fs::remove_dir_all(path_out);
						}
						_ => {}
					}
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