# Architecture

These are Yomibu's standing rules. Existing code is not proof of compliance, and
examples add no requirements. [AGENTS.md](AGENTS.md) says how work is done,
[GLOSSARY.md](GLOSSARY.md) what terms mean, and [README.md](README.md) how to run things.

## Design

Two practices govern the design:

| Practice | Meaning here |
| --- | --- |
| Hexagonal architecture | Every external interaction goes through a port trait owned by core, implemented in the adapters crate and called from app. Dependencies point inward. |
| Functional core, imperative shell | Domain decisions are pure functions of explicit inputs. Effects, time, randomness and configuration live in app, adapters, compose and the driving crate. |

Two practices support them. Model valid states with types: parse input once at the
boundary into a type that cannot hold the invalid case, usually a Rust newtype ("parse,
don't validate"). Design by contract, in its light form: a port's guarantees live in
its types and in contract tests that every adapter passes; a doc comment adds a line
or two only for what those cannot say. SOLID, Clean Architecture, Clean Code and
tactical DDD follow from these four; do not add traits, layers or wrappers to
demonstrate them. Bounded contexts and CQRS have [triggers](#triggers).

### Domain language

Use [glossary](GLOSSARY.md) terms within Yomibu's model. Qualify external terms with
their provider, as in **WaniKani assignment** or `wanikani::Assignment`, and translate
them into Yomibu terms at the adapter, which is the anti-corruption layer: provider
structures never shape the domain model. Preserve original field names in captured payloads.
Agree new or changed terms with the user first, following [AGENTS.md](AGENTS.md#terminology).

## Boundaries

```text
yomibu-app      -> yomibu-core
yomibu-adapters -> yomibu-core
yomibu-compose  -> yomibu-core + yomibu-app + yomibu-adapters
yomibu-cli      -> yomibu-compose + yomibu-app + yomibu-core
```

| Location | Owns |
| --- | --- |
| `crates/yomibu-core/src/domain/` | Domain values and pure decisions, one module per concept. |
| `crates/yomibu-core/src/ports/infrastructure/` | Outbound traits for storing, retrieving and transmitting: storage, cache, files. |
| `crates/yomibu-core/src/ports/capabilities/` | Outbound traits for replaceable behaviour that computes, decides or generates (the strategy pattern): selection, generation, assessment. |
| `crates/yomibu-app/src/` | Operations: validate input through core rules, run steps that call ports and components, apply core decisions, combine outcomes. |
| `crates/yomibu-adapters/src/<provider>/` | Port implementations grouped by provider or backend: SQL, SDK clients, file formats, error translation. |
| `crates/yomibu-compose/src/` | The composition root shared by driving crates. Builds infrastructure once from startup configuration, and components per composition from the default TOML or a request's own. Holds the component registries. |
| `crates/yomibu-cli/src/` | Argument parsing and output. Help and error messages guide the user to the next step: the CLI is its own documentation, and README names only how to start. A future web or MCP crate takes the same position. |

Placement rules:

- Infrastructure or capability is a property of the port: storing, retrieving or
  transmitting is infrastructure; computing, deciding or generating is a capability.
  One adapter type may implement ports of both roles.
- A domain decision with one pure implementation is a function in core, not a port. Make
  a capability port when the behaviour needs an external service, or when the
  composition must be able to choose between implementations. A pure component
  implementing a capability lives in adapters like any other.
- Components receive their dependencies, ports and shared provider clients, from the
  composition root and never construct them.
- A component is a named implementation of one capability with a typed options struct.
  It exposes a name and a one-line description for listing, nothing more. A type that
  implements two capability traits is a component in each of them, listed separately in
  the composition. Grouping by provider is directory layout in adapters, not a unit in
  the composition.
- A composition is, per capability, the ordered list of components an operation uses;
  leaving a component out of the list is the only way to disable it. Options come from
  configuration, never from derived CLI flags or environment variables. Provider
  credentials and endpoints are startup configuration; a request chooses among
  configured providers.
- Operations run their steps in a fixed order. The composition decides which components
  fill each capability and in what order within it; it does not reorder steps. Steps are
  code, never configuration interpreted by a generic engine.
- Core and app contracts expose Yomibu types only: no SDK, database or framework types,
  and no provider-specific identifiers or restrictions, even under neutral names.
- Port modules contain only traits and the value and error types they exchange, never
  functions. Each layer enforces its own contract; when app and an adapter check the
  same condition, they are keeping different promises, not duplicating knowledge.
- Keep visibility as narrow as the current consumer needs.

## Rules

| Area | Obligation |
| --- | --- |
| Dependencies | Follow the graph. App may use adapters only as a dev-dependency for integration tests. |
| Functional core | Domain is deterministic for the same inputs: no I/O, clock, randomness, environment or ports. Pass observations in. |
| Ports | Every required I/O and every replaceable capability has a port from its first implementation. Interfaces describe behaviour; settings are typed data. |
| Application | Operations are app's only public surface; CLI, web and MCP call the same ones. An operation receives its ports and components at construction and inputs per call. A promoted step ([trigger](#triggers)) is called by the composing operation, never copied. Expected failures are results, never panics. Partial results must never look like success. |
| Adapters | Translate external representations and failures into the port's contract. No silent fallback to another provider. Never expose credentials or raw provider messages. |
| Storage | Choose contracts by consumer need, not by backend structure. Every storage backend, PostgreSQL, in-memory or file, implements the same ports and passes the same contract tests. Integrity one backend enforces, such as foreign keys, does not extend across backends; mixing them for related data is a decision, not a default. PostgreSQL schema changes update the bootstrap file and recreate the database; no migrations until the user changes this policy. |
| Simplicity | The clearest design that meets the required boundaries and behaviour: explicit names, obvious control flow, the fewest concepts. Add nothing for a need that does not exist yet; every abstraction serves a boundary, behaviour or invariant that exists now. Keep one definition of each piece of knowledge; similar-looking code is not the same knowledge, and shared code is extracted on the third occurrence, not the second. |

## Triggers

These are not day-one requirements. Each becomes required when its trigger occurs.
Propose it then, naming the trigger, and wait for the user's decision.

| Deferred guarantee | Trigger |
| --- | --- |
| Deadlines, cancellation and I/O bounds on ports | The first operation inside a long-lived process such as web or MCP, or one that calls a network service repeatedly. |
| Distinguishing confirmed, rejected and uncertain write completion | The first write whose accidental duplicate harms the user. |
| Retry policy | The first provider whose failures are transient by contract. A write with uncertain completion is never retried automatically. |
| Operation-wide failure policy, continue or fail fast | The first operation with an optional capability. |
| Separate read and write contracts (CQRS) | The first case where readers and writers need different guarantees or shapes. |
| A second bounded context | A glossary term that needs two definitions. It becomes a second module tree with translation at the seam, not a new crate. |
| Splitting adapters into crates | A second binary needing a different adapter subset, or a dependency too heavy for every build. Try Cargo features first. |
| Secret storage such as the macOS keychain | The first credential you do not want in the configuration file. Compose reads it; operations never see credentials. |
| A second CI platform | The first platform-specific code. |
| A component that nests other components | A real case the ordered list per capability cannot express. |
| A step as its own operation | A driving crate needs the step alone, or a second operation would repeat it. |

## Naming and dependencies

Name things for what they are now: `learner_id` is an identifier, `collection_ids` a
list, `list_collection_ids` an operation that returns only IDs. Use the idiomatic
spelling in each context. CLI commands are `yomibu <noun> <verb>` with singular nouns,
as in `yomibu collection list`; the verb is required, except that `yomibu story` alone
runs the main operation. Reuse established libraries for standard representations and
protocols; write Yomibu's own rules by hand. Each new dependency gets a sentence in the PR.

## Verification

Automated checks, run by [CI](.github/workflows/ci.yml) and locally per the
[README](README.md#architecture-checks):

| Check | Enforces |
| --- | --- |
| `tools/check_architecture.py` | The crate graph, including renamed, optional and platform dependencies. |
| Bylaw through `tools/check_domain.py` | Domain depends only on domain and reviewed libraries; core and app use only inner crates and the reviewed library list in `bylaw.toml`. |
| Clippy disallow list in `clippy.toml`, switched on by `forbid` in core and app | Core and app have no ambient time, randomness, environment, filesystem, network or process access. |
| `tools/provider_placement.yml` with ast-grep | No provider names in core or app identifiers. |
| `cargo machete`, `unreachable_pub`, warnings as errors | No unused dependencies or unreachable public items. |
| `tools/check_pr.py` on pull requests | Reports lines, public items and guide words changed, warns above a soft limit, and fails when AGENTS.md or ARCHITECTURE.md change together with crate code. |

Review still decides whether a port is justified, a name matches the glossary and an
abstraction serves a current need. Test each responsibility where it is owned: domain
rules with explicit inputs, operations with test-supplied ports, adapters against the
real backend, and the CLI for parsing and output with substituted operations. When a
prerequisite such as the database is missing, fail the check; never substitute a backend.

## Changing architecture

Only the user changes a rule, and this guide and the checks are updated together. An
improvement claim is not an exception. Within the rules, signatures, async mechanics,
drivers and internal wiring are the agent's choices.

## Design references

The [SQL schema](docs/plan/schema.postgresql.sql) is a retained requirement. The other
files in `docs/plan/` are proposals and history; adopt their details only by explicit
decision.
