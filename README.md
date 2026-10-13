# Yomibu

Yomibu creates Japanese reading passages from vocabulary you have studied. It has
a Rust command-line interface (CLI) and library. The readings are experimental:
they have not been checked for natural phrasing or correct meaning in context.

## First story

From the repository root, build with `cargo build --locked`.

On macOS, save your API keys in Keychain with `target/debug/yomibu auth`. You can supply
keys through environment variables on any supported platform:

```sh
export YOMIBU_WANIKANI_API_KEY=...
export YOMIBU_OPENAI_API_KEY=...
```

Replace `...` with your keys. Download your WaniKani data, then generate a story:

```sh
target/debug/yomibu sync
target/debug/yomibu story
```

Generation needs vocabulary you have studied in WaniKani.
Use `target/debug/yomibu --help` to list commands. Use
`target/debug/yomibu help sync` or `target/debug/yomibu help story` for options,
credential setup and permissions.
See [configuration examples](config/).

## Developing

`crates/yomibu-app` is the application library; `crates/yomibu-cli` builds `yomibu`.
Shared contracts live in `yomibu-core`; implementations live in `yomibu-ext`.
The [architecture guide](AGENTS.md#architecture) describes the layout.
The [glossary](GLOSSARY.md) explains domain terms.
The [planned story request](docs/plan/story-generation-request.yml) is an annotated
design draft, not configuration accepted by the current application.

Run tests with `cargo test --locked --all`. They use local servers and invented
data, so no account or model key is needed. [CI](.github/workflows/ci.yml) defines
the full checks and runs them on Linux and macOS.

Build API documentation with `cargo doc --locked --no-deps`; the output is in
`target/doc/yomibu_app`.
