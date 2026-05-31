use std::fs;
use std::path::PathBuf;

/// Returns the base directory for all define_cli data.
/// Default: ~/.define/
/// Creates it if it doesn't exist.
pub fn base_dir(override_dir: Option<&PathBuf>) -> Result<PathBuf, String> {
    let dir = match override_dir {
        Some(d) => d.clone(),
        None => dirs::home_dir()
            .ok_or_else(|| "Could not find home directory.".to_string())?
            .join(".define"),
    };
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create directory {}: {}", dir.display(), e))?;
    Ok(dir)
}

/// Returns the cache directory path: <base>/cache/
/// Creates it if it doesn't exist.
pub fn cache_dir(override_dir: Option<&PathBuf>) -> Result<PathBuf, String> {
    let dir = base_dir(override_dir)?.join("cache");
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create cache directory: {}", e))?;
    Ok(dir)
}

/// Returns the cache file path for a given word.
/// The word is URL-encoded to handle special characters.
fn cache_path(word: &str, override_dir: Option<&PathBuf>) -> Result<PathBuf, String> {
    let encoded = urlencoding::encode(word);
    Ok(cache_dir(override_dir)?.join(format!("{}.json", encoded)))
}

/// Check cache for a word. Returns the raw JSON string if found.
/// Returns None if the word is not cached.
pub fn read_cache(word: &str, override_dir: Option<&PathBuf>) -> Result<Option<String>, String> {
    let path = cache_path(word, override_dir)?;
    if path.exists() {
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read cache for '{}': {}", word, e))?;
        Ok(Some(content))
    } else {
        Ok(None)
    }
}

/// Write raw API JSON to the cache for this word.
pub fn write_cache(word: &str, json: &str, override_dir: Option<&PathBuf>) -> Result<(), String> {
    let path = cache_path(word, override_dir)?;
    fs::write(&path, json)
        .map_err(|e| format!("Failed to write cache for '{}': {}", word, e))
}

/// Delete all files in the cache directory.
/// Returns the number of files deleted.
pub fn clear_cache(override_dir: Option<&PathBuf>) -> Result<usize, String> {
    let dir = cache_dir(override_dir)?;
    let mut count = 0;
    let entries = fs::read_dir(&dir)
        .map_err(|e| format!("Failed to read cache directory: {}", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read cache entry: {}", e))?;
        if entry.path().is_file() {
            fs::remove_file(entry.path())
                .map_err(|e| format!("Failed to delete cache file: {}", e))?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a unique temp directory for test isolation.
    fn test_dir() -> PathBuf {
        let dir = std::env::temp_dir()
            .join("define_cli_test_cache")
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
    fn test_cache_dir_created() {
        let dir = test_dir();
        let cache = cache_dir(Some(&dir)).unwrap();
        assert!(cache.exists());
        assert!(cache.ends_with("cache"));
    }

    #[test]
    fn test_write_and_read_cache() {
        let dir = test_dir();
        let json = r#"[{"word":"hello"}]"#;
        write_cache("hello", json, Some(&dir)).unwrap();
        let result = read_cache("hello", Some(&dir)).unwrap();
        assert_eq!(result, Some(json.to_string()));
    }

    #[test]
    fn test_read_cache_miss() {
        let dir = test_dir();
        let result = read_cache("nonexistent", Some(&dir)).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_clear_cache() {
        let dir = test_dir();
        write_cache("hello", "data1", Some(&dir)).unwrap();
        write_cache("world", "data2", Some(&dir)).unwrap();
        let count = clear_cache(Some(&dir)).unwrap();
        assert_eq!(count, 2);
        assert_eq!(read_cache("hello", Some(&dir)).unwrap(), None);
        assert_eq!(read_cache("world", Some(&dir)).unwrap(), None);
    }

    #[test]
    fn test_clear_cache_empty() {
        let dir = test_dir();
        let count = clear_cache(Some(&dir)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_cache_special_characters() {
        let dir = test_dir();
        let json = r#"[{"word":"naïve"}]"#;
        write_cache("naïve", json, Some(&dir)).unwrap();
        let result = read_cache("naïve", Some(&dir)).unwrap();
        assert_eq!(result, Some(json.to_string()));
    }
}
