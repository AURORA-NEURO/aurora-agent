# Evaluation trajectory check contract

`evaluation_trajectory_check` evaluates a serialized `bioevalx::Trajectory` under
`bioprism-mcp/evaluation-trajectory-check/0.1`. It checks declared path properties and bounded
state-distance suffixes; it does not infer decision quality from path length or tool-call count.

The response keeps four layers distinct:

- `step_records` preserves every step's act, irreversibility, success, and optional caller-supplied
  state distance. `acts` is the sorted set of distinct labels, so it is intentionally not required
  to have the same length as `steps`;
- `property_records` preserves the named `preceded_by`, `no_blind_retry`, or `followed_by`
  declaration. `property_outcomes` attaches violation indices and distinguishes `held` from
  `vacuous`; a property with no opportunity is not credited as passing;
- `recovery_records` preserves each failed step, the next different strategy step when one exists,
  and its pair-derived latency. There is no average recovery score that could hide a failure that
  never recovered;
- `bounded_suffix` keeps immediate distance, best downstream distance, declared horizon, observed
  steps, and completeness separate. A suffix truncated by the trajectory end is incomplete even
  when it has a useful downstream observation.

`property_count`, `held_count`, `violated_count`, `vacuous_count`, and `recovery_count` reconcile
with their corresponding rows. The Python SDK exposes typed step, property, outcome, recovery, and
bounded-suffix projections and rejects forged counts, names, act sets, held/vacuous states, pair
latencies, and suffix completeness. TypeScript exposes the same schema and row shapes while
preserving the raw property declarations and pair-shaped recovery ledger.
