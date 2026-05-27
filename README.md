# define_cli

A fast, simple CLI tool to look up English word definitions from the terminal.

```
$ define ephemeral

EPHEMERAL
/ɪˈfem(ə)r(ə)l/
──────────────────────────────────────────────────

ADJECTIVE

  1. Lasting for a very short time.
     "fashions are ephemeral"

  Synonyms: transitory, transient, fleeting, short-lived
  Antonyms: permanent, eternal, enduring
```

## Install

```bash
cargo build --release
cp target/release/define_cli /usr/local/bin/define
```

## Usage

```bash
define hello
define serendipity
```

## Built With

- [Rust](https://www.rust-lang.org/)
- [Free Dictionary API](https://dictionaryapi.dev/)

## License

MIT
