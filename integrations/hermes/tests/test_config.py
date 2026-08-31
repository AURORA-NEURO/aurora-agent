import json
import sys
import unittest

from aurora_hermes import (
    ConfigError,
    UnsupportedFeatureError,
    UnsafeCommandError,
    diagnose,
    dump_json,
    load_json,
    normalized_mapping,
    redacted_mapping,
    validate_config,
)


def stdio_config():
    return {
        "mcp_servers": {
            "aurora": {
                "command": sys.executable,
                "args": ["-m", "bioprism_mcp", "--root", "${workspaceFolder}"],
                "env": {"AURORA_TOKEN": "${AURORA_TOKEN}", "MODE": "offline"},
            }
        }
    }


class ConfigTests(unittest.TestCase):
    def test_stdio_normalizes_without_resolving_placeholders(self):
        specs = validate_config(stdio_config())
        self.assertEqual(specs[0].kind, "stdio")
        self.assertEqual(specs[0].env[0], ("AURORA_TOKEN", "${AURORA_TOKEN}"))
        self.assertEqual(specs[0].args[-1], "${workspaceFolder}")

    def test_shell_strings_are_rejected(self):
        config = {"mcp_servers": {"bad": {"command": "python -m thing; curl evil"}}}
        with self.assertRaises(UnsafeCommandError):
            validate_config(config)

    def test_literal_secret_values_are_rejected_and_redaction_does_not_echo(self):
        config = {"mcp_servers": {"bad": {"command": "aurora", "env": {"AURORA_TOKEN": "super-secret-token"}}}}
        with self.assertRaises(ConfigError) as raised:
            validate_config(config)
        self.assertNotIn("super-secret-token", str(raised.exception))
        redacted = redacted_mapping(config)
        self.assertEqual(redacted["mcp_servers"]["bad"]["env"]["AURORA_TOKEN"], "<redacted>")

    def test_http_and_explicit_unsupported_features_are_distinct(self):
        config = {"mcp_servers": {"remote": {"url": "https://example.test/mcp", "headers": {"Authorization": "${AURORA_TOKEN}"}}}}
        spec = validate_config(config)[0]
        self.assertEqual(spec.kind, "http")
        with self.assertRaises(UnsupportedFeatureError):
            validate_config({"mcp_servers": {"remote": {"url": "https://example.test", "oauth": {"issuer": "x"}}}})
        with self.assertRaises(UnsupportedFeatureError):
            validate_config({"mcp_servers": {"remote": {"url": "https://example.test", "transport": "sse"}}})

    def test_malformed_shapes_fail_closed(self):
        with self.assertRaises(ConfigError):
            validate_config({"mcp_servers": {"bad": {"url": "file:///tmp/no"}}})
        with self.assertRaises(ConfigError):
            load_json("[]")

    def test_json_serialization_is_deterministic_and_loadable(self):
        one = dump_json(stdio_config())
        two = dump_json({"mcp_servers": dict(reversed(list(stdio_config()["mcp_servers"].items())))})
        self.assertEqual(one, two)
        self.assertEqual(load_json(one), normalized_mapping(stdio_config()))
        self.assertEqual(json.loads(one)["mcp_servers"]["aurora"]["env"]["AURORA_TOKEN"], "${AURORA_TOKEN}")

    def test_diagnostics_are_unmeasured_by_default_and_non_invasive_when_probed(self):
        spec = validate_config(stdio_config())[0]
        self.assertEqual(diagnose(spec).status, "unmeasured")
        self.assertEqual(diagnose(spec, probe=True).status, "ready")


if __name__ == "__main__":
    unittest.main()
