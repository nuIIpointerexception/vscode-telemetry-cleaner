use cleaner::{CliArgs, ZenGarden};
use clap::Parser;
use color_eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = CliArgs::parse();

    // zen garden is now the default interface
    let mut garden = ZenGarden::new(&args);
    garden.run(args).await?;

    Ok(())
}


