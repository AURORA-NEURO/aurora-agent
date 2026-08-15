from __future__ import annotations

from pathlib import Path
import sys
import unittest

from prism_sdk import (
    Admissibility,
    AsyncClient,
    AsyncWorkspace,
    AuthoringError,
    Client,
    EvidenceTier,
    Finding,
    Independence,
    JudgementBuilder,
    OracleCombineRequest,
    OracleManifest,
    OracleRef,
    OracleVersion,
    Position,
    PositionDistribution,
    ReferencePanelRequest,
    ValidityWindow,
    Workspace,
)


ROOT = Path(__file__).parent
FAKE = ROOT / "fake_mcp_server.py"


def command() -> list[str]:
    return [sys.executable, "-u", str(FAKE)]


def manifest(*, circular: bool = False) -> OracleManifest:
    return OracleManifest(
        oracle=OracleRef("demo:checksum", OracleVersion(1, 2, 0)),
        declared_tier=EvidenceTier.DETERMINISTIC,
        establishes=frozenset({"artifact"}),
        cannot_establish=frozenset({"biological", "causal"}),
        validity=ValidityWindow("2025-01-01T00:00:00Z", "2030-01-01T00:00:00Z"),
        independence=Independence(
            from_evaluated_system=not circular,
            shared=frozenset({"labels"}) if circular else frozenset(),
        ),
    )


def judgement() -> object:
    return (
        JudgementBuilder(manifest(), "2026-01-01T00:00:00Z", Position.SUPPORTED)
        .finding(Finding.checksum_mismatch("/digest", "bad", "good"))
        .rationale("content hash matched the canonical artifact")
        .build()
    )


class OracleModelTests(unittest.TestCase):
    def test_manifest_demotes_circular_evidence_and_retains_admissibility(self) -> None:
        circular = manifest(circular=True)
        self.assertEqual(circular.effective_tier, EvidenceTier.PROPERTY)
        self.assertTrue(circular.admissibility("2026-01-01T00:00:00Z").is_admissible)
        self.assertEqual(
            circular.admissibility("2031-01-01T00:00:00Z").state,
            "expired",
        )
        self.assertEqual(circular.to_dict()["independence"]["shared"], ["labels"])

    def test_judgement_preserves_tier_planes_finding_and_distribution(self) -> None:
        distribution = PositionDistribution.from_mapping(
            {Position.SUPPORTED: 0.5, Position.CONTRADICTED: 0.5}
        )
        self.assertEqual(distribution.modes(), frozenset({Position.SUPPORTED, Position.CONTRADICTED}))
        value = (
            JudgementBuilder(manifest(), "2026-01-01T00:00:00Z", Position.SUPPORTED)
            .belief(distribution)
            .build()
        )
        document = value.to_dict()
        self.assertEqual(document["tier"], "deterministic")
        self.assertEqual(document["establishes"], ["artifact"])
        self.assertEqual(document["belief"]["supported"], 0.5)
        self.assertEqual(document["admissibility"], {"state": "admissible"})

    def test_requests_validate_bounds_and_emit_exact_tool_arguments(self) -> None:
        request = OracleCombineRequest(
            "world:demo",
            "2026-01-01T00:00:00Z",
            (judgement(),),
            EvidenceTier.EXECUTION,
            7,
        )
        arguments = request.to_mcp_arguments()
        self.assertEqual(arguments["minimum_deciding_tier"], "execution")
        self.assertEqual(arguments["judgements"][0]["oracle"]["id"], "demo:checksum")
        self.assertEqual(ReferencePanelRequest({"reads": []}).to_mcp_arguments()["max_items"], 100)
        with self.assertRaises(AuthoringError):
            PositionDistribution.from_mapping({"supported": 0.7, "contradicted": 0.1})
        with self.assertRaises(AuthoringError):
            OracleCombineRequest("world:demo", "2026-01-01T00:00:00Z", (), EvidenceTier.JUDGE, 1)


class OracleWorkspaceTests(unittest.TestCase):
    def test_workspace_exposes_oracle_and_evaluation_contracts(self) -> None:
        with Client(command(), timeout=2) as client:
            workspace = Workspace(client)
            combined = workspace.oracle_combine(
                "world:demo",
                "2026-01-01T00:00:00Z",
                [judgement()],
                minimum_deciding_tier="execution",
                max_items=4,
            )
            self.assertEqual(combined["echo"]["minimum_deciding_tier"], "execution")
            panel = workspace.oracle_reference_panel({"reads": []}, max_items=3)
            self.assertEqual(panel["echo"]["max_items"], 3)
            missing = workspace.oracle_missingness(
                {"groups": []}, {"name": "outcome"}, {"sensitivity": "aggregate"}, 5
            )
            self.assertEqual(missing["echo"]["small_cell_floor"], 5)
            worldline = workspace.evaluation_worldline_audit({"observations": [], "decisions": []})
            self.assertIn("worldline", worldline["echo"])
            reference = workspace.bioeval_reference_audit({"kind": "unresolved", "reason": "pending"})
            self.assertEqual(reference["echo"]["reference"]["kind"], "unresolved")
            trajectory = workspace.evaluation_trajectory_check({"steps": []}, step=0, horizon=1)
            self.assertEqual(trajectory["echo"]["horizon"], 1)


class AsyncOracleWorkspaceTests(unittest.IsolatedAsyncioTestCase):
    async def test_async_workspace_matches_sync_oracle_surface(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            workspace = AsyncWorkspace(client)
            result = await workspace.oracle_combine(
                "world:demo", "2026-01-01T00:00:00Z", [judgement()]
            )
            self.assertEqual(result["echo"]["subject"], "world:demo")
            result = await workspace.evaluation_reproduction_check(
                {"outputs": []}, biological_claim="not a validity claim"
            )
            self.assertEqual(result["echo"]["biological_claim"], "not a validity claim")


if __name__ == "__main__":
    unittest.main()
