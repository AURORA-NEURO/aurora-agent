from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).parent))

from github_actions_evidence import (  # noqa: E402
    ExportError,
    build_payload,
    canonical_bytes,
    payload_digest,
    write_payload,
)


class GithubActionsEvidenceTests(unittest.TestCase):
    def test_composite_action_contract_is_present(self) -> None:
        action = Path(__file__).parents[1] / ".github" / "actions" / "github-actions-evidence" / "action.yml"
        metadata = action.read_text(encoding="utf-8")
        self.assertIn("using: composite", metadata)
        self.assertIn("$GITHUB_ACTION_PATH/../../../tools/github_actions_evidence.py", metadata)
        self.assertIn("value: ${{ steps.export.outputs['payload-digest'] }}", metadata)
        fixture = Path(__file__).parent / "fixtures" / "github-actions-checks.json"
        fixture_payload = json.loads(fixture.read_text(encoding="utf-8"))
        self.assertEqual([row["name"] for row in fixture_payload["jobs"]], ["unit", "lint"])

    def test_payload_is_bounded_deterministic_and_event_aware(self) -> None:
        event = {
            "workflow_run": {
                "id": 42,
                "conclusion": "success",
                "html_url": "https://github.com/example/repo/actions/runs/42",
            },
            "token": "must-not-be-copied",
        }
        checks = {
            "jobs": [
                {"name": "unit", "conclusion": "success", "duration_ms": 1200},
                {"name": "lint", "status": "skipped", "detail": "advisory"},
            ]
        }
        payload = build_payload(checks, event)
        self.assertEqual(payload["provider"], "github_actions")
        self.assertEqual(payload["run"]["id"], "42")
        self.assertEqual(payload["run"]["conclusion"], "success")
        self.assertEqual(payload["jobs"][1]["conclusion"], "skipped")
        self.assertNotIn("token", json.dumps(payload))
        self.assertEqual(payload_digest(payload), hashlib.sha256(canonical_bytes(payload)).hexdigest())

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "nested" / "payload.json"
            github_output = Path(directory) / "github-output.txt"
            digest = write_payload(payload, output, github_output=github_output)
            self.assertEqual(output.read_bytes(), canonical_bytes(payload) + b"\n")
            self.assertIn(f"payload-digest={digest}", github_output.read_text(encoding="utf-8"))
            self.assertIn("check-count=2", github_output.read_text(encoding="utf-8"))
            with self.assertRaisesRegex(ExportError, "output path.*control characters"):
                write_payload(payload, Path(directory) / "bad\noutput.json")

    def test_missing_run_duplicate_and_oversized_inputs_fail_closed(self) -> None:
        with self.assertRaisesRegex(ExportError, "run.id"):
            build_payload([{"name": "unit", "conclusion": "success"}])
        with self.assertRaisesRegex(ExportError, "duplicate check name"):
            build_payload(
                [
                    {"name": "unit", "conclusion": "success"},
                    {"name": "unit", "conclusion": "success"},
                ],
                {"run_id": "run-1"},
            )
        with self.assertRaisesRegex(ExportError, "more than 64"):
            build_payload(
                [{"name": f"job-{index}", "conclusion": "success"} for index in range(65)],
                {"run_id": "run-1"},
            )

    def test_malformed_digest_and_control_text_are_not_rewritten(self) -> None:
        with self.assertRaisesRegex(ExportError, "lowercase SHA-256"):
            build_payload(
                [{"name": "unit", "conclusion": "success", "result_digest": "A" * 64}],
                {"run_id": "run-1"},
            )
        with self.assertRaisesRegex(ExportError, "control characters"):
            build_payload(
                [{"name": "unit\nforged", "conclusion": "success"}],
                {"run_id": "run-1"},
            )

    def test_cli_accepts_empty_optional_event_input(self) -> None:
        from github_actions_evidence import main

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            checks = root / "checks.json"
            checks.write_text(json.dumps([{"name": "unit", "conclusion": "success"}]), encoding="utf-8")
            output = root / "payload.json"
            previous_run_id = os.environ.get("GITHUB_RUN_ID")
            os.environ["GITHUB_RUN_ID"] = "run-from-environment"
            try:
                exit_code = main(
                    [
                        "--checks",
                        str(checks),
                        "--event",
                        "",
                        "--run-id",
                        "",
                        "--output",
                        str(output),
                    ]
                )
            finally:
                if previous_run_id is None:
                    os.environ.pop("GITHUB_RUN_ID", None)
                else:
                    os.environ["GITHUB_RUN_ID"] = previous_run_id
            self.assertEqual(exit_code, 0)
            self.assertEqual(json.loads(output.read_text(encoding="utf-8"))["run"]["id"], "run-from-environment")


if __name__ == "__main__":
    unittest.main()
