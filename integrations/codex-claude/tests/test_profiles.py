import unittest

from aurora_codex_claude import (
    CLAUDE,
    CODEX,
    ConfigError,
    aurora_http_spec,
    aurora_stdio_spec,
    permission_entries,
    render_config,
    render_entry,
    resolve_profile,
)


class ProfileDifferenceTests(unittest.TestCase):
    def test_profiles_differ_in_format_key_and_placeholder_contract(self):
        self.assertEqual(CLAUDE.config_format, "json")
        self.assertEqual(CODEX.config_format, "toml")
        self.assertEqual(CLAUDE.servers_key, "mcpServers")
        self.assertEqual(CODEX.servers_key, "mcp_servers")
        self.assertTrue(CLAUDE.expands_placeholders)
        self.assertFalse(CODEX.expands_placeholders)

    def test_same_spec_renders_under_both_profiles_without_drift(self):
        spec = aurora_stdio_spec(env={"MODE": "offline"})
        claude = render_config(CLAUDE, spec)
        codex = render_config(CODEX, spec)
        self.assertIn('"mcpServers"', claude)
        self.assertIn("[mcp_servers.aurora]", codex)
        # One registry, one spec: the launch contract itself is identical.
        self.assertEqual(render_entry(CLAUDE, spec)["command"], render_entry(CODEX, spec)["command"])
        self.assertEqual(render_entry(CLAUDE, spec)["args"], render_entry(CODEX, spec)["args"])

    def test_http_entry_shape_follows_each_documented_host_surface(self):
        spec = aurora_http_spec("remote", "https://mcp.example.test/mcp")
        self.assertEqual(
            render_entry(CLAUDE, spec),
            {"type": "http", "url": "https://mcp.example.test/mcp", "headers": {}},
        )
        self.assertEqual(
            render_entry(CODEX, spec),
            {"url": "https://mcp.example.test/mcp"},
        )

    def test_placeholders_stay_literal_for_claude_and_are_refused_for_codex(self):
        spec = aurora_stdio_spec(root="${workspaceFolder}")
        entry = render_entry(CLAUDE, spec)
        self.assertEqual(entry["args"], ["--root", "${workspaceFolder}"])
        with self.assertRaises(ValueError) as raised:
            render_config(CODEX, spec)
        self.assertIn("placeholder", str(raised.exception))
        self.assertIn("args[1]", str(raised.exception))

    def test_permission_entries_exist_only_where_the_host_documents_a_surface(self):
        spec = aurora_stdio_spec(allowlist=["fiber_compile"])
        self.assertEqual(permission_entries(CLAUDE, spec), ("mcp__aurora__fiber_compile",))
        with self.assertRaises(ValueError) as raised:
            permission_entries(CODEX, spec)
        self.assertIn("codex", str(raised.exception))

    def test_resolve_profile_accepts_ids_and_instances_and_refuses_strangers(self):
        self.assertIs(resolve_profile("claude"), CLAUDE)
        self.assertIs(resolve_profile(CODEX), CODEX)
        with self.assertRaises(ValueError):
            resolve_profile("cursor")
        with self.assertRaises(TypeError):
            resolve_profile(42)


if __name__ == "__main__":
    unittest.main()
