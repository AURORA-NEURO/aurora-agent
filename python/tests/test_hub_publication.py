from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import ApiClient, AsyncApiClient, AsyncWorkspace, BioAtlasPublicationAuditArgs, HubLeaderboardRenderArgs, Workspace, bioatlas_publication_audit, hub_leaderboard_render
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def leaderboard_payload() -> dict:
    ranked_entry = {"rank": 1, "entry": {"submission": "sub-1", "score": {"value": 0.18}}, "verification": "self-reported", "label": {"label": "held_out"}}
    unranked_entry = {"entry": {"submission": "sub-2", "score": {"value": 0.11}}, "reason": {"reason": "not_published", "state": None}}
    return {
        "ok": True,
        "schema": "bioprism-mcp/hub-leaderboard/0.1",
        "board": "glioma-first-divergence",
        "ranked_count": 1,
        "unranked_count": 1,
        "leader_count": 1,
        "headline": "Rank 1 under the stated conditions; no clinical validity and no superiority outside the stated conditions.",
        "rendered": {"board": "glioma-first-divergence", "conditions": {"metric": "first-divergence-rate"}, "ranked": [ranked_entry], "unranked": [unranked_entry]},
        "guarantees": [
            "ranking is computed only within declared comparability conditions",
            "withdrawn, under-review, below-floor, contaminated, undisclosed, and incomparable entries remain visible as unranked reasons",
            "evidence scale and disclosure eligibility are checked before an entry is rankable",
            "the board headline carries its conditions, caveat, and clinical/non-universal nonclaims",
        ],
    }


def card_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/hub-card/0.1",
        "card": {"resource_type": "bioatlas-card", "resource_id": "d", "version": "v", "submission": "sub-1", "scope": {}, "provenance": [], "access": "public", "state": "available", "verification": "self-reported", "score": {"display": "published", "score": {"value": 0.91}, "label": {"label": "held_out"}}, "non_claims": [], "attributions": [], "limitations": "bounded"},
        "score": {"attached": True, "pack": "p", "computed_at": 5},
        "moderation_state": "accepted",
        "verification": "self-reported",
        "guarantees": [],
    }


def audit_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/bioatlas-publication-audit/0.1",
        "workflow": "bioatlas_publication_audit",
        "atlas": {"ok": True, "summary": {"coverage_supports_aggregation": True}},
        "evidence_audit": {"release_posture": {"ready_for_requested_claims": True}},
        "card": card_payload(),
        "leaderboard": leaderboard_payload(),
        "release_request": {"present": True, "id": "release-1", "targets": [{"target": "numeric_card_score", "eligible": True, "blockers": [], "notes": ["evidence"]}], "ready": True, "fail_closed": False, "no_implicit_release": True},
        "cross_layer": {"numeric_score_requires_evidence_audit": True, "numeric_score_evidence_ready": True, "atlas_aggregation_ready": True, "leaderboard_ranked_count": 1, "leaderboard_unranked_count": 1, "unranked_leaderboard_entries_remain_visible": True, "withheld_scores_are_not_zeroes": True},
        "guarantees": ["atlas coverage, evidence-conditioned claims, moderation/card rendering, and leaderboard ranking remain distinct gates", "a publication readiness claim is emitted only for explicit requested targets"],
        "limitations": ["this does not publish a web page"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        value = audit_payload() if name == "bioatlas_publication_audit" else leaderboard_payload()
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(value)}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        value = audit_payload() if name == "bioatlas_publication_audit" else leaderboard_payload()
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(value)}]})


class HubPublicationProjectionTests(unittest.TestCase):
    def test_args_keep_entry_and_release_target_bounds(self) -> None:
        board_args = HubLeaderboardRenderArgs({}, [], {}, {}, include_details=True)
        self.assertTrue(board_args.include_details)
        with self.assertRaises(ArgumentError):
            HubLeaderboardRenderArgs({}, [{} for _ in range(2001)], {}, {})
        with self.assertRaises(ArgumentError):
            BioAtlasPublicationAuditArgs({}, max_items=0)

    def test_leaderboard_preserves_unranked_reason_and_headline_nonclaims(self) -> None:
        report = hub_leaderboard_render(leaderboard_payload())
        self.assertEqual(report.ranked_count, 1)
        self.assertEqual(report.unranked_count, 1)
        self.assertFalse(report.details_omitted)
        self.assertEqual(report.rendered.unranked[0].reason.kind, "not_published")
        self.assertTrue(report.all_unranked_reasons_are_typed)
        self.assertTrue(report.headline_has_nonclaims)
        self.assertTrue(report.rankability_is_gated)
        self.assertTrue(report.unranked_entries_remain_visible)

    def test_composed_audit_keeps_each_gate_and_all_facades(self) -> None:
        report = bioatlas_publication_audit(audit_payload())
        self.assertTrue(report.explicit_release_requested)
        self.assertTrue(report.release_ready)
        self.assertTrue(report.numeric_score_is_conditioned)
        self.assertTrue(report.unranked_entries_remain_visible)
        self.assertTrue(report.gates_are_separate)
        self.assertTrue(report.score_withholding_is_explicit)
        args = BioAtlasPublicationAuditArgs({"atlas": "fixture"})
        self.assertTrue(Workspace(_SyncTool()).bioatlas_publication_audit_report(args).release_ready)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).bioatlas_publication_audit_report(args)).numeric_score_is_conditioned)
        with patch.object(ApiClient, "call_tool", return_value=audit_payload()) as call:
            result = ApiClient("http://127.0.0.1:1").bioatlas_publication_audit_report(args)
        self.assertEqual(result.cross_layer.leaderboard_unranked_count, 1)
        call.assert_called_once_with("bioatlas_publication_audit", args.to_mcp_arguments())

        board_args = HubLeaderboardRenderArgs({}, [], {}, {})
        with patch.object(ApiClient, "call_tool", return_value=leaderboard_payload()) as board_call:
            self.assertEqual(ApiClient("http://127.0.0.1:1").hub_leaderboard_render_report(board_args).leader_count, 1)
        board_call.assert_called_once_with("hub_leaderboard_render", board_args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=audit_payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).bioatlas_publication_audit_report(args)
            self.assertEqual(result.schema, "bioprism-mcp/bioatlas-publication-audit/0.1")
            async_call.assert_called_once_with("bioatlas_publication_audit", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
