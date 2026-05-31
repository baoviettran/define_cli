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

    #[cfg_attr(feature = "audio", arg(long, help = "Play audio pronunciation"))]
    #[cfg_attr(not(feature = "audio"), arg(long, help = "Show phonetic pronunciation"))]
    pub pronounce: bool,

    #[cfg(feature = "audio")]
    #[arg(long, value_enum, default_value_t, help = "Pronunciation accent (us, uk, au)")]
    pub accent: Accent,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

impl Cli {
    /// Returns the accent string for audio URL filtering.
    /// Returns "us" when the audio feature is disabled (default for fallback).
    pub fn accent_str(&self) -> &str {
        #[cfg(feature = "audio")]
        {
            self.accent.as_str()
        }
        #[cfg(not(feature = "audio"))]
        {
            "us"
        }
    }
}

#[cfg(feature = "audio")]
#[derive(Clone, Debug, Default, clap::ValueEnum)]
pub enum Accent {
    #[default]
    Us,
    Uk,
    Au,
}

#[cfg(feature = "audio")]
impl Accent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Accent::Us => "us",
            Accent::Uk => "uk",
            Accent::Au => "au",
        }
    }
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
