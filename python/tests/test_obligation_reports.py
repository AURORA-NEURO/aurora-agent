from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    ObligationGateCheckArgs,
    ObligationGateCheckReport,
    obligation_gate_check_report,
)


class ObligationGateReportTests(unittest.TestCase):
    def test_allowed_and_blocked_gate_states_remain_typed(self) -> None:
        blocked = obligation_gate_check_report({
            "ok": True,
            "schema": "bioprism-mcp/obligation-gate-check/0.1",
            "outcome_kind": "blocked",
            "allowed": False,
            "goal": "publish a validation report",
            "action": {"id": "publish", "regret": "irreversible"},
            "gate": {"gate": "blocked", "reason": {"reason": "prerequisites_unmet"}},
            "refusal": {"reason": "prerequisites_unmet"},
            "graph": {
                "valid": True,
                "sha256": "a" * 64,
                "obligation_count": 2,
                "effective_states": [{"obligation": "validation", "effective": "unseen"}],
            },
        })
        self.assertIsInstance(blocked, ObligationGateCheckReport)
        self.assertFalse(blocked.allowed)
        self.assertEqual(blocked.refusal["reason"], "prerequisites_unmet")

        allowed = obligation_gate_check_report({
            "ok": True,
            "schema": "bioprism-mcp/obligation-gate-check/0.1",
            "outcome_kind": "allowed",
            "allowed": True,
            "goal": "publish a validation report",
            "action": {"id": "publish", "regret": "irreversible"},
            "gate": {"gate": "allowed", "action": "publish", "checked": ["validation"]},
            "refusal": None,
            "graph": {"valid": True, "sha256": "b" * 64, "obligation_count": 2},
        })
        self.assertTrue(allowed.allowed)
        self.assertIsNone(allowed.refusal)

        args = ObligationGateCheckArgs({"goal": "g", "obligations": {}}, {"id": "read", "regret": "reversible"}, max_items=2)
        self.assertEqual(args.to_mcp_arguments()["max_items"], 2)
        with self.assertRaises(ArgumentError):
            ObligationGateCheckArgs({}, {}, max_items=0)
        with self.assertRaises(ArgumentError):
            obligation_gate_check_report({
                "ok": True,
                "schema": "bioprism-mcp/obligation-gate-check/0.1",
                "outcome_kind": "allowed",
                "allowed": True,
                "goal": "g",
                "action": {},
                "gate": {},
                "refusal": {"reason": "unexpected"},
                "graph": {},
            })


if __name__ == "__main__":
    unittest.main()
