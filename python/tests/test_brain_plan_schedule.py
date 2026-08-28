from __future__ import annotations

import pytest

from prism_sdk import BrainPlanSchedule, BrainRunError, validate_brain_plan_schedule


def _plan() -> dict[str, object]:
    return {
        "schema": "bioprism-brain-plan/0.1",
        "objective": "fan out then synthesize",
        "ordered_step_ids": ["alpha", "beta", "gamma", "synthesize"],
        "steps": [
            {"id": "alpha", "depends_on": [], "estimated_cost": 2},
            {"id": "beta", "depends_on": [], "estimated_cost": 4},
            {"id": "gamma", "depends_on": [], "estimated_cost": 3},
            {"id": "synthesize", "depends_on": ["alpha", "beta", "gamma"], "estimated_cost": 5},
        ],
        "estimated_cost": 14,
        "execution_waves": [["alpha", "beta"], ["gamma"], ["synthesize"]],
        "critical_path_cost": 9,
        "max_parallelism": 2,
        "estimated_parallel_rounds": 3,
        "peak_parallelism": 2,
        "requires_approval": True,
        "execution": "not_started",
        "plan_digest": "a" * 64,
    }


def test_brain_plan_schedule_validates_dag_waves_and_costs() -> None:
    schedule = validate_brain_plan_schedule(_plan())

    assert isinstance(schedule, BrainPlanSchedule)
    assert schedule.execution_waves == (("alpha", "beta"), ("gamma",), ("synthesize",))
    assert schedule.critical_path_cost == 9
    assert schedule.to_dict()["execution_waves"] == [["alpha", "beta"], ["gamma"], ["synthesize"]]


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("peak_parallelism", 3),
        ("estimated_parallel_rounds", 2),
        ("critical_path_cost", 8),
        ("execution_waves", [["alpha", "synthesize"], ["beta"], ["gamma"]]),
    ],
)
def test_brain_plan_schedule_rejects_tampered_metadata(field: str, value: object) -> None:
    plan = _plan()
    plan[field] = value

    with pytest.raises(BrainRunError):
        validate_brain_plan_schedule(plan)
