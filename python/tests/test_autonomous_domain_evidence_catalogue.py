from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousEvidencePlan,
    AutonomousEvidenceRequirement,
    AutonomousDomainEvidenceSourceCatalogue,
    AutonomousDomainHttpSourcePreset,
    AutonomousHttpConnectorPolicy,
    AutonomousHttpConnectorRequest,
    ArgumentError,
    builtin_autonomous_domain_evidence_source_profiles,
    builtin_autonomous_domain_http_source_presets,
    content_digest,
    create_builtin_autonomous_domain_evidence_source_catalogue,
    create_autonomous_domain_http_source_acquirer,
    register_autonomous_domain_http_source_matrix,
)


class _StaticAcquirer:
    def __init__(self, value: object) -> None:
        self.value = value
        self.calls = 0

    def acquire(self, _context: object) -> object:
        self.calls += 1
        return self.value


def _plan(domain: str, capability: str) -> AutonomousEvidencePlan:
    workflow_digest = content_digest({"workflow": domain, "version": 1})
    requirement = AutonomousEvidenceRequirement(
        requirement_id=f"{domain}:evidence:answer",
        domain=domain,
        workflow_id=f"{domain}:evidence",
        workflow_digest=workflow_digest,
        stage_id="answer",
        label="answer",
        objective=f"Collect reviewed evidence for {domain}.",
        required_capabilities=(capability,),
        evaluator_signals=("agreement",),
    )
    return AutonomousEvidencePlan(
        domains=(domain,), workflow_ids=(requirement.workflow_id,), workflow_digests=(workflow_digest,),
        requirements=(requirement,), missing_requirement_ids=(requirement.requirement_id,),
    )


def test_builtin_catalogue_profiles_cover_every_domain_and_are_metadata_only() -> None:
    profiles = builtin_autonomous_domain_evidence_source_profiles()
    assert {profile.domain for profile in profiles} == set(AUTONOMOUS_DOMAINS)
    assert len({profile.profile_digest for profile in profiles}) == len(profiles)
    catalogue = create_builtin_autonomous_domain_evidence_source_catalogue()
    wire = catalogue.to_dict()
    assert wire["covered_domain_count"] == 0
    assert all(row["state"] == "missing" for row in wire["coverage"])
    assert "must_never_enter" not in json.dumps(wire).lower()


def test_matrix_registers_one_reviewed_http_route_for_every_domain_without_dispatch() -> None:
    catalogue = create_builtin_autonomous_domain_evidence_source_catalogue()
    presets = {preset.domain: preset for preset in builtin_autonomous_domain_http_source_presets()}
    acquirers = {domain: _StaticAcquirer({"domain": domain, "answer": "ok"}) for domain in AUTONOMOUS_DOMAINS}
    matrix = register_autonomous_domain_http_source_matrix(
        catalogue=catalogue,
        entries=[
            {"preset": presets[domain].preset_id, "source_id": f"http-{domain}", "acquirer": acquirers[domain]}
            for domain in AUTONOMOUS_DOMAINS
        ],
    )
    assert matrix["preset_count"] == len(AUTONOMOUS_DOMAINS)
    assert {row["state"] for row in matrix["coverage"]} == {"partial"}
    assert all(acquirer.calls == 0 for acquirer in acquirers.values())
    assert all(route["contract_digest"] is None for route in catalogue.routes())
    assert all(route["secret_material"] == "never_returned" for route in catalogue.routes())


def test_catalogue_prepares_and_executes_a_profile_bound_reconciliation_only_after_approval() -> None:
    catalogue = create_builtin_autonomous_domain_evidence_source_catalogue()
    acquirer = _StaticAcquirer({"answer": "stable"})
    profile = next(profile for profile in builtin_autonomous_domain_evidence_source_profiles() if profile.domain == "coding")
    catalogue.register_route(
        source_id="coding-http", profile_id=profile.profile_id, provider="caller-http-coding", acquirer=acquirer,
        capabilities=("review",), source_kinds=("repository",), operations=("repository_snapshot",),
        metadata={"operation": "repository_snapshot"},
    )
    plan = _plan("coding", "review")
    requirement_id = plan.requirements[0].requirement_id
    prepared = catalogue.prepare(plan, requirement_id)
    assert prepared.profile["profile_digest"] == profile.profile_digest
    assert acquirer.calls == 0
    with pytest.raises(ArgumentError):
        catalogue.execute(plan, prepared)
    result = catalogue.execute(plan, prepared, approve_source_dispatch=True, normalizer=lambda value, _context: value)
    assert result.status == "consensus"
    assert acquirer.calls == 1


def test_catalogue_rejects_secrets_and_fails_closed_on_route_or_profile_drift() -> None:
    catalogue = create_builtin_autonomous_domain_evidence_source_catalogue()
    profile = next(profile for profile in builtin_autonomous_domain_evidence_source_profiles() if profile.domain == "browser")
    with pytest.raises(ArgumentError):
        catalogue.register_route(
            source_id="unsafe", profile_id=profile.profile_id, provider="caller", acquirer=_StaticAcquirer({}),
            metadata={"operation": "search", "api_key": "must-never-enter-metadata"},
        )
    route = catalogue.register_route(
        source_id="browser-a", profile_id=profile.profile_id, provider="caller", acquirer=_StaticAcquirer({"answer": 1}),
        capabilities=("web_research",), source_kinds=("web_search",), operations=("search",),
        metadata={"operation": "search"},
    )
    plan = _plan("browser", "web_research")
    prepared = catalogue.prepare(plan, plan.requirements[0].requirement_id)
    with pytest.raises(ArgumentError):
        catalogue.register_profile(
            type(profile)(
                profile_id=profile.profile_id, version="2", domain=profile.domain, purpose=profile.purpose,
                source_kinds=profile.source_kinds, capabilities=profile.capabilities, operations=profile.operations,
                required_metadata=profile.required_metadata, freshness=profile.freshness, auth_mode=profile.auth_mode,
                pagination=profile.pagination, normalizer_id=profile.normalizer_id, normalizer_version="2",
                default_quorum=profile.default_quorum, default_max_concurrency=profile.default_max_concurrency,
                limitations=profile.limitations,
            ), replace=True,
        )
    catalogue.unregister_route(route["source_id"])
    assert catalogue.routes() == ()
    with pytest.raises(ArgumentError):
        catalogue.execute(plan, prepared, approve_source_dispatch=True, normalizer=lambda value, _context: value)


def test_profile_and_preset_wire_round_trips_reject_tampering() -> None:
    profile = builtin_autonomous_domain_evidence_source_profiles()[0]
    preset = builtin_autonomous_domain_http_source_presets()[0]
    assert type(profile).from_dict(profile.to_dict()).to_dict() == profile.to_dict()
    assert AutonomousDomainHttpSourcePreset.from_dict(preset.to_dict()).to_dict() == preset.to_dict()
    tampered = dict(preset.to_dict())
    tampered["default_provider"] = "attacker"
    with pytest.raises(ArgumentError):
        AutonomousDomainHttpSourcePreset.from_dict(tampered)


def test_http_preset_acquirer_keeps_transport_and_headers_transient() -> None:
    calls: list[str] = []

    class _Response:
        status = 200
        headers = {"Content-Type": "application/json"}

        def __init__(self) -> None:
            self.offset = 0

        def __enter__(self) -> "_Response":
            return self

        def __exit__(self, *_args: object) -> None:
            return None

        def getcode(self) -> int:
            return self.status

        def read(self, _size: int = -1) -> bytes:
            payload = b'{"claim":"observed"}'
            if self.offset:
                return b""
            self.offset = len(payload)
            return payload

    acquirer = create_autonomous_domain_http_source_acquirer(
        lambda _manifest, _request: AutonomousHttpConnectorRequest("GET", "https://example.test/evidence"),
        policy=AutonomousHttpConnectorPolicy(allowed_hosts=("example.test",)),
        header_resolver=lambda _manifest, _request: {"Authorization": "Bearer transient-only"},
        opener=lambda request, _timeout: (calls.append(request.full_url), _Response())[1],
    )
    assert calls == []
    value = acquirer.acquire({"request": {"metadata": {"operation": "search"}}})
    assert value == {"claim": "observed"}
    assert calls == ["https://example.test/evidence"]
    assert "transient-only" not in repr(value)
