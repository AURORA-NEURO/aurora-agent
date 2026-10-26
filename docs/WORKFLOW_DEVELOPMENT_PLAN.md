# Autonomous workflow development plan

The requested end state is an autonomous natural-language workflow builder and executor with
hundreds of real integrations and deep glioma research support. The current implementation is an
initial executable increment. The original objective remains unfinished; neither generated
platform descriptors nor fixture registration counts establish integration coverage.

| Area | Current evidence | Required before claiming the requested end state |
| --- | --- | --- |
| Natural-language authoring | Local model -> structural proposal -> schema/DAG validation; a live single-source plan worked | Held-out multi-step task fidelity, parameter/constraint preservation, ambiguity handling, graph editing, branching and bounded iteration |
| Execution | Sequential read-only DAG, per-call budgets, public-value checkpoints, explicit retry, digest-checked resume | Cancellation, scheduling, concurrent independent branches, shared-worker fencing, effectful execution through the existing effect ledger, compensation/reconciliation policies |
| Integration breadth | Eight concrete reviewed public-source adapters; strict reviewed GET OpenAPI imports | At least hundreds of independently configured and verified services; recorded per-operation capabilities, availability, pagination, rate limits and failure behavior |
| Authentication | Application-owned transient header resolution for imported operations; no model-owned secrets | Protected OAuth/account connection lifecycle, scoped grants, token refresh/revocation, tenant isolation and auditable credential resolution |
| Glioma research | Live literature/trial registry/TCGA project metadata composition with source-separated citations and missingness | Composed molecular, imaging/pathology, longitudinal, single-cell/spatial and independent replication analyses using the existing typed tools, grounded local synthesis, and claim audits |
| Evidence quality | Metadata provenance/omissions, explicit unknown states, preserved source partitions | Independent relevance/quality evaluation, contradiction and negative-result handling, cohort-overlap checks, freshness policies, source-specific quality failures |
| Product interfaces | Dependency-free SDK and local CLI commands | An editable workflow UI, authenticated service API, operator views for runs/holds/recovery, accessible errors and installation/onboarding flows |
| Reliability | Focused invariants, existing Python regression suite, separate live smoke checks | Provider/source outage campaigns, process-termination recovery, task-quality and cost/latency measurements, cross-SDK contract support and release/deployment validation |

## Next implementation order

1. Measure and improve local-model multi-step fidelity using a held-out public research task set.
   Verify requested output stages, exact limits and source boundaries rather than only JSON shape.
   Keep planning failures and semantic omissions visible; do not turn a valid graph into a success
   claim for an unfulfilled task.
2. Bridge the composed glioma workflow into the existing bounded snapshot/grounded-synthesis and
   claim-audit surfaces. Preserve exact citation closure and source identity. Public human cohort
   summaries remain separate from the Rust preclinical worker boundary.
3. Add real integration onboarding over reviewed OpenAPI and MCP operations. Count a service only
   after its auth/scopes, actual request/response behavior and failure handling are verified; use
   representative integration families rather than generated labels.
4. Add effectful workflow operations through the repository's existing authorization/effect
   reconciliation contracts. Bind approval to the exact operation, request, destination and plan;
   preserve uncertain external completion across crashes instead of automatically repeating writes.
5. Add graph editing and operational controls, then concurrency/scheduling and shared execution.
   Validate the installed application and deployment artifacts before claiming production readiness.

## Completion evidence

Completion requires demonstrations that a user request produces the requested graph and outputs,
survives interruption without duplicate completed effects, uses real configured integrations at
the requested scale, and produces glioma research artifacts with auditable source closure and
explicit unresolved questions. Tests of only the registry, schema validator, simulator, or one
retrieval cannot establish that broader result. Research tools continue to require qualified
scientific review and do not make individual diagnosis or treatment decisions.
