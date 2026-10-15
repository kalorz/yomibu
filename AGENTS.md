# Repository guide

## Architecture

Read [ARCHITECTURE.md](ARCHITECTURE.md) before changing architecture or implementing
features. It defines the target boundaries and accepted practices; existing code
is not proof of compliance. Follow the task's scope: architecture documentation
does not authorize deleting or rebuilding code. Preserve all `docs/plan/` files
and the planned SQL schema during the restart.

### Dependencies and composition

Use the [ownership and dependency rules](ARCHITECTURE.md#boundaries) and
[composition rules](ARCHITECTURE.md#composition-and-request-execution).
The application owns workflows; domain code computes decisions without invoking
ports; extensions implement supplied capabilities and infrastructure. Required
ports exist from their first implementation. Do not defer them until a second
adapter or weaken them for fewer lines, types or apparent duplication.

## Terminology

Read [GLOSSARY.md](GLOSSARY.md) before naming domain concepts. Use its names in code,
documentation and explanations to the user. Use collection, subject and entry with
their distinct meanings; do not introduce catalogue as a synonym. Older wording
in preserved plans is not permission to invent alternative names. Explain new
concepts and add their definitions to the glossary with the change. Flag conflicts rather
than silently changing their meaning.

## Before editing

- Identify the affected use case, boundaries, contracts and relevant plan requirements.
  A brief note is enough; routine work needs no new design document.
- Identify verification that would reject the likely boundary violation or incorrect
  behavior. Distinguish existing checks from proposed checks.
- Resolve ordinary implementation choices within the rules. Flag conflicts before
  dependent work; never silently change a rule, exemption, assertion or allowlist
  to accommodate a violating implementation.

## Delivery

- Review new abstractions against their required boundary, behavior or invariant.
  Follow the [simplicity guidance](ARCHITECTURE.md#simplicity). Preserve compatibility
  only when requested; do not introduce speculative registries or migration layers.
- Test changed domain rules, application workflows through ports and actual backend
  guarantees. Exercise invalid capability output. Keep tests reproducible, isolated
  and free of real credentials; leave the shared process environment unchanged.
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
