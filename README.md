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
target/debug/yomibu sync
target/debug/yomibu story
```

Usable vocabulary is required.
Run `yomibu` for all commands. See `yomibu help sync` and `yomibu help story`
for credential setup and permissions.
See [configuration examples](config/).

## Current commands

Use `cargo run --locked -- --help` for flags.

| Command | Purpose |
| --- | --- |
| `auth` | Set up credentials in macOS Keychain |
| `help` | Show commands and options |
| `sync` | Refresh the cache; needs `YOMIBU_WANIKANI_API_KEY` |
| `story` | Generate one short passage with one AI request |

## Developing

`crates/yomibu` is the application library; `crates/yomibu-cli` builds `yomibu`.
Shared contracts live in `yomibu-core`; implementations live in `yomibu-components`.
Read the [architecture and engineering guide](AGENTS.md#architecture), then the
relevant code and tests. The guide describes the target layout; code migration
is pending. The [glossary](GLOSSARY.md) explains domain terms.

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
`target/doc/yomibu`.
