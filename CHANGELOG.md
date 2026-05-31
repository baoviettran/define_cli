# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## v4 — Audio Pronunciation — 2026-05-31

**Shipped:** `65f46ea..c1bb664`

Audio pronunciation playback from the terminal.

### Added
- `define --pronounce hello` — fetch and play MP3 pronunciation through speakers
- `--accent us|uk|au` — choose pronunciation accent (default: us)
- Phonetic text fallback when no audio URL is available
- Graceful error handling for missing audio devices

### Dependencies
- `rodio` 0.22 (cross-platform audio playback via cpal)

### New files
- `src/audio.rs` — fetch audio bytes, decode MP3, play through speakers

## v3 — Cache & History — 2026-05-31

**Shipped:** `87dd8c4..a0dea8a`

Local cache for instant repeat lookups and history tracking.

### Added
- Local cache: responses saved to `~/.define/cache/<word>.json`
- Cache hit: instant response, works offline
- Cache miss: fetch from API, then store
- `define cache clear` — wipe all cached responses
- History log: every lookup appended to `~/.define/history.txt`
- `define history` — list all looked-up words, newest first
- `define history --stats` — show total lookups and most frequent words
- `define history clear` — wipe lookup history

### Dependencies
- `dirs` 6 (cross-platform home directory resolution)

### New files
- `src/cache.rs` — read/write/clear cache logic
- `src/history.rs` — append/read/clear history log with stats

## v2 — Flags & Polish — 2026-05-28

**Shipped:** `d22eac8..0b28305`

Command-line flags for output control and composability.

### Added
- `--short` — show only the first definition (one line)
- `--json` — output raw JSON for piping to `jq` or other tools
- `--no-color` — plain text output, no ANSI codes
- `--pronounce` — print audio pronunciation URL
- `-h / --help` — auto-generated help text
- `-V / --version` — print version
- Auto-detect TTY: colors disabled when output is piped
- Codebase split into `api.rs`, `render.rs`, `cli.rs` modules

### Dependencies
- `clap` 4 (argument parsing, derive)

## v1 — Core Lookup — 2026-05-27

**Shipped:** `8d45beb..4aa2787`

The foundation. Look up any English word from the terminal.

### Added
- `define <word>` — look up definitions via the Free Dictionary API
- Colored ANSI output: word header (bold cyan), phonetics (dim), part of speech (bold magenta), definitions, examples (dim), synonyms (green), antonyms (yellow)
- URL encoding for words with special characters (naïve, façade, etc.)
- Graceful error handling: word not found, network errors, bad JSON
- Correct exit codes (0 success, 1 error)
- Errors to stderr, output to stdout
- Test fixtures for unit testing without live API
- 4 unit tests (deserialization + rendering)

### Dependencies
- `ureq` 2.5 (HTTP client)
- `serde` 1.0 + `serde_json` 1.0 (JSON deserialization)
- `urlencoding` 2.1 (URL percent-encoding)
