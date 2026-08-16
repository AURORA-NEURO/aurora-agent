from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    ObservedWorldDeclareArgs,
    ObservedWorldDeclareReport,
    WorldClaimCheckReport,
    observed_world_declare_report,
    world_claim_check_report,
)


def provenance(*, observed: bool = True) -> dict:
    if observed:
        return {
            "top": "observed",
            "stands_on": ["observed"],
            "assumptions": [],
            "unsupported_counterfactuals": [],
            "selection": {"selection": "consecutive", "criterion": "all eligible participants"},
        }
    return {
        "top": "mechanistic",
        "stands_on": ["mechanistic"],
        "assumptions": ["tumour growth rate"],
        "unsupported_counterfactuals": [],
        "selection": {"selection": "undeclared"},
    }


def claim(kind: str = "biology", quantity: str = "observed outcome") -> dict:
    return {"kind": kind, "quantity": quantity, "counterfactual": None, "population": None}


def observed_world_payload() -> dict:
    design = {
        "cohort_size": 2,
        "strata": [{"name": "all", "count": 2}],
        "selection": {"selection": "consecutive", "criterion": "all eligible participants"},
        "stands_for_population": "RG-DEMO population",
        "unsupported_counterfactuals": ["treatment effect"],
    }
    world = {
        "id": "observed-demo",
        "sources": [{"name": "cohort", "version": "v1", "access": {"access": "controlled", "policy": "reviewer-only"}, "embedded": False}],
        "design": design,
        "outcome_labels": ["negative", "positive"],
    }
    return {
        "ok": True,
        "world": world,
        "provenance": {
            "top": "observed",
            "stands_on": ["observed"],
            "assumptions": [],
            "unsupported_counterfactuals": ["treatment effect"],
            "selection": {"selection": "consecutive", "criterion": "all eligible participants"},
        },
        "world_id": "observed-demo",
        "source_count": 1,
        "controlled_sources": ["cohort"],
        "outcome_label_count": 2,
        "guarantees": ["pinned observed-world sources are retained"],
    }


class WorldTests(unittest.TestCase):
    def test_supported_claim_preserves_rung_selection_and_caveat(self) -> None:
        payload = {
            "ok": True,
            "supported": True,
            "claim": claim(),
            "grounded": {
                "claim": claim(),
                "stands_on": ["observed"],
                "furthest_from_observation": "observed",
            },
            "caveat": "This is a biological claim from an observed world; it is a statement about the cohort.",
            "provenance": provenance(),
        }
        report = world_claim_check_report({"ok": True, "mcp": {"result": {"structuredContent": payload}}})
        self.assertIsInstance(report, WorldClaimCheckReport)
        self.assertTrue(report.supported)
        self.assertTrue(report.provenance.observed_only)
        self.assertEqual(report.grounded.furthest_from_observation, "observed")
        self.assertIsNotNone(report.caveat)

    def test_refusal_is_typed_and_fail_closed(self) -> None:
        payload = {
            "ok": False,
            "supported": False,
            "claim": claim(quantity="tumour growth rate"),
            "refusal": "claim is circular: tumour growth rate was fixed by a mechanistic construction",
            "provenance": provenance(observed=False),
            "fail_closed": True,
        }
        report = world_claim_check_report(payload)
        self.assertTrue(report.refused)
        self.assertEqual(report.refusal.split(":", 1)[0], "claim is circular")
        self.assertTrue(report.fail_closed)
        self.assertIsNone(report.grounded)

    def test_report_rejects_parity_or_provenance_forgery(self) -> None:
        payload = {
            "ok": True,
            "supported": True,
            "claim": claim(),
            "grounded": {"claim": claim(), "stands_on": ["observed"], "furthest_from_observation": "observed"},
            "caveat": "caveat",
            "provenance": provenance(),
        }
        payload["grounded"]["claim"]["quantity"] = "different"
        with self.assertRaises(ArgumentError):
            world_claim_check_report(payload)
        payload = {
            "ok": False,
            "supported": True,
            "claim": claim(),
            "refusal": "refused",
            "provenance": provenance(),
            "fail_closed": True,
        }
        with self.assertRaises(ArgumentError):
            world_claim_check_report(payload)

    def test_observed_declaration_preserves_controlled_sources_and_provenance(self) -> None:
        payload = observed_world_payload()
        report = observed_world_declare_report({"ok": True, "mcp": {"result": {"structuredContent": payload}}})
        self.assertIsInstance(report, ObservedWorldDeclareReport)
        self.assertEqual(report.world_id, "observed-demo")
        self.assertEqual(report.controlled_sources, ("cohort",))
        self.assertEqual(report.world.design.cohort_size, 2)
        self.assertTrue(report.world.sources[0].controlled)
        self.assertTrue(report.provenance.observed_only)

    def test_observed_declaration_request_and_report_reconcile_fail_closed(self) -> None:
        payload = observed_world_payload()
        request = ObservedWorldDeclareArgs(
            "observed-demo",
            payload["world"]["sources"],
            payload["world"]["design"],
            payload["world"]["outcome_labels"],
        )
        self.assertEqual(request.to_mcp_arguments()["id"], "observed-demo")
        with self.assertRaises(ArgumentError):
            ObservedWorldDeclareArgs("observed-demo", payload["world"]["sources"] * 2, payload["world"]["design"], ["positive"])
        payload["controlled_sources"] = []
        with self.assertRaises(ArgumentError):
            observed_world_declare_report(payload)


if __name__ == "__main__":
    unittest.main()
