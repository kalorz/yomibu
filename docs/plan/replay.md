# Replay plan

Proposed companion to [durable execution](durable-execution.md), building on the
[serializable story request/options](../../crates/yomibu-core/src/domain/generation/request.rs).
No implementation is included. Today's run report is evidence, not a complete
replay bundle.

## Define the action

- **Resume:** continue the same interrupted job from durable checkpoints, using
  its captured inputs and remaining budget.
- **Replay:** create a linked job using captured inputs and settings. Generation calls
  run again; identical output is not guaranteed.
- **Rerun:** create a linked job using the same intent with current learner data
  and configuration. Label it explicitly because its inputs may differ.

Reading a stored result makes no calls. Every replay/rerun has a new job ID,
parent reference and explicit budget. Neither changes the original job's history.

## Capture inputs and artifacts

Freeze effective settings at submission. On the first attempt, capture a coherent
learner snapshot and the time used to evaluate eligibility. Recovery never replaces
these with newer learner progress.

The bundle contains:

- Normalized request, resolved options/limits/seeds, capability identities/settings/
  revisions, application build and capture schema.
- Learner progress, scoped material contents and IDs,
  and the versions of eligibility rules and resources used.
- Metadata/vector profiles and selection evidence, including query vectors and
  ranking values where used; record final selection order.
- Template/preparation inputs and exact provider request payloads/options without
  authentication headers. Save payloads before dispatch.
- Provider responses, candidate provenance, assessment findings and repair lineage
  as those phases complete. Save artifacts before their consumers run.

Store bytes or retain immutable referenced versions. Hashes verify content but
cannot recover missing data. Snapshots preserve historical evidence; they do not
replace current learner state or declare permanent mastery.

## Replay contract

Incomplete inputs block replay; resume recoverable jobs under durable execution
rules. Never silently substitute current defaults, fresh selection evidence,
changed resources or another provider/model.

Reuse captured selection evidence; rebuild deterministic preparation with compatible
versions. Initial generation payloads must match recorded bytes/options. New model
outputs can change later assessment and repair inputs. Missing or unsupported
versions make replay explicitly unavailable.

Resolve credentials afresh and check current access to the learner and snapshots.
Snapshot expiry/deletion makes dependent replay unavailable. Cross-store snapshot
coordination and retention policy remain open.

## Validation before delivery

Change defaults, source data, evaluation time and resources after capture. Verify
replay preserves inputs, rerun uses changes, deterministic preparation matches,
unsupported bundles fail and paid calls consume the new budget.
