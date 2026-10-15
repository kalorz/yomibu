# Yomibu

Yomibu is being rebuilt to create Japanese reading material. The previous
implementation has been removed; the workspace retains four minimal crates,
architecture enforcement and the [design plans](docs/plan/).

Build with `cargo build --locked`. The `yomibu` executable reports that no commands
are implemented and exits unsuccessfully. Story generation, synchronization and
credential management are not currently available.

## Developing

`crates/yomibu-app` is the application library; `crates/yomibu-cli` builds `yomibu`.
Shared contracts live in `yomibu-core`; implementations live in `yomibu-ext`.
The [architecture guide](ARCHITECTURE.md) defines enduring boundaries and practices;
[AGENTS.md](AGENTS.md) gives the working instructions for agents.
The [glossary](GLOSSARY.md) explains domain terms.
The [request example](docs/plan/story-generation-request.yml) explores a possible
structure; it neither specifies CLI input formats nor finalizes product behavior.
[Durable execution](docs/plan/durable-execution.md) and [replay](docs/plan/replay.md)
are exploratory proposals.

Run Rust tests with `cargo test --locked --all`; application tests exercise the
collection query with supplied storage. [CI](.github/workflows/ci.yml) defines the
full checks and runs them on Linux and macOS. The architecture enforcement tests
below remain active.

Check crate boundaries with `python3 -B tools/check_architecture.py` using Python 3
and the repository Rust toolchain. Install the pinned architecture analyzer with
`cargo install cargo-bylaw --version 0.1.0 --locked`, then run
`python3 -B tools/check_domain.py`. Verify the enforcement contracts with
`python3 -B -m unittest discover -s tools -p 'test_*.py' -v`.
Architecture checks run separately from `cargo test`.
See [verification](ARCHITECTURE.md#verification) for coverage and remaining
enforcement gaps. No legacy violations are exempted.

Build API documentation with `cargo doc --locked --no-deps`; the output is in
`target/doc/yomibu_app`.
