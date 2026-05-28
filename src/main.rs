mod api;
mod cli;
mod render;

use clap::Parser;
use std::io::IsTerminal;

fn main() {
    let cli = cli::Cli::parse();
    let no_color = cli.no_color || !std::io::stdout().is_terminal();

    if cli.json {
        match api::fetch_raw(&cli.word) {
            Ok(raw) => print!("{}", raw),
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    let entries = match api::fetch_definition(&cli.word) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    if cli.pronounce {
        match api::find_audio_url(&entries) {
            Some(url) => println!("{}", url),
            None => {
                eprintln!("No audio pronunciation available.");
                std::process::exit(1);
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
