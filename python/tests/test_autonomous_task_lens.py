from __future__ import annotations

from dataclasses import replace

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AUTONOMOUS_TASK_LENS_DOMAINS,
    AutonomousDomainTaskLens,
    autonomous_domain_task_lens,
    builtin_autonomous_domain_task_lenses,
    content_digest,
)
from prism_sdk.errors import ArgumentError


def test_builtin_task_lenses_cover_all_domains_and_are_canonical() -> None:
    lenses = builtin_autonomous_domain_task_lenses()
    assert tuple(lens.domain for lens in lenses) == tuple(AUTONOMOUS_DOMAINS)
    assert tuple(lens.domain for lens in lenses) == tuple(AUTONOMOUS_TASK_LENS_DOMAINS)
    assert len({lens.lens_id for lens in lenses}) == len(lenses) == 12
    assert len({lens.lens_digest for lens in lenses}) == len(lenses)

    coding = autonomous_domain_task_lens("coding")
    evaluation = autonomous_domain_task_lens("evaluation")
    assert coding.lens_digest == "616bf58c2e6dcfb4bb926477b692c9d28fa0a3737ce17279852a662bdee68a51"
    assert evaluation.lens_digest == "065a919cc799ca3d2acbe95b5b98502b230c45f42cd5e79464db4d4725eb2136"
    assert coding.lens_digest == content_digest({key: value for key, value in coding.to_dict().items() if key != "lens_digest"})

    for lens in lenses:
        public = lens.to_dict()
        contract = lens.prompt_contract()
        assert public["schema"] == "bioprism-autonomous-domain-task-lens/0.1"
        assert public["lens_digest"] == lens.lens_digest
        assert contract["lens_digest"] == lens.lens_digest
        assert contract["model_hints_are"].startswith("preferences_only")
        assert contract["execution"].startswith("guidance_only")
        assert contract["secret_material"] == "never_returned"
        assert all(isinstance(value, str) and value for value in lens.planning_dimensions)
        assert all(isinstance(value, str) and value for value in lens.decision_checks)
        assert all(isinstance(value, str) and value for value in lens.evidence_priorities)
        assert all(isinstance(value, str) and value for value in lens.evaluator_signals)
        assert all(isinstance(value, str) and value for value in lens.model_capability_hints)


def test_task_lens_validation_rejects_unknown_domains_and_duplicate_guidance() -> None:
    with pytest.raises(ArgumentError):
        autonomous_domain_task_lens("unknown")

    coding = autonomous_domain_task_lens("coding")
    with pytest.raises(ArgumentError):
        replace(coding, planning_dimensions=("scope", "scope"))

    assert set(AUTONOMOUS_TASK_LENS_DOMAINS) == set(AUTONOMOUS_DOMAINS)
