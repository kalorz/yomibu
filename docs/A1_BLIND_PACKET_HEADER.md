# A1 common binding sheet for manual blind review

Version 0.3, 2026-10-03. Send this sheet, the copyable review prompt, and a batch
of case JSON to the external reviewer. Do not send implementation notes, tests,
private reference records, or analyzer output. Keep the same scope for every batch.

Material is original synthetic evaluation input, Codex-assisted authoring for
Yomibu on 2026-10-03, reusable under CC0-1.0. It contains no learner material.
Explain in English and preserve all Japanese text. The case JSON supplies no
context beyond its sentence and permission bindings. Permission senses are
requested lexical uses, not evidence that those uses fit the sentence.

Every case requires five checks: Vocabulary, Inflection, Particles, Nominal,
and Scope. The last must account for all original text and every limitation.
A checked, absent feature can Pass within a recognized construction; unknown
applicability is Inconclusive, and an omitted check is NotRun. A supported
permission violation may establish Fail while other checks remain unresolved.
No judgment means an accepted exercise, mastery, or general Japanese validity.

## Exact bounded constructions

- Maximum 100 Unicode scalar values, counting whitespace/punctuation. Original
  UTF-8 bytes stay unchanged; locations are zero-based, half-open byte spans.
- One clause: optional nominal topic followed by a nominal predicate, or an
  optional nominal topic and optional direct object followed by one regular
  godan/ichidan polite verb predicate. Each nominal position is one whole lexical
  unit, potentially a compound. No automatic component-to-whole permission.
- Nominal predicate: affirmative non-past noun + です. Adjective predicates and
  nominal past/negative are outside scope. The verb endings are ます, ました,
  ません, ませんでした. Verb class needs lexical evidence; spelling alone is
  insufficient. Plain forms, irregulars, honorifics, extra auxiliaries, and other
  constructions are outside scope.
- At most one topic は and one direct-object を in that order. This does not
  resolve contrast/discourse roles or general object/verb compatibility. Route,
  departure, and other を uses remain unresolved. The whole predicate must fit
  the bounded construction before a particle permission can be judged.
- An optional final 。 is allowed. Other punctuation, whitespace within/around
  the construction, questions, extra clauses, and extra modifiers are outside
  this operational boundary. Preserve them and report the limitation.
- General naturalness, multiword expressions, and contextual reading/sense
  selection are unassessed. Any relevant outcome-changing alternatives must
  remain visible; morphology or model agreement cannot resolve them.

## Input field meanings

`bindings.vocabulary` contains exact `written_form`, katakana `reading`, and
associated `sense` permissions. No normalization, implicit spelling equivalence,
substring matching, or kanji-based authorization. Identical duplicates do not
create a new use; competing tuples require review. A recognized whole word
absent from the permissions is a constraint violation. An unknown identity or
unresolved reading/sense match is Inconclusive, not automatically prohibited.
Regular inflected verbs refer to their dictionary base and base reading.

Verify that a supplied sense is attested for the identified whole lexeme and
base reading; do not claim that it is selected by the sentence's context. Ordinary
polysemy alone does not prevent this bounded check from passing. Preserve other
senses as unassessed alternatives. Competing supplied identities, unresolved
readings/boundaries, or lexical/POS alternatives affecting a required check remain
Inconclusive. Dictionary entry grouping is not the criterion. A case requiring
contextual meaning or idiom resolution remains unsupported; structural evidence
cannot settle it. Unknown noun/adjective applicability makes Nominal and Scope
Inconclusive, even if both uses share a surface ending.

`direct_object: true` is an explicit claim that this particular lexical use is
transitive for the scoped construction. **Verify that claim against sources.**
It is not proof supplied by the sentence's を, a guarantee of semantic fit, or
permission for every を use. False means there is no such positive binding;
it does not assert that the verb is intransitive.

The `grammar` list preserves arbitrary descriptions in order. IDs are one-based
positions, local to this input. Never parse the wording to infer permission.
`bindings.grammar` explicitly attaches each permitted rule to a declaration ID:

| Rule name | Permission |
| --- | --- |
| NominalDesu | Affirmative non-past nominal です |
| TopicWa | The scoped simple nominal topic は |
| ObjectWo | The scoped direct object を with supported transitive use |
| PoliteNonPast | Regular verb + ます |
| PolitePast | Regular verb + ました |
| PoliteNegativeNonPast | Regular verb + ません |
| PoliteNegativePast | Regular verb + ませんでした |

## Neutral source index

These are starting points, not per-case answers or proof of the supplied lexical
claims. Inspect relevant entries/passages and give precise locators. No dictionary
runtime output has reference authority. Do not copy source exercises.

- TUFS [nominal predication, 001](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/explanation/001.html), points 1–2.
- TUFS [topic, 002](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/explanation/002.html), points 1–2.
- TUFS [polite non-past, 023](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/explanation/023.html), points 1–3 and table.
- TUFS [polite past/negative past, 025](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/explanation/025.html), points 1–3.
- TUFS [verb groups, 039](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/explanation/039.html), points 2–5; compare the [card table](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/card/039.html) where a romanized table entry differs.
- TUFS [direct object, 027](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/explanation/027.html), points 1–2.
- TUFS [broader particle uses, 053](https://www.coelang.tufs.ac.jp/mt/ja/gmod/contents/explanation/053.html), section II, points 1–4.
- EDRDG [JMdict/EDICT project](https://www.edrdg.org/wiki/JMdict-EDICT_Dictionary_Project.html), format, lexical restrictions, and inflections. Locate actual entries for lexical evidence; documentation alone cannot verify a case.
- [Kotobank](https://kotobank.jp/), named publisher dictionary sections such as Shogakukan's Digital Daijisen and Seisenban Nihon Kokugo Daijiten. Cite the particular dictionary, headword, POS, and sense; do not treat all entries on an aggregate page as one dictionary or a search snippet as inspected evidence.
