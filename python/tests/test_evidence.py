from __future__ import annotations

from pathlib import Path
import sys
import unittest

from prism_sdk import (
    BioCapabilityEvidenceAuditRequest,
    ClaimRequest,
    Client,
    EVIDENCE_DIMENSIONS,
    EvidenceItem,
    EvidenceStatus,
    AsyncClient,
    AsyncWorkspace,
    Workspace,
)
from prism_sdk.errors import ArgumentError


ROOT = Path(__file__).parent
FAKE = ROOT / "fake_mcp_server.py"


def command() -> list[str]:
    return [sys.executable, "-u", str(FAKE)]


def request() -> BioCapabilityEvidenceAuditRequest:
    evidence = [
        EvidenceItem(
            "grounding-1",
            "evidence_grounding",
            EvidenceStatus.OBSERVED,
            domain="oncology",
            support={"source": "ledger:run-1", "scope": "pack/4"},
        ),
        EvidenceItem(
            "time-1",
            "temporal_validity",
            "observed",
            support={"decision_epoch": 10, "evidence_epoch": 9},
        ),
    ]
    claims = [ClaimRequest("claim-1", "publishable capability profile", ("evidence_grounding", "temporal_validity"))]
    return BioCapabilityEvidenceAuditRequest(
        evidence,
        claims,
        vectors=(
            {"system": "agent-a", "capability": "retrieval", "value": 0.8},
            {"system": "agent-b", "capability": "retrieval", "value": 0.7},
        ),
        information={"problem": {"id": "p1"}, "belief": {"h1": 0.5}, "acquisitions": []},
        reference={"id": "reference-1"},
        reference_state="recorded",
        biological_claim="research-only claim",
        max_items=7,
    )


class EvidenceModelTests(unittest.TestCase):
    def test_typed_request_preserves_dimensions_support_and_optional_audits(self) -> None:
        built = request().to_mcp_arguments()

        self.assertEqual(built["max_items"], 7)
        self.assertEqual(built["evidence"][0]["source"], "ledger:run-1")
        self.assertEqual(built["evidence"][1]["evidence_epoch"], 9)
        self.assertEqual(built["claim_requests"][0]["requires"], ["evidence_grounding", "temporal_validity"])
        self.assertEqual(len(EVIDENCE_DIMENSIONS), 9)
        self.assertEqual(built["reference_state"], "recorded")

    def test_request_fails_closed_on_missing_metric_basis_and_unsafe_fields(self) -> None:
        with self.assertRaises(ArgumentError):
            BioCapabilityEvidenceAuditRequest(
                [EvidenceItem("e1", "evidence_grounding", "declared")],
                [ClaimRequest("c1", "claim", ("evidence_grounding",))],
            )
        with self.assertRaises(ArgumentError):
            EvidenceItem("e1", "evidence_grounding", "observed", support={"id": "override"})
        with self.assertRaises(ArgumentError):
            BioCapabilityEvidenceAuditRequest(
                [
                    EvidenceItem("e1", "evidence_grounding", "declared"),
                    EvidenceItem("e1", "evidence_grounding", "declared"),
                ],
                [ClaimRequest("c1", "claim", ("evidence_grounding",))],
                vectors=({"system": "a"}, {"system": "b"}),
            )

    def test_sync_workspace_exposes_evidence_conditioned_audit(self) -> None:
        with Client(command(), timeout=2) as client:
            result = Workspace(client).biocapability_evidence_audit(request())

        self.assertEqual(result["echo"]["claim_requests"][0]["id"], "claim-1")
        self.assertEqual(result["echo"]["evidence"][0]["dimension"], "evidence_grounding")


class AsyncEvidenceWorkspaceTests(unittest.IsolatedAsyncioTestCase):
    async def test_async_workspace_exposes_evidence_conditioned_audit(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).biocapability_evidence_audit(request())

        self.assertEqual(result["echo"]["max_items"], 7)


if __name__ == "__main__":
    unittest.main()
