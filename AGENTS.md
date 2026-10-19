# Repository guide

## Architecture

Read [ARCHITECTURE.md](ARCHITECTURE.md) before changing architecture or implementing
features. It defines enduring boundaries and accepted practices; existing code
is not proof of compliance. Follow the task's scope: architecture documentation
does not authorize deleting or rebuilding code. Preserve all `docs/plan/` files;
the planned SQL schema remains a requirement. Preserving plans does not approve
their proposals; use approved requirements and the architecture's stated document statuses.

Follow the [schema-change policy](ARCHITECTURE.md#schema-changes).

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
Document legitimate differences between model contexts; resolve conflicts explicitly.

Qualify external domain terms with their provider or system when ambiguity is possible.
A namespace counts as qualification. Translate external concepts into Yomibu's agreed
terminology at the integration boundary. For example, use **WaniKani assignment** in
prose, `wanikani::Assignment` or `WaniKaniAssignment` in integration code, and
**learner progress** after translation. Avoid redundant prefixes inside clear provider
namespaces. Preserve original API field names and captured payloads.

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
  boundaries or behavioral guarantees, follow [architecture change rules](ARCHITECTURE.md#changing-architecture).
  Never silently alter rules or checks to accommodate a violating implementation.

## Plans and design discussions

When asked to start with an interview, first read current main, the authoritative
ticket/epic and retained requirements. Separate settled decisions, new ticket
assumptions and unapproved plan choices. Ask one consequential question at a time;
do not ask the user to choose routine Rust, SQL or library details, or invent
questions to satisfy a process. Summarize the outcome and acceptance checks for review.
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
Label provider or workflow examples with their scope so they do not imply restrictions
on shared model concepts.
