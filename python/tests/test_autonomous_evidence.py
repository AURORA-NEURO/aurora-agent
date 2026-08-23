from __future__ import annotations

from prism_sdk import (
    AutonomousEvidencePlan,
    AutonomousPromptBuilder,
    AutonomousTaskSpec,
    build_autonomous_evidence_plan,
    builtin_autonomous_domain_profiles,
    builtin_autonomous_workflow_strategies,
)


def test_evidence_plan_covers_every_builtin_domain_and_preserves_dependency_order() -> None:
    workflows = builtin_autonomous_workflow_strategies()
    plan = build_autonomous_evidence_plan(workflows)

    assert len(plan.domains) == 12
    assert len(plan.requirements) > 40
    assert plan.coverage_status == "not_evaluated"
    assert len(plan.missing_requirement_ids) == len(plan.requirements)
    assert len(plan.next_stage_ids) == 12
    assert len(plan.plan_digest) == 64
    wire = plan.to_dict()
    assert wire["execution"] == "planning_only;no_source_or_provider_dispatch"
    assert wire["secret_material"] == "never_returned"
    assert "evidence was acquired" in wire["does_not_claim"]


def test_evidence_plan_accepts_digest_bound_observations_and_rejects_ambiguous_short_labels() -> None:
    workflows = builtin_autonomous_workflow_strategies()
    full_ids = [item.requirement_id for item in build_autonomous_evidence_plan(workflows).requirements]
    complete = build_autonomous_evidence_plan(workflows, available_evidence=full_ids)
    assert complete.coverage_status == "complete"
    assert complete.coverage_ratio == 1.0
    assert not complete.missing_requirement_ids

    # ``observations`` is intentionally shared by several domain workflows, so a short label
    # cannot silently satisfy more than one reviewed requirement.
    ambiguous = build_autonomous_evidence_plan(workflows, available_evidence=("observations",))
    assert ambiguous.coverage_status == "missing"
    assert not ambiguous.covered_requirement_ids


def test_prompt_builder_includes_the_evidence_contract_without_retaining_task_text() -> None:
    profile = builtin_autonomous_domain_profiles()[0]
    spec = AutonomousTaskSpec(
        task="Inspect the repository and report verified changes.",
        domain=profile.domain,
        capability=profile.default_capability,
        risk_class=profile.risk_class,
    )
    prompt = AutonomousPromptBuilder.build(spec, profile, workflow=builtin_autonomous_workflow_strategies()[0])
    evidence_chunks = [chunk for chunk in prompt["context"] if chunk.get("id") == "autonomy-evidence-plan"]
    assert len(evidence_chunks) == 1
    assert "planning_only;no_source_or_provider_dispatch" in evidence_chunks[0]["content"]
    assert spec.task not in prompt["context"][0]["content"]
