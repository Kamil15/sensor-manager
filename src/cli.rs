use clap::{self, Parser};


#[derive(Debug, Parser)]
#[command(long_about = None)]
pub struct Cli {
    #[arg(short, long, default_value_t = 300)]
    pub interval_audio: u64,
    #[arg(short, long)]
    pub auto_audio: bool,
}