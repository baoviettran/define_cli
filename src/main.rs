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
}