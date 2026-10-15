# Durable execution plan

Proposed design for the [story request](story-generation-request.yml).
Extend the existing [application runner](../../crates/yomibu-app/src/application/generate_story/mod.rs);
no generic command dispatcher is needed. No implementation is included.

## Boundaries

Follow the [architecture rules](../../AGENTS.md#dependencies-and-composition).
The application coordinates jobs and invokes the shared story workflow. Core owns
job rules and storage contracts; extensions implement storage. Each executable
constructs dependencies. Command data describes work; job state describes execution.
Clients and capability instances remain outside both.

Job storage is separate from the synchronous learner cache. Backend choice remains
open. Require atomic submission, claims, checkpoints and terminal publication.
Remote storage must allow async I/O.

## Submission

- Validate request shape; resolve defaults and generated seeds once.
- Atomically create a durably queued job. A caller-scoped idempotency key returns
  the existing job for the same submitted content; different content conflicts.
- Persist effective request/options, learner scope, absolute deadline from submission,
  budget counters, attempt history and [replay bundle](replay.md) references.
  Capture learner inputs on the first attempt, before paid calls. Exclude credentials.
- Acknowledge only durable acceptance. An uncertain write outcome requires lookup
  by idempotency key before resubmission.

## Execution and recovery

States: `queued`, `running`, `succeeded`, `failed`, `cancelled`, `needs_attention`.
Safe retries return to `queued`; unknown external outcomes enter `needs_attention`.
Reconciliation must establish the outcome before recovery continues.

Workers claim jobs atomically with an expiring lease, renewed while running.
A changing lease token rejects stale worker writes. Lease expiry permits another
attempt only when persisted checkpoints prove recovery is safe.

Before each external call, atomically check the lease and cancellation flag,
persist intent and reserve budget. Store its response and completed checkpoint
before continuing. Reuse completed artifacts after restart.
If completion is uncertain, do not resend automatically. Submission idempotency and
leases do not guarantee exactly-once external calls.

Retries share the job's remaining limits and deadline; bound attempts and backoff.
Cancellation is durable: workers stop new calls, reconcile in-flight outcomes, then
publish cancellation. Publish terminal state and result/error atomically; terminal
outcomes are immutable. No acceptable candidate is a distinct failure reason.

## Validation before delivery

First exercise one worker, then lease takeover. Test duplicate submissions, stale
writes, crashes around dispatch/checkpoints, budget exhaustion and cancellation
races through application ports and real storage. Retention and reconciliation
interfaces remain open.
