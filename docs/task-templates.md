# Task templates

Optional prompts for task authors. Fill only what varies and omit unneeded fields.
The standing instructions in [AGENTS.md](../AGENTS.md) apply without being repeated.
Ordinary prompts remain valid; agents must not require a completed template.

## Implementation

```text
Task: <one observable behavior>
Done when: <observable acceptance conditions>
Verify: <known checks, or "choose focused checks at the owning layer">
Not in scope: <likely adjacent work>
Additional constraints: <optional>
Approach: <optional, if already agreed>
Size checkpoint: <optional: what to count and when to pause for reassessment>
```

Acceptance conditions describe behavior; verification supplies evidence. Name likely
distractions in exclusions, such as additional providers, caching or configuration
discovery. Replace vague quality goals with concrete requirements.

A size checkpoint applies only when explicitly chosen for that task. It prompts
review of the approach, not compressed code or weakened architectural boundaries.

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
