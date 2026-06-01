# Project Research Summary

**Project:** define_cli
**Domain:** Terminal-native English dictionary CLI with TUI, vocabulary quiz, word comparison, and offline support
**Researched:** 2026-06-01
**Confidence:** HIGH

## Executive Summary

`define_cli` is a Rust CLI dictionary tool that has shipped v1-v4 (core lookup, flags, cache/history, audio pronunciation) and is planning v5-v6+ (compare mode, TUI, quiz mode, offline dictionary, distribution). Research across stack, features, architecture, and pitfalls converges on a clear strategy: build a shared `LookupResolver` data layer, introduce a two-path dispatch (plain text vs TUI), and progressively add output modes (compare, TUI, quiz) that all consume the same data abstraction.

The recommended stack adds `ratatui`/`crossterm` for TUI (feature-gated behind `tui` flag, mirroring the existing `audio` pattern), `rayon` or `std::thread` for concurrent lookups in compare mode, `chrono` for cache expiry, and `cargo-dist` for post-v6 distribution. The offline dictionary should be cache-based: pre-fetch common words from the Free Dictionary API rather than embedding external datasets, avoiding binary bloat and licensing complications.

Key risks are concentrated in three areas: (1) terminal state corruption from `std::process::exit()` bypassing destructors when TUI is active -- requires refactoring before any TUI work; (2) crossterm version diamond dependencies causing silent keystroke loss -- mitigated by pinning `crossterm = "0.29"` to match ratatui's backend feature; (3) interleaved stdout from concurrent threads in compare mode -- solved by collecting all results first, then rendering sequentially. All three have well-documented prevention strategies.

## Key Findings

### Recommended Stack

New dependencies are incremental, one major concept per version, matching the ROADMAP constraint.

**Core technologies (new):**
- **ratatui 0.30 + crossterm 0.29:** TUI framework and terminal backend -- de facto standard for Rust TUIs; must pin crossterm version to avoid diamond dependency conflicts
- **rayon 1.12** (or `std::thread`): Data-parallelism for concurrent word lookups -- zero-config thread pool with `par_iter()`
- **chrono 0.4:** Timestamp handling for cache expiry -- lightweight, only need `Utc::now()` and date arithmetic
- **rand 0.9:** Random selection for quiz question generation -- standard RNG, extremely lightweight
- **indicatif 0.17:** Progress bars for bulk offline dictionary download -- optional, used by `cargo` itself
- **cargo-dist 0.30:** Cross-platform binary distribution (dev tool only) -- sets up GitHub Actions CI/CD, generates shell installers and Homebrew taps

**Keep as-is:** `ureq` (blocking HTTP, no need for async), `serde`/`serde_json`, `clap`, `dirs`, `rodio` (feature-gated). Do NOT switch to `reqwest`/`tokio` -- async runtime is overkill for parallel word lookups.

**Feature flags:** Add `tui` flag gating `ratatui`/`crossterm` (same pattern as `audio`). When disabled, `--tui` is hidden from help.

### Expected Features

**Must have (table stakes remaining):**
- Scrollable TUI for long definitions -- words like "run" have 30+ meanings that flood stdout
- Concurrent word fetching for compare mode -- sequential fetches feel slow for 3+ words
- Keyboard interrupt handling -- Ctrl+C must restore terminal state in TUI/quiz
- Terminal resize handling -- TUI must re-render on window resize

**Should have (competitive differentiators):**
- Interactive side-by-side compare panels (TUI) -- no other dictionary tool offers this; killer feature for writers comparing synonyms
- Multiple quiz types (multiple choice, fill-in-the-blank, matching) -- variety drives engagement
- Audio in quiz mode -- reuse existing `rodio` integration on answer reveal
- TUI browse/exploration mode -- fzf-like interface for definitions (defer to v7+)

**Defer (v7+):**
- Spaced repetition (SM-2 algorithm) -- start with simple scoring, add scheduling later
- TUI browse/exploration mode -- needs offline dictionary for suggestion word list
- Configurable themes -- low priority, nice polish item

**Anti-features (explicitly NOT build):**
- Translation/multi-language, web app, API server, cloud sync, AI-generated definitions, embedded dictionary in binary, native spell check

### Architecture Approach

The single most important structural change is introducing a **two-path dispatch** after data fetching:

```
                         [Plain path: render.rs -> stdout]
CLI parse -> fetch layer |
                         [TUI path:   tui.rs   -> terminal (interactive)]
```

**Major components (new):**
1. **`LookupResolver` (`src/lookup.rs`)** -- abstracts the three-tier lookup cascade (cache -> API -> offline fallback) into a single function. All output modes call this. Currently the cascade logic is inline in `main.rs` (lines 76-89); extracting it prevents duplication across compare, TUI, and quiz modes.
2. **`offline.rs`** -- manages on-demand download and lookup of a compact dictionary file (gzipped JSON in `Entry` schema, ~1-2MB for 5K words). Loads lazily into `HashMap` via `OnceLock`.
3. **`compare.rs`** -- concurrent word lookups using `std::thread::spawn` (v5 learning goal). Collects all results via `JoinHandle`, then renders sequentially to avoid output interleaving.
4. **`tui.rs`** -- ratatui App struct with event loop. Follows standard pattern: `ratatui::run()` for terminal lifecycle, local `App` struct passed by `&mut`, event poll -> state update -> draw cycle.
5. **`quiz.rs` + `quiz_state.rs`** -- quiz types (multiple choice, fill-in-the-blank), scoring, simplified spaced repetition (exponential backoff on `streak`), persistent state in `~/.define/quiz_state.json`.

The module graph remains a DAG rooted at `main.rs`. No circular dependencies. `lookup.rs` is the central hub.

### Critical Pitfalls

1. **Terminal state corruption on panic in TUI mode** -- `ratatui::run()` installs a panic hook that restores the terminal, but the codebase has 9 `std::process::exit(1)` calls that bypass destructors. Refactor all `exit()` to `Result` propagation before introducing TUI. This is a prerequisite for v6.
2. **Crossterm version diamond dependency** -- ratatui supports crossterm 0.27/0.28/0.29 via feature flags. Two versions in the dependency tree cause silent keystroke loss and broken terminal restore. Pin explicitly: `ratatui = { version = "0.30", features = ["crossterm_0_29"] }` and `crossterm = "0.29"`. Add `cargo tree | grep crossterm` to CI.
3. **Interleaved stdout from concurrent threads** -- never print from worker threads. Collect all results via `JoinHandle`, render sequentially in input order. Also: set HTTP timeouts on `ureq::Agent` before adding concurrency, and make cache writes atomic (write to `.tmp` then `rename()`).
4. **Offline dictionary data licensing** -- do not scrape the Free Dictionary API in bulk (likely violates ToS). Do not use WordNet (requires attribution) or ECDICT (unclear license). Recommended: use Google 10000 English word list (public domain) as a priority queue, fetch definitions from API on demand, store as user's personal cache. Zero licensing risk.
5. **Quiz state not persisted across sessions** -- design `quiz_state.json` from day one, not as an afterthought. Without persistence, spaced repetition is impossible and the `--hard` flag cannot work.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Infrastructure Prep + Offline Foundation
**Rationale:** Before adding concurrency (v5) or TUI (v6), prerequisite refactoring must happen: eliminate `exit()` calls, add HTTP timeouts, make cache writes atomic, and build the shared `LookupResolver`. The offline dictionary module is independent and can be built alongside these refactorings.
**Delivers:** `src/lookup.rs` (LookupResolver), `src/offline.rs` (offline dictionary download + lookup), refactored error handling (no more `exit()`), HTTP timeouts, atomic cache writes
**Addresses:** Concurrent fetching prerequisite, TUI prerequisite, offline fallback table stake
**Avoids:** Pitfall 1 (terminal corruption), Pitfall 3 (interleaved output), Pitfall 5 (exit across threads)

### Phase 2: Compare Mode (v5 -- Concurrency)
**Rationale:** Compare mode introduces the v5 learning goal (concurrency) without requiring TUI. Fully functional in plain-text mode. Depends on `lookup.rs` from Phase 1.
**Delivers:** `src/compare.rs`, `--compare` flag, concurrent word fetching via `std::thread::spawn`, plain-text columnar output, `render_compare()` in render.rs
**Uses:** `std::thread` for concurrency (per ROADMAP learning goal), existing `render.rs` for output
**Implements:** Compare component from architecture
**Avoids:** Pitfall 3 (interleaved output -- collect-then-render), Pitfall 5 (exit across threads -- already fixed in Phase 1)

### Phase 3: TUI Mode (v6 -- TUI Framework)
**Rationale:** TUI is the v6 learning goal. It depends on the clean dispatch structure and `lookup.rs` already in place. Building TUI before quiz ensures the rendering infrastructure is solid.
**Delivers:** `src/tui.rs` (feature-gated), `--tui` flag, scrollable definition view, TUI compare panels (side-by-side), `ratatui`/`crossterm` integration, terminal lifecycle management via `ratatui::run()`
**Uses:** `ratatui 0.30`, `crossterm 0.29` (pinned), `tui` feature flag
**Implements:** TUI component from architecture
**Avoids:** Pitfall 1 (terminal corruption -- `ratatui::run()` + no `exit()`), Pitfall 2 (crossterm version conflict -- pinned), Pitfall 7 (blocking audio -- spawn on separate thread)

### Phase 4: Quiz Mode (v6+ -- State Management)
**Rationale:** Quiz is the last major component because it depends on TUI for rendering, history for word source, and lookup for definitions. Building it last means all infrastructure is battle-tested.
**Delivers:** `src/quiz.rs`, `src/quiz_state.rs`, multiple quiz types (multiple choice, fill-in-the-blank), session scoring, persistent quiz state, simplified spaced repetition
**Implements:** Quiz component from architecture
**Avoids:** Pitfall 8 (state not persisted -- designed from day one), Pitfall 13 (sparse history -- re-fetch from cache/offline)

### Phase 5: Distribution
**Rationale:** Post-v6 polish. Distribution is a dev tool concern, not a runtime feature. Set up `cargo-dist`, GitHub Actions CI/CD, multi-platform binaries, and install methods (crates.io, shell installer, Homebrew tap).
**Delivers:** CI/CD pipeline, cross-platform release binaries (6 targets), shell installer, Homebrew tap, crates.io publishing
**Uses:** `cargo-dist 0.30`, `cargo-release 1.1`
**Avoids:** Pitfall 9 (name collision -- reserve early), Pitfall 10 (cross-compilation failures -- use native runners, provide audio/no-audio variants)

### Phase Ordering Rationale

- **Phase 1 before everything:** `exit()` elimination and `lookup.rs` are prerequisites for both v5 (concurrency) and v6 (TUI). Building the offline dictionary here also unblocks quiz wrong-answer generation later.
- **Phase 2 before Phase 3:** Compare mode works in plain text without TUI, proving the concurrency architecture. TUI can then layer on top of a working compare module.
- **Phase 3 before Phase 4:** Quiz is TUI-only; the TUI infrastructure must be stable before quiz adds its complexity.
- **Phase 5 is independent:** Distribution can happen after any phase, but makes most sense after all features are shipped.

### Research Flags

**Phases likely needing deeper research during planning:**
- **Phase 1 (Offline dictionary):** Offline data source licensing needs legal verification. The recommended approach (cache-based pre-fetch from Free Dictionary API) avoids most risk, but bulk fetching ToS should be confirmed.
- **Phase 4 (Quiz mode):** Quiz question generation quality (selecting good distractors) is unexplored. The spaced repetition algorithm (simplified SM-2) needs implementation research.

**Phases with standard patterns (skip research):**
- **Phase 2 (Compare mode):** `std::thread::spawn` + `JoinHandle` collect is a textbook Rust pattern. Plain-text columnar rendering is straightforward.
- **Phase 3 (TUI mode):** ratatui App struct + event loop is the canonical pattern. Well-documented in ratatui docs and examples.
- **Phase 5 (Distribution):** `cargo-dist init --yes` is a single command. GitHub Actions setup is automated.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All crate versions verified via crates.io and official docs. Alternatives rejected with clear rationale. ratatui 0.30, crossterm 0.29, rayon 1.12 all current as of 2026-06-01. |
| Features | HIGH | Table stakes derived from competitor analysis (dict, rdict, atlas.dict, cargo-thesaurust). Feature dependency graph is clear. Competitive advantage claim is defensible. |
| Architecture | HIGH | Two-path dispatch and LookupResolver pattern follow standard Rust CLI architecture. Module graph is a clean DAG. ratatui App pattern verified via Context7 docs. |
| Pitfalls | HIGH | All critical pitfalls backed by official documentation (ratatui, crossterm, ureq) or direct codebase analysis (9 exit() calls, CONCERNS.md). Prevention strategies are concrete and testable. |

**Overall confidence:** HIGH

### Gaps to Address

1. **Free Dictionary API bulk fetching ToS:** The recommended offline strategy involves batch-fetching 1K-5K words from the API. Need to verify the API's terms of service permit this. Mitigation: the `define offline download` command is user-initiated and rate-limited, not automated scraping. If ToS is unclear, fall back to a public-domain word list without definitions (autocomplete only) and rely on cache for offline definitions.
2. **Quiz distractor quality:** How to select plausible wrong answers for multiple-choice questions. Random selection from history may produce poor choices. Mitigation: start with random selection, refine based on testing. The offline dictionary word list provides a broader pool for distractors.
3. **ratatui 0.30 API stability:** Research used Context7 docs for ratatui 0.30. The API is recent (2025-12-26). Verify at implementation time that `ratatui::run()` API is stable and matches documented behavior.
4. **crates.io name availability:** Cannot verify `define-cli` or `define_cli` availability without web access. Reserve the name early. Consider `def` as binary name.

## Sources

### Primary (HIGH confidence)
- ratatui v0.30.0 official docs (docs.rs/ratatui/0.30.0) -- App pattern, `ratatui::run()`, crossterm backend compatibility, layout, widgets
- crossterm v0.29 official docs (Context7) -- event handling, raw mode, key events
- rayon v1.12.0 docs -- parallel iterators, `par_iter()`, `join`
- Free Dictionary API (GitHub: meetDeveloper/freeDictionaryAPI) -- response format, no API key required
- Project codebase analysis (`src/main.rs`, `src/cli.rs`, `src/api.rs`, `src/render.rs`, `src/cache.rs`, `src/history.rs`, `src/audio.rs`, `Cargo.toml`) -- current architecture, 9 `exit(1)` locations, existing patterns
- CONCERNS.md -- pre-identified risks for v5 and v6

### Secondary (MEDIUM confidence)
- cargo-dist v0.30.3 docs -- distribution pipeline, GitHub Actions setup
- Google 10000 English word list (GitHub: first20hours/google-10000-english) -- public domain frequency list for offline seeding
- dwyl/english-words (GitHub) -- 466K words, Unlicense, for autocomplete suggestions
- Competitive analysis: atlas.dict (Go, TUI+offline), rdict (Rust, Wiktionary-based), cargo-thesaurust (Rust, thesaurus)
- Dictionary data source comparison: WordNet (attribution required), GCIDE (public domain), Wiktionary dumps (CC-BY-SA, 3GB)

### Tertiary (LOW confidence)
- crates.io name availability for `define-cli` -- requires web verification
- Free Dictionary API rate limiting policy -- not documented, unknown
- Specific dictionary data source license terms -- general knowledge, need legal review for compliance

---
*Research completed: 2026-06-01*
*Ready for roadmap: yes*
