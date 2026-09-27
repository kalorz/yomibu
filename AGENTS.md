# Repository engineering rules

- Read `SPEC.md` for authoritative decisions and `PLAN.md` for milestones. Keep
  both aligned with accepted changes; future milestones are not implicit scope.
- Make routine reversible implementation decisions autonomously. Ask about
  consequential product choices or difficult-to-reverse changes.
- Preserve the CLI/library separation and explicit data flow. Prefer concrete
  types, small public APIs, and Rust idioms over Ruby service-object patterns.
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
- Keep tests deterministic, independent of external services, and parallel-safe.
  Test real domain behavior; use local HTTP mock servers and isolated temporary
  directories. Add regression tests for non-trivial bugs. Never use real secrets
  in fixtures or mutate process-global environment for parallel tests.
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
- Comments explain intent, invariants, and non-obvious choices. After meaningful
  work, provide concise "Rust notes for a Ruby developer" about choices that
  materially influenced the implementation; keep lessons out of source comments.

## Strict TDD for behavioral changes

- Work in small Red-Green-Refactor cycles.
- RED: write the smallest meaningful test for the next behavior and run it.
  Confirm that it fails for the expected reason before writing production code.
- GREEN: implement the smallest reasonable production change that makes the
  focused test pass, then run it and confirm it is green.
- REFACTOR: explicitly review production and test code for simplification,
  duplication, naming, modelling, ownership/borrowing, and idiomatic Rust.
  Refactor where worthwhile, then rerun tests.
- The refactor phase must not be skipped. If no code change is warranted,
  explicitly record that the refactor review was performed and no change was
  justified.
- Bug fixes start with a regression test that reproduces the bug.
- Do not write production behavior ahead of its failing test.
- Non-behavioral scaffolding such as Cargo/toolchain configuration, CI,
  documentation, and test infrastructure required to make the first behavioral
  test runnable is exempt from requiring an artificial failing test.
