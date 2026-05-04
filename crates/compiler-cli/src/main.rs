use color_eyre::eyre;

mod cli;
mod watching;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    cli::run()
}
