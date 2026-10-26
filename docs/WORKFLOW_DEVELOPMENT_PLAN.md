# Autonomous workflow development plan

The requested end state is an autonomous natural-language workflow builder and executor with
hundreds of real integrations and deep glioma research support. The executable implementation is
growing. The original objective remains unfinished; neither generated
platform descriptors nor fixture registration counts establish integration coverage.

| Area | Current evidence | Required before claiming the requested end state |
| --- | --- | --- |
| Natural-language authoring | Local model -> schema/DAG validation; caller-bound operation/output/argument/value-flow contracts; up to four explicitly bounded planning attempts, with one attempt by default | Held-out multi-step task fidelity, reliable translation from free text into the right contract, ambiguity handling, graph editing, branching and bounded iteration |
| Execution | Sequential read-only DAG, per-call budgets, public-value checkpoints, explicit retry, digest-checked resume | Cancellation, scheduling, concurrent independent branches, shared-worker fencing, effectful execution through the existing effect ledger, compensation/reconciliation policies |
| Integration breadth | Eleven concrete reviewed research adapters, including OpenAlex, Crossref and Zenodo; reviewed GET OpenAPI imports; reviewed tools from caller-connected stdio MCP sessions | Hundreds of independently configured and verified services; recorded capabilities, availability, pagination, rate limits and failure behavior for each operation |
| Authentication | Credentialless public research readers; application-owned transient headers for imported HTTP operations and application-owned MCP client credentials; no model-owned secrets | Protected OAuth/account connection lifecycle, scoped grants, token refresh/revocation, tenant isolation and auditable credential resolution |
| Glioma research | Literature/trial registry/TCGA project metadata reconciliation; bounded local-model drafts; exact included-citation and metadata-field audits; explicit human-review holds | Composed molecular, imaging/pathology, longitudinal, single-cell/spatial and independent replication analyses using the existing typed tools; independent scientific entailment and claim-quality review |
| Evidence quality | Metadata provenance/omissions, explicit unknown states, preserved source partitions | Independent relevance/quality evaluation, contradiction and negative-result handling, cohort-overlap checks, freshness policies, source-specific quality failures |
| Product interfaces | Dependency-free SDK and local CLI commands | An editable workflow UI, authenticated service API, operator views for runs/holds/recovery, accessible errors and installation/onboarding flows |
| Reliability | Focused invariants, installed-wheel/API smoke, Python regressions, separate live source checks, contract checks on normalized dispatch and resume | Provider/source outage campaigns, process-termination recovery, task-quality and cost/latency measurements, cross-SDK contract support and release/deployment validation |

The eleven-adapter count names implemented reviewed readers, not eleven independent providers or
an assertion that every source is always available. PubMed and NCBI Gene, for example, share NCBI
infrastructure. Approved one-record live queries verified OpenAlex, Crossref and Zenodo in the
[research-source increment](RESEARCH_SOURCE_INTEGRATIONS.md); earlier live checks exercised the
composed PubMed/ClinicalTrials.gov/GDC lane. Other source adapters retain their fixture/parser
evidence independently of those live observations.

The three additional readers are explicitly registered in the SDK or selected with the CLI's
`--research-sources` option. The default catalogue remains stable for existing saved plans.

The [MCP bridge](WORKFLOW_MCP.md) registers only tools explicitly reviewed on an already connected
client. Its 225-tool local stdio fixture demonstrates protocol discovery, schema checking and
dispatch capacity. It does not represent 225 real external services. OpenAPI/MCP onboarding adds
execution mechanisms; provider-specific configuration and independent live verification are still
required before those mechanisms count toward the requested integration breadth.

## Observed local-model limits

With local Ollama `qwen2.5:1.5b`, a caller-contracted one-step PubMed task produced an accepted
plan with a two-record retrieval limit. A four-step PubMed plus ClinicalTrials.gov -> reconciliation
-> review task did not produce an accepted plan within three bounded planning attempts, even
after reference feedback. A grounded metadata draft was blocked for missing citations and a
clinical claim. The draft reused cached public reconciliation data; these checks dispatched zero
source calls and do not establish retrieval success for the newly proposed task.

These are caller-contract cases, not a held-out natural-language benchmark. Repeated prompt
checks are not independent task examples. The observations establish neither general multi-step
task fidelity nor scientific or clinical adequacy; the failed planning and draft audits remain
visible limitations.

## Next implementation order

1. Measure and improve local-model multi-step fidelity using a held-out public research task set
   and the [caller-bound requirement contract](WORKFLOW_REQUIREMENTS.md).
   Verify requested output stages, exact limits and source boundaries rather than only JSON shape.
   Keep planning failures and semantic omissions visible; do not turn a valid graph into a success
   claim for an unfulfilled task.
2. Extend the [implemented local glioma synthesis and audit](GLIOMA_WORKFLOW_SYNTHESIS.md) to richer
   reviewed evidence planes. Preserve exact citation closure, source identity, missingness,
   context omissions and explicit unassessed scientific support. Public human cohort summaries
   remain separate from the Rust preclinical worker boundary.
3. Expand actual integration onboarding through the reviewed OpenAPI and MCP bridges. Count a service only
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
