from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    OperationsCatalogArgs,
    OperationsCatalogReport,
    OpsAcceptanceArgs,
    OpsAcceptanceReport,
    operations_catalog_report,
    ops_acceptance_report,
)


def _topology(deployment: str, technology: str) -> dict:
    classes = []
    for class_id in ("metadata", "artifact", "event", "analytics", "search"):
        immutable = class_id in {"artifact", "event"}
        durable = "Canonical" if immutable or class_id == "metadata" else "Rebuildable"
        mutability = "immutable" if class_id == "artifact" else "append_only" if class_id == "event" else "mutable"
        classes.append({
            "class": class_id,
            "name": class_id,
            "store": {
                "name": f"{class_id}-store",
                "technology": technology,
                "durability": durable,
                "mutability": mutability,
                "rebuilt_from": [] if durable == "Canonical" else ["metadata"],
            },
            "promises": {"durability": durable, "mutability": mutability},
            "holds_immutable_evidence": immutable,
        })
    return {"deployment": deployment, "technologies": [technology], "classes": classes}


def operations_payload(*, full: bool = True) -> dict:
    payload = {
        "ok": True,
        "detail_mode": "full" if full else "summary",
        "max_items": 2,
        "topologies": {
            "local": _topology("local", "sqlite"),
            "team": _topology("team", "postgresql"),
            "promise_parity": {"compared": 5, "holds": True, "differences": []},
            "technology_is_not_promise_parity": True,
        },
        "data_classes": [{"class": name, "name": name, "holds_immutable_evidence": name in {"artifact", "event"}} for name in ("metadata", "artifact", "event", "analytics", "search")],
        "deployment_planes": [{"plane": name, "name": name, "control_plane": name != "execution_pool"} for name in ("control_api", "catalog", "artifact_storage", "scheduler", "execution_pool", "analytics", "search", "signing", "observability")],
        "tenant_patterns": [{"pattern": name, "name": name} for name in ("shared_control", "dedicated_installation", "air_gapped_registry", "hybrid_public_metadata")],
        "slo_objectives": ["api-read-availability", "operation-acceptance"],
        "service_contracts": {
            "summary": {"satisfied": 0, "diverges": 9, "not_implemented": 0, "divergences": 4, "total": 9},
            "entries": [
                {"module_id": "40.03", "title": "Service Graph", "contract": "service-graph", "crates": [], "verdict": "diverges", "divergence_count": 2, "divergences": ["no hosted service"], "omitted_divergences": 1},
                {"module_id": "40.04", "title": "Boundaries", "contract": "domain-boundaries", "crates": ["bioprism-services"], "verdict": "diverges", "divergence_count": 2, "divergences": ["orphaned concern", "undeclared crossing"], "omitted_divergences": 0},
            ],
            "entry_count": 9,
            "omitted_entries": 7,
        },
        "metrics": {
            "metrics_schema_version": "bioprism-metrics/0.1",
            "atlasx_schema_version": "bioprism-atlasx/0.1",
            "named_in_scope": 4,
            "named_but_undefined": 3,
            "defined_here": [{"metric": "profile coverage", "blueprint_name": True, "numerator": "measured", "denominator": "grid", "refuses": "empty grid"}],
            "undefined_metrics_returned": [{"origin": "capability_metrics", "module_title": "Grounding", "metric": "precision", "denominator": None}, {"origin": "capability_metrics", "module_title": "Grounding", "metric": "completion", "denominator": None}],
            "omitted_undefined_metrics": 1,
            "undefined_is_not_zero": True,
        },
        "sdk": {"registration_note": "registration is not execution", "execution_and_isolation_are_not_implied": True},
        "limitations": ["no live deployment"],
    }
    if full:
        payload["details"] = {"service_entries": [{"module_id": "40.03"}], "undefined_metrics": [{"metric": "precision"}]}
    return payload


def acceptance_payload() -> dict:
    return {
        "ok": True,
        "summary": {"met": 0, "refuted": 1, "unverifiable": 2, "total": 3, "is_release_ready": False, "is_decidable": False},
        "findings": [
            {"criterion": "signed_bundle", "verdict": "refuted", "basis": {"basis": "linked_type", "krate": "bioprism-safety", "item": "SignatureStatus"}, "detail": "signature is not checked"},
            {"criterion": "clean_demo", "verdict": "unverifiable", "basis": {"basis": "no_observer", "because": "no shell"}, "detail": "members are present"},
        ],
        "omitted_findings": 1,
        "guarantees": ["unverifiable is not a pass"],
        "limitations": ["no external CI"],
    }


class OperationsTests(unittest.TestCase):
    def test_requests_bound_detail_and_item_limits(self) -> None:
        self.assertEqual(OperationsCatalogArgs(True, 2).to_mcp_arguments(), {"include_details": True, "max_items": 2})
        self.assertEqual(OpsAcceptanceArgs(3).to_mcp_arguments(), {"max_items": 3})
        with self.assertRaises(ArgumentError):
            OperationsCatalogArgs(max_items=0)
        with self.assertRaises(ArgumentError):
            OpsAcceptanceArgs(max_items=1001)

    def test_catalog_preserves_topology_parity_service_debt_and_full_details(self) -> None:
        report = OperationsCatalogReport.from_wire(operations_payload())
        self.assertTrue(report.promise_parity_holds)
        self.assertEqual(len(report.local.classes), 5)
        self.assertEqual(report.service_contracts.summary.divergence_count, 4)
        self.assertEqual(report.metrics.named_but_undefined, 3)
        self.assertEqual(len(report.details["service_entries"]), 1)
        self.assertFalse(report.service_contracts_all_satisfied)

    def test_acceptance_preserves_unverifiable_and_http_envelope(self) -> None:
        report = ops_acceptance_report({"ok": True, "mcp": {"result": {"structuredContent": acceptance_payload()}}})
        self.assertIsInstance(report, OpsAcceptanceReport)
        self.assertFalse(report.release_ready)
        self.assertFalse(report.decidable)
        self.assertEqual(report.verdict_counts, {"met": 0, "refuted": 1, "unverifiable": 2})
        self.assertEqual(report.findings[1].basis.basis, "no_observer")

    def test_catalog_rejects_forged_parity_and_acceptance_ratio(self) -> None:
        payload = operations_payload(full=False)
        payload["topologies"]["promise_parity"]["holds"] = False
        with self.assertRaises(ArgumentError):
            operations_catalog_report(payload)
        payload = acceptance_payload()
        payload["summary"]["is_release_ready"] = True
        with self.assertRaises(ArgumentError):
            ops_acceptance_report(payload)


if __name__ == "__main__":
    unittest.main()
