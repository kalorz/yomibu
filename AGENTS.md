# Repository guide

## Architecture

Read [ARCHITECTURE.md](ARCHITECTURE.md) before changing architecture or implementing
features. It defines enduring boundaries and accepted practices; existing code
is not proof of compliance. Follow the task's scope: architecture documentation
does not authorize deleting or rebuilding code. Preserve all `docs/plan/` files;
the planned SQL schema remains a requirement. Preserving plans does not approve
their proposals; use approved requirements and the architecture's stated document statuses.

### Dependencies and composition

Use the [ownership and dependency rules](ARCHITECTURE.md#boundaries) and
[composition rules](ARCHITECTURE.md#composition-and-request-execution).
The application owns workflows; domain code computes decisions without invoking
ports; extensions implement supplied capabilities and infrastructure. Required
ports exist from their first implementation. Do not defer them until a second
adapter or weaken them for fewer lines, types or apparent duplication.

## Terminology

Read [GLOSSARY.md](GLOSSARY.md) before naming domain concepts. Use this shared domain
language in code, tests, documentation and explanations to the user. Use collection,
subject and entry with their distinct meanings; do not introduce catalogue as a synonym.
Older wording in preserved plans is not permission to invent alternative names.

Before adopting a new domain term or changing an existing meaning, explain it to the
user and obtain agreement. Record the agreed definition in the glossary before using
it in code, including API and configuration names. This also governs concepts used in
documentation and user explanations, not every local variable or ordinary technical word.
Earlier agreement counts; do not request it again or require a separate glossary commit.
Document legitimate differences between model contexts; resolve conflicts explicitly.

## Before editing

- Briefly identify the requested behavior, affected boundaries/contracts and simplest
  implementation approach. Implement in small, complete increments through the
  required boundaries. Distinguish requirements from examples and unfinished proposals.
  Routine work needs no new design document. If implementation outgrows the stated
  approach, explain the cause and reassess it before adding more machinery.
- Identify verification that would reject the likely boundary violation or incorrect
  behavior. Distinguish existing checks from proposed checks.
- Keep visibility as narrow as required. Public APIs must serve a current consumer
  or an approved contract. Do not expose modules or items merely to silence
  visibility or unused-code warnings.
- When adding substantial general-purpose functionality or custom tooling, follow
  the [reuse guidance](ARCHITECTURE.md#simplicity). Start with adopted dependencies;
  evaluate alternatives when a concrete gap arises, not on every task or session.
- Resolve ordinary implementation choices within the rules. For changes to agreed
  boundaries or contracts, follow [architecture change rules](ARCHITECTURE.md#changing-architecture).
  Never silently alter rules or checks to accommodate a violating implementation.

## Plans and design discussions

Discuss new or changed architectural boundaries and public contracts before
implementation. Routine implementation within approved contracts needs no additional
design approval. Previously granted approval counts.

For these discussions, show signatures, inputs, outputs, important failures, owning
layers and a minimal usage example with dependency construction. Prefer pseudocode
to prose; Ruby-style sketches are welcome. Do not invent abstractions to fill a plan.

Finish with affected files, exclusions and observable acceptance checks. Keep prose
to brief tradeoffs and open decisions. Plan only the current step; existing plans
do not authorize their full implementation.

## Delivery

- Before delivery, simplify the complete diff using
  [implementation economy](ARCHITECTURE.md#simplicity), including tests and documentation.
  Remove avoidable machinery; merely reporting it does not complete the task.
  For substantial remaining complexity, explain the current need and why a concrete
  simpler alternative is insufficient. Simplicity review findings must identify an
  alternative that preserves required behavior and boundaries; fewer lines or a single
  adapter do not justify removing a required port. Preserve compatibility only when requested.
- For new workflows or changed boundaries, trace the actual path from entry point
  through application, ports and adapters. Cite relevant code and an application test
  using supplied port implementations. New boundary violations block completion;
  passing tests or moving files into the intended directories is insufficient.
- Test changed responsibilities at their [owning layer](ARCHITECTURE.md#verification),
  rather than repeating dependency edge cases through callers. Keep tests reproducible,
  isolated and free of real credentials; leave the shared process environment unchanged.
  Never silently substitute another backend. Rerun affected checks after final edits.
- Use [CI](.github/workflows/ci.yml) for existing completion commands. Run focused
  checks during development and workspace checks when shared contracts change.
  For documentation-only work, check links, consistency and whitespace.
- Report actual results, unverified guarantees and boundary changes. For substantial
  changes, report additions and deletions separately and list new files. Passing CI
  is not proof of purity. Follow the [verification requirements](ARCHITECTURE.md#verification)
  for architecture checks; report missing prerequisites.

## Documentation

Keep rules and examples in `ARCHITECTURE.md`, domain definitions in `GLOSSARY.md`,
and behavior guarantees beside interfaces. Link instead of duplicating. Maintain
CLI help in code and README as a quickstart. Write plain English; add detail to
resolve concrete ambiguity, not to inventory code or repeat generic tutorials.
