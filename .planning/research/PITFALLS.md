# Domain Pitfalls: TUI, Offline Dictionary, Concurrency, Quiz Mode, and Distribution

**Project:** define_cli
**Researched:** 2026-06-01
**Domain:** Rust CLI adding TUI layer, offline dictionary data, concurrent lookups, interactive quiz mode, and distribution channels

---

## Critical Pitfalls

These cause rewrites, broken terminals, data corruption, or release failures.

---

### Pitfall 1: Terminal State Corruption on Panic or Unhandled Error in TUI Mode

**What goes wrong:**
The user runs `define --tui ephemeral`. A panic or unhandled error occurs during rendering or event handling (e.g., division by zero, index out of bounds). The process crashes without restoring the terminal. The user's shell is left in raw mode: keystrokes echo as raw control characters, no newline on Enter, the cursor is invisible. They must blindly type `reset` or `stty sane` to recover.

**Why it happens:**
Ratatui enters "raw mode" and the "alternate screen buffer" when initializing. Raw mode disables the terminal's normal input processing (line buffering, echo, signal handling). The alternate screen buffer replaces the visible terminal content. If the process terminates abnormally, neither is restored. The current codebase uses `std::process::exit(1)` in 9 places in `main.rs`, all of which bypass destructors. If `exit()` is called while the TUI is active, the terminal is stuck in raw mode.

**Consequences:**
- Broken terminal for every user who hits an error in TUI mode
- Trust destruction: a dictionary tool that breaks your terminal is unacceptable
- Particularly bad on macOS where Terminal.app is more fragile than Linux terminals

**Prevention:**
1. **Use `ratatui::run()` as the primary entry point for TUI mode.** As of ratatui 0.30, `ratatui::run()` installs a panic hook that calls `restore()` before panicking, and always restores the terminal when the closure returns (whether normally or via panic). This is the highest-level safety net. (Source: ratatui docs, HIGH confidence)
2. **If using manual `init()`/`restore()`, put the main loop in a separate function** so `restore()` is always reached:
   ```rust
   fn main() -> std::io::Result<()> {
       let mut terminal = ratatui::init();
       let result = run_app(&mut terminal);
       ratatui::restore();
       result
   }
   ```
3. **Eliminate all `std::process::exit(1)` calls** before introducing TUI. Refactor main to propagate errors via `Result` and `std::process::ExitCode`. The 9 `exit(1)` calls in `src/main.rs` are safe today (no TUI state to corrupt) but become a ticking time bomb once TUI is added. This is already flagged in CONCERNS.md.
4. **Ensure crossterm version compatibility** -- ratatui 0.30+ supports multiple crossterm major versions via `crossterm_{version}` feature flags. If ratatui uses crossterm 0.29 but another dependency pulls in crossterm 0.28, they track raw mode separately, and `restore()` from the wrong version may not actually restore the terminal. Pin the crossterm version explicitly: `ratatui = { version = "0.30", features = ["crossterm_0_29"] }` and `crossterm = "0.29"`.

**Detection:**
- Any use of `std::process::exit()` inside a code path that could be reached while TUI is active
- Manual terminal init without a corresponding `restore()` in all exit paths
- Dependency tree showing multiple crossterm major versions (`cargo tree | grep crossterm`)

**Phase relevance:** TUI phase (v6 or later) and concurrency phase (v5, must eliminate `exit()` first)

---

### Pitfall 2: Crossterm Version Diamond Dependency Conflict

**What goes wrong:**
ratatui depends on crossterm. Another dependency (or an older version of ratatui) also depends on crossterm, but a different major version. The two crossterm versions maintain separate event queues and separate raw mode state. Results: events get lost (keystrokes silently dropped), raw mode tracked by the wrong version so `restore()` thinks raw mode is already disabled when it is not. The terminal state is broken even on normal exit.

**Why it happens:**
Crossterm 0.27, 0.28, and 0.29 are semver-incompatible major versions. Ratatui 0.30 supports all three via feature flags (`crossterm_0_27`, `crossterm_0_28`, `crossterm_0_29`). If your `Cargo.toml` says `crossterm = "0.29"` but ratatui resolves to the 0.28 backend, you now have two versions in the dependency tree. This is explicitly documented as a known problem in ratatui's backend documentation.

**Consequences:**
- Intermittent keystroke loss (user presses 'q' to quit, nothing happens)
- Terminal not restored on exit even when `restore()` is called correctly
- Very hard to debug because it only manifests at runtime, not compile time (types may still be compatible through re-exports)

**Prevention:**
1. **Always pin crossterm to match the ratatui backend feature.** In `Cargo.toml`:
   ```toml
   ratatui = { version = "0.30", features = ["crossterm_0_29"] }
   crossterm = "0.29"
   ```
2. **Run `cargo tree | grep crossterm` during CI** to verify only one crossterm major version exists.
3. **Never use bare `ratatui = "0.30"` without specifying the crossterm feature** if you also depend on crossterm directly for event handling (which you will for quiz mode).

**Detection:**
- `cargo tree -i crossterm` shows multiple major versions
- Intermittent "missed keypress" bugs in TUI

**Phase relevance:** TUI phase and quiz phase (both depend on ratatui + crossterm)

---

### Pitfall 3: Concurrent `ureq` Blocking I/O Starves Threads and Produces Interleaved Output

**What goes wrong:**
`define --compare word1 word2 word3` spawns threads to fetch words concurrently. But `ureq` uses blocking I/O -- each thread blocks on the HTTP call. With the default thread pool size, this works for 3 words but scales poorly. More critically, if multiple threads print to stdout simultaneously, output is interleaved character-by-character. The user sees garbled text instead of clean columns.

**Why it happens:**
The current architecture is entirely synchronous: `args -> URL-encode -> HTTP GET -> deserialize -> render -> print to stdout`. Adding concurrency via `std::thread::spawn` works for the fetch step, but the current code prints results as they arrive. Two threads writing to `stdout` concurrently produce interleaved output because `print!` and `println!` are not atomic across threads. Additionally, `ureq` is a blocking HTTP client -- each concurrent request consumes a full OS thread that does nothing but wait for the HTTP response.

**Consequences:**
- Garbled output for compare mode (unusable feature)
- With many words (e.g., `--compare` with 20 words), thread exhaustion or excessive resource usage
- If the HTTP timeout is not set (currently it is not -- flagged in CONCERNS.md), one hung request blocks its thread indefinitely

**Prevention:**
1. **Collect all results first, render all at once.** Spawn threads to fetch data, collect `Vec<Result<Entry, String>>` via `JoinHandle`, then render sequentially in input order. Never print from worker threads.
2. **Use `std::thread::scope` to avoid `JoinHandle` lifetime issues** when borrowing data:
   ```rust
   let results: Vec<Result<Entry, String>> = std::thread::scope(|s| {
       let handles: Vec<_> = words.iter().map(|w| {
           s.spawn(|| api::fetch_definition(w))
       }).collect();
       handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("Thread panicked".into()))).collect()
   });
   ```
3. **Set HTTP timeouts** before adding concurrency. Use `ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10)))` (this is ureq 3.x API; the project currently uses ureq 2.5). This is critical: without timeouts, a hung API server causes a thread to block forever.
4. **Limit concurrency.** For a dictionary CLI, fetching 10 words in parallel is plenty. Use `std::thread` directly rather than rayon -- the overhead of rayon's work-stealing scheduler is unnecessary for a small fixed number of HTTP requests.

**Detection:**
- Running `--compare` with 5+ words and seeing garbled output
- Hanging indefinitely on compare with an unreachable network
- CONCERNS.md already flags stdout interleaving as a v5 risk

**Phase relevance:** Concurrency phase (v5)

---

### Pitfall 4: Offline Dictionary Data Source Licencing and Format Mismatch

**What goes wrong:**
An offline dictionary (1K-5K words) is downloaded on demand to `~/.define/`. The data comes from a source that requires attribution (e.g., WordNet, Wiktionary data dumps). The binary is published to crates.io or distributed via Homebrew without proper licence compliance. The open-source licence of the data source requires including a copy of the licence or an attribution notice. A DMCA takedown or licence violation forces a release retraction.

**Why it happens:**
Dictionary data is almost always licensed. Common sources:
- **WordNet 3.0**: Princeton licence, requires attribution. Free but not public domain.
- **Wiktionary dumps**: CC BY-SA. Requires attribution and share-alike.
- **GNU Collaborative International Dictionary of English (GCIDE):** Public domain / GPL. Safe but the data is older and less maintained.
- **ECDICT (GitHub):** An aggregated dictionary project of unclear licence provenance. Often used in projects but legally risky.

The project description says "small (1K-5K common words), downloaded on demand." This implies building or downloading a custom dataset. If the dataset is scraped from the Free Dictionary API itself, that may violate the API's terms of service.

**Consequences:**
- Licence violation leading to takedown or legal liability
- Forced rewrite of offline data source mid-release
- Distribution channels (crates.io, Homebrew) may reject the package if licence issues are flagged

**Prevention:**
1. **Choose a data source with an explicitly permissive licence.** GCIDE (public domain) is the safest option for English definitions. It is older but covers common words well.
2. **Document the data source licence in the README and Cargo.toml (`license-file` or `license`).** Include `LICENSE-THIRD-PARTY` if using WordNet.
3. **Do not scrape the Free Dictionary API to build the offline dictionary.** The API's terms of service likely prohibit bulk scraping. Use a proper dictionary data source instead.
4. **Consider shipping no data and relying solely on the cache for offline use.** The existing cache system already works offline for previously-looked-up words. The offline dictionary is a nice-to-have, not a must-have. Defer it if licence clarity cannot be achieved.

**Detection:**
- No `LICENSE` file for offline data alongside the project licence
- Data source has no explicit licence statement
- Scraping API responses in bulk (rate of requests much higher than normal usage)

**Phase relevance:** Offline dictionary phase

---

### Pitfall 5: `std::process::exit()` Across Threads is Undefined Behavior for Cleanup

**What goes wrong:**
During `--compare` (v5, concurrent threads), an error occurs in a worker thread. The error handler calls `std::process::exit(1)`. This immediately terminates the process without running destructors on other threads. Any thread holding a file lock (for cache writes), a mutex, or an open file descriptor will not clean up properly. The result: corrupted cache files, partial history entries, leaked resources.

**Why it happens:**
`std::process::exit()` calls `libc::exit()` which terminates the process immediately. Rust's `Drop` trait implementations are not run for threads other than the calling thread. The current codebase has 9 calls to `exit(1)` in `main.rs`. If error handling is copied into threaded code without refactoring, this becomes a data integrity bug.

**Consequences:**
- Corrupted cache files (partial JSON from interrupted writes)
- Lost history entries (partial lines from interrupted `writeln!`)
- These are the same bugs flagged in CONCERNS.md under "Concurrent access to history and cache (no locking)" and "No cache file atomicity"

**Prevention:**
1. **Refactor all `exit(1)` calls to return `Result` before adding concurrency.** This is a prerequisite for v5. The main function should return `Result<(), Box<dyn Error>>` or a custom error type, and errors should propagate via `?`.
2. **Use `std::thread::scope` to ensure all threads complete before the main thread proceeds.** Scoped threads guarantee all spawned threads finish before the scope exits.
3. **Collect errors from worker threads, then handle them in the main thread.** Worker threads return `Result`, never call `exit()`. The main thread decides whether to exit and under what conditions.
4. **Make cache writes atomic** (write to `.tmp` then `rename()`) -- this is already flagged in CONCERNS.md and should be done before or during v5.

**Detection:**
- Any `std::process::exit()` call reachable from a spawned thread
- Grep: `exit(1)` in any file that could be used in a threaded context

**Phase relevance:** Concurrency phase (v5), must be addressed before threading is introduced

---

## Moderate Pitfalls

These cause significant bugs or poor UX but are recoverable without rewrites.

---

### Pitfall 6: TUI and Plain Output Code Paths Diverge, Leading to Feature Mismatches

**What goes wrong:**
`--tui` mode renders definitions in a scrollable interactive view. Plain output mode continues to render with the existing `render_entries()` function. Over time, the TUI gets new features (better formatting, clickable synonyms, inline pronunciation controls) that plain mode does not. Or worse, a bug is fixed in one renderer but not the other. Users piping to scripts get different output than TUI users, and the discrepancy is confusing.

**Why it happens:**
The project has a single `render.rs` that outputs ANSI strings. Adding TUI means a second rendering path (ratatui widgets). These are fundamentally different rendering models: one returns a `String` to print, the other builds widget trees drawn to a frame buffer. The temptation is to duplicate data formatting logic in both paths.

**Consequences:**
- Fix a display bug in TUI, forget to fix it in plain mode (or vice versa)
- TUI shows more information than plain mode, or different information
- Maintenance burden doubles for every formatting change

**Prevention:**
1. **Separate data formatting from rendering.** Create a "view model" layer that produces structured data (not strings), and have both renderers consume it:
   ```
   Entry -> View Model (WordView { phonetic, definitions, examples... }) -> TUI widgets
                                                                  -> ANSI string
   ```
2. **Write tests against the view model**, not the rendered output. Both renderers are tested by rendering the same view model and comparing.
3. **Keep plain output as the primary path.** TUI is opt-in (`--tui`). Plain output must never feel neglected. Start every feature in plain mode, then add TUI rendering.

**Detection:**
- Duplicated definition-truncation logic (the `take(3)` / `take(5)` from `render.rs`) appearing in TUI widgets
- Bug fixes applied to one renderer but not the other

**Phase relevance:** TUI phase and all subsequent phases

---

### Pitfall 7: Blocking Audio Playback in TUI Blocks the Event Loop

**What goes wrong:**
In quiz mode or TUI mode, the user presses a key to hear pronunciation. The `play_pronunciation()` function calls `rodio::Player::sleep_until_end()`, which blocks the current thread until audio finishes playing. Since the event loop is on the same thread, the TUI freezes: no key events are processed, no screen redraws happen, no resize handling. The UI appears to hang for 2-3 seconds while the word is spoken.

**Why it happens:**
`rodio::Player::sleep_until_end()` is a blocking call on the current thread. The current architecture runs everything on the main thread with blocking I/O. In TUI mode, the main thread must continuously handle events and redraw. Blocking the event loop kills the TUI.

**Consequences:**
- TUI appears frozen during audio playback
- Resize events during playback are lost (UI not updated)
- User thinks the tool crashed and force-kills it

**Prevention:**
1. **Spawn audio playback on a separate thread** when in TUI mode:
   ```rust
   std::thread::spawn(move || {
       audio::play_pronunciation(url).unwrap_or_else(|e| eprintln!("{}", e));
   });
   ```
2. **Use the event loop's idle time** to check if audio is still playing (optional: show a small speaker icon while playing).
3. **In plain mode, blocking is fine** -- the user expects the command to finish. Only the TUI/event-loop path needs the spawn.

**Detection:**
- TUI becomes unresponsive during `--pronounce`
- `sleep_until_end()` called from the same thread as `terminal.draw()`

**Phase relevance:** TUI phase (when audio and TUI coexist)

---

### Pitfall 8: Quiz Mode State Not Persisted Across Sessions

**What goes wrong:**
The user runs `define quiz` twice. The first session has a "hard" list of words they got wrong. The second session starts fresh and has no memory of the first session's performance. The user expected spaced repetition to remember their weak words across sessions, but the state is only in memory.

**Why it happens:**
The ROADMAP for v6 describes quiz mode as using history data but does not mention persisting quiz state (scores, streaks, wrong-word tracking). The current data model has `history.txt` (word + timestamp) but no concept of quiz performance. If quiz state is only in a Rust struct, it is lost when the process exits.

**Consequences:**
- Spaced repetition (a key differentiator) is impossible without persistence
- `--hard` flag in the ROADMAP cannot work without tracking wrong answers
- User must start from scratch every session, reducing learning effectiveness

**Prevention:**
1. **Add a quiz state file** (e.g., `~/.define/quiz_state.json`) that persists per-word performance:
   ```json
   { "ephemeral": { "correct": 3, "wrong": 1, "last_quizzed": 1700000000 } }
   ```
2. **Write this as part of the initial quiz feature design**, not as an afterthought. The ROADMAP should explicitly include "persist quiz performance."
3. **Use atomic writes** (write to `.tmp`, then rename) for the quiz state file, applying the lesson from cache file atomicity (CONCERNS.md).

**Detection:**
- ROADMAP v6 spec does not mention quiz state persistence
- No file format defined for quiz data

**Phase relevance:** Quiz phase (v6), should be designed in from the start

---

### Pitfall 9: Crates.io Name Collision and Binary Name `define` Conflicts

**What goes wrong:**
The crate is published to crates.io. The name `define_cli` is used as the crate name, but the binary is named `define`. A user installs via `cargo install define_cli` and gets a `define` binary. On some systems, `define` may conflict with existing tools or commands. Additionally, the crate name `define-cli` may already be taken on crates.io, requiring a rename at publish time.

**Why it happens:**
crates.io crate names are globally unique. The project uses `define_cli` in `Cargo.toml`. The binary name defaults to the package name unless overridden with `[[bin]] name = "define"`. Short binary names like `define` are desirable but may conflict with shell builtins, awk functions, or other tools.

**Consequences:**
- `cargo install define_cli` fails if the name is taken
- Binary name `define` conflicts with an existing command on the user's system
- Forced to use an awkward name like `define-cli` or `definetool` as the binary name

**Prevention:**
1. **Check crates.io early.** Search crates.io for `define-cli`, `define_cli`, `define_tool` before committing to a name.
2. **Reserve the name** by publishing a bare-bones 0.1.0 version early (even if the tool is not ready for use). Crates.io allows name reservation via publishing.
3. **Test the binary name for conflicts.** Run `which define` and `command -v define` on Linux, macOS, and check if it shadows anything important.
4. **Consider using `def` as the binary name** (shorter, unlikely to conflict, easy to type) with `define-cli` as the crate name. This is a design decision to make early.

**Detection:**
- `cargo search define-cli` shows an existing crate
- `which define` returns a system command

**Phase relevance:** Distribution phase (pre-publish)

---

### Pitfall 10: Homebrew Tap Maintenance Burden and Cross-Compilation Failures

**What goes wrong:**
A Homebrew tap is created for distributing pre-built binaries. The CI pipeline builds release binaries for `x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, and `x86_64-pc-windows-msvc`. The `rodio` dependency fails to cross-compile for some targets because it depends on ALSA on Linux (which requires `libasound2-dev` system library). The macOS universal binary fails because `rodio` links against CoreAudio differently on Intel vs Apple Silicon. Windows builds fail due to a different audio backend.

**Why it happens:**
Cross-compiling Rust projects with C dependencies is notoriously painful. `rodio` depends on `cpal` which depends on platform-specific audio libraries: ALSA (Linux), CoreAudio (macOS), WASAPI (Windows). Cross-compiling from Linux to macOS or Windows requires the right sysroot and libraries. The `audio` feature flag gates this dependency, but if the default build includes audio, all release binaries need it.

**Consequences:**
- CI builds fail for some targets
- Binary distribution is incomplete (missing macOS ARM64 or Windows binaries)
- Users on some platforms must build from source

**Prevention:**
1. **Use GitHub Actions with actual VM runners for each target** (not cross-compilation). Build on `ubuntu-latest`, `macos-13` (Intel), `macos-14` (Apple Silicon), and `windows-latest`. This is slower but reliable.
2. **Ensure `audio` feature flag actually excludes all platform audio dependencies.** Verify with `cargo build --no-default-features` that `rodio` and its entire dependency tree are excluded. Check with `cargo tree --no-default-features`.
3. **Provide two binary variants per platform:** with-audio and without-audio. Name them `define` and `define-noaudio` in the release. This lets users without audio libraries install the tool.
4. **Consider GitHub Releases as the primary distribution channel** before investing in Homebrew. Homebrew taps add maintenance overhead for formula updates, SHA256 checksums, and bottle builds. GitHub Releases with `cargo-dist` or `cross` is simpler to start.

**Detection:**
- `cargo build --target x86_64-apple-darwin` fails on Linux (cross-compilation missing sysroot)
- Binary size doubles due to audio dependencies
- Homebrew formula cannot build bottles for all platforms

**Phase relevance:** Distribution phase

---

## Minor Pitfalls

These cause inconvenience or suboptimal behavior but are easily fixed.

---

### Pitfall 11: TUI Flicker from Full-Screen Redraws

**What goes wrong:**
The TUI flickers noticeably when redrawing, especially on slower terminals or over SSH. Every frame clears and redraws the entire screen. On each keystroke, the user sees a brief flash.

**Why it happens:**
Ratatui uses immediate-mode rendering: every frame re-draws the entire UI. However, ratatui's `Terminal::draw()` only sends differential updates (only changed cells are written to the terminal). Flicker happens when the rendering code recomputes layout or widget state unnecessarily, causing every cell to appear "changed" to the diff algorithm.

**Prevention:**
1. **Ensure widget state is stable between frames.** Do not allocate new widgets on every draw call; reuse widget instances. Ratatui's layout cache (enabled by default) helps with this.
2. **Use `Viewport::Inline` instead of fullscreen alternate screen** for the `--tui` flag on a single word lookup. Inline mode renders within the existing terminal output and avoids the alternate screen entirely.
3. **Disable layout cache if it causes issues** (`default-features = false, features = ["crossterm"]` -- `layout-cache` is on by default but can be toggled).

**Phase relevance:** TUI phase

---

### Pitfall 12: Cache and Offline Dictionary Schema Diverge

**What goes wrong:**
The cache stores raw API JSON (full Free Dictionary API format). The offline dictionary uses a different, simplified format (e.g., just word + definition + phonetic). Two code paths exist for looking up a word: one reads the cache, one reads the offline dictionary. They return different types or different data shapes. Code that consumes lookup results must handle both formats.

**Why it happens:**
The cache is a pass-through of the API response. The offline dictionary is a curated subset. Without abstraction, every consumer of lookup results checks "was this from cache or from offline?" and handles each case differently.

**Prevention:**
1. **Normalize both sources into the existing `Entry` struct.** The offline dictionary should produce the same `Vec<Entry>` that the API produces (even if many fields are empty/none). The cache already produces `Vec<Entry>`. A unified type means consumers do not care about the source.
2. **If the offline dictionary cannot produce the full `Entry` struct, define an intermediate type** that both sources map to, and renderers consume.

**Phase relevance:** Offline dictionary phase

---

### Pitfall 13: History File Cannot Power Quiz Mode Effectively Without Enrichment

**What goes wrong:**
The quiz mode relies on history.txt for its word list. But history only records `{timestamp}\t{word}`. It has no definitions, no context about what the user was studying, and no information about word difficulty. The quiz can only show the word and ask "what did this mean?" but cannot show the actual definition from the history data (it was never stored).

**Why it happens:**
History was designed as a simple audit log, not a study tool. The ROADMAP says quiz mode "turns your lookup history into a vocabulary quiz" but the history format is too sparse.

**Prevention:**
1. **Enrich history entries** to include at minimum the word's first definition or a key identifier. Or accept that quiz mode always re-fetches from cache/API for the definitions (which works for cached words but fails offline for uncached words).
2. **Alternatively, keep history simple and have quiz mode always look up the word** from cache or offline dictionary when generating a quiz card. This works as long as the word is in cache or the offline dictionary. The quiz should gracefully skip words that are not available locally.

**Phase relevance:** Quiz phase (v6)

---

## Phase-Specific Warnings

| Phase | Topic | Likely Pitfall | Mitigation |
|-------|-------|---------------|------------|
| v5 | Concurrent lookups | Output interleaving from threads (Pitfall 3) | Collect all results, render sequentially |
| v5 | Concurrent lookups | `exit()` across threads corrupts state (Pitfall 5) | Refactor to `Result` propagation before adding threads |
| v5 | Concurrent lookups | No HTTP timeout causes thread hang | Set `timeout_global` on ureq Agent |
| v5 | Concurrent lookups | Cache write race condition | Write to `.tmp` then `rename()`; collect results before caching |
| v6 | Quiz mode | Terminal stuck in raw mode on panic (Pitfall 1) | Use `ratatui::run()` |
| v6 | Quiz mode | Crossterm version conflict (Pitfall 2) | Pin `crossterm = "0.29"` matching ratatui feature |
| v6 | Quiz mode | Quiz state not persisted (Pitfall 8) | Design `quiz_state.json` from day one |
| v6 | Quiz mode | Blocking audio in event loop (Pitfall 7) | Spawn audio on separate thread |
| v6 | Quiz mode | History too sparse for quiz cards (Pitfall 13) | Re-fetch from cache/offline dict for definitions |
| TUI | TUI output mode | TUI/plain renderer divergence (Pitfall 6) | Shared view model layer |
| TUI | TUI output mode | Flicker from full redraws (Pitfall 11) | Use inline viewport; stable widget state |
| Offline | Offline dictionary | Licence violation on dictionary data (Pitfall 4) | Use GCIDE or public-domain source |
| Offline | Offline dictionary | Schema mismatch with cache format (Pitfall 12) | Normalize to `Vec<Entry>` |
| Distribution | crates.io/Homebrew | Name collision (Pitfall 9) | Reserve name early; pick non-conflicting binary name |
| Distribution | CI/CD builds | Cross-compilation failures with rodio (Pitfall 10) | Build on native runners; provide audio/no-audio variants |

---

## Prerequisite Ordering

Some pitfalls must be addressed before the phase they affect:

1. **Before v5 (concurrency):** Eliminate all `std::process::exit(1)` calls. Refactor main to return `Result`. Add HTTP timeouts to ureq. Make cache writes atomic.
2. **Before TUI/quiz (v6):** Pin crossterm version. Verify only one major version in dependency tree. Ensure `ratatui::run()` is used as the TUI entry point.
3. **Before offline dictionary:** Audit dictionary data source licence. Design the data format to normalize to `Vec<Entry>`.
4. **Before distribution:** Reserve crates.io name. Test binary name for conflicts. Set up CI with native runners (not cross-compilation) per target platform.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| TUI terminal state corruption | HIGH | Documented in ratatui 0.30 official docs; `ratatui::run()` installs panic hook; current codebase has 9 `exit(1)` calls |
| Crossterm version conflict | HIGH | Explicitly documented in ratatui backend docs as a known issue with specific mitigation |
| Concurrent ureq + output interleaving | HIGH | ureq docs confirm blocking I/O model; CONCERNS.md already flags stdout interleaving as v5 risk; `exit()` across threads is well-documented Rust behavior |
| Offline dictionary licencing | MEDIUM | General knowledge of dictionary data sources and their licences; specific licence terms should be verified with a lawyer or careful reading of each source's licence text |
| Quiz state persistence gap | HIGH | Roadmap spec review shows no mention of persisting quiz state; history format confirmed by reading `src/history.rs` |
| Crates.io naming | LOW | Cannot check crates.io without WebSearch; name availability is time-sensitive |
| Cross-compilation with rodio | MEDIUM | Well-known issue with cpal/rodio on different platforms; `audio` feature flag already exists as mitigation |

---

## Sources

- ratatui 0.30 official documentation (docs.rs/ratatui/0.30.0) -- HIGH: `ratatui::run()`, `init()/restore()`, crossterm version compatibility
- ratatui backend documentation (ratatui.rs/concepts/backends/) -- HIGH: crossterm version conflict details
- ureq 3.3.0 documentation (docs.rs/ureq/latest/ureq/) -- HIGH: blocking I/O model, Agent configuration, timeouts
- Project codebase analysis (src/main.rs, src/cache.rs, src/history.rs, src/api.rs, src/audio.rs, Cargo.toml) -- HIGH: current architecture, `exit(1)` locations, cache/history implementation
- CONCERNS.md -- HIGH: pre-identified risks for v5 and v6
- docs/ROADMAP.md -- HIGH: version plans, dependency timeline, feature specifications
- LOW confidence: crates.io name availability, specific dictionary data source licence terms (require direct verification)
