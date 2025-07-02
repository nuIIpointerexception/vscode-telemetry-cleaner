use clap::Parser;

#[derive(Parser, Clone)]
#[command(name = "cleaner")]
#[command(about = "A tool for managing VSCode/Cursor telemetry and privacy settings")]
pub struct CliArgs {
    #[arg(long)]
    pub no_pause: bool,

    #[arg(long)]
    pub no_signout: bool,

    #[arg(long)]
    pub no_terminate: bool,

    #[arg(long)]
    pub zen: bool,

    #[arg(long, help = "Automatically clean Augment extension (skips selection)")]
    pub augment: bool,

    #[arg(long, help = "Automatically clean Cursor IDE (skips selection)")]
    pub cursor: bool,
}
