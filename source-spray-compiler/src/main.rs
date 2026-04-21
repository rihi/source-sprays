#![feature(option_reference_flattening)]

use color_eyre::eyre;

mod cli;
mod image_util;
mod spray_util;
mod vtf;
mod watching;
mod crc32_writer;
mod crc32_inverse;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    cli::run()
}
