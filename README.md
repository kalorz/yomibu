# Yomibu

Yomibu is an unofficial WaniKani tool for Japanese reading practice, with a Rust
CLI and library. It syncs learner data, previews story requests, generates sentence
candidates, and checks supplied text. The candidates are experimental; the checks
do not prove naturalness or correct meaning in context.

## Try it without an account

Install [Rustup](https://rustup.rs/), then run this from the repo root. Cargo uses
the pinned Rust 1.98.1 toolchain. The first build may download dependencies.
The demo needs no API key or dictionary and makes no model calls.

```sh
yomibu_demo_dir=$(mktemp -d)
trap 'rm -rf "$yomibu_demo_dir"' EXIT

cp tests/fixtures/mixed.json "$yomibu_demo_dir/wanikani.json"
cargo run --locked -- status --data-dir "$yomibu_demo_dir"

cargo run --locked -- prepare-retrieval \
  --inventory tests/fixtures/story/inventory.json \
  --request tests/fixtures/story/request.json \
  --embedding-cache "$yomibu_demo_dir/vectors.json" \
  --embedding-provider lexical-baseline

cargo run --locked -- preview-story \
  --inventory tests/fixtures/story/inventory.json \
  --request tests/fixtures/story/request.json \
  --embedding-cache "$yomibu_demo_dir/vectors.json" --json
```

Status prints saved observations. Preparation writes a vector cache using token
overlap, without understanding meaning. Preview shows the request without sending it.
Add Cargo's `--offline` before `--` once dependencies are cached.

## Current commands

Use `cargo run --locked -- --help` for flags.

| Command | Purpose |
| --- | --- |
| `status` | Read the saved WaniKani cache; no network or writes |
| `sync` | Refresh the cache; needs `WANIKANI_API_TOKEN` |
| `prepare-retrieval` | Prepare vectors with an explicitly chosen encoder |
| `preview-story` | Show the exact request using cached vectors; offline |
| `generate-story` | Make one paid attempt; needs a dictionary, `OPENAI_API_KEY`, and `--allow-model-call` |
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
72 MB and extracts about 217 MB into ignored `target/a1/current`. Tests use local
servers and synthetic data; no account or model key is needed.

```sh
python3 -B -m unittest discover -s scripts -p 'test_setup_a1_dictionary.py' -v
python3 scripts/setup_a1_dictionary.py
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps
git diff --check
```

[CI](.github/workflows/ci.yml) runs on Linux and macOS. API docs build into
`target/doc/yomibu`. [History](docs/history/README.md) holds earlier results and code pins.
