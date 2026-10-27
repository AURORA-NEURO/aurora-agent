# Autonomous workflow development plan

The requested end state is an autonomous natural-language workflow builder and executor with
hundreds of real integrations and deep glioma research support. The executable implementation is
growing. The original objective remains unfinished; neither generated
platform descriptors nor fixture registration counts establish integration coverage.

| Area | Current evidence | Required before claiming the requested end state |
| --- | --- | --- |
| Natural-language authoring | Local model -> schema/DAG validation; optional unreviewed NLP obligation suggestions with complete native input/output contracts, explicit future-work role instructions and durable caller acceptance; exact versioned request replay; caller-bound contracts and bounded repair; typed glioma source/limit grammar compiles deterministic multi-step graphs and retains unresolved clauses | Controlled actual-model evaluation, held-out complete intent extraction and general multi-step fidelity, broader clarification UX, branching and bounded iteration |
| Execution | Sequential read-only DAG, per-call budgets, public-value checkpoints, explicit retry, digest-checked resume; durable cooperative cancellation and separate release grant | Scheduling, concurrent independent branches, shared-worker fencing, effectful execution through the existing effect ledger, compensation/reconciliation policies |
| Integration breadth | Twenty concrete reviewed research adapter paths across seventeen declared API service groups; fixed operator-selected capability profiles; a versioned API-service/reader registry bound to actual executors; strict public GET, fixed read-only GraphQL POST and reviewed OpenAPI HTTPS transports; caller-connected stdio MCP sessions | Hundreds of independently configured and verified services; protected account onboarding, availability, pagination, rate limits and failure behavior for each operation |
| Authentication | Credentialless public readers; typed application-attested connection sessions bind subject/resource scope and revocable credential handles into imported HTTP catalogue identity; caller-owned MCP sessions | Provider-authenticated account onboarding, protected OAuth/token refresh, persistent account connections and tenant-bound private-response retention |
| Glioma research | Literature/trial registry/TCGA reconciliation; bounded local drafts and exact citation audits; separate molecular catalogues and association metadata; fixed TP53 aggregates, Ensembl/NCBI reference comparison, UniProt/Reactome annotation context, CIViC curation review and Cellosaurus metadata with native replay and review holds | Broader molecular measurements and independently validated crosswalks; imaging/pathology, longitudinal, single-cell/spatial and replication analyses using the existing typed tools; independent scientific entailment and claim-quality review |
| Evidence quality | Metadata provenance/omissions, explicit unknown states, preserved source partitions | Independent relevance/quality evaluation, contradiction and negative-result handling, cohort-overlap checks, freshness policies, source-specific quality failures |
| Product interfaces | Dependency-free SDK/CLI; opt-in authenticated local editor and HTTP routes through the existing Rust server and fixed Python worker, with retained task contracts, exact approval, run controls and explicit exports | Broader usability/accessibility evaluation, provider onboarding, release installation and production service operation |
| Reliability | Focused invariants, installed-wheel/API smoke, Python regressions, separate live source checks, contract checks on normalized dispatch and resume | Provider/source outage campaigns, process-termination recovery, task-quality and cost/latency measurements, cross-SDK contract support and release/deployment validation |

The twenty-path count names implemented reviewed readers, not twenty independent providers or
an assertion that every source is always available. PubMed and NCBI Gene, for example, share NCBI
infrastructure. Approved one-record live queries verified OpenAlex, Crossref and Zenodo in the
[research-source increment](RESEARCH_SOURCE_INTEGRATIONS.md); earlier live checks exercised the
composed PubMed/ClinicalTrials.gov/GDC lane. A later bounded molecular workflow observed
cBioPortal, NCBI Gene, Open Targets and GWAS Catalog; Europe PMC was separately observed.
Those eleven adapter paths have bounded live observations. Two new paths add the
[GDC TP53 aggregate reader](GDC_GENE_MUTATION_COUNTS.md) and
[Ensembl TP53 reference reader](ENSEMBL_GENE_REFERENCES.md). Their fixed compositions completed
bounded live runs: one GDC request yielded two project citations; two Ensembl requests plus one
NCBI Gene request yielded three source-reference citations. Both resumed local stages from retained
source results and replayed completed checkpoints with zero additional dispatches. The GDC count
reader shares its provider with the existing project reader. These observations establish neither
continuous availability nor thirteen independent providers.

The three additional readers are explicitly registered in the SDK or selected with the CLI's
`--research-sources` option. The default catalogue remains stable for existing saved plans.
The two TP53 paths are separately opt-in through `--glioma-mutations` and `--gene-references`;
their exact CLI templates also register the required operations without dispatch. A further pair,
[UniProt](UNIPROT_ANNOTATIONS.md) and [Reactome](REACTOME_PATHWAYS.md), provides fixed public
human P04637 name/pathway annotation context through [reviewed public HTTPS](REVIEWED_PUBLIC_HTTP.md).
The opt-in `--protein-pathways` composition keeps general source annotations separate from
glioma measurements, activity, enrichment and causal claims. A bounded live composition used two
GETs, retained four protein-name rows, two gene-name candidates, 46 pathway annotations and 53
citations, and replayed the completed checkpoint without further dispatch. All fifteen reader
paths now have bounded observations; this does not imply fifteen independent providers, continuous
availability, scientific validation or hundreds of services.

A sixteenth opt-in path, [GDC project SSM-data coverage](GDC_SSM_COVERAGE.md), retrieves public
project availability aggregates. Its [denominator evidence composition](GLIOMA_DENOMINATOR_WORKFLOW.md)
compares source-native count availability and necessary count ordering with the existing TP53
aggregates. It never substitutes project totals or associated-SSM counts for gene-specific assay
eligibility and never calculates prevalence. `--denominator-coverage` enables the reader/composition;
the fixed CLI template and browser choice also keep source/model authority separate.

Two further reviewed readers supply [HGNC TP53 identifiers](HGNC_GENE_REFERENCES.md) and
[NCI Thesaurus glioma terminology](NCI_THESAURUS.md). Their fixed
[capability profiles](WORKFLOW_CAPABILITIES.md) use one repeated startup/CLI selection rather
than a new launcher switch per adapter. The three-source
[identifier review](GLIOMA_GENE_IDENTITY_WORKFLOW.md) retains HGNC/Ensembl/NCBI native candidates,
missingness and exact metadata concordance without validating biological equivalence or joining
measurement planes. NCI ontology relations remain source terminology assertions.

A nineteenth optional [CIViC metadata reader](CIVIC_GLIOMA_EVIDENCE.md) adds accepted
glioblastoma-name evidence categories and source-publication identifiers through a fixed anonymous
GraphQL POST. Exact raw bytes, native pointers and missing/null distinctions support offline replay.
One fixed live POST returned 25 items from a source-reported filtered total of 45 with another
page indicated. Its exact bytes and native pointers replay offline. One-page item counts do not
establish independent publications or biological denominators, continuous availability or source
completeness. The separate [curation review composition](GLIOMA_CIVIC_CURATION_WORKFLOW.md) retains
every admitted item, missing/null state and exact native source-ID group through local analysis
and a research-review handoff. A retained-response derivative checked 431 value citations across
those 25 items with zero new source calls. Its 18 observed source IDs are not an independent
publication count; this derivative was not a new acquisition or a full workflow execution.

A twentieth optional [Cellosaurus metadata reader](CELLOSAURUS_CELL_LINE_METADATA.md) requests one
fixed five-row human/glioblastoma-name page. It retains primary accessions, identifier/synonym
arrays, source taxonomy/disease assertions, dates and the admitted problematic-cell-line comment
category. A separately retained format-inspection GET informed the strict reader contract. That
capture establishes the observed response format, not a live execution of this new adapter.
A separate caller-supplied callback admitted those exact retained bytes with five records and
208 native citations; an independent offline check resolved every citation and missing-field
context. It retained the original format-capture clock and made zero new HTTP/model calls.
This establishes retained-response admission, not fresh default-reader conformance.
Unknown comment categories, invalid wrappers and over-bound structures refuse as a whole.
Missing optional comments remain missing; their absence establishes no absence of cell-line
identity problems. Metadata does not validate authenticity, model suitability or disease
equivalence. Pagination, total count, release identity and scientific relevance remain unassessed.

The [provider registry](WORKFLOW_PROVIDER_REGISTRY.md) manifest `reviewed-public-readers/2026-10-27.4` binds twenty reviewed reader paths to
seventeen explicitly reviewed API service groups, with NCBI and GDC readers deduplicated. The eight
default readers and their seven groups remain unchanged; twelve readers are optional. It checks
actual operation/default-executor/component identities, supports source-free request conformance
inspection, and retains admitted adapter attempts separately from actual HTTP counts. Application
reviewed group declarations and referenced evidence never become independently verified provider
coverage. Protected provider onboarding, OAuth, distributed quotas/pagination and hundreds of real
services remain unfinished. Reviewed OpenAPI HTTPS now has bounded DNS jobs, absolute I/O deadlines,
strict shared HTTP framing, TLS-before-credential admission and typed sanitized rate failures;
these transport checks do not establish a real protected-provider connection.

An opt-in [incremental local planner](INCREMENTAL_WORKFLOW_PLANNING.md) chooses one operation
then its arguments/bindings, and finishes with model-selected outputs. Generated step symbols do
not choose the workflow. Its final graph passes the unchanged caller-contract compiler; partial
prefixes carry no executable plan. Bounded clarification, refusal and budget exhaustion preserve
receipts without substituting a template. This is a new protocol, not evidence of broad NLP success.

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
   A fresh paired 0.2/0.3 protocol probe accepted 1/5 versus 4/5 constrained positive cases,
   including a synthetic schema fixture, but full intent controls regressed from 2/3 to 1/3.
   Both protocols could compile an available reader while omitting an essential unavailable
   writer; 0.3 also silently substituted a permitted mode for an essential incompatible request.
   Next work must represent and check task obligations beyond available-operation contracts.
   Native graph validity and positive-case gains do not establish general NLP autonomy.
   [Intent extraction](WORKFLOW_INTENT_EXTRACTION.md) supplies every selected operation's complete
   native descriptor, including output schemas, and validates retained requests against their
   exact protocol version. The prior available-HGNC refusal under protocol 0.1 remains failed;
   omitted output contracts were a context gap, not a proven cause. A separate protocol 0.2
   Cellosaurus task also refused available declared work after seeing the full contract,
   citing the model's own execution access. Protocol 0.3 adds generic instructions to interpret
   future workflow work independently of the model's current callable tools. Role confusion is
   a hypothesis, and this change establishes no remedy or general reliability gain. Both
   refusals retain their original versions and bytes. Semantic controls and independent
   evaluation remain required; candidate syntax and a blocked unsupported action are separate
   from full task coverage, caller acceptance and successful workflow execution.
2. Extend the [implemented local glioma synthesis and audit](GLIOMA_WORKFLOW_SYNTHESIS.md) to richer
   reviewed evidence planes. Preserve exact citation closure, source identity, missingness,
   context omissions and explicit unassessed scientific support. Public human cohort summaries
   remain separate from the Rust preclinical worker boundary.
   The documented GDC aggregation is now implemented as the separate
   [fixed TP53 mutation-count composition](GLIOMA_MUTATION_WORKFLOW.md), retaining TCGA-GBM/LGG
   partitions and exact native count pointers. Missing buckets remain unknown; counts do not
   establish a gene-specific assay denominator, prevalence, independent cohorts or clinical
   significance. The [source-reference comparison](GLIOMA_GENE_REFERENCES.md) retains every
   Ensembl Entrez candidate and exact NCBI identifier agreement/mismatch without validating
   scientific identifier equivalence or joining measurement planes. Existing molecular catalogue
   schemas remain catalogue-only. Next increments need richer independently reviewed measurements,
   explicit denominators and source-release/identifier validation before cross-source analyses.
3. Expand actual integration onboarding through the reviewed OpenAPI and MCP bridges. Count a service only
   after its auth/scopes, actual request/response behavior and failure handling are verified; use
   representative integration families rather than generated labels.
4. Add effectful workflow operations through the repository's existing authorization/effect
   reconciliation contracts. Bind approval to the exact operation, request, destination and plan;
   preserve uncertain external completion across crashes instead of automatically repeating writes.
5. Add graph editing and operational controls, then concurrency/scheduling and shared execution.
   Validate the installed application and deployment artifacts before claiming production readiness.
   A live identifier-template failure exposed an immutable source-call cap that correctly blocks
   another attempt in the same plan. A separately compiled composition can retain validated source
   evidence and its original acquisition clock; it does not establish recovery of that failed
   checkpoint. Any future recovery design must preserve the original approval, failed attempt,
   call accounting and source provenance rather than silently enlarging its budget.

## Completion evidence

Completion requires demonstrations that a user request produces the requested graph and outputs,
survives interruption without duplicate completed effects, uses real configured integrations at
the requested scale, and produces glioma research artifacts with auditable source closure and
explicit unresolved questions. Tests of only the registry, schema validator, simulator, or one
retrieval cannot establish that broader result. Research tools continue to require qualified
scientific review and do not make individual diagnosis or treatment decisions.
