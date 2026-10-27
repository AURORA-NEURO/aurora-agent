# Autonomous workflow development plan

The requested end state is an autonomous natural-language workflow builder and executor with
hundreds of real integrations and deep glioma research support. The executable implementation is
growing. The original objective remains unfinished; neither generated
platform descriptors nor fixture registration counts establish integration coverage.

| Area | Current evidence | Required before claiming the requested end state |
| --- | --- | --- |
| Natural-language authoring | Local model -> schema/DAG validation; caller-bound contracts and bounded repair; typed glioma source/limit grammar compiles deterministic multi-step graphs and retains unresolved clauses | Held-out general multi-step task fidelity, broader intent extraction and clarification UX, graph editing, branching and bounded iteration |
| Execution | Sequential read-only DAG, per-call budgets, public-value checkpoints, explicit retry, digest-checked resume; durable cooperative cancellation and separate release grant | Scheduling, concurrent independent branches, shared-worker fencing, effectful execution through the existing effect ledger, compensation/reconciliation policies |
| Integration breadth | Eleven concrete reviewed research adapters, including OpenAlex, Crossref and Zenodo; reviewed GET OpenAPI imports; reviewed tools from caller-connected stdio MCP sessions | Hundreds of independently configured and verified services; recorded capabilities, availability, pagination, rate limits and failure behavior for each operation |
| Authentication | Credentialless public readers; typed application-attested connection sessions bind subject/resource scope and revocable credential handles into imported HTTP catalogue identity; caller-owned MCP sessions | Provider-authenticated account onboarding, protected OAuth/token refresh, persistent account connections and tenant-bound private-response retention |
| Glioma research | Literature/trial registry/TCGA reconciliation; bounded local drafts and exact citation audits; separate molecular-profile catalogues, Entrez identities and Open Targets/GWAS association metadata with native replay and review holds | Actual molecular measurements and validated crosswalks; imaging/pathology, longitudinal, single-cell/spatial and independent replication analyses using the existing typed tools; independent scientific entailment and claim-quality review |
| Evidence quality | Metadata provenance/omissions, explicit unknown states, preserved source partitions | Independent relevance/quality evaluation, contradiction and negative-result handling, cohort-overlap checks, freshness policies, source-specific quality failures |
| Product interfaces | Dependency-free SDK/CLI; opt-in authenticated local editor and HTTP routes through the existing Rust server and fixed Python worker, with retained task contracts, exact approval, run controls and explicit exports | Broader usability/accessibility evaluation, provider onboarding, release installation and production service operation |
| Reliability | Focused invariants, installed-wheel/API smoke, Python regressions, separate live source checks, contract checks on normalized dispatch and resume | Provider/source outage campaigns, process-termination recovery, task-quality and cost/latency measurements, cross-SDK contract support and release/deployment validation |

The eleven-adapter count names implemented reviewed readers, not eleven independent providers or
an assertion that every source is always available. PubMed and NCBI Gene, for example, share NCBI
infrastructure. Approved one-record live queries verified OpenAlex, Crossref and Zenodo in the
[research-source increment](RESEARCH_SOURCE_INTEGRATIONS.md); earlier live checks exercised the
composed PubMed/ClinicalTrials.gov/GDC lane. A later bounded molecular workflow observed
cBioPortal, NCBI Gene, Open Targets and GWAS Catalog; Europe PMC was separately observed.
All eleven current adapter paths therefore have bounded live observations. These are observations
of those requests, not a continuous availability guarantee or eleven independent providers.

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
forbidden clinical/patient label. The draft reused cached public reconciliation data; these checks dispatched zero
source calls and do not establish retrieval success for the newly proposed task.

These are caller-contract cases, not a held-out natural-language benchmark. Repeated prompt
checks are not independent task examples. The observations establish neither general multi-step
task fidelity nor scientific or clinical adequacy; the failed planning and draft audits remain
visible limitations.
After tightening the generation schema to admitted research labels and exact citation IDs, a
later small-model draft was refused by runtime structured-output validation. No repaired scientific
answer was substituted for that refusal.

The [typed glioma interpreter](GLIOMA_WORKFLOW_INTENT.md) is a separate model-free planning mode.
It compiles supported source/limit clauses into the existing graph and binds normalized arguments,
required outputs and value flow. Unsupported dates, molecular subtypes, interventions, effects,
clinical requests and conflicting clauses retain their original spans and prevent compilation.
This addresses graph wiring for that documented grammar; it is not unrestricted NLP or a held-out
task benchmark. Additional caller contracts are conjoined, with tighter bounds and conflicting
requirements refused instead of replaced.

A later prospective four-class caller-contract comparison accepted two classes each with
`qwen2.5:1.5b` and `qwen2.5:7b`, using six and four provider calls respectively and zero source
dispatches. Both passed bounded single-source and independent two-output plans. The larger model
invented child output pointers for review graphs; an independent graph audit rejected them despite
legacy generic flow checks accepting their source identities. The comparison does not justify a
default-model change or establish general NLP comprehension. The frozen failures remain failures.

Reference-path checks now reject paths proven impossible by a producer's closed schema. Open or
optional output paths remain runtime checks, not successful existence proofs. Requirements `0.2`
can constrain the exact producer pointer and consumer argument path; newly compiled typed glioma
graphs use complete native bundles at those paths. Legacy contracts and plan digests stay stable.

The [molecular metadata workflow](GLIOMA_MOLECULAR_WORKFLOW.md) completed six stages in a bounded
live run with nine source requests and 46 citations. Its native source planes remain separate;
profile availability is not a measured alteration, Entrez and Ensembl remain uncrosswalked,
association rankings are not causality, and cohort overlap remains unknown. The report awaits
scientific human review. A local replay made zero additional source/model requests.

[Cancellation](WORKFLOW_CANCELLATION.md) prevents new dispatch, drains an admitted synchronous
call and retains its actual result or failure. Checkpoint 0.2 retains cancellation through reload;
releasing it grants neither source authority nor failed-call retry. The CLI supports a separate
cancel signal file. This is a serial step-boundary control, not remote rollback or effect recovery.

The [connection session](WORKFLOW_CONNECTIONS.md) checks application subject, fixed operation
resources, expiry, credential availability and revocation before imported requests and catalogue
replay. Its account identity is application-attested, not verified by a provider. The session
nonce is independent of credential bytes; replacing a session requires a new plan. Legacy copied
header mappings have no account verification or revocation lifecycle. Protected OAuth, refresh,
restart-stable connection identity and private data retention remain required work.

The [local artifact sink](WORKFLOW_ARTIFACT.md) is a concrete independent application API for one
approved canonical JSON publication. It uses persistent authorization CAS, the SQLite effect
journal, an OS writer lock and filesystem no-clobber publication. Exact-byte observation can
settle interrupted completion; absent or conflicting files do not trigger automatic redispatch.
This is not yet a write operation in executable workflow plans. Distributed ownership, remote
effects, compensation, effect retries and power-loss durability remain unproven or unimplemented.

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
   A candidate next measurement increment is a separate reader for the documented GDC
   [`/analysis/top_cases_counts_by_genes` aggregation](https://docs.gdc.cancer.gov/API/Users_Guide/Data_Analysis/#analysis-endpoints),
   retaining a reviewed gene and TCGA-GBM/LGG project partitions. This is planned, not an
   implemented or live-verified source. Missing buckets must remain unknown; project-level
   counts of cases with SSM data do not establish a gene-specific assay denominator, prevalence,
   independent cohorts, or clinical significance. Existing molecular catalogue schemas remain
   catalogue-only.
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
