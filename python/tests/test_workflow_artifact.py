from __future__ import annotations

from contextlib import ExitStack
from dataclasses import replace
import hashlib
import json
import os
import subprocess
import sys
from types import SimpleNamespace

import pytest

from prism_sdk.authoring import canonical_json
from prism_sdk.autonomous_authorization import (
    AutonomousAuthorizationContext, AutonomousAuthorizationGate, AutonomousAuthorizationLedger,
    AutonomousAuthorizationPersistenceCoordinator, TransactionalJsonAutonomousAuthorizationSnapshotPersistence,
)
from prism_sdk.autonomous_effects import SQLiteAutonomousEffectJournal
from prism_sdk.autonomy_persistence import AutonomousExecutionController, AutonomousExecutionPolicy
from prism_sdk.workflow_artifact import (
    PreparedWorkflowArtifact, SQLiteWorkflowArtifactAuthorizationStore, WorkflowArtifactCancelled,
    WorkflowArtifactError, WorkflowArtifactSink,
)


@pytest.fixture
def app(tmp_path):
    root = tmp_path / "artifacts"
    root.mkdir(mode=0o700)
    with ExitStack() as stack:
        store = stack.enter_context(SQLiteWorkflowArtifactAuthorizationStore(tmp_path / "authorization.sqlite3"))
        journal = stack.enter_context(SQLiteAutonomousEffectJournal(tmp_path / "effects.sqlite3"))
        ledger = AutonomousAuthorizationLedger()
        persistence = AutonomousAuthorizationPersistenceCoordinator(
            ledger, TransactionalJsonAutonomousAuthorizationSnapshotPersistence(store))
        persistence.restore()
        sink = WorkflowArtifactSink(root, journal=journal, authorization_persistence=persistence)
        policy = AutonomousExecutionPolicy(allow_side_effects=True, max_effectful_calls=1)
        yield SimpleNamespace(root=root, store=store, journal=journal, ledger=ledger,
            persistence=persistence, sink=sink, policy=policy, tmp_path=tmp_path)


def prepare(app, **changes):
    return app.sink.prepare(**(dict(plan_digest="a" * 64, task_digest="b" * 64, catalog_digest="c" * 64,
        step_id="review:one", resolved_arguments={"report_kind": "public_research"}, filename="review.json",
        value={"evidence": ["public"], "unknown": None, "measured": False}, policy=app.policy) | changes))


def authorize(app, artifact, *, grant_id="artifact-grant", **changes):
    app.ledger.issue(**(dict(grant_id=grant_id, tenant_id="tenant-a", actor_id="actor-a", session_id="session-a",
        authorization_digest=artifact.manifest_digest, allowed_domains=("operations",),
        allowed_operations=("effect_dispatch",), allowed_capabilities=("workflow_artifact_write",),
        allowed_risk_classes=("local_artifact_write",), issued_at=1000, expires_at=100000, max_uses=1) | changes))
    app.persistence.flush()
    return AutonomousAuthorizationContext(gate=AutonomousAuthorizationGate(app.ledger), grant_id=grant_id,
        tenant_id="tenant-a", actor_id="actor-a", session_id="session-a",
        authorization_digest=artifact.manifest_digest, domains=("operations",),
        capability="workflow_artifact_write", risk_class="local_artifact_write", clock=lambda: 2000)


def controller(app, **changes):
    return AutonomousExecutionController(**(dict(execution_id="workflow." + "a" * 64, domain="operations",
        capability="workflow_artifact_write", risk_class="local_artifact_write", policy=app.policy) | changes))


def publish(app, artifact=None, context=None, execution=None, **kwargs):
    artifact = prepare(app) if artifact is None else artifact
    context = authorize(app, artifact) if context is None else context
    execution = controller(app) if execution is None else execution
    return app.sink.publish(artifact, authorization_context=context, execution=execution, **kwargs)


def assert_no_artifact(app):
    assert list(app.root.glob("*.json")) == []
    assert list(app.root.glob(".aurora-stage-*")) == []


def test_real_publication_retains_exact_canonical_bytes_and_metadata_only_journals(app):
    artifact = prepare(app, value={"payload": "transient-sensitive-text", "secret": "caller-artifact-only"})
    context = authorize(app, artifact)
    execution = controller(app)
    receipt = publish(app, artifact, context, execution)
    assert receipt["status"] == "completed"
    assert (app.root / "review.json").read_bytes() == artifact._content
    assert receipt["content_digest"] == hashlib.sha256(artifact._content).hexdigest()
    assert app.sink.inspect(artifact) == receipt
    assert app.journal.get(receipt["effect_id"]).status == "completed"
    assert execution.state.effectful_calls == 1
    metadata = canonical_json(app.journal.snapshot().to_dict()) + app.store.read()
    for transient in ("transient-sensitive-text", "caller-artifact-only", "review.json", "public_research"):
        assert transient not in metadata
    assert app.ledger.get(context.grant_id).used_count == 1
    assert app.journal.verify_integrity()["verified"] is True


def test_repeated_publish_refuses_without_consuming_another_grant_and_readonly_recovery_works(app):
    artifact = prepare(app)
    context = authorize(app, artifact)
    first = publish(app, artifact, context)
    count = len(app.ledger.events())
    with pytest.raises(WorkflowArtifactError, match="already exists"):
        publish(app, artifact, context)
    recovered = app.sink.reconcile(artifact)
    assert recovered["status"] == "completed" and recovered["journal_status"] == "completed"
    assert recovered["authorization_proven"] is False
    assert recovered["content_digest"] == first["content_digest"]
    assert len(app.ledger.events()) == count


@pytest.mark.parametrize("existing", [b'conflicting', b'{"evidence":["public"],"measured":false,"unknown":null}'])
def test_existing_equal_or_conflicting_destination_is_never_overwritten_or_promoted_to_authority(app, existing):
    artifact = prepare(app)
    (app.root / "review.json").write_bytes(existing)
    context = authorize(app, artifact)
    with pytest.raises(WorkflowArtifactError, match="already exists"):
        publish(app, artifact, context)
    observed = app.sink.reconcile(artifact)
    assert observed["authorization_proven"] is False and observed["journal_status"] == "not_found"
    assert app.ledger.get(context.grant_id).used_count == 0
    assert (app.root / "review.json").read_bytes() == existing


@pytest.mark.parametrize("field,value", [
    ("tenant_id", "other"), ("actor_id", "other"), ("session_id", "other"),
    ("authorization_digest", "f" * 64), ("domains", ("research",)),
    ("capability", "other"), ("risk_class", "other"),
])
def test_wrong_context_binding_produces_no_artifact(app, field, value):
    artifact = prepare(app)
    context = authorize(app, artifact)
    setattr(context, field, value)
    with pytest.raises(ValueError):
        publish(app, artifact, context)
    assert_no_artifact(app)
    assert app.ledger.get("artifact-grant").used_count == 0


@pytest.mark.parametrize("grant_changes", [
    {"allowed_domains": ("coding", "operations")}, {"allowed_operations": ("effect_dispatch", "plan")},
    {"allowed_capabilities": ()}, {"allowed_risk_classes": ()}, {"max_uses": None}, {"expires_at": 1500},
])
def test_broad_or_expired_grants_produce_no_artifact(app, grant_changes):
    artifact = prepare(app)
    context = authorize(app, artifact, **grant_changes)
    with pytest.raises(ValueError):publish(app, artifact, context)
    assert_no_artifact(app)


def test_persisted_revocation_is_restored_before_dispatch_even_from_stale_ledger(app):
    artifact = prepare(app)
    context = authorize(app, artifact)
    other_ledger = AutonomousAuthorizationLedger()
    other = AutonomousAuthorizationPersistenceCoordinator(other_ledger,
        TransactionalJsonAutonomousAuthorizationSnapshotPersistence(app.store))
    other.restore()
    other_ledger.revoke(context.grant_id, revoked_at=1800)
    other.flush()
    with pytest.raises(ValueError, match="revoked"):publish(app, artifact, context)
    assert_no_artifact(app)
    assert app.ledger.get(context.grant_id).status == "revoked"


@pytest.mark.parametrize("change", ["plan", "task", "catalog", "arguments", "filename", "value", "policy"])
def test_any_approved_manifest_change_refuses_before_grant_consumption(app, change):
    artifact = prepare(app)
    context = authorize(app, artifact)
    changes = {"plan": {"plan_digest": "d"*64}, "task": {"task_digest": "d"*64},
        "catalog": {"catalog_digest": "d"*64}, "arguments": {"resolved_arguments": {"different": True}},
        "filename": {"filename": "other.json"}, "value": {"value": ["different"]},
        "policy": {"policy": AutonomousExecutionPolicy(allow_side_effects=True,max_effectful_calls=2)}}[change]
    with pytest.raises(WorkflowArtifactError):publish(app, prepare(app, **changes), context)
    assert_no_artifact(app)
    assert app.ledger.get(context.grant_id).used_count == 0


@pytest.mark.parametrize("when", [1, 2, 3, 4])
def test_cancellation_at_each_prepublication_callback_leaves_no_artifact_or_stage(app, when):
    artifact = prepare(app)
    context = authorize(app, artifact)
    calls = 0
    def cancelled():
        nonlocal calls
        calls += 1
        return calls == when
    with pytest.raises(WorkflowArtifactCancelled):publish(app, artifact, context, cancelled=cancelled)
    assert_no_artifact(app)
    effect_id = app.sink._boundary.effect_id(app.sink._request(artifact, artifact.manifest()))
    if when >= 3:
        assert app.journal.get(effect_id).status == "uncertain"
        observed = app.sink.reconcile(artifact)
        assert observed["status"] == "not_found" and observed["retry_safe"] is False
        assert observed["journal_status"] == "uncertain"


@pytest.mark.parametrize("when", [1, 2, 3, 4])
@pytest.mark.parametrize("mutation", ["context", "coordinator", "controller", "state"])
def test_callback_swapping_authority_or_controller_is_refused_at_every_boundary(app, when, mutation):
    artifact = prepare(app)
    context = authorize(app, artifact)
    other_artifact = prepare(app, filename="different.json", value={"different": True})
    other_context = authorize(app, other_artifact, grant_id="other-grant")
    execution = controller(app)
    calls = 0
    def cancelled():
        nonlocal calls
        calls += 1
        if calls == when:
            if mutation == "context":
                context.grant_id = other_context.grant_id
                context.authorization_digest = other_context.authorization_digest
            elif mutation == "coordinator":app.persistence.ledger = AutonomousAuthorizationLedger()
            elif mutation == "controller":execution.policy = AutonomousExecutionPolicy(allow_side_effects=True,max_effectful_calls=2)
            else:execution.state = replace(execution.state, tool_calls=0)
        return False
    with pytest.raises(WorkflowArtifactError, match="bindings changed|state changed"):
        publish(app, artifact, context, execution, cancelled=cancelled)
    assert_no_artifact(app)
    assert app.ledger.get("other-grant").used_count == 0


@pytest.mark.parametrize("callback_kind", ["cancellation", "clock"])
def test_final_callback_persisted_revocation_stops_publication_after_dispatch_intent(app,callback_kind):
    artifact = prepare(app)
    context = authorize(app, artifact)
    calls = 0
    def callback():
        nonlocal calls
        calls += 1
        if calls == (4 if callback_kind=="cancellation" else 2):
            app.ledger.revoke(context.grant_id, revoked_at=2000)
            app.persistence.flush()
        return False if callback_kind=="cancellation" else 2000
    kwargs={"cancelled":callback} if callback_kind=="cancellation" else {}
    if callback_kind=="clock":context.clock=callback
    with pytest.raises(WorkflowArtifactError, match="no longer valid"):
        publish(app, artifact, context, **kwargs)
    assert_no_artifact(app)


@pytest.mark.parametrize("failure", ["cas_conflict", "cas_exception", "journal", "controller"])
def test_persistence_failures_refuse_before_any_artifact_is_published(app, monkeypatch, failure):
    artifact = prepare(app)
    context = authorize(app, artifact)
    execution = controller(app)
    def fail(*args, **kwargs):raise OSError("transient-sensitive-path")
    if failure == "cas_conflict":monkeypatch.setattr(app.store, "write_if_unchanged", lambda *args: False)
    elif failure == "cas_exception":monkeypatch.setattr(app.store, "write_if_unchanged", fail)
    elif failure == "journal":monkeypatch.setattr(app.journal, "append", fail)
    else:monkeypatch.setattr(execution, "_persist", fail)
    with pytest.raises((ValueError,OSError)):publish(app, artifact, context, execution)
    assert_no_artifact(app)


@pytest.mark.parametrize("manifest,content", [("null",b"null"),("[]",b"null"),("{",b"null"),
    ("{} ",b"null"),("{}",bytearray(b"null")),("{}","null"),("x"*16385,b"null")])
def test_malformed_or_mutable_prepared_inputs_raise_bounded_contract_errors(manifest, content):
    with pytest.raises(WorkflowArtifactError):PreparedWorkflowArtifact(manifest,content)


@pytest.mark.parametrize("name", ["../escape.json","sub/escape.json","sub\\escape.json","a.json:ads",
    "CON.json","NUL.json","COM1.json","LPT9.json","a.json.","a\x00.json"])
def test_unsafe_basename_is_refused_before_authorization(app,name):
    with pytest.raises(WorkflowArtifactError):prepare(app,filename=name)
    assert_no_artifact(app)


@pytest.mark.parametrize("value", [float("nan"),float("inf"),"\ud800",2**64, -(2**63)-1,(1,2),{1:"bad"}])
def test_noncanonical_or_nonjson_values_are_refused(app,value):
    with pytest.raises(WorkflowArtifactError):prepare(app,value=value)


def test_nested_and_oversized_values_refuse_and_normal_prepare_detaches_caller_values(app):
    nested = None
    for _ in range(34):nested = [nested]
    with pytest.raises(WorkflowArtifactError):prepare(app,value=nested)
    with pytest.raises(WorkflowArtifactError):prepare(app,value="x"*1000001)
    value={"a":[1]};args={"b":[2]}
    artifact=prepare(app,value=value,resolved_arguments=args)
    value["a"].append(3);args["b"].append(4)
    assert artifact._content==b'{"a":[1]}'
    assert artifact.manifest()["resolved_arguments_digest"]==hashlib.sha256(b'{"b":[2]}').hexdigest()
    assert prepare(app,value=False).manifest_digest!=prepare(app,value=0).manifest_digest


def test_root_identity_change_and_hardlink_destinations_are_refused(app):
    artifact=prepare(app);context=authorize(app,artifact)
    outside=app.tmp_path/"existing.json";outside.write_bytes(artifact._content)
    os.link(outside,app.root/"review.json")
    with pytest.raises(WorkflowArtifactError):publish(app,artifact,context)
    assert app.sink.inspect(artifact)["status"]=="unknown"
    assert outside.read_bytes()==artifact._content
    (app.root/"review.json").unlink()
    app.root.rename(app.tmp_path/"previous-root");app.root.mkdir()
    with pytest.raises(WorkflowArtifactError,match="root identity changed"):publish(app,artifact,context)
    assert_no_artifact(app)


@pytest.mark.parametrize("path", ["",":memory:","relative.sqlite3",None,1])
def test_authorization_store_rejects_temporary_or_relative_configurations(path):
    with pytest.raises(WorkflowArtifactError):SQLiteWorkflowArtifactAuthorizationStore(path)


def test_sink_rejects_memory_effect_journal_and_nontransactional_authority(app):
    with SQLiteAutonomousEffectJournal(":memory:") as memory:
        with pytest.raises(WorkflowArtifactError,match="persistent"):
            WorkflowArtifactSink(app.root,journal=memory,authorization_persistence=app.persistence)
    with pytest.raises(WorkflowArtifactError,match="transactional"):
        WorkflowArtifactSink(app.root,journal=app.journal,authorization_persistence=None)


def test_real_sqlite_cas_conflict_preserves_newer_authority_across_reopen(app):
    artifact=prepare(app);context=authorize(app,artifact)
    stale_ledger=AutonomousAuthorizationLedger()
    stale=AutonomousAuthorizationPersistenceCoordinator(stale_ledger,
        TransactionalJsonAutonomousAuthorizationSnapshotPersistence(app.store))
    stale.restore()
    app.ledger.revoke(context.grant_id,revoked_at=1800);app.persistence.flush()
    with pytest.raises(ValueError,match="compare-and-set"):stale.flush()
    with SQLiteWorkflowArtifactAuthorizationStore(app.tmp_path/"authorization.sqlite3") as reopened:
        restored=AutonomousAuthorizationLedger()
        fresh=AutonomousAuthorizationPersistenceCoordinator(restored,
            TransactionalJsonAutonomousAuthorizationSnapshotPersistence(reopened))
        fresh.restore()
        assert restored.get(context.grant_id).status=="revoked"


def test_unpersisted_grant_and_unrelated_coordinator_ledger_never_authorize_publication(app):
    artifact=prepare(app)
    context=authorize(app,artifact)
    unrelated=AutonomousAuthorizationLedger()
    context.gate=AutonomousAuthorizationGate(unrelated)
    with pytest.raises(WorkflowArtifactError,match="does not bind"):
        publish(app,artifact,context)
    assert_no_artifact(app)
    context.gate=AutonomousAuthorizationGate(app.ledger)
    app.store._connection.execute("DELETE FROM workflow_artifact_authorization")
    with pytest.raises(WorkflowArtifactError,match="durably persisted"):
        publish(app,artifact,context)
    assert_no_artifact(app)


def test_completed_journal_without_actual_file_never_fabricates_artifact_completion(app):
    artifact=prepare(app);context=authorize(app,artifact)
    receipt=publish(app,artifact,context)
    (app.root/"review.json").unlink()
    observed=app.sink.reconcile(artifact)
    assert observed["status"]=="not_found" and observed["retry_safe"] is False
    assert observed["journal_status"]=="completed"
    assert app.journal.get(receipt["effect_id"]).status=="completed"


def test_execution_policy_disabling_effects_refuses_after_scope_check_before_publication(app):
    policy=AutonomousExecutionPolicy()
    artifact=prepare(app,policy=policy)
    context=authorize(app,artifact)
    execution=controller(app,policy=policy)
    with pytest.raises(ValueError,match="side effects are disabled"):
        publish(app,artifact,context,execution)
    assert_no_artifact(app)


def test_nonboolean_cancellation_and_linked_writer_lock_fail_closed(app):
    artifact=prepare(app);context=authorize(app,artifact)
    with pytest.raises(WorkflowArtifactError,match="return boolean"):
        publish(app,artifact,context,cancelled=lambda:0)
    assert_no_artifact(app)
    lock=app.root/".aurora-write.lock"
    os.link(lock,app.tmp_path/"lock-alias")
    with pytest.raises(WorkflowArtifactError,match="lock is unsafe"):
        publish(app,artifact,context)
    assert_no_artifact(app)


_CHILD = r'''
import json, os, sys
from pathlib import Path
from prism_sdk.autonomous_authorization import AutonomousAuthorizationLedger, AutonomousAuthorizationPersistenceCoordinator, TransactionalJsonAutonomousAuthorizationSnapshotPersistence, AutonomousAuthorizationContext, AutonomousAuthorizationGate
from prism_sdk.autonomous_effects import SQLiteAutonomousEffectJournal
from prism_sdk.autonomy_persistence import AutonomousExecutionController, AutonomousExecutionPolicy
from prism_sdk.workflow_artifact import SQLiteWorkflowArtifactAuthorizationStore, WorkflowArtifactSink
base=Path(sys.argv[1]);mode=sys.argv[2]
with SQLiteWorkflowArtifactAuthorizationStore(base/'authorization.sqlite3') as store, SQLiteAutonomousEffectJournal(base/'effects.sqlite3') as journal:
    ledger=AutonomousAuthorizationLedger();persistence=AutonomousAuthorizationPersistenceCoordinator(ledger,TransactionalJsonAutonomousAuthorizationSnapshotPersistence(store));persistence.restore()
    sink=WorkflowArtifactSink(base/'artifacts',journal=journal,authorization_persistence=persistence)
    policy=AutonomousExecutionPolicy(allow_side_effects=True,max_effectful_calls=1)
    artifact=sink.prepare(plan_digest='a'*64,task_digest='b'*64,catalog_digest='c'*64,step_id='review:one',resolved_arguments={'report_kind':'public_research'},filename='review.json',value={'evidence':['public'],'unknown':None,'measured':False},policy=policy)
    context=AutonomousAuthorizationContext(gate=AutonomousAuthorizationGate(ledger),grant_id='artifact-grant',tenant_id='tenant-a',actor_id='actor-a',session_id='session-a',authorization_digest=artifact.manifest_digest,domains=('operations',),capability='workflow_artifact_write',risk_class='local_artifact_write',clock=lambda:2000)
    execution=AutonomousExecutionController(execution_id='workflow.'+'a'*64,domain='operations',capability='workflow_artifact_write',risk_class='local_artifact_write',policy=policy)
    if mode=='crash':sink._after_publish=lambda:os._exit(23)
    if mode=='hold':
        def wait():
            print('published-with-lock',flush=True)
            sys.stdin.readline()
        sink._after_publish=wait
    print(json.dumps(sink.publish(artifact,authorization_context=context,execution=execution)),flush=True)
'''


def child(app,mode):
    env={**os.environ,"PYTHONDONTWRITEBYTECODE":"1"}
    return subprocess.Popen([sys.executable,"-B","-c",_CHILD,str(app.tmp_path),mode],
        stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env=env)


def test_process_death_after_publication_reconciles_real_bytes_and_survives_sqlite_reopen(app):
    artifact=prepare(app);context=authorize(app,artifact)
    process=child(app,"crash")
    output,error=process.communicate(timeout=30)
    assert process.returncode==23,(output,error)
    assert (app.root/"review.json").read_bytes()==artifact._content
    effect_id=app.sink._boundary.effect_id(app.sink._request(artifact,artifact.manifest()))
    assert app.journal.get(effect_id).status=="dispatched"
    uses_before=json.loads(app.store.read())["grants"][0]["used_count"]
    recovered=app.sink.reconcile(artifact)
    assert recovered["status"]=="completed" and recovered["journal_status"]=="reconciled"
    assert json.loads(app.store.read())["grants"][0]["used_count"]==uses_before==1
    with SQLiteAutonomousEffectJournal(app.tmp_path/"effects.sqlite3") as reopened:
        assert reopened.get(effect_id).status=="reconciled"
    with pytest.raises(WorkflowArtifactError,match="already exists"):publish(app,artifact,context)


def test_os_writer_lock_blocks_second_process_and_preserves_single_publication(app):
    artifact=prepare(app);context=authorize(app,artifact)
    first=child(app,"hold")
    try:
        assert first.stdout.readline().strip()=="published-with-lock"
        second=child(app,"normal")
        out,error=second.communicate(timeout=30)
        assert second.returncode!=0,(out,error)
        assert "writer lock" in error
        assert app.sink.inspect(artifact)["status"]=="unknown"
        assert (app.root/"review.json").read_bytes()==artifact._content
    finally:out,error=first.communicate("release\n",timeout=30)
    assert first.returncode==0,(out,error)
    assert app.sink.inspect(artifact)["status"]=="completed"
    assert app.journal.get(app.sink.inspect(artifact)["effect_id"]).dispatch_attempt==1
    with pytest.raises(WorkflowArtifactError):publish(app,artifact,context)


def test_staging_residue_and_absent_or_conflicting_uncertain_files_never_become_retry_ready(app,monkeypatch):
    artifact=prepare(app);context=authorize(app,artifact)
    def fail(*args,**kwargs):raise OSError("sensitive-transient-error")
    monkeypatch.setattr(os,"link",fail)
    with pytest.raises(ValueError):publish(app,artifact,context)
    observed=app.sink.reconcile(artifact)
    assert observed["status"]=="not_found" and observed["retry_safe"] is False
    assert observed["journal_status"]=="uncertain"
    app.sink._stage(artifact).write_bytes(b"partial")
    observed=app.sink.reconcile(artifact)
    assert observed["status"]=="unknown" and observed["retry_safe"] is False
    (app.root/"review.json").write_bytes(b"conflicting")
    assert app.sink.reconcile(artifact)["status"]=="unknown"
    assert "sensitive-transient-error" not in canonical_json(app.journal.snapshot().to_dict())
