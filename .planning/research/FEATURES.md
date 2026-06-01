# Feature Landscape

**Domain:** CLI dictionary tool with TUI, vocabulary quiz, word comparison, and offline support
**Researched:** 2026-06-01

## Table Stakes

Features users expect in a dictionary CLI. Missing any of these makes the tool feel incomplete compared to `dict`, `wn`, or even a quick browser search.

### Already Shipped (v1-v4)

| Feature | Status | Why Expected |
|---------|--------|--------------|
| Basic word lookup | Shipped | Core purpose of the tool |
| Phonetic text | Shipped | Users need pronunciation guides |
| Part of speech, definitions, examples | Shipped | Minimum definition data |
| Synonyms, antonyms | Shipped | Standard in every dictionary |
| Colored output | Shipped | Terminal-native polish |
| `--short`, `--json`, `--no-color` | Shipped | Pipe/script composability |
| Local cache | Shipped | Repeat lookups must be fast |
| History tracking | Shipped | Users expect to see past lookups |
| Audio pronunciation | Shipped | Modern expectation for dictionaries |
| Exit codes + stderr errors | Shipped | Unix convention |

### Table Stakes Remaining

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Scrollable TUI for long definitions** | Words like "run" have 30+ meanings; plain text truncates or floods stdout. A TUI with scroll lets users explore without losing context. | Medium | ratatui Paragraph + Scrollbar. Must be opt-in (`--tui` or `define --tui word`) to preserve pipe safety. |
| **Offline fallback dictionary** | The Free Dictionary API is free but single-server; users report downtime. Cache-only works for previously looked-up words, but new words fail offline. A small embedded/downloaded wordlist fills this gap. | High | Needs data source (see Offline Dictionary Data Sources below), on-demand download, and fallback logic in the fetch pipeline. |
| **Concurrent word fetching (compare)** | `define --compare fast quick swift` fetching 3 words sequentially feels slow. Parallel HTTP is table stakes for multi-word operations. | Medium | `std::thread` or `rayon` for parallel fetches. Cache hits are instant, so this mainly helps cold lookups. |
| **Session score summary (quiz)** | Any quiz mode without scoring/summary feels unfinished. Users need feedback on performance. | Low | Simple counters (correct, wrong, streak, percentage). Trivial to implement. |
| **Keyboard interrupt handling** | Ctrl+C in quiz/TUI must restore terminal state. Without this, users get a broken terminal (raw mode not restored). | Medium | Must use `std::panic::catch_unwind` or drop guards in crossterm/ratatui setup. |
| **Terminal resize handling** | TUI must re-render on window resize. Ratatui fires `Event::Resize` -- just need to handle it. | Low | Single handler in the event loop. |
| **Word-of-the-day or recent words suggestion** | When launching quiz or TUI, showing recent lookup words or common words gives users a starting point instead of a blank screen. | Low | Read from history.txt, no external deps needed. |

## Differentiators

Features that are not expected but make this tool stand out from `dict -d`, `wn`, online dictionaries, and the handful of other Rust CLI dictionary tools (cargo-thesaurust, rdict, atlas.dict).

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Interactive side-by-side compare panel (TUI)** | Most dictionary tools show compare output as static columns in stdout. An interactive TUI with scrollable panels per word lets users navigate long definitions independently, scroll synonyms, and see phonetics alongside. | High | ratatui Layout (horizontal split), Scrollbar per panel, independent scroll state per word. This is the killer feature for writers choosing between synonyms. |
| **Multiple quiz types** | Single fill-in-the-blank is boring. Multiple-choice (4 options drawn from synonyms/antonyms/random words), matching (word to definition), and reverse quiz (definition to word) add variety and learning depth. | Medium | Draw wrong answers from cache/history for plausibility. Multiple-choice is easiest; matching requires more state. |
| **Spaced repetition for quiz** | Anki proved spaced repetition works. Tracking which words users get wrong and surfacing them more frequently turns a toy quiz into a useful study tool. | High | Needs persistent stats storage (JSON file in `~/.define/quiz/`). Simple algorithm: ease factor + interval (SM-2 lite). Can start very simple and refine. |
| **Audio in quiz mode** | Hearing the word after revealing the answer reinforces learning. The `rodio` integration already exists from v4. | Low | Just call existing audio module on reveal. Already have `--pronounce` infrastructure. |
| **TUI browse/exploration mode** | Instead of command-per-word, a TUI where users type and see results live (like fzf for definitions). Type a word, see preview; arrow keys to navigate suggestions; Enter to see full definition in a panel. | High | Similar to atlas.dict's browse mode. Requires word list for suggestions (either offline dict or cached words). |
| **Export quiz results** | Users studying for exams may want to export words they got wrong. `define quiz --export wrong.txt`. | Low | Write to file from quiz state. Trivial. |
| **Configurable theme** | Light/dark terminal theme detection + user preference. Users with unusual terminal color schemes expect tools to respect them. | Low | ratatui supports Style overrides. Detect via env var or `--theme` flag. |

## Anti-Features

Features to explicitly NOT build. These scope creep the tool or conflict with its identity as a terminal-native CLI.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Translation/multi-language** | Massive scope expansion. Data sources become bilingual dictionaries (completely different problem). The tool is English-only by design. | Stay English-only. If needed later, it's a separate tool. |
| **Web app or GUI** | The entire value proposition is terminal-native UX. A web app would compete with dictionary.com, not complement terminal workflows. | Keep it CLI-only. The TUI is for interactive terminal use, not a GUI. |
| **API server mode** | Adds deployment complexity, security concerns, and moves away from "single binary, zero config." | Users who need this should use the Free Dictionary API directly. |
| **Cloud sync** | Single-user local tool. Cloud sync adds network dependency, privacy concerns (word history is personal), and auth complexity. | Local files in `~/.define/`. Users can git-clone or rsync if they want sync. |
| **AI/LLM-generated definitions** | Would make the tool dependent on an LLM API, losing the offline-first and free-no-key design. Also reduces definition quality (hallucinated definitions). | Stick to curated dictionary data sources. |
| **Embedded dictionary in binary** | Embedding a full dictionary makes the binary 10-100MB. The current on-demand download approach keeps the binary small. | Download on first use, store in `~/.define/offline/`. Same as atlas.dict's approach but with a smaller data set. |
| **Native spell check / suggestions** | Full spell check is a separate project (hunspell, aspell). A simple "did you mean?" from similar words in the offline dictionary is sufficient. | Offer fuzzy matching from offline word list only. No external spell check library. |

## Feature Dependencies

```
Offline fallback dictionary --> Compare mode (faster cold lookups)
Offline fallback dictionary --> Quiz mode (word list for generating wrong answers)
Offline fallback dictionary --> TUI browse mode (word list for suggestions)
History (v3) --------------> Quiz mode (source of words to quiz)
Audio (v4) -----------------> Quiz mode (pronounce words in quiz)
TUI output mode ------------> Compare TUI panels (side-by-side interactive)
TUI output mode ------------> Quiz mode TUI (interactive quiz interface)
Concurrent fetching --------> Compare mode (parallel word lookups)
Concurrent fetching --------> Offline dictionary download (download + index concurrently)
```

Key insight: The offline dictionary is the critical enabler for most differentiating features. Without a local word list, quiz wrong-answer generation, TUI suggestions, and fast offline compare are all significantly harder or worse.

## MVP Recommendation (per version)

### v5 -- Compare & Multi-word
Prioritize:
1. **Concurrent word fetching** -- required for compare to feel fast
2. **Plain-text columnar compare output** -- core deliverable
3. **`--compare` flag with 2-5 words** -- basic functionality

Defer:
- TUI-enhanced compare (requires ratatui, comes later)

### v6 -- Quiz Mode
Prioritize:
1. **Fill-in-the-blank quiz from history** -- core loop, simplest quiz type
2. **Session scoring + streak counter** -- table stakes for quiz
3. **Multiple-choice quiz type** -- draws wrong answers from cache/history
4. **Keyboard interrupt / terminal restore** -- critical for any interactive mode

Defer:
- Spaced repetition (complex, add in v7+)
- TUI-enhanced quiz (plain terminal quiz is fine for v6)

### v6+ -- TUI Output Mode
Prioritize:
1. **`--tui` flag for scrollable word view** -- ratatui Paragraph + Scrollbar
2. **TUI compare panels** -- side-by-side scrollable
3. **TUI quiz interface** -- richer quiz experience

Defer:
- TUI browse/exploration mode (needs offline dictionary for suggestions)
- Configurable themes

### v7+ -- Offline Dictionary
Prioritize:
1. **On-demand download of compact word list** -- 1K-5K common words
2. **Fallback to offline data when API unreachable** -- transparent to user
3. **`define offline download` command** -- manual trigger

## Competitive Feature Matrix

How this tool compares to alternatives:

| Feature | define_cli (this) | dict (system) | rdict (Rust) | atlas.dict (Go) | cargo-thesaurust (Rust) |
|---------|-------------------|----------------|---------------|------------------|------------------------|
| English definitions | Yes (API) | Yes (dictd) | Yes (Wiktionary) | Yes (FreeDict) | Yes (thesaurus) |
| Colored output | Yes | Limited | No | Yes (lipgloss) | Basic |
| Audio pronunciation | Yes | No | No | No | No |
| Cache | Yes (JSON files) | Server-side | SQLite | Embedded binary | No |
| History | Yes | No | No | No | No |
| Compare mode | Planned (v5) | No | No | No | No |
| Quiz mode | Planned (v6) | No | No | No | No |
| TUI | Planned (v6+) | No | No | Yes | Yes |
| Offline | Planned (v7+) | Yes | Yes (1.5GB) | Yes (embedded) | No |
| Pipe-friendly | Yes | Yes | Yes | Yes | Yes |
| Multi-language | No (by design) | Yes | Yes | Yes | No |

Competitive advantage: No other terminal dictionary tool combines audio pronunciation, history tracking, quiz mode, and compare mode. The closest is atlas.dict (TUI + offline) but it lacks quiz, compare, audio, and history.

## Offline Dictionary Data Sources

Critical for enabling quiz wrong-answer generation, TUI browse suggestions, and true offline support.

### Recommended: Build a Custom Small Dictionary

Build a compact JSON file of 1K-5K common English words by seeding from the Free Dictionary API itself:

**Approach:** `define offline download` fetches the top N words from a curated frequency list (see sources below), looks up each via the Free Dictionary API, and caches them locally in a compact format at `~/.define/offline/dict.jsonl`.

**Pros:** Data format matches existing `Entry` types exactly. Definitions come from the same API the tool already uses, ensuring consistency. No data licensing issues (API responses are free). Users can grow their offline dictionary by looking up more words.

**Cons:** Initial download takes time (API rate limiting unknown). Requires network for initial build.

**Estimated size:** 3K words x ~2KB per entry = ~6MB JSONL. With gzip compression: ~1-2MB.

### Alternative Data Sources

| Source | Size | License | Format | Has Definitions | Suitability |
|--------|------|---------|--------|----------------|-------------|
| **dwyl/english-words** | 466K words, 4MB | Unlicense | Plain text (one word per line) | No -- word list only | Good for autocomplete/suggestions, not definitions. Pair with API fetch. |
| **first20hours/google-10000-english** | 9,894 words, 80KB | Unspecified (Google corpus) | Plain text, frequency-ordered | No -- word list only | Excellent source for "most common words" ranking. Use to seed offline download. |
| **Open English WordNet** | 161K words, ~10MB (JSON) | WordNet license (permissive for NLP) | JSON, LMF XML | Yes -- synsets, glosses, relations | Comprehensive but data format differs from Free Dictionary API. Requires mapping. |
| **kaikki.org Wiktionary dumps** | ~3GB JSONL | CC-BY-SA (Wiktionary) | JSONL per language | Yes -- full Wiktionary entries with etymology, senses, examples | Most comprehensive, but way too large for a "small offline dictionary." |
| **FreeDict** | Varies by language pair | GPL v2+ | DSL (custom) | Yes | Used by atlas.dict, but mainly bilingual, not monolingual English. GPL complicates distribution. |
| **Custom seed from API** | 1-5K words, ~1-6MB | Free (API data) | Same as existing cache | Yes | Best fit: same schema, same source, incremental growth. |

### Recommendation

**Use the Google 10000 English word list as a priority queue for downloading definitions from the Free Dictionary API.** This gives:
1. A frequency-ranked word list (most useful words first) -- source: `first20hours/google-10000-english` (Unlicense-compatible, PUBLIC DOMAIN)
2. Full definitions from the same API the tool already uses -- no schema mismatch
3. Incremental growth: each `define` call that hits the API also enriches the offline store
4. Small initial download: start with top 1,000 words (~2MB compressed)

### Secondary: dwyl/english-words for Autocomplete

Use `dwyl/english-words` (466K words, Unlicense, 4MB) as a fallback word list for autocomplete suggestions in TUI browse mode. No definitions, but users can type any English word and get a suggestion. Definitions then come from the API or offline cache.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Table stakes features | HIGH | Based on competitor analysis of dict, rdict, atlas.dict, cargo-thesaurust and standard CLI expectations |
| Quiz features | MEDIUM | Based on general vocabulary quiz patterns (Anki, flashcard apps) applied to CLI context. No direct CLI dictionary+quiz competitor found. |
| Compare features | HIGH | Columnar multi-word display is straightforward. TUI side-by-side panels are well-documented in ratatui. |
| Offline data sources | HIGH | All sources verified via GitHub API and direct download. Sizes and formats confirmed. |
| TUI browse mode | MEDIUM | Pattern well-established (fzf, atlas.dict) but not standard for dictionary tools specifically. |
| Spaced repetition | MEDIUM | SM-2 algorithm is well-documented, but fitting it into a CLI dictionary is novel. |

## Sources

- ratatui docs (Context7, HIGH confidence): available widgets include Paragraph, Table, Tabs, List, Scrollbar, Block, BarChart, Gauge, Sparkline
- crossterm docs (Context7, HIGH confidence): raw mode, event polling/reading, key events, mouse capture
- Free Dictionary API: https://github.com/meetDeveloper/freeDictionaryAPI (HIGH -- verified response format)
- dwyl/english-words: https://github.com/dwyl/english-words (HIGH -- 466K words, Unlicense, verified download)
- Google 10000 English: https://github.com/first20hours/google-10000-english (HIGH -- 9,894 words, frequency-ranked, verified download)
- Open English WordNet: https://en-word.net/ (HIGH -- 161K words, JSON format available, ~10MB)
- kaikki.org Wiktionary dumps: https://kaikki.org/dictionary/English/ (HIGH -- 3GB JSONL, CC-BY-SA)
- rdict: https://github.com/Lodobo/rdict (HIGH -- Rust offline dict using Wiktionary, 1.5GB JSON)
- atlas.dict: https://github.com/fezcode/atlas.dict (HIGH -- Go offline dict with TUI, FreeDict data, embedded)
- open-dsl-dict/wiktionary-dict: https://github.com/open-dsl-dict/wiktionary-dict (HIGH -- bilingual Wiktionary DSL dicts, CC-BY-SA/GFDL)
