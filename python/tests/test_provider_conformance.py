from __future__ import annotations

import json

import pytest

from prism_sdk import (
    MAX_PROVIDER_CONFORMANCE_CHECKS,
    PROVIDER_CONFORMANCE_CHECK_NAMES,
    PROVIDER_CONFORMANCE_PROVIDERS,
    ProviderError,
    assert_provider_protocol_conformance,
    run_provider_protocol_conformance,
)


def test_all_builtin_provider_protocols_pass_keyless_loopback_gate():
    report = run_provider_protocol_conformance()

    assert report.status == "passed"
    assert report.provider_count == len(PROVIDER_CONFORMANCE_PROVIDERS)
    assert report.check_count == MAX_PROVIDER_CONFORMANCE_CHECKS
    assert report.failed_provider_count == 0
    assert report.failed_check_count == 0
    assert report.transport == "local_loopback_fixture_never_external"
    assert report.retention == "metadata_only;request_response_and_credentials_not_retained"
    assert "offline-fixture-token" not in json.dumps(report.to_dict(), sort_keys=True)
    assert all(provider.fixture_call_count == 3 for provider in report.providers)
    assert all(provider.check_count == len(PROVIDER_CONFORMANCE_CHECK_NAMES) for provider in report.providers)
    assert all(check.status == "passed" for check in report.checks)

    assert_provider_protocol_conformance(report)


def test_provider_protocol_gate_supports_bounded_single_provider_runs():
    report = run_provider_protocol_conformance(
        providers=("anthropic",),
        model="fixture-model-v2",
    )

    assert report.status == "passed"
    assert report.providers[0].provider == "anthropic"
    assert report.providers[0].protocol == "anthropic_messages"
    assert report.providers[0].fixture_call_count == 3
    assert_provider_protocol_conformance(report.to_dict())


@pytest.mark.parametrize(
    "providers,model",
    [
        (("openai", "openai"), "fixture-model"),
        (("unknown",), "fixture-model"),
        (("openai",), ""),
        (("openai",), "line\nbreak"),
    ],
)
def test_provider_protocol_gate_rejects_ambiguous_or_unsafe_inputs(providers, model):
    with pytest.raises(ProviderError):
        run_provider_protocol_conformance(providers=providers, model=model)


def test_provider_protocol_gate_detects_tampered_metadata_report():
    payload = run_provider_protocol_conformance(providers=("openai",)).to_dict()
    payload["passed_check_count"] = 0

    with pytest.raises(ProviderError):
        assert_provider_protocol_conformance(payload)
