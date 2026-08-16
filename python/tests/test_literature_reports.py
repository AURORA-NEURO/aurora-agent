from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    LiteratureBindCheckArgs,
    LiteratureBindCheckReport,
    literature_bind_check_report,
)


class LiteratureReportTests(unittest.TestCase):
    def test_binding_and_citation_support_remain_separate(self) -> None:
        citable = literature_bind_check_report({
            "ok": True,
            "schema": "bioprism-mcp/literature-bind-check/0.1",
            "outcome_kind": "citable",
            "bound": True,
            "citable": True,
            "evidence": {
                "outcome_kind": "citable",
                "bound": True,
                "citable": True,
                "citation": {"claim_kind": "published_claim_support", "cited_as": "primary", "direct_evidence": True},
                "refusal": None,
                "refusal_kind": None,
                "citation_refusal": None,
                "citation_refusal_kind": None,
            },
        })
        self.assertIsInstance(citable, LiteratureBindCheckReport)
        self.assertTrue(citable.bound)
        self.assertTrue(citable.citable)

        refused = literature_bind_check_report({
            "ok": True,
            "schema": "bioprism-mcp/literature-bind-check/0.1",
            "outcome_kind": "refused",
            "bound": False,
            "citable": None,
            "evidence": {
                "outcome_kind": "refused",
                "bound": False,
                "citable": None,
                "refusal": {"binding_refusal": "citation_laundering"},
                "refusal_kind": "citation_laundering",
                "citation_refusal": None,
                "citation_refusal_kind": None,
            },
        })
        self.assertFalse(refused.bound)
        self.assertEqual(refused.refusal_kind, "citation_laundering")

        args = LiteratureBindCheckArgs(
            {"text": "source"},
            {"disease": "glioma"},
            "primary",
            {"horizon": "open"},
            claim_kind="published_claim_support",
        )
        self.assertEqual(args.to_mcp_arguments()["at_tier"], "primary")
        with self.assertRaises(ArgumentError):
            LiteratureBindCheckArgs({}, {}, "primary", {}, claim_kind="not_a_claim")


if __name__ == "__main__":
    unittest.main()
