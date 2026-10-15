# Architecture

This document defines Yomibu's enduring architectural rules. Existing code is not
proof of compliance. Examples illustrate the rules; they are not exhaustive designs
or commitments to exact APIs. Architecture checks below remain proposed.

The [SQL schema](docs/plan/schema.postgresql.sql) is a retained requirement.
Other [design references](#design-references) contain proposals: preserving a plan
does not approve all its contents. Product behavior comes from approved requirements
and interface contracts. Architectural decisions can [evolve explicitly](#changing-architecture).

## Design practices

Hexagonal architecture and functional core/imperative shell govern the design.
Supporting practices apply as described here; examples do not add mandatory features.

| Practice | What we adopt | Yomibu example |
| --- | --- | --- |
| Hexagonal architecture | Explicit ports and adapters with dependencies toward inner contracts. | Application calls a generator port; the provider SDK stays in its adapter. |
| Functional core, imperative shell | Deterministic domain decisions from explicit inputs; effects outside domain. | Core calculates eligibility from supplied learner observations. |
| Strategic DDD | Shared language within model boundaries and explicit translation between models. See [selected concepts](#selected-ddd-concepts). | Collection, subject and entry keep their meanings across operations using the same model. |
| Tactical DDD | Model identity, values and consistency rules where needed. See [selected concepts](#selected-ddd-concepts). | Distinct subject and entry IDs prevent accidental substitution. |
| SOLID | Cohesive responsibilities, consumer-sized interfaces, behavioral substitutability and dependency inversion. | A selector receives only the query and embedding interfaces it needs. |
| Clean Architecture | Domain rules and use cases stay independent of delivery and external technologies; no extra layers are implied. | CLI, web and MCP invoke the same application operations. |
| Clean Code | Clear names and control flow; extra wrappers need a concrete purpose. | A readable assessment operation need not become a generic pipeline. |
| OOP | Encapsulation and polymorphism through Rust types and traits where useful. | A validated value can encapsulate constraints; eligibility can remain a pure function. |
| Design by Contract | Explicit preconditions, guarantees and failures, verified at relevant boundaries. | Every storage adapter honors its query's scope, ordering and atomicity guarantees. |
| Model valid states with types | Types distinguish meaningful states; external facts still need runtime checks. | An unresolved selection request differs from a resolved storage query. |
| CQRS (Command Query Responsibility Segregation) | Separate read/change contracts where consumers or guarantees differ. See [storage](#storage-and-cqrs). | Narrow query and import interfaces can share one PostgreSQL adapter. |

### Selected DDD concepts

These guide modeling decisions, not a checklist of constructs to create everywhere.

| Area / concept | Rule and illustrative application |
| --- | --- |
| Strategic: Ubiquitous Language | Use [GLOSSARY.md](GLOSSARY.md) in discussion, code and tests. Collection, subject and entry retain their distinct meanings within the same model. |
| Strategic: Bounded Contexts | Identify where a model and its language apply. Crates, phases and tables do not automatically define contexts. Document a meaning that legitimately differs by context. |
| Strategic: relationships between models | Define translations at model boundaries; external provider structures must not dictate the domain model. Translate WaniKani observations while preserving their source meaning. |
| Tactical: Entities and Value Objects | Use identity for entities and meaningful equality/validation for values. Subjects have identity; subject IDs and entry IDs are different value types. |
| Tactical: Aggregates | Choose consistency boundaries from business invariants. Atomic entry/detail publication informs the decision; a foreign-key graph alone does not define an aggregate. |
| Tactical: Domain Services | Use pure domain functions for rules with no natural entity owner, such as eligibility calculations across supplied observations. |
| Tactical: Repositories | Use aggregate persistence contracts where needed. Read/query ports may return projections without reconstructing aggregates. |

## Boundaries

Permitted production dependencies between workspace crates:

```text
yomibu-app -> yomibu-core
yomibu-ext -> yomibu-core
yomibu-cli -> yomibu-app + yomibu-ext (+ yomibu-core)
```

Future `yomibu-web` and `yomibu-mcp` have the CLI's position. Retain these boundaries;
create directories for actual responsibilities. A workflow phase does not automatically
require a domain module. Purity alone does not place a helper in domain; it must express
domain meaning. Pure library dependencies are allowed. Review integration-test
dependencies separately; they must not leak into production dependencies or public APIs.

Paths below are relative to `crates/`:

| Location | Ownership |
| --- | --- |
| `yomibu-core/src/domain/` | Domain values and pure decisions, such as eligibility and assessment aggregation. |
| `yomibu-core/src/ports/infrastructure/` | Outbound service contracts: storage queries/writes, secrets and cache. Storage queries remain infrastructure even when called `select`. |
| `yomibu-core/src/ports/capabilities/` | Outbound contracts for replaceable operations, such as selection, importing, embedding and assessment, including pure implementations. |
| `yomibu-app/src/application/` | Inbound APIs, shared normalization through core rules, operation composition, execution limits and failure handling. |
| `yomibu-ext/src/` | Implementations grouped by provider or component; SDK clients, SQL, file formats, provider options and error translation. |
| Driving crates | Translate supported inputs into application requests, invoke operations and present results. Composition roots configure and connect implementations. |

### Port roles and labels

Direction is relative to the application/domain boundary: inbound/driving ports
are called by driving adapters; outbound/driven ports are called by application or
capability implementations. Returning results does not reverse those roles.
Infrastructure/capability is a second classification, only for outbound ports.
It describes responsibility, not lifetime: a shared embedder remains a capability;
a storage transaction may be per-request.

Label contract declarations in Rustdoc, alongside their behavior guarantees:

```text
Inbound application method or trait: Port: inbound / driving.
Outbound capability trait:          Port: outbound / driven. Role: capability.
Outbound infrastructure trait:      Port: outbound / driven. Role: infrastructure.
```

Use these labels with module placement. Label contracts rather than whole implementation
types: one type may implement several ports. Inbound APIs may be public application
methods; a trait is justified when substitution or another concrete need requires it.
Do not add empty marker traits just for classification. Add macro/attribute metadata
only if checks need information existing declarations cannot reliably provide;
labels alone do not enforce architecture.

Outbound ports use infrastructure and capabilities; there is no `ports/resources`
category. Shared SDK clients are adapter details. Shared embedding behavior is a
capability; stored dictionary access fits infrastructure. Even future model/GPU
resource management would normally be infrastructure. Sharing alone does not justify
a new category; propose a distinct responsibility through an architecture decision.

## Architectural rules

| Area | Obligation |
| --- | --- |
| Dependencies | Follow the graph. Core/app public contracts expose no SDK clients/responses, database connections/rows or transport-framework types. |
| Functional core | Domain decisions are deterministic for the same explicit inputs. Domain must not depend on ports/adapters, perform I/O or obtain ambient time, randomness or environment values. Supply observations and seeds explicitly. |
| Ports | Required infrastructure services and replaceable capabilities have interfaces from their first implementation, even with one adapter or a pure implementation. Interfaces describe behavioral boundaries; settings are ordinarily typed data. |
| Application | Execute operations through supplied ports and apply core decisions. External data/service access stays behind ports; runtime scheduling may live in the shell. |
| Adapters | Translate external representations and honor contracts. Optimizations preserve semantics; expected failures are results, not panics. |
| Simplicity | Every extra abstraction serves a required boundary, behavior or invariant. A single implementation does not weaken that justification. |
| Terminology | Use glossary terms within their model context; define new concepts and resolve conflicting meanings explicitly. |

A domain object invoking a generator or even a pure selector port violates the
boundary: invoke capabilities outside domain, then pass returned data to pure
functions. Indirect calls and effectful callbacks cannot bypass this rule; local
mutation is allowed. Opaque captured provider payloads can be data, but domain
decisions must not interpret SDK representations. Core owns policy; application
coordinates enforcement, including where schema comments say "application".

A private topic-extraction helper needs no trait. Different model settings do not
require different generation ports; retrieving secrets is service behavior requiring
a port. Concrete provider settings belong with their extension's configuration.

### Simplicity

| Principle | Interpretation |
| --- | --- |
| KISS | Choose the clearest design satisfying required boundaries and behavior. |
| DRY | Do not repeat knowledge: maintain one authoritative definition of each rule or decision. Similar code need not represent the same knowledge. |
| YAGNI | Defer unrequested behavior and machinery, never agreed architectural obligations. |

Required ports, dependency boundaries, domain purity and contract verification are
current requirements. Never defer them for one adapter or fewer lines/types. Build
requested behavior within those boundaries; a hypothetical provider or generic plugin
loader is not automatically needed. Propose a rule change instead of bypassing a
boundary in the name of simplicity.

A rule may have equivalent pure and SQL representations. Verify their shared meaning
through contract tests rather than forcing inefficient data loading to reuse code.

## Composition and request execution

Composition roots select available implementations and resolve configuration/secrets.
For example, they can construct OpenAI and Anthropic clients together; application
selects supplied capability identities per request, including both for parallel work.
Identity lookup is allowed; constructing SDK clients or branching on provider response
fields in app is not. Construction can be startup, scoped or deferred through supplied
interfaces without introducing provider dependencies into app.

Capabilities may compose injected outbound ports without constructing their providers.
One extension/type can implement several ports and share clients or pools. Keep request
state separate. Provider-independent execution enforcement belongs in app; provider
protocol handling belongs in ext.

Application owns operation-wide budgets, deadlines, cancellation and failure handling.
Nested operations, capability calls and retries share the parent's execution limits;
enforce them before dispatch. Do not hide calls behind private SDK clients or reset
limits when invoking a sub-operation. Propagate deadlines/cancellation and bound I/O.
Do not automatically retry uncertain external completion. Keep credentials out of
application requests, reports and captured execution artifacts.

### Application operations and input

Expose an inbound operation when callers need to request it independently. Each has
explicit inputs, preconditions and results; phases and helpers do not automatically
become services. Here, a service is an in-process application responsibility, not an
additional deployed process.

Illustrative entry points include assessing an existing story, deconstructing a
sentence, synchronizing data and generating a story. A standalone assessment entry
point and generation workflow reuse the same assessment operation. They obtain needed
context explicitly instead of assuming hidden earlier phases.

For an explicitly requested `story --sync` option, the CLI conveys that intent to the
application, which composes the same synchronization operation exposed by `sync`
before generation. It does not invoke another CLI command or duplicate synchronization
logic. This is an explicit prerequisite, not an implicit import inside selection.
Standalone and composed calls preserve the operation's guarantees; nested calls use
the parent's execution limits. No generic workflow engine is required.

Driving adapters translate supported input forms into application inputs. Application
APIs are independent of those forms. CLI flags, JSON or other formats are delivery
choices; parsing and input precedence belong there. Shared semantic defaults and domain
validation belong in application/core so equivalent requests behave consistently.

### Illustrative selector composition

This example demonstrates ownership, not a complete workflow or fixed API:

```text
driving adapter -> application -> selector
selector -> injected embedder -> query vector
selector -> injected repository(resolved query data) -> selected material
selector -> application -> next requested operation
```

The application coordinates the overall operation. The selector resolves evidence
through supplied ports. The repository receives query data, never an embedder or
effectful callback; it may use SQL joins, filters, ranking and limits efficiently.
Pure domain decisions remain in core. Detailed selection and assessment behavior
belongs in approved requirements and interface contracts, not this illustration.

## Storage and CQRS

Choose contracts by consumer needs, query semantics and required atomic invariants,
not one per table or foreign-key-connected group. Narrow query and write interfaces
may share one PostgreSQL adapter, pool, canonical schema and existing foreign keys.
Queries can return projections and perform joins without reconstructing aggregates.

Reads do not change business state. Commands may read and return results; preserve
required read/write transactions, including atomic entry/detail publication under
the retained schema. CQRS requires no separate databases, event sourcing, messaging
or asynchronous synchronization. Repository reads are I/O and remain outside domain.

Separate interfaces do not promise arbitrary backend combinations. PostgreSQL cannot
satisfy a required subject FK when the subject exists only in CSV. Importing real
subject records into PostgreSQL can satisfy it; fake parents, weakened required FKs
and adapter-specific schema variants cannot. With file-based progress and SQL subjects,
PostgreSQL cannot enforce references in those files. Application checks must address
missing/deleted subjects and concurrency; a lookup alone is not equivalent to an FK.

Keep records together where database-enforced integrity is required. Support mixed
storage only where the agreed contracts can be met; do not silently weaken guarantees.
Repository groupings remain design decisions.

## Port contracts

Document required scope, inputs, outputs, ordering, limits, supported operations,
failures, atomicity and relevant concurrency/deadline/budget behavior beside interfaces.
Validate outputs against applicable invariants; implementing a trait does not prove
its results valid. Distinguish malformed responses from valid results reporting failed
assessment. Unavailable evidence or uncertain completion must not become empty success.
Adapters translate external failures into contract failures; no silent provider fallback.

Replacing a provider under an existing contract must not require provider-specific
application logic. New behavior can justify changing a contract through an explicit
decision, following the architecture change rules below.

## Verification

Current [CI](.github/workflows/ci.yml) runs formatting, Clippy, tests, Rustdoc and Git
checks. It does not explicitly enforce architecture. Required architecture checks must
reject deliberate violations and accept permitted designs. Check production dependencies
across supported features/targets, permitting
reviewed test-only integration dependencies. Reject production app-to-ext dependencies,
SDK types in inner APIs and domain port calls, including aliases/re-exports. Review
indirect effects: Rust and text searches do not prove purity. Port labels aid inspection.

Test each responsibility where it is owned:

| Scope | Evidence |
| --- | --- |
| Domain | Business rules and their edge cases, using explicit inputs. |
| Application | Workflow decisions, error handling and shared limits across nested/parallel calls. Exercise invalid capability output and uncertain completion through port implementations outside production crates. |
| Adapter | Translation and actual backend/provider guarantees. Run the shared contract suite against each implementation; mocks alone cannot prove SQL guarantees. |
| Driving adapter | Parsing, input precedence, invocation, output and exit status. Substitute application behavior where needed; a narrow inbound interface can justify that isolation. |

Keep detailed rule edge cases at their owning layer. For example, CLI tests inject
an application rejection to test its presentation instead of rerunning eligibility
cases through the CLI. Reusing an input is justified when assertions cover different
obligations. Retain a few integration/end-to-end tests for wiring; test public behavior,
not incidental private layout. SQL contract tests verify equivalent policy, ordering
and transactions. A composed selector and optimized SQL queries are permitted designs.

Review glossary use, interface cohesion and invariant ownership. Report unavailable
prerequisites without substituting backends. Never silently suppress failures or
broaden allowlists to accommodate a violation.

## Changing architecture

Architectural decisions may evolve through explicit user approval, including approval
already given in the task. Propose the reason, affected guarantees and consequences
before changing agreed boundaries or contract guarantees. Until approved, current rules
remain binding. Update affected guidance, contracts and checks together; an improvement
claim does not authorize an agent's own exception. Routine implementation choices within
the rules need no further approval or new document.

## Design references

The [request example](docs/plan/story-generation-request.yml) explores a possible
structure; it neither specifies CLI input formats nor finalizes product behavior.
[Durability](docs/plan/durable-execution.md) and [replay](docs/plan/replay.md) remain
exploratory proposals, not binding contracts. Adopt their details only by explicit
decision. Preserve these files as design history; existing-code references do not
require retaining that implementation. The SQL schema's retained status is stated above.
