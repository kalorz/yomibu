# Analyze command contract fixtures

Original synthetic command inputs, authored for the separate offline CLI milestone
on 2026-10-04 and reusable under CC0-1.0. They contain no learner data or research
reference labels. These are engineering examples, not independently reviewed
linguistic ground truth or additions to the frozen A1 benchmark.

- `nominal.json`: a bounded nominal Pass with an explicit vocabulary tuple and rule.
- `unlisted.json`: the same sentence without word permission; completed Fail.
- `object.json`: an ordinary object sentence; completed Inconclusive despite a
  transitive-use binding because the object/predicate combination is unassessed.

The JSON uses the ordinary command contract; reuse metadata belongs here and is
not required in a user's input. The separate
[research packets](../../../docs/history/analysis/a1-fixtures/README.md) are archived.
