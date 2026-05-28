use crate::api::Entry;

pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const CYAN: &str = "\x1b[36m";
pub const MAGENTA: &str = "\x1b[35m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const RESET: &str = "\x1b[0m";
const DIVIDER: &str = "──────────────────────────────────────────────────";

pub fn render_entries(entries: &[Entry], no_color: bool) -> String {
    let bold = if no_color { "" } else { BOLD };
    let dim = if no_color { "" } else { DIM };
    let cyan = if no_color { "" } else { CYAN };
    let magenta = if no_color { "" } else { MAGENTA };
    let green = if no_color { "" } else { GREEN };
    let yellow = if no_color { "" } else { YELLOW };
    let reset = if no_color { "" } else { RESET };

    let mut out = String::new();

    for entry in entries {
        out.push_str(&format!(
            "{}{}{}{}\n",
            bold, cyan, entry.word.to_uppercase(), reset
        ));

        if let Some(phonetic) = entry
            .phonetics
            .iter()
            .find_map(|p| p.text.as_ref().and_then(|t| if t.is_empty() { None } else { Some(t) }))
        {
            out.push_str(&format!("{}{}{}\n", dim, phonetic, reset));
        }

        out.push_str(&format!("{}{}{}\n", dim, DIVIDER, reset));

        for meaning in &entry.meanings {
            out.push('\n');
            out.push_str(&format!(
                "{}{}{}{}\n",
                bold, magenta, meaning.part_of_speech.to_uppercase(), reset
            ));
            out.push('\n');

            for (i, def) in meaning.definitions.iter().take(3).enumerate() {
                out.push_str(&format!("  {}. {}\n", i + 1, def.definition));
                if let Some(example) = &def.example {
                    out.push_str(&format!("     {}\"{}\"{}\n", dim, example, reset));
                }
            }

            if !meaning.synonyms.is_empty() {
                let syns: Vec<&str> = meaning.synonyms.iter().take(5).map(|s| s.as_str()).collect();
                out.push_str(&format!(
                    "  {}Synonyms:{} {}{}{}\n",
                    bold, reset, green, syns.join(", "), reset
                ));
            }

            if !meaning.antonyms.is_empty() {
                let ants: Vec<&str> = meaning.antonyms.iter().take(5).map(|s| s.as_str()).collect();
                out.push_str(&format!(
                    "  {}Antonyms:{} {}{}{}\n",
                    bold, reset, yellow, ants.join(", "), reset
                ));
            }
        }

        out.push_str(&format!("{}{}{}\n", dim, DIVIDER, reset));
    }

    out
}

pub fn render_short(entries: &[Entry], no_color: bool) -> String {
    let bold = if no_color { "" } else { BOLD };
    let cyan = if no_color { "" } else { CYAN };
    let reset = if no_color { "" } else { RESET };

    match entries.first() {
        Some(entry) => {
            let first_def = entry
                .meanings
                .first()
                .and_then(|m| m.definitions.first())
                .map(|d| d.definition.as_str())
                .unwrap_or("No definition found.");
            format!("{}{}{}{}: {}", bold, cyan, entry.word, reset, first_def)
        }
        None => "No definition found.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        std::path::Path::new(&dir).join("tests/fixtures").join(name)
    }

    #[test]
    fn test_render_single_entry() {
        let json = fs::read_to_string(fixture_path("ephemeral.json")).unwrap();
        let entries: Vec<Entry> = serde_json::from_str(&json).unwrap();
        let output = render_entries(&entries, false);

        assert!(output.contains(&format!("{}{}EPHEMERAL{}", BOLD, CYAN, RESET)));
        assert!(output.contains(&format!("{}/ɪˈfem(ə)r(ə)l/{}", DIM, RESET)));
        assert!(output.contains(&format!("{}{}ADJECTIVE{}", BOLD, MAGENTA, RESET)));
        assert!(output.contains("Lasting for a very short time."));
        assert!(output.contains(&format!("{}\"fashions are ephemeral\"{}", DIM, RESET)));
        assert!(output.contains(&format!("{}Synonyms:{}", BOLD, RESET)));
        assert!(output.contains(&format!(
            "{}transitory, transient, fleeting, short-lived{}",
            GREEN, RESET
        )));
        assert!(output.contains(&format!("{}Antonyms:{}", BOLD, RESET)));
        assert!(output.contains(&format!(
            "{}permanent, eternal, enduring{}",
            YELLOW, RESET
        )));
    }

    #[test]
    fn test_render_no_color() {
        let json = fs::read_to_string(fixture_path("ephemeral.json")).unwrap();
        let entries: Vec<Entry> = serde_json::from_str(&json).unwrap();
        let output = render_entries(&entries, true);

        assert!(output.contains("EPHEMERAL"));
        assert!(output.contains("ADJECTIVE"));
        assert!(output.contains("Lasting for a very short time."));
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn test_render_short() {
        let json = fs::read_to_string(fixture_path("ephemeral.json")).unwrap();
        let entries: Vec<Entry> = serde_json::from_str(&json).unwrap();
        let output = render_short(&entries, false);

        assert!(output.contains("ephemeral"));
        assert!(output.contains("Lasting for a very short time."));
        assert!(!output.contains('\n'));
    }

    #[test]
    fn test_render_short_no_color() {
        let json = fs::read_to_string(fixture_path("ephemeral.json")).unwrap();
        let entries: Vec<Entry> = serde_json::from_str(&json).unwrap();
        let output = render_short(&entries, true);

        assert!(output.contains("ephemeral"));
        assert!(output.contains("Lasting for a very short time."));
        assert!(!output.contains("\x1b["));
        assert!(!output.contains('\n'));
    }

    #[test]
    fn test_render_multiple_entries() {
        let json = fs::read_to_string(fixture_path("run.json")).unwrap();
        let entries: Vec<Entry> = serde_json::from_str(&json).unwrap();
        let output = render_entries(&entries, false);

        assert!(output.contains(&format!("{}{}RUN{}", BOLD, CYAN, RESET)));
        assert!(output.contains(&format!("{}{}VERB{}", BOLD, MAGENTA, RESET)));
        assert!(output.contains(&format!("{}{}NOUN{}", BOLD, MAGENTA, RESET)));
        assert_eq!(output.matches(&format!("{}{}RUN{}", BOLD, CYAN, RESET)).count(), 2);
    }
}
