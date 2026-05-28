use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Entry {
    pub word: String,
    pub phonetics: Vec<Phonetic>,
    pub meanings: Vec<Meaning>,
}

#[derive(Deserialize, Debug)]
pub struct Phonetic {
    pub text: Option<String>,
    pub audio: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Meaning {
    pub part_of_speech: String,
    pub definitions: Vec<Definition>,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct Definition {
    pub definition: String,
    pub example: Option<String>,
}

pub fn fetch_definition(word: &str) -> Result<Vec<Entry>, String> {
    let encoded = urlencoding::encode(word);
    let url = format!(
        "https://api.dictionaryapi.dev/api/v2/entries/en/{}",
        encoded
    );

    let response = match ureq::get(&url).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(404, _)) => {
            return Err(format!("Word \"{}\" not found in dictionary.", word));
        }
        Err(ureq::Error::Status(code, _)) => {
            return Err(format!("API returned status {}.", code));
        }
        Err(e) => {
            return Err(format!("Network error: {}", e));
        }
    };

    serde_json::from_reader(response.into_reader())
        .map_err(|e| format!("Failed to parse response: {}", e))
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
        let json = std::fs::read_to_string(fixture_path("ephemeral.json")).unwrap();
        let entries: Vec<Entry> = serde_json::from_str(&json).unwrap();

        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.word, "ephemeral");

        assert_eq!(entry.phonetics.len(), 2);
        assert_eq!(
            entry.phonetics[0].text,
            Some("/ɪˈfem(ə)r(ə)l/".to_string())
        );
        assert_eq!(entry.phonetics[1].text, Some("".to_string()));

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
        let json = std::fs::read_to_string(fixture_path("run.json")).unwrap();
        let entries: Vec<Entry> = serde_json::from_str(&json).unwrap();

        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].meanings.len(), 2);
        assert_eq!(entries[0].meanings[0].part_of_speech, "verb");
        assert_eq!(entries[0].meanings[1].part_of_speech, "noun");

        assert_eq!(entries[1].meanings.len(), 1);
        assert_eq!(entries[1].meanings[0].part_of_speech, "noun");
    }
}
