mod api;
mod render;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("Usage: define <word>");
        std::process::exit(1);
    }

    let word = &args[0];

    match api::fetch_definition(word) {
        Ok(entries) => print!("{}", render::render_entries(&entries, false)),
        Err(e) => {
            eprintln!("{}{}{}", render::YELLOW, e, render::RESET);
            std::process::exit(1);
        }
    }
}
