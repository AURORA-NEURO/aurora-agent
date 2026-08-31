import unittest

from aurora_codex_claude import (
    CLAUDE,
    CODEX,
    ConfigError,
    UnsafeCommandError,
    UnsupportedFeatureError,
    aurora_http_spec,
    aurora_stdio_spec,
    render_config,
    spec_from_mapping,
)


def _stdio_mapping(**overrides):
    entry = {
        "name": "aurora",
        "transport": "stdio",
        "command": "./target/release/bioprism-mcp",
        "args": ["--root", "."],
    }
    entry.update(overrides)
    return entry


class UnsupportedOptionTests(unittest.TestCase):
    def test_provider_specific_keys_are_refused_by_name(self):
        provider_specific = {
            "oauth": {"issuer": "https://example.test"},
            "bearer_token_env_var": "AURORA_TOKEN",
            "http_headers": {"Authorization": "Bearer x"},
            "headersHelper": "/opt/bin/get-headers",
            "startup_timeout_ms": 20000,
            "startup_timeout_sec": 30,
            "tool_timeout_ms": 120000,
            "tool_timeout_sec": 120,
            "enabled_tools": ["fiber_compile"],
            "cwd": "/tmp",
        }
        for key, value in provider_specific.items():
            with self.subTest(key=key):
                with self.assertRaises(UnsupportedFeatureError) as raised:
                    spec_from_mapping(_stdio_mapping(**{key: value}))
                self.assertIn(repr(key), str(raised.exception))

    def test_sse_and_websocket_transports_are_named_refusals(self):
        for transport in ("sse", "ws"):
            with self.subTest(transport=transport):
                with self.assertRaises(UnsupportedFeatureError) as raised:
                    spec_from_mapping({"name": "remote", "type": transport, "url": "https://mcp.example.test"})
                self.assertIn(transport, str(raised.exception))

    def test_streamable_http_alias_is_accepted_as_http(self):
        spec = spec_from_mapping(
            {"name": "remote", "type": "streamable-http", "url": "https://mcp.example.test/mcp"}
        )
        self.assertEqual(spec.kind, "http")

    def test_codex_remote_headers_are_refused_while_claude_accepts_them(self):
        claude_spec = aurora_http_spec(
            "remote", "https://mcp.example.test/mcp", headers={"X-Request-Tag": "aurora"}
        )
        self.assertEqual(render_config(CLAUDE, claude_spec).count("headers"), 1)
        with self.assertRaises(UnsupportedFeatureError) as raised:
            render_config(CODEX, aurora_http_spec(
                "remote", "https://mcp.example.test/mcp", headers={"Authorization": "${AURORA_TOKEN}"}
            ))
        message = str(raised.exception)
        self.assertIn("headers", message)
        # The refusal must not echo the credential reference into a Codex file.
        self.assertNotIn("${AURORA_TOKEN}", message)

    def test_unknown_specification_keys_fail_closed(self):
        with self.assertRaises(ConfigError) as raised:
            spec_from_mapping(_stdio_mapping(vibes="on"))
        self.assertIn("vibes", str(raised.exception))

    def test_shell_syntax_in_command_is_an_unsafe_command_refusal(self):
        with self.assertRaises(UnsafeCommandError):
            aurora_stdio_spec(command="bioprism-mcp --root . && curl https://evil.test")

    def test_conflicting_and_malformed_shapes_fail_closed(self):
        with self.assertRaises(ConfigError):
            spec_from_mapping(_stdio_mapping(url="https://mcp.example.test/mcp"))
        with self.assertRaises(ConfigError):
            spec_from_mapping({"name": "remote", "transport": "http"})
        with self.assertRaises(ConfigError):
            spec_from_mapping({"name": "remote", "transport": "http", "url": "file:///etc/passwd"})
        with self.assertRaises(ConfigError):
            render_config(CLAUDE, [])
        with self.assertRaises(ConfigError):
            render_config(CLAUDE, [aurora_stdio_spec(), aurora_stdio_spec()])


if __name__ == "__main__":
    unittest.main()
