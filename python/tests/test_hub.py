from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    HubSearchArgs,
    HubSearchReport,
    hub_search_report,
)


def search_payload(*, truncated: bool = False) -> dict:
    visible_excluded = [
        {"name": "bioprism/other", "version": "1.0.0", "failed": "tier at least reviewed"}
    ]
    return {
        "ok": True,
        "catalog_count": 1,
        "release_count": 2,
        "requested_limit": None,
        "effective_limit": 100,
        "matches": [
            {
                "name": "bioprism/onco",
                "version": "1.0.0",
                "digest": "sha256:onco",
                "summary": "oncology reference pack",
                "tier": "reviewed",
                "authority": {"authority": "authoritative", "registry": "origin"},
                "freshness": {"freshness": "authoritative"},
                "why": [
                    {"why": "keyword_matched", "keyword": "onco"},
                    {"why": "tier_met", "required": "reviewed", "observed": "reviewed", "according_to": "origin"},
                ],
            }
        ],
        "match_count": 1,
        "excluded": visible_excluded,
        "excluded_count": 1,
        "omitted_excluded": 0,
        "truncated": truncated,
        "guarantees": ["every match carries its matching facets, authority, tier, digest, and freshness"],
        "limitations": ["catalog contents and freshness epochs are caller-supplied in-memory values"],
    }


class HubTests(unittest.TestCase):
    def test_request_preserves_federation_catalogs_query_and_bounds(self) -> None:
        request = HubSearchArgs({"members": {}}, [{"releases": {}}], {"facets": []}, max_items=7)
        wire = request.to_mcp_arguments()
        self.assertEqual(wire["max_items"], 7)
        self.assertEqual(HubSearchArgs.from_wire(wire).catalogs[0]["releases"], {})
        with self.assertRaises(ArgumentError):
            HubSearchArgs({}, [], {}, max_items=0)
        with self.assertRaises(ArgumentError):
            HubSearchArgs({}, [{}] * 101, {}, max_items=1)

    def test_report_preserves_reasons_authority_freshness_and_near_misses(self) -> None:
        report = hub_search_report({"ok": True, "mcp": {"result": {"structuredContent": search_payload()}}})
        self.assertIsInstance(report, HubSearchReport)
        self.assertEqual(report.matches[0].why[0].kind, "keyword_matched")
        self.assertTrue(report.matches[0].authority.authoritative)
        self.assertTrue(report.matches[0].freshness.from_authority)
        self.assertEqual(report.excluded[0].failed, "tier at least reviewed")
        self.assertTrue(report.exhaustive)
        self.assertEqual(report.authoritative_match_count, 1)

    def test_report_keeps_carried_authority_and_every_freshness_state_distinct(self) -> None:
        freshness_variants = [
            {"freshness": "within_bound", "lag": 1, "bound": {"max_lag_epochs": 2}, "synced_at": 9},
            {"freshness": "beyond_bound", "lag": 3, "bound": {"max_lag_epochs": 2}, "synced_at": 7},
            {"freshness": "undetermined", "bound": {"max_lag_epochs": 2}, "synced_at": 9},
            {"freshness": "ahead_of_reference", "synced_at": 11, "reference": 10},
        ]
        for freshness in freshness_variants:
            payload = search_payload()
            payload["matches"][0]["authority"] = {
                "authority": "carried",
                "mirror": "site-mirror",
                "origin": "origin",
            }
            payload["matches"][0]["freshness"] = freshness
            report = hub_search_report(payload)
            self.assertEqual(report.matches[0].authority.answered_by, "site-mirror")
            self.assertEqual(report.matches[0].authority.decision_owner, "origin")
            self.assertEqual(report.matches[0].freshness.kind, freshness["freshness"])

    def test_report_rejects_unexplained_or_inconsistent_search_evidence(self) -> None:
        forged = search_payload()
        forged["matches"][0]["why"] = []
        with self.assertRaises(ArgumentError):
            hub_search_report(forged)
        forged = search_payload()
        forged["match_count"] = 2
        with self.assertRaises(ArgumentError):
            hub_search_report(forged)
        forged = search_payload()
        forged["omitted_excluded"] = 1
        with self.assertRaises(ArgumentError):
            hub_search_report(forged)
        forged = search_payload()
        forged["matches"][0]["authority"] = {"authority": "carried", "mirror": "origin", "origin": "origin"}
        with self.assertRaises(ArgumentError):
            hub_search_report(forged)


if __name__ == "__main__":
    unittest.main()
