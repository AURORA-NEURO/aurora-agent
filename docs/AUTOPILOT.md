# Autopilot: grant-authorised autonomous mission driving

`bioprism-autopilot` drives an instantiated mission through plan → dispatch → classify → repair
cycles until the workflow's evidence is complete, the attempt budget is spent, or something is
refused in a way that makes re-sending dishonest. Blueprint 40.36 specifies the retry
classification the drive consumes; the autonomous driver, the authority document, and the
repair-subset construction are this crate's design and are labelled as such. The CLI surface is
`autopilot grant-template`, `autopilot run`, `autopilot resume`, `autopilot goal-step`,
`autopilot goal-verify`, and `autopilot verify`.

The MCP server exposes `autopilot_drive` and `autopilot_verify`. The drive accepts an accepted
`domain_workflow_instantiate` result and an explicit grant. Its mode defaults to `preview`,
which makes no dispatch or writes; `mode: "execute"` starts a bounded drive and returns the
sealed report plus its verification. `max_dispatches_this_call` can cap one invocation below the
grant's total attempt budget. When another action is available at that cap, the report says
`final_status: "paused"` and the response includes a caller-owned `recovery.next_request`. Submit
that request to `autopilot_drive` with `mode: "resume"` to continue. The server does not retain
the report or recovery state, authenticate the caller, own a deadline, sleep for retry delays, or
repeat a completed mission. Requested retry delays are returned as virtual logical-tick events
for a caller-owned scheduler.

## Authority comes from the grant, and only from the grant

There is no default grant, no environment fallback, and no way to widen a grant after
construction. The grant's authority is applied by overwriting the dispatched mission's policy:
execution is turned on, and the allow-list and side-effect posture are replaced with the grant's,
so a mission authored wider than its grant is narrowed, never widened.

The grant document (`deny_unknown_fields`; any unrecognised field is a parse error, not an
ignored knob):

- `allowed_tools` — required, no default. Bare tool names (ASCII alphanumerics and underscores),
  between 1 and 512 entries, no duplicates. An empty list is refused: the grant is the only
  source of execution authority, so an absent list grants nothing rather than everything.
  `agent_mission` is refused as recursive mission dispatch.
- `allow_side_effects` — default `false`. Permits caller-supplied confirmation flags to reach
  side-effecting tools.
- `max_attempts` — required. Total mission dispatches, full and repair combined, between 1
  and 16. An undelivered dispatch (transport error, no report) still counts against the budget.
- `schedule.retry_base_delay` and `schedule.retry_max_delay` — optional bounded logical-clock
  ticks for deterministic exponential repair backoff. Both default to zero; they never widen the
  retry class allow-list, and the host owns waiting and deadline enforcement.
- `retry.retry_retryable_as_is` — default `true`. Re-dispatch steps whose recorded evidence
  declares 40.36 `retryable_as_is`.
- `retry.retry_retryable_after_change` — default `false`. The only change the drive can make is
  re-materializing bindings from retained results, which is why this defaults off.
- `retry.retry_unknown` — default `false`. A failure that declared no retry decision is re-sent
  only under this explicit opt-in.
- `require_reconciliation_complete` — default `true`. Success requires a reconciliation record
  with `complete` completion and valid integrity; a mission report alone is never enough.
- `stop_on_first_success` — default `true`, and only `true` is accepted. `false` is refused
  loudly rather than being an unknown field or a silently ignored option.

There is deliberately no field for retrying `terminal`: a decision 40.36 calls dead-as-written
cannot be purchased with a flag, so the illegal state is unrepresentable.

## The 40.36 classification, from evidence only

The mission executor records exactly four per-step statuses — `succeeded`, `refused`, `blocked`,
`cancelled` — and lands every dispatched failure on `refused`, whether the cause was executor
policy or a nested tool error. The retry decision is therefore not recoverable from the status
alone, and the classifier refuses to guess it:

| recorded evidence | class |
|---|---|
| status `succeeded` | `succeeded`; never re-dispatched |
| status `blocked` | `blocked`; the step never ran, carries no failure class, and is rescheduled exactly when its failed prerequisites are |
| any failure whose recorded evidence declares a 40.36 decision | that declared class (`terminal`, `retryable_after_change`, or `retryable_as_is`) |
| status `refused` with no retained tool envelope | `terminal`: the executor itself refused, and policy behaving correctly is not a transient fault |
| status `refused` with a retained tool envelope and no declared decision | `unknown`; never coerced toward retryable |
| status `cancelled` | `unknown`, and the drive never re-dispatches a cancelled step at all |
| any other status string | `unknown`; a future status must not silently become retryable |

A declared decision is recognised only in these places, and only as the exact strings `terminal`,
`retryable_after_change`, `retryable_as_is`:

1. the retained wire envelope at `/result/structuredContent/retryability` or
   `/result/structuredContent/error/retryability`;
2. the recorded error text, when that text parses as a JSON object carrying `retryability` or
   `error.retryability`.

Anything else — a different spelling, a bare boolean, prose that mentions retrying — is not a
signal. An unrecognised value in a `retryability` slot is not a near-miss to be repaired; it is
`unknown`.

## The success rule

The drive reports success only when all of the following hold, each read from a retained record:

1. every step of the base mission has a recorded `succeeded` result in some attempt (the most
   recent attempt that dispatched the step decides);
2. the latest attempt's own mission report has `mission_status == "succeeded"`;
3. under `require_reconciliation_complete` (the default): the latest attempt carries a
   reconciliation record whose completion is `complete`, whose integrity is valid, and whose
   canonical digest is verified, in that attempt's own scope — the full plan for a full dispatch,
   the re-dispatched subset for a repair. A full record is checked by recomputing its digest. An
   MCP summary counts only when it embeds the full canonical reconciliation record, its import
   receipt, and summary fields that match that record. Autopilot validates the record against the
   registry contract, recomputes its canonical digest, and binds the exact projection to the
   mission report; a summary-only projection cannot support success.

Nothing is inferred. Steps all succeeding while the report says otherwise is an
`inconsistent_report` stop, not a success; a missing or incomplete reconciliation under a
requiring grant is a `reconciliation_incomplete` stop.

When the grant requires reconciliation and no reconciliation source can exist — the mission
carries no `workflow_binding` and no instantiation artifact was supplied — the drive refuses
before its first dispatch (`reconciliation_unavailable`, zero attempts used): the success rule is
already provably unreachable, and dispatching anyway would spend side effects on an attempt that
cannot reach success. The same reasoning refuses a repair for a binding-less mission
(`repair_reconciliation_unavailable`).

## Repair semantics

A repair re-dispatches the subset of not-yet-succeeded steps, and only when every such step can
be included:

- a step whose recorded outcome is a cancellation is never re-dispatched, whatever the grant's
  retry options say and whatever the surrounding report's mission status;
- a `terminal` failure is never re-dispatched; no grant option exists for it;
- `retryable_as_is`, `retryable_after_change`, and `unknown` failures are included only when the
  grant's corresponding retry option is on;
- `blocked` steps carry no failure class and are included alongside their failed prerequisites;
- bindings from already-succeeded steps are re-materialized from retained payloads, mirroring the
  executor's own payload derivation; a payload that was not retained, or a source pointer missing
  from it, excludes the dependent step with the reason recorded — a succeeded step is never
  re-run to regenerate a payload;
- a step depending on an excluded step is itself excluded.

Exclusion is permanent under a fixed grant and fixed retained evidence, so when any needed step
is excluded the drive stops with per-step accounting instead of dispatching a repair that cannot
reach success.

The constructed repair is also validated against the mission's resource-reservation rules before
it receives dispatch authorization. If construction or validation refuses the subset, the drive
ends with `final_status: "refused"` and a `repair_refusal` detail; the report retains every earlier
attempt and records the refusal class, reason, and unresolved steps. The refused repair does not
cross the dispatcher boundary or consume another dispatch.

A repair mission's `workflow_binding` evidence plan is filtered to the subset with its digest
recomputed, and the repair attempt's reconciliation covers only that subset, labelled
`repair_subset`. Claim requests carry forward only when every `requires_steps` entry belongs to
the repair subset. Claims that depend on a previously succeeded or excluded step are omitted and
their ids appear in the repair action's `dropped_claim_ids`. This keeps each claim lineage bound
to outputs from the attempt that produced them; the driver never merges a prior result into a new
claim evaluation. `evaluator_review` and `route_review` are omitted because they reviewed the
original claim set or exact mission draft, not the changed repair. The report states this
attempt-scoped limitation.

An error or panic from the dispatch boundary — no mission report was returned — ends the drive:
the mission outcome is unknown at mission level, side effects may have run, and the drive stops
rather than re-send blind. Rust panics are converted to that conservative outcome when the build
uses unwinding; an aborting panic strategy terminates the process. A returned value that fails the
mission-report contract is also retained in the private
attempt record with its digest and validation error, checkpointed, and treated as unknown; the planner
stops rather than retrying a dispatch whose side effects may have run. Its public attempt row has a
null outcome summary, a `report_validation_error`, and one unknown classification row per step.
Each parsed report is checked against the exact mission sent to that dispatch: its recomputed plan
and execution posture must match, and every present step result must have a unique planned id and
carry the dispatched tool and required flag. Claim requests and evaluator review must exactly echo
the dispatched mission, and claim lineage is recomputed from those inputs and that report's own
result rows. Aggregate succeeded, refused, blocked, and cancelled
counts must match the rows present; the required-failure total cannot omit a visible required
refusal/block, cannot exceed the required plan size, and must match exactly when every row is
present. `returned_bytes` cannot undercount successful or wire-backed result bytes. A missing row
stays explicit unknown evidence for that step; a
duplicate, foreign, or misidentified row invalidates the whole returned report. This binding also
runs when a grant waives reconciliation, so that waiver cannot turn a receipt for a different plan
into success or retry evidence.

A mission report recording an operator cancellation ends the drive regardless of retry options.

## Restart-safe driving without secret retention

The Rust kernel exposes `drive_mission_with_checkpoint` and
`drive_instantiation_with_checkpoint`. Their callback runs after each dispatch is appended to the
private in-memory history and before another plan is constructed. Hosts can seal that history with
`seal_autopilot_checkpoint` and persist it through the caller-owned JSON store adapters. A sealed
checkpoint schema `0.2` contains only the grant and mission digests, bounded attempt/step counts,
step-id and result-metadata digests, retry status counts, reconciliation posture including whether
the reconciliation digest was verified, generation, and a chained snapshot digest. It never
contains mission arguments, provider output, credentials, raw evidence, or dispatch error text.
Checkpoint `0.1` remains readable when it contains no reconciliation record. A reconciliation-
required grant cannot resume a `0.1` history that has reconciliation because that version did not
retain whether its digest had been verified.

After a process restart, `resume_mission_with_checkpoint` or
`resume_instantiation_with_checkpoint` requires the host to supply the original mission and
rehydrated `AttemptRecord` values. The kernel recomputes every retained projection and refuses
before dispatch when a grant, mission, attempt, generation, or snapshot digest differs. The
transactional persistence adapter adds compare-and-swap protection so two workers cannot both
advance the same checkpoint head. This is restart-safe recovery, not blind replay: a host that
cannot rehydrate the private material must stop rather than reconstruct it from incomplete
metadata.

The CLI makes that boundary usable with `autopilot run --recovery-dir <dir>` and
`autopilot resume --recovery-dir <dir>`. A new run requires an empty directory. Each completed
dispatch writes an append-only generation containing a digest-only checkpoint, a separate private
attempt record, and a chained commit head. Before it invokes the mission executor, it writes a
pending-dispatch marker; after a process restart, an unmatched marker stops resume because the
mission's side effects are unknown. A marker matching a fully committed attempt can be cleared
without dispatch. Resume verifies every generation against the same grant and instantiation before
the planner can continue. It refuses missing, extra, reordered, or mismatched recovery files.

`attempt-*.private.json` contains dispatched mission documents, arguments, reports, reconciliation
records, and dispatch errors. Those files are not encrypted by the CLI; protect and retain the
recovery directory as private operational data. Each attempt is bounded to 20 MB and the full
rehydration history to 80 MB. The digest-only `checkpoint-*.json` files contain no mission or
report payloads. Put any `--report-out` artifact outside the recovery directory.

The MCP adapter supports caller-owned chunked execution through
`drive_instantiation_bounded` and `resume_instantiation_bounded`. The response's digest-only
`checkpoint` is separate from `rehydrated_attempts`, which contains dispatched mission documents,
arguments, reports, reconciliation records, and dispatch errors. A ready-to-submit
`recovery.next_request` is returned only when the full continuation fits the MCP 20 MB request
bound. Persist that request securely in the caller's own store because it contains private history.
If it exceeds the bound, the drive remains paused but cannot continue through this endpoint; stop
without replay. `mode: "resume"` requires the original grant and instantiation plus both recovery
values; kernel validation binds them to the checkpoint before another dispatch. An in-flight MCP
invocation that ends without a response has no caller-visible checkpoint, so its mission outcome
is unknown and must not be replayed automatically. Use the CLI recovery directory when local
pending-dispatch journaling is needed.

The grant may also include a `schedule` object with `retry_base_delay` and `retry_max_delay`.
These are bounded logical clock ticks, not seconds owned by the kernel. Repair `n` waits for
`min(retry_base_delay * 2^(n-1), retry_max_delay)` through the caller-owned `AutopilotWait` seam;
saturating arithmetic and a one-year logical-tick ceiling prevent overflow or unbounded delay.
The initial full dispatch is never delayed, terminal/unknown policy decisions are unchanged, and
an undelivered dispatch is still never retried. Resuming from a checkpoint recomputes the same
retry index from the rehydrated attempt count, so a process restart cannot reset the backoff.

## Bounded goal-level continuation in the Rust kernel

`bioprism_autopilot::drive_goal` adds a caller-controlled outer loop above `drive_mission`. It
addresses the distinction between completing one mission and completing a larger objective: a
successful mission returns control to the goal controller and never completes the goal by itself.
The controller can request another mission, stop with a typed reason, or return an explicit
`GoalDecision::Complete` containing an evaluator identity and evidence digest. That completion is
retained as the caller-evaluator's assertion; the kernel checks its shape and binds it to the last
cycle, but does not independently validate the evaluator or evidence.

Each invocation requires a nonempty goal id, an existing `AutonomyGrant`, and a
`GoalControlBudget`. The budget allows 1–128 mission cycles and at most 2,048 total dispatches,
with an additional ceiling of 16 dispatches per cycle. Before each mission, the kernel narrows a
copy of the same grant to the remaining aggregate allowance. A mission refusal, paused drive, or
unknown dispatch outcome ends the loop immediately. Successful or exhausted missions return to
the controller. If the controller requests more work after a cycle or dispatch ceiling has been
reached, the loop reports which ceiling stopped it and retains only a digest of that pending
mission in its goal report.

The controller receives the goal id, completed-cycle count, cumulative dispatch count, sealed cycle
summaries, and the preceding Autopilot report during the synchronous callback. It owns planning,
evaluation, memory, deadlines, waiting, and any choice to persist private mission or report data.
If that callback panics while Rust unwinding is enabled, the kernel returns a digest-only
`controller_error` refusal with completed cycles intact; an aborting panic strategy still
terminates the process.
The goal report itself contains mission/report/grant digests, dispatch counts, status, evaluator
assertion metadata, a chained cycle digest, and mandatory limitations. The returned raw cycle
reports are kept separate in `GoalControlOutcome::cycle_reports`. `verify_goal_control_report`
rejects unknown fields, malformed digests, broken cycle/report digests, inconsistent totals,
budget violations, impossible status transitions, and incomplete disposition evidence. As with the
mission report, an unkeyed digest detects edits against a retained digest but does not authenticate
the caller, controller, evaluator, or storage.

`resume_goal` continues a sealed goal-control report only when it ended at an explicit
`no_admissible_work` or `needs_review` stop. It requires the original grant and the exact last
Autopilot report, rehydrated by the caller and checked against the goal report's digest and status.
It preserves the report's budget and cycle history and does not replay completed missions. A caller
must not resume `cancelled`, `abandoned`, refused, paused, or unknown outcomes. The report can be
wrapped with `seal_goal_control_checkpoint` and checked with
`validate_goal_control_checkpoint`. The `/0.1` checkpoint contains the verified goal report,
generation, predecessor digest, and a retention marker; it excludes the private last mission
report and controller state. `JsonGoalControlCheckpointPersistence` provides bounded canonical JSON
storage, and `TransactionalGoalControlCheckpointPersistenceCoordinator` requires the caller's
store to perform an atomic compare-and-swap against the expected snapshot digest. A caller restores
the stored checkpoint and passes its `goal_control_report` to `resume_goal_from_checkpoint` along
with the rehydrated last Autopilot report. The coordinator detects stale writers, but its digest is
not authentication or anti-rollback protection: deployment storage and rollback policy remain
caller-owned. The MCP server exposes this kernel as `autopilot_goal_step`, one supplied goal
transition per request. A new request supplies a goal id, grant, explicit cycle/dispatch budget,
and either one mission or an evaluator/stop decision. After a mission, the server returns the
Autopilot report separately and stops with a caller-owned checkpoint; resume supplies that
checkpoint and the separately retained latest Autopilot report, so completed missions are not
replayed. `autopilot_goal_verify` independently checks the goal report and optional exact
checkpoint binding without dispatch. MCP returns checkpoints to the caller and does not write
them. An interrupted mission must use the existing mission-level recovery boundary rather than
replaying from a goal-level checkpoint. The MCP step response distinguishes `dispatches_this_call`
from the cumulative `total_dispatches`; the per-call count comes from the driver's dispatch seam,
so a missing report cannot make an attempted call look like no dispatch.

## Goal-level control in the CLI

`bioprism autopilot goal-step --request <path> --recovery-dir <dir>` applies one supplied goal
decision. A new request includes `goal_id`, the complete `grant`, a `budget`, and one `decision`:

```json
{
  "goal_id": "evidence-review",
  "grant": {
    "allowed_tools": ["workspace_capabilities"],
    "max_attempts": 2
  },
  "budget": {"max_cycles": 4, "max_total_dispatches": 8},
  "decision": {
    "kind": "run_mission",
    "mission": { "mission_id": "cycle-1", "steps": [] }
  }
}
```

The mission must be a complete mission document accepted by the normal Autopilot driver; the
short mission above illustrates the request shape only. `run_mission` dispatches through the
grant-authorised mission driver, then returns a `no_admissible_work` safe stop and a checkpoint.
To continue, submit a new decision with the same goal id and grant, omit `budget`, and reuse the
same recovery directory. The CLI restores the latest safe checkpoint and the private Autopilot
report before invoking the next mission, so a completed cycle is not repeated. An explicit
`stop` with `no_admissible_work` or `needs_review` can also be resumed. `cancelled` and
`abandoned` stops, completion, refusals, and unknown dispatch outcomes are terminal.

`complete` accepts an `evaluator_id` and lowercase SHA-256 `evidence_sha256`. As described above,
these fields preserve the caller-evaluator's assertion; the CLI validates their shape and binds
them to the last cycle but does not independently evaluate the evidence. A new goal cannot be
completed before it has a mission cycle.

The recovery directory is append-only and must be protected by its owner. Goal reports and
checkpoints contain digests and cycle metadata; each raw Autopilot report is kept in a separate
private file alongside a pre-dispatch intent marker. An identity-only directory with no pending
marker is safe to retry because no dispatcher call began; once an intent marker exists, an
unmatched marker makes the next invocation refuse because the mission outcome is unknown and will
not replay it. The original request may be retried against an identity-only directory with the
same stored budget; after a checkpoint commits, continuation omits `budget` and reuses the stored
value. Local content digests detect edits against retained state but do not authenticate or prevent
rollback.
`--report-out` writes the metadata-only goal-control report outside the recovery directory.
`bioprism autopilot goal-verify --report <path> [--checkpoint <path>]` verifies that report and,
when supplied, requires the checkpoint to bind that exact report. It may also verify a checkpoint
without a separate report. Verification performs no dispatch or writes.

## Dry run dispatches nothing

`autopilot run --dry-run` plans attempt 1 only: it prints the exact mission the drive would
dispatch — grant policy overwrite applied — with its content digest, step count, and attempt
budget, performs zero dispatches, and writes zero files (`--report-out` is echoed, not written).
The response labels itself `no_dispatch`, `dispatch: not_started`, `writes: none`.

## The report and its verification

A drive report chains every receipt: the grant and its digest,
the base mission digest, per-attempt mission and report digests, per-step classification rows
with the signal that produced each decision, reconciliation status and scope per attempt, the
final status (`succeeded`, `exhausted`, `outcome_unknown`, `refused`, or caller-bounded `paused`),
and its structured stop detail. Exhausted stops carry per-step unresolved accounting; an accounting never reads
"nothing unresolved" while steps remain unresolved. A paused report carries a continuation reason
and is valid evidence of a chunk boundary, not evidence that the mission completed. Transport
failures and malformed replies end with `outcome_unknown`, distinct from attempt-budget exhaustion
and refusal. Human CLI output includes the structured stop reason and explanation.

`report_sha256` is computed over the canonical report with the digest field removed. An edit that
leaves the recorded digest untouched is detectable by recomputation; the digest is not a signature
or an external identity anchor. `autopilot verify` recomputes it and checks the
structural contract, returning a projection rather than a bare boolean. For schema `0.7` it also
requires the embedded grant and its matching digest, a mission id and digest, totals consistent
with the grant and retained attempt rows, contiguous attempt indexes, full-then-repair attempt
ordering, and a classification row for every dispatched step in order. The stop detail must
match `final_status`; success evidence must cover every planned step and cite an attempt that
recorded that step as succeeded. For `outcome_unknown`, the unresolved-step detail must match the
plan order and latest recorded classification for each step. A malformed returned report is distinguished from a transport
failure by its retained report digest and validation error. Required reconciliation evidence must
agree with the grant and the latest attempt's scope, completion, integrity, digest, and verification
posture. Each attempt includes its reconciliation record so the verifier can run the canonical
registry validation and recompute the reconciliation digest itself; changing the record and
restamping the outer report still fails the structural check. The projection includes
`current_schema_shape_valid` and `shape_errors` alongside `digest_match`, `digest_malformed`
(a claimed report digest that is not 64 lowercase hex characters is a shape defect, reported
distinctly rather than as a tamper mismatch), `limitations_present`, `final_status_known`,
`attempts_present`, and `final_detail_valid`; `valid` is the conjunction. Required reconciliation
success must match the latest attempt's completion, integrity, scope, digest, and verified-digest
state. A bad digest remains visible on an exhausted report with
`reconciliation_digest_verified: false`, but cannot support success. Schemas `0.6`, `0.5`, `0.4`,
`0.3`, `0.2`, and `0.1` remain readable under their original contracts; the verification projection sets
`reconciliation_digest_verification_supported: false` for them so legacy readability is not
mistaken for the new digest check. A report that is not
an object or claims a foreign schema is an error, not an invalid verification, because there is no
autopilot report to verify.

Verification checks internal consistency of the document and its recorded digests. It does not
authenticate who produced or last restamped the report, prove the drive was run against the
reader's current workspace, establish that dispatched tools' outputs are correct, or grant domain,
deployment, or release authority. A `succeeded` final status is the executor's and reconciler's
accounting, nothing more.

## Exit codes

- `0` — the command completed and its assertion held: a drive that ended `succeeded`, a report
  that verifies, a template or dry run that completed.
- `1` — the command completed and the checked property does not hold: a finished drive whose
  final status is `exhausted`, `outcome_unknown`, or `refused`, or a report that fails verification. Exit 1 reports a
  completed drive, not an error.
- `3` — invalid input: a malformed grant document (every grant refusal names the field that must
  change), mission, mission report, workflow instantiation, or autopilot report.
- `7` — policy denied: the grant does not authorise the mission it was asked to drive — a step's
  tool outside the allow-list, a confirmation flag without side-effect authority, or a missing
  allow-list. The mission is well-formed; the authority is what is missing.

A canonicalisation failure inside the binary exits 5 (`io`), the code unclassified internal
failures share.

## Limitations, verbatim from every report

Every autopilot report carries these lines; verification refuses a report missing any of the
first six:

1. "no recurrence: the drive runs one mission to a stop state and never repeats a completed mission"
2. "bounded MCP exposure: the MCP adapter runs one caller-granted drive per invocation and does not own recurrence or caller identity"
3. "metadata-only cross-process resume: checkpoints retain digests and bounded status metadata, while callers must rehydrate private mission and report material"
4. "wall-clock ownership and deadlines remain caller-owned: a grant may authorize logical-tick retry backoff, but the wait seam and deadline policy live outside the kernel"
5. "an ambiguous dispatch outcome is reported as outcome_unknown and is never re-dispatched"
6. "an invalid mission report is retained by digest and never re-dispatched because its side effects may already have run"
7. "an undelivered dispatch is never re-sent: a missing mission report leaves side effects unknown at mission level"
8. "a repair attempt's reconciliation covers only the re-dispatched subset and is labelled with that scope"
9. "succeeded steps are never re-dispatched; a binding whose retained payload is gone excludes its dependent instead"
10. "repair claim lineage is limited to claims whose complete evidence basis is in the repair subset; prior results and evaluator or route reviews are not merged or reused"

The final autopilot report is written only by a drive that reaches a stop state. A checkpoint may
survive a mid-loop error, but it is not a partial report: it is restart metadata, and the host
must still rehydrate the private attempt records before a resumed drive can dispatch.
