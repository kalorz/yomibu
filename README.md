# Yomibu

Yomibu creates Japanese readings from vocabulary you have studied. It has a Rust
CLI and library. Readings are experimental; generation does not prove naturalness or
correct meaning in context.

## First story

Build with `cargo build --locked`. On macOS, run `yomibu auth` once to save keys
in Keychain. Other platforms use environment variables:

```sh
export YOMIBU_WANIKANI_API_KEY=...
export YOMIBU_OPENAI_API_KEY=...
target/debug/yomibu story
```

Usable vocabulary is required.
Run `yomibu` for all commands or `yomibu help story` for setup and permissions.
See [story settings](docs/STORY_GENERATION.md#configuration-and-optional-work).

## Current commands

Use `cargo run --locked -- --help` for flags.

| Command | Purpose |
| --- | --- |
| `auth` | Set up credentials in macOS Keychain |
| `help` | Show commands and options |
| `sync` | Refresh the cache; needs `YOMIBU_WANIKANI_API_KEY` |
| `story` | Generate one short passage with one AI request |

## Usage guides

[Sync](docs/SYNC.md) · [Story generation](docs/STORY_GENERATION.md)

## Developing

`crates/yomibu` is the application library; `crates/yomibu-cli` builds `yomibu`.
Shared contracts live in `yomibu-core`; implementations live in `yomibu-components`.
Read [AGENTS](AGENTS.md), then use the [task map](ARCHITECTURE.md#current-module-map)
to find code and tests. Read only the relevant [SPEC](SPEC.md) contract.
[PLAN](PLAN.md) lists open work; the
[glossary](docs/GLOSSARY.md) explains terms and milestone names.

Tests use local servers and synthetic data; no account or model key is needed.

Repeat these checks:

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps
git diff --check
```

[CI](.github/workflows/ci.yml) runs on Linux and macOS. API docs build into
`target/doc/yomibu`. [History](docs/history/README.md) holds earlier results and code pins.
