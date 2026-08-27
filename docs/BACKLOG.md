# Remaining backlog

48 code-bearing blueprint modules are not yet cited by any crate or design note,
across 9 sections. This is the enumerated form of `docs/COVERAGE.md`'s
percentage: a percentage says how far there is to go, a list says what is actually left.

Regenerate with `tools/backlog.sh`. Programme sections are excluded for the same reason they
are excluded from the coverage denominator — they specify no behaviour.

A module leaves this list when a crate cites it, which is the same weak criterion coverage
uses. This file tracks *attention*, not completeness.

Both high-level SDK agents now also expose a coordinated persistence lifecycle. Restore and flush
compose model inventory, runtime health, provider/model health when a coordinator exists, redacted
activation, selection-promotion authority, evaluator calibration, memory, online learning, prompt
learning, capability replay journals, route/planning/evaluation decision-cycle checkpoints, and
long-horizon execution checkpoints in a fixed dependency order, then return one digest-bound
component report. Strict
failures preserve the typed redacted report; non-strict passes expose unconfigured and
not-attempted components, while `require_all`/`requireAll` makes missing coordinators fail closed.
Activation restore preserves revocation, identity, and monotonic revision fences, while a supplied
selection store requires a configured selection lifecycle. Model inventory flush re-commits its
last validated image only after a live catalogue digest check and never rediscoveries a provider.
The lifecycle explicitly retains per-component CAS/atomic-store semantics rather than claiming a
distributed cross-store transaction; deployment identity, approval ordering, encryption, and crash
recovery between independent writes remain caller-owned. Capability-journal restore also
rehydrates only metadata-only replay identities, and execution restore precedes admission so a
restarted worker cannot resume with its duplicate-call barrier empty. Decision-cycle restore now
precedes execution admission and flushes before capability replay state, while keeping all route,
planning, evaluation, learning, and settlement values digest-only. Neither checkpoint restores
prompts, provider responses, credentials, tool arguments, effect authority, or raw values;
cross-store interruption remains an explicit reconciliation case.

The TypeScript workflow path now reaches stage-contract parity with Python through
`AutonomousWorkflowStageExecutionPlan`: every blueprint carries digest-bound capability,
evidence, evaluator, and selected-tool metadata; staged dispatch rejects stale contracts and
unselected tools; and checkpoint/receipt projections retain the stage-plan digest. The remaining
deployment work is intentionally caller-owned: durable multi-host storage, provider/source
availability, evaluator authority, credential provisioning, and effect reconciliation.

The TypeScript autonomous facade now also has a digest-bound workflow portfolio compiler and
bounded executor. It composes explicit per-domain blueprints into a dependency-checked portfolio,
replays the non-executing compiler to detect task/request/workflow/plan drift after restart, and
dispatches verified ready items in deterministic waves through the ordinary model/provider/tool/
learning boundary. It still does not claim provider-specific source interpretation, external
validation, durable multi-host scheduling, or authorization; credentials and effect approval
remain caller-owned, and execution JSON retains only metadata while raw runs stay transient.
The portfolio now also has a restart-safe checkpoint/controller boundary. Settled item/result
digests can be rehydrated by a caller-owned private store and are validated before pending waves
resume, preventing completed provider work from being replayed. The checkpoint is metadata-only;
durable multi-host leases, external effect reconciliation, and provider-specific evidence adapters
remain separate integration work. Portfolio items now also bind the existing explicit evaluator,
idempotent learner settlement, and optional feedback outbox into the same twelve-domain surface.
Evaluator policy digests, learning episode identity, pending feedback, and settlement failure are
checkpoint-visible without persisting task text, evidence bodies, provider responses, or keys;
provider completion alone still never produces reward. A reusable TypeScript evaluator bridge now
routs caller-owned value-only evidence through the reviewed twelve-domain registry, binds its
contract catalogue digest into resumable learning policy, and refuses incomplete custom registry
coverage or cross-domain evidence; source acquisition, truth authority, and production evaluator
operations remain deployment work. A bounded feedback worker now drains the same value-only
outbox with conditional leases, receipt-backed crash recovery, retry/terminal-failure reporting,
and no provider replay; multi-host persistence and operational scheduling remain deployment work.

Portfolio admission is now an explicit provider-free layer above that executor. The TypeScript
`AutonomousAgent.admitWorkflowPortfolio()` API validates/replays a reviewed portfolio, composes
all-domain model/provider/credential readiness with shared selection constraints, optional
calibration and evidence-readiness gates, tool gaps, and dependency closure, then emits a bounded
`ready_for_approval`/`partial`/`blocked` digest. It returns eligible model-arm identities and
redacted remediation metadata but never freezes selection or authorizes a provider, connector,
tool, source, learner, or effect. `validateAutonomousWorkflowPortfolioAdmission()` supports
caller-owned persistence/display checks. Durable external storage, approval UX, live health
refresh, and multi-host admission leases remain deployment responsibilities.

The admission artifact now has in-memory, strict JSON, transactional JSON, Web Storage, and a
serialized controller with compare-and-swap fencing. Resumable portfolio checkpoints use schema
`0.3` and bind `admission_digest` into their input identity; an explicitly required admission is
validated before settled-item rehydration and held items cannot dispatch. This closes the local
restart/remote-handoff integrity seam while leaving storage encryption, transport, leases, and
approval UX to the embedding deployment.

Remote portfolio provider execution now has explicit `approval_required` job state and a caller
controlled requeue transition. Approval pauses cannot be reported as partial completion, and the
worker renews its lease while private resolution/provider execution is in flight; heartbeat loss
falls into the existing typed transport/reconciliation path. Production deployments still own
the actual queue transport, distributed lease clock, approval authorization, and secret manager.

The action-admission controller now has a complete keyless Python operator process in addition to
the TypeScript/Python library surfaces: provider-free action-plan compilation (including an
all-twelve-domain matrix), durable submit/status/review/handoff commands, exact-record optimistic
concurrency, canonical atomic file persistence, and downstream-only handoffs. It still does not
pretend that a reviewer digest is a credential or that a local file is distributed authorization;
deployment-owned identity, queue transport, encryption, worker rehydration, provider/source/tool
authority, evaluator truth, and effect approval remain explicit integration responsibilities.

The downstream action handoff now has a public replay verifier in both SDKs. Workers can
rehydrate a handoff and prove plan/admission continuity, admitted status, all-domain selected
and requested-domain closure, fixed downstream-gate identity, and the outer digest before
opening any later gate. The verifier intentionally does not promote a reviewer digest into
authorization or claim provider/source/evaluator/effect readiness; those deployment boundaries
remain explicit.

The TypeScript runtime now includes a keyless provider protocol conformance gate. It runs all seven
built-in provider presets through the actual request, credential, response, model-discovery, and SSE
stream boundaries using an intercepted fetch fixture, and refuses missing credentials before any
fixture dispatch. Reports are bounded, digest-addressed, and metadata-only; no API key, prompt,
request, response, or header is persisted. This validates protocol wiring in CI without claiming
live quota, model availability, provider uptime, or user credential readiness, which remain
deployment/runtime checks.

Python now has protocol-conformance parity through `run_provider_protocol_conformance()` and
`assert_provider_protocol_conformance(report)`. The gate runs all seven built-in provider families
through the real `LLMRuntime` using an ephemeral local loopback fixture, covering credential
refusal, provider-specific request/header policy, response normalization, model discovery, and
SSE streaming. Its 56 checks and per-provider call counts are metadata-only; the synthetic
credential and all request/response material are discarded before the digest is emitted. This is
still a local protocol gate, not live provider availability, quota, permission, or user-key
verification.

Python now also has a digest-bound workflow portfolio compiler. `AutonomousAgent.plan_workflow_portfolio`
composes explicit requests across all twelve reviewed domains, preserves dependency waves and
cycle/partial/required-domain coverage, and projects each task's workflow, evidence, model-capability,
and route identities without retaining task text or making provider/tool calls. The matching
`verify_workflow_portfolio` replay catches task, dependency, workflow, evidence, policy, and
catalogue drift after restart. Python now also exposes a bounded executor that replays the plan
before dispatch, schedules ready items in deterministic dependency waves, propagates failed and
approval states to dependents, and persists a metadata-only checkpoint after each wave. Restart
requires a caller-owned rehydration callback that proves each successful result digest before
dependent work resumes. Durable multi-host queues, lease ownership, and external authorization
are still deployment work.

Python portfolio admission now has parity with that boundary. `AutonomousAgent.admit_workflow_portfolio`
replays a reviewed plan, projects keyless readiness, model capability/constraint eligibility, optional
tool/evidence/calibration holds, and dependency-closed remediation into a bounded digest. It never
resolves credentials or dispatches a provider, tool, connector, source, learner, or effect. Passing
the admission image to `execute_workflow_portfolio` binds its digest into checkpoint identity, so a
restart without the same reviewed gate fails closed before rehydration or new work. Live model health,
approval UX, persistence encryption, distributed leases, and external authorization remain deployment
responsibilities.

Python now also has portfolio-level evidence supervision parity. `execute_workflow_portfolio_evidence`
composes the existing evidence runtime across provider dependency waves, enforces item-domain and
request-plan alignment, carries direct predecessor evidence digests, and keeps provider failure,
pending evaluation, reconciliation, and dependency omission explicit. The resumable variant binds
provider execution, evidence plan, request digests, evaluator identity, runtime policy, and item
metadata into metadata-only checkpoints; journals and value rehydration are required for replay,
so source adapters are never silently reacquired after a completed item. Local in-memory/JSON/CAS
storage and controller seams are included; distributed transactions, source retention, and
tenant-level authorization remain embedding-deployment work.

Python now closes the next operational gap with a lease-fenced portfolio evidence work queue.
`admit_autonomous_workflow_portfolio_evidence_work_items()` binds every item to the reviewed
portfolio, optional admission, provider execution, evidence plan, request digest, checkpoint, and
dependency wave. The local and CAS-backed queues enforce dependency closure, provider-status
holds, lease ownership/renewal, expiry reconciliation, bounded retry/backoff, evaluator-pending
requeue, cancellation, and metadata-only snapshots. JSON, transactional JSON, SQLite, local
flush, atomic reload/CAS coordination, and caller-owned workers are exported and exercised over
all twelve domains. The queue still does not provide distributed consensus, source/evaluator
authority, credential storage, or effect authorization; those remain deployment responsibilities.

Python evaluator calibration is now wired to the same learning gate. The provider-free
`calibrate_autonomous_evaluators()` harness normalizes caller-owned evidence through the reviewed
domain registry, uses deterministic calibration/holdout splits, computes bounded reliability bins,
Brier/ECE/MCE and coverage metrics, and returns aggregate-only digests. Replay detects evaluator
catalogue and case-set drift, while `admit_autonomous_evaluator_calibration()` is the explicit
domain-scoped `admit_learning`/`hold_learning` decision. `AutonomousAgent.readiness()` and
portfolio `readiness_options` accept the report so `require_calibrated_learning` cannot mistake
observed bandit pulls for calibrated evaluator quality. Canonical JSON, CAS JSON, SQLite, registry,
and restore/flush seams are included; labels, evidence, prompts, credentials, and provider values
remain caller-owned and are never persisted by the calibration subsystem.

Python delayed learning now has a single operational controller in
`autonomous_learning_controller.py`. `AutonomousLearningController` enforces calibration
admission before immediate episode/trajectory settlement, and again at queued-command dispatch,
so direct low-level calls cannot bypass the all-domain gate. Its value-only feedback outbox has
idempotent command digests, worker leases, stale-lease reconciliation, bounded retry/terminal
failure states, and explicit applied result digests. `AutonomousLearningFeedbackWorker` settles
precomputed evaluator decisions without provider replay; prompt/response/credential/tool/evidence
payloads are rejected before enqueue. In-memory, canonical JSON, CAS JSON, SQLite, and restore/
flush coordinator seams are exported and tested, including stale-writer fencing and lease
recovery. Durable encryption, distributed consensus/scheduling, evaluator truth, and external
authorization remain deployment responsibilities.

Python now also has deployment-readiness parity with the TypeScript façade. The
`AutonomousDeploymentReadinessAuditor` joins keyless agent readiness with credential-provisioning
metadata and caller-owned persistence, queue, approval, external-auth, and telemetry assertions.
It emits digest-bound capability and twelve-domain rows with explicit model, provider, credential,
tool, evidence, and learning blockers, while refusing secret-shaped input and performing no
provider, source, tool, queue, credential, or learning mutation. `agent.deployment_readiness()`
provides the application entrypoint and canonical report validator; deployment initialization,
encryption, distributed scheduling, external authorization, and source/evaluator authority remain
deployment responsibilities.

Online learner state now has a first-class TypeScript restart seam. The snapshot validator binds the
bandit state digest and outer snapshot digest, rejects unsupported or credential-shaped fields, and
the JSON/CAS/browser adapters provide stale-writer protection for UCB, epsilon-greedy, and Thompson
statistics plus credited evaluator outcome digests. This persists adaptation metadata without
persisting prompts, provider output, credentials, or evidence; evaluator authority, reward quality,
and multi-process storage ownership remain deployment concerns.

Evaluator feedback now has the same durable handoff. The TypeScript outbox snapshot is canonical,
digest-checked, duplicate-resistant, byte-bounded, and command-shape validated; JSON, browser,
transactional CAS, and a mutation-flushing persistence coordinator are exported. Leases, retries,
terminal failures, applied status, and settlement result digests survive a worker restart, and stale workers are
fenced before they can overwrite another learner's queue. This closes local evaluator-credit
recovery across all domains without persisting prompts, provider output, credentials, tool arguments,
or evidence; the embedding deployment still owns the backing store and distributed scheduling.

Settlement receipts now have a matching TypeScript persistence seam. Receipt snapshots are
allow-listed, digest-bound, duplicate-resistant, value-only, and byte-bounded; canonical JSON,
browser storage, transactional CAS, and a mutation-flushing coordinator are exported. The
coordinator can be supplied directly to `AutonomousLearningController`, fences stale receipt
writers, and rolls back local state after a failed durable write. Private episode/trajectory
material remains in its separate caller-owned state store, and production storage/replication
remain deployment responsibilities.

The companion episode/trajectory state store is now strict and restart-safe. Its JSON snapshot
validator checks allow-listed metadata, duplicate identities, value-only rows, digest integrity,
and byte bounds; canonical JSON, browser Web Storage, CAS, and serialized coordinator adapters are
exported. Pending single-domain episodes and cross-domain trajectories can be restored without
provider replay, while task text, prompts, responses, credentials, and raw evidence remain outside
the state image. The deployment still owns the actual database, encryption, and replication.

The objective layer now has the same restart discipline. Goal snapshots are strictly allow-listed,
canonical-digest validated, hash-chain checked, retention checked, and bounded before restore;
`JsonAutonomousGoalPersistence`, `TransactionalJsonAutonomousGoalPersistence`, and
`WebStorageAutonomousGoalTextStore` provide portable JSON, CAS-fenced, and browser storage seams.
The coordinator remembers the restored snapshot digest and rejects stale writers, so a restarted
worker cannot overwrite a newer goal lifecycle or evaluator/learning projection. Only value-only
goal state is persisted; prompts, provider outputs, tool arguments, evidence bodies, credentials,
and approval authority remain outside the SDK, and deployment still owns the backing store.

The new `AutonomousDeploymentReadinessAuditor` composes the agent's all-domain readiness report,
the protected `ProviderSetup` plan, and caller-owned deployment capability assertions into a
digest-bound onboarding/deployment audit. It explicitly reports model, provider, credential,
tool, evidence, learning, persistence, queue, approval, external-auth, and telemetry gates for
all twelve domains, while remaining provider/source/queue-free and never granting authority.
This closes the local “what is missing before deployment?” projection seam; actual database,
distributed queue, auth/session, telemetry, source truth, and approval implementations remain
deployment work.

`AutonomousHttpSnapshotTextStore` now supplies a bounded HTTPS/host-policy/timeout/cancellation
transport for the existing strict JSON and transactional CAS adapters. It supports all-domain
learner, evaluator, goal, evidence, admission, and remote-job snapshots without knowing their
schemas; conditional HTTP writes map cleanly to the existing stale-writer fence, and protected
header resolution is transient. The embedding service still owns atomic CAS semantics, tenant
isolation, encryption, authorization, backups, and distributed consensus.

Run-level operational traces now use the same transport safely. Strict JSON, browser, and
transactional run-trace persistence revalidate the hash chain and carry `snapshot_digest` through
restart and stale-writer recovery, so the deployment can retain decision/selection/provider
failure metadata remotely without creating a second telemetry format or persisting prompts,
responses, tools, evidence, or credentials. Collector/export policy and external observability
remain deployment-owned.

The evidence handoff now preserves that same identity explicitly. Portfolio evidence checkpoints
use schema `0.2` and carry the nullable provider admission digest; `requireAdmission: true`
refuses resumable evidence execution without a reviewed admission before journal replay or
acquisition. Dependency-aware evidence work-queue items also carry the admission digest, allowing
remote workers to verify plan → admission → provider execution → evidence continuity from
metadata alone. Queue storage, leases, approval, and source/evaluator authority remain caller
responsibilities.

The worker adapters now also accept the verified action dispatch handoff as the rehydration
boundary: the handoff digest is bound into the durable job identity, and sync/async workers
refuse domain, plan, admission, gate-list, or outer-digest drift before runner invocation.

Domain evidence adapters now also have a digest-bound selector. It supports deterministic
lexicographic routing, caller-supplied health/success/reward/latency/cost scoring, conservative
abstention, candidate/registry drift detection, and an explicit acquirer handoff without putting
source authorization or credentials into the selection plan. Provider-specific signal production,
approval UX, and durable operational health aggregation remain integration work.

The evidence adapter health loop is now implemented as a TypeScript reference subsystem. It records
digest-bound acquisition and evaluator observations, derives domain-scoped adaptive signals,
opens bounded failure circuits, wraps selected acquirers/evaluators, and persists a hash-chained
metadata-only snapshot. Restart restore and stale/tampered snapshot refusal are tested across all
twelve domains. Production applications still own the atomic backing store, approval UX, and any
provider-specific cost/health telemetry.

Adapter health persistence now also has a bounded canonical JSON/text seam, browser Web Storage
adapter, snapshot/event validation, and optional compare-and-swap fencing. The persistence
coordinator serializes local writes and rejects stale multi-host writers instead of overwriting
another host's newer health history. Filesystem/database ownership remains with the embedding
application because this dependency-free SDK does not assume a Node or browser storage runtime.

Evidence acquisition now also has a reusable bounded retry boundary. Typed transient failures from
the HTTP source bridge can retry with deterministic exponential delay, while authorization,
validation, unknown, and exhausted failures remain explicit. Per-attempt observations retain only
domain, attempt, stable failure class, latency, and delay; no original errors, requests, credentials,
or values are persisted. Caller approval, global rate policy, and source-specific retry semantics
remain authoritative.

Reviewed adapter failover now composes the retry boundary. A caller can explicitly budget bounded
same-run fallback across score-ordered, digest-verified candidates for every domain; no-budget,
non-transient, and authorization failures remain terminal. Failover projections retain only candidate
identity, manifest digest, rank, stable failure class, and counters, so fallback cannot silently
widen source scope or persist source payloads.

Evidence routing now also has an operational-readiness projection. The TypeScript
`AutonomousEvidenceReadinessAuditor` combines twelve-domain coverage, the current digest-bound
selection plan, optional persisted adapter health, and bounded retry/failover policy into a
metadata-only `ready`/`degraded`/`blocked`/`missing` report with explicit counts and digests. The
strict default requires observed health and refuses open circuits; a caller can explicitly choose
a permissive degraded posture for startup or review UI. The audit never dispatches a source or
provider and does not replace external liveness, credential, incident, or authorization systems.

The high-level TypeScript `AutonomousAgent.readiness()` projection now accepts that same evidence
registry, optional health store, and readiness-policy options. When configured, it composes the
auditor's twelve-domain status into the keyless readiness report, marks degraded or blocked
evidence routes as domain `partial`, and emits redacted remediation/digest metadata. The
integration preserves the no-dispatch guarantee and leaves source authorization to the reviewed
evidence execution controller.

The TypeScript evidence path now also has a job-level
`AutonomousEvidenceExecutionResumableController`. It persists approval, dispatch-pending,
evaluator-wait, partial/failure, reconciliation, and completion checkpoints as bounded metadata,
requires an explicit resolution after an uncertain restart, and reuses caller-rehydrated runtime
journals to replay completed source work without a second dispatch. JSON, transactional CAS, and
browser storage adapters reject stale writers and tampered digests; source values, requests,
credentials, and provider payloads remain caller-owned.
`AutonomousAgent.executeReviewedEvidenceResumable()` exposes this lifecycle without forcing
applications to construct the lower-level controller themselves.

The existing evidence-to-provider resumable controller can now opt into the same source
checkpoint with `evidenceCheckpointStore` and `evidenceJobId`. Provider approval may therefore
pause after source completion, while a restart rehydrates the source journal and proves zero
duplicate source dispatch before the provider boundary is separately resumed or reapproved.

Evidence routing now also has a reviewed execution controller. The TypeScript
`AutonomousEvidenceExecutionController` binds the evidence plan, selection, readiness image,
retry policy, and explicit failover budget into one reviewable plan; execution revalidates the
registry and readiness digest, requires explicit source-dispatch approval, and only then invokes
the existing evidence runtime. Projection, evaluator, journal, and value rehydration remain
caller-owned, and plan preparation performs zero source calls. This closes the composition gap
without claiming source truth, provider authorization, or durable external execution.

Provider-specific evidence semantics now also have an explicit
`AutonomousEvidenceProviderContractRegistry`: protocol, operation, domain/capability/source scope,
auth posture, freshness, pagination, and required request metadata are bound to an exact adapter
manifest and carried into the execution-plan digest. Every primary and fallback attempt validates
that contract before dispatch, and `AutonomousAgent` exposes reviewed prepare/execute helpers that
carry the contract and caller-owned health store through the same boundary. This still does not
implement provider clients, credential storage, source truth interpretation, or external
authentication/session resolution; those remain deployment-owned.

The TypeScript evidence boundary now also has an LLM-backed adapter bridge. It invokes a registered
provider through `LLMRuntime`, resolves only opaque caller-owned credential handles, preserves
provider/model health and invocation observers, supports static or transient model resolution,
schema-gated structured output, parser/projector hooks, and metadata-only idempotency keys. Parsed
credential-shaped response fields are rejected before transient evidence enters the runtime, and
offline tests exercise the bridge across all twelve autonomous domains. This closes provider-backed
evidence invocation without claiming source retrieval, model discovery, source truth, or domain
evaluation; those remain explicit caller-owned boundaries.

The high-level TypeScript facade now exposes `runWithReviewedEvidence()` as an explicit
source-to-brain composition for all twelve domains. It binds the reviewed evidence plan and
readiness digest to source approval, acquisition, projection/evaluation, transient prompt
assembly, model selection, and ordinary provider invocation while retaining independent source
and provider approval gates. The default prompt is metadata-only; a caller-owned callback may
project raw values transiently, and the returned digest projection excludes those values and the
provider response. Unsettled evidence blocks invocation unless the caller opts into the bounded
incomplete-evidence mode; offline tests cover the default, refusal, and all-domain paths.

The Python façade now exposes the same source-to-brain composition through
`AutonomousAgent.run_with_reviewed_evidence(...)`. It accepts a reviewed domain set, bounded
acquisition requests, caller-owned `acquirer`/`projector`/`evaluator` adapters, an optional
`AutonomousEvidenceRuntimeJournal`, and opaque credential/model handles. Three decisions remain
independent: `approve_source_dispatch` gates source calls, accepted evaluator settlement gates
the provider unless `allow_incomplete_evidence=True`, and `approve_provider_call` is forwarded to
the normal model-selection/provider boundary. `run_mode="domain"` gives deterministic single
domain execution, `run_mode="cross_domain"` binds 2--8 reviewed specialists, and the default
`run_mode="auto"` reuses route-first intake. `to_dict()` retains only digests, statuses, route
metadata, and retention posture; raw evidence values, prompt projections, and provider responses
remain transient caller-owned objects. Journal replay requires `rehydrate_value`, and missing
values become `reconciliation_required` rather than silently reacquiring a source. Credentialless
tests cover refusal, replay, redaction, and provider-backed execution across all twelve domain
plans, bringing the Python and TypeScript source-to-brain contracts into parity.

Python evidence-backed execution now also has the restart boundary that previously existed only
in the TypeScript façade. `run_resumable_evidence_backed(...)` and
`AutonomousEvidenceBackedController` persist a digest-bound checkpoint immediately before the
provider boundary, retain only plan/request/policy/result digests, and require the caller-owned
evidence journal for replay. `InMemoryAutonomousEvidenceBackedCheckpointStore`, canonical JSON
persistence, and transactional compare-and-swap persistence are available for local, browser,
and service adapters. A restored provider result must pass the exact checkpoint digest through
`rehydrate_provider_run`; otherwise the run remains `provider_reconciliation_required` until the
caller explicitly opts into `resume_provider=True`. All twelve domain plans are exercised
credentiallessly, including source replay, provider-pending recovery, tamper rejection, and the
no-duplicate-dispatch invariant.

The evidence-backed brain operation now has a restart-safe controller and checkpoint boundary.
`runAutonomousEvidenceBackedResumable()` and `AutonomousEvidenceBackedController` bind the task,
request set, run policy, evidence plan, prompt projection, and provider result to a bounded
metadata-only checkpoint. The shared execution controller hydrates append-only evidence journals
before dispatch, replaying completed source work without reacquisition while requiring caller-owned
value rehydration. Provider results are never replayed implicitly: a completed result must match a
caller rehydration digest, and a pending provider boundary requires an explicit resume decision.
In-memory, JSON, and CAS-fenced stores plus all-domain restart tests are included; production
applications still own durable storage, transient values, and provider outcome reconciliation.

The TypeScript autonomous agent now also has an opt-in `structuredDomainResponse` contract for every
built-in domain. It derives a digest-bound JSON Schema and prompt contract from the reviewed workflow,
requires ordered stage results and domain-specific answer fields, and semantically revalidates the
transient provider response after dispatch. Coding, browser, data, science, biomedical, neuroscience,
operations, enterprise, multi-agent, multimodal, cross-domain, and evaluation paths are covered
offline. This closes the generic-answer-to-domain-evaluator composition gap while preserving the
caller/evaluator distinction: structured model output is not external-world truth.

Evaluator calibration is now an explicit TypeScript subsystem. The provider-free
`AutonomousEvaluatorCalibrationHarness` computes deterministic calibration/holdout splits,
coverage and abstention, reliability bins, Brier score, expected and maximum calibration error,
threshold accuracy, and per-domain admission status for all twelve evaluator profiles. Reports
retain only evaluator, case-set, metric, and policy digests; replay recomputes the report without
provider or learner side effects. `assertAutonomousEvaluatorCalibrationReady()` and the offline
scenario `requireCalibratedLearning` option hold bandit settlement before provider execution when
holdout evidence is missing, miscalibrated, or incomplete. This is an evaluator-signal gate, not
external truth, reward synthesis, or a substitute for domain validation.

The calibration admission is also installed on `AutonomousLearningController`, not only the
offline scenario harness. Direct episode settlement, delayed-credit trajectories, workflow and
cross-domain settlement, and feedback-outbox dispatch recheck the admitted episode domain before
bandit mutation; a blocked run cannot even enqueue a learning command. The controller therefore
provides one opt-in gate for local, remote, and restart-replayed learning while preserving the
same evaluator-signal-only boundary.

Calibration report restore is policy-validated as well as digest-validated: bin counts,
coverage/abstention rates, aggregate totals, per-domain thresholds, missing coverage, gate
decision, and reason rows are recomputed from the retained projection. A caller can bind a new
digest to a forged `ready` projection, but the learning admission still refuses the inconsistent
report.

The keyless `AutonomousAgent.readiness()` projection now accepts the same calibration report and
required-learning flag. It exposes aggregate admitted/held counts and a redacted per-domain
admission reason before any provider, model-discovery, tool, or learner operation; a required
calibration hold moves the affected readiness row to `partial` and adds the remediation action.

`AutonomousEvaluatorCalibrationRegistry` now carries validated reports across restarts with
deterministic query projections and bounded metadata-only snapshots. In-memory, JSON, and
compare-and-swap JSON stores are included; restore revalidates every report and the snapshot
digest, while cases, labels, evidence, prompts, responses, and credentials remain caller-owned.

The response contract now emits a deterministic, replayable composition evaluation and a safe
`reward_input`. An explicit `AutonomousLearningController.settleStructuredResponse` helper routes
that signal through the same idempotent bandit/outbox settlement boundary as other evaluator
feedback. This makes response-format adaptation usable online while keeping the boundary honest:
the reward covers reporting integrity only, never task correctness, source truth, or external effects.

The TypeScript evidence boundary now also has a strict source-truth admission layer. A
`createAutonomousEvidenceSourceAcquirer` route binds one provider contract to explicit source
authority, status, observation/expiry timestamps, source/citation digests, and limitations. Its
freshness policy distinguishes accepted, partial, stale, unverified, and refused observations and
fails closed on future timestamps, missing required source identity, expired bounded caches, and
contract drift. `AutonomousEvidenceSourceLedger` provides a restart-verifiable metadata-only hash
chain for every admitted or refused attempt; raw values, locators, prompts, responses, and keys
remain caller-owned. Offline tests exercise all twelve domains plus stale/unverified/future,
multi-source-kind, digest-mismatch, tampered-chain, and reviewed-failover paths. The same gate can
now be installed inside each selected/fallback candidate, preserving the actual adapter and
contract identity in its source receipt. This closes the provenance/freshness composition gap
while preserving explicit domain evaluators and source authority boundaries.

The TypeScript SDK now also exposes `AutonomousEvidenceSourceReconciler`, a request-free,
digest-bound fan-out/fan-in plan for independent source routes. It binds route and metadata
digests, quorum, bounded concurrency, parent evidence, and a named normalizer version before
dispatch; execution requires approval and refuses route or normalizer drift. It retains separate
typed source failures, groups caller-normalized transient values by digest, and distinguishes
consensus, consensus-with-dissent, disagreement, insufficient evidence, and total failure without
claiming that quorum is truth. All twelve autonomous domains are covered offline, including
bounded concurrency and disagreement/secret-boundary tests. This closes the source-comparison and
provider-disagreement composition gap while keeping evaluator authority and domain semantics
caller-owned.

The source provenance ledger now also has portable restart persistence: canonical JSON/text storage,
transactional compare-and-swap fencing, bounded browser Web Storage, snapshot/head/digest validation,
contiguous chain enforcement, and stale-writer refusal. Restore still retains only source and result
metadata; source values, locators, prompts, responses, credentials, and provider sessions remain
caller-owned.

The TypeScript SDK now also provides `AutonomousDomainEvidenceSourceCatalogue`, which supplies a
versioned source profile and route-registration boundary for every autonomous domain. Profiles bind
source kinds, capabilities, operations, freshness/auth/pagination posture, normalizer identity,
quorum defaults, and explicit limitations; registered routes bind provider, source, contract, and
adapter digests without retaining query metadata or credentials. Requirement preparation filters
routes by domain and capability without dispatch, while approved execution revalidates route/profile
drift and delegates to bounded reconciliation. Offline tests cover all twelve domains, custom
profiles, required metadata, capability scope, approval, dissent, typed failures, secret rejection,
and restart drift. This closes the practical domain-to-source composition gap while leaving source
clients, credential sessions, evaluators, and truth authority caller-owned.

The TypeScript catalogue now also has a digest-bound `AutonomousEvidenceNormalizerRegistry`, with
`identity/1` and `builtin.<domain>.claim-projection/1` entries for all twelve domains. Default
catalogue execution resolves the registry rather than requiring an ad hoc callback, and prepared
plans fail closed when the registry changes. Claim projections retain only operation, bounded
shape/count/byte metadata, transient value and shape digests, and explicit limitations; unsafe
normalizer output and same-spec callback replacement are rejected before quorum.

The catalogue is now a first-class brain input through TypeScript
`AutonomousAgent.runWithDomainEvidenceCatalogue()`. It composes all selected workflow evidence
requirements into digest-bound catalogue reconciliations, applies bounded parallel source
dispatch, uses the built-in normalizer registry, and feeds a metadata-only evidence context into
the ordinary routing/prompt/model/provider path. Source approval, evidence settlement, provider
approval, and optional learning remain independent. A caller-owned prompt builder may explicitly
bridge transient values, while the result projection remains digest-only and rejects catalogue,
route, profile, or normalizer drift before dispatch.

The Python façade now provides the matching `AutonomousAgent.run_with_domain_evidence_catalogue(...)`
composition. It prepares every requirement for the selected domains, executes bounded catalogue
fan-out, carries plan/catalogue/normalizer digests into the result, and routes settled evidence
through domain, cross-domain, or automatic provider invocation. Source dispatch, evidence
settlement, and provider approval remain independent; the default prompt is metadata-only and an
explicit prompt builder is the sole opt-in for transient raw values. The Python result is also
metadata-only when serialized and preserves the existing memory/learning options at the provider
boundary. Offline parity tests cover all twelve domains, approval pauses, dissent blocking, raw
value retention, and catalogue drift.

The TypeScript SDK now also exposes `registerAutonomousDomainHttpEvidenceSource`, which composes
the bounded HTTP transport with a typed domain source profile and catalogue route. It binds optional
adapter manifests, source/provider identities, endpoint/request/header resolvers, and explicit
host/scheme/method/size/timeout policy without dispatching during registration. Approved execution
then reaches the same all-domain reconciliation path; offline tests exercise twelve-domain success,
approval pauses, transient header handling, auth refusal, unsafe endpoint policy, and metadata
redaction. This closes the concrete HTTP-to-domain evidence bridge while keeping endpoint clients,
credential sessions, response interpretation, and evaluator authority caller-owned.

The HTTP bridge can now optionally bind `AutonomousEvidenceProviderContractRegistry` contracts at
registration time. The registered route carries the contract digest, while the actual acquirer
enforces protocol, operation metadata, capability, freshness, pagination, and auth posture before
the HTTP adapter is reached. Additive unrelated adapters no longer invalidate a bound contract;
replacement of its bound adapter still fails closed. All twelve offline HTTP paths exercise this
contract-backed composition.

The TypeScript layer now adds a provider-neutral HTTP source preset/matrix on top of that bridge.
`builtinAutonomousDomainHttpSourcePresets()` derives one digest-bound preset from every reviewed
domain evidence profile, while `registerAutonomousDomainHttpSourceMatrix()` validates complete
twelve-domain coverage, prevents duplicate route identities, auto-binds matching provider
contracts, and preserves the no-dispatch registration boundary. Callers still provide endpoint,
request, header/credential, fetch, and response interpretation functions; the presets do not
invent provider URLs or truth authority. Offline tests exercise registration, approved execution,
secret-boundary rejection, stale profile rejection, incomplete matrix refusal, and transient
header redaction for every domain. Workflow selection confidence is also now part of the durable
stage invocation and replay contract, so an ambiguity floor cannot be silently dropped on a
workflow stage or resume.

The portfolio now also has a bounded evidence supervisor. It verifies successful provider items,
rejects cross-domain requests, scopes each evidence runtime to its item's domain, preserves direct
predecessor evidence digests, and executes acquisition/projection/evaluation in the portfolio's
dependency waves. Per-item journals support replay with caller-owned value reconciliation, while
approval refusals, acquisition failures, pending evaluation, and downstream omissions remain
explicit. JSON is metadata-only; source acquisition, evaluator authority, and durable journal
storage remain application-owned.

Portfolio evidence now also has a digest-bound restart controller. It flushes metadata-only wave
checkpoints bound to the exact provider execution, evidence plan, request set, evaluator identity,
runtime policy, and item result digests; resumed completed items require their caller-owned journals
and value rehydration, so a restart cannot silently reacquire evidence. Tampered checkpoint/input or
policy drift fails before adapter dispatch. The checkpoint store remains an application-owned
atomic adapter, and multi-host consensus/lease coordination is still deployment work.

The portfolio evidence runtime now also has a dependency-aware multi-worker work queue. Admission
binds every domain item to the reviewed provider/evidence/request/checkpoint identities; claims are
lease-fenced, only direct predecessor-complete items are runnable, and provider refusals, expired
leases, retries, evaluator handoffs, dependency failures, cancellation, and reconciliation remain
explicit. A metadata-only worker and CAS-fenced snapshot coordinator are included. The caller still
owns the actual item executor, source adapters, and multi-host transaction backend.
The queue now also exposes bounded JSON and transactional text-store adapters, browser storage
support, public snapshot validation, and a worker reaper that converts abandoned leases into
explicit reconciliation rows before new claims are made.
For genuinely shared workers, the CAS coordinator and atomic worker now reload and commit every
queue transition, retry bounded conflicts, and prevent duplicate claims after two hosts restore
the same snapshot. The backing text store still must provide a real atomic compare-and-swap;
the SDK cannot manufacture distributed consensus from an ordinary read/write store.

Portfolio execution now also offers a caller-owned hash-chained decision trace. It records plan
verification, item dispatch decisions, blocked/omitted outcomes, learning status, progress, and
the terminal portfolio state as metadata-only events; `trace_digest` is bound into the execution
identity. This provides a useful dashboard/remote-worker observability seam without retaining
tasks, prompts, outputs, evidence, credentials, or tool payloads and without granting the trace
sink execution authority.

The trace now has a durable seam as well: validated in-memory storage, digest-checked snapshots,
strict JSON/text persistence, transactional CAS fencing, and browser storage are exported. Restore
is identity-bound and atomic from the store's perspective; it cannot resume provider work or grant
approval. This leaves database encryption, retention/rotation, access control, and distributed
multi-region replication to the embedding deployment.

Remote portfolio provider execution is now represented by a lease-fenced metadata queue and
pull-based worker. Admission binds plan/item/request identities; the worker rehydrates private
requests and reviewed artifacts through a resolver, requires admission by default, persists only
checkpoint/result/trace digests, and fails closed on plan, admission, request, trace, or lease
drift. JSON, CAS, browser-storage, retry, expiry, and reconciliation paths are covered offline;
deployment still owns the API transport, resolver vault, tenant authorization, and distributed
lease implementation.

The checkpoint seam now includes an optional atomic compare-and-swap contract plus bounded JSON
and transactional-JSON adapters. The controller serializes local operations, fences every flush
against the restored digest, and surfaces stale-writer conflicts instead of overwriting progress;
non-transactional stores are explicitly single-writer only.

The autonomous façade now compiles every built-in workflow into a digest-bound evidence plan.
Python `AutonomousTaskOrchestrator.evidence_plan()` and TypeScript `AutonomousAgent.evidencePlan()`
report qualified requirements, ambiguous-label handling, coverage, and dependency-safe next
stages; blueprints and prompts carry the same contract. The Python and TypeScript facades now also
expose a bounded evidence runtime that binds exact requirements to caller-owned acquisition,
projection, and versioned evaluation adapters, while retaining only digest-bound receipts and
replay/reconciliation metadata. It does not retrieve sources, interpret raw media, authorize
connectors, or establish truth on its own; external source adapters, domain evaluators, UI, and
durable production persistence remain application-specific deployment work.

TypeScript now adds an explicit `AutonomousEvidenceAdapterRegistry`. It can register scoped
caller-owned acquisition/projector functions, report manifest-only coverage for all twelve domains,
route runtime requests by requirement domain, refuse ambiguous or cross-domain adapter selection,
and keep raw values outside registry projections and durable metadata. The registry provides the
process boundary; it does not invent source truth or credentials.
The existing bounded HTTP connector now has a direct evidence bridge with caller-owned endpoint and
request resolvers, explicit header/fetch seams, shared host/HTTPS/size/timeout policy, and refused
HTTP status propagation into failed evidence receipts. Offline tests exercise that bridge across all
twelve domains without opening a network connection.

The shared brain lifecycle now includes atomic priority-ordered dequeue and side-effect-safe
cancellation across the Rust MCP projection, the durable Python SQLite adapter, and the typed
TypeScript controller. This closes the worker handoff gap while preserving the remaining boundary:
multi-host consensus, external-effect verification, protected task rehydration, and provider
invocation are still caller-owned deployment responsibilities.

The MCP transport now executes additional in-tree contracts for tabular ingestion and conformance,
observed-world declaration, provenance-bounded claim checking, federated resolution, dependency
locking, trajectory ingestion and divergence review, specimen-lineage auditing, pre-analytic
mutation checks, and multimodal contradiction programs. Those callable surfaces are intentionally
not removed from this list: this backlog measures blueprint citation/ownership coverage, while
transport exposure is a separate integration layer and does not turn external SDK, CI, UI, OTLP
export, or service prose into implemented code. The transport also exposes the in-tree operational acceptance,
capacity projection, result-bundle verification, adaptive evaluation,
posterior release-gate, oracle reference-standard, oracle mesh, and atlas contracts; these remain separately tracked here when
their blueprint ownership is not yet cited by a crate or design note. It now also exposes
bioevalx worldline/reproduction/trajectory checks, runtime effect authorization and tape
verification, research-only oncology boundary and criteria-aware response checks, and stress
family/report sweeps; these are transport integrations of existing in-tree contracts, not claims
that the remaining SDK, UI, CI, OTLP export, or service gaps are complete.
The runtime transport now also runs bounded deterministic effect programs through record/replay,
budget, fault, and fork checks. The million-scale transport exposes mechanistic twin discrepancy
qualification plus distributed placement, attestation, locality, fencing, and duplicate-effect
audits. These endpoints make existing Rust contracts agent-callable; they do not claim that real
containers, distributed workers, multi-node durable queues, external-state restoration, or
biological calibration exist. The factory now has a bounded shared-local-file authority envelope,
explicit lease recovery, attempt fencing, transition journaling, orphan-lock audit, and local
admission/class occupancy bounds; that authority is not multi-host consensus or a distributed
scheduler and does not provide tenant fairness.
The transport also exposes the deeper OncoWorld longitudinal-clock, integrated-classification, and
identity-join contracts; these remain transport integrations of existing domain invariants and do
not imply that clinical inference, identity or contamination oracles, or external data connectors
are implemented.
The same transport now exposes patient-derived-model transport, methylation classification and
version reconciliation, and radiogenomic design/target checks; those calls execute existing
in-tree invariants but do not supply classifiers, image models, identity or contamination oracles,
or clinical inference.
The transport also exposes validated bioevaluation reference-standard and acquisition-trace audits;
they report uncertainty shape, obligation closure, stopping posture, redundancy, deferred decisive
cost, and named-policy regret but do not score predictions, run an oracle, execute acquisitions, or
provide ontology graph distance.
The transport also exposes the endpoint-agnostic oncology outcome/estimand record and clonal
history compatibility checks; these preserve typed censoring and phylogeny ambiguity but do not
estimate cohort survival or infer a phylogenetic tree.
It now also exposes the §36 bioethics contracts for physical-action referral, human-subject
screening, dual-use review, validation evidence, and representation attribution, plus the §43
numeric influence analyzer over caller-declared factor regions. These calls keep institutional
decisions, external execution, and unknown influence preconditions explicit; they do not create
an IRB, biosafety review board, runtime sandbox, clinical decision support system, or a biological
truth oracle.
The transport now also exposes `security_redteam_simulate`, which composes the deeper in-tree
section-13 disclosure, regression-corpus, trust-boundary/influence, incident-blast-radius,
forensic-timeline, audit-chain, and attestation contracts. It makes containment and disclosure
claims fail closed on partial lineage, unresolved results, skipped lifecycle rungs, forbidden
artifact destinations, and unwitnessable claims. This is still a contract replay: it does not
execute a fuzzer, sandbox, detector, credential revocation, quarantine, notification, incident
workflow, external checkpoint, or durable audit store, so the remaining `36.07` and `36.19`
blueprint entries stay in the attention list below.
The transport now also exposes `domain_decision_readiness_audit`, a catalogue-bound structural
gate over caller-selected reports. It keeps support/qualification floors, contradiction/refusal
vetoes, review allowance, linkage, lineage, and every missing requirement explicit across all
29 groups. Its human-review state is not scientific, clinical, release, execution, or truth
authority; the remaining external acquisition, UI, process, and authority gaps stay in this list.
The transport now also exposes `biocapability_evidence_audit`, a strict composition surface for
the §33 metric, value-of-information, reference-standard, temporal-validity, reproducibility,
cross-modal, causal, translation, and multi-agent contracts already present in the workspace. It
requires explicit claim requests, validates dimension-specific support fields, blocks future or
unknown evidence, and keeps declared evidence separate from measured support. This increases
agent-callable depth but does not supply assays, estimators, replication infrastructure, external
data acquisition, clinical validation, or distributed agent execution, so the remaining §33
blueprint entries stay in the attention list below.
The public-hub transport now also exposes `bioatlas_publication_audit`, which binds the atlas,
evidence-conditioned claim, moderation/card, and leaderboard contracts into one explicit target
workflow. It refuses implicit release claims, requires evidence for numeric card scores, and keeps
holes, withheld scores, and unranked entries visible. This is a continuation-safe in-memory audit;
it does not publish a web page, authenticate identities, execute assays, detect leakage, or create
clinical authority, so the remaining §34 interface/integration entries stay in the attention list.
The transport also exposes evidence-backed routing, token-context planning, and deterministic
WeaveLang compilation/replay inspection. These calls remain bounded library integrations: routing
cannot synthesize an architecture, token planning does not render or retrieve payloads, and
WeaveLang does not provide network execution, model calls, signing, or target generation.
It also exposes multiparty choreography projection/model checking and the shipped FIBER
conformance/release gate; these verify local protocol and fixture contracts only, and do not
constitute distributed execution, external certification, or production readiness.
The graph projection endpoint generates four provenance-bound navigation views from one compiled
section; it does not add a second relevance policy, storage layer, renderer, or truth claim.
Provider capability gates are similarly evidence-only: they do not run sandbox, security, or
performance suites and do not convert caller measurements into thresholds.
The transport now also exposes the typed graph-lens catalogue and cohort leakage lens. These calls
make section-42 question contracts, scope preconditions, nonvisual witness rows, and sealed report
receipts callable; they do not implement a renderer, split repair, data acquisition, or an external
clinical leakage oracle.
It also exposes lineage-family split verification and evaluator stewardship conclusion. Those calls
check imported release records and issue only dimension-scoped review approvals; they do not generate
benchmark worlds, authenticate actors, schedule reviewers, or create a temporal expiry service.
The transport also exposes in-memory infrastructure quality gates and bitemporal ledger ingestion.
These preserve not-runnable quality checks, causal quarantine, idempotency, independent time axes,
and digest-only projections; they do not add durable storage, clock authority, schema inference,
repair, queues, or external event delivery.
It also exposes the Fabric goal-to-molecule constraint compiler. The endpoint checks the real hard
constraints and Pareto frontier, but does not pretend to perform model decomposition, registry
retrieval, probing, participant binding, runtime recompilation, or effect execution.
The interweave workflow catalogue is callable as well: its owed-deliverable counts are derived from
the typed nine-item set, while the missing programs, fixtures, adapters, and runtime participants
remain explicit rather than being represented by a completion percentage.
BioQL compilation is callable too: it type-checks explicit schemas for units, frames, builds, clocks,
ontology expansion, labels, provenance, and cost bounds, but does not execute queries, load stores,
infer schemas, convert units, expand ontologies, or enforce permissions.
The epistemic value-of-information, exact bounded adaptive acquisition, observed-context
compression, bounded evidence-selection, and explicit-contract decision-equivalence quotient are
now callable as well, alongside the benchmark trace compiler and pack portfolio contracts. They
preserve explicit losses and beliefs, exhaustive rate-distortion/submodularity checks, protected
closure, exact small-instance comparison, branch-dependent policy trees, review-gated causal
localization, and declaration-versus-measurement boundaries; they do not add acquisition execution,
trajectory replay, benchmark execution, pack generation, SDKs, external APIs, or a public evaluation
service.
The quotient additionally preserves model identity, permitted-action boundaries, exact loss-difference
profiles, tie sets, and deterministic compression across Rust, MCP, Python, and TypeScript. The
versioned `fiber-query/0.3` boundary now carries and executes that explicit contract inside FIBER;
legacy 0.1/0.2 queries still defer the pass. The versioned `fiber-query/0.4` boundary now also
binds a normalized compatible-model prior, a bounded observed evidence pool, a compatibility
floor, and a distortion tolerance, so FIBER can execute identification, exhaustive frontier
enumeration, and minimal-sufficient-context classification. This remains a bounded observed
context compiler: it does not execute an acquisition, perform causal identification, or claim that
an evidence item was actually acquired. The versioned `fiber-query/0.5` boundary now carries the
same adaptive contract into FIBER: normalized prior, complete outcome partitions, scalarized
path budget, finite horizon, and a certificate-bound named policy tree. It plans only under its
exact 16-acquisition, 16-step, and 65,536-state caps; it still does not schedule, authorize,
execute, or receipt the plan.
The foundation contract surface is callable too: it keeps admissibility, refinement, applicability,
counterfactual world strength, reveal policy, and plane consistency independent, without creating
clinical authority, causal identification, or runtime world validation.
The pack-health gate, SDK registry admission check, and repository change-impact analysis are now
callable as well. They respectively bind benchmark health to a pack digest, separate manifest
validation and trust from host registration, and preserve conservative graph propagation stops;
none of them creates benchmark generation, dynamic loading, signature verification, a sandbox, or a
semantic document diff.
The transport now also exposes deterministic `world_generate`, bounded `hub_submission_review`,
and `telemetry_project` workflows. Generation parses and validates both generated documents before
returning digest-bound output; hub review checks submission/licence/provenance/nonclaim contracts
and can replay append-only moderation; telemetry projects typed events with semantic-loss reports
and refuses metrics without observed support. These remain local, in-memory contracts: they do not
create a benchmark execution service, identity provider, durable public hub, OTLP exporter, or
network publication path.
The factory lifecycle and public-hub layers are now deeper as well: `factory_lifecycle_simulate`
replays leases, expiry, idempotency-aware recovery, staged commits, compensation, quarantine, and
cancellation; `factory_authority_verify` verifies the shared-local-file queue envelope and
hash-chained transition journal without dispatch; `hub_disclosure_review`, `hub_card_render`, and `hub_leaderboard_render` preserve
digest-bound disclosure ratchets, publication-state score withholding, comparability, and
unranked reasons. `release_audit` composes registry, bundle, quality, conformance, research-CI,
operations, pack-health, repository-impact, and developer-platform evidence into a strict required
gate plus advisory projection. These are bounded local workflows, not durable queues, identity
providers, web pages, CI runners, or deployment approvals. The bundle layer now verifies explicit
offline Ed25519 signatures and caller-supplied registry policy with signed delegation, rotation,
revocation, role, producer, and validity checks, but that bounded snapshot is not an external
identity authority or release service. The authority
coordinates cooperating processes on one local filesystem, but does not implement multi-host
consensus, network-partition tolerance, or tenant fairness.
`developer_delivery_audit` now composes the developer-platform and repository baselines with
optional SDK admission, conformance, provider capability, governance-document, conservative
impact, and release evidence. Its explicit target matrix makes local delivery, guarded claims,
foreign-artifact gaps, and missing evidence mechanically visible; it still does not implement the
foreign full Python SDK surface, gRPC clients, GitHub Actions, CI runners, or authoring UI; the
Rust HTTP/event gateway and Python HTTP client are now present.
The registry lifecycle and metrics profile surfaces are now callable too:
`registry_lifecycle_simulate` replays attested pack publication, promotion, reassessment,
supersession, withdrawal, lookup, revision history, and index integrity, while
`metrics_profile_audit` exposes per-capability leaders, measured/unmeasured populations,
per-system holes, and optional weighting sensitivity. They make existing local contracts
continuation-safe and public-card-ready; they do not add network registry transport, signing,
estimators, statistical inference, or a rendered UI.
The infrastructure lifecycle surfaces are now callable as well: `cache_invalidation_simulate`
rebuilds complete keys and turns partial dependency knowledge into explicit unknown regions and
unproven entries, while `storage_lifecycle_simulate` makes tiering, pin protection, reserve-aware
quota accounting, and non-copyable delegation/absorption replayable. They remain deterministic
in-memory contract projections: no durable cache, invalidation feed, byte mover, quota-enforcing
backend, tenant boundary, encryption, replication, or OTLP exporter is implied.
The trace layer now also owns the OpenTelemetry adapter: `trace_otel_ingest` imports bounded OTLP
JSON exports into the existing Event IR, preserves raw spans, resolves earlier parent links, and
returns explicit loss for inferred kinds, missing timestamps, unsupported fields, duplicate
attributes, unresolved parents, and multi-trace exports. It is a deterministic importer rather
than a collector client or OTLP exporter, so those external transport surfaces remain absent.
The developer-platform transport now also owns `bioprism-api`: a bounded HTTP/1.1 gateway delegates
REST and JSON-RPC calls to the MCP server, exposes cursor-based event/SSE snapshots, and maintains
a signed, retryable webhook outbox with idempotent acknowledgement. This covers the executable
REST/event portion of the platform, while gRPC, TLS termination, durable storage, and an external
delivery worker remain explicitly absent.
The gateway now also exposes a synchronous `/v1/missions/preflight` handoff that validates the
original mission policy and static schemas while returning an authoritative no-dispatch plan; it
does not create a job or imply that binding-dependent arguments have been executed.
Its `/v1/missions` inventory route now gives operators deterministic, status-filtered bounded
summaries and lifecycle links without exposing unbounded reports; the registry remains process-local
and non-durable.
Asynchronous jobs now also project authoritative trace events into a bounded live `progress` view.
Queued, running, cancellation-requested, and terminal responses share phase, wave, step, outcome,
byte, and latest-event counters; terminal reconciliation prevents the operational projection from
drifting away from the report. This still deliberately does not provide durable queue storage,
distributed scheduling, force-kill semantics, or domain-level success claims.
The gateway now also retains a bounded per-mission trace window and exposes cursor-based
`GET /v1/missions/{mission_id}/trace` retrieval. SDKs type the event pages and make retention gaps
explicit, so replay tooling can distinguish an empty page from history that was discarded.
Each asynchronous trace row is also emitted as a `mission.trace` event through the shared cursor,
SSE, and signed webhook outbox. This makes cross-domain mission monitoring composable with the
existing delivery worker contract instead of requiring a second event transport.
The SDK layer now adds typed Python mission inventory pages and bounded synchronous/asynchronous
wait helpers. A wait returns only a terminal job, never spins without a deadline, and preserves the
last live job on timeout; this is an orchestration convenience, not a durable queue or scheduler.
Python now also types the shared event cursor and webhook delivery pages, including retention gaps,
ordered event IDs, retry attempts, signatures, and pending counts. This keeps mission monitoring and
ordinary domain-tool observability on one contract in both SDK families.
The TCP serving path now shares an immutable router across connection threads, allocates request
IDs atomically, and clones the ready MCP session for independent dispatch. Mutable mission/event
state remains explicitly bounded behind its own locks, removing the former global request mutex.
The Python client also parses the bounded SSE snapshot with the same extension-field tolerance and
cursor-header semantics as TypeScript, keeping streaming-compatible monitoring dependency-free.
The first Python integration layer now exists under `python/`: a standard-library MCP client with
sync/async lifecycle handling, bounded JSON-RPC framing, structured refusal preservation, and
helpers for the shipped cross-domain workflows. This is intentionally narrower than the full
Python SDK backlog: heavyweight biological format adapters and the remaining nine-distribution
ergonomics remain foreign or unimplemented rather than being inferred from the transport or
authoring clients; bounded text VCF and benchmark distribution utilities are now concrete first
steps above that boundary.
The Python autonomous layer now also provides a reviewed-catalogue API executor bridge for all
twelve domain profiles and a caller-owned metadata-only receipt sink. It keeps discovery,
credential ingestion, approval, and domain interpretation outside the bridge, while bounded
transport/refusal failures remain explicit. This is a composition seam over the existing gateway,
not an external connector catalogue, durable queue, OTLP exporter, identity provider, or hosted
worker; those broader runtime surfaces remain intentionally listed below.
The next Python autonomous integration layer now adds a restart-safe hash-chained JSONL journal
for direct domain-tool receipts and a caller-owned connector registry/dispatcher over the typed
provider-manifest contract. These enforce exact twelve-domain scope, capability and approval
gates, transient connector values, credential-shaped request rejection, idempotent receipt
deduplication, and tamper/capacity failures. They still do not implement provider-specific
clients, key storage, domain response interpretation, distributed workers, durable queues, or an
OTLP exporter; those remain explicit integration work rather than hidden behind a local callback.
Both SDKs now also include a policy-gated, provider-neutral HTTP transport adapter with explicit
host/scheme/method admission, transient header resolution, no redirects, bounded request/response
bytes, timeout classification, and digest-only non-JSON/oversized projections. It closes the
generic transport seam without claiming provider-specific auth, pagination, source validation, or
multi-host delivery.
Both SDKs now add `AutonomousHttpMetadataEventSink` on top of that connector: a bounded,
allow-listed, recursively secret-free POST exporter for run/portfolio trace metadata with event-
digest idempotency, bounded transient retries, explicit 4xx refusal, and `409` duplicate receipts.
It closes the operational event handoff without claiming collector durability, OTLP semantics,
distributed queue consensus, tenant authorization, or evaluator truth; the deployment still owns
the transient header resolver and collector service.
The TypeScript autonomous runtime now also provides an explicit metadata-only run trace boundary:
`InMemoryAutonomousRunTraceStore`, hash-chained snapshots, bounded queries, provider invocation
observation, and `runWithTrace()`/`runCrossDomainWithTrace()` across all twelve domains. It makes
route, provider-turn, refusal, pause, and terminal state legible without retaining prompts,
responses, credentials, tool arguments, connector values, or raw evidence; durable persistence,
external telemetry export, and evaluator truth remain caller-owned deployment work.
The application-facing `AutonomousBrainFacade` now carries that same boundary through
`executeWithTrace()` and `executePlannedWithTrace()`: plan compilation, connector start/finish,
provider turns, approval pauses, terminal outcomes, and plan/request identity checks are recorded
in one bounded trace. This closes the high-level observability seam without weakening approval
gates or claiming that transport completion is domain truth.
The TypeScript surface now also connects the metadata-only brain job scheduler to the facade via
`AutonomousBrainJobWorker`. A caller-owned resolver binds a rehydrated request, execution mode,
and private policy digest to the durable job spec; the worker enforces durable approval release,
lease renewal, planned execution, direct/cycle/adaptive tracing, all-domain route checks, and
post-dispatch reconciliation. It closes the local worker handoff without claiming multi-host
consensus, provider idempotency, or secret-manager ownership.
The evidence-worker queue now adds a digest-bound acceptance proof at completion: exact queued
requirement/source/workflow identity, receipt and assessment content hashes, accepted evaluator
verdict, completed-requirement membership, replay state, and leased item digest are all bound into
`acceptance_digest`. Queue schema `0.2` migrates old metadata snapshots while quarantining legacy
completed items whose acceptance proof was never persisted. Remote brain settlement also verifies
job/spec identity and exact successful result digests, and handles already-terminal claims before
lease validation. This closes local false-success and stale-settlement gaps; external source truth,
distributed CAS, and provider idempotency remain deployment-owned.
The TypeScript scheduler persistence seam now also includes bounded JSON adapters for text-backed
stores, a browser Web Storage single-writer adapter, and an optional atomic compare-and-swap fence.
The coordinator serializes local flushes and refuses stale restored workers before provider
dispatch. A deployment still owns the actual IndexedDB, OPFS, SQLite, Postgres, or service-backed
transaction and must provide the CAS primitive before claiming multi-host lease safety.
Python now exposes the same bounded provider-neutral HTTP snapshot text store with strict endpoint
admission, no redirects, transient header resolution, bounded UTF-8 JSON, and `If-Match`/
`If-None-Match` fencing, so its autonomous learning, evidence, trace, and job adapters can share
the remote persistence seam. The server-side atomic write, authorization, tenant isolation,
retention, and backup contract remain deployment-owned.
The Python decision-cycle coordinator now consumes that seam through strict JSON and transactional
CAS persistence adapters, restoring the verified route/plan/evaluation state chain and refusing
stale writers before a newer cycle snapshot can be overwritten.
The same adapters now expose bounded provider-neutral pagination: strict array/items-page parsing,
transient cursor continuation, cursor-cycle detection, page/item/aggregate-byte ceilings, and
metadata-only partial progress when a later page fails. Provider-specific envelope parsing remains
an explicit callback, and the transport still does not claim source interpretation, domain truth,
or distributed delivery.
The provider runtime and autonomous façades now expose a bounded provider-neutral multimodal
content contract:
text/image URL/inline-image parts are translated into OpenAI Responses, OpenAI-compatible Chat,
and Anthropic wire shapes, HTTPS/base64 validation is fail-closed, policy messages remain text-only,
and unknown provider-native fields are rejected. Image payloads stay request-local and are absent
from health, learning, and public projections. The TypeScript `contentParts` and Python
`content_parts` options propagate through direct runs, tool loops, missions, every-domain
cross-domain fan-out/synthesis, workflows, learning, and restart-safe child execution. This
closes typed multimodal invocation without claiming image understanding, file acquisition, source
interpretation, or domain-truth validation; callers must resupply transient parts after restart.
The connector layer now also has a typed API source-plan/source-execute bridge that binds the
returned plan digest before retrieval and keeps connector scope separate from provider payloads.
Both SDKs now expose that bridge (`createAutonomousApiSourceConnectorExecutor` on TypeScript and
`create_autonomous_api_source_connector_executor` on Python). This makes the existing gateway
usable from the autonomous runtime without turning it into a credential client or silently
enabling discovery; concrete provider-specific adapters and external authentication/session
resolution, source interpretation, and domain validation remain caller-owned.
Connector dispatch now also accepts a caller-owned restart-safe receipt store. The bounded,
fsynced connector JSONL journal verifies a hash chain, deduplicates exact identities, rejects
conflicting outcomes, and returns a metadata-only replay barrier after restart; a new attempt
identity is required for retry. It intentionally does not claim distributed exactly-once delivery,
cross-process fencing, provider idempotency, or response caching.
Connector planning now also emits a typed deterministic selection plan across the exact requested
domains and capability. It retains candidate/manifest digests, binds the registry snapshot, and
requires the reviewed plan digest before dispatch; health, cost, latency, and evaluator-ranked
selection remain explicit caller-owned inputs rather than hidden heuristics.
The adaptive selector now accepts only bounded caller/evaluator signals for health, success rate,
latency, cost, evaluator reward, and eligibility. It records normalized scores and a signal digest,
uses fixed weights with deterministic tie-breaking, and is exposed through `AutonomousAgent` so
the façade can plan and dispatch the same reviewed route without introducing a second policy.
The durable brain-job boundary now also has an atomic Python `BrainJobStore.claim_next()` scheduler
primitive and a dependency-free TypeScript-local `InMemoryAutonomousBrainJobScheduler`. Both keep
task/prompt/credential/provider values caller-owned, bind idempotency to digests, fence leases,
bound retries, quarantine uncertain post-dispatch work, and expose restart-checked metadata
snapshots. These close local scheduling and SDK portability gaps; multi-host consensus, tenant
fairness, external delivery guarantees, and provider-specific execution remain absent.
The Python side now also ships `DurableBrainControlPlaneAdapter` and its async façade: a concrete
application-owned transport over the SQLite journal with fail-closed authorization, queued approval
admission, restart-safe lifecycle calls, and digest-only projections aligned with the Rust
`brain_job_*` vocabulary. This closes the missing local durable transport seam; HTTP/MCP identity,
multi-host consensus, tenant fairness, external delivery guarantees, and provider-specific
execution remain deployment-owned.
Python durable job state now has a portable handoff as well: queue records and the complete
hash-chained event journal can be validated, serialized as canonical JSON, restored into a fresh
SQLite worker, and fenced with conditional writes through a caller-owned text store. The snapshot
preserves queued, leased, approval-paused, terminal, and reconciliation-required states across all
twelve domains while retaining no task, prompt, credential, provider response, or tool payload.
Multi-host scheduling, storage encryption, and external effect authority remain deployment-owned.
The MCP control-plane projection now also exposes the matching remote lifecycle vocabulary:
`brain_job_claim_next`, `brain_job_claim`, `brain_job_renew`, `brain_job_checkpoint`,
`brain_job_complete`, `brain_job_fail`, `brain_job_reconcile`, and `brain_job_cancel`. The
TypeScript `AutonomousDurableJobController` can atomically dequeue by priority, claims and renews
its worker lease, records a digest-only execution admission before entering the local provider
boundary, and settles completion, failure, or cancellation without sending payloads. Cancellation
preserves a `reconciliation_required` quarantine after a dispatched or unknown boundary. The Rust
projection remains process-scoped; Python `BrainJobStore` is still the restart-safe authority and
multi-host persistence, authentication, and external effect verification remain deployment-owned.
Python now also includes a credentialless `local-offline` built-in connector adapter covering all
twelve operation contracts. It projects caller-supplied fixture metadata into digests, shapes,
counts, and explicit `observed`/`partial` outcomes so routing, approval, worker recovery, replay,
and evaluator tests are executable without a provider key or network. It does not replace the
remaining external provider-specific adapters, source retrieval, authentication, or domain-truth
validation, which remain caller-owned by design.
Python now also has a credentialless staged connector execution path: domain-scoped built-in
manifests preserve every workflow capability, `AutonomousAgent.run_connector_workflow()` walks
the prepared dependency DAG, and connector outcomes use the existing structured workflow
checkpoint/status contract. Replay without caller payload rehydration pauses for reconciliation;
digest-verified rehydration resumes without reinvocation. Provider-backed workflow execution,
external source retrieval, and independently verified domain truth remain separate caller-owned
surfaces.
The domain-workflow handoff now also has a retained verification boundary: callers can validate
catalogue/contract/binding identity, rerun mission preflight, and optionally replay the original
instantiation request before re-review. The verifier is intentionally structural and non-executing;
it does not remove the remaining domain adapters, external evidence connectors, or durable execution
orchestration work below.
The handoff now also supports a bounded domain-workflow portfolio: up to 64 explicit group plans
can be composed with per-item authoritative preflight and retained blocked rows. Complete-catalogue
coverage and deliberate partial planning are separate states, so a missing domain-specific argument
does not disappear behind a portfolio-level pass. This is still a planning/evidence composition
surface rather than a scheduler or execution authority.
The same portfolio now has a retained verification/replay continuation: the verifier recomputes the
portfolio digest, checks coverage, aligns optional original requests by item index, preserves each
identity/replay/preflight mismatch, and reruns authoritative mission preflight without dispatch.
This closes the multi-domain plan-to-revalidation seam while leaving semantic sufficiency, external
provider execution, and durable orchestration explicitly outside the boundary.
The Python layer now also covers the evaluator/oracle/mutation and environment/pack authoring
contracts: it builds versioned oracle manifests and judgements, preserves tier demotion and
admissibility, validates distributions and findings, and exposes the oracle mesh, reference-panel,
missingness, worldline, reproduction, trajectory, and reference-standard tools. Rust still owns
combination and scientific decisions; the remaining Python backlog is heavyweight biological
adapters and full distribution ergonomics. The metrics layer now additionally exposes
bounded descriptive analytics for scalar observations, paired robustness/cross-modal/translation
contrasts, replicate spread, cost/latency, and calibration; those summaries remain descriptive and
do not replace inferential statistics or external evidence acquisition.
The Python layer now also provides typed evidence-conditioned BioCapability requests: nine named
evidence dimensions, explicit status handling, dimension-specific support maps, claim prerequisite
validation, duplicate-ID rejection, and sync/async/HTTP transport helpers. The Rust kernel still
owns comparability, nested evidence audits, and release decisions; the Python models do not mint
scientific support or publication authority.
The same Python surface now exposes a bounded `bioql_compile()` bridge for explicit query/schema
compilation over sync MCP, async MCP, and HTTP. Local validation only bounds strings and canonical
JSON; Rust remains authoritative for BioQL syntax, schema, units, frames, clocks, provenance,
access labels, and cost semantics, and the bridge never executes a query.
Typed Python envelopes now also cover `world_claim_check`, `lab_plan`, and `routing_decide` across
sync MCP, async MCP, and HTTP. They keep serialized provenance, obligation/action graphs, routing
fingerprints, evidence ledgers, budgets, and task identity bounded and explicit without duplicating
Rust's support, privacy, reachability, abstention, or safe-default decisions.
The context SDK surface now covers the full FIBER lifecycle: typed compile, handle-or-source refine,
plan explanation, certificate verification, and bounded projection bundles over sync MCP, async MCP,
and HTTP. Relative-path traversal and ambiguous source selection fail before transport, while layer
sufficiency, omission accounting, certificate semantics, and projection fidelity remain Rust-owned.
Repository knowledge and observability are now typed as well: catalog discovery, route-specific
documentation bundles, conservative module impact, and redacted telemetry projection all have
bounded sync/async/HTTP envelopes. Markdown remains opt-in and capped; telemetry metric inputs
remain coupled to observations so the SDK cannot turn asserted-only or unclassified data into an
apparently authoritative operational record.
The benchmark utility complements that kernel for Python notebooks: it separates measured from
declared/missing/blocked rows, computes deterministic distribution summaries, performs direction-aware
paired contrasts, and offers cluster-aware percentile bootstrap intervals with explicit assumptions.
It remains descriptive and does not turn resampling into a significance test, causal estimator, or
clinical claim.
The developer workbench now provides an authoritative Rust contract for the implementable core of
the remaining authoring-platform idea: sessions carry artifact cards, notebook cells, dependency
ordering, logical change history, stale-digest findings, evidence-aware dashboard rows, and a
review-only CI workflow plan. Python and TypeScript expose the same composition surface. This does
not close the external authoring UI, consumer-repository action, hosted GitHub runner, or full
Python distribution backlog, so those gaps remain explicit rather than being relabelled complete.
The retained-workbench continuation is now implemented as `developer_workbench_verify`: a caller can
store the complete report, later re-audit the current session, replay its dashboard and optional CI
request, and receive content-digest/mismatch witnesses through REST, MCP, CLI, Python, and TypeScript.
This closes the local authoring handoff audit seam while leaving the external UI, package publishing,
GitHub authentication, hosted runner, and provider-observed execution evidence intentionally open.
The next retention seam is also implemented: `developer_workbench_import/query/get` provides a
bounded, content-addressed registry shared by MCP and REST, with CLI and typed SDK facades plus an
atomic `--workbench-state` checkpoint. Import, query, restore, and lookup verify report/snapshot
digests and preserve transport-normalized envelopes; they do not turn local retention into a hosted
authoring UI, a GitHub-backed repository action, CI execution, or release authority.
The `ci_execution_evidence_audit` route adds the next safe boundary without claiming the external
runner: it regenerates the canonical plan, requires a matching plan digest and per-check result
digests, reconciles exact check names and requiredness, and keeps caller/provider provenance,
missingness, failure, cancellation, and structural-only verification visible. A complete passing
report can become a bounded handoff signal, but it is not provider authentication, log retrieval,
deployment approval, or scientific validity.
`ci_provider_normalize` now closes the provider-shape ingestion gap before that audit: bounded
GitHub Actions, GitLab CI, and generic payloads map into canonical `CiRunEvidence`, missing result digests are
derived and labeled, supplied malformed digests are refused, and unknown/non-passing statuses stay
visible to the downstream audit. It remains caller-supplied structural normalization rather than
provider contact, signature verification, log retrieval, or external CI execution.
The repository now also includes a reusable `.github/actions/github-actions-evidence` composite
action and a deterministic exporter test contract. It supports both bounded caller-selected GitHub
Actions rows and an authenticated, token-free-in-output discovery mode for one run's jobs. The
optional collection mode adds bounded GitHub artifact metadata and job-log locators, plus a
caller-supplied attestation file, and can emit the exact provider-evidence request consumed by the
Rust audit/registry when an explicit CI plan is provided. An explicit byte-collection switch now
downloads HTTPS artifact/log responses under per-response and total caps and binds local SHA-256
digests. Explicit digest scopes and optional attestation subject-digest joins now survive into the
Rust audit, while archive extraction, log interpretation, signature verification, and release
authority remain out of scope. Both modes refuse duplicate, oversized, malformed, or
control-character-bearing inputs. This materially covers the local consumer-repository handoff for
11.21 and the bounded discovery/byte-hash portion of 11.22, but it does not execute checks, verify
attestations, upload artifacts, or provide hosted runner/release authority.
Consumers must still retain the payload and pin a reviewed action revision.
`ci_provider_evidence_audit` now adds the next conformance layer for artifact, log, and attestation
rows: it preserves the supplied records, checks provider/run/check bindings and subject references,
computes separate deterministic row-family digests, and fails closed on malformed or unbound rows.
This is still a local structural handoff; it does not fetch bytes, authenticate a provider, verify
signatures, execute CI, or establish release authority.
The delivery audit and content-addressed receipt now accept this conformance result as a separate
`ci_provider_evidence` target/evidence row. Receipt verification recomputes the complete retained
projection and distinguishes provider-evidence tampering from target or canonical CI tampering; this
still does not create a remote artifact verifier or external CI authority.
The provider-evidence retention seam is now implemented as a bounded shared registry: import
re-audits the complete request, deterministic query/get preserves provider/run/plan joins and
artifact/log/attestation record-family digests, and failed/unknown runs remain explicit retained
evidence. MCP, REST, CLI, Python, and TypeScript expose the same contract, while
`--ci-provider-evidence-state` provides atomic restart recovery with snapshot and per-record digest
checks. Import summaries and compact queries also expose local-byte hash and attestation
subject-digest binding counts, with minimum-threshold filters for operator posture searches. This
closes local evidence retention and operator lookup, but leaves provider contact,
remote byte retrieval, signature verification, hosted execution, and release authority outside the
repository boundary.
The bounded HTTP event checkpoint now also restores subscription metadata and signed pending
outbox envelopes. Restored subscriptions are paused, pending rows expose
`secret_rebind_required`, and an explicit in-memory rebind re-signs them before activation; webhook
secrets are never checkpointed. This is restart-aware local recovery, not distributed event storage,
consensus, or an external delivery worker.
The delivery audit now accepts that request directly as `ci_provider`, returning the normalized
projection and downstream CI evidence audit together while refusing mixed canonical/provider
evidence inputs. This closes the local composition gap without claiming that an external runner,
provider signature, or log service exists.
`developer_delivery_audit` accepts either canonical evidence through `ci_evidence` or provider-shaped
evidence through `ci_provider`; the independent `ci_execution_evidence` release target is fail-closed
when its evidence is absent or not ready, while unrelated local-delivery targets remain independently
auditable.
The delivery audit also exposes a separate `execution_provenance` target, allowing a caller to
request mission-trace readiness independently of CI evidence or to require both explicit signals;
neither path silently upgrades structural evidence into execution authority.
`developer_delivery_receipt` now recomputes that audit into a deterministic, content-addressed
structural handoff. It sorts target rows canonically, preserves evidence presence/readiness and
blockers, and emits delivery/target/receipt digests for cross-transport joins without timestamps;
it does not add signatures, durable storage, provider execution, or release authority.
`developer_delivery_receipt_verify` recomputes that handoff from the completed delivery audit and
keeps tampered digest, target, evidence, and readiness fields separately visible. This makes the
receipt replayable for cross-domain consumers without claiming provider authentication, durable
revocation, execution, or deployment authority.
`execution_provenance_audit` adds the corresponding mission-side handoff: returned plan identity,
terminal results, deterministic trace ordering/tool identity, and delegated-check digests are
reconciled in one structural projection. It does not replay a mission or replace external execution,
provider authentication, deployment approval, or durable audit storage.
The mission layer now composes the shipped domain tools into a deterministic DAG with an explicit
preview/execute boundary, tool allow-list, side-effect policy, output budgets, and refusal-to-blocked
dependency propagation. This materially improves agent usefulness across domains without claiming
that distributed scheduling, durable queues, sandboxed arbitrary code, or external CI execution exist.
The executor now closes the transport-contract gap as well: it performs bounded authoritative JSON
Schema validation from the live MCP `tools/list` definitions before static mission acceptance and
again after JSON-pointer bindings are materialized, refusing malformed serial or parallel calls
with a schema digest and bounded diagnostics before nested dispatch.
The capability-discovery layer now makes that catalogue searchable by explicit intent, domain, group,
and tool, with digest-bound results and optional authoritative MCP input schemas; its scores are
routing evidence only and do not grant permission or assert readiness.
The companion `capability_audit` verifies that every catalogued callable has an authoritative
transport schema and that every advertised schema is catalogued, while preserving intentional
multi-group membership and per-domain coverage rows. It also checks object input-schema shape,
required-field closure, and schema-size bounds across the full advertised tool set.
`capability_route` batches named cross-domain needs into a reproducible proposal with explicit
versus ranked resolution status, bounded candidate unions, optional authoritative schemas, and an
unambiguous no-execution boundary; the caller still reviews arguments before constructing a mission.
`capability_route_review` now provides that review as a reusable native handoff checkpoint: it
checks exactly-once selections, candidate membership, explicit argument objects, dependency
references/cycles, and deterministic waves, returning a blocked diagnostic or a mission draft that
still requires mission preflight. Optional authoritative schema validation adds per-tool digests and
bounded issue paths without granting execution permission or validating domain meaning. The review
also emits a deterministic content-addressed `review_id`, allowing route handoff evidence to be
joined across MCP, REST, SDK, event, and webhook records without timestamp coupling.
`capability_route_plan` now provides the corresponding bounded composition seam across MCP, REST,
Python, and TypeScript: it reruns the review, builds the exact selected mission, invokes only the
authoritative non-executing preflight, and returns a plan digest plus structured route/preflight
blockers. It intentionally leaves dispatch and authorization with the caller.
`capability_route_plan_verify` now closes the corresponding replay seam across MCP, REST, Python,
and TypeScript. It can verify a retained plan structurally, rerun mission preflight, and—when the
caller preserves the original route and selections—replay route review and compare all content
digests. Missing replay inputs are explicit limitations rather than an implicit freshness claim.
The companion `capability_dashboard` provides the missing operator inventory: it binds catalogue
groups to the authoritative `tools/list` schemas, separates callable/partial/declared-only rows,
keeps crate, CLI, Python, MCP-membership, and schema-backed counts independent, and reports explicit
gaps. Its bounded filters, truncation warnings, and dashboard digest make surface coverage
reproducible without claiming that a declared local artifact has been installed or executed.
The adapter registry now gives the same treatment to biological source boundaries: `adapter_plan`
selects native or Python-delegated routes by explicit format and source shape, carries the closed
semantic-loss vocabulary and scope dimensions, and reports dependency missingness versus dependency
uncertainty. This is a planning and contract layer; heavyweight Python readers still need to emit
source-specific conformance reports before their normalized worlds are publishable.
The first concrete Python implementations are now the bounded text VCF reader, BIDS manifest
auditor, and parsed DICOM projection auditor. The VCF route validates the full stream, preserves
typed and raw representations, records line/source hashes, and refuses to infer reference builds.
The BIDS route validates manifest paths, entities, sidecar inheritance/conflicts, participant
coverage, and derivative descriptions without reading binary image bytes. The DICOM route validates
UID hierarchy, dimensions, frame geometry, slice positions, provenance, and privacy-safe summaries
without decoding pixels. Indexed/compressed VCF, binary DICOM transfer syntaxes, NIfTI/affine
interpretation, AnnData/Zarr, BAM/CRAM, and OME-Zarr remain separate format-specific adapters rather
than being hidden behind a generic parser claim. The NIfTI projection route now covers header/affine
semantics without claiming to decode arrays, compression, extensions, or BIDS sidecars.
The AnnData/Zarr projection route now covers dimensions, index identity, annotations, sparse metadata,
embeddings, pairwise matrices, raw dimensions, and safe `uns` summaries without claiming to decode
HDF5/Zarr chunks or matrix values.
The alignment projection route now covers reference dictionaries, CIGAR accounting, coordinate
bounds, flags, pairing, sorting, and coverage without decoding BAM/CRAM payloads, indexes, or
reference bases.
The typed Python adapter runtime now closes the concrete execution handoff across the parsed VCF,
BIDS, DICOM, NIfTI, AnnData, alignment, FASTA, FASTQ, SAM, GFF3, PDB, SDF/MOL, mzML, OME-Zarr, and FHIR routes, normalizes outcome states and
document digests, and refuses raw-byte routes explicitly when their optional binary-reader binding
is absent.
The verified optional-reader layer now binds installed nibabel and anndata environments for raw
NIfTI header and H5AD/Zarr metadata inspection, while keeping full array/matrix materialization and
missing dependencies explicit.
Dependency-gated pydicom and pysam bindings now cover metadata-only DICOM, indexed/compressed VCF/BCF,
and BAM/CRAM record projection when those packages are installed; absent packages remain explicit.
The OME-Zarr route now reads and audits multiscale metadata directly from Zarr groups without
loading image chunks or pixel values.
The FHIR route now reads dependency-free JSON resources, Bundles, and Bulk Data NDJSON, checks
bounded resource and reference structure across every record, protects identifiers with
source-bound digests, and keeps profile validation, terminology expansion, clinical interpretation,
and external reference resolution explicitly outside its conformance claim.
The FASTQ route now validates complete multiline records, sequence/quality lengths, printable
quality ranges, duplicate read identifiers, and paired-read completeness; read identifiers, bases,
and quality strings remain source-bound digests or aggregate summaries rather than disclosed content.
The mzML route now audits bounded XML, spectrum identity/counts, MS levels, scan-time summaries,
binary-array type/compression/precision declarations, and encoded lengths without decoding or
emitting m/z, intensity, or time arrays.
The FASTA route now audits multiline reference records, duplicate identifiers, optional nucleotide or
protein alphabet claims, lengths, symbol counts, and GC totals without disclosing sequence strings.
The GFF3 route now audits bounded feature rows, coordinates, scores, strands, phases, URL-encoded
attributes, duplicate IDs, Parent resolution/cycles, directives, and embedded FASTA boundaries
without disclosing annotation values or feature identifiers.
The BED route now audits bounded BED3--BED12 interval rows, zero-based half-open coordinates,
optional score/strand/thick/RGB fields, transcript-style block geometry, duplicate intervals and
names, and coordinate ordering without disclosing chromosome labels, item names, or track metadata.
It remains a structural interval boundary: assembly/reference-build identity, annotation ontology,
and biological feature meaning are explicit caller-owned context rather than inferred from the file.
The PDB route now audits fixed-column atoms, models, chains, residues, coordinates, alternate
locations, crystallographic cells, resolution, CONECT edges, and bounded geometry without emitting
raw structure records.
The SDF/MOL route now audits bounded MDL V2000 molecular graphs, atom and bond counts, element
symbols, formal charge/isotope/radical property blocks, connected components, coordinate summaries,
duplicate data fields, and source-bound molecule/graph digests without emitting names, property
values, or raw records. V3000 is explicitly refused until a separate bounded implementation exists.
The SAM route now audits bounded text alignments, headers and sequence dictionaries, flag/mate
consistency, CIGAR query/reference accounting, coordinate bounds, optional-tag typing, and declared
coordinate sort order without emitting read names, reference labels, sequences, qualities, or tag
values. Binary BAM/CRAM remains an explicit dependency-gated route.
The `domain_acquisition_catalogue` now joins the authoritative 29-group capability catalogue to
the adapter registry without pretending that either registry alone is execution evidence. Every
declared domain receives separate transport and interpretation rows, bounded file/plain-HTTP and
caller-managed connector families remain distinct, and native/Python-delegated adapter matches
retain their declared scope-label basis. The four evidence transport/intake tools are explicitly
cross-cutting memberships for every group, making the existing source-plan and intake scope gates
usable across the whole catalogue. This still leaves provider authentication, ontology resolution,
source-specific conformance, and external execution as separate follow-on contracts.


## §11 Developer Platform — 6 uncovered

- `11.04` Python Sdk
- `11.17` Authoring Studio
- `11.18` Authoring Studio And Notebook Workflow
- `11.20` Capability Dashboard And Query
- `11.21` Github Action For Consumer Repositories
- `11.22` Github Action And Ci Integration

## §33 Biocapability Atlas And Metrics — 10 uncovered

- `33.03` Evidence Grounding Provenance And Claim Support
- `33.05` Information Acquisition And Context Value
- `33.06` Value Of Experiment Assay Selection And Active Discovery
- `33.07` Tissue Sample Time And Resource Efficiency
- `33.08` Temporal Validity And Evidence Firewall Metrics
- `33.09` Cross Modal Consistency And Contradiction Metrics
- `33.10` Causal Identification Intervention And Mechanism Metrics
- `33.12` Reproducibility Reexecution And Claim Stability
- `33.13` Translation Spine And Evidence Maturity Metrics
- `33.14` Multi Agent Coordination And Molecule Value

## §34 Bioatlas Public Hub And Ecosystem — 10 uncovered

- `34.01` Users Personas And Jobs To Be Done
- `34.02` Information Architecture And Navigation
- `34.04` Worldline Timeline And State Explorer
- `34.05` Biodecision Cell Inference Microscope
- `34.06` Fork Compare And Counterfactual Lab
- `34.07` Oracle Evidence And Disagreement Explorer
- `34.11` Architecture And Agent Molecule Registry
- `34.19` Notebook Ide Mcp And Agent Integrations
- `34.21` No Key Demos And Onboarding
- `34.22` Open Source Community And Star Flywheel

## §14 Governance And Quality — 9 uncovered

- `14.01` Project Governance
- `14.02` Open Governance And Rfc Process
- `14.04` Contributor Model And Code Ownership
- `14.05` Rfc Adr And Technical Decision Process
- `14.18` Conflicts Of Interest And Sponsorship
- `14.22` Documentation Information Architecture And Review
- `14.23` Community Conduct Inclusion And Appeals
- `14.24` Sustainability Finance And Public Benefit
- `14.25` Periodic Program Review

## §40 Build Ready Engineering Contracts — 6 uncovered

- `40.01` Technology Baseline
- `40.02` Monorepo And Package Layout
- `40.40` Ci Cd And Release Automation
- `40.41` First 100 Implementation Tickets
- `40.43` Engineering Adr Register
- `40.45` Ownership Raci And Maintainer Boundaries

The engineering_manifest_audit route is the new artifact-level foundation for these entries: it
validates baseline declarations, package topology, ticket contracts and readiness, ADR
supersession, RACI rows, independent-review separation, canonical digest, and fail-closed issue
severity. The backlog keeps the six blueprint entries because the route does not claim to replace
the surrounding process, repository automation, external ticket authority, or release systems.

The `engineering_execution_plan` route now adds the deterministic in-repository planning layer:
it selects a bounded ticket window, derives dependency-aware waves, computes a critical path, and
reports manifest-admission, dependency-closure, truncation, and schedule-completeness gates. It
does not close the six entries: external tracker synchronization, real CI execution, effort and
staffing data, ownership authority, and release automation remain intentionally outside this
artifact-level planner.

The `release_pipeline_audit` route now adds an artifact-level contract for the CI/CD and release
automation entry (`40.40`): it checks stage DAG closure, artifact lineage, digest-bound provenance,
signature and approval declarations, environment protection, promotion order, and rollback
targets. It still does not replace hosted CI, signing infrastructure, registry state, approval
authority, deployment execution, or rollback testing, so the six blueprint entries remain
uncovered as process/external artifacts.

The `operational_readiness_audit` route adds an artifact-level companion for service-operability
concerns: objective/indicator evidence, dependency fallbacks, runbook review, incident closure,
and baseline controls. It intentionally leaves live telemetry, on-call reachability, executed
restore/fallback tests, incident-management authority, and external operational process uncovered.

The `security_privacy_audit` route adds an artifact-level companion for the remaining section-36
governance gap: asset classification and retention, authorized information flows, identity
hardening, threat treatment, independent review evidence, and security/privacy controls. It does
not replace infrastructure scanning, identity authority, legal analysis, erasure execution,
vendor assurance, or an operational red-team/incident program, so those external/process claims
remain explicit.

The `sandbox_admission_audit` route adds an artifact-level companion for `36.07`: content-addressed
artifact identity and lineage, rootless/read-only/no-escalation profiles, bounded network and mount
surfaces, explicit dangerous capabilities, resource ceilings, quarantine, and reviewed output
release. `sandbox_runtime_simulate` now adds the deterministic process-side companion: it evaluates
ordered capability/target/resource requests, charges usage, and preserves refusal and not-run
suffixes. Neither route replaces a kernel sandbox, runtime admission controller, scanner,
credential revocation, quarantine storage, or operator response; external enforcement for `36.07`
therefore remains explicit rather than being relabelled complete.

The `security_program_audit` route adds an artifact-level companion for the program portion of
`36.19`: authorized scope, independent campaign review, evidence, finding/remediation closure,
incident timelines, disclosure sequencing, publication safety, and regression controls. It does
not replace scanners, a live incident system, containment, notification, disclosure delivery,
vendor coordination, or durable security operations; those runtime and external-process claims
remain explicit.

## §19 Reference Examples — 3 uncovered

- `19.01` Decision Cell Example
- `19.15` Evaluation Conditioned Routing Example
- `19.21` Reliable Repair Weave Program

## §36 Biology Security Privacy Ethics And Governance — 2 uncovered

- `36.07` Sandboxing Untrusted Code And Research Artifacts
- `36.19` Security Privacy Safety Red Team Program

## §10 Registry And Hub — 1 uncovered

- `10.01` Registry Overview

## §26 Bio Evaluation Engine — 1 uncovered

- `26.19` Biocapability Atlas

The transport now exposes the implemented 26.03 claim-grounding, 26.05 acquisition, and 26.09
estimand/identification kernels through `bioeval_grounding_audit`, `bioeval_acquisition_audit`, and
`bioeval_estimand_audit`. The evaluator-health kernel is also exposed through
`bioeval_evaluator_audit`, preserving harness failures as unscored rather than task failures;
the 26.17 scoring-plane kernel is exposed through `bioeval_plane_audit`, preserving unscored and
inapplicable dimensions rather than manufacturing zeros;
the metamorphic-response kernel is exposed through `bioeval_metamorphic_audit`, preserving false
sensitivity, false invariance, wrong-direction, and undetermined trials rather than flattening
them into a single pass rate;
the release-gate waiver kernel is exposed through `bioeval_waiver_audit`, preserving the original
gate verdict, exact affected-version scope, expiry, follow-up, unevaluable counts, and the
non-waivable safety-veto rule rather than turning an exception into an untraceable pass;
the factorial-design kernel is exposed through `bioeval_design_audit`, preserving explicit
baselines, one-factor contrasts, unattributable multi-factor arms, and missing interaction cells
rather than manufacturing component effects from unmatched arms;
the evaluator-mesh kernel is also exposed through bioeval_mesh_audit, preserving transitive
shared-input classes, circular-oracle refusals, within-class versus across-class witnesses, and
abstentions rather than counting correlated evaluators as independent votes;
the nonrenewable-resource kernel is also exposed through bioeval_burden_audit, preserving
inherited residuals, exact-unit draws, failed-action waste, and fork double-spend refusals;
the prospective seal/reveal kernel is also exposed through bioeval_reveal_audit, preserving
rubric and commitment digests, one-shot state locks, uncommitted outcomes, and selective
publication rather than treating a partial reveal as a complete score;
the contextual-integrity kernel is also exposed through bioeval_boundary_audit, preserving
authorized flow, respected denial, violation, irreversible veto, bypass, channel exposure, and
Pareto-separated utility/safety rather than manufacturing a combined privacy score;
the atlasx publication-surface kernel is now also exposed through atlas_surface_audit, preserving
CapabilityGrid denominator coverage, named debt discharge, withheld failure browsing,
denominator-safe rate checks, and surface soundness. The remaining item is the blueprint citation
itself, not a missing grounding, acquisition, estimand, evaluator-health, scoring-plane,
metamorphic, waiver, design, mesh, burden, reveal, boundary, or atlasx transport wrapper.
The restart boundary is now also exposed as a single recovery matrix. It deliberately keeps
mission terminal restoration, event rows, subscription metadata, pending outbox evidence,
process-local secrets, and external delivery effects in separate rows; it does not turn local
checkpoints into automatic execution resumption, distributed consensus, or receiver acceptance.

The Python long-horizon execution journal now has the same remote handoff seam. Its complete
hash-chained JSONL history can be snapshotted into strict canonical JSON, restored into a fresh
local journal, and fenced with conditional writes through any text store, including the bounded
HTTP snapshot transport. All twelve domains are covered by restart tests, and malformed envelopes,
tampered chain/head digests, payload-shaped metadata, oversize snapshots, and stale writers fail
closed. The provider conversation, prompts, credentials, tool arguments, and raw outputs remain
outside the snapshot; distributed scheduling, storage encryption, and deployment authorization
remain caller responsibilities.

Python resumable batch checkpoints now close the same race at the controller layer. Strict JSON
and transactional JSON adapters preserve the request/result digest image, and every controller
progress flush uses the restored checkpoint digest as a CAS fence. This protects domain, automatic,
and cross-domain batches from stale workers while retaining no task text, prompts, provider values,
connector observations, tool arguments, or credentials.

The Python model-health ledger now closes its remote restart seam too. Hash-chained provider/model
observations can be snapshotted as strict canonical JSON, restored into a fresh SQLite health
store, and CAS-fenced before they influence model selection across all twelve domains. The
snapshot retains only bounded telemetry and digests; live credentials, provider responses, prompts,
and evidence remain caller-owned.

The Python learning ledger now has the same cross-process handoff discipline. JSONL and SQLite
ledgers export one portable canonical snapshot containing value-only evaluator outcomes, pending
episode metadata, bandit state, and replay metadata. Every row is validated and individually
digested; the snapshot binds the ordered record set with a head digest and outer snapshot digest.
The coordinator supports caller-owned text stores and transactional compare-and-swap writes, and
restore is atomic for both local ledger implementations. All twelve autonomous domains are covered
by restart and HTTP round-trip tests, with malformed envelopes, non-canonical rows, secret-shaped
fields, oversized replay metadata, tampered row/snapshot digests, and stale writers failing closed.

Python episodic memory now closes the adjacent restart gap. Its existing hash-chained SQLite
events can be exported as a strict canonical snapshot and restored atomically while rebuilding the
materialized query index from validated episode/evaluation events. The memory coordinator supports
the same caller-owned JSON and conditional-write/HTTP adapters as the other autonomous state
surfaces. All twelve domains are covered by local and HTTP restart tests; duplicate episode
events, evaluations for unknown episodes, malformed normalized packets, broken chain/head/event
digests, raw-content fields, and stale writers fail closed.

The Python `AutonomousAgent` now composes the three restart-sensitive selection inputs at its
application boundary: evaluator/bandit learning, episodic memory, provider/model health, and
restart-safe runtime circuit/transport health.
Each coordinator must be bound to the exact ledger/store used by the agent, and each restore or
flush remains explicit so deployments can order evaluator feedback, health observations, and
memory writes inside their own transaction. The façade exposes aliases for online learning and
provider health, while all-domain restart tests verify CAS fencing, secret/task redaction, and
misconfiguration refusal. The runtime-health coordinator is identity-bound to the exact
`LLMRuntime`, snapshots only bounded transport metadata, and uses canonical digest validation plus
CAS fencing. This closes the local integration gap without turning historical health into
authorization or treating provider success as task reward; storage, identity, approval,
encryption, and external reconciliation remain deployment-owned.

Python objective state now has the matching goal handoff boundary. `AutonomousGoalLedger` snapshots
carry the sorted current objective records and full lifecycle event chain with strict sequence,
state-binding, retention, head, and outer digest checks. Restore rebuilds the SQLite current-state
index atomically, while JSON and conditional-write/HTTP coordinators fence stale revisions. The
all-domain test matrix covers restart, tampering, lifecycle consistency, and stale writers without
persisting task text, provider payloads, credentials, or raw criterion evidence.

The lower-level Python `ProviderHealthLedger` now has the same transport boundary: canonical
provider/model observations can be snapshotted, restored atomically, and handed through the
conditional-write/HTTP adapter with stale-writer fencing. Its all-domain tests cover restart and
tamper refusal while keeping request messages, response text, headers, credential handles, and
model prompts outside historical transport evidence.

The Python reviewed-evidence surface now also has a provider-backed LLM acquisition seam. The
`AutonomousLLMEvidenceAdapter` binds a reviewed requirement to the existing `LLMRuntime`, supports
static or context-selected models, structured response parsing, caller-owned prompt builders,
credential handles or explicitly credentialless local providers, and metadata-only projections.
`AutonomousLLMEvidenceAdapterRouter` requires an explicit per-domain mapping for cross-domain runs,
and `AutonomousAgent.run_with_llm_evidence` / `run_resumable_llm_evidence` compose that mapping with
source approval, evidence evaluation, provider approval, journaling, and restart checkpoints.
The adapter rejects secret-shaped response fields and malformed provider output; no credential,
prompt, or provider response is placed in durable evidence state. This closes the Python gap with
the TypeScript LLM evidence adapter while leaving provider registration, credential onboarding,
model selection policy, and external network authorization caller-owned.

The next Python increment makes that seam an explicit autonomous decision boundary rather than a
caller-owned callback convention. `AutonomousLLMEvidenceAdapterRegistry` freezes bounded,
digest-addressed adapter manifests across all twelve built-in domains; a registry replacement
invalidates prior selection plans instead of silently redirecting a run.
`AutonomousLLMEvidenceAdapterSelector` supports deterministic lexicographic selection for
reproducible operation and weighted adaptive selection from a validated health signal projection.
`InMemoryAutonomousLLMEvidenceAdapterHealthStore` records a hash-chained acquisition/evaluation
ledger, derives bounded success-quality-latency signals, and opens failing adapter circuits
without persisting prompts, provider payloads, credentials, or raw error text. JSON and
conditional-write coordinators provide restart recovery with compare-and-swap fencing.
`AutonomousLLMEvidenceAdapterFailoverAcquirer` verifies the selection digest before every run,
retries only bounded retryable provider failures, fails closed on malformed prompts or credential
errors, records fallback metadata, and exposes explicit evaluator reward credit for online
adaptation. The failover adapter implements the existing `acquire`/`project` contract, so it can
be passed directly to `AutonomousAgent.acquire_evidence`, `run_with_llm_evidence`, or the
resumable evidence boundary without widening durable state.

Python now closes the adjacent operational-readiness gap with
`AutonomousLLMEvidenceReadinessAuditor`. It projects coverage, the exact registry and selection
digests, selected-manifest health, open-circuit state, bounded failover policy, and explicit
`ready`/`degraded`/`blocked`/`missing` rows for every requested domain. Strict default policy
requires observed health and a minimum success rate; `require_health=False` is an explicit
degraded startup posture rather than an authorization shortcut. The canonical report supports
strict round-trip validation and is composed into `AutonomousAgent.readiness()` through the
caller-owned `evidence_readiness` configuration. No source, provider, model discovery, credential,
or learner mutation occurs during the audit, and the report excludes prompts, requests, values,
responses, keys, and raw errors.

The next Python evidence increment now makes provider and source assumptions executable. Each
`AutonomousEvidenceProviderContract` is digest-bound to one adapter manifest and declares the
provider protocol, operations, domains, capabilities, source kinds, auth posture, freshness,
pagination, and required request metadata. The registry verifies those bindings immediately before
each selected failover candidate is invoked, so stale manifests, undeclared capabilities, and
missing operations fail closed without entering the provider boundary. The source admission layer
adds a caller-owned descriptor contract for source identity, digest, authority, status, observation
time, expiry, citation, and limitations; its freshness/authority policy records accepted and
refused decisions in a metadata-only hash chain. Canonical JSON and compare-and-swap persistence
support process restart and stale-writer rejection. All twelve domains are covered by offline
tests for contract coverage, failover, refusal, secret-shaped values, tamper resistance, and
restart recovery. This remains an admission and provenance boundary—not provider authentication,
source authenticity, or truth validation—and retains no credentials, prompts, responses, source
values, or locators.

The Python failover path now closes the retry-versus-failover distinction. A typed
`AutonomousEvidenceRetryPolicy` classifies bounded transient failures, retries one exact reviewed
route with capped exponential backoff, and emits attempt number, status, failure class, delay, and
latency without persisting error text or values. `AutonomousLLMEvidenceSourceBoundary` composes the
provider contract and metadata-only source admission inside every candidate route, including each
retry, while the separate failover budget advances only for classifications permitted by the retry
policy. Readiness serialization now round-trips the nested retry policy, and all-domain tests cover
recovery, source receipts, refusal boundaries, no-raw projections, and retry telemetry.

Python now adds the missing multi-source evidence adjudication layer. `AutonomousEvidenceSourceReconciler`
creates a request-free, digest-bound plan for up to sixteen caller-owned routes, explicit quorum,
bounded concurrency, parent evidence lineage, and a named normalizer version. Execution is approval-
gated and produces deterministic `consensus`, `consensus_with_dissent`, `disagreement`,
`insufficient_evidence`, or `failed` status without converting provider agreement into truth. Source
acquisition and normalization failures become value-free per-route metadata, while transient source
and normalized values remain available only to the caller. Strict plan/result round trips reject
route drift, normalizer drift, tampering, credential-shaped metadata, oversized values, and missing
approval. All twelve domains are covered by consensus/dissent, disagreement, failure, and bounded
fan-out tests.

The next autonomous brain increment closes the final implicit prompt boundary in both SDKs.
`AutonomousPromptTemplate` binds a caller-owned renderer to a versioned domain/stage/capability
manifest, template digest, optional output-contract digest, message bound, and byte bound.
`AutonomousPromptRegistry` produces deterministic, digest-addressed selection plans with exact
stage preference, capability-fit ordering, candidate identities, and registry-drift refusal.
Rendering verifies the plan before executing the transient renderer, validates provider-neutral
message roles and JSON safety, rejects credential-shaped fields, and exposes only a prompt digest
and bounded metadata in its projection. Python and TypeScript LLM evidence adapters accept the
registry/selection boundary and bind the rendered-prompt digest into provider idempotency. The
all-domain tests cover selection, stale replacement, tampered plans, secret-shaped messages,
metadata redaction, and offline invocation. This is still not provider authorization: provider
credentials, model dispatch, source acquisition, tool execution, effects, evaluator credit, and
online learning remain separate explicit gates.

The built-in prompt pack now turns the generic prompt control plane into an immediately useful
cross-domain starting point. `builtin_autonomous_prompt_registry()` and
`builtinAutonomousPromptRegistry()` provide one content-addressed specialist renderer for every
autonomous domain, with domain-specific reasoning, provenance, safety, coordination, multimodal,
operations, governance, or evaluation guidance and capability labels. Built-in rendering accepts
only a bounded reviewed objective, returns transient system/user messages, and remains behind an
explicit registry selection plan; no provider, key, tool, effect, or learner authority is
implicit. Python and TypeScript tests cover complete twelve-domain coverage, capability-bound
selection, subset construction, duplicate/unsupported/missing-objective refusal, and projection
redaction.

Provider-assisted planning now uses the versioned prompt control plane as well. Single-domain,
cross-domain, ordered-step, and plan-and-run planner calls accept prompt template/registry/
selection controls at the explicit `planning` stage, verify stale selections before dispatch,
and bind the transient planner prompt digest into the planner outcome identity. Planner messages
remain transient and all result projections remain digest-only; offline Python and TypeScript
coverage exercises approval gating, specialist prompt delivery, and all-domain planner parity.

The next autonomous brain increment adds evaluator-driven prompt adaptation without turning
provider output into self-authority. Python and TypeScript now expose a caller-owned
`AutonomousPromptLearningState` containing only registry-bound prompt-arm identities, bounded
pull/failure/reward statistics, and a capped replay ledger. `select_adaptive_autonomous_prompts`
and `selectAdaptiveAutonomousPrompts` use deterministic UCB1 exploration: unobserved prompt
variants are tried first, then the highest value-plus-exploration arm is selected with stable
tie-breaking. `settle_autonomous_prompt_selection` and its TypeScript equivalent require an
explicit evaluator id/version, bounded reward, pass signal, outcome digest, and optional
settlement key; repeated keys replay without double credit. State and selection digests bind
every choice to the current prompt registry and manifest, so replacement, stage drift,
capability drift, malformed ledger fields, and stale plans fail closed. Direct and cross-domain
execution plus provider-assisted planning accept the adaptive state and project only the
selection digest, arm identity, generation, and policy; tasks, rendered messages, provider
payloads, evaluator feedback, credentials, and secrets remain outside durable learning state.

Durable workflow stages now consume the same adaptive prompt state. Python and TypeScript
workflow executors forward state and exploration policy into every stage, child, and synthesis
invocation, selecting against the actual stage/domain/capability request rather than one global
answer arm. Workflow contract digests bind the prompt registry and exploration policy while
allowing caller-settled reward state to advance between resumptions; registry replacement,
exploration drift, stale state, stage drift, and malformed adaptive state still fail closed
before provider dispatch. All-domain workflow coverage asserts adaptive selection metadata at
the stage boundary, with no prompt text, task text, credentials, or provider payloads entering
checkpoints or learning state.

The prompt learner is now restart-safe at the SDK boundary. Python and TypeScript provide
canonical, digest-bound JSON snapshots and registry-bound persistence coordinators. A plain
adapter supports caller-owned durable storage; a transactional adapter requires compare-and-set
semantics so concurrent workers cannot overwrite a newer learner generation. Restore, flush, and
settlement are serialized, settlement generation advances happen only after persistence succeeds,
and failed stale writes roll back the local state. Snapshots are value-only and bounded: they keep
arm statistics, replay keys, registry identity, generation lineage, and retention markers, while
rejecting prompt text, tasks, provider payloads, evaluator content, credentials, secrets, tampering,
registry drift, malformed state, and oversized images. Focused Python and TypeScript coverage now
exercises all-domain recovery, idempotent replay, stale-writer fencing, registry replacement, and
tamper rejection.

High-level application runs now consume the persistent prompt learner directly. Agent facades bind
the coordinator's registry/state to direct and cross-domain execution, expose a bounded
registry-verified `adaptive_selection` receipt in each result, and provide explicit selection
extraction plus evaluator settlement helpers. Python and TypeScript coverage exercises all twelve
domains, specialist fan-out, synthesis, generation persistence, restart recovery, secret/task
redaction, and refusal of external state overrides. Provider success still cannot credit a prompt
arm: only the caller's evaluator settlement advances the CAS-fenced learner.

Provider planning is now settlement-visible at the same boundary. Single-domain, cross-domain,
ordered-step, and automatic planning results expose the exact adaptive selection metadata used to
render the transient planner prompt. Direct planning methods and automatic runs bind the configured
persistent coordinator, including planning-specific option aliases, and reject registry/state
replacement attempts. This closes the planning-to-learning handoff without persisting prompts,
tasks, credentials, provider transcripts, or evaluator payloads; the remaining production work is
caller integration of explicit evaluator signals and durable storage policy for each deployment.

The TypeScript facade now has the same high-level automatic entrypoint as Python. `runAuto()` can
route and execute any built-in domain or a bounded cross-domain fan-out, while preserving the
provider-free blueprint boundary and returning a typed next action for route, plan, provider, or
effect review. Its provider mode reuses the shared aggregate budget and existing plan-acceptance
bridge, so callers do not get a second implicit planning or invocation path. The remaining
deployment responsibility is still explicit credential, evaluator, effect, and durable-store
integration rather than hidden SDK authority.

The next TypeScript brain increment closes the remaining high-level automatic-cycle parity gap.
`runAutoCycle()` and `runAutonomousAutoDecisionCycle()` resolve one deterministic or explicitly
approved semantic route, choose the matching single-domain or cross-domain decision-cycle kernel,
and pass the route back as a digest-verified override. The result retains the nested cycle,
evaluator settlement, online learner/bandit updates, provider-planning review, and restart cursor
without duplicating route logic or making provider success into reward. A shared cost budget spans
semantic routing, planning, fan-out, synthesis, and execution. All built-in single-domain profiles,
cross-domain execution, and semantic approval refusal are covered by offline TypeScript tests;
credentials, evaluator evidence, effects, and durable stores remain explicit application
responsibilities.

That automatic-cycle parity gap is now closed for evaluator-guided replanning as well. The
TypeScript `runAutoReplanCycle()` / `runAutonomousAutoReplanCycle()` facade resolves the route
once, dispatches to the matching replan kernel, preserves evaluator-driven bounded attempts,
and forwards learning, provider planning, shared budgets, and restart rehydration. Coverage now
includes all built-in single-domain profiles, a real bounded replan, cross-domain fan-out,
semantic approval refusal, and terminal replay without a second provider call.

The next depth layer remains deployment integration rather than hidden authority: connect these
facades to caller-owned evaluator evidence, durable result/rehydration stores, effect
reconciliation, credential provisioning, and production observability. Those integrations must
continue to preserve the existing route, approval, secret, and value-only learning boundaries.

The cycle evaluator bridge now closes the callback-plumbing gap between those built-in evaluator
contracts and live TypeScript decision/replan cycles. `createAutonomousCycleEvaluatorBridge()`
supports every built-in domain, ordinary and adaptive single-domain runs, and specialist/synthesis
cross-domain credit while exposing only metadata to the caller's evidence factory. The registry,
evaluator catalogue digest, policy digest, and explicit evidence boundary are covered offline;
source acquisition, evaluator truth, and durable evidence storage remain caller-owned.

Python now has the same reusable evidence boundary through
`create_autonomous_cycle_evaluator_bridge()`. The bridge validates all twelve autonomous profiles,
preserves exact single-domain evaluator identity, routes cross-domain specialist/synthesis steps
through their exact profiles, exposes catalogue/policy digests, rejects inline evidence, and keeps
provider completion outside reward. Caller-owned evidence acquisition, truth authority, durable
evidence storage, and production evaluator operations remain deployment work.

Python now also closes the all-domain pre-dispatch contract-audit parity gap.
`agent.domain_audit()` and `audit_autonomous_domain_contracts()` verify profile/workflow
registries, default-capability closure, stage DAG/evidence/evaluator contracts, exact tool
binding posture, and caller-owned evidence coverage for every built-in domain. Reports are row-
and aggregate-digest-bound and perform no provider/source/tool/credential/queue/learning activity.
The seven Python profiles whose default capability was previously absent from their declared
catalogue are now closed over that capability. Runtime availability, source truth, authorization,
and deployment observability remain explicit external gates.

Python now closes the cross-SDK control-plane supervision gap. `AutonomousBrainControlPlaneMonitor`
and its async counterpart build on `BrainControlClient` to provide bounded status fan-out across
all twelve domains, hash-chain event cursor validation, explicit approval routing, and bounded
reached/timed-out polling. Unsafe projection fields are refused before return, and task text,
prompts, credentials, provider responses, tool arguments, and effect values remain outside the
monitor. This is operator lifecycle infrastructure, not a provider worker or authorization oracle.

Python now adds the unified `agent.launch_preflight()` handoff. It composes the all-domain
structural contract audit, model/provider/evidence readiness, and deployment capability gates into
one digest-bound report with per-domain combined state, source-report digests, blocker/warning
counts, bounded remediation, and an explicit zero-dispatch ledger. The default posture remains
blocked or partial until caller-owned inventories and deployment gates are supplied; a
`ready_for_review` row still does not grant provider, source, tool, effect, credential, or learner
authority.

TypeScript now closes the corresponding facade gap with `AutonomousBrainFacade.launchPreflight()`.
It composes the existing domain audit, keyless readiness projection, and deployment audit for all
twelve domains, validates the aggregate digest and zero-dispatch posture, and refuses secret-shaped
capability metadata before any provider/source/tool boundary.

The next handoff now records explicit review decisions as well. Python's
`agent.launch_admission()` and TypeScript's `brain.admitLaunchPreflight()` bind `approve`/`hold`
to the exact preflight digest, retain all twelve domain admission states, require an external
authorization digest for approval, and never turn the record into provider, source, tool, learner,
credential, queue, or effect authority. Deployment-owned schedulers still decide whether and how
to bind that review record to execution.

Launch admission is now executable at the high-level boundary as well. TypeScript direct,
decision-cycle, and adaptive-cycle facade entrypoints validate the admission after provider-free
route planning and before connector/provider dispatch; Python direct and cross-domain wrappers do
the same, and `run_auto_with_launch_admission` covers automatic single/cross-domain routing. The
automatic paths reject provider-assisted semantic routing until that classifier boundary is
separately reviewed, preventing a provider call from occurring before a domain-scoped launch
decision. Provider, source, tool, learner, queue, credential, and effect authority remain
independent deployment controls.

The next-action handoff is now executable as metadata as well. Python `agent.action_plan(...)` and
TypeScript `brain.actionPlan(...)` project the existing route, evidence plan, domain policy, task
intent, and task decision into one digest-bound single-domain or cross-domain action plan. The
plan deterministically prioritizes route review, policy resolution, evidence acquisition, plan
acceptance, effect review, provider approval, and evaluator settlement, and round-trips with
candidate-level tamper checks across all twelve domains. It remains provider/source/tool/effect
free; production deployments still own the caller-controlled admission, credential, evaluator,
queue, observability, and reconciliation integrations.

The action-plan boundary now has an explicit admission/execution handoff. Python
`agent.admit_action_plan(...)` / `agent.execute_action_plan(...)` and TypeScript
`brain.executeActionPlan(...)` replay the plan from transient task and route inputs, bind review
gates to the plan digest, and return before dispatch when any gate is missing. An admitted plan
selects the existing provider, evidence-first, workflow, planning, or cross-domain kernel while
leaving credentials, evidence, evaluator settlement, connector execution, effects, and durable
authorization caller-owned. Remaining deployment work is to connect these handoffs to an
application-owned authorization store and operator UI, then exercise them in the live worker
and release environments.

The action-plan deployment seam is now implemented as a durable review ledger in both SDKs.
`InMemoryAutonomousActionAdmissionLedger` stores revisioned plan/admission records, reviewer
authorization digests, reason digests, and predecessor links; a review derives a fresh admission
from the exact stored plan instead of mutating approval state in place. Canonical JSON snapshot
adapters, generation links, and transactional compare-and-set fencing support restart and
multi-writer recovery. All twelve domains and cross-domain plans are covered, and the ledger
remains metadata-only. The remaining production responsibility is wiring the caller's identity
provider/operator UI and secret/effect/evaluator systems to these explicit records.

The operator review surface now sits directly above the ledger in both SDKs. The controller
projects all twelve domains, requires an external authorization digest plus an expected record
digest for review, rejects held/blocked records at the dispatch-handoff boundary, and returns
separate downstream credential, provider/source, tool/effect, and evaluator gates. It remains a
projection and handoff API rather than an execution or authorization oracle; deployment identity
verification and the actual UI remain caller-owned.

The high-level brain façades now consume that verified handoff directly as well. Python
`execute_action_handoff()` and TypeScript `executeActionHandoff()` replay the transient request,
reproduce the admitted gate set, and delegate to the existing route/model/provider boundary for
all twelve built-in domains plus cross-domain plans. Handoff continuity still does not replace
credentials, provider/source readiness, evaluator evidence, tool/effect authority, or durable
deployment authorization.

Long-horizon goals now have the same reviewed execution seam. Both goal-agent runtimes accept a
caller-owned `action_handoff_resolver`; it can return a plain handoff or a `{handoff, request}`
binding for transient cross-domain routing inputs. The worker validates the handoff before claim
and the runtime revalidates it at execution, then invokes the high-level handoff method rather than
falling back to an unreviewed raw run. The binding is excluded from goal/schedule/control-loop
projections, while the existing caller-owned credentials, provider/source, evaluator, tool/effect,
and deployment authorization responsibilities remain explicit. Remaining production work is still
application wiring: persist protected task/request rehydrators, connect real identity/approval
stores, and exercise restart/reconciliation behavior against the deployment's durable worker.

The goal worker/restart seam is now stricter. Both SDKs verify that protected task rehydration
matches the immutable goal `task_digest` before claim, and journal prepared/claimed/dispatch/settled
events can carry only a task digest plus an `execution_binding_digest` for transient parameters
(including action handoffs). Raw task text, handoffs, credentials, prompts, provider values, and
results remain excluded. `activeFor`/`active_for` plus `assertNoActive`/`assert_no_active` fence a
new worker pass until active pre- or post-dispatch events are recovered/reconciled, so a restart
cannot silently substitute a different task or handoff. Tests cover drift refusal, digest propagation,
metadata-only persistence, ordered coordinator recovery, tamper rejection, and the all-domain
worker path. `AutonomousGoalRecoveryCoordinator` now composes journal and control-loop startup in
both SDKs: it restores the journal first, reconciles active boundaries, flushes that reconciliation
through the journal CAS fence, and only then exposes the loop checkpoint. Its sealed report and
guarded resume helper preserve the metadata-only boundary and make post-dispatch uncertainty
explicit. Remaining production work is application wiring: durable protected rehydrators, real
identity/approval stores, deployment-level ledger/journal atomicity, and external resolution of
genuinely uncertain provider/effect outcomes.

Capability-level automatic intake is now also shared by both SDKs. After domain routing, a
provider-free vocabulary router proposes a more specific reviewed capability for every built-in
domain, including debugging versus implementation, lineage versus analysis, rollback versus
observability, biomedical safety versus provenance, multimodal alignment, specialist synthesis,
and evaluation replay. Confidence and margin thresholds abstain instead of guessing, explicit
caller capabilities remain authoritative, and the selected value flows into task intent,
model-selection context, learning identity, and tool planning. The proposal is digest-bound and
metadata-only; task text, prompts, credentials, provider payloads, tool arguments, and effects
remain caller-owned. The remaining production responsibility is still to connect those reviewed
capabilities to deployment-specific adapters and evidence sources rather than treating a lexical
classification as execution or domain truth.

Cross-domain capability propagation is now aligned across SDKs: each specialist child resolves
its own capability before tool-portfolio ranking, and the selected/default value is bound into its
task intent, model-selection context, learning identity, and compiled workflow-step arguments.
The open deployment work is to bind these reviewed child contracts to caller-owned tool/source
catalogues, evaluator evidence, and approval records; the deterministic route remains neither
provider authority nor effect authority.

TypeScript now exposes provider invocation and failover receipts on autonomous execution results,
matching the existing Python provider-audit seam. The records are ordered, digest-bound,
metadata-only projections of provider/model attempts, turns, token and cost counters, latency,
failure classification, request-id digests, and bounded failover strategy; direct runs, tool loops,
all twelve built-in domains, and cross-domain child aggregation are covered by tests. This closes
the SDK result-observability gap without treating transport success as task correctness. Remaining
deployment work is to connect these receipts to caller-owned evaluator settlement, durable trace
stores, provider cost ledgers, and operator policy surfaces; credentials, raw payloads, reward, and
effect authority remain explicitly outside the SDK receipt boundary.

Both SDKs now also expose conservative run-trace analytics above the verified metadata journal.
`analyze_autonomous_run_trace()` / `analyzeAutonomousRunTrace()` aggregate all twelve reviewed
domains plus observed provider and model dimensions into digest-bound reports with terminal
coverage, status/phase counts, failure codes, measured latency quantiles, token observation
counts, tool-call counts, attribution gaps, and deterministic threshold alerts. Missing metrics
remain explicitly `null`/`unmeasured`; the layer does not infer cost, provider health, task
correctness, or domain truth. Reports retain only metadata and carry explicit authority and
retention markers. A bounded `AutonomousRunAnalyticsLedger` now provides longitudinal ingestion,
retained-window deduplication/conflict classification, all-domain/provider/model rollups,
eviction accounting, digest-verified restore, canonical JSON persistence, and optional CAS
fencing in both SDKs. It still does not become an evaluator, provider-health oracle, billing
ledger, or alert delivery service. Remaining deployment work is to connect the ledger to
caller-owned tenant authorization, durable placement/backup, alert routing, evaluator
settlement, provider billing, and external health sources without weakening the value-free
boundary.

The Python SDK now closes the corresponding effect-safety gap. `AutonomousEffectBoundary` gives
approved non-read-only domain tools a deterministic effect identity, caller-visible idempotency
key, hash-chained `prepared`/`dispatching`/`dispatched` markers, conservative uncertain-failure
handling, resolver-gated replay, and canonical compare-and-set snapshots. `AutonomousAgent` and
`AutonomousDomainToolRuntime` accept the boundary, while an optional three-argument
`effect_executor` receives the transient idempotency context. The boundary is metadata-only and
never stores arguments, outputs, prompts, tasks, provider payloads, credentials, or raw errors;
all twelve built-in domains are covered by integration tests. The remaining deployment work is
still caller-owned effect-store/resolver wiring, external idempotency enforcement, durable ledger
placement, and operator reconciliation policy; exactly-once execution is not claimed by the SDK.

Evaluator-gated memory now closes its high-level prompt loop in both SDKs. Direct, automatic,
workflow, and cross-domain runs can query stable consolidated lessons per routed domain and resolve
their digest through an explicit caller-owned callback immediately before prompt assembly. Local
lessons remain domain-scoped, explicitly portable lessons are deduplicated across fan-out, and
candidate/stale/conflicted rows are excluded. The prompt receives transient advisory text with
non-authority/non-effect framing, while run projections and selection/request identity retain only
lesson IDs, lesson digests, and a consolidated retrieval digest. Required mode fails closed when
the index or resolver is unavailable; default mode preserves advisory memory failure semantics.
All twelve built-in domains have approval-only integration coverage without API keys or provider
dispatch. This increment adds the context-aware resolver bridge, canonical bounded JSON/in-memory
lesson-text adapters, and a restart-safe evaluator-to-consolidator scheduler in both SDKs. Resolver
authorization now sees lesson scope, eligible domains, capabilities, risk classes, confidence, and
requested domain/capability before text is read; the consolidation and scheduler snapshots remain
text-free, and every queue lease/job/report identity is tamper-detected. Scheduling is explicit,
priority/age deterministic, retry-bounded, quarantined after exhaustion, and projected across all
twelve domains. Remaining deployment work is to supply encryption, tenant identity/access control,
protected rehydration, and external exactly-once effect reconciliation; the SDK does not invent
those authorities.

Protected rehydration is now implemented as a shared SDK boundary in both runtimes. A caller can
bind an opaque reference to tenant, actor, session, authorization, purpose, domain, expiry, and
the digest of a value held in caller-owned storage. Resolvers and optional authorizers are invoked
only after context and replay checks; one-time references are consumed only after the returned
value matches its expected digest, failures are bounded and quarantineable, and snapshots contain
no protected value. The memory-consolidation scheduler now optionally binds its durable policy,
claims, and worker results to the same execution-context digest, so a restored queue cannot be
loaded under another tenant or authorization context. This closes the SDK contract gap while
leaving vault encryption, identity issuance, authorization decisions, and external effect
 reconciliation explicitly deployment-owned.

The shared protected boundary now also has a receipt adapter in both SDKs. Evidence runtimes
and connector-backed workflow/mission executors can derive an opaque, context-bound reference
from a metadata-only receipt and rehydrate a caller-owned value without requiring every caller
to implement a separate resolver callback. Explicit legacy callbacks remain supported and take
precedence; the protected adapter is a deterministic fallback that binds purpose, domain, and
the receipt's value/payload digest. All twelve domains are covered by cross-SDK adapter tests,
and the TypeScript domain catalog is isolated into a dependency-leaf module so importing the
public barrel cannot trigger an autonomous-facade module cycle. The vault, identity provider,
authorization authority, encryption, and external effect reconciliation remain deployment-owned.

The same fallback now reaches the long-horizon goal agent. When an application omits a bespoke
task resolver, each goal is reconstructed through a caller-owned protected `goal_task` receipt
just before dispatch, while an explicit resolver still takes precedence. Goal identities are raw
UTF-8 task SHA-256 digests, so the adapter now supports bounded `canonical_json` and `utf8_sha256`
schemes and binds the selected scheme into the opaque reference. This closes the goal-runtime
rehydration seam without putting task text, private runtime handles, credentials, or provider
payloads into the ledger, journal, snapshot, or result. Production deployment still owns the
resolver store, authorization context, rotation, and uncertain external-effect reconciliation.

The restart-safe high-level brain batch controller now consumes the same protected receipt
boundary. `AutonomousBatchProtectedRehydration` / `AutonomousBrainBatchProtectedRehydrator`
receives only batch identity digests, resolves a caller-owned protected result, optionally decodes
it into a typed runtime value, and lets the batch engine perform its final successful-status and
metadata-only item-digest checks. Explicit batch rehydrators remain authoritative. Receipt identity
drift, tenant/authorization mismatch, expiry, replay, digest mismatch, and invalid decoded results
fail closed before new provider work. Python and TypeScript tests cover partial restart, explicit
callback precedence, protected result lookup, tampering, and all twelve built-in domains. The
remaining deployment responsibility is still the encrypted result store, identity/authorization
authority, retention/rotation policy, and reconciliation of genuinely uncertain external effects.

The protected receipt boundary now reaches the durable high-level brain workers. Python sync/async
remote workers and the TypeScript durable worker can reconstruct private job resolutions from
caller-owned receipts bound to job/spec/domain/capability/attempt/approval identity, with explicit
resolver precedence and async lookup support. Focused tests cover all domains, tampering, approval
gates, and metadata-only persistence. Deployment work remains the caller-owned receipt/vault,
authorization, rotation, and external-effect reconciliation integration.

The TypeScript remote control-plane worker now shares this path with the local worker: callers may
provide `protectedRehydration` without implementing a bespoke `resolve` callback. Remote tests
rehydrate every built-in domain through the queue, verify approval-gated restart behavior, reject
tampered spec identity before dispatch, and prove explicit resolver precedence. The remaining
deployment work is still intentionally external: receipt storage, encryption, identity and
authorization issuance, retention/rotation, and reconciliation of uncertain effects.

Protected provider-effect reconciliation is now implemented in both SDKs. A caller-owned receipt
resolver can rehydrate provider status through the shared tenant-bound protected boundary while
the journal retains only effect identity digests and lifecycle metadata. The receipt is bound to
effect/call/provider/operation/attempt identity, raw idempotency keys remain transient, and all
built-in domains are covered by tamper and replay tests. Remaining deployment work is the actual
provider status authority, encrypted receipt storage, identity/authorization issuance, rotation,
and operator policy for genuinely uncertain external effects.

Generic provider-neutral evidence adapter orchestration is now at parity across the SDKs. Python
exposes `AutonomousEvidenceAdapterRegistry`, digest-bound deterministic/weighted selection plans,
metadata-only health observations with hash-chained JSON/CAS restart persistence, an adaptive
health controller, and explicitly budgeted retry/failover over reviewed candidates. The surface
covers all twelve built-in domains and rejects registry/selection drift, open circuits, tampered
snapshots, unsupported signals, and secret-shaped metadata before source dispatch. The existing
LLM-specific orchestration remains available for prompt/model-backed evidence; the generic layer
is for caller-owned file, browser, database, scientific, enterprise, and connector adapters.
Deployment responsibilities remain unchanged: source truth, credentials, approval, encrypted
storage, distributed leases, external network authorization, and evaluator authority stay outside
the SDK.

Generic evidence execution is now composed in Python as well as TypeScript. The reviewed execution
controller gates source dispatch, rechecks readiness, enforces provider/source contracts, and
drives the existing runtime through bounded failover. The resumable controller adds canonical
checkpoint/CAS persistence and append-only replay revisions, so the all-domain facade can recover
without issuing an implicit duplicate source call. Remaining deployment work is still caller-owned:
credential storage and rotation, source truth, identity and authorization, distributed leasing,
encrypted raw-value retention, and reconciliation of uncertain external effects.

The high-level TypeScript agent now composes the same restart-safe LLM transport-health boundary
already available in Python. `runtimeHealthPersistence` must be bound to the exact `LLMRuntime`,
and `restoreRuntimeHealth()` / `flushRuntimeHealth()` (plus transport-health aliases) explicitly
restore and CAS-flush provider/model counters and circuit continuity. This keeps provider
availability recovery aligned across SDKs without restoring credentials, prompts, responses,
evaluator rewards, or authorization; provider registration and deployment checkpoint ordering
remain caller-owned.

The TypeScript `AutonomousAgent` now also retains one lazy, serialized
`AutonomousModelInventoryCoordinator` for its lifetime. Repeated high-level inventory refreshes
reuse the last successful CAS expectation, and `restoreModelInventory()` rehydrates the validated
metadata-only catalogue while preserving that fence for the next refresh. This removes the false
stale-writer failure caused by constructing a fresh coordinator for every refresh and covers the
same all-domain model-discovery/restart boundary without restoring credentials, provider payloads,
or evaluator quality claims.

Evaluator calibration is now a first-class lifecycle on both high-level agents. Python and
TypeScript can register validated aggregate reports, restore them through a registry-bound
coordinator, and flush them with the last snapshot digest retained for CAS fencing. Readiness can
resolve a specific report by digest after restart, while rejecting missing reports, conflicting
inline/digest inputs, and cross-registry persistence bindings. The lifecycle deliberately persists
only evaluator metrics, report digests, and registry metadata; calibration cases, labels, evidence,
prompts, responses, credentials, and evaluator authority remain caller-owned. Learning admission
continues to fail closed until the explicitly selected report is validated and admitted.

Python model inventory now matches the TypeScript restart lifecycle. The high-level agent lazily
retains one store-bound persistence coordinator, persists refreshes with the last successful
snapshot digest, restores the metadata-only catalogue in place, and fences stale writers with
compare-and-swap. A failed refresh persistence operation rolls the live catalogue back to its
pre-refresh image. All-domain coverage remains provider-discovery metadata only: credentials,
circuits, quality priors, evaluator evidence, and selection authority are not restored from the
inventory snapshot.

Both SDKs now expose a bounded recovery planner for the failure path that sits after route,
provider, tool, response-quality, and mission decisions. `planAutonomousRecovery()` /
`plan_autonomous_recovery()` converts an explicit value-only status observation into ordered next
actions, stable reason codes, retry-budget state, and domain-specific escalation guardrails for
all twelve domains. Reconciliation outranks retry, missing credentials remain a collection step,
and exhausted/unclassified failures stop and escalate. The plan is digest- and retention-validated
and contains no task text, prompts, provider values, credentials, tool arguments, or effect data;
it is a guidance handoff only and does not execute recovery or grant authority.

The recovery path now also has a durable review process in both SDKs. `AutonomousRecoveryHandoffLedger`
accepts idempotent plan/run-digest/attempt submissions, retains only bounded metadata, and exposes
revision-fenced decisions for retry approval, uncertain-effect reconciliation, escalation, and
closure. Credential collection cannot be bypassed by retry approval, reconciliation cannot be
downgraded into a provider retry, and all transitions are independently digest-bound. Canonical
snapshot persistence and optional compare-and-swap coordinators restore the queue without replaying
provider work. The control plane remains intentionally non-executing: deployment-owned request
rehydration, provider/source/tool/effect authority, external reconciliation, reviewer identity,
encryption, tenant isolation, retention, leases, and evaluator settlement are still required.
