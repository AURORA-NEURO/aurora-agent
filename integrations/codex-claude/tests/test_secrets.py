import os
import unittest
from unittest import mock

from aurora_codex_claude import (
    CLAUDE,
    CODEX,
    ConfigError,
    assert_no_resolved_secrets,
    aurora_http_spec,
    aurora_stdio_spec,
    is_secret_bearing_name,
    redacted_mapping,
    render_config,
)

_SECRET_NAME = "AURORA_TEST_TOKEN"
_SECRET_VALUE = "harbor-pilot-9f2c-actual-secret"


class SecretNonResolutionTests(unittest.TestCase):
    def test_literal_value_under_secret_bearing_name_is_rejected_without_echo(self):
        with self.assertRaises(ConfigError) as raised:
            render_config(CLAUDE, aurora_stdio_spec(env={_SECRET_NAME: _SECRET_VALUE}))
        self.assertNotIn(_SECRET_VALUE, str(raised.exception))
        self.assertIn("AURORA_TEST_TOKEN", str(raised.exception))

    def test_literal_authorization_header_is_rejected_for_both_profiles(self):
        for profile in (CLAUDE, CODEX):
            with self.subTest(profile=profile.profile_id):
                with self.assertRaises(ConfigError):
                    render_config(
                        profile,
                        aurora_http_spec(
                            "remote",
                            "https://mcp.example.test/mcp",
                            headers={"Authorization": "Bearer live-token-value"},
                        ),
                    )

    def test_placeholder_survives_rendering_and_process_env_never_leaks_in(self):
        with mock.patch.dict(os.environ, {_SECRET_NAME: _SECRET_VALUE}):
            spec = aurora_stdio_spec(env={_SECRET_NAME: f"${{{_SECRET_NAME}}}"})
            text = render_config(CLAUDE, spec)
        self.assertIn("${" + _SECRET_NAME + "}", text)
        self.assertNotIn(_SECRET_VALUE, text)
        self.assertNotIn(_SECRET_NAME.lower(), text.replace("${" + _SECRET_NAME + "}", ""))

    def test_secret_leak_scanner_detects_a_planted_resolved_value(self):
        """The belt-and-braces scanner must fire on a planted leak, not just pass clean output."""
        with self.assertRaises(AssertionError) as raised:
            assert_no_resolved_secrets(f'env={{"X": "{_SECRET_VALUE}"}}', {_SECRET_NAME: _SECRET_VALUE})
        self.assertIn(_SECRET_NAME, str(raised.exception))
        # Short values are outside the scanner's reach by design and must not fire.
        assert_no_resolved_secrets('env={"X": "abc"}', {"AURORA_TEST_TOKEN": "abc"})

    def test_redaction_never_repeats_literals_under_secret_bearing_names(self):
        config = {"env": {_SECRET_NAME: _SECRET_VALUE, "MODE": "offline"}}
        redacted = redacted_mapping(config)
        self.assertEqual(redacted["env"][_SECRET_NAME], "<redacted>")
        self.assertEqual(redacted["env"]["MODE"], "offline")

    def test_secret_bearing_name_detection_is_case_insensitive_and_fail_closed(self):
        self.assertTrue(is_secret_bearing_name("authorization"))
        self.assertTrue(is_secret_bearing_name("Api-Key"))
        self.assertFalse(is_secret_bearing_name("LOG_LEVEL"))


if __name__ == "__main__":
    unittest.main()
