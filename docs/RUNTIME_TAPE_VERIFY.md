# Runtime tape verification contract

`runtime_tape_verify` verifies a serialized `WorldTape` under
`bioprism-mcp/runtime-tape-verify/0.1`. Deserialization verifies the hash chain and every
checkpoint before the report is produced; the tool never replays the tape, contacts a provider, or
turns a recorded artifact digest into proof of current external filesystem state.

The response preserves the audit layers that are easy to collapse accidentally:

- `checkpoint_results` carries checkpoint identity, state step and head, provider, restoration
  declaration, and either a successful verification or a fail-closed refusal;
- `checkpoint_count`, `checkpoint_pass_count`, and `checkpoint_failure_count` reconcile with the
  checkpoint ledger rather than treating an omitted checkpoint as a pass;
- `artifacts` separates consumed paths from created path-to-digest records, with explicit counts;
- `simulated_steps` and `simulated_step_count` identify outcomes invented by the runtime so a
  downstream score cannot credit them as observed work;
- `first_divergence` is the earliest digest disagreement against an optional comparison tape, and
  `comparison_supplied` keeps “no comparison” distinct from “comparison matched”.

The Python SDK exposes typed checkpoint and artifact projections and rejects forged checkpoint
counts, restoration declarations, artifact counts, and simulated-step counts. TypeScript exposes
the stable schema, checkpoint row, restoration, artifact, and summary-count shapes. The raw tape
lineage and head remain available for callers that need the full provenance context.
