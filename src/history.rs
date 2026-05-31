use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

/// Returns the history file path: <base>/history.txt
/// Creates the base directory if it doesn't exist.
pub fn history_path(override_dir: Option<&PathBuf>) -> Result<PathBuf, String> {
    let dir = crate::cache::base_dir(override_dir)?;
    Ok(dir.join("history.txt"))
}

/// Get current UTC time as ISO-8601 string.
fn now_iso8601() -> String {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    format!("{}", duration.as_secs())
}

/// Append a word lookup to the history file.
/// Format: "{timestamp}\t{word}\n"
pub fn append_history(word: &str, override_dir: Option<&PathBuf>) -> Result<(), String> {
    let path = history_path(override_dir)?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create history directory: {}", e))?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("Failed to open history file: {}", e))?;

    writeln!(file, "{}\t{}", now_iso8601(), word)
        .map_err(|e| format!("Failed to write history: {}", e))
}

/// A single history entry.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub word: String,
}

/// Read all history entries, newest first.
pub fn read_history(override_dir: Option<&PathBuf>) -> Result<Vec<HistoryEntry>, String> {
    let path = history_path(override_dir)?;

    if !path.exists() {
        return Ok(vec![]);
    }

    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read history file: {}", e))?;

    let mut entries: Vec<HistoryEntry> = content
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, '\t').collect();
            if parts.len() == 2 {
                Some(HistoryEntry {
                    timestamp: parts[0].to_string(),
                    word: parts[1].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    entries.reverse(); // newest first
    Ok(entries)
}

/// Delete the history file.
pub fn clear_history(override_dir: Option<&PathBuf>) -> Result<(), String> {
    let path = history_path(override_dir)?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete history file: {}", e))?;
    }
    Ok(())
}

/// Render history entries as a display string.
/// Format: "{word:<20} {timestamp}"
pub fn render_history(entries: &[HistoryEntry]) -> String {
    let mut out = String::new();
    for entry in entries {
        out.push_str(&format!("{:<20} {}\n", entry.word, entry.timestamp));
    }
    out
}

/// Render history statistics.
/// Shows total lookups and top words by frequency.
pub fn render_stats(entries: &[HistoryEntry]) -> String {
    use std::collections::HashMap;

    let mut freq: HashMap<&str, usize> = HashMap::new();
    for entry in entries {
        freq.entry(&entry.word).and_modify(|c| *c += 1).or_insert(1);
    }

    let mut sorted: Vec<(&str, usize)> = freq.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let mut out = String::new();
    out.push_str(&format!("Lookups: {}\n\n", entries.len()));
    out.push_str("Top words:\n");

    for (word, count) in sorted.iter().take(10) {
        out.push_str(&format!("  {:<20} {}\n", word, count));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> PathBuf {
        let dir = std::env::temp_dir()
            .join("define_cli_test_history")
            .join(format!(
                "{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_append_and_read_history() {
        let dir = test_dir();
        append_history("hello", Some(&dir)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        append_history("ephemeral", Some(&dir)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        append_history("hello", Some(&dir)).unwrap();

        let entries = read_history(Some(&dir)).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].word, "hello");
        assert_eq!(entries[1].word, "ephemeral");
        assert_eq!(entries[2].word, "hello");
    }

    #[test]
    fn test_read_empty_history() {
        let dir = test_dir();
        let entries = read_history(Some(&dir)).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_clear_history() {
        let dir = test_dir();
        append_history("hello", Some(&dir)).unwrap();
        clear_history(Some(&dir)).unwrap();
        let entries = read_history(Some(&dir)).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_clear_history_when_no_file() {
        let dir = test_dir();
        clear_history(Some(&dir)).unwrap();
    }

    #[test]
    fn test_render_history() {
        let entries = vec![
            HistoryEntry {
                timestamp: "1700000003".to_string(),
                word: "hello".to_string(),
            },
            HistoryEntry {
                timestamp: "1700000002".to_string(),
                word: "ephemeral".to_string(),
            },
        ];
        let output = render_history(&entries);
        assert!(output.contains("hello"));
        assert!(output.contains("ephemeral"));
        assert!(output.contains("1700000003"));
        assert!(output.contains("1700000002"));
    }

    #[test]
    fn test_render_stats() {
        let entries = vec![
            HistoryEntry { timestamp: "1".to_string(), word: "hello".to_string() },
            HistoryEntry { timestamp: "2".to_string(), word: "ephemeral".to_string() },
            HistoryEntry { timestamp: "3".to_string(), word: "hello".to_string() },
        ];
        let output = render_stats(&entries);
        assert!(output.contains("Lookups: 3"));
        assert!(output.contains("hello"));
        assert!(output.contains("ephemeral"));
    }
}
