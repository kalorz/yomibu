# Repository engineering rules

- Use the [task map](ARCHITECTURE.md#current-module-map) to find code and tests.
  Read those before changing behavior, then the relevant [SPEC](SPEC.md) section.
  Use [README](README.md) for setup, [PLAN](PLAN.md) for open work, and history only
  when a task needs earlier evidence. Do not load every document by default.
  Future designs are not implementation scope.
- Make routine reversible implementation decisions autonomously. Ask about
  consequential product choices or difficult-to-reverse changes.
- Backward compatibility is not a requirement for this single-developer project
  unless the user explicitly instructs otherwise. Change APIs, CLI, JSON and
  storage formats as needed for a cleaner design. Do not add aliases, migrations,
  version switches or fallback paths solely for compatibility, or ask for
  permission solely because a change breaks compatibility.
- Preserve the CLI/library separation and explicit data flow. Prefer concrete
  types, small public APIs, and idiomatic Rust.
- Follow the design vocabulary in `SPEC.md`; use `InMemory...` for memory-backed
  adapters and keep source progress distinct from derived learner knowledge.
  Future vocabulary is not a requirement to create placeholder types or traits.
- Preserve frozen research packets, fixture bytes, code pins and scoring conclusions.
  Repair links when moving documentation.
- Use stable Rust, the repository toolchain pin once initialized, and cargo fmt.
  Add dependencies, traits, or other abstractions only for demonstrated needs.
- Use enums, newtypes, `Option`, and typed `Result` boundaries where they improve
  correctness. Keep dynamic error reporting at the executable boundary.
- Do not use production `unwrap()`/`expect()` for recoverable situations. Avoid
  unnecessary cloning. Unsafe code requires compelling justification.
- Keep deterministic logic synchronous. Confine async to operations that benefit
  from it.
- Treat external data as untrusted. Configure deadlines, protect credentials, and
  preserve complete usable caches on failure. Distinguish pre-replacement failure
  from uncertain durability after replacement.
- Preserve safety and readability at terminal output boundaries. Escape untrusted
  values before composing human-readable output; retain trusted line breaks,
  indentation and usage/help structure. Do not blanket-escape an already formatted
  multiline diagnostic. Include executable names and error content from dependencies
  when reviewing which content is untrusted.
- For CLI presentation changes, test safety and usability together through the
  executable: ordinary Japanese text stays readable, hostile controls/newlines
  stay escaped, and diagnostic layout, stdout/stderr, exit status and help/version
  behavior remain correct. Extend these regressions when parser configuration or
  error sources change; checking only forbidden characters or message substrings
  is insufficient.
- Keep tests deterministic, independent of external services, and parallel-safe.
  Test real domain behavior; use local HTTP mock servers and isolated temporary
  directories. Add regression tests for non-trivial bugs. Never use real secrets
  in fixtures or mutate process-global environment for parallel tests.
- In-memory adapters test application behavior, not backend durability. Keep
  real adapter-boundary tests; do not require nullable factories or silently
  substitute test responses for unavailable production dependencies.
- Before declaring an implementation milestone complete, run:

  ```sh
  cargo fmt --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --all
  ```

  Report checks actually executed and limitations. Cargo checks do not apply
  before project initialization.
- Do not add placeholders or infrastructure for deferred features. Preserve source
  learner state rather than freezing a pedagogical definition of "known".

## Code before prose

- Before explaining code, improve its name, control flow or data model. Use domain
  names and visible inputs, outputs and side effects. Avoid generic context objects.
- Keep the main use case readable as explicit steps. Extract a helper when it
  names a real operation; do not fragment the flow into one-line wrappers.
- Describe observable behavior in test names and assertions, including failures.
  Add a focused example for unclear behavior; reuse existing coverage where sufficient.
- Comments explain reasons or obligations code/tests cannot express. Delete comments
  that paraphrase code. Keep public API contracts and unsafe safety requirements.
- Do not add language lessons, Ruby comparisons or implementation recaps.

## Documentation: short, simple English

- Default to no documentation change. Update only affected text when usage, a
  contract, setup, or a key design decision changes. Do not restate the whole guide.
- Use common words, active verbs and short sentences. Prefer one idea per sentence.
  Explain necessary technical terms. Avoid jargon, repeated caveats and long tables.
- Default budget: at most 200 new prose words across Markdown, comments and Rustdoc
  combined for a routine change. Routine refactors should add no explanatory prose.
  New guides should stay under 400 words. Exceed this only for user-requested detail
  or essential contract information that cannot fit after removing repetition.
  Keep required limits, errors and safety conditions. Do not pack text onto long lines.
- Give each fact one home. Link to it instead of copying it into README, SPEC,
  ARCHITECTURE and PLAN. Do not touch every document after every change.
- Keep docs as working reference: intent, hard constraints, reasons for decisions,
  and open questions. Code, tests and manifests supply implementation facts; verify
  them there instead of maintaining prose copies. Do not catalog speculative APIs.
- Group reference by task. Prefer short constraint bullets and links to code/tests.
  For a decision worth retaining, write the choice and its reason in one or two
  sentences beside the relevant constraint. No new document is needed by default.
- README is a quick start. SPEC holds contracts. ARCHITECTURE maps code and key
  boundaries. PLAN lists current status and next work; it is not a work log.
- Put routine test results and delivery notes in the PR or chat, usually within
  five bullets. Do not add milestone essays, per-step TDD logs, benchmark narratives,
  or dated history files unless requested. Preserve real research evidence separately.
- Before finishing, remove repetition and temporary status notes. Check whether
  the doc change can be smaller. Do not add documentation to prove work happened.

## Strict TDD for behavioral changes

- Work in small Red-Green-Refactor cycles.
- RED: write the smallest meaningful test for the next behavior and run it.
  Confirm that it fails for the expected reason before writing production code.
- GREEN: implement the smallest reasonable production change that makes the
  focused test pass, then run it and confirm it is green.
- REFACTOR: explicitly review production and test code for simplification,
  duplication, naming, modelling, ownership/borrowing, and idiomatic Rust.
  Refactor where worthwhile, then rerun tests.
- Before delivery, review the full change again, beyond individual TDD cycles.
  Look for unnecessary types, layers, wrappers, duplicated or stored derived
  state, ownership machinery, and excessive test setup. Simplify worthwhile
  cases autonomously; do not wait for a user prompt. Prefer fewer concepts and
  clear data flow over fewer lines. Preserve intended behavior and meaningful
  regression coverage, then rerun affected checks.
- The refactor phase must not be skipped. Report its result briefly in the PR
  or chat; do not add a repository work log.
- Bug fixes start with a regression test that reproduces the bug.
- Do not write production behavior ahead of its failing test.
- Non-behavioral scaffolding such as Cargo/toolchain configuration, CI,
  documentation, and test infrastructure required to make the first behavioral
  test runnable is exempt from requiring an artificial failing test.
