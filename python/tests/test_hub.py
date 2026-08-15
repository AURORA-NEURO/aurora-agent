from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    HubLockReport,
    HubResolveReport,
    HubLockArgs,
    HubResolveArgs,
    HubSearchArgs,
    HubSearchReport,
    hub_lock_report,
    hub_resolve_report,
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


def resolution_payload(name: str = "bioprism/root", version: str = "1.0.0", digest: str = "sha256:root") -> dict:
    return {
        "subject": {"name": name, "version": version, "digest": digest},
        "provenance": {
            "authority": {"authority": "authoritative", "registry": "origin"},
            "freshness": {"freshness": "authoritative"},
            "accepted_under": {
                "require_authority": False,
                "accept_undetermined": False,
                "accept_beyond_bound": False,
                "max_accepted_lag": None,
            },
            "notes": [],
        },
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

    def test_resolution_and_lock_reports_retain_digest_policy_and_dependency_witnesses(self) -> None:
        resolved = {
            "ok": True,
            "resolution": resolution_payload(),
            "answered_by": "origin",
            "authoritative": True,
            "catalog_count": 1,
            "guarantees": ["the federation is checked before a catalog answer is accepted"],
            "limitations": ["catalogs and epochs are caller-supplied values"],
        }
        report = hub_resolve_report({"ok": True, "mcp": {"result": {"structuredContent": resolved}}})
        self.assertIsInstance(report, HubResolveReport)
        self.assertEqual(report.resolution.digest, "sha256:root")
        self.assertTrue(report.authoritative)
        lock = {
            "ok": True,
            "entry_count": 2,
            "fully_authoritative": True,
            "answering_registries": ["origin"],
            "remarked_entry_count": 0,
            "entries": [
                {
                    "name": "bioprism/root",
                    "locked": {
                        "resolution": resolution_payload(),
                        "required_by": [{"on": "bioprism/root", "req": {"req": "any"}, "source": {"source": "root"}}],
                    },
                },
                {
                    "name": "bioprism/child",
                    "locked": {
                        "resolution": resolution_payload("bioprism/child", "1.0.0", "sha256:child"),
                        "required_by": [{
                            "on": "bioprism/child",
                            "req": {"req": "compatible", "spec": "1.0.0"},
                            "source": {"source": "pack", "name": "bioprism/root", "version": "1.0.0"},
                        }],
                    },
                },
            ],
            "omitted_entries": 0,
            "max_items": 10,
            "guarantees": ["transitive dependencies are fixed by a bounded deterministic fixpoint"],
        }
        lock_report = hub_lock_report(lock)
        self.assertIsInstance(lock_report, HubLockReport)
        self.assertTrue(lock_report.exhaustive)
        self.assertEqual(lock_report.entries[1].required_by[0].source.kind, "pack")
        self.assertEqual(lock_report.entries[1].resolution.digest, "sha256:child")
        self.assertEqual(HubResolveArgs({}, [], {}).to_mcp_arguments()["request"], {})
        self.assertEqual(HubLockArgs({}, [], {}, max_items=2).max_items, 2)


if __name__ == "__main__":
    unittest.main()
