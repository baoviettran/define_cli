use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "define", version, about = "Look up English word definitions from the terminal")]
pub struct Cli {
    /// The English word to look up
    pub word: Option<String>,

    #[arg(long, help = "Show only the first definition")]
    pub short: bool,

    #[arg(long, help = "Output raw JSON")]
    pub json: bool,

    #[arg(long, help = "Plain text output without colors")]
    pub no_color: bool,

    #[arg(long, help = "Print audio pronunciation URL")]
    pub pronounce: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Show lookup history
    History {
        #[command(subcommand)]
        action: Option<HistoryAction>,

        #[arg(long, help = "Show lookup statistics")]
        stats: bool,
    },
    /// Manage the local cache
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum HistoryAction {
    /// Delete lookup history
    Clear,
}

#[derive(Subcommand, Debug)]
pub enum CacheAction {
    /// Delete all cached responses
    Clear,
}
