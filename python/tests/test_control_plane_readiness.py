import unittest

from prism_sdk import (
    ArgumentError,
    ControlPlaneReadinessCompareReport,
    ControlPlaneReadinessCompareRequest,
    ControlPlaneReadinessReport,
    ControlPlaneReadinessRequest,
    ControlPlaneReadinessQueryReport,
    ControlPlaneReadinessQueryRequest,
)


def projection_payload():
    return {
        "ok": True,
        "schema": "bioprism-control-plane-readiness/0.1",
        "workflow": "control_plane_readiness_audit",
        "readiness_claimed": False,
        "execution": "not_started",
        "audit": {
            "subject_id": "subject-control-plane",
            "control_plane_state": "ready_for_human_review",
            "policy_satisfied": True,
            "components": {"domain_decision_readiness": {"satisfied": True}},
            "component_states": {"domain_decision_readiness": {"state": "ready_for_human_review"}},
            "component_count": 5,
            "blockers": [],
            "digest": "a" * 64,
        },
        "artifact_registry": {"indexed": True, "content_digest": "b" * 64},
    }


class ControlPlaneReadinessTests(unittest.TestCase):
    def test_request_preserves_component_policy_and_explicit_packets(self):
        request = ControlPlaneReadinessRequest(
            "subject-control-plane",
            policy={"require_route_review": True, "require_release_ready": True},
            readiness_audit={"workflow": "domain_decision_readiness_audit"},
            route_review={"workflow": "capability_route_review"},
        )
        arguments = request.to_arguments()
        self.assertTrue(arguments["policy"]["require_route_review"])
        self.assertEqual(arguments["route_review"]["workflow"], "capability_route_review")
        with self.assertRaises(ArgumentError):
            ControlPlaneReadinessRequest("subject-control-plane", policy={"require_route_plan": "yes"})

    def test_report_preserves_structural_state_and_non_authority(self):
        report = ControlPlaneReadinessReport.from_wire(projection_payload())
        self.assertTrue(report.ready_for_human_review)
        self.assertEqual(report.digest, "a" * 64)
        self.assertEqual(report.artifact_registry["indexed"], True)
        blocked = projection_payload()
        blocked["audit"]["control_plane_state"] = "blocked"
        blocked["audit"]["policy_satisfied"] = False
        self.assertFalse(ControlPlaneReadinessReport.from_wire(blocked).policy_satisfied)

    def test_query_request_and_report_keep_cursor_and_state_filters(self):
        request = ControlPlaneReadinessQueryRequest(
            subject_id="subject-control-plane",
            control_plane_state="ready_for_human_review",
            policy_satisfied=True,
            max_items=4,
            include_audits=True,
        )
        self.assertEqual(request.to_arguments()["control_plane_state"], "ready_for_human_review")
        report = ControlPlaneReadinessQueryReport.from_wire(
            {
                "workflow": "artifact_registry_control_plane_readiness_query",
                "rows": [{"content_digest": "b" * 64}],
                "next_after": None,
                "has_more": False,
                "registry_generation": 2,
                "registry_size": 1,
            }
        )
        self.assertEqual(report.rows[0]["content_digest"], "b" * 64)
        with self.assertRaises(ArgumentError):
            ControlPlaneReadinessQueryRequest(control_plane_state="not-a-state")

    def test_compare_request_and_report_preserve_directional_structural_diff(self):
        request = ControlPlaneReadinessCompareRequest(
            before=projection_payload(),
            after=projection_payload(),
            subject_id="subject-control-plane",
        )
        self.assertEqual(request.to_arguments()["subject_id"], "subject-control-plane")
        report = ControlPlaneReadinessCompareReport.from_wire(
            {
                "ok": True,
                "schema": "bioprism-control-plane-readiness-compare/0.1",
                "workflow": "control_plane_readiness_compare",
                "comparison": {
                    "subject_id": "subject-control-plane",
                    "state_direction": "improved",
                    "evidence_direction": "mixed",
                    "component_changes": [],
                    "blockers_added": [],
                    "blockers_removed": [],
                    "improvements": [{"kind": "blockers_removed"}],
                    "regressions": [{"kind": "policy_changed"}],
                    "comparison_digest": "c" * 64,
                },
                "readiness_claimed": False,
                "execution": "not_started",
            }
        )
        self.assertEqual(report.evidence_direction, "mixed")
        self.assertEqual(report.comparison_digest, "c" * 64)
        with self.assertRaises(ArgumentError):
            ControlPlaneReadinessCompareRequest(before={}, after={}, subject_id="")


if __name__ == "__main__":
    unittest.main()
