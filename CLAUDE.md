# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`define_cli` is a Rust CLI that looks up English word definitions from the terminal using the Free Dictionary API. It is versioned incrementally (v1 through v6); each version is independently shippable.

## Two-Repo Structure

This repo (`define_cli`) is the **public** code repo. Planning docs live in a **private** `define-docs` repo, mounted as a git submodule at `docs/`.

- **Code-only changes** → commit to this repo only (no submodule update needed)
- **Doc changes** (SPEC, ARCHITECTURE, ROADMAP, DESIGN, PLAN) → commit inside `docs/` (submodule), push, then update the submodule pointer in this repo

To update docs:
```bash
cd docs
git add -A && git commit -m "docs: ..." && git push
cd ..
git add docs && git commit -m "chore: update docs submodule"
```

To clone fresh with docs:
```bash
git clone --recurse-submodules https://github.com/baoviettran/define_cli.git
```

## Build & Run

```bash
cargo build                          # debug (audio enabled by default)
cargo build --release                # release binary → target/release/define_cli
./target/release/define_cli hello    # run
```

**Linux:** Audio playback requires `libasound2-dev` (`sudo apt install libasound2-dev`).
Build without audio: `cargo build --no-default-features`.

## Test

```bash
cargo test                           # all tests
cargo test test_deserialize_ephemeral  # single test
```

Unit tests live in `#[cfg(test)] mod tests` inside `src/main.rs`. Test fixtures (saved API responses) are in `tests/fixtures/`.

## Architecture

v1 is a single-file app: everything in `src/main.rs`.

Three logical layers:
1. **CLI Parser** — `std::env::args()` (manual; `clap` comes in v2)
2. **HTTP Client** — `fetch_definition()` using `ureq`, URL-encodes words via `urlencoding`
3. **Output Renderer** — `render_entries()` returns a `String` with ANSI escape codes

Data flow: args → URL-encode → HTTP GET → deserialize JSON into `Vec<Entry>` → render to string → print to stdout. Errors go to stderr, exit code 1.

Key types: `Entry`, `Phonetic`, `Meaning`, `Definition` — all derive `serde::Deserialize`.

**Optional features:** `audio` (default: enabled) — gates `rodio` dependency and `src/audio.rs`. When disabled, `--pronounce` shows phonetic text instead of playing audio, and `--accent` flag is hidden.

## Version Roadmap

| Version | Focus | Key new dependency |
|---|---|---|
| v1 | Core lookup | ureq, serde |
| v2 | Flags & Polish (`--short`, `--json`, `--no-color`) | clap |
| v3 | Cache & History | dirs, std::fs |
| v4 | Audio Pronunciation | rodio |
| v5 | Compare & Multi-word | std::thread or rayon |
| v6 | Quiz Mode | crossterm |

Each version introduces one major new Rust concept. See `docs/ROADMAP.md` for full details.

## Conventions

- No `unwrap()` or `expect()` on paths handling user input or network data — use `?` and `Result`
- Errors to stderr (`eprintln!`), output to stdout (`print!`)
- Exit code 0 on success, 1 on error
- Follow the ROADMAP versioning — don't skip ahead or mix scope across versions
- Specs (`docs/SPEC.md`, `docs/ARCHITECTURE.md`) are the source of truth for requirements

## Progress Tracking

**Where to look:** `docs/ROADMAP.md` is the source of truth for what's shipped and what's next. Each version has a status line:
- `Status: Shipped (YYYY-MM-DD)` — done, merged, tested
- `Status: In progress` — currently being built
- `Status: Planned` — not started yet

**When starting a new session:**
1. Read `docs/ROADMAP.md` — find the current version (first non-shipped)
2. Read `docs/DESIGN.md` and `docs/PLAN.md` in the same docs/ directory for that version's implementation plan
3. If the version has no DESIGN.md or PLAN.md yet, write them before coding (see `superpowers:writing-plans`)

**When finishing a version:**
1. Update ROADMAP.md status to `Shipped (YYYY-MM-DD)`
2. Add entry to CHANGELOG.md in the public repo
3. Commit docs (submodule), then update submodule pointer
4. Commit CHANGELOG.md to public repo

**CHANGELOG.md** (public repo) records what shipped in each version with a short summary and commit range.
