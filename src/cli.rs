use clap::Parser;

#[derive(Parser, Clone)]
#[command(name = "cleaner")]
#[command(about = "A tool for managing VSCode telemetry and privacy settings")]
pub struct CliArgs {
    #[arg(long)]
    pub no_pause: bool,

    #[arg(long)]
    pub no_signout: bool,

    #[arg(long)]
    pub no_terminate: bool,

    #[arg(long)]
    pub zen: bool,
}
