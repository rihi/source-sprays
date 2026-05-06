# Source Sprays

This repository contains primarily Rust tools I made for working with Source Engine sprays.

The main functionality is building spray VTF files from image inputs, including sprays with
multiple animation frames and explicit mip levels. The project contains a browser tool for quick
use, a CLI for local or scripted builds, and a Windows thumbnail provider for previewing VTF files
in Explorer.

## Tools

### Browser Spray Compiler

The browser spray compiler is hosted on GitHub Pages:
[rihi.github.io/source-sprays](https://rihi.github.io/source-sprays/)

It is a simple static webpage using the Rust compiler code through WebAssembly, so sprays can be
assembled locally in the page without needing to install the CLI. This is the easiest way to create a spray
from images when you do not need automation.

### CLI Spray Compiler

`source-spray-compiler-cli` is the command line tool for creating `.vtf` spray files.

The CLI has three main subcommands:
- `compile`: compile image files or directory definitions into VTF sprays. It also supports
  recursive compiling and watching for changes.
- `compile-manual`: build a spray from explicitly listed mip and frame image inputs.
- `derive-low-res`: create a low-resolution VTF with the same CRC32 as an existing VTF.

For `compile`, single image files compile directly into a VTF. Directory definitions can contain
explicit mip/frame files, named as:

```text
mip<N>frame<M>.<ext>
mip<N>.<ext>
frame<M>.<ext>
```

Missing mip levels are inferred from the nearest available mip, preferring the next larger one and
falling back to the next smaller one. In recursive mode, `compile` only picks up files and directories whose names
contain `.spray`.

Use `--help` on the CLI or any subcommand to see the available arguments:

```bash
cargo run -p source-spray-compiler-cli -- --help
cargo run -p source-spray-compiler-cli -- compile --help
```

### VTF Thumbnail Provider

`source-spray-thumbnails` is a Windows COM DLL thumbnail provider for `.vtf` files. It allows VTF
textures to show previews in Windows Explorer.

Register or unregister the built DLL with `regsvr32`. The provider is registered for the current
user, not system-wide.


## Repository Layout

- `crates/`: contains all rust crates.
  - `common`: shared VTF parsing, image, and thumbnail utilities.
  - `compiler-core`: spray building, VTF writing, resizing, DXT compression, mip handling, and CRC helpers.
  - `compiler-cli`: command line interface for compiling sprays.
  - `compiler-wasm`: WebAssembly interface between the compiler core and the browser frontend.
  - `thumbnails`: Windows shell thumbnail provider for VTF files.
- `compiler-web-frontend`: static browser UI for compiling sprays.
