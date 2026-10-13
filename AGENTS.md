# Repository guide

Read relevant code, tests and task contracts; load history only when needed.

## Architecture

Target layout; migration is pending in a separate PR: `yomibu` becomes `yomibu-app`,
`yomibu-components` becomes `yomibu-ext`. Directories place necessary code;
they do not require building every category or deferred example.

Hexagonal architecture: driving adapters invoke application services; driven adapters
implement core-owned ports. DDD entities, value objects and domain services hold
business rules. Crates are dependency boundaries, not necessarily bounded contexts.

### Directory ownership

Paths mean `crates/<crate>/src/...`.

| Crate / directory | Put here |
| --- | --- |
| `yomibu-core/domain/<subject>/` | State and invariants, grouped by knowledge, generation and independent analysis concepts. Aggregates enforce consistency within their subject. |
| `yomibu-core/ports/infrastructure/` | Store, repository/query, source and secret contracts. |
| `yomibu-core/ports/capabilities/` | Selection, preparation, generation and assessment contracts. |
| `yomibu-core/ports/resources/` | Shared dictionary/embedder contracts when independent consumers need them. |
| `yomibu-core/extension/` | Extension declarations and typed settings contracts. |
| `yomibu-app/application/<use_case>/` | Services and adjacent workflow steps: coordinate ports, ordering, budgets and failures; invoke core rules. |
| `yomibu-app/configuration/` | Typed settings and immutable invocation overrides. |
| `yomibu-app/reports/` | Result projections. |
| `yomibu-ext/<extension>/` | Provider/package implementations, private integration clients, transport DTOs and backend details, including secret-store adapters. |
| `yomibu-cli/composition/` | Configuration/environment/credentials; construct extensions and inject dependencies. |
| `yomibu-cli/commands/`, `yomibu-cli/output/` | Parse requests, invoke services, render results. The executable owns the runtime and composition root. |

### Dependencies and composition

Allowed production dependencies:

```text
yomibu-app -> yomibu-core
yomibu-ext -> yomibu-core
yomibu-cli -> yomibu-app + yomibu-ext (+ yomibu-core when needed)
```

- App and extensions stay independent; core provides their shared contracts.
- Port categories are project conventions. Capabilities are roles; pipelines order
  them. Components implement one or more capabilities, including pure calculations.
- One instance may serve multiple phases; one phase may use multiple configured
  instances. Distinguish implementation identity, instance identity and settings.
- Infrastructure is normally application-scoped; compose workflows per invocation, omitting
  optional capabilities as needed. Core invariants always apply. Share resources;
  isolate request scope and mutable execution state from shared clients.
- Extensions package capability implementations, infrastructure adapters and resources.
  External extensions depend on core, without built-ins or mandatory runtime loading/registries.
- Repository/query ports expose domain types and define filter/ranking semantics; adapters
  execute backend queries. Embedders produce vectors; vector-search adapters search;
  ranking capabilities coordinate both.

### Tests and supporting directories

Crate tests cover domain rules, app workflows, extension conformance/backend guarantees
and CLI behavior. Root `tests/{fixtures,support}/` holds shared inputs/helpers;
`config/` holds examples, `docs/` references/history, and Cargo files build configuration.

## Engineering rules

- Implement the smallest sufficient use case. Estimate scope; reassess and simplify
  unexpected growth.
- Prefer direct functions and concrete types. Justify abstractions by an agreed
  boundary, invariant or current problem. Extract helpers for meaningful operations
  or actual duplication.
- No backward compatibility unless requested; do not add compatibility-only aliases,
  migrations or fallbacks. Read-only explanations/reviews do not authorize edits.
- Preserve source progress separately from derived eligibility. Name volatile
  adapters `InMemory...`. Preserve historical evidence, fixture bytes, code pins
  and conclusions; current engineering fixtures may evolve.
- Keep deterministic logic synchronous. Use typed recoverable errors, never
  production `unwrap()`/`expect()` for them; dynamic errors belong at executable boundaries.
- Bound external I/O and protect credentials. Preserve usable caches on failure;
  distinguish pre-replacement failure from uncertain durability after replacement.
- Escape untrusted values, including dependency errors and executable names, before
  formatting. Preserve Japanese readability and trusted diagnostic layout; never
  blanket-escape assembled output.

## Documentation: short, simple English

Document intent, guarantees, and decisions that code cannot explain.
Use code, tests and manifests for implementation facts.

Keep CLI descriptions, option help and examples in Clap command definitions,
accessible through `help` / `--help`. Limit README usage to a quickstart;
do not maintain separate Markdown command references.

| Document | Purpose |
| --- | --- |
| `README.md` | Purpose, setup, quickstart, development commands. |
| `AGENTS.md` | Architecture and engineering rules. |
| `SPEC.md` | Product intent and constraints. |
| `docs/GLOSSARY.md` | Domain terms. |
| `docs/SYNC.md`, `docs/STORY_GENERATION.md` | Task guarantees. |
| `docs/history/` | Frozen evidence. |

Default to no docs; patch changed usage/contracts only. New prose needs a reader
and purpose. Link instead of duplicating. Aim under 400 words per guide, 200 added
per routine change across Markdown/comments/Rustdoc; exceed only for requested
detail or essential contracts. Preserve limits, errors and safety conditions.

## Strict TDD for behavioral changes

- RED: verify a behavioral test fails as expected before implementation.
  GREEN: implement minimally and verify.
  REFACTOR: review each cycle and the complete change; remove unnecessary types,
  wrappers, options, stored state, fallbacks and duplicate tests. Rerun affected tests.
  Reuse simple test setup; avoid implementation-mirroring tests and per-helper coverage.
  Nonbehavioral changes need no artificial tests.
- Tests must be deterministic and parallel-safe: local HTTP mocks, isolated temporary
  directories, no real secrets or process-global environment mutation. Test backend
  boundaries; memory adapters cannot prove durability. Never silently replace
  unavailable production dependencies with test doubles.
- Exercise CLI presentation/parser/diagnostic changes through the executable:
  controls, Japanese readability, layout, stdout/stderr, exit status and help/version.

Before completing implementation, run:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

Report checks/limitations and, for code changes, files touched, production lines
added/deleted and new public abstractions in PR/chat. Prioritize readability over line counts.
