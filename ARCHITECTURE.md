# Architecture

This document defines Yomibu's enduring architectural rules. Existing code is not
proof of compliance. Examples illustrate the rules; they are not exhaustive designs
or commitments to exact APIs. [Verification](#verification) distinguishes implemented
checks from rules still requiring enforcement or review.

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

A domain model represents concepts, relationships and rules for part of the problem;
it can span many types and functions. Yomibu's domain model includes collections,
subjects, entries and progress rules. A Rails ActiveRecord class or a PostgreSQL table
is not the whole domain model. Request types and database schemas are representations;
mapping a row to a struct does not itself establish a DDD bounded context. Separate
representations where meanings or responsibilities differ; share values where they match.

| Area / concept | Rule and illustrative application |
| --- | --- |
| Strategic: Ubiquitous Language | Agree new terms or changed meanings with the user, then record them in [GLOSSARY.md](GLOSSARY.md) before code uses them. Follow the [vocabulary agreement rule](AGENTS.md#terminology); reuse established meanings in discussion, code and tests. |
| Strategic: Bounded Contexts | Identify where a model and its language apply. Crates, phases and tables do not automatically define contexts. Document a meaning that legitimately differs by context. |
| Strategic: relationships between domain models | Define relationships and translations between models in different bounded contexts, including external domain models. Preserve WaniKani progress semantics when translating observations; Yomibu applies its own eligibility rules. Provider structures must not dictate the domain model. |
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
| Terminology | Use glossary terms within their model context. Obtain agreement on new or changed meanings and record them before introducing them into code. |

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

Retained experimental code follows the same rules from its first feature slice.
An architectural exception requires explicit agreement on its scope and exit condition;
"experimental" or "temporary" alone is not an exemption.

Implementation economy means the fewest concepts, layers and moving parts that clearly
satisfy requested behavior and agreed architecture. Prefer direct control flow, existing
tools and ordinary Rust constructs. Remove unnecessary forwarding layers, duplicated
state or policy, speculative flexibility, redundant tests and prose that repeats code.
Each extra abstraction must serve a specific current responsibility or invariant;
required ports already meet that test. Large changes prompt review, not arbitrary line
limits, compressed formatting, displaced complexity or weakened boundaries.

A rule may have equivalent pure and SQL representations. Verify their shared meaning
through contract tests rather than forcing inefficient data loading to reuse code.

Prefer reuse over rebuilding general-purpose functionality. Start with the standard
library and dependencies already adopted by the project. Before adding substantial
generic functionality or custom tooling, evaluate established libraries/tools against
the required behavior, maintenance, testing, licensing, relevant security advisories
and integration/dependency cost. Choose for semantic fit and total maintenance burden,
not dependency count alone; popularity alone does not establish suitability or security.
Briefly explain consequential choices in the change description, including any gap
requiring custom code. Ordinary tasks reuse those decisions; do not repeat tool
research each session.

Keep custom code for Yomibu-specific rules, small straightforward helpers and concrete
unmet requirements. For example, reuse serialization, HTTP and SQL libraries while
owning learning policies. Infrastructure and replaceable capabilities still require
ports; a pure utility library does not automatically need a forwarding interface.
For architecture enforcement, let adopted tools own parsing and dependency analysis;
keep custom checks limited to documented gaps and test Yomibu's rules, not the tool's
general implementation.

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

### Failure handling

By default, continue independent work when useful results can still satisfy the
operation's contract, including explicitly permitted partial results. Each application
operation defines required outcomes, which capability roles may be skipped and how
execution continues after an error. Optionality belongs to a role in that operation,
not to an adapter such as Sudachi globally. Never weaken required constraints or treat
an omitted required check as a pass.

Application owns this policy across CLI, web and MCP. Driving adapters convey the
requested policy and present outcomes; they do not decide which guarantees to drop.
Validate configuration and dependencies relevant to the requested operation. Missing
unused integrations must not block it. An unavailable dependency makes its requested
capability unavailable with an explicit reason; fail early when an essential missing
capability makes the operation impossible.

Illustrative outcomes under the default policy:

| Situation | Outcome |
| --- | --- |
| Optional Sudachi analysis is unavailable or fails. | Skip that analysis, continue useful work and report the omission. |
| Sudachi is required for an eligibility assessment. | Do not report that assessment as passed; retain other outputs only as the operation permits. |
| Sentence analysis requires Sudachi and does not use an AI model. | Missing AI configuration is irrelevant; missing Sudachi fails this operation. |
| One of several generation runs fails. | Keep successful results if completion requirements permit, and report the failed run. |
| Generation has no usable generator. | Fail before starting work that cannot produce the requested result. |

Structured outcomes report completed, unavailable, skipped and failed work, reasons,
and whether required guarantees were established. Warnings accompany that information;
partial results must remain distinguishable from complete success.

An optional fail-fast policy (illustrative CLI option: `--fail-fast`) stops on the first
unavailable requested capability or execution error, including an otherwise optional
capability. A valid negative assessment is a business result, not an execution error.
For parallel work, stop scheduling and cancel running operations where possible;
already completed external calls cannot be undone. Both policies retain the shared
execution limits and uncertain-completion rules above.

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
checks, plus [crate dependency checks](tools/check_architecture.py). Run them locally
with `python3 -B tools/check_architecture.py`; the checker uses offline Cargo metadata
with the repository toolchain and requires Python 3, without building the application.
It checks declared workspace dependencies, including renamed, optional, build and
platform-specific edges.
The four retained crates must remain present; web/MCP are optional driving peers. New
workspace crates require explicit role classification. Reviewed app-to-ext integration
test dependencies are permitted only as dev-dependencies; other deviations fail.

The [checker tests](tools/test_architecture.py) use isolated Cargo workspaces to prove
rejection of forbidden edges and acceptance of allowed ones. Run them with
`python3 -B -m unittest discover -s tools -p 'test_*.py' -v`; CI runs both commands.

Domain and core/app dependency rules are configured in [bylaw.toml](bylaw.toml), using
[cargo-bylaw](https://github.com/danielgerlag/cargo-bylaw) 0.1.0. Install it with
`cargo install cargo-bylaw --version 0.1.0 --locked`; the separate executable keeps
its rust-analyzer dependencies out of Yomibu's dependency graph. The repository
Rust toolchain includes `rust-src`, required for semantic analysis. Run these
checks with `python3 -B tools/check_domain.py`.

Bylaw analyzes core/app libraries with all features on each CI host (Linux and macOS).
Domain may depend on domain modules and reviewed external libraries; other core
modules are forbidden. Core/app declared and actual dependencies are restricted to
the inner crates and the reviewed library list; the manifest check separately enforces
direction between inner crates. The list includes `serde_core` and `thiserror_impl`,
owners of resolved reexports, not new provider dependencies. Library-list changes need
review against the boundaries; never automatically copy dependencies from manifests.
Test-only dependencies are outside this library rule. Analysis resolves aliases,
reexports and macro expansions in active configurations. Inactive `cfg` branches
are not checked. When adding target- or feature-dependent code, include the relevant
configurations in architecture verification; all-features alone does not cover every
feature combination. Unit-test-only code also requires appropriate verification.
The manifest check above covers all declared platform dependencies between workspace crates.

The [dependency runner](tools/check_domain.py) delegates analysis to Bylaw and rejects
incomplete reports or missing/weakened rules. It applies one Bylaw 0.1.0 namespace
correction: ignore root prefixes (`crate`, `super`, `self`)
recorded for grouped imports, while still rejecting items owned by the crate root. The
[contract tests](tools/test_domain.py) check these responsibilities and a few
allowed/forbidden examples against the real analyzer; they do not recreate its parser
suite. Run the Python tests above after installing Bylaw. CI runs both the tests and
the dependency runner; `cargo test` alone does not run architecture checks.

There are no baselined violations. The old implementation and its temporary exceptions
have been removed. Keep the namespace correction only until the analyzer handles those
imports correctly; then run Bylaw directly while retaining equivalent verification.

Clippy's [core configuration](crates/yomibu-core/clippy.toml) restricts selected ambient
APIs inside [domain](crates/yomibu-core/src/domain/mod.rs). The separate
[application configuration](crates/yomibu-app/clippy.toml) restricts selected direct
filesystem, network, environment and process APIs while permitting application timing.
Domain and application enable their restrictions with `forbid`, including printing.
Cargo lint defaults leave integration fixtures usable. The
[lint tests](tools/test_domain_clippy.py) exercise allowed and forbidden examples.

Each crate enables `unreachable_pub`: it flags `pub` items that cannot be reached
from outside the crate, not unnecessary APIs that are publicly reachable.
CI treats compiler/Clippy warnings as errors, including existing unused-code and
redundancy checks, plus opt-in `redundant_clone` and `unused_async` checks. The latter
does not require removing async capability signatures; trait implementations have a
separate lint that is not enabled. These checks catch specific patterns, not unnecessary
public APIs or excessive architectural machinery. Review remains required. Do not
enable entire pedantic/restriction groups or size quotas to stand in for that judgment;
use narrow, explained lint exceptions when a required contract justifies them.

These guards are not proof of functional purity. Clippy does not detect every I/O API,
indirect effect or source of nondeterminism. Review new APIs and libraries, including
randomness sources; extend the restrictions when adopting them. Adopted tools own
parsing and resolution; keep custom code limited to demonstrated gaps.

| Obligation | Coverage and next action |
| --- | --- |
| Workspace and domain dependency direction | Implemented above, subject to the configuration limits. No legacy exceptions remain. |
| Core/app independence from SDKs, database libraries and delivery frameworks | Bylaw rejects unapproved libraries in declared and actual dependencies. Pure libraries may be reviewed and added; allowing a library does not establish purity of every API. |
| Domain avoids ambient effects | Scoped Clippy restrictions reject configured API uses, including aliases and function pointers. Review unlisted APIs, callbacks, indirect effects and nondeterminism. |
| Public contracts expose domain/application representations | The library gate catches explicit references to unapproved SDK types, not poor contract design. Review associated types, opaque payload handling and representation ownership. |
| Application uses supplied ports and applies operation-wide policy | Scoped Clippy checks reject selected direct I/O APIs. No workflows are implemented yet. When adding them, verify the actual call path, required/optional failures and shared limits at the owning operation. Application timing remains permitted. |
| Adapters satisfy behavioral contracts | No adapters are implemented yet. Define reusable contract cases beside each implemented port's guarantees and run them against its adapters; dependency analysis cannot establish ordering, atomicity or failure semantics. |
| Required ports, role labels, glossary meanings and justified abstractions | Review required. A trait or documentation label alone cannot establish the correct boundary or meaning. Do not add marker machinery or generic lint rules for subjective judgments. |

Test each responsibility where it is owned:

| Scope | Evidence |
| --- | --- |
| Domain | Business rules and their edge cases, using explicit inputs. |
| Application | Workflow decisions, required/optional failure handling and shared limits across nested/parallel calls. Exercise invalid capability output and uncertain completion through port implementations outside production crates. |
| Adapter | Translation and actual backend/provider guarantees. Run the shared contract suite against each implementation; mocks alone cannot prove SQL guarantees. |
| Driving adapter | Parsing, input precedence, invocation, output and exit status. Substitute application behavior where needed; a narrow inbound interface can justify that isolation. |

Keep detailed rule edge cases at their owning layer. For example, CLI tests inject
an application rejection to test its presentation instead of rerunning eligibility
cases through the CLI. Reusing an input is justified when assertions cover different
obligations. Retain a few integration/end-to-end tests for wiring; test public behavior,
not incidental private layout. SQL contract tests verify equivalent policy, ordering
and transactions. A composed selector and optimized SQL queries are permitted designs.

Cover failure handling and execution policies relevant to the operation's responsibilities.
For workflows with optional or composed work, verify continuation, fail-fast, dependency
failures and shared limits as applicable. A single required storage query needs
success/failure coverage, without introducing additional policy machinery.
CLI tests check policy forwarding, warnings and result presentation using supplied
application outcomes, without repeating the application's failure matrix.

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
