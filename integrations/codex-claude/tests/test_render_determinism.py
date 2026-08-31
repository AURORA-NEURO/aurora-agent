import json
import unittest
from pathlib import Path

from aurora_codex_claude import (
    CLAUDE,
    CODEX,
    AuroraServerSpec,
    aurora_stdio_spec,
    dump_json,
    dump_toml,
    render_config,
    render_entry,
    spec_from_mapping,
)

_REPO_ROOT = Path(__file__).resolve().parents[3]


def neutral_mapping(order_reversed: bool = False) -> dict:
    entry = {
        "name": "aurora",
        "transport": "stdio",
        "command": "./target/release/bioprism-mcp",
        "args": ["--root", "."],
        "env": {"MODE": "offline", "LOG_LEVEL": "info"},
    }
    if order_reversed:
        entry["env"] = dict(reversed(list(entry["env"].items())))
    return {"transport": "stdio", **entry}


class RenderDeterminismTests(unittest.TestCase):
    def test_repeated_renders_are_byte_identical_for_both_profiles(self):
        spec = aurora_stdio_spec(env={"MODE": "offline"})
        for profile in (CLAUDE, CODEX):
            with self.subTest(profile=profile.profile_id):
                self.assertEqual(render_config(profile, spec), render_config(profile, spec))

    def test_input_insertion_order_does_not_change_output_bytes(self):
        one = render_config(CLAUDE, spec_from_mapping(neutral_mapping()))
        two = render_config(CLAUDE, spec_from_mapping(neutral_mapping(order_reversed=True)))
        three = render_config(CLAUDE, spec_from_mapping(dict(reversed(list(neutral_mapping().items())))))
        self.assertEqual(one, two)
        self.assertEqual(one, three)

    def test_codex_output_is_exact_toml_with_lf_newlines_and_trailing_newline(self):
        spec = aurora_stdio_spec(root=".", env={"MODE": "offline"})
        text = render_config(CODEX, spec)
        self.assertEqual(
            text,
            "[mcp_servers.aurora]\n"
            'command = "./target/release/bioprism-mcp"\n'
            'args = ["--root", "."]\n'
            'env = { MODE = "offline" }\n',
        )
        self.assertNotIn("\r", text)
        self.assertTrue(text.endswith("\n"))

    def test_claude_output_matches_canonical_json_of_the_repo_contract(self):
        text = render_config(CLAUDE, aurora_stdio_spec(name="aurora-agent"))
        self.assertEqual(
            text,
            '{"mcpServers":{"aurora-agent":{"args":["--root","."],'
            '"command":"./target/release/bioprism-mcp","env":{}}}}',
        )
        self.assertIsInstance(json.loads(text)["mcpServers"]["aurora-agent"], dict)

    def test_generated_claude_document_normalizes_the_root_mcp_json_file(self):
        """The adapter reproduces the repository's own .mcp.json shape byte-for-byte."""
        mcp_json = _REPO_ROOT / ".mcp.json"
        if not mcp_json.exists():
            self.skipTest("repository .mcp.json not available (standalone checkout)")
        raw = json.loads(mcp_json.read_text(encoding="utf-8"))
        names = sorted(raw["mcpServers"])
        specs = []
        for name in names:
            raw_entry = raw["mcpServers"][name]
            specs.append(
                AuroraServerSpec(
                    name=name,
                    kind="stdio",
                    command=raw_entry["command"],
                    args=tuple(raw_entry.get("args", ())),
                    env=tuple(raw_entry.get("env", {}).items()),
                )
            )
        rendered = render_config(CLAUDE, specs)
        self.assertEqual(json.loads(rendered), raw)
        self.assertEqual(rendered, dump_json({"mcpServers": raw["mcpServers"]}))

    def test_toml_writer_quotes_names_that_are_not_bare_keys(self):
        spec = AuroraServerSpec(name="aurora remote", kind="http", url="https://mcp.example.test/mcp")
        text = render_config(CODEX, spec)
        self.assertIn('[mcp_servers."aurora remote"]', text)
        self.assertIn('url = "https://mcp.example.test/mcp"', text)

    def test_render_entry_structure_is_inspectable_independent_of_bytes(self):
        entry = render_entry(CODEX, aurora_stdio_spec())
        self.assertEqual(entry["command"], "./target/release/bioprism-mcp")
        self.assertEqual(tuple(entry["args"]), ("--root", "."))
        self.assertNotIn("env", entry)


if __name__ == "__main__":
    unittest.main()
