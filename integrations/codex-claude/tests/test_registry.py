import re
import unittest
from pathlib import Path

from aurora_codex_claude import (
    AURORA_CAPABILITIES,
    REGISTRY_SNAPSHOT_SOURCE,
    UnknownCapabilityError,
    aurora_stdio_spec,
    validate_allowlist,
)

_REPO_ROOT = Path(__file__).resolve().parents[3]
_LAUNCHER_SOURCE = _REPO_ROOT / "crates" / "mcp" / "src" / "main.rs"


class RegistryGroundingTests(unittest.TestCase):
    def test_every_registry_name_appears_in_the_mcp_launcher_contract(self):
        """The snapshot must stay a subset of what bioprism-mcp actually advertises."""
        if not _LAUNCHER_SOURCE.exists():
            self.skipTest(f"{REGISTRY_SNAPSHOT_SOURCE} not available (standalone checkout)")
        contract_text = _LAUNCHER_SOURCE.read_text(encoding="utf-8")
        missing = [
            name
            for name in sorted(AURORA_CAPABILITIES)
            if not re.search(rf"\b{re.escape(name)}\b", contract_text)
        ]
        self.assertEqual(missing, [], "registry names absent from the launcher contract")

    def test_registry_is_a_frozenset_and_nonempty(self):
        self.assertIsInstance(AURORA_CAPABILITIES, frozenset)
        self.assertGreater(len(AURORA_CAPABILITIES), 0)


class AllowlistValidationTests(unittest.TestCase):
    def test_unknown_capability_is_refused_with_the_offender_named(self):
        with self.assertRaises(UnknownCapabilityError) as raised:
            validate_allowlist(["fiber_compile", "fiber_teleport"])
        message = str(raised.exception)
        self.assertIn("fiber_teleport", message)
        self.assertNotIn("fiber_compile,", message)

    def test_allowlist_rejects_empty_duplicates_and_non_sequences(self):
        with self.assertRaises(UnknownCapabilityError):
            validate_allowlist([])
        with self.assertRaises(UnknownCapabilityError):
            validate_allowlist(["fiber_compile", "fiber_compile"])
        with self.assertRaises(UnknownCapabilityError):
            validate_allowlist("fiber_compile")
        with self.assertRaises(UnknownCapabilityError):
            validate_allowlist([42])

    def test_validated_allowlist_is_sorted_and_registry_backed(self):
        self.assertEqual(
            validate_allowlist(["world_validate", "fiber_compile", "context_compare"]),
            ("context_compare", "fiber_compile", "world_validate"),
        )

    def test_spec_carries_canonical_allowlist_or_none_for_unrestricted(self):
        unrestricted = aurora_stdio_spec()
        self.assertIsNone(unrestricted.allowlist)
        restricted = aurora_stdio_spec(allowlist=["fiber_verify", "fiber_compile"])
        self.assertEqual(restricted.allowlist, ("fiber_compile", "fiber_verify"))
        with self.assertRaises(UnknownCapabilityError):
            aurora_stdio_spec(allowlist=["not_a_tool"])


if __name__ == "__main__":
    unittest.main()
