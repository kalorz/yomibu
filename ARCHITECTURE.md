# Architecture

Draft for the restart; review before making it the governing reference in
[AGENTS.md](AGENTS.md). This describes the target architecture, not current code
compliance. Enforcement below is proposed. Preserve every file in
[docs/plan](docs/plan), including the SQL schema and story-request YAML.

## Boundaries

Use hexagonal architecture with a functional core and imperative shell.
Production dependencies between workspace crates follow this graph:

```text
yomibu-app -> yomibu-core
yomibu-ext -> yomibu-core
yomibu-cli -> yomibu-app + yomibu-ext (+ yomibu-core)
```

Future web and MCP crates occupy the CLI's architectural position.

| Location | Responsibility |
| --- | --- |
| `yomibu-core/src/domain/` | Domain values, invariants, and deterministic decisions. |
| `yomibu-core/src/ports/infrastructure/` | Outbound contracts for deployment-configured services: storage, secrets, cache. |
| `yomibu-core/src/ports/capabilities/` | Outbound contracts for replaceable operations: selection, importing, embedding, preparation, generation, assessment, repair. |
| `yomibu-app/src/application/` | Inbound use-case API and whole-workflow orchestration. |
| `yomibu-ext/src/` | Concrete capability and infrastructure implementations. |
| CLI, web, MCP | Driving adapters and composition roots that construct and connect implementations. |

Paths above are relative to `crates/`; create directories when needed.
Inbound/driving and outbound/driven describe who initiates an operation, not data
flow. Infrastructure/capabilities subdivide outbound responsibilities, not instance
lifetimes. Public application functions or methods can be inbound ports without
forwarding traits. Core owns outbound contracts; types from provider SDKs, database
drivers, and delivery frameworks stay outside them. Pure library value types are allowed.

## Purity and composition

Domain code must not depend directly or indirectly on ports or adapters, perform
I/O, or obtain ambient time, randomness, or environment values. Supply relevant
observations and seeds explicitly. Effectful callbacks do not bypass this rule;
local mutation is allowed.

Introduce required infrastructure and capability interfaces with their first
implementation. Aim for the smallest cohesive interface each consumer needs.
Ordinary domain helpers and different configuration values do not automatically
require separate ports. Architectural boundaries themselves justify interfaces.

Capabilities may use other injected outbound ports. The application owns overall
execution order, budgets, deadlines, cancellation, and recovery; nested calls must
participate in those controls. Implementations must not hide paid calls or retries.
Provider construction belongs at composition roots; applications select among
supplied capabilities according to requests.

An extension may implement multiple ports, and one type may implement several
traits. Shared clients and database pools do not require a third resources category.

## Selection example

Follow the [story request](docs/plan/story-generation-request.yml):

```text
CLI -> application -> selector
selector -> embedder -> query vector
selector -> repository(resolved query) -> ordered material
selector -> application -> preparer -> application -> generator
```

The selector resolves requested evidence and prepares typed query data. The
repository receives filters, ordered ranking rules, vectors/profiles, scope, targets,
and limits; it executes filtering, joins, ranking, and limits efficiently. It does
not call an embedder. Preserve mandatory eligibility, missing-vector behavior, and
deterministic ties. Selected material remains distinct from full eligibility.

## Enforcement to design

| Rule | Planned verification |
| --- | --- |
| Dependency direction | Cargo graph checks across supported targets/features; distinguish test-only dependencies. |
| Domain purity | Module/API checks plus review of indirect effects; Rust cannot prove general purity. |
| Contract behavior | Application tests through ports and adapter tests against actual backends. |
| Cohesive interfaces and ownership | Review responsibilities, callers, and invariants. |

Demonstrate that architecture checks reject representative violations before
feature reconstruction. Do not claim enforcement merely because a check passes.

Agent instructions should require identifying affected boundaries before editing,
verifying relevant contracts before delivery, and reporting evidence and gaps.
Flag rule conflicts and changes to enforcement explicitly. Remove abstractions
without a required boundary, behavior, or invariant. Use established domain terms.

Keep explanations here, concise working instructions in `AGENTS.md`, and behavioral
contracts beside interfaces. Exact query types, repository boundaries, eligibility
capture, and the enforcement mechanisms remain design work.
