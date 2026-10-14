# Repository guide

## Architecture

Create directories only when needed.

In hexagonal architecture, driving adapters call application services; driven adapters
implement core ports. Domain-driven design (DDD) places rules in entities, value objects
and domain services. Crates need not match DDD bounded contexts (model boundaries).
Core owns domain rules, types and minimal extension contracts. Reuse or purity alone
does not justify putting code there.

Use a functional core and imperative shell: core business logic must not perform
I/O or read or mutate external state. App services and adapters own those effects.
This boundary applies equally to functions, methods and domain objects.
Pure provider-specific transformations stay in extensions.

### Directory ownership

Crates live in `crates/`. Each row names a crate and a directory inside its `src/` folder.

| Crate / directory | Put here |
| --- | --- |
| `yomibu-core/domain/<subject>/` | Domain state and rules, grouped by knowledge, generation and independent analysis. Keep source progress separate from derived eligibility. Use aggregates where related data must stay consistent. |
| `yomibu-core/ports/infrastructure/` | Interfaces for storage, repository queries, data sources and secret storage. |
| `yomibu-core/ports/capabilities/` | Interfaces for selection, preparation, generation and assessment. |
| `yomibu-core/ports/resources/` | Interfaces for shared dictionaries or embedders when separate consumers need them. |
| `yomibu-core/extension/` | Extension declarations and typed settings contracts shared by app and extensions. |
| `yomibu-app/application/<use_case>/` | Application services and workflow steps. Coordinate ports, execution order, budgets and failures; use core to enforce domain rules. |
| `yomibu-app/configuration/` | Typed application settings and immutable overrides for each request. |
| `yomibu-app/reports/` | Data prepared for reporting results. |
| `yomibu-ext/<extension>/` | Implementations grouped by provider or package. Keep integration clients, external data formats and backend details here, including adapters for secret storage. |
| `yomibu-cli/composition/` | CLI option and environment variable names, configuration and credentials. Construct extensions and connect dependencies. |
| `yomibu-cli/commands/`, `yomibu-cli/output/` | Parse requests, call services and format results. The executable owns the runtime and dependency construction (composition root). |

### Dependencies and composition

Production dependencies (`A -> B` means A may depend on B):

```text
yomibu-app -> yomibu-core
yomibu-ext -> yomibu-core
yomibu-cli -> yomibu-app + yomibu-ext (+ yomibu-core when needed)
```

- Ports specify behavior, execution requirements and failures. Implementations meeting
  the same contract swap through composition without backend-specific app logic.
  Revise contracts when required behavior changes.
- Port categories are project conventions. Components implement one or more capability
  roles, including pure calculations; pipelines order them.
- Infrastructure normally lasts for the app instance; choose capabilities per request.
  Share clients/resources while isolating mutable request state. One instance may serve
  several phases; each phase may use several instances. Keep implementation, instance
  identity and settings distinct.
- Extensions can combine capabilities, infrastructure adapters and resources. External
  extensions use core contracts; runtime loading and registries are optional.
- Query ports define core results, filtering and ranking. Storage/vector-search adapters
  retrieve data; embedders create vectors; ranking capabilities coordinate them.

### Supporting directories

Root `tests/fixtures/` and `tests/support/` hold shared test data and helpers.
`config/` holds examples; Cargo files define the build.

## Engineering rules

- Design from requested behavior. Existing names, structures, interfaces and plans
  are revisable within scope. Reuse code when it fits. Refactors preserve behavior;
  redesigns preserve the guarantees required by the task.
- Implement the smallest complete use case. Reassess unexpected growth; propose
  additional features separately.
- Justify each abstraction by a current caller, agreed boundary or invariant.
  Future flexibility alone is insufficient.
- Before delivery, simplify callers and remove unnecessary wrappers, options, stored
  state, cloning, repeated validation and duplicate tests.
- Add compatibility-only aliases, migrations or fallbacks only when requested.
- Handle recoverable failures without panics. Protect credentials. App services bound
  whole operations; adapters bound individual I/O.

## Plans and design discussions

Before implementing any slice, present its interfaces and data structures in chat
for discussion and explicit user approval. Show signatures, inputs, outputs,
important failures, owning layers and a minimal usage example with dependency
construction. Prefer pseudocode to prose; Ruby-style sketches are welcome.
Show existing contracts when unchanged; do not invent abstractions to fill a plan.

Wait for approval before implementation, including for small slices. If approved
interfaces or data structures need to change, present the revision and obtain
approval before implementing it.

Finish with affected files, exclusions and observable acceptance checks. Keep prose
to brief tradeoffs and open decisions. Plan only the current step; existing plans
do not authorize their full implementation.

## Documentation

Use plain English following [ISO 24495-1:2023](https://www.iso.org/standard/78907.html):
make information easy to find, understand and use. Explain unfamiliar terms.

Document intent, guarantees and decisions; keep implementation facts in code, tests
and build files. Define CLI help and examples in code for Clap's `help` and `--help`.

- `README.md`: purpose, setup, quickstart and development commands.
- `AGENTS.md`: architecture and working rules.
- `GLOSSARY.md`: domain terms.

Change docs for reader needs; add rules for project decisions or recurring problems.
Link rather than duplicate. Aim for fewer than 400 words per guide and 200 added words
per routine change across Markdown, comments and Rustdoc. Exceed only for requested
detail or essential contracts, retaining limits, errors and safety conditions.

## Testing and verification

- Test behavior; regressions must expose the original bug. Reuse tests, avoid
  implementation-mirroring checks and add no artificial tests for unchanged behavior.
- Test core rules and app workflows through ports. Prove replaceability with an
  independent implementation, including invalid outputs. Test adapter parsing, output,
  errors and wiring through public interfaces without repeating domain tests.
- Keep tests reproducible, isolated and parallel-safe. Use test credentials; leave the
  shared process environment unchanged. Select doubles explicitly. Test backend
  guarantees on actual backends; report unavailable prerequisites without substitutions.
- Rerun affected tests after simplification.

Use [CI](.github/workflows/ci.yml) for required checks. Use focused checks during
development; run workspace checks when shared ports change. Check documentation for
documentation-only changes. Report checks and gaps, new public contracts, unexpected
growth and any nonmechanical changes in structural refactors.
