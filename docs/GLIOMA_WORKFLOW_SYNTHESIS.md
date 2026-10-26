# Local-model glioma research drafts

`python/prism_sdk/glioma_workflow_synthesis.py` extends the public glioma evidence workflow with a
bounded local-model draft and a deterministic citation/metadata audit. This repository-defined
composition follows `LocalNeurosurgicalAgent`'s existing separation of source context, structured
claims, citation closure, and qualified review. It does not replace the Rust source validators or
claim medical or scientific validity from citation closure.

```text
PubMed + ClinicalTrials.gov + optional aggregate TCGA project metadata
    → reconciliation → bounded local-model draft → exact citation/metadata audit
```

The registered operations are `glioma.evidence.synthesize` and `glioma.evidence.audit_draft`.
`glioma_grounded_workflow_proposal(question, **template_options)` composes them with the existing
retrieval and reconciliation stages. It removes the earlier metadata-only review step and exposes
the final audit as the workflow output.

```python
from prism_sdk.glioma_workflow import register_glioma_workflow_operations
from prism_sdk.glioma_workflow_synthesis import (
    glioma_grounded_workflow_proposal,
    register_glioma_synthesis_operations,
)
from prism_sdk.llm_runtime import LLMRuntime, ollama_provider
from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan
from prism_sdk.workflow_integrations import builtin_workflow_operation_catalog

runtime = LLMRuntime()
runtime.register_provider(ollama_provider())  # An already running local Ollama service.
catalog = register_glioma_workflow_operations(builtin_workflow_operation_catalog())
register_glioma_synthesis_operations(
    catalog,
    runtime=runtime,
    model="YOUR_INSTALLED_LOCAL_MODEL",
    provider="ollama",
    approve_provider_call=True,  # Application-owned model authority.
    max_records=32,
    max_context_bytes=48_000,
    max_output_tokens=2048,
)
question = "Review public glioma evidence themes and missing molecular measurements"
plan = compile_workflow_plan(
    glioma_grounded_workflow_proposal(question), catalog, task=question
)
run = execute_workflow_plan(plan, catalog, approved=True)  # Separate source authority.
audit = run.outputs.get("audit")
```

Registration is pure and makes no provider or source request. The runtime and model must be supplied
by the application. Only registered credentialless in-memory or loopback HTTP providers are admitted.
Model approval is captured separately from source approval: source-dispatch approval cannot grant a
model call. Model identity, configuration fingerprint, context caps, and output-token cap contribute
to catalogue identity, so changing them invalidates an earlier bound plan. Captured approval is
invocation authority and does not change capability identity; an unapproved registration can be
recreated with approval while preserving the plan's model binding.
The runtime's own retry policy remains in force under that captured configuration.

For direct use, construct `LocalGliomaWorkflowSynthesizer(runtime, model, provider)` and call
`synthesize(reconciliation, approve_provider_call=True)`. The resulting artifact still requires
`audit_glioma_synthesis_draft(synthesis)`. Applications can supply `expected_model_binding` to that
audit; registered audit executors always bind to their configured model. Provider transport errors,
invalid structured responses, configuration drift, and invalid source context fail explicitly with
no synthetic fallback. An empty admitted record set returns `needs_evidence` without invoking a
model.

The context is richer than a count summary but remains metadata. It includes exact citation titles,
journals, partial dates, DOI state, publication/MeSH terms, registry title/status/phase/date/enrollment
metadata, and aggregate GDC project/category metadata. It carries source authority, URI, retrieval
time, source/record/bundle digests, and the original missing-data states. The earlier reconciliation
does not retain abstract text, so this module does not supply or claim abstract/full-text context.
It never retrieves cases, controlled-access material, molecular values, or laboratory instructions.

Selection is deterministic and alternates across source partitions before admitting further rows.
The caller sets a record cap from 1 to 128 and a serialized context cap from 4096 to 64,000 UTF-8
bytes. Records that do not fit remain omitted rather than being partially rewritten. Omitted counts
and the digest of omitted citation identities remain explicit. The retained reconciliation is
limited to 1,000,000 bytes, and complete synthesis artifacts are bounded to 2,000,000 bytes.

The model receives a stable `citation_id` for each included record. Only those exact included IDs
are available for claims. A valid PMID, trial ID, or TCGA project that exists in the original
reconciliation but was omitted by the context budget cannot pass this audit. Source titles and
question text are untrusted data, and no tools or additional source calls are exposed to the draft.

Model output follows the existing `answer`, `unknowns`, and `claims` convention. Each claim declares
its kind, scope, evidence state, exact citations, and field assertions:

```json
{
  "claim_id": "registry-status",
  "kind": "source_observation",
  "scope": "public_record_metadata",
  "text": "Unverified prose describing the cited registry status.",
  "evidence_state": "metadata_observation",
  "explicitly_hypothetical": false,
  "citations": ["cite:<the exact supplied identity>"],
  "assertions": [
    {
      "citation_id": "cite:<the exact supplied identity>",
      "field": "overall_status",
      "value": "RECRUITING"
    }
  ]
}
```

Every assertion must equal the cited supplied metadata value with its exact JSON type. Missing/null
metadata must remain `unknown`; numeric zero is not boolean false. A literal false field can support
only a negative metadata observation, such as a source-declared release flag. It never establishes a
negative biological result. Negative biological evidence remains `unassessed` with no fabricated
observations. Unknown source fields, empty sources, context omissions, and unassessed research
dimensions are copied into the final audit independently of what the model chooses to mention.

The audit blocks unknown/out-of-context citation IDs, mismatched field assertions, assertion/citation
binding errors, duplicate claim identities, patient scope, clinical-action claims, and attempts to
promote a scientific summary into an observed metadata fact. Hypotheses must remain explicitly
hypothetical. Free-form statements are not checked for scientific entailment. Even a passing exact
metadata audit labels every claim's semantic entailment and scientific validity `unassessed`,
returns prose only as `unverified_model_answer`, and ends at `held_for_scientific_human_review`.
An audit with violations returns `blocked` and does not publish the answer in its final handoff.

Completion of workflow dispatch is separate from acceptance of a draft. The final artifact never
grants diagnosis, treatment, prognosis, trial matching, laboratory, or release authority. Model
output is untrusted research draft material, including when it uses valid citations. Public human
metadata never enters the Rust preclinical engine through this module.

Synthesis artifacts retain the reconciliation, exact context, model/request binding, structured draft,
response/claim digests, and outer artifact digest. Audit replays context selection and field
projection from that reconciliation before evaluating claims. It rejects isolated restamped context,
record, question, response, or model-binding changes. These are self-computed integrity receipts,
not signatures, source authentication, model attestation, or protection against an actor able to
replace every input and recompute every digest. Source authenticity and scientific review remain
separate requirements. Explicit workflow checkpoints retain public metadata and unverified draft
text so completed synthesis steps can resume without another model invocation.

Run the dependency-free tests from the repository root:

```powershell
$env:PYTHONPATH = (Join-Path (Get-Location) 'python')
python -m unittest discover -s python/tests -p test_glioma_workflow_synthesis.py -v
```
