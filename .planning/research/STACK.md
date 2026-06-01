# Technology Stack

**Project:** define_cli
**Researched:** 2026-06-01
**Scope:** New dependencies needed for v5+ (TUI, offline dictionary, improved cache, compare mode, quiz mode, distribution)

## Recommended Stack

### Existing Dependencies (Keep As-Is)

These are already in use and require no changes. Do not upgrade versions mid-version unless a critical bug or security fix demands it.

| Crate | Current Lock | Purpose | Why Keep |
|-------|-------------|---------|----------|
| `ureq` | 2.12.1 | HTTP client | Synchronous, simple, no async runtime needed |
| `serde` + `serde_json` | 1.0.228 / 1.0.150 | JSON serialization | Already handles API response and cache deserialization |
| `clap` | 4.6.1 | CLI parser | Handles `--tui`, `--compare`, quiz subcommands |
| `dirs` | 6.0.0 | Platform directories | Path resolution for `~/.define/` |
| `rodio` | 0.22.2 (optional) | Audio playback | Feature-gated, working |

### New Dependencies by Feature Area

#### TUI Output Mode (`--tui` flag)

| Crate | Version | Purpose | Why This One | Confidence |
|-------|---------|---------|--------------|------------|
| `ratatui` | `0.30` | TUI framework | De facto standard for Rust TUIs. v0.30 (2025-12-26) is the latest, introduces modular architecture with `no_std` support, major widget/layout upgrades. The `ratatui` crate re-exports everything an app needs; no need to depend on `ratatui-core` or `ratatui-widgets` directly. | HIGH |
| `crossterm` | `0.29` | Terminal backend + event handling | Required alongside ratatui. Provides `CrosstermBackend`, raw mode, event polling (`poll()`/`read()`), key events. The ratatui docs explicitly say `cargo add ratatui crossterm`. Pure Rust, cross-platform, no external deps. | HIGH |
| `color-eyre` | `0.6` (dev) | Error handling for TUI | Recommended by ratatui's own hello-world example. Provides panic hooks that restore terminal state on crash -- critical for TUI apps where a panic leaves the terminal in raw mode. `eyre::Result` is more informative than `std::io::Result` for user-facing errors. | MEDIUM |

**Do NOT use:**
- `tui` (the original): Abandoned since 2023. `ratatui` is the active fork.
- `termion`: Does not support Windows. This project must be cross-platform (Linux, macOS, Windows).
- `iced` / `egui`: GUI frameworks, not terminal-native. Wrong paradigm.

**Feature gate:** TUI deps should be behind a `tui` feature flag (same pattern as `audio`). This keeps the binary small for users who only need plain-text output.

```toml
[features]
default = ["audio"]
audio = ["dep:rodio"]
tui = ["dep:ratatui", "dep:crossterm"]
```

When `tui` is disabled, `--tui` flag should be hidden (same pattern as `--accent` under audio gate).

#### Compare Mode (Concurrent Lookups)

| Crate | Version | Purpose | Why This One | Confidence |
|-------|---------|---------|--------------|------------|
| `rayon` | `1.12` | Data-parallelism | Perfect fit for "fetch these N words in parallel." `par_iter()` over a list of words, each fetch runs on the thread pool. Requires rustc 1.85+, which this project already exceeds (edition 2024). Zero-config thread pool, data-race free by construction. The ROADMAP already specifies v5 uses `std::thread or rayon`. | HIGH |

**Do NOT use:**
- `tokio`: Async runtime is heavyweight for "fetch 3-5 words concurrently." Introduces complexity (`.await`, async main, pinning) for no benefit in a synchronous CLI. `ureq` is blocking; wrapping it in `tokio::task::spawn_blocking` is just indirection.
- `reqwest`: Would require switching from `ureq` (blocking) to an async HTTP client, cascading async through the entire codebase. Not worth it for parallel word lookups.

**Pattern:**
```rust
use rayon::prelude::*;

// Fetch multiple words concurrently
let results: Vec<Result<Entry, String>> = words
    .par_iter()
    .map(|word| fetch_definition(word))
    .collect();
```

**Feature gate:** Not needed -- rayon is lightweight enough to include unconditionally once compare mode is built. No system deps.

#### Improved Cache (Expiry, Size Limits)

| Crate | Version | Purpose | Why This One | Confidence |
|-------|---------|---------|--------------|------------|
| `chrono` | `0.4` | Timestamp handling for cache expiry | Only need `chrono::Utc::now()` and date arithmetic for "is this cache entry older than 30 days?" Lightweight, well-maintained, version 0.4.44 is current. | HIGH |

No additional crate needed for cache size limits -- implement with `std::fs` (already in use). A simple LRU eviction strategy can be built by sorting files by last-access time and removing the oldest ones past a size threshold.

**Do NOT use:**
- SQLite (`rusqlite`): Overkill for JSON cache files. The current flat-file approach (`~/.define/cache/*.json`) is correct -- each word is one file, easy to inspect, easy to delete. Adding a database introduces a new paradigm and dependency for marginal benefit.
- `cached` / `moka`: These are in-memory caches. This project needs persistent disk cache with expiry.

#### Offline Fallback Dictionary

| Data Source | Size | Format | License | Why This One | Confidence |
|-------------|------|--------|---------|--------------|------------|
| Build from Free Dictionary API cache | Variable (user-dependent) | JSON (same as API response) | Free API, no key | The user's existing cache IS the offline dictionary. When a word has been looked up before, the cached JSON is already available offline. No separate download needed for previously-seen words. | HIGH |
| Self-built word list from cache history | Variable | JSON | N/A | Parse `~/.define/history.txt`, fetch definitions for all words when online, store as cache. `define offline download` command could batch-fetch the user's most-used words. | HIGH |

**Do NOT embed a dictionary in the binary:**
- The project constraint says "Keep binary reasonably small -- small on-demand dictionary, not embedded."
- A 1K-5K word dictionary would add 500KB-5MB to the binary.
- The Free Dictionary API data format is rich (phonetics, examples, synonyms) -- curating a static subset means losing that richness.

**Do NOT use external dictionary datasets:**
- `dwyl/english-words` (479K words): Only word list, no definitions. Useless for fallback.
- `ECDICT` (English-to-Chinese): Wrong language pair. Also 76K+ entries is too large.
- WordNet: Requires NLP parsing. The `parse_wiktionary_en` crate exists but adds complexity for uncertain quality.

**Recommended approach:** The offline fallback strategy should be:
1. **Cache-first, always**: Already implemented. If a word was looked up before, it works offline.
2. **Pre-fetch command**: `define offline download --words 1000` fetches the 1000 most common English words from the API and caches them.
3. **History-based seeding**: `define offline download --from-history` fetches all words from the user's history that aren't cached.
4. **Common words list**: Ship a small text file of ~1000 most common English words (not definitions, just the list). Use it to pre-populate cache on demand.

This requires zero new crates -- just a word frequency list (text file, ~10KB) and a download command.

**New crate for download progress:**
| Crate | Version | Purpose | Why This One | Confidence |
|-------|---------|---------|--------------|------------|
| `indicatif` | `0.17` | Progress bars for bulk downloads | When downloading 1000+ words, users need to see progress. `indicatif` is the standard Rust progress bar library, used by `cargo` itself. Lightweight, handles multi-line progress display. | MEDIUM |

#### Quiz Mode

No new crates needed beyond `ratatui`/`crossterm` (for TUI-enhanced quiz) and `chrono` (for spaced repetition timing). Quiz data comes from history and cache. The learning value of v6 is TUI patterns, not new dependencies.

| Crate | Version | Purpose | Why This One | Confidence |
|-------|---------|---------|--------------|------------|
| `rand` | `0.9` | Random selection for quiz questions | `rand` is the standard RNG crate. Version 0.9 is current. Needed for selecting random words from history for quiz mode, shuffling answer choices. Extremely lightweight. | HIGH |

#### Distribution

| Tool | Version | Purpose | Why This One | Confidence |
|-------|---------|---------|--------------|------------|
| `cargo-dist` (CLI tool) | `0.30` | Build + distribute cross-platform binaries | The industry-standard tool for distributing Rust CLIs. Generates shell installers, PowerShell installers, tarballs, and zip archives for 6+ platforms from a single `dist init` command. Sets up GitHub Actions CI/CD automatically. Supports Homebrew tap generation. Used by Astral (uv, ruff), Tig, and other popular Rust CLIs. Latest version 0.30.3 (2025-12-14). This is a dev tool, NOT a runtime dependency. | HIGH |
| `cargo-release` | `1.1` | Version bumping and changelog generation | Companion to cargo-dist. Automates version bumps, tag creation, and CHANGELOG updates. Runs locally before pushing. | MEDIUM |

**Do NOT use:**
- `cross`: Cross-compilation tool that requires Docker. cargo-dist handles cross-compilation via GitHub Actions runners natively (including ARM64 Linux runners which are now free).
- `cargo-packager`: More focused on GUI app bundles (DMG, NSIS). Overkill for a CLI binary.
- Manual release scripts: Error-prone, hard to maintain. cargo-dist handles checksums, signing, and attestation.

**Distribution targets:**
```
x86_64-unknown-linux-gnu
x86_64-unknown-linux-musl   # Static binary, no glibc dep
aarch64-unknown-linux-gnu    # ARM64 Linux (native runners)
x86_64-apple-darwin          # Intel Mac
aarch64-apple-darwin         # Apple Silicon Mac
x86_64-pc-windows-msvc       # Windows
```

**Install methods:**
1. `cargo install define_cli` (from crates.io)
2. One-liner shell installer: `curl ... | sh` (generated by cargo-dist)
3. Homebrew tap (generated by cargo-dist)
4. GitHub Releases direct download (generated by cargo-dist)

### Summary: Cargo.toml After All Versions

```toml
[package]
name = "define_cli"
version = "0.6.0"
edition = "2024"

[dependencies]
# Core (unchanged from v1)
ureq = "2.5"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
urlencoding = "2.1"
clap = { version = "4", features = ["derive"] }
dirs = "6"

# v4: Audio (optional)
rodio = { version = "0.22", optional = true }

# v5: Compare mode
rayon = "1.12"

# v5-v6: Cache expiry + quiz timing
chrono = "0.4"

# v6: Quiz mode
rand = "0.9"

# v5: Offline download progress
indicatif = { version = "0.17", optional = true }

# v6: TUI (optional)
ratatui = { version = "0.30", optional = true }
crossterm = { version = "0.29", optional = true }

[features]
default = ["audio"]
audio = ["dep:rodio"]
tui = ["dep:ratatui", "dep:crossterm"]
offline-download = ["dep:indicatif"]
```

Note: Exact version pins will depend on what's current at the time each version is built. Use `cargo add crate@">=X.Y, <X.(Y+1)"` or just `"X"` (semver compatible) and let Cargo resolve.

## Version Introduction Order

| Version | New Deps | Learning Concept |
|---------|----------|-----------------|
| v5 | `rayon`, `chrono`, `indicatif` (optional) | Data parallelism, concurrent lookups, cache expiry, offline pre-fetch |
| v6 | `ratatui`, `crossterm`, `rand` | TUI framework, interactive terminal UI, quiz logic |
| Post-v6 | (dev tools only) `cargo-dist`, `cargo-release` | Distribution, CI/CD |

This maps directly to the ROADMAP and each version's "one new Rust concept" constraint.

## Alternatives Considered

| Category | Recommended | Rejected | Why |
|----------|-------------|----------|-----|
| TUI framework | ratatui | tui (original) | Abandoned since 2023 |
| TUI framework | ratatui | termion | No Windows support |
| TUI framework | ratatui | iced/egui | GUI, not terminal-native |
| Concurrency | rayon | tokio | Async runtime is overkill for parallel word lookups |
| Concurrency | rayon | std::thread | More boilerplate, no work-stealing; rayon is the ROADMAP spec |
| HTTP client | ureq (keep) | reqwest | Would require async runtime, cascading change |
| Cache storage | flat files (keep) | SQLite | Overkill for per-word JSON files |
| Error handling (TUI) | color-eyre | anyhow | eyre has better panic hook for terminal state restoration |
| Distribution | cargo-dist | cross + manual | cargo-dist automates everything end-to-end |
| Dictionary data | Cache-based | Embedded dataset | Binary size constraint |
| Dictionary data | Cache-based | WordNet/Wiktionary | Complex parsing, uncertain quality |
| Progress bars | indicatif | dialoguer | indicatif is lighter, just progress bars |

## Installation Per Version

### v5 (Compare Mode + Improved Cache + Offline)
```bash
cargo add rayon chrono
cargo add indicatif --optional
```

### v6 (TUI + Quiz Mode)
```bash
cargo add ratatui --optional
cargo add crossterm --optional
cargo add rand
```

### Distribution Setup (Post-v6)
```bash
cargo install cargo-dist --locked
dist init --yes
```

## Sources

- ratatui v0.30.0 changelog: https://github.com/ratatui/ratatui/blob/main/ratatui/CHANGELOG.md (2025-12-26)
- ratatui quickstart: https://docs.rs/ratatui/0.30.0/index.html
- ratatui architecture (modular): https://github.com/ratatui/ratatui/blob/main/ARCHITECTURE.md
- crossterm event handling: https://context7.com/crossterm-rs/crossterm/llms.txt
- rayon README: https://github.com/rayon-rs/rayon/blob/main/README.md
- cargo-dist releases: https://github.com/axodotdev/cargo-dist/releases (v0.30.3, 2025-12-14)
- cargo-dist quickstart: https://github.com/axodotdev/cargo-dist/blob/main/book/src/quickstart/rust.md
- crates.io versions: rayon 1.12.0, ratatui 0.30.0, crossterm 0.29.0, chrono 0.4.44, uuid 1.23.2, color-eyre 0.6.5, rusqlite 0.40.0 (all verified via `cargo search`)
