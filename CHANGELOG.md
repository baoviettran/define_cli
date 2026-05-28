# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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
