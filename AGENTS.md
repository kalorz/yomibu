# Repository guide

## Architecture

Create directories only when needed.

In hexagonal architecture, driving adapters call application services; driven adapters
implement core ports (interfaces). Domain-driven design (DDD) places business rules
in entities, value objects and domain services. Crates need not match DDD bounded
contexts (model boundaries).

Core owns domain rules, types and minimal contracts for replaceable extensions. Reuse or
purity alone does not justify putting code there.

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

- Core ports specify required behavior, execution requirements and failures.
- Connect new implementations at the composition root. Existing ports must allow
  this without changing core or adding provider-specific app logic.
- Port categories are project conventions. Components implement one or more
  capability roles, including pure calculations. Pipelines order their execution.
- One instance can serve several phases; a phase can use several configured instances.
  Keep implementation, instance identity and settings distinct.
- Infrastructure normally lasts for the app's lifetime. Choose optional capabilities
  per request. Core enforces its rules. Share clients and resources, while keeping
  request scope and mutable execution state separate.
- Extensions group capabilities, infrastructure adapters and resources. External
  extensions use core contracts. Built-in dependencies, runtime loading and registries
  are not required.
- Repository and query ports define core result types, filtering and ranking.
  Adapters run backend queries. Embedders create vectors; vector-search adapters
  find matches; ranking capabilities coordinate both.

### Supporting directories

Root `tests/fixtures/` and `tests/support/` hold shared test data and helpers.
`config/` holds examples; Cargo files define the build.

## Engineering rules

- For structural refactors, move and reconnect existing code first. Add logic or
  public APIs only to preserve required behavior or enforce an agreed architecture
  boundary.
- Before adding an abstraction, identify its current caller, required behavior or
  invariant, and why existing code cannot serve it. Future flexibility alone is
  insufficient.
- Before delivery, review nonmechanical additions for removal: wrappers, generic
  handling, configuration options, cloning and repeated validation. Remove additions
  whose purpose disappears when callers are simplified.
- Preserve compatibility only when requested. Add no aliases, migrations or fallback
  behavior solely for old versions.
- Handle recoverable failures without panicking.
- Protect credentials. App services enforce limits for the whole operation;
  adapters enforce limits on individual I/O operations.

## Documentation

Use plain English following [ISO 24495-1:2023](https://www.iso.org/standard/78907.html):
write what intended readers need and make it easy to find, understand and use.
Keep necessary technical terms and explain unfamiliar ones.

Document intent, guarantees and decisions that code does not explain.
Keep implementation facts in code, tests and build files.

Define CLI descriptions, option help and examples in code so Clap exposes them through
`help` and `--help`. Keep README usage to a quickstart.

| Document | Purpose |
| --- | --- |
| `README.md` | Purpose, setup, quickstart, development commands. |
| `AGENTS.md` | Architecture and engineering rules. |
| `GLOSSARY.md` | Domain terms. |

Change docs only when readers need new or clearer information. Add AGENTS rules
for project decisions or repeated problems an instruction can prevent. Link rather
than duplicate. Aim for fewer than 400 words per guide and 200 added words per
routine change across Markdown, comments and Rustdoc. Exceed only for requested
detail or essential contracts; preserve limits, errors and safety conditions.

## Testing and verification

- Test changed behavior. Regression tests must expose the original bug. Reuse tests;
  avoid checks tied to internal code structure. Changes that preserve behavior need
  no artificial tests.
- Test core rules, app workflows through ports, and extension and backend contracts.
  Check public ports with implementations outside production crates, including
  implementations that return invalid output.
- Test driving adapters through their public interfaces: parsing, output, errors
  and dependency connections. Avoid repeating domain tests at each interface.
- Keep tests reproducible, isolated and safe to run together. Use test credentials
  and leave the shared process environment unchanged. Select test doubles explicitly.
  Test backend guarantees on actual backends; report missing prerequisites.
  Do not silently substitute backends.
- Rerun affected tests after simplification.

Use [CI](.github/workflows/ci.yml) for required completion checks. Run focused checks
during development and workspace checks when shared ports change.
For documentation-only changes, check the documentation.

Report the checks you ran and any gaps. Explain new public contracts or unexpectedly
large changes when the reviewer needs that context.
For structural refactors, identify changes beyond file moves and explain why they
are needed.
