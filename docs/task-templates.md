# Task templates

Optional templates for issues and session briefs. Fill only what varies and omit
unneeded fields.
The standing instructions in [AGENTS.md](../AGENTS.md) apply without being repeated.
Ordinary prompts remain valid; agents must not require a completed template.

## Epics and tasks

- [Epic](../.github/ISSUE_TEMPLATE/epic.md): a user outcome delivered through several
  tasks, with shared decisions, exclusions and completion criteria.
- [Task](../.github/ISSUE_TEMPLATE/task.md): one bounded piece of work, standalone or
  part of an epic, with acceptance conditions and verification.

GitHub offers these templates when creating issues after they reach the default branch.
Their bodies also work as session briefs; omit the YAML metadata at the top.

Acceptance conditions describe behavior; verification supplies evidence. Name likely
distractions in exclusions, such as additional providers, caching or configuration
discovery. Replace vague quality goals with concrete requirements.

## Design

```text
Design only; implement nothing.
Question: <the boundary or contract to decide>
Show: minimal signatures, inputs/outputs, relevant failures and guarantees,
      owning layers, and one call example with dependency construction.
Compare: at most two meaningful options; do not invent alternatives.
Recommend: one option, its decisive tradeoff, and unresolved assumptions.
```

Use the existing [design discussion guidance](../AGENTS.md#plans-and-design-discussions)
for approval and handoff; the template creates no additional approval step.
