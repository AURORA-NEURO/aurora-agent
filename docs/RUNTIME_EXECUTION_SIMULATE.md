# Runtime execution simulation contract

`runtime_execution_simulate` runs a bounded effect program only against the deterministic in-process
runtime. It never reaches a host filesystem, network, process, model, message, or payment endpoint.
Its response is a replay/audit receipt under
`bioprism-mcp/runtime-execution-simulate/0.1`, not evidence that an external provider exists.

The report keeps these states separate:

- `request_count`, `recorded_requests`, `recording_complete`, and `partial_recording` describe
  whether the requested program finished before an execution error or hard budget exhaustion;
- `live_outcomes` and `live_outcome_count` describe the bounded run, while `policy_journal` and its
  count retain authorization decisions, including denials that never became tape effects;
- `world` exposes deterministic call/time/state-manifest/file-change evidence; `budget` preserves
  resource accounting, soft warnings, abort resource, and fully consumed effect count;
- `replay` verifies the recorded prefix and reports matched outcomes, replay error, completeness,
  and outcome count. `replay_complete` is distinct from complete recording: a partial run can still
  replay its complete recorded prefix;
- `fork` is optional and preserves the observed fork state, inherited prefix length, suffix
  outcomes, child tape, and branch comparison. `fork_requested` prevents null fork evidence from
  being mistaken for a non-requested or refused continuation.

The Python SDK exposes typed replay, deterministic-world, budget, and fork projections and rejects
forged counts or inconsistent complete/partial/replay states. TypeScript exposes the same stable
schema and nested result shapes. The raw tape and outcome values remain available because effect
outcomes are intentionally tagged by the runtime and must not be flattened into one generic score.
