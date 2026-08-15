from __future__ import annotations

from pathlib import Path
import sys
import unittest

from prism_sdk import (
    AsyncClient,
    AsyncWorkspace,
    AuthoringError,
    Client,
    DecisionCellBuilder,
    InputRef,
    MutationPlan,
    MutationSpec,
    PackArtifact,
    PackBuilder,
    Workspace,
    content_digest,
    validate_pack,
)


ROOT = Path(__file__).parent
FAKE = ROOT / "fake_mcp_server.py"


def command() -> list[str]:
    return [sys.executable, "-u", str(FAKE)]


def pack() -> PackArtifact:
    return (
        PackBuilder(
            pack_id="demo.pack",
            version=(1, 0, 0),
            schema_range=(1, 2),
            title="Decision evidence",
            measures="sufficiency of evidence selection",
            blueprint_module="15.01",
            axis="mechanism",
            capabilities=[{"agent": "evidence_acquisition"}],
            domains=["science", "biomedical"],
            owners=["aurora"],
            license="Apache-2.0",
        )
        .parent("world:demo", 3)
        .decision_family("smallest-sufficient-context")
        .mutation_relation("preserves_verdict")
        .oracle("deterministic")
        .authored_instances(8)
        .trial_counts(12, 2)
        .effective_sample(8)
        .build()
    )


class AuthoringModelTests(unittest.TestCase):
    def test_pack_is_digest_bound_and_has_public_denominators(self) -> None:
        artifact = pack()
        self.assertEqual(artifact.digest, content_digest(artifact.document))
        self.assertEqual(artifact.digest, "eb56a40579ce099d459181c065a93efa6a121897978d85aceb3eacc9636a41c8")
        self.assertEqual(artifact.counts["validated_instances"], 8)
        self.assertEqual(artifact.counts["decision_parents"], 3)
        arguments = artifact.to_mcp_arguments({"calibration": [], "trivial_baselines": [], "contamination": []})
        self.assertEqual(arguments["pack"]["manifest"]["id"], "demo.pack")
        self.assertNotIn("digest", arguments["pack"], "the address is derived, not mutable pack content")
        exposed = artifact.document
        exposed["manifest"]["id"] = "tampered.pack"
        self.assertEqual(artifact.document["manifest"]["id"], "demo.pack")
        self.assertEqual(artifact.digest, content_digest(artifact.document))

    def test_pack_validation_reports_all_cross_field_errors(self) -> None:
        document = pack().document
        document["manifest"]["dependencies"] = [{"id": "demo.other", "digest": "not-a-digest"}]
        document["content"]["instances"] = {
            "kind": "deterministic_generator",
            "seeds": {"start": 4, "end_exclusive": 5},
            "declared": 3,
            "validated": 4,
        }
        report = validate_pack(document)
        self.assertFalse(report.ok)
        self.assertGreaterEqual(len(report.errors), 3)
        with self.assertRaises(AuthoringError):
            report.raise_if_invalid()

    def test_pack_capabilities_keep_rust_external_taxonomy_tags(self) -> None:
        artifact = pack()
        self.assertEqual(artifact.document["manifest"]["capabilities"], [{"agent": "evidence_acquisition"}])
        malformed = artifact.document
        malformed["manifest"]["capabilities"] = ["evidence_acquisition"]
        report = validate_pack(malformed)
        self.assertEqual(report.errors[0].code, "invalid_capability")

    def test_decision_cell_is_set_valued_and_fail_closed(self) -> None:
        cell = (
            DecisionCellBuilder(
                "cell-1",
                "select evidence",
                InputRef.from_document("world.json", {"world": 1}),
                InputRef.from_document("query.json", {"query": 1}),
            )
            .accepting("valid", "equivalent")
            .requiring_witness("closure", "causal")
            .build()
        )
        self.assertTrue(cell.accepts("equivalent", {"closure", "causal"}, True).passed)
        self.assertEqual(cell.accepts("invalid", {"closure", "causal"}, True).reason, "wrong_verdict")
        missing = cell.accepts("valid", {"closure"}, True)
        self.assertEqual(missing.reason, "missing_witnesses")
        self.assertEqual(missing.missing_witnesses, ("causal",))
        self.assertEqual(cell.accepts("valid", {"closure", "causal"}, False).reason, "closure_incomplete")

    def test_mutation_plan_matches_the_rust_standard_suite_shape(self) -> None:
        plan = MutationPlan.standard()
        self.assertEqual(len(plan.mutations), 8)
        self.assertEqual(plan.mutations[-1].relation["kind"], "preprocessing_leakage")
        self.assertEqual(plan.standard_tool_arguments("fixtures/world.json")["suite"], "standard")
        with self.assertRaises(AuthoringError):
            MutationPlan((MutationSpec("same", "camouflage_tags"), MutationSpec("same", "camouflage_tags"))).to_list()

    def test_workspace_sends_authored_pack_and_bounded_mutation_request(self) -> None:
        with Client(command(), timeout=2) as client:
            workspace = Workspace(client)
            health = workspace.pack_health_assess(pack(), {"calibration": [], "trivial_baselines": [], "contamination": []})
            self.assertEqual(health["echo"]["pack"]["manifest"]["id"], "demo.pack")
            mutation = workspace.mutation_family("fixtures/world.json", include_worlds=False, max_worlds=5)
            self.assertEqual(mutation["echo"]["suite"], "standard")
            self.assertEqual(mutation["echo"]["max_worlds"], 5)


class AsyncAuthoringTests(unittest.IsolatedAsyncioTestCase):
    async def test_async_workspace_preserves_authoring_contracts(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            workspace = AsyncWorkspace(client)
            health = await workspace.pack_health_assess(
                pack(), {"calibration": [], "trivial_baselines": [], "contamination": []}
            )
            self.assertEqual(health["echo"]["pack"]["manifest"]["id"], "demo.pack")
            mutation = await workspace.mutation_family("fixtures/world.json", max_worlds=2)
            self.assertEqual(mutation["echo"]["suite"], "standard")
            self.assertEqual(mutation["echo"]["max_worlds"], 2)


if __name__ == "__main__":
    unittest.main()
