# Architecture

Draft for the restart; review before making it the governing reference in
[AGENTS.md](AGENTS.md). This describes the target architecture, not current code
compliance. Enforcement below is proposed. Preserve every file in
[docs/plan](docs/plan), including the SQL schema and story-request YAML.

## Design practices

Hexagonal architecture and functional core/imperative shell govern the design.
Use the other practices to support those boundaries, with the following purposes:

| Practice | Application in Yomibu |
| --- | --- |
| Hexagonal architecture | Define application boundaries, ports, adapters and dependency direction. |
| Functional core, imperative shell | Compute domain decisions from explicit inputs; coordinate effects outside the domain. |
| Strategic DDD | Use the glossary's domain language and identify model ownership and boundaries. A bounded context need not equal a crate or repository. |
| Tactical DDD | Use value objects, distinct IDs, entities and aggregates to express actual invariants. Foreign-key connections alone do not define aggregates. |
| SOLID | Keep responsibilities cohesive, expose the interfaces consumers need, preserve behavioral contracts and depend on abstractions. |
| Clean Architecture | Keep use cases and domain rules independent of delivery mechanisms and external technologies. No additional directory layers are implied. |
| Clean Code | Prefer clear names and understandable control flow; splitting functions or adding wrappers must serve a purpose. |
| OOP | Use encapsulation and polymorphism through Rust structs, enums and traits where useful; ordinary functions remain appropriate. |
| Design by Contract | Define port preconditions, guarantees, invariants and failure semantics, including ordering, limits, atomicity and uncertain outcomes where applicable. Contract tests verify interchangeable behavior. |
| Model valid states with types | Use distinct IDs, request versus resolved-query types and explicit assessment outcomes to prevent meaningful mistakes. External facts still need runtime checks; add types for real distinctions or invariants. |

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
Direction is relative to the application and domain boundary:

```text
CLI -> application.generate_story(request)  : inbound / driving
application -> selector.select(request)    : outbound / driven
```

These arrows show calls, not Cargo dependencies. A returned result does not change
port direction. Infrastructure/capabilities subdivide outbound responsibilities,
not instance lifetimes. Public application methods can be inbound ports: CLI and
web can call the same `generate_story` method without an additional trait repeating
that method. Core owns outbound contracts. Pure library value types are allowed.

## Mandatory rules for the restart

| Area | Obligation |
| --- | --- |
| Dependencies | Follow the crate graph. Inner contracts expose no types from database drivers, provider SDKs or delivery frameworks. |
| Functional core | Domain code makes deterministic decisions on explicit inputs. It must not depend directly or indirectly on ports or adapters, perform I/O or obtain ambient time, randomness or environment values. |
| Ports | Required infrastructure and replaceable capabilities have interfaces from their first implementation, even with one adapter. |
| Application | Coordinate whole workflows through supplied ports. Composition roots configure available implementations; requests select among them through the application. |
| Adapters | Implement contracts and translate external representations. Storage may use joins and backend optimizations while preserving query guarantees. |
| Simplicity | Each additional abstraction serves a required boundary, behavior or invariant. A required architectural boundary itself justifies an interface. |
| Terminology | Follow [GLOSSARY.md](GLOSSARY.md) in code, documentation and conversation. Explain and define genuinely new concepts; do not invent synonyms for established ones. |

Effectful callbacks do not bypass the domain rule; local mutation is allowed.
Use the smallest cohesive interface each consumer needs. The embedder needs an
interface even with only OpenAI implemented. A private helper that collects story
topics can remain a function. Different configuration values alone do not require
separate ports.

### Simplicity

| Principle | Interpretation |
| --- | --- |
| KISS | Choose the clearest design that satisfies required boundaries and behavior. |
| DRY | Keep business rules and authoritative definitions consistent. Similar-looking code with different responsibilities need not share an abstraction. |
| YAGNI | Defer speculative features and machinery. Required ports, boundaries, invariants and their verification are present needs. |

Mandatory architectural rules and behavioral contracts take precedence over
preferences for fewer lines, fewer types or removing apparent duplication.

## Composition and request execution

Capabilities may use other injected outbound ports. The application owns overall
execution order, budgets, deadlines, cancellation, and recovery; nested calls must
participate in those controls. Implementations must not hide paid calls or retries.
For example, startup can resolve credentials through environment configuration or
a secret store, construct OpenAI and Anthropic clients, and supply both generation
implementations. One request can select OpenAI, another Anthropic, and a third both
for parallel runs. The application handles configured capability identities without
constructing SDK clients or containing provider-specific branches. Construction
may be scoped or deferred; startup is an example, not a lifetime requirement.

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
Flag rule conflicts and changes to enforcement explicitly. These are standing agent
obligations to encode in `AGENTS.md`, not reminders users must repeat in prompts.

Keep explanations here, concise working instructions in `AGENTS.md`, and behavioral
contracts beside interfaces. Exact query types, repository boundaries, eligibility
capture, and the enforcement mechanisms remain design work.
