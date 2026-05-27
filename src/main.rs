use serde::Deserialize;
use std::fs;

#[derive(Deserialize, Debug)]
struct Entry {
    word: String,
    phonetics: Vec<Phonetic>,
    meanings: Vec<Meaning>,
}

#[derive(Deserialize, Debug)]
struct Phonetic {
    text: Option<String>,
    audio: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Meaning {
    part_of_speech: String,
    definitions: Vec<Definition>,
    synonyms: Vec<String>,
    antonyms: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct Definition {
    definition: String,
    example: Option<String>,
}

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[36m";
const MAGENTA: &str = "\x1b[35m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";
const DIVIDER: &str = "──────────────────────────────────────────────────";

fn render_entries(entries: &[Entry]) -> String {
    let mut out = String::new();

    for entry in entries {
        out.push_str(&format!(
            "{}{}{}{}\n",
            BOLD,
            CYAN,
            entry.word.to_uppercase(),
            RESET
        ));

        // pick first non-empty phonetic
        if let Some(phonetic) = entry
            .phonetics
            .iter()
            .find_map(|p| p.text.as_ref().and_then(|t| if t.is_empty() { None } else { Some(t) }))
        {
            out.push_str(&format!("{}{}{}\n", DIM, phonetic, RESET));
        }

        out.push_str(&format!("{}{}{}\n", DIM, DIVIDER, RESET));

        for meaning in &entry.meanings {
            out.push('\n');
            out.push_str(&format!(
                "{}{}{}{}\n",
                BOLD,
                MAGENTA,
                meaning.part_of_speech.to_uppercase(),
                RESET
            ));
            out.push('\n');

            for (i, def) in meaning.definitions.iter().take(3).enumerate() {
                out.push_str(&format!("  {}. {}\n", i + 1, def.definition));
                if let Some(example) = &def.example {
                    out.push_str(&format!("     {}\"{}\"{}\n", DIM, example, RESET));
                }
            }

            if !meaning.synonyms.is_empty() {
                let syns: Vec<&str> = meaning.synonyms.iter().take(5).map(|s| s.as_str()).collect();
                out.push_str(&format!(
                    "  {}Synonyms:{} {}{}{}\n",
                    BOLD,
                    RESET,
                    GREEN,
                    syns.join(", "),
                    RESET
                ));
            }

            if !meaning.antonyms.is_empty() {
                let ants: Vec<&str> = meaning.antonyms.iter().take(5).map(|s| s.as_str()).collect();
                out.push_str(&format!(
                    "  {}Antonyms:{} {}{}{}\n",
                    BOLD,
                    RESET,
                    YELLOW,
                    ants.join(", "),
                    RESET
                ));
            }
        }

        out.push_str(&format!("{}{}{}\n", DIM, DIVIDER, RESET));
    }

    out
}

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        std::path::Path::new(&dir).join("tests/fixtures").join(name)
    }

    #[test]
    fn test_deserialize_ephemeral() {
        let json = fs::read_to_string(fixture_path("ephemeral.json")).unwrap();
        let entries: Vec<Entry> = serde_json::from_str(&json).unwrap();

        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.word, "ephemeral");

        // phonetics — first has text, second has empty string
        assert_eq!(entry.phonetics.len(), 2);
        assert_eq!(
            entry.phonetics[0].text,
            Some("/ɪˈfem(ə)r(ə)l/".to_string())
        );
        assert_eq!(entry.phonetics[1].text, Some("".to_string()));

        // meanings
        assert_eq!(entry.meanings.len(), 1);
        assert_eq!(entry.meanings[0].part_of_speech, "adjective");
        assert_eq!(entry.meanings[0].definitions.len(), 1);
        assert_eq!(
            entry.meanings[0].definitions[0].definition,
            "Lasting for a very short time."
        );
        assert_eq!(
            entry.meanings[0].definitions[0].example,
            Some("fashions are ephemeral".to_string())
        );
        assert_eq!(
            entry.meanings[0].synonyms,
            vec!["transitory", "transient", "fleeting", "short-lived"]
        );
        assert_eq!(
            entry.meanings[0].antonyms,
            vec!["permanent", "eternal", "enduring"]
        );
    }

    #[test]
    fn test_deserialize_run() {
        let json = fs::read_to_string(fixture_path("run.json")).unwrap();
        let entries: Vec<Entry> = serde_json::from_str(&json).unwrap();

        assert_eq!(entries.len(), 2);

        // first entry: verb + noun
        assert_eq!(entries[0].meanings.len(), 2);
        assert_eq!(entries[0].meanings[0].part_of_speech, "verb");
        assert_eq!(entries[0].meanings[1].part_of_speech, "noun");

        // second entry: noun
        assert_eq!(entries[1].meanings.len(), 1);
        assert_eq!(entries[1].meanings[0].part_of_speech, "noun");
    }

    #[test]
    fn test_render_single_entry() {
        let json = fs::read_to_string(fixture_path("ephemeral.json")).unwrap();
        let entries: Vec<Entry> = serde_json::from_str(&json).unwrap();
        let output = render_entries(&entries);

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
    fn test_render_multiple_entries() {
        let json = fs::read_to_string(fixture_path("run.json")).unwrap();
        let entries: Vec<Entry> = serde_json::from_str(&json).unwrap();
        let output = render_entries(&entries);

        // should show RUN header
        assert!(output.contains(&format!("{}{}RUN{}", BOLD, CYAN, RESET)));
        // should show both verb and noun parts of speech
        assert!(output.contains(&format!("{}{}VERB{}", BOLD, MAGENTA, RESET)));
        assert!(output.contains(&format!("{}{}NOUN{}", BOLD, MAGENTA, RESET)));
        // two entries means two RUN headers
        assert_eq!(output.matches(&format!("{}{}RUN{}", BOLD, CYAN, RESET)).count(), 2);
    }
}