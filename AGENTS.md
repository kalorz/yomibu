# Repository guide

## Architecture

Read [ARCHITECTURE.md](ARCHITECTURE.md) before changing architecture or implementing
features. It defines enduring boundaries and accepted practices; existing code
is not proof of compliance. Follow the task's scope: architecture documentation
does not authorize deleting or rebuilding code. Preserve all `docs/plan/` files
and the planned SQL schema during code removal. Preserving plans does not approve
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

- Identify the affected use case, boundaries, contracts and approved requirements;
  distinguish these from examples and unfinished proposals.
  A brief note is enough; routine work needs no new design document.
- Identify verification that would reject the likely boundary violation or incorrect
  behavior. Distinguish existing checks from proposed checks.
- Resolve ordinary implementation choices within the rules. For changes to agreed
  boundaries or contracts, follow [architecture change rules](ARCHITECTURE.md#changing-architecture).
  Never silently alter rules or checks to accommodate a violating implementation.

## Delivery

- Review new abstractions against their required boundary, behavior or invariant.
  Follow the [simplicity guidance](ARCHITECTURE.md#simplicity). Preserve compatibility
  only when requested; do not introduce speculative registries or migration layers.
- Test changed responsibilities at their [owning layer](ARCHITECTURE.md#verification),
  rather than repeating dependency edge cases through callers. Keep tests reproducible,
  isolated and free of real credentials; leave the shared process environment unchanged.
  Never silently substitute another backend. Rerun affected checks after final edits.
- Use [CI](.github/workflows/ci.yml) for existing completion commands. Run focused
  checks during development and workspace checks when shared contracts change.
  For documentation-only work, check links, consistency and whitespace.
- Report actual results, unverified guarantees and boundary changes. Passing CI is
  not proof of purity. Follow the [verification requirements](ARCHITECTURE.md#verification)
  for architecture checks; report missing prerequisites.

## Documentation

Keep rules and examples in `ARCHITECTURE.md`, domain definitions in `GLOSSARY.md`,
and behavior guarantees beside interfaces. Link instead of duplicating. Maintain
CLI help in code and README as a quickstart. Write plain English; add detail to
resolve concrete ambiguity, not to inventory code or repeat generic tutorials.
