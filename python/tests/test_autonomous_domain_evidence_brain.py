from __future__ import annotations

import json

import pytest

from test_autonomy import _Workspace, _model, _runtime

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    ModelCatalogue,
    builtin_autonomous_domain_evidence_source_profiles,
    create_builtin_autonomous_domain_evidence_source_catalogue,
    ArgumentError,
)


class _RouteAcquirer:
    def __init__(self, value: object, calls: list[str]) -> None:
        self.value = value
        self.calls = calls

    def acquire(self, context: object) -> object:
        self.calls.append(context["request"]["source_id"])  # type: ignore[index]
        return self.value


def _catalogue(calls: list[str]):
    catalogue = create_builtin_autonomous_domain_evidence_source_catalogue()
    for profile in builtin_autonomous_domain_evidence_source_profiles():
        for suffix in ("a", "b"):
            catalogue.register_route(
                source_id=f"brain-{profile.domain}-{suffix}",
                profile_id=profile.profile_id,
                provider=f"fixture-{profile.domain}-{suffix}",
                request_id=f"request-{profile.domain}-{suffix}",
                metadata={"operation": profile.operations[0]},
                acquirer=_RouteAcquirer(
                    {"claim": f"stable-{profile.domain}", "raw_marker": f"raw-{profile.domain}"},
                    calls,
                ),
            )
    return catalogue


def _profile_id(domain: str) -> str:
    return next(profile.profile_id for profile in builtin_autonomous_domain_evidence_source_profiles() if profile.domain == domain)


def test_catalogue_brain_composes_every_domain_and_keeps_raw_values_transient():
    runtime, store, server, thread = _runtime()
    calls: list[str] = []
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "domain-catalogue-brain")
    catalogue = _catalogue(calls)
    try:
        for domain in AUTONOMOUS_DOMAINS:
            result = agent.run_with_domain_evidence_catalogue(
                task=f"review a bounded {domain} task with catalogue evidence",
                catalogue=catalogue,
                domains=(domain,),
                credentials={"openai": handle},
                model_candidates=_model(),
                prepare_options={"profile_id": _profile_id(domain), "quorum": 2, "max_concurrency": 2},
                approve_source_dispatch=True,
                approve_provider_call=True,
                prompt_builder=lambda projection, selected_domain=domain: {
                    "reviewed_catalogue": {
                        "domain": selected_domain,
                        "raw_marker": next(iter(next(iter(projection.values.values())).values()))["raw_marker"],
                    }
                },
                run_mode="domain",
            )
            assert result.status == "completed", domain
            assert result.execution_status == "completed_provider_call", domain
            assert result.prepared, domain
            assert all(item.result is not None and item.result.status == "consensus" for item in result.prepared), domain
            assert result.prompt_context["reviewed_catalogue"]["raw_marker"] == f"raw-{domain}"  # type: ignore[index]
            assert f"raw-{domain}" not in json.dumps(result.to_dict())
            assert "raw_marker" not in json.dumps(result.to_dict())
        expected_calls = sum(
            2 * len(agent.evidence_plan((domain,)).requirements)
            for domain in AUTONOMOUS_DOMAINS
        )
        assert len(calls) == expected_calls
        assert getattr(server, "request_count", 0) == len(AUTONOMOUS_DOMAINS)
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_catalogue_brain_keeps_source_and_provider_approval_separate():
    runtime, store, server, thread = _runtime()
    calls: list[str] = []
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "domain-catalogue-approval")
    catalogue = _catalogue(calls)
    try:
        review = agent.run_with_domain_evidence_catalogue(
            task="review a coding catalogue task",
            catalogue=catalogue,
            domains=("coding",),
            credentials={"openai": handle},
            model_candidates=_model(),
            prepare_options={"profile_id": "builtin.coding.evidence", "quorum": 2},
            approve_source_dispatch=False,
            approve_provider_call=True,
            run_mode="domain",
        )
        assert review.status == "evidence_review_required"
        assert calls == []
        assert getattr(server, "request_count", 0) == 0

        provider_review = agent.run_with_domain_evidence_catalogue(
            task="review a coding catalogue task after source approval",
            catalogue=catalogue,
            domains=("coding",),
            credentials={"openai": handle},
            model_candidates=_model(),
            prepare_options={"profile_id": "builtin.coding.evidence", "quorum": 2},
            approve_source_dispatch=True,
            approve_provider_call=False,
            run_mode="domain",
        )
        assert provider_review.status == "provider_review_required"
        assert provider_review.execution_status == "approval_required"
        assert len(calls) == 2 * len(agent.evidence_plan(("coding",)).requirements)
        assert getattr(server, "request_count", 0) == 0
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_catalogue_brain_routes_cross_domain_and_auto_modes_through_existing_learning_boundary():
    runtime, store, server, thread = _runtime()
    calls: list[str] = []
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "domain-catalogue-routing")
    catalogue = _catalogue(calls)
    try:
        cross_domain = agent.run_with_domain_evidence_catalogue(
            task="coordinate a bounded coding and data review",
            catalogue=catalogue,
            domains=("coding", "data"),
            credentials={"openai": handle},
            model_candidates=_model(),
            prepare_options={"quorum": 1, "max_concurrency": 1},
            approve_source_dispatch=True,
            approve_provider_call=True,
            run_mode="cross_domain",
        )
        assert cross_domain.status == "completed"
        assert cross_domain.execution_status == "completed"
        assert cross_domain.execution is not None
        assert getattr(cross_domain.execution, "status", None) == "completed"

        automatic = agent.run_with_domain_evidence_catalogue(
            task="inspect a bounded repository implementation and test result",
            catalogue=catalogue,
            domains=("coding",),
            credentials={"openai": handle},
            model_candidates=_model(),
            prepare_options={"quorum": 1, "max_concurrency": 1},
            approve_source_dispatch=True,
            approve_provider_call=True,
            run_options={"learning_mode": "off"},
            run_mode="auto",
        )
        assert automatic.status == "completed"
        assert automatic.execution_status is not None
        assert automatic.execution is not None
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_catalogue_brain_blocks_unresolved_dissent_and_rejects_catalogue_drift():
    runtime, store, server, thread = _runtime()
    calls: list[str] = []
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "domain-catalogue-dissent")
    catalogue = _catalogue(calls)
    profile = next(profile for profile in builtin_autonomous_domain_evidence_source_profiles() if profile.domain == "coding")
    catalogue.register_route(
        source_id="brain-coding-dissent",
        profile_id=profile.profile_id,
        provider="fixture-coding-dissent",
        request_id="request-coding-dissent",
        metadata={"operation": profile.operations[0]},
        acquirer=_RouteAcquirer({"claim": "conflicting", "raw_marker": "dissent"}, calls),
    )
    try:
        result = agent.run_with_domain_evidence_catalogue(
            task="review coding evidence with dissent",
            catalogue=catalogue,
            domains=("coding",),
            credentials={"openai": handle},
            model_candidates=_model(),
            prepare_options={
                "profile_id": profile.profile_id,
                "source_ids": ["brain-coding-a", "brain-coding-b", "brain-coding-dissent"],
                "quorum": 3,
                "max_concurrency": 3,
            },
            approve_source_dispatch=True,
            approve_provider_call=True,
            run_mode="domain",
        )
        assert result.status == "evidence_incomplete"
        assert all(item.result is not None and item.result.status == "disagreement" for item in result.prepared)
        assert getattr(server, "request_count", 0) == 0
        mutated = False

        def mutate_catalogue(_requirement: object) -> dict[str, object]:
            nonlocal mutated
            if not mutated:
                mutated = True
                catalogue.register_route(
                    source_id="brain-coding-drift",
                    profile_id=profile.profile_id,
                    provider="fixture-coding-drift",
                    request_id="request-coding-drift",
                    metadata={"operation": profile.operations[0]},
                    acquirer=_RouteAcquirer({"claim": "drift", "raw_marker": "drift"}, calls),
                )
            return {}

        with pytest.raises(ArgumentError, match="catalogue changed after preparation"):
            # The caller-owned preparation hook demonstrates that route mutation between the
            # initial identity capture and source dispatch is rejected before any acquirer runs.
            agent.run_with_domain_evidence_catalogue(
                task="review coding evidence with catalogue mutation",
                catalogue=catalogue,
                domains=("coding",),
                credentials={"openai": handle},
                model_candidates=_model(),
                prepare_options={"profile_id": profile.profile_id, "quorum": 2},
                prepare_for_requirement=mutate_catalogue,
                approve_source_dispatch=True,
                approve_provider_call=True,
                run_mode="domain",
            )
        assert len(calls) == 3 * len(agent.evidence_plan(("coding",)).requirements)
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()
