# Reviewed workflow capability profiles

`python/prism_sdk/workflow_capabilities.py` groups concrete reviewed readers and local
compositions behind fixed operator-selected IDs. This repository-defined application extension
has no blueprint module identifier. It avoids adding a launcher switch for every future adapter.
A profile is an application configuration unit, never a provider, a live verification, an account
connection, or permission to call a model or source.

| Profile | Concrete operations | Fixed plan |
| --- | --- | --- |
| `gene-identity` | HGNC TP53 references, Ensembl TP53 references, NCBI TP53 summary, native concordance analysis and review handoff | Five stages; metadata agreement remains separate from scientific equivalence |
| `glioma-ontology` | NCI EVS NCIt Glioma C3059 lookup | One native source output; no descendant retrieval, clinical interpretation or cohort inference |
| `civic-glioblastoma-evidence` | Fixed CIViC accepted glioblastoma-name evidence metadata window | One bounded first page; curation categories are not scientific support or patient/cohort/assay denominators |

The default eight-reader catalogue and saved-plan digest are unchanged. A selected profile adds
only its listed concrete operations, including dependencies. Overlapping normalized descriptors
must match canonical JSON bytes; Python equality cannot collapse `true` with `1`, `false` with
`0`, or an integer default with its floating representation. Existing executor bindings are
preserved rather than replaced. Catalogue
`subset()` retains exact bindings, while `extended()` constructs a new validated catalogue so a
descriptor conflict or capacity error leaves both inputs unchanged. Matching descriptors alone
do not validate an executor; the [provider registry](WORKFLOW_PROVIDER_REGISTRY.md) performs its
separate default-executor conformance inspection.

## SDK and CLI

```python
from prism_sdk import builtin_workflow_operation_catalog
from prism_sdk.workflow_capabilities import with_workflow_capabilities, compile_workflow_capability

selected = ("gene-identity", "glioma-ontology")
catalog = with_workflow_capabilities(builtin_workflow_operation_catalog(), selected)
plan = compile_workflow_capability(
    "Inspect the documented fixed public TP53 identifier evidence",
    catalog, "gene-identity", enabled_profiles=selected,
)
```

```console
aurora-agent workflow-catalogue --capability gene-identity --capability glioma-ontology
aurora-agent workflow-template --template gene-identity --task "Inspect fixed TP53 references" --out identity-plan.json
aurora-agent workflow-template --template glioma-ontology --task "Inspect NCIt glioma terminology" --out ontology-plan.json
```

Selecting a CLI template explicitly selects that reviewed profile for this command. To execute
or resume its saved plan, supply the same capability selections so the complete catalogue still
matches. Model builders can also choose registered operations through their ordinary allowlist;
they do not receive a prepared profile graph. Fixed templates remain a separate model-free mode.
Neither catalogue inspection nor template compilation makes a network or model request.

All profile IDs validate before any extension loads. Unknown IDs, duplicates, non-string values
and oversized selections fail; arbitrary module, class, URL and command names are never accepted.
The 32-selection bound is capacity, not evidence that 32 integrations exist.

## Local application

The fixed worker accepts repeated startup `--capability <reviewed-id>`. The Rust launcher forwards
repeated `--workflow-capability <reviewed-id>` as separate arguments to that fixed worker. Rust
bounds names and count; Python owns the reviewed ID allowlist and rejects unknown selections
before creating plan/run storage. Existing individual startup flags remain supported.

The catalogue response adds a `capabilities` inventory. Each profile distinguishes operator
selection, exact descriptor presence, missing or changed operations, unestablished executor
conformance and unmeasured source availability. The browser builds inert text options from this
inventory and enables an option only when the operator selected it and every required operation
is present. Disconnect clears the dynamic options and invalidates an unavailable selection.

A plan request uses `planner: "capability:gene-identity"`,
`planner: "capability:glioma-ontology"` or `planner: "capability:civic-glioblastoma-evidence"`.
The request cannot supply capability configuration or
enable a disabled profile. The worker compiles the fixed graph with its native requirements,
conjoins any caller contract and step limit, and retains the exact planner selector in the saved
plan. Task text labels the review; it does not change fixed source selectors. Model and source
approval stay separate; inspection and planning grant neither.

## Limits

These three profiles do not establish hundreds of real integrations, protected account onboarding,
availability, scientific identifier equivalence, glioma classification, clinical support, or
effectful DAG execution. [HGNC](HGNC_GENE_REFERENCES.md), the
[three-source identifier composition](GLIOMA_GENE_IDENTITY_WORKFLOW.md), and
[NCI Thesaurus](NCI_THESAURUS.md) and [CIViC](CIVIC_GLIOMA_EVIDENCE.md) retain their own native evidence and omissions. Real source
observations, offline replay, fixture tests and model planning measurements remain separate.
