mod api;
mod audio;
mod cache;
mod cli;
mod history;
mod render;

use clap::Parser;
use std::io::IsTerminal;

fn main() {
    let cli = cli::Cli::parse();
    let no_color = cli.no_color || !std::io::stdout().is_terminal();

    // Handle subcommands first
    if let Some(cmd) = &cli.command {
        match cmd {
            cli::Commands::History { stats, action } => {
                match action {
                    Some(cli::HistoryAction::Clear) => {
                        handle_history_clear();
                    }
                    None => {
                        handle_history(*stats);
                    }
                }
            }
            cli::Commands::Cache { action } => match action {
                cli::CacheAction::Clear => {
                    handle_cache_clear();
                }
            },
        }
        return;
    }

    // No subcommand — require a word
    let word = match &cli.word {
        Some(w) => w,
        None => {
            eprintln!("Usage: define <word>");
            eprintln!("       define history [--stats]");
            eprintln!("       define cache clear");
            std::process::exit(1);
        }
    };

    // --json: try cache first, then fetch raw
    if cli.json {
        match cache::read_cache(word, None) {
            Ok(Some(cached)) => {
                let _ = history::append_history(word, None);
                print!("{}", cached);
                return;
            }
            Ok(None) => {}
            Err(_) => {} // cache read failure → fall through to fetch
        }

        match api::fetch_raw(word) {
            Ok(raw) => {
                let _ = cache::write_cache(word, &raw, None);
                let _ = history::append_history(word, None);
                print!("{}", raw);
            }
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // Normal lookup: try cache first, then fetch
    let entries = match cache_lookup(word) {
        Ok(Some(entries)) => entries,
        Ok(None) => match fetch_and_cache(word) {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let _ = history::append_history(word, None);

    if cli.pronounce {
        match api::find_audio_url(&entries, cli.accent.as_str()) {
            Some(url) => {
                if let Err(e) = audio::play_pronunciation(url) {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
            None => {
                let phonetic = entries
                    .iter()
                    .flat_map(|e| &e.phonetics)
                    .find_map(|p| {
                        p.text
                            .as_ref()
                            .and_then(|t| if t.is_empty() { None } else { Some(t.as_str()) })
                    });
                match phonetic {
                    Some(t) => println!("{}", t),
                    None => {
                        eprintln!("No audio pronunciation available.");
                        std::process::exit(1);
                    }
                }
            }
        }
        return;
    }

    if cli.short {
        println!("{}", render::render_short(&entries, no_color));
    } else {
        print!("{}", render::render_entries(&entries, no_color));
    }
}

/// Try to read cached entries for a word.
fn cache_lookup(word: &str) -> Result<Option<Vec<api::Entry>>, String> {
    match cache::read_cache(word, None)? {
        Some(json) => {
            let entries: Vec<api::Entry> = serde_json::from_str(&json)
                .map_err(|e| format!("Failed to parse cached response: {}", e))?;
            Ok(Some(entries))
        }
        None => Ok(None),
    }
}

/// Fetch from API and cache the result.
fn fetch_and_cache(word: &str) -> Result<Vec<api::Entry>, String> {
    let raw = api::fetch_raw(word)?;
    let _ = cache::write_cache(word, &raw, None); // non-fatal
    let entries: Vec<api::Entry> = serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    Ok(entries)
}

/// Handle `define history` or `define history --stats`.
fn handle_history(stats: bool) {
    let entries = match history::read_history(None) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    if entries.is_empty() {
        println!("No lookup history yet.");
        return;
    }

    if stats {
        print!("{}", history::render_stats(&entries));
    } else {
        print!("{}", history::render_history(&entries));
    }
}

/// Handle `define cache clear`.
fn handle_cache_clear() {
    match cache::clear_cache(None) {
        Ok(count) => println!("Cleared {} cached response(s).", count),
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

/// Handle `define history clear`.
fn handle_history_clear() {
    match history::clear_history(None) {
        Ok(()) => println!("History cleared."),
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}
