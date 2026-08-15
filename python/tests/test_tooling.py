from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    ToolCatalogue,
    ToolSchemaError,
    ToolValidationReport,
)


class ToolCatalogueTests(unittest.TestCase):
    def setUp(self) -> None:
        self.catalogue = ToolCatalogue.from_definitions(
            [
                {
                    "name": "shape_echo",
                    "description": "fixture",
                    "inputSchema": {
                        "type": "object",
                        "required": ["value"],
                        "properties": {
                            "value": {"type": "integer", "minimum": 1},
                            "mode": {"type": "string", "enum": ["safe", "fast"]},
                            "payload": {
                                "type": "array",
                                "minItems": 1,
                                "items": {"type": "string", "maxLength": 8},
                            },
                        },
                    },
                },
                {
                    "name": "union_echo",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "value": {"anyOf": [{"type": "string"}, {"type": "integer"}]},
                        },
                    },
                },
            ]
        )

    def test_plan_is_digest_bound_and_does_not_execute(self) -> None:
        plan = self.catalogue.plan("shape_echo", {"value": 3, "mode": "safe", "payload": ["ok"]})
        self.assertEqual(plan.tool, "shape_echo")
        self.assertEqual(plan.to_mcp_arguments()["value"], 3)
        self.assertEqual(plan.schema_digest, self.catalogue.get("shape_echo").schema_digest)
        self.assertTrue(plan.report.fully_checked)
        self.assertEqual(len(self.catalogue.definitions), 2)
        self.assertEqual(len(self.catalogue.digest), 64)

    def test_shape_preflight_preserves_distinct_failures(self) -> None:
        missing = self.catalogue.validate("shape_echo", {})
        self.assertIsInstance(missing, ToolValidationReport)
        self.assertFalse(missing.ok)
        self.assertTrue(any(issue.code == "required" for issue in missing.issues))

        wrong_type = self.catalogue.validate("shape_echo", {"value": True})
        self.assertTrue(any(issue.code == "type" for issue in wrong_type.issues))

        wrong_enum = self.catalogue.validate("shape_echo", {"value": 1, "mode": "unknown"})
        self.assertTrue(any(issue.code == "enum" for issue in wrong_enum.issues))

        union = self.catalogue.validate("union_echo", {"value": 3})
        self.assertTrue(union.ok)
        rejected_union = self.catalogue.validate("union_echo", {"value": []})
        self.assertFalse(rejected_union.ok)
        self.assertTrue(any(issue.code == "anyOf_no_match" for issue in rejected_union.issues))

        with self.assertRaises(ToolSchemaError):
            self.catalogue.plan("shape_echo", {"value": 0})
        with self.assertRaises(ToolSchemaError):
            self.catalogue.plan("missing_tool", {})

    def test_unsupported_schema_features_are_warnings_not_false_validation(self) -> None:
        catalogue = ToolCatalogue.from_definitions(
            [
                {
                    "name": "future",
                    "inputSchema": {
                        "type": "object",
                        "dependentSchemas": {"value": {"required": ["other"]}},
                    },
                }
            ]
        )
        report = catalogue.validate("future", {"value": 1})
        self.assertTrue(report.ok)
        self.assertFalse(report.fully_checked)
        self.assertTrue(any(issue.code == "unsupported_schema_keyword" for issue in report.warnings))

    def test_catalogue_rejects_ambiguous_definitions(self) -> None:
        with self.assertRaises(ArgumentError):
            ToolCatalogue.from_definitions(
                [
                    {"name": "same", "inputSchema": {"type": "object"}},
                    {"name": "same", "inputSchema": {"type": "object"}},
                ]
            )
        with self.assertRaises(ToolSchemaError):
            self.catalogue.plan("shape_echo", {"value": object()})
        with self.assertRaises(ArgumentError):
            ToolCatalogue(self.catalogue.definitions, "0" * 64)


if __name__ == "__main__":
    unittest.main()
