# Scoped local JSON artifact publication

`python/prism_sdk/workflow_artifact.py` is a repository-defined application sink. It has no
blueprint implementation claim. It publishes one approved canonical JSON artifact through the
existing autonomous authorization, execution policy, SQLite effect journal, and effect boundary.
It is independent of executable workflow plans: the read-only workflow executor and catalog still
refuse general effectful operations. A caller must explicitly compose this application API.

## Application setup and approval

The application chooses an existing private root and private absolute SQLite paths. It must
exclude untrusted writers from the root, its ancestors, and its metadata databases. The sink does
not make directories or accept nested output paths. Output names are bounded ASCII JSON basenames;
traversal, path separators, alternate data streams, and Windows device names are refused.

```python
from pathlib import Path
import time

from prism_sdk.autonomous_authorization import (
    AutonomousAuthorizationContext,
    AutonomousAuthorizationGate,
    AutonomousAuthorizationLedger,
    AutonomousAuthorizationPersistenceCoordinator,
    TransactionalJsonAutonomousAuthorizationSnapshotPersistence,
)
from prism_sdk.autonomous_effects import SQLiteAutonomousEffectJournal
from prism_sdk.autonomy_persistence import (
    AutonomousExecutionController,
    AutonomousExecutionPolicy,
)
from prism_sdk.workflow_artifact import (
    SQLiteWorkflowArtifactAuthorizationStore,
    WorkflowArtifactSink,
)

# These absolute, application-owned directories already exist.
artifact_root = Path("/application/private/artifacts").resolve()
metadata_root = Path("/application/private/metadata").resolve()
policy = AutonomousExecutionPolicy(allow_side_effects=True, max_effectful_calls=1)

with (
    SQLiteWorkflowArtifactAuthorizationStore(metadata_root / "authorization.sqlite3") as store,
    SQLiteAutonomousEffectJournal(metadata_root / "effects.sqlite3") as journal,
):
    ledger = AutonomousAuthorizationLedger()
    persistence = AutonomousAuthorizationPersistenceCoordinator(
        ledger, TransactionalJsonAutonomousAuthorizationSnapshotPersistence(store)
    )
    persistence.restore()
    sink = WorkflowArtifactSink(
        artifact_root, journal=journal, authorization_persistence=persistence
    )
    prepared = sink.prepare(
        plan_digest=approved_plan_digest,
        task_digest=approved_task_digest,
        catalog_digest=approved_catalog_digest,
        step_id="public_review_export",
        resolved_arguments={"report_kind": "public_glioma_research"},
        filename="glioma-review.json",
        value=public_review,
        policy=policy,
    )

    # The application reviews prepared.manifest() and grants its exact digest. Issuing this
    # grant is an explicit caller decision, not permission inferred from the model or a digest.
    now = int(time.time() * 1000)
    grant = ledger.issue(
        grant_id="review-export-1",
        tenant_id="research-workspace",
        actor_id="approved-operator",
        session_id="application-session-1",
        authorization_digest=prepared.manifest_digest,
        allowed_domains=("operations",),
        allowed_operations=("effect_dispatch",),
        allowed_capabilities=("workflow_artifact_write",),
        allowed_risk_classes=("local_artifact_write",),
        issued_at=now,
        expires_at=now + 60_000,
        max_uses=1,
    )
    persistence.flush()
    context = AutonomousAuthorizationContext(
        gate=AutonomousAuthorizationGate(ledger),
        grant_id=grant.grant_id,
        tenant_id=grant.tenant_id,
        actor_id=grant.actor_id,
        session_id=grant.session_id,
        authorization_digest=prepared.manifest_digest,
        domains=("operations",),
        capability="workflow_artifact_write",
        risk_class="local_artifact_write",
    )
    execution = AutonomousExecutionController(
        execution_id="workflow." + approved_plan_digest,
        domain="operations",
        capability="workflow_artifact_write",
        risk_class="local_artifact_write",
        policy=policy,
    )
    receipt = sink.publish(
        prepared, authorization_context=context, execution=execution
    )
    observed = sink.inspect(prepared)
```

The manifest binds the plan, task, catalog, step, exact resolved arguments, root identity,
filename, exact canonical content bytes, execution policy, and refusal to overwrite. The sink
checks supplied digests as consistency identifiers; it does not independently establish that a
plan was reviewed or a research claim is scientifically supported. Payloads are immutable bytes
in the prepared object; caller mutations after preparation cannot change them. The default and
maximum content bound is 1,000,000 bytes, with explicit structure/depth bounds. Canonical JSON uses
the repository's existing encoding and digest functions; `false`, `0`, `null`, and missing fields
remain distinct.

A context must bind the exact manifest and exact operation/domain/capability/risk scope. The grant
must have precisely those scopes and one allowed use. Before dispatch the sink restores the
current persisted authority, checks revocation/expiry, authorizes, and CAS-flushes consumption.
Unpersisted grants, changed tenant/actor/session/digest, broad grants, stale coordinator rebinding,
and persistence failures stop publication. User cancellation callbacks are followed by binding
checks. The final caller clock is evaluated before a fresh durable grant read immediately before
publication. Resource digests remain audit metadata; the explicit manifest comparison provides the
resource binding absent from a generic grant.

## Filesystem and recovery behavior

Cooperating processes use an OS-backed nonblocking writer lock (`msvcrt.locking` on Windows,
`flock` on POSIX). The sink checks root and ancestor identities, refuses linked or reparse entries,
writes an exclusive staging file, flushes and fsyncs its bytes, and publishes using an atomic
hard link that refuses an existing destination. It then removes staging and verifies the actual
file bytes. An existing equal file is still refused by `publish`; byte equality alone conveys no
write authorization. Filesystems without the required locking or hard-link operations fail
closed. The application must provide its private-root access controls; this is not isolation from
a malicious process with permission to mutate that root.

The existing effect journal stores metadata digests and transitions, never the artifact payload,
resolved argument values, filename, task, or arbitrary exception text. The artifact itself contains
the exact caller JSON, so the caller remains responsible for its content and data-use policy.
The SQLite authorization store retains scoped grant and event metadata. Both SQLite stores use
WAL and synchronous FULL; SQLite temporary/in-memory journal configurations are refused by this
sink.

`inspect(prepared)` performs a bounded exact-byte observation without consuming a grant. It returns
`completed`, `not_found`, or `unknown`. A busy writer lock, conflicting bytes, links, inaccessible
files, or staging residue remain unknown. Successful observations identify the content and root;
they do not establish who created a file or prove authorization.

`sink.reconcile(prepared)` additionally checks the effect journal under the same writer lock. Exact
observed bytes can settle a previously dispatched/uncertain effect as reconciled. This recovery
uses no grant and therefore works after the one-use grant has been consumed. The result explicitly
sets `authorization_proven: false`. Completed records remain completed, while a missing/conflicting
file remains a separate observation. No artifact bytes are invented from a completed journal row.

Absent/unknown observations never become retry-ready. This increment does not implement redispatch
of an existing effect, replacement, staging cleanup after process death, or automatic retry. A
future retry adapter must prove absence with no active publisher and obtain a separate explicit
retry decision without bypassing the original effect identity. `publish` refuses existing effect
records.

`cancelled=` optionally receives a caller-owned function returning a literal boolean. A stop
observed before publication leaves no output artifact; normal exception unwinding removes this
call's staging file. If a process dies, staging residue is retained as uncertainty. Once publication
is admitted, there is no rollback promise: interruptions after publication require inspection.
The private `_after_publish` method exists for crash tests; it is not a public callback API.

## Evidence and limits

Focused tests exercise actual temporary files, scoped grants, durable SQLite CAS/reopen, no-clobber
publication, process death after publication before completion bookkeeping, and two-process writer
lock contention. They cover cancellation, callback authority substitution, final-clock revocation,
wrong session and changed manifests, journal/controller/CAS failures before publication, malformed
prepared inputs, JSON bounds, and metadata redaction.

These tests establish the exercised local process-interruption behavior. Directory-entry durability
under power loss is unmeasured. Distributed leases, distributed exactly-once delivery, network or
cloud sinks, executable-workflow write operations, OAuth, secret storage, automatic retry, and
clinical decisions are not implemented. No remote source or model call is needed to publish a
previously reviewed public research artifact.
