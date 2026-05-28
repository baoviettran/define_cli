use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "define", version, about = "Look up English word definitions from the terminal")]
pub struct Cli {
    pub word: String,

    #[arg(long, help = "Show only the first definition")]
    pub short: bool,

    #[arg(long, help = "Output raw JSON")]
    pub json: bool,

    #[arg(long, help = "Plain text output without colors")]
    pub no_color: bool,

    #[arg(long, help = "Print audio pronunciation URL")]
    pub pronounce: bool,
}
