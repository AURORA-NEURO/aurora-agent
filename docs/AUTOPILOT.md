# Autopilot: grant-authorised autonomous mission driving

`bioprism-autopilot` drives an instantiated mission through plan → dispatch → classify → repair
cycles until the workflow's evidence is complete, the attempt budget is spent, or something is
refused in a way that makes re-sending dishonest. Blueprint 40.36 specifies the retry
classification the drive consumes; the autonomous driver, the authority document, and the
repair-subset construction are this crate's design and are labelled as such. The CLI surface is
`autopilot grant-template`, `autopilot run`, and `autopilot verify`.

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
   reconciliation record whose completion is `complete` and whose integrity is valid, in that
   attempt's own scope — the full plan for a full dispatch, the re-dispatched subset for a
   repair.

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

A repair mission's `workflow_binding` evidence plan is filtered to the subset with its digest
recomputed, and the repair attempt's reconciliation covers only that subset, labelled
`repair_subset`. A repair carries none of the base mission's `claim_requests`,
`evaluator_review`, or `route_review`: the stripped claim ids are disclosed on the repair
dispatch action (`dropped_claim_ids`), and the limitation is stated in every report. Claim
lineage exists only for attempt 1.

A transport error — the dispatch returned no mission report — ends the drive: the mission outcome
is unknown at mission level, side effects may have run, and the drive stops rather than re-send
blind. A mission report recording an operator cancellation ends the drive regardless of retry
options.

## Restart-safe driving without secret retention

The Rust kernel exposes `drive_mission_with_checkpoint` and
`drive_instantiation_with_checkpoint`. Their callback runs after each dispatch is appended to the
private in-memory history and before another plan is constructed. Hosts can seal that history with
`seal_autopilot_checkpoint` and persist it through the caller-owned JSON store adapters. A sealed
checkpoint contains only the grant and mission digests, bounded attempt/step counts, step-id and
result-metadata digests, retry status counts, reconciliation posture, generation, and a chained
snapshot digest. It never contains mission arguments, provider output, credentials, raw evidence,
or dispatch error text.

After a process restart, `resume_mission_with_checkpoint` or
`resume_instantiation_with_checkpoint` requires the host to supply the original mission and
rehydrated `AttemptRecord` values. The kernel recomputes every retained projection and refuses
before dispatch when a grant, mission, attempt, generation, or snapshot digest differs. The
transactional persistence adapter adds compare-and-swap protection so two workers cannot both
advance the same checkpoint head. This is restart-safe recovery, not blind replay: a host that
cannot rehydrate the private material must stop rather than reconstruct it from incomplete
metadata.

The grant may also include a `schedule` object with `retry_base_delay` and `retry_max_delay`.
These are bounded logical clock ticks, not seconds owned by the kernel. Repair `n` waits for
`min(retry_base_delay * 2^(n-1), retry_max_delay)` through the caller-owned `AutopilotWait` seam;
saturating arithmetic and a one-year logical-tick ceiling prevent overflow or unbounded delay.
The initial full dispatch is never delayed, terminal/unknown policy decisions are unchanged, and
an undelivered dispatch is still never retried. Resuming from a checkpoint recomputes the same
retry index from the rehydrated attempt count, so a process restart cannot reset the backoff.

## Dry run dispatches nothing

`autopilot run --dry-run` plans attempt 1 only: it prints the exact mission the drive would
dispatch — grant policy overwrite applied — with its content digest, step count, and attempt
budget, performs zero dispatches, and writes zero files (`--report-out` is echoed, not written).
The response labels itself `no_dispatch`, `dispatch: not_started`, `writes: none`.

## The report and its verification

A completed drive produces one autopilot report chaining every receipt: the grant and its digest,
the base mission digest, per-attempt mission and report digests, per-step classification rows
with the signal that produced each decision, reconciliation status and scope per attempt, the
final status (`succeeded`, `exhausted`, or `refused`), and its structured stop detail. Exhausted
stops carry per-step unresolved accounting; an accounting never reads "nothing unresolved" while
steps remain unresolved.

`report_sha256` is computed over the canonical report with the digest field removed, so any later
edit is detectable by recomputation alone. `autopilot verify` recomputes it and checks the
structural contract, returning a projection rather than a bare boolean: `digest_match`,
`digest_malformed` (a claimed digest that is not 64 lowercase hex characters is a shape defect,
reported distinctly rather than as a tamper mismatch), `limitations_present`,
`final_status_known`, and `attempts_present`; `valid` is the conjunction. A report that is not an
object or claims a foreign schema is an error, not an invalid verification, because there is no
autopilot report to verify.

Verification proves the document is the one that was stamped. It does not prove the drive was
run against the reader's current workspace, that the dispatched tools' outputs are correct, or
that any domain, deployment, or release authority exists; a `succeeded` final status is the
executor's and reconciler's accounting, nothing more.

## Exit codes

- `0` — the command completed and its assertion held: a drive that ended `succeeded`, a report
  that verifies, a template or dry run that completed.
- `1` — the command completed and the checked property does not hold: a finished drive whose
  final status is `exhausted` or `refused`, or a report that fails verification. Exit 1 reports a
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
first four:

1. "no recurrence: the drive runs one mission to a stop state and never repeats a completed mission"
2. "no MCP tool exposure: the autopilot is not an MCP tool and registers nothing with the server"
3. "metadata-only cross-process resume: checkpoints retain digests and bounded status metadata, while callers must rehydrate private mission and report material"
4. "wall-clock ownership and deadlines remain caller-owned: a grant may authorize logical-tick retry backoff, but the wait seam and deadline policy live outside the kernel"
5. "an undelivered dispatch is never re-sent: a missing mission report leaves side effects unknown at mission level"
6. "a repair attempt's reconciliation covers only the re-dispatched subset and is labelled with that scope"
7. "succeeded steps are never re-dispatched; a binding whose retained payload is gone excludes its dependent instead"
8. "a repair re-dispatches steps without the base mission's claim requests or reviews; claim lineage exists only for attempt 1"

The final autopilot report is written only by a drive that reaches a stop state. A checkpoint may
survive a mid-loop error, but it is not a partial report: it is restart metadata, and the host
must still rehydrate the private attempt records before a resumed drive can dispatch.
