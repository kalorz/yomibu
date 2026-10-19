# Repository guide

## Architecture

Read [ARCHITECTURE.md](ARCHITECTURE.md) before changing architecture or implementing
features. Follow the task's scope: architecture documentation does not authorize
deleting or rebuilding code. Preserve all `docs/plan/` files. Use approved requirements
and the [design reference guidance](ARCHITECTURE.md#design-references) when interpreting
plans. Use [README.md](README.md#developing) and existing configuration for tools and
commands.

### Dependencies and composition

The rules are maintained in ARCHITECTURE.md. Follow its
[architectural obligations](ARCHITECTURE.md#architectural-rules),
[ownership and dependency rules](ARCHITECTURE.md#boundaries),
[composition rules](ARCHITECTURE.md#composition-and-request-execution) and
[schema-change policy](ARCHITECTURE.md#schema-changes).

## Terminology

Read [GLOSSARY.md](GLOSSARY.md) before naming domain concepts. Follow the
[domain language rules](ARCHITECTURE.md#domain-language), including provider translation.
Follow the [naming guidance](ARCHITECTURE.md#simplicity) for values and operations.

For each ticket, identify the affected domain terms and check them against GLOSSARY.md
and accepted ticket/epic decisions before editing. Before completion, check terminology
across the touched schema, code/contracts, tests and documentation, including the
translation of provider terms. Reuse canonical terms; raise unresolved naming or meaning
conflicts instead of introducing synonyms. Keep the check scoped to the affected work.

Before adopting a new domain term or changing an existing meaning, explain it to the
user and obtain agreement. Record the agreed definition in the glossary before using
it in code, including API and configuration names. This also governs concepts used in
documentation and user explanations, not every local variable or ordinary technical word.
Earlier agreement counts; do not request it again or require a separate glossary commit.

## Before editing

- Briefly identify the requested behavior, affected boundaries/contracts and simplest
  implementation approach. Implement in small, complete increments through the
  required boundaries. Distinguish requirements from examples and unfinished proposals.
  Routine work needs no new design document. If implementation outgrows the stated
  approach, explain the cause and reassess it before adding more machinery.
- Identify verification that would reject the likely boundary violation or incorrect
  behavior. Distinguish existing checks from proposed checks.
- When adding substantial general-purpose functionality or custom tooling, follow
  the [reuse guidance](ARCHITECTURE.md#simplicity).
- Resolve ordinary implementation choices within the rules. For changes to agreed
  boundaries or behavioral guarantees, follow [architecture change rules](ARCHITECTURE.md#changing-architecture).
  Never silently alter rules or checks to accommodate a violating implementation.

## Plans and design discussions

During design discussions, start from current repository guidance, authoritative
specifications and settled decisions. Identify new assumptions and distinguish
requirements from unapproved plan choices. Ask focused questions about unresolved
behavior, scope and tradeoffs; summarize decisions and acceptance checks for review.
Respect the requested stage: approval to record decisions does not itself authorize
implementation planning or implementation.

Agents own Rust signatures, async/sync mechanics, driver choices and internal wiring
within approved behavior, guarantees and architectural boundaries. Internal API changes
do not themselves require approval; Rust `pub` alone does not establish an external
compatibility commitment. Discuss new or changed behavior, domain meaning, architectural
boundaries or externally committed guarantees, and new deployment requirements or material
operational tradeoffs, before implementation. Previously granted approval counts.

For consequential infrastructure choices, such as the first I/O adapter or a new runtime,
obtain a focused independent technical review in the same session before presenting the
recommendation or implementing it. Resolve technical findings yourself; escalate only
decisions within the approval scope above.

For these discussions, lead with the recommendation and user-visible consequences.
Show inputs, outputs, important failures, owning layers and a minimal example with
dependency construction; use signatures or Ruby-style pseudocode where useful. Do not
invent abstractions to fill a plan.

Finish with affected files, exclusions and observable acceptance checks. Keep prose
to brief tradeoffs and open decisions. Plan only the current step; existing plans
do not authorize their full implementation. Keep working plans proportionate and link
to existing specifications and CI commands instead of copying them.

When the user changes acceptance criteria, update the authoritative issue or
specification and affected contracts before handoff. A working plan or PR summary
alone is not the record of the changed requirements. Make superseded guarantees
and obsolete plans explicit; preserving a plan does not make its choices requirements.
When the ticket is the specification, keep it authoritative rather than creating
a duplicate design document. Extract only reusable rules and agreed terminology
into the architecture guide and glossary.

Optional [task templates](docs/task-templates.md) help write implementation and design briefs.

## Ownership review

For changed inner contracts or validation, review
[provider independence](ARCHITECTURE.md#provider-independence) during design and
implementation review, before SQL, performance or plan-adherence checks. Treat the
implementation plan as fallible; its approval is not an implicit architecture exception.
For each changed inner contract or validation rule, justify restrictions against the
approved model, identify provider/backend assumptions even under generic names, and use
a concrete counterexample to test the boundary at its owning layer. Cite blockers by
file, line and violated rule.

## Delivery

- After implementation and review fixes, make one final simplification pass over
  the complete diff using [implementation economy](ARCHITECTURE.md#simplicity),
  including tests and documentation. Use behavior-preserving refactoring to remove
  avoidable machinery. Preserve observable behavior, required ports, contract guarantees
  and meaningful verification, including failures and execution limits. Make no changes
  when the result is already clear and economical. If refactoring materially changes
  how correctness is established, have the affected parts reviewed again.
  For substantial remaining complexity, explain the current need and why a concrete
  simpler alternative is insufficient.
- **Leave touched code better.** You may make small, behavior-preserving refactorings
  in touched code and its immediate collaborators without separate approval when they
  simplify the implementation, remove duplicated knowledge, or improve compliance with
  agreed architecture. Keep cleanup proportionate to the task and verify affected
  behavior. Preserve required ports and contract guarantees. Do not expand into
  unrelated rewrites, speculative abstractions, or cosmetic churn. Propose broader
  improvements separately. Briefly identify incidental refactoring and its benefit in
  the delivery report; cleanup is not mandatory when no useful improvement is evident.
- Give reviewers the approved scope and exclusions; distinguish requirements from
  revisable implementation choices. Required findings must identify a concrete defect
  or violated contract or rule; distinguish optional improvements. Challenge complexity
  caused by implementation choices using a simpler viable alternative that preserves
  required behavior and boundaries. Verify uncertain library assumptions from documentation
  or a temporary probe. Review feedback does not itself authorize scope or guarantee changes.
- Apply accepted review feedback across the affected path: inspect related parameters,
  callers, tests and documentation for the same issue. Handle routine internal corrections
  together and propose consequential public-interface changes as a group under the
  existing approval rules. Keep this pass within the task and its immediate collaborators.
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
  For changes limited to documentation, check links, consistency and whitespace;
  local architecture checks are not required. Otherwise, run the
  [local architecture checks](README.md#architecture-checks) before committing.
- Report actual results, unverified guarantees and boundary changes. For substantial
  changes, report additions and deletions separately and list new files. Passing CI
  is not proof of purity. Follow the [verification requirements](ARCHITECTURE.md#verification)
  for architecture checks; report missing prerequisites.

## Documentation

Keep agent workflow and delivery requirements here, architectural rules, rationale,
examples and coverage limits in [ARCHITECTURE.md](ARCHITECTURE.md), and agreed domain
definitions in [GLOSSARY.md](GLOSSARY.md). Keep behavior guarantees beside interfaces,
CLI help in code, and tool usage and configuration in [README.md](README.md) and
existing configuration. README remains a quickstart. Link instead of duplicating.
Write plain English; add detail to resolve concrete ambiguity, not to inventory code
or repeat generic tutorials.
Label provider or workflow examples with their scope so they do not imply restrictions
on shared model concepts.
