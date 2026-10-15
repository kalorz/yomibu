# Architecture

This is the target architecture for Yomibu's restart and subsequent implementation.
It governs new design; it does not claim current source code complies or authorize
implementation during a planning task. Architecture checks below remain proposed.

Preserve all [plans](docs/plan). The [SQL schema](docs/plan/schema.postgresql.sql)
defines the retained data model; the [story request](docs/plan/story-generation-request.yml)
defines intended request behavior, including its explicitly provisional fields.
Old plans' references to existing runners are not requirements to preserve that
implementation. Use [GLOSSARY.md](GLOSSARY.md) for domain names. Resolve conflicts
explicitly; do not invent answers to open product questions.

## Design practices

Hexagonal architecture and functional core/imperative shell govern the design.
Apply the supporting practices to these concrete responsibilities:

| Practice | Application in Yomibu |
| --- | --- |
| Hexagonal architecture | The application calls selector/generator ports; PostgreSQL and provider implementations remain outside it. |
| Functional core, imperative shell | Core calculates eligibility, target constraints and assessment aggregation. Application/capability implementations obtain observations and execute effects. |
| Strategic DDD | Keep collection, subject and entry meanings consistent across import and generation. Identify model ownership without equating bounded contexts with crates or repositories. |
| Tactical DDD | Express actual consistency rules, such as publishing an entry and its matching detail together. Foreign-key connections alone do not define aggregates. |
| SOLID | Give the selector only needed query/embedding interfaces. Replacement adapters preserve behavior, not just signatures. One extension may implement several cohesive ports. |
| Clean Architecture | CLI, web and MCP call the same application use case with transport-independent input; SDK types stay in adapters. |
| Clean Code | Use glossary names and readable workflow steps; tiny forwarding functions or generic factories need a concrete purpose. |
| OOP | Use structs to encapsulate valid state and traits for replaceable behavior. Pure eligibility calculations can be ordinary functions. |
| Design by Contract | Specify scope, ordering, limits, failure and atomicity guarantees; verify them through application and real-backend contract tests. |
| Model valid states with types | Distinguish subject IDs from entry IDs and unresolved selection requests from resolved queries. Keep failed, inconclusive and errored assessments distinct. External facts still require runtime checks. |
| CQRS (Command Query Responsibility Segregation) | Separate read/change contracts and models where consumers or guarantees differ; they may share one adapter and schema. See [storage](#storage-and-cqrs). |

## Boundaries

Permitted production dependencies between workspace crates:

```text
yomibu-app -> yomibu-core
yomibu-ext -> yomibu-core
yomibu-cli -> yomibu-app + yomibu-ext (+ yomibu-core)
```

Future `yomibu-web` and `yomibu-mcp` have the CLI's position. Retain these crate
boundaries; create directories only for actual responsibilities. Domain folders
organize domain rules; a workflow phase does not automatically require a domain
module. Purity alone does not place a helper in domain; it must express domain meaning.
Pure library dependencies are allowed. Integration-test dependencies are
reviewed separately; they must not leak into production dependencies or public APIs.

Paths below are relative to `crates/`:

| Location | Ownership |
| --- | --- |
| `yomibu-core/src/domain/` | Domain values and pure decisions: eligibility, structure validation, target constraints, score aggregation and deterministic ties. |
| `yomibu-core/src/ports/infrastructure/` | Outbound service contracts: learning-data queries/writes, secrets and cache. Storage queries remain infrastructure even when called `select`. |
| `yomibu-core/src/ports/capabilities/` | Outbound operation contracts: selection, importing, embedding, preparation, generation, assessment and repair, including pure replaceable implementations. |
| `yomibu-app/src/application/` | Inbound APIs, normalization through core rules, whole-workflow execution, request budgets and failure handling. |
| `yomibu-ext/src/` | Implementations grouped by provider or component; SDK clients, SQL, file formats, provider options and error translation. |
| Driving crates | Parse transport input, invoke application services and present results. Their composition roots configure and connect implementations. |

Direction describes calls relative to the application/domain boundary:

```text
CLI -> application.generate_story(request) : inbound / driving
application -> selector.select(request)   : outbound / driven
```

These are not Cargo dependency arrows. Results returning do not reverse port roles.
Inbound APIs may be public application methods; no trait that merely repeats those
methods is required. Infrastructure/capabilities classify outbound responsibilities,
not lifetimes: a shared embedder remains a capability; a transaction may be per-request.
There is no required `ports/resources` category. Keep shared SDK clients inside
extensions; expose shared embedding behavior through its capability port.

## Mandatory rules for the restart

| Area | Obligation |
| --- | --- |
| Dependencies | Follow the graph. Core/app public contracts expose no SDK clients/responses, database connections/rows or transport-framework types. |
| Functional core | Domain decisions are deterministic for the same explicit inputs. Domain must not depend on ports/adapters, perform I/O or obtain ambient time, randomness or environment values. Supply observations and seeds explicitly. |
| Ports | Introduce required infrastructure/capability interfaces with their first implementation, including pure capabilities. Ordinary helpers and configuration values do not each require a port. |
| Application | Execute workflows through supplied ports and apply core decisions. Keep external data/service access behind ports; runtime scheduling primitives may live in the shell. |
| Adapters | Translate external representations and honor contracts. Backend optimizations must preserve query semantics. Expected failures are results, not panics. |
| Simplicity | Every extra abstraction serves a required boundary, behavior or invariant. A single implementation does not weaken that justification. |
| Terminology | Follow the glossary in code, documentation and conversation. Explain and define new concepts; do not rename established ones silently. |

A domain object invoking `generator.generate(...)` violates purity even through a
core trait. Calling a pure selector port from domain code also violates the module
boundary: invoke the capability outside domain, then pass its returned data to pure
functions. Indirect calls and effectful callbacks cannot bypass the rule; local mutation is allowed.
Opaque captured provider payloads may be retained for replay as data; domain decisions
must not interpret SDK representations. Core defines policy even when schema comments
say the "application" enforces it: application services coordinate enforcement.

### Simplicity

| Principle | Interpretation |
| --- | --- |
| KISS | Choose the clearest design satisfying required boundaries and behavior. |
| DRY | Keep business rules and authoritative definitions consistent. Similar adapter mapping code with different responsibilities need not share a framework. |
| YAGNI | Defer speculative features and machinery. Required ports, invariants and their verification are present needs. |

Mandatory architecture and behavior outrank fewer lines/types or removing apparent
duplication. A private function extracting story topics needs no trait; a configured
preparation capability does, even if its implementation calls pure helper functions.
Do not create a generic registry, dispatcher or plugin loader without a current need.

## Composition and request execution

The composition root selects available implementations and resolves environment or
secret-store configuration. It may construct OpenAI and Anthropic clients together.
The application selects supplied capability identities per request, including both
for parallel generation. Looking up a configured identity is allowed; constructing
an OpenAI SDK client or branching on provider-specific response fields in app is not.
Startup construction is an example; scoped or deferred construction may be delegated
through supplied interfaces without introducing provider dependencies into app.

Capabilities may use other injected outbound ports. A selector receives an embedder
and repository; it does not construct either. One extension/type may implement several
ports and share a client or pool. Keep request state separate from shared clients.
Provider-independent enforcement around supplied capabilities belongs in app;
provider protocol handling belongs in ext.

The application owns request-wide budgets, deadlines, cancellation and recovery.
Nested calls must pass through that enforcement before dispatch, including retries
and parallel calls; counting them only afterward is insufficient. A selector must
not bypass it with a private SDK client. Propagate cancellation/deadlines, bound
adapter operations, and never automatically retry an uncertain external outcome.
Keep credentials out of requests, reports and replay artifacts. Follow the separate
[durability](docs/plan/durable-execution.md) and [replay](docs/plan/replay.md) contracts
when implementing those features.

## Selection and generation

For the [planned request](docs/plan/story-generation-request.yml):

1. CLI reads YAML and translates it into typed application input. Application/core
   handle shared defaults and semantic validation so web and MCP behave consistently.
2. Application establishes authorized learner/source scope and execution limits,
   then calls the selector. Generation reads imported data; it does not implicitly import.
3. Selector resolves requested evidence, calls the supplied embedder for story topics
   when needed, and constructs resolved query data. Match the configured embedding
   profile/input format/dimensions/metric; equal dimensions alone do not imply compatibility.
4. Repository receives values, not SDK clients or effectful callbacks. It executes
   filters, joins, ranking and limits, returning ordered material and required evidence.
   It never calls an embedder or silently generates missing entry vectors.
5. Selector returns to application. Application validates applicable result invariants
   with core functions, then passes the shared selection and outline to each configured
   preparer. Preparation returns generation input; application invokes the corresponding
   generator, pools candidates, and coordinates assessment and optional repair.

Core owns eligibility policy. Storage may execute equivalent predicates efficiently;
contract tests must verify equivalence. Do not require loading all collections into
memory merely to evaluate filters in core. Preserve learner scope, hidden exclusions,
applicable AND filters, declared ordering precedence, target reservations, limits and
stable ID ties. Missing vectors sort as requested; unsupported operations fail explicitly
instead of dropping conditions. The selector must not treat selected material as the
full inventory. Preserve a coherent eligibility view for preparation and assessment;
the mechanism is still an open design choice.

Capability outputs are not assumed valid because they implement a trait. Validate
contract invariants using captured facts; reject malformed results before downstream use.
A valid candidate failing an assessment remains available with its findings for
configured repair; an assessment failure is not a malformed capability response.
Inconclusive, errored and unrun assessments remain distinct from a pass. Candidate ties
use declared run/candidate order, not completion timing. Repair produces new candidates
and requires full reassessment under the shared budget. Detailed formulas, source rules
and provisional settings remain in the YAML, not duplicated here.

## Storage and CQRS

Choose storage contracts by consumer needs, query semantics and invariants that must
hold atomically; neither one interface per table nor one aggregate per FK-connected
group is required. A narrow selection reader and an import writer may share one
PostgreSQL adapter, pool, canonical schema and existing foreign keys. Queries may
join progress, entries and embeddings and apply limits within SQL.

Read contracts return consumer-specific results without changing business state.
Commands may read and return results; keep required read/write transactions atomic.
Publish each entry with its matching detail atomically. Splitting interfaces does not
promise arbitrary backend combinations: each adapter must meet the whole contract.
Do not add fake parent rows, weaken required foreign keys or create schema variants
to accommodate a split. SQL cannot enforce a foreign key to a CSV record.

CQRS requires neither separate databases nor event sourcing, messaging or asynchronous
synchronization. An embedding-based selector remains effectful; repository reads are
also I/O and stay outside domain code. Actual repository groupings remain undecided.

## Port contracts

Document required scope, inputs, outputs, ordering, limits, supported operations,
error meanings, atomicity and relevant concurrency/deadline/budget behavior beside
each interface. Distinguish unavailable evidence, invalid output and uncertain
completion; do not disguise them as empty success or silently change providers.
Adapters translate SDK/SQL failures into contract failures the application understands.
Changing the provider for an existing contract must not require provider-specific
application logic; introducing genuinely new behavior may require a contract change.

## Verification

Current [CI](.github/workflows/ci.yml) runs formatting, Clippy, tests, Rustdoc and Git
checks. It does not explicitly enforce architecture. The following are requirements
for future checks, not commands or mechanisms already implemented:

| Concern | Required evidence / rejection example |
| --- | --- |
| Crate graph | Check production dependencies across supported features/targets; allow reviewed test-only integration dependencies. Reject a production app-to-ext dependency. |
| Domain purity | Module/API checks must reject port calls, ambient effects and representative aliases/re-exports. Review indirect calls: Rust and text searches do not prove purity. |
| Public contracts | Review signatures and exercise implementations outside production crates. Reject SDK types and invalid capability results, including a selector that drops a target. |
| Driving adapters | Test parsing, output, errors and dependency connections through public interfaces; keep semantic rules in shared application/core tests. Reject CLI-only defaults that alter the same request's meaning in web/MCP. |
| Nested effects | Application tests verify shared limits across parallel/nested calls, cancellation and uncertain outcomes. Reject a retry or embedding call bypassing the budget. |
| Storage | Real-backend contract tests cover scope, filters before limits, ordering and atomic entry/detail writes. A mock alone cannot establish SQL guarantees. |
| Terms and abstractions | Review glossary usage, consumer needs and invariant ownership. Similar-looking code or one implementation alone does not justify removing a boundary. |

Before rebuilding features, demonstrate checks rejecting deliberate violations and
accepting permitted designs, including a composed selector and optimized SQL queries.
Test architecture restrictions explicitly; behavioral tests should assert contracts,
not incidental private function layout. Report unavailable prerequisites without
substituting backends. Do not suppress failures or broaden allowlists silently.

Exact query types, repository groupings, eligibility capture, dispatch choices and
check implementations remain design work. Other open product decisions are marked
in the plans. Decide them explicitly when needed; no generic framework is presumed.
