# Architecture Research: Multi-Mode CLI with TUI, Offline Dictionary, Concurrency, and Quiz

**Project:** define_cli
**Researched:** 2026-06-01
**Mode:** Ecosystem
**Focus:** Component boundaries, data flow, and build order for adding TUI, offline dictionary, compare mode, quiz mode, and distribution to an existing pipeline-style Rust CLI.

## Current Architecture Summary

The current application is a single-threaded, synchronous pipeline:

```
CLI parse --> cache check --> API fetch --> deserialize --> render --> stdout
```

Seven modules in a DAG rooted at `main.rs`: `cli`, `api`, `render`, `cache`, `history`, `audio`. All functions return `Result<T, String>`. State is not held between invocations -- each run is fire-and-forget. Output is a plain `String` printed to stdout. There is no persistent application state beyond flat files on disk.

**Key constraint from the existing code:** `render.rs` returns `String` (not prints directly), which is a good foundation for the architecture below -- the TUI can consume `Entry` structs instead of rendered strings.

---

## Recommended Architecture: Two-Path Dispatch

The core architectural change is introducing a **dispatch fork** after data fetching, before output. The data layer (fetch, cache, offline fallback, deserialize) stays shared. The output layer splits into two independent paths:

```
                         ┌─ Plain path: render.rs ──────> stdout (String)
CLI parse ──> fetch ─────┤
                         └─ TUI path:   tui.rs ────────> terminal (interactive)
```

This is the single most important structural decision. It preserves pipe-friendliness by default (plain path) and adds TUI as an opt-in mode. The fetch layer does not care which output mode is active.

### System Overview

```text
┌─────────────────────────────────────────────────────────────────────┐
│                         CLI Layer                                   │
│                       src/cli.rs                                     │
│     Cli struct, Commands enum, --tui, --compare, --quiz flags        │
├─────────────────────────────────────────────────────────────────────┤
│                        Orchestrator                                  │
│                      src/main.rs                                     │
│   Routes to: plain pipeline, TUI app, compare mode, quiz mode        │
├─────────────────────────────────────────────────────────────────────┤
│                      Fetch Layer (shared)                            │
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐                       │
│  │ cache.rs │  │ offline  │  │  api.rs      │                        │
│  │ (v3)     │  │ (new)    │  │  (v1)        │                        │
│  └────┬─────┘  └────┬─────┘  └──────┬───────┘                        │
│       │              │               │                                │
│       └──────────────┴───────────────┘                                │
│              LookupResolver (new)                                     │
│    Tries: cache --> API --> offline fallback                          │
├─────────────────────────────────────────────────────────────────────┤
│                    Output Layer (forked)                              │
│  ┌──────────┐  ┌───────────┐  ┌──────────┐  ┌───────────┐         │
│  │render.rs │  │ tui.rs    │  │compare.rs│  │ quiz.rs   │          │
│  │(plain)   │  │(new, v5+) │  │(new, v5) │  │(new, v6+) │          │
│  └──────────┘  └───────────┘  └──────────┘  └───────────┘         │
├─────────────────────────────────────────────────────────────────────┤
│                    Storage Layer                                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐                  │
│  │ cache.rs │  │history.rs│  │ quiz_state.rs     │                   │
│  │(v3)      │  │(v3)      │  │(new, v6+)         │                   │
│  └──────────┘  └──────────┘  └──────────────────┘                  │
├─────────────────────────────────────────────────────────────────────┤
│                    Cross-cutting                                     │
│  ┌──────────┐  ┌──────────┐                                          │
│  │ audio.rs │  │ offline  │                                          │
│  │(v4, opt) │  │ dictionary│                                          │
│  └──────────┘  └──────────┘                                          │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Component Boundaries

### 1. Fetch Layer -- `LookupResolver` (new module: `src/lookup.rs`)

**Responsibility:** Abstract the three-tier lookup strategy into a single function. Callers do not know or care whether data came from cache, API, or offline dictionary.

**Communicates with:** `api.rs`, `cache.rs`, `offline.rs`. Returns `Vec<Entry>`.

**Proposed interface:**
```rust
// src/lookup.rs

pub fn lookup(word: &str) -> Result<LookupResult, String> {
    let override_dir = None; // caller can pass test dirs
    lookup_with_dir(word, override_dir)
}

pub fn lookup_with_dir(word: &str, override_dir: Option<&PathBuf>)
    -> Result<LookupResult, String>
```

**Rationale:** Currently, `main.rs` has inline cache-check-then-fetch logic (lines 76-89). Moving this into a dedicated resolver module means the compare mode (which calls lookup for N words) and quiz mode both reuse the same three-tier strategy without duplicating the cascade logic.

**Why not keep it in `main.rs`:** The cascade logic (cache -> API -> offline) will grow from 2 tiers to 3, and will be called from multiple modes (plain, compare, quiz, TUI). Extracting it now prevents duplication.

**Build order:** Build before compare mode (v5), since compare calls lookup for multiple words concurrently.

### 2. Offline Dictionary Module -- `src/offline.rs` (new)

**Responsibility:** Provide word definitions when the API is unreachable and cache has no entry. Downloads a compact dictionary file on demand to `~/.define/offline/`. Looks up words from this local file.

**Communicates with:** `cache.rs` (shares `base_dir()` for data directory). Filesystem only.

**Data format decision -- flat JSON (recommended):**

Use a single gzipped JSON file with the same `Entry` schema as the API. This means:
- The same `serde::Deserialize` structs work for both API responses and offline data
- No new types needed
- Can be built by pre-fetching common words from the Free Dictionary API and concatenating the JSON arrays
- Gzip compression reduces a 5K-word dictionary from ~2-5MB to ~500KB-1MB

**Why not SQLite:** The existing project philosophy is flat-file storage with zero extra dependencies. A single gzipped JSON file loaded into a `HashMap<String, Vec<Entry>>` at startup gives O(1) lookups without pulling in `rusqlite`. For 5K words this fits comfortably in memory (< 10MB).

**Why not embed in binary:** Embedding would inflate the binary and prevent updates without a new release. On-demand download respects the project's "small binary" constraint and allows dictionary updates independently.

**Proposed interface:**
```rust
// src/offline.rs

/// Check if offline dictionary is available on disk.
pub fn is_available(override_dir: Option<&PathBuf>) -> bool;

/// Download offline dictionary (~500KB-1MB gzipped) to ~/.define/offline/.
pub fn download(override_dir: Option<&PathBuf>) -> Result<(), String>;

/// Look up a word from the offline dictionary. Returns None if word not present.
pub fn lookup_word(word: &str, override_dir: Option<&PathBuf>)
    -> Result<Option<Vec<api::Entry>>, String>;
```

**Loading strategy:** Lazy-load on first use. Parse the gzipped JSON into a `HashMap<String, Vec<Entry>>` and keep it in a `OnceLock` or `std::sync::LazyLock` for the process lifetime. Subsequent lookups are a HashMap get.

**Build order:** Can be built independently of TUI and compare. Needed before the `lookup.rs` resolver is complete.

### 3. TUI Module -- `src/tui.rs` (new, feature-gated)

**Responsibility:** Interactive terminal UI using ratatui. Scrollable definition display, keyboard navigation, tab switching between multiple word entries.

**Communicates with:** `lookup.rs` (for data), `api.rs` (for `Entry` types), `render.rs` (conceptually, for styling decisions, though ratatui uses its own styling).

**Architecture pattern: App struct with event loop.**

Based on ratatui's documented pattern (Context7, HIGH confidence), the TUI module should follow the standard App struct pattern:

```rust
// src/tui.rs
#![cfg(feature = "tui")]

use ratatui::{Frame, DefaultTerminal};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};

pub struct App {
    /// Current mode state
    mode: TuiMode,
    /// Should the app quit?
    should_quit: bool,
}

pub enum TuiMode {
    /// Displaying a single word definition
    Definition {
        entries: Vec<api::Entry>,
        scroll: usize,
        /// Active meaning tab (noun/verb/adj)
        meaning_tab: usize,
    },
    /// Comparing multiple words side by side
    Compare {
        results: Vec<CompareResult>,
        selected_word: usize,
    },
    /// Quiz mode
    Quiz {
        quiz_state: quiz::QuizState,
    },
    /// Loading spinner while fetching
    Loading { word: String },
}

/// Entry point for --tui mode.
/// Called from main.rs after CLI parsing determines --tui is active.
pub fn run_tui(word: &str) -> Result<(), String> {
    // 1. Show loading screen
    // 2. Fetch data via lookup::lookup(word)
    // 3. Enter event loop: draw -> poll events -> update state -> draw
    // 4. On quit, terminal is restored by ratatui::run()
    todo!()
}
```

**Key architectural detail: `ratatui::run` handles terminal lifecycle.** Based on Context7 docs (HIGH confidence), `ratatui::run(|terminal| { ... })` automatically handles raw mode, alternate screen, and restoration on exit or panic. This eliminates the need for manual setup/teardown.

**Event loop pattern (from Context7, HIGH confidence):**
```rust
fn run_tui(word: &str) -> Result<(), String> {
    ratatui::run(|terminal| {
        let mut app = App::new();
        // Initial data fetch
        app.load_word(word)?;

        loop {
            terminal.draw(|frame| app.render(frame))?;
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.handle_key(key.code)?;
                    if app.should_quit { break Ok(()); }
                }
                Event::Resize(_, _) => { /* next draw handles it */ }
                _ => {}
            }
        }
    }).map_err(|e| e.to_string())
}
```

**Why feature-gated:** ratatui + crossterm adds significant compile time and binary size. Users who only want plain text output should not pay this cost. The `tui` feature flag mirrors the existing `audio` feature pattern already established in the codebase.

**Widget layout for definition view:**
```text
┌─ define: ephemeral ──────────────────────────────────── [q] quit ─┐
│                                                                       │
│  /ɪˈfem(ə)r(ə)l/                                  [P] Play audio     │
│                                                                       │
│  ┌ adjective ────────────────────────────────────────────────────┐    │
│  │ 1. Lasting for a very short time.                            │    │
│  │    "fashions are ephemeral"                                   │    │
│  │                                                                │    │
│  │    synonyms: transitory, transient, fleeting, short-lived      │    │
│  │    antonyms: permanent, eternal, enduring                     │    │
│  └────────────────────────────────────────────────────────────────┘    │
│                                                                       │
│  ──────────────────────────────────────────────────────────────── │
│  source: cache | lookup: 2026-06-01T10:30:00Z                        │
└───────────────────────────────────────────────────────────────────────┘
```

**Widget layout for compare view:**
```text
┌─ define: compare ──────────────── [←/→] switch │ [q] quit ─────┐
│                                                                       │
│  ┌ ephemeral ─────────┐  ┌ transitory ─────────┐                   │
│  │ adjective           │  │ adjective            │                   │
│  │ 1. Lasting for a    │  │ 1. Not permanent or  │                   │
│  │    very short time. │  │    enduring; fleeting │                   │
│  │                     │  │                      │                   │
│  │ syn: transient,     │  │ syn: transient,       │                   │
│  │ fleeting            │  │     momentary         │                   │
│  └─────────────────────┘  └──────────────────────┘                   │
│                                                                       │
└───────────────────────────────────────────────────────────────────────┘
```

**Build order:** TUI depends on `lookup.rs` being available for fetching data. But the simplest TUI (single word definition view) can be built first with a direct call to `api::fetch_definition()`, then refactored to use `lookup.rs` later. However, building `lookup.rs` first is the cleaner path.

### 4. Compare Module -- `src/compare.rs` (new)

**Responsibility:** Look up multiple words concurrently and render them side-by-side. In plain mode, sequentially or in columns. In TUI mode, as split panels.

**Communicates with:** `lookup.rs` (for fetching each word), `render.rs` (for plain-text output), `tui.rs` (for TUI panel rendering).

**Concurrency strategy: `std::thread` (not rayon).**

This is v5's learning goal. For N word lookups, spawn N threads using `std::thread::spawn`, collect results via `std::thread::JoinHandle`, and join them all. Each thread calls `lookup::lookup(word)`.

```rust
// src/compare.rs

use std::thread;

pub struct CompareResult {
    pub word: String,
    pub entries: Result<Vec<api::Entry>, String>,
}

pub fn compare_words(words: &[String]) -> Vec<CompareResult> {
    let handles: Vec<thread::JoinHandle<CompareResult>> = words
        .iter()
        .map(|word| {
            let w = word.clone();
            thread::spawn(move || {
                let entries = lookup::lookup(&w);
                CompareResult {
                    word: w,
                    entries,
                }
            })
        })
        .collect();

    handles
        .into_iter()
        .map(|h| h.join().unwrap_or_else(|_| CompareResult {
            word: "unknown".into(),
            entries: Err("Thread panicked".into()),
        }))
        .collect()
}
```

**Why `std::thread` instead of `rayon`:** The project's stated learning goal for v5 is `std::thread`. Rayon is better suited for data-parallel iteration over large collections, whereas here we have N independent network requests -- a classic thread-per-task pattern. `std::thread::spawn` is simpler, has no dependencies, and teaches the fundamental concept. If performance becomes a concern with many words (100+), rayon can be introduced later, but for 2-10 word comparisons the overhead difference is negligible.

**Interaction with blocking I/O:** `ureq` is blocking, but each thread blocks independently, so N threads make N concurrent requests. No async runtime needed. This is a standard Rust pattern for concurrent blocking I/O.

**Plain-text rendering:** When not in TUI mode, `compare.rs` renders each word's definitions sequentially (or in a columnar format if the terminal is wide enough), using the existing `render::render_entries()` function per word.

**TUI rendering:** When `--tui` is combined with `--compare`, the TUI module renders the compare results in horizontal split panels using `ratatui::Layout::horizontal`.

**Build order:** Requires `lookup.rs` to be available. Can be built before or in parallel with TUI.

### 5. Quiz Module -- `src/quiz.rs` + `src/quiz_state.rs` (new, feature-gated with `tui`)

**Responsibility:** Vocabulary quiz using words from lookup history. Multiple quiz types (fill-in-the-blank, multiple choice, matching). Spaced repetition for word scheduling. TUI-only (no plain-text quiz mode -- quiz requires interactivity).

**Communicates with:** `history.rs` (for word source), `offline.rs` or `lookup.rs` (for definitions), `tui.rs` (for rendering).

**Why TUI-only:** A quiz requires interactive input (selecting answers, typing words). This is fundamentally a TUI experience. Plain-text quiz would require reading stdin line-by-line, which is possible but a poor UX. Keep it TUI-only for v6.

**Quiz state management:**
```rust
// src/quiz_state.rs

use std::collections::HashMap;

pub struct QuizState {
    /// Current quiz type
    pub quiz_type: QuizType,
    /// Current question
    pub current: Option<Question>,
    /// Score tracking
    pub correct: usize,
    pub incorrect: usize,
    /// Word scheduling for spaced repetition
    pub scheduling: Scheduling,
    /// All words in the quiz pool
    pub pool: Vec<QuizWord>,
    /// Current index into the pool
    pub pool_index: usize,
}

pub enum QuizType {
    MultipleChoice,
    FillInTheBlank,
    Matching,
}

pub struct Question {
    pub prompt: String,
    pub answer: String,
    pub options: Vec<String>, // for multiple choice
}

pub struct QuizWord {
    pub word: String,
    pub entries: Vec<api::Entry>,
    /// When this word is next due (epoch seconds)
    pub next_due: u64,
    /// Consecutive correct answers (for scheduling)
    pub streak: u32,
}

/// Persistence for quiz progress
pub fn save_state(state: &QuizState, override_dir: Option<&PathBuf>)
    -> Result<(), String>;
pub fn load_state(override_dir: Option<&PathBuf>)
    -> Result<Option<QuizState>, String>;
```

**Spaced repetition (simplified SM-2):** Track per-word `next_due` timestamp and `streak`. On correct answer: `streak += 1`, `next_due = now + 2^streak minutes` (1m, 2m, 4m, 8m, 16m, capped at 1 day). On incorrect: `streak = 0`, `next_due = now + 1 minute`. This is a simple exponential backoff that captures the core idea of spaced repetition without the complexity of full Anki-style SM-2.

**Persistence:** Serialize `QuizState` as JSON to `~/.define/quiz_state.json`. Uses the same `base_dir()` pattern and `override_dir` for test isolation.

**Question generation:** Given a word from history:
- **Multiple choice:** Show definition, pick 3 wrong definitions from other words as distractors
- **Fill-in-the-blank:** Show "The word that means: <definition>"
- **Matching:** Show N words on left, N shuffled definitions on right

**Build order:** Quiz depends on `tui.rs` being available. It is the last major component to build.

### 6. Enhanced Render Module -- `src/render.rs` (existing, extended)

**Changes needed:**
- Keep all existing functions (`render_entries`, `render_short`) unchanged for backward compatibility
- Add `render_compare()` for plain-text multi-word column output
- The render module remains the plain-text output authority; TUI does not use these functions (ratatui has its own rendering)

**Build order:** Extend incrementally as new output modes are needed.

### 7. Module Graph After All Changes

```text
main.rs ──> cli.rs           (uses Cli, Commands, flags)
main.rs ──> lookup.rs        (uses lookup, LookupResult)
main.rs ──> render.rs        (uses render_entries, render_short, render_compare)
main.rs ──> compare.rs       (uses compare_words) [when --compare active]
main.rs ──> tui.rs           (uses run_tui) [feature-gated: "tui"]
main.rs ──> quiz.rs          (uses run_quiz) [feature-gated: "tui"]
main.rs ──> history.rs       (unchanged)
main.rs ──> cache.rs         (unchanged)
main.rs ──> audio.rs         (unchanged) [feature-gated: "audio"]

lookup.rs ──> cache.rs       (reads cache first)
lookup.rs ──> api.rs         (fetches from API)
lookup.rs ──> offline.rs     (fallback when API unavailable)

offline.rs ──> cache.rs      (uses base_dir for data directory)
offline.rs ──> api.rs        (uses Entry type for deserialization)

compare.rs ──> lookup.rs     (calls lookup for each word)
compare.rs ──> render.rs     (for plain-text output)
compare.rs ──> api.rs        (uses Entry type)

tui.rs ──> lookup.rs         (for data fetching)
tui.rs ──> compare.rs        (for compare panel data)
tui.rs ──> quiz.rs           (for quiz rendering)
tui.rs ──> api.rs            (uses Entry type)

quiz.rs ──> quiz_state.rs    (state management)
quiz.rs ──> history.rs       (word source)
quiz.rs ──> lookup.rs        (for fetching definitions)
quiz.rs ──> api.rs           (uses Entry type)

quiz_state.rs ──> api.rs     (uses Entry type)
quiz_state.rs ──> cache.rs   (uses base_dir for quiz_state.json)
```

No circular dependencies. The graph remains a DAG rooted at `main.rs`. `lookup.rs` is the central hub that all output modes depend on for data.

---

## Data Flow

### Flow 1: Plain Text Lookup (existing, refactored)

```
user types "define ephemeral"
       |
       v
  cli.rs: Cli::parse()
  word=Some("ephemeral"), tui=false
       |
       v
  main.rs: dispatch to plain path
       |
       v
  lookup.rs: lookup("ephemeral")
       |
       v
  cache.rs: read_cache("ephemeral")
    └── hit? ──> deserialize ──> return Vec<Entry>
    └── miss? ──>
       |
       v
  api.rs: fetch_raw("ephemeral")
    └── success? ──> cache.rs: write_cache() ──> deserialize ──> return Vec<Entry>
    └── error? ──>
       |
       v
  offline.rs: lookup_word("ephemeral")
    └── found? ──> return Vec<Entry>
    └── not found? ──> return Err("Word not found")
       |
       v
  render.rs: render_entries(&entries, no_color)
       |
       v
  stdout: print!(...)
```

### Flow 2: TUI Lookup (new)

```
user types "define --tui ephemeral"
       |
       v
  cli.rs: Cli::parse()
  word=Some("ephemeral"), tui=true
       |
       v
  main.rs: dispatch to tui::run_tui()
       |
       v
  tui.rs: App::new() + App::load_word("ephemeral")
       |
       v
  lookup.rs: lookup("ephemeral")  [same three-tier cascade as Flow 1]
       |
       v
  tui.rs: enter event loop
       |
       v
  ┌─> terminal.draw(|frame| app.render(frame))
  |       |
  |       v
  |   ratatui widgets render Entry data directly
  |
  └─> event::read()
          |
          v
      app.handle_key(key)
          |
          v
      update App state (scroll, tab, quit)
          |
          └──> loop back to draw
```

### Flow 3: Compare Mode (new)

```
user types "define --compare ephemeral transitory"
       |
       v
  cli.rs: Cli::parse()
  compare=Some(["ephemeral", "transitory"])
       |
       v
  main.rs: dispatch to compare path
       |
       v
  compare.rs: compare_words(["ephemeral", "transitory"])
       |
       v
  std::thread::spawn ──> lookup::lookup("ephemeral")  (thread 1)
  std::thread::spawn ──> lookup::lookup("transitory") (thread 2)
       |
       v
  Join both threads, collect Vec<CompareResult>
       |
       v
  if tui:
    tui.rs: render side-by-side panels
  else:
    render.rs: render_compare(results)
```

### Flow 4: Quiz Mode (new)

```
user types "define quiz"
       |
       v
  cli.rs: Cli::parse()
  command=Some(Quiz)
       |
       v
  main.rs: dispatch to quiz::run_quiz()
       |
       v
  quiz_state.rs: load_state()
    └── exists? ──> restore previous state
    └── none? ──>
       |
       v
  history.rs: read_history()
       |
       v
  Generate QuizWord pool from history (lookup definitions via lookup.rs)
       |
       v
  quiz.rs: enter TUI event loop
       |
       v
  ┌─> terminal.draw(|frame| quiz.render(frame))
  |       renders question, options, score, progress
  |
  └─> event::read()
          |
          v
      quiz.handle_input(key)
          |
          v
      Check answer, update score, update scheduling
          |
          v
      quiz_state.rs: save_state()  (persist progress)
          |
          └──> loop back to draw (next question or summary)
```

---

## Main Dispatch Logic (proposed for `main.rs`)

The orchestrator in `main.rs` needs to be restructured to handle the new modes. The key decision tree:

```rust
fn main() {
    let cli = cli::Cli::parse();

    // 1. Subcommands (unchanged: history, cache)
    if let Some(cmd) = &cli.command {
        // ... existing subcommand handling ...
        return;
    }

    // 2. TUI mode
    #[cfg(feature = "tui")]
    if cli.tui {
        if let Some(words) = &cli.compare {
            // TUI compare mode
        } else if let Some(word) = &cli.word {
            // TUI single word
        } else {
            // TUI home / quiz
        }
        return;
    }

    // 3. Compare mode (plain text)
    if let Some(words) = &cli.compare {
        let results = compare::compare_words(words);
        print!("{}", render::render_compare(&results, no_color));
        return;
    }

    // 4. Normal single-word lookup (existing, refactored)
    // ... existing pipeline ...
}
```

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Putting TUI rendering logic in `render.rs`
**What:** Adding ratatui widget rendering code to the existing `render.rs` module.
**Why bad:** `render.rs` returns `String`. ratatui rendering works by mutating a `Frame` buffer directly. These are fundamentally different output mechanisms. Mixing them creates a module with two unrelated responsibilities.
**Instead:** Keep `render.rs` for plain-text output. Create `src/tui.rs` for all ratatui rendering. If shared styling constants (colors, formatting) are needed, put them in a `src/style.rs` module that both can import.

### Anti-Pattern 2: Making `Entry` carry rendering metadata
**What:** Adding fields like `highlight_color` or `display_order` to the `Entry` struct for TUI purposes.
**Why bad:** `Entry` is a serde deserialization type that maps directly to the Free Dictionary API JSON. Adding fields breaks the clean serde mapping and makes deserialization more complex.
**Instead:** Keep `Entry` as a pure data type. The TUI module creates its own view-model or display state that references `Entry` data.

### Anti-Pattern 3: Blocking the TUI event loop with network requests
**What:** Calling `lookup::lookup()` directly inside the event loop's draw or event handler.
**Why bad:** `lookup::lookup()` involves blocking network I/O (via `ureq`). Blocking the event loop freezes the UI with no loading feedback until the request completes or times out.
**Instead:** Show a loading state in the TUI, perform the fetch in a background `std::thread`, and update the app state when the thread completes. For single-word lookups, fetch before entering the event loop. For compare mode with multiple words, fetch all words in parallel threads, then enter the event loop with results.

### Anti-Pattern 4: Global mutable state for TUI app
**What:** Using `static mut` or `lazy_static!` with `Mutex` for the app state.
**Why bad:** The TUI event loop is single-threaded (one thread handles events and drawing). Global mutable state adds unnecessary synchronization complexity.
**Instead:** Pass `&mut App` through the event loop. The ratatui examples pattern (Context7, HIGH confidence) uses a local `App` struct passed by mutable reference to both the draw function and event handlers.

### Anti-Pattern 5: Embedding the offline dictionary in the binary
**What:** Using `include_bytes!()` to embed a dictionary file.
**Why bad:** Adds megabytes to binary size, cannot be updated without a new release, duplicates the data across all build targets.
**Instead:** Download on first use to `~/.define/offline/`. Provide a `define offline download` subcommand for explicit management.

---

## Feature Flags

Based on the existing `audio` feature pattern in the codebase:

```toml
[features]
default = ["audio"]
audio = ["dep:rodio"]
tui = ["dep:ratatui", "dep:crossterm"]
```

**Interaction between flags:**
- `--tui` without `tui` feature: Show error message "TUI mode requires the 'tui' feature. Install with: cargo install define_cli --features tui"
- `--compare` without `tui`: Works fine in plain-text mode (sequential column output)
- `--quiz` requires `tui`: Quiz is TUI-only. Without the `tui` feature, `--quiz` shows the error above.
- `audio` + `tui` both enabled: TUI's audio button works via `audio::play_pronunciation()` (already feature-gated)

**CLI flags to add to `Cli` struct:**
```rust
#[cfg(feature = "tui")]
#[arg(long, help = "Show interactive TUI output")]
pub tui: bool,

#[arg(long, value_name = "WORD...", num_args = 1.., help = "Compare multiple words")]
pub compare: Option<Vec<String>>,
```

---

## Scalability Considerations

| Concern | At 100 users (local CLI) | At 10K users | At 1M users |
|---------|--------------------------|--------------|-------------|
| Offline dictionary download | No issue -- one-time download per user | No issue -- no server component | N/A (local tool) |
| TUI performance | ratatui renders at 60fps easily for definition views | N/A | N/A |
| Compare concurrency | 2-10 threads, fine for any count | N/A | N/A |
| Quiz state file | Single JSON file, trivially fast | N/A | N/A |
| Cache size | Flat files, ~10KB per word, no practical limit for typical use | N/A | N/A |

Since this is a single-user local CLI tool, scalability concerns are minimal. The primary concern is binary size and compile time, which are managed by feature flags.

---

## Build Order (Dependencies Between Components)

The components must be built in an order that respects their dependencies:

```
Phase 1: Offline Dictionary + LookupResolver
  └── offline.rs (independent, can build first)
  └── lookup.rs (depends on offline.rs)
  └── Refactor main.rs to use lookup.rs instead of inline cascade

Phase 2: Compare Mode (v5 - concurrency)
  └── compare.rs (depends on lookup.rs)
  └── Extend cli.rs with --compare flag
  └── Extend render.rs with render_compare()
  └── Update main.rs dispatch

Phase 3: TUI Mode (v6 - TUI)
  └── tui.rs (depends on lookup.rs, optionally compare.rs)
  └── Feature gate everything behind "tui"
  └── Extend cli.rs with --tui flag
  └── Update main.rs dispatch

Phase 4: Quiz Mode (v6+ - state management)
  └── quiz_state.rs (depends on api.rs for Entry type)
  └── quiz.rs (depends on quiz_state.rs, history.rs, lookup.rs, tui.rs)
  └── Extend cli.rs with quiz subcommand
  └── Update main.rs dispatch
```

**Why this order:**
1. `lookup.rs` is the shared data layer that compare, TUI, and quiz all depend on. Build it first.
2. Compare mode introduces concurrency (the v5 learning goal) without TUI. It can be fully functional in plain-text mode.
3. TUI mode can then consume the lookup and compare infrastructure already built.
4. Quiz is last because it depends on all three: TUI for rendering, history for word source, lookup for definitions.

**What can be parallelized within a phase:**
- `offline.rs` and `lookup.rs` can be developed together
- `compare.rs` plain-text rendering and `compare.rs` data fetching can be done together
- `tui.rs` single-word view and `tui.rs` compare panels can be done together

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| TUI architecture (ratatui App pattern) | HIGH | Based on Context7 docs, ratatui official examples, standard pattern |
| ratatui event loop and terminal lifecycle | HIGH | `ratatui::run` API confirmed in Context7 docs |
| Layout and widget composition | HIGH | Layout::horizontal/vertical, Tabs, Scrollbar all confirmed in Context7 |
| Offline dictionary format (flat JSON + gzip) | MEDIUM | Reasonable approach based on project constraints, but no ecosystem survey of alternatives |
| Concurrency approach (std::thread) | HIGH | Standard Rust pattern for blocking I/O, matches project's v5 learning goal |
| Quiz state management | MEDIUM | Proposed approach is sound but quiz UX patterns are less standardized |
| Feature flag interactions | HIGH | Follows established `audio` feature pattern in codebase |
| Main dispatch restructuring | HIGH | Straightforward extension of existing dispatch logic |

## Gaps to Address

1. **Offline dictionary source:** Where exactly to obtain the 1K-5K common word dataset. Options include pre-fetching from the Free Dictionary API (possible ToS concern with bulk fetching), or using an open dictionary dataset like WordNet or EASY (both different schemas that would need conversion). This needs phase-specific research.

2. **Quiz question generation quality:** How to select good distractors for multiple choice. Random selection from history may produce poor choices (e.g., selecting a word with the same definition). Needs more thought during quiz implementation.

3. **ratatui version stability:** The project should pin to a specific ratatui version. At time of research, v0.29 and v0.30 are current. The Context7 docs show v0.30 APIs. Verify compatibility at implementation time.

4. **Terminal compatibility for TUI:** The `ratatui::run` function uses crossterm backend by default, which supports Linux, macOS, and Windows. But specific terminal emulators (especially older Windows cmd.exe) may have limited support. The project already targets all three platforms, so this should be tested.

## Sources

- ratatui Context7 docs: `/ratatui/ratatui` -- App pattern, event loop, `ratatui::run`, Layout, Tabs, Scrollbar, stateful widgets (HIGH confidence)
- crossterm Context7 docs: `/crossterm-rs/crossterm` -- event handling, `poll`, `read`, `try_read` (HIGH confidence)
- rayon Context7 docs: `/rayon-rs/rayon` -- parallel iterators, `join`, `par_iter` (HIGH confidence)
- Existing codebase: `src/main.rs`, `src/cli.rs`, `src/api.rs`, `src/render.rs`, `src/cache.rs`, `src/history.rs` -- current architecture, patterns, conventions
- CLAUDE.md: Project constraints, version roadmap, conventions
- PROJECT.md: Active requirements, constraints, key decisions

---
*Architecture research: 2026-06-01*
