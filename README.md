# Yomibu

Yomibu creates Japanese readings from vocabulary you have studied. It has a Rust
CLI and library. Readings are experimental; checks do not prove naturalness or
correct meaning in context.

## First story

Build with `cargo build --locked`, then supply two keys:

```sh
export YOMIBU_WANIKANI_API_KEY=...
export YOMIBU_OPENAI_API_KEY=...
target/debug/yomibu story
```

Usable vocabulary is required. No dictionary or embedding model is needed.
Run `yomibu` for all commands or `yomibu help story` for setup and permissions.
See [story settings](docs/STORY_GENERATION.md#configuration-and-optional-work).

## Try it without an account

Install [Rustup](https://rustup.rs/), then run this from the repo root. Cargo uses
the pinned Rust 1.98.1 toolchain. The first build may download dependencies.
The demo needs no API key or dictionary and makes no model calls.

```sh
yomibu_demo_dir=$(mktemp -d)
trap 'rm -rf "$yomibu_demo_dir"' EXIT

cp tests/fixtures/mixed.json "$yomibu_demo_dir/wanikani.json"
cargo run --locked -- status --data-dir "$yomibu_demo_dir"

cargo run --locked -- preview-story --data-dir "$yomibu_demo_dir" \
  --inventory tests/fixtures/story/inventory.json \
  --request tests/fixtures/story/request.json --json
```

Status prints saved observations. Preview shows the request without sending it.
Add Cargo's `--offline` before `--` once dependencies are cached.

## Current commands

Use `cargo run --locked -- --help` for flags.

| Command | Purpose |
| --- | --- |
| `status` | Read the saved WaniKani cache; no network or writes |
| `sync` | Refresh the cache; needs `YOMIBU_WANIKANI_API_KEY` |
| `prepare-retrieval` | Prepare vectors with an explicitly chosen encoder |
| `preview-story` | Show the exact generation request offline |
| `story` | Generate one short passage with one AI request |
| `analyze` | Check one sentence offline; needs input JSON and a dictionary |
| `dictionary import` / `verify` | Install or verify a local dictionary bundle; no downloads |

A completed Pass, Fail, or Inconclusive exits zero. Zero means the check ran,
not that the sentence is an accepted exercise.

## Usage guides

[Sync/status](docs/SYNC.md) · [Story generation](docs/STORY_GENERATION.md) ·
[Analysis](docs/ANALYSIS.md) · [Dictionary](docs/DICTIONARY.md) · [Retrieval](docs/RETRIEVAL.md)

## Developing

`crates/yomibu` is the library; `crates/yomibu-cli` builds `yomibu`.
Read [AGENTS](AGENTS.md), then use the [task map](ARCHITECTURE.md#current-module-map)
to find code and tests. Read only the relevant [SPEC](SPEC.md) contract.
[PLAN](PLAN.md) lists open work; the
[glossary](docs/GLOSSARY.md) explains terms and milestone names.

Quick tests without a dictionary:

```sh
cargo test --locked -p yomibu --test knowledge --test inventory
```

The full suite needs Python 3.8+ and the pinned dictionary. Setup downloads about
72 MB and extracts about 217 MB into ignored
`target/test-resources/sudachi-core/current`. Tests use local servers and synthetic
data; no account or model key is needed.

```sh
python3 -B -m unittest discover -s scripts -p 'test_setup_test_dictionary.py' -v
python3 scripts/setup_test_dictionary.py
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps
git diff --check
```

[CI](.github/workflows/ci.yml) runs on Linux and macOS. API docs build into
`target/doc/yomibu`. [History](docs/history/README.md) holds earlier results and code pins.
