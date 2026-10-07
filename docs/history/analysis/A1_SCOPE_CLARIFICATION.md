# A1 Scope check clarification

2026-10-04. This makes the existing protocol/binding sheet v0.3 interpretation
explicit; it changes no construction, permission, threshold or runtime behavior.
Previously reviewed packets and the common binding sheet remain unchanged.

Scope checks structural coverage and applicability within the declared boundary.
It is separate from the overall result and from vocabulary/grammar permission
checks. It does not copy their Pass/Fail/Inconclusive outcomes.

- Scope Pass requires all original text to fit a supported bounded construction,
  with structural applicability established and no uncovered or unsupported text.
- A recognized, supported use without its required permission is Fail in the
  applicable Vocabulary, Inflection, Particles or Nominal check. That missing
  permission alone does not make Scope Fail or Inconclusive. Scope may Pass.
- Unsupported text or unresolved structural applicability makes Scope
  Inconclusive, even when the limitation is fully described. Merely listing all
  text/limitations is not enough for Pass. Lexical/POS or particle-role uncertainty
  affects Scope when it prevents establishing the bounded construction.
- Another check's uncertainty does not automatically determine Scope. Conversely,
  Scope Pass cannot establish vocabulary permission or contextual meaning.
  General naturalness and contextual reading/sense selection remain explicitly
  unassessed; their exclusion alone does not make every core case Inconclusive.
- Keep omitted checks as NotRun and execution/input errors separate. A supported
  permission Fail can determine the completed overall result while Scope or
  other checks remain Inconclusive. Overall Pass still requires every required
  check to Pass.

This follows the [binding sheet](A1_BLIND_PACKET_HEADER.md), especially its
required-check inventory, exact constructions and applicability rules, and the
[reference protocol](A1_REFERENCE_PROTOCOL.md). It is a project-contract
clarification, not linguistic evidence derived from analyzer output.

For pending reference reconciliation, preserve any different reviewer reading,
record this interpretation and append corrections to affected check cells with
their reasons. Preserve raw responses and original judgments. No new blind
review is needed solely for this clarification. Apply it consistently across
the reference set before freeze and the later reason/span audit.
