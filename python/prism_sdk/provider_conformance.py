"""Keyless protocol conformance for the built-in Python provider adapters.

This module is deliberately a fixture harness, not a provider client.  It starts a short-lived
loopback HTTP server and sends normal :class:`~prism_sdk.llm_runtime.LLMRuntime` traffic through
that server.  Consequently the conformance gate covers the real credential, HTTP, normalization,
discovery, and SSE boundaries without requiring a user key or external network access.

Only bounded metadata leaves the harness.  Request bodies, response bodies, headers, and the
synthetic fixture credential are retained only in the in-process fixture while one provider is
being checked and are never placed in the report or its digest.
"""

from __future__ import annotations

from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import threading
from collections.abc import Mapping, Sequence
from typing import Any, Callable

from .authoring import content_digest
from .llm_runtime import (
    CredentialError,
    LLMRuntime,
    ProviderConfig,
    ProviderError,
    ProviderRequest,
    ProviderResponse,
    ProviderStreamEvent,
    anthropic_provider,
    deepseek_provider,
    groq_provider,
    mistral_provider,
    openai_provider,
    openrouter_provider,
    xai_provider,
)


PROVIDER_PROTOCOL_CONFORMANCE_SCHEMA = "bioprism-python-provider-protocol-conformance/0.1"
PROVIDER_PROTOCOL_CONFORMANCE_MODE = "keyless_fixture_only"
PROVIDER_CONFORMANCE_PROVIDERS = (
    "openai",
    "anthropic",
    "deepseek",
    "groq",
    "mistral",
    "openrouter",
    "xai",
)
PROVIDER_CONFORMANCE_CHECK_NAMES = (
    "registration",
    "credential_guard",
    "request_wire_shape",
    "credential_header",
    "response_normalization",
    "model_discovery",
    "stream_normalization",
    "secret_redaction",
)
MAX_PROVIDER_CONFORMANCE_PROVIDERS = len(PROVIDER_CONFORMANCE_PROVIDERS)
MAX_PROVIDER_CONFORMANCE_CHECKS = (
    MAX_PROVIDER_CONFORMANCE_PROVIDERS * len(PROVIDER_CONFORMANCE_CHECK_NAMES)
)
_FIXTURE_CREDENTIAL = "offline-fixture-token"
_FIXTURE_REQUEST_ID = "offline-fixture-request"
_DEFAULT_MODEL = "aurora-conformance-model"


@dataclass(frozen=True, slots=True)
class ProviderConformanceCheck:
    """One safe, metadata-only conformance assertion."""

    provider: str
    protocol: str
    check: str
    status: str
    code: str
    metadata_only: bool = True

    def to_dict(self) -> dict[str, Any]:
        return {
            "provider": self.provider,
            "protocol": self.protocol,
            "check": self.check,
            "status": self.status,
            "code": self.code,
            "metadata_only": self.metadata_only,
        }


@dataclass(frozen=True, slots=True)
class ProviderConformanceProviderResult:
    """The aggregate outcome for one provider adapter."""

    provider: str
    protocol: str
    status: str
    check_count: int
    passed_check_count: int
    failed_check_count: int
    fixture_call_count: int
    metadata_only: bool = True

    def to_dict(self) -> dict[str, Any]:
        return {
            "provider": self.provider,
            "protocol": self.protocol,
            "status": self.status,
            "check_count": self.check_count,
            "passed_check_count": self.passed_check_count,
            "failed_check_count": self.failed_check_count,
            "fixture_call_count": self.fixture_call_count,
            "metadata_only": self.metadata_only,
        }


@dataclass(frozen=True, slots=True)
class ProviderProtocolConformanceReport:
    """A reproducible, secret-free provider protocol report."""

    schema: str
    mode: str
    status: str
    provider_count: int
    passed_provider_count: int
    failed_provider_count: int
    check_count: int
    passed_check_count: int
    failed_check_count: int
    providers: tuple[ProviderConformanceProviderResult, ...]
    checks: tuple[ProviderConformanceCheck, ...]
    transport: str
    retention: str
    secret_material: str
    report_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "mode": self.mode,
            "status": self.status,
            "provider_count": self.provider_count,
            "passed_provider_count": self.passed_provider_count,
            "failed_provider_count": self.failed_provider_count,
            "check_count": self.check_count,
            "passed_check_count": self.passed_check_count,
            "failed_check_count": self.failed_check_count,
            "providers": [provider.to_dict() for provider in self.providers],
            "checks": [check.to_dict() for check in self.checks],
            "transport": self.transport,
            "retention": self.retention,
            "secret_material": self.secret_material,
            "report_digest": self.report_digest,
        }


@dataclass(frozen=True, slots=True)
class _FixtureCall:
    method: str
    path: str
    headers: Mapping[str, str]
    body: Mapping[str, Any] | None


class _LoopbackFixture:
    """Short-lived local server that provides the three protocol response families."""

    def __init__(self, model: str) -> None:
        self.model = model
        self.calls: list[_FixtureCall] = []
        self._lock = threading.Lock()

        fixture = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, format: str, *args: Any) -> None:  # noqa: A002
                return

            def _headers(self) -> dict[str, str]:
                return {name.lower(): value for name, value in self.headers.items()}

            def _write(self, payload: bytes, content_type: str) -> None:
                self.send_response(200)
                self.send_header("Content-Type", content_type)
                self.send_header("Content-Length", str(len(payload)))
                self.send_header("x-request-id", _FIXTURE_REQUEST_ID)
                self.end_headers()
                self.wfile.write(payload)

            def do_GET(self) -> None:  # noqa: N802
                fixture._record("GET", self.path, self._headers(), None)
                payload = {
                    "data": [
                        {
                            "id": fixture.model,
                            "active": True,
                            "owned_by": "offline-fixture",
                            "context_window_tokens": 32_000,
                            "max_output_tokens": 4_000,
                            "capabilities": ["reasoning"],
                            "supported_parameters": ["tools", "response_format"],
                        }
                    ]
                }
                self._write(json.dumps(payload, separators=(",", ":")).encode("utf-8"), "application/json")

            def do_POST(self) -> None:  # noqa: N802
                headers = self._headers()
                try:
                    length = int(headers.get("content-length", "0"))
                    raw = self.rfile.read(length)
                    decoded = json.loads(raw.decode("utf-8"))
                except (ValueError, TypeError, UnicodeDecodeError, json.JSONDecodeError):
                    decoded = None
                body = decoded if isinstance(decoded, Mapping) else None
                fixture._record("POST", self.path, headers, body)
                if isinstance(body, Mapping) and body.get("stream") is True:
                    self._write(_stream_payload(fixture.model, body), "text/event-stream")
                    return
                self._write(_normal_payload(fixture.model, body), "application/json")

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    @property
    def base_url(self) -> str:
        return f"http://127.0.0.1:{self.server.server_port}"

    def __enter__(self) -> "_LoopbackFixture":
        self.thread.start()
        return self

    def __exit__(self, exc_type: Any, exc_value: Any, traceback: Any) -> None:
        self.server.shutdown()
        self.thread.join(timeout=5.0)
        self.server.server_close()

    def _record(
        self,
        method: str,
        path: str,
        headers: Mapping[str, str],
        body: Mapping[str, Any] | None,
    ) -> None:
        with self._lock:
            self.calls.append(_FixtureCall(method, path, dict(headers), None if body is None else dict(body)))


def _normal_payload(model: str, body: Mapping[str, Any] | None) -> bytes:
    if isinstance(body, Mapping) and isinstance(body.get("input"), list):
        payload: Mapping[str, Any] = {
            "id": _FIXTURE_REQUEST_ID,
            "model": model,
            "output_text": '{"ok":true}',
            "usage": {"input_tokens": 3, "output_tokens": 2, "total_tokens": 5},
        }
    elif isinstance(body, Mapping) and isinstance(body.get("system"), str):
        payload = {
            "id": _FIXTURE_REQUEST_ID,
            "model": model,
            "content": [{"type": "text", "text": "fixture-answer"}],
            "usage": {"input_tokens": 3, "output_tokens": 2},
            "stop_reason": "end_turn",
        }
    else:
        payload = {
            "id": _FIXTURE_REQUEST_ID,
            "model": model,
            "choices": [
                {
                    "message": {"role": "assistant", "content": '{"ok":true}'},
                    "finish_reason": "stop",
                }
            ],
            "usage": {"prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5},
        }
    return json.dumps(payload, separators=(",", ":")).encode("utf-8")


def _stream_payload(model: str, body: Mapping[str, Any]) -> bytes:
    if isinstance(body.get("input"), list):
        frames = (
            'event: response.output_text.delta\ndata: {"delta":"fixture-stream"}\n\n',
            'event: response.completed\ndata: {"response":{"id":"offline-fixture-request","model":"'
            + model
            + '","usage":{"input_tokens":3,"output_tokens":2,"total_tokens":5}}}\n\n',
        )
    elif isinstance(body.get("system"), str):
        frames = (
            'event: message_start\ndata: {"message":{"id":"offline-fixture-request","model":"'
            + model
            + '","usage":{"input_tokens":3}}}\n\n',
            'event: content_block_delta\ndata: {"index":0,"delta":{"type":"text_delta","text":"fixture-stream"}}\n\n',
            'event: message_delta\ndata: {"usage":{"output_tokens":2}}\n\n',
            'event: message_stop\ndata: {}\n\n',
        )
    else:
        frames = (
            'data: {"id":"offline-fixture-request","model":"'
            + model
            + '","choices":[{"delta":{"content":"fixture-stream"}}]}\n\n',
            'data: {"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}\n\n',
        )
    return "".join(frames).encode("utf-8")


def _provider_config(provider: str, base_url: str) -> ProviderConfig:
    factory: Callable[..., ProviderConfig] = {
        "openai": openai_provider,
        "anthropic": anthropic_provider,
        "deepseek": deepseek_provider,
        "groq": groq_provider,
        "mistral": mistral_provider,
        "openrouter": openrouter_provider,
        "xai": xai_provider,
    }[provider]
    return factory(base_url=base_url, allow_insecure_http=True)


def _request_for(config: ProviderConfig, model: str) -> ProviderRequest:
    response_schema = (
        {"type": "object", "properties": {"ok": {"type": "boolean"}}, "required": ["ok"]}
        if config.protocol != "anthropic_messages"
        else None
    )
    return ProviderRequest(
        model=model,
        messages=(
            {"role": "system", "content": "You are an offline protocol fixture."},
            {"role": "user", "content": "Return the fixture response."},
        ),
        max_output_tokens=64,
        temperature=0.0,
        require_json=config.protocol != "anthropic_messages",
        response_schema=response_schema,
    )


def _check(
    provider: str,
    protocol: str,
    name: str,
    assertion: Callable[[], bool],
) -> ProviderConformanceCheck:
    try:
        passed = bool(assertion())
    except Exception as error:
        passed = False
        code = {
            CredentialError: "credential_error",
            ProviderError: "provider_error",
            AssertionError: "assertion_error",
        }.get(type(error), "check_error")
    else:
        code = "ok" if passed else "mismatch"
    return ProviderConformanceCheck(
        provider=provider,
        protocol=protocol,
        check=name,
        status="passed" if passed else "failed",
        code=code,
    )


def _wire_shape_check(config: ProviderConfig, model: str, call: _FixtureCall | None) -> bool:
    if call is None or call.method != "POST" or call.path != config.endpoint[2] or call.body is None:
        return False
    body = call.body
    if body.get("model") != model or body.get("max_output_tokens", body.get("max_tokens")) != 64:
        return False
    if config.protocol == "openai_responses":
        return (
            isinstance(body.get("input"), list)
            and isinstance(body.get("text"), Mapping)
            and isinstance(body["text"].get("format"), Mapping)
        )
    if config.protocol == "anthropic_messages":
        return (
            isinstance(body.get("messages"), list)
            and isinstance(body.get("system"), str)
            and "response_format" not in body
        )
    return (
        isinstance(body.get("messages"), list)
        and isinstance(body.get("response_format"), Mapping)
    )


def _credential_header_check(config: ProviderConfig, call: _FixtureCall | None) -> bool:
    if call is None:
        return False
    if config.protocol == "anthropic_messages":
        return (
            call.headers.get("x-api-key") == _FIXTURE_CREDENTIAL
            and call.headers.get("anthropic-version") == "2023-06-01"
        )
    return call.headers.get("authorization") == f"Bearer {_FIXTURE_CREDENTIAL}"


def _response_check(config: ProviderConfig, model: str, response: ProviderResponse | None) -> bool:
    if response is None or response.status_code != 200:
        return False
    if response.provider != config.provider or response.model != model:
        return False
    if response.request_id != _FIXTURE_REQUEST_ID:
        return False
    if config.protocol == "anthropic_messages":
        return response.text == "fixture-answer" and response.structured is None
    return response.text == '{"ok":true}' and response.structured == {"ok": True}


def _discovery_check(config: ProviderConfig, model: str, calls: Sequence[_FixtureCall], descriptors: Sequence[Any]) -> bool:
    if not descriptors or any(descriptor.model != model for descriptor in descriptors):
        return False
    model_calls = [call for call in calls if call.method == "GET"]
    if len(model_calls) != 1:
        return False
    call = model_calls[0]
    return call.path == config.models_endpoint[2] and _credential_header_check(config, call)


def _stream_check(config: ProviderConfig, model: str, events: Sequence[ProviderStreamEvent]) -> bool:
    if not events:
        return False
    text = "".join(event.text_delta for event in events)
    if text != "fixture-stream" or not any(event.done for event in events):
        return False
    return all(event.provider == config.provider and event.model == model for event in events)


def _run_provider(provider: str, model: str) -> tuple[ProviderConformanceProviderResult, tuple[ProviderConformanceCheck, ...]]:
    checks: list[ProviderConformanceCheck] = []
    with _LoopbackFixture(model) as fixture:
        config = _provider_config(provider, fixture.base_url)
        runtime = LLMRuntime(sleeper=lambda _delay: None)
        runtime.register_provider(config)
        handle = runtime.credentials.register(provider, _FIXTURE_CREDENTIAL)
        request = _request_for(config, model)

        calls_before_guard = len(fixture.calls)
        guard_refused = False
        try:
            tuple(runtime.invoke_stream(provider, request))
        except CredentialError:
            guard_refused = len(fixture.calls) == calls_before_guard
        except Exception:
            guard_refused = False

        checks.append(
            _check(
                provider,
                config.protocol,
                "registration",
                lambda: runtime.provider_requires_credential(provider) is True,
            )
        )
        checks.append(_check(provider, config.protocol, "credential_guard", lambda: guard_refused))

        response: ProviderResponse | None = None
        try:
            response = runtime.invoke(provider, request, credential=handle)
        except Exception:
            response = None
        unary_call = next((call for call in fixture.calls if call.method == "POST"), None)
        checks.append(
            _check(provider, config.protocol, "request_wire_shape", lambda: _wire_shape_check(config, model, unary_call))
        )
        checks.append(
            _check(provider, config.protocol, "credential_header", lambda: _credential_header_check(config, unary_call))
        )
        checks.append(
            _check(provider, config.protocol, "response_normalization", lambda: _response_check(config, model, response))
        )

        descriptors: tuple[Any, ...] = ()
        try:
            descriptors = runtime.discover_models(provider, credential=handle)
        except Exception:
            descriptors = ()
        checks.append(
            _check(
                provider,
                config.protocol,
                "model_discovery",
                lambda: _discovery_check(config, model, fixture.calls, descriptors),
            )
        )

        events: tuple[ProviderStreamEvent, ...] = ()
        try:
            events = tuple(runtime.invoke_stream(provider, request, credential=handle))
        except Exception:
            events = ()
        checks.append(
            _check(provider, config.protocol, "stream_normalization", lambda: _stream_check(config, model, events))
        )

        checks.append(
            _check(
                provider,
                config.protocol,
                "secret_redaction",
                lambda: _FIXTURE_CREDENTIAL not in json.dumps(
                    [check.to_dict() for check in checks], sort_keys=True
                ),
            )
        )

        passed = sum(check.status == "passed" for check in checks)
        result = ProviderConformanceProviderResult(
            provider=provider,
            protocol=config.protocol,
            status="passed" if passed == len(PROVIDER_CONFORMANCE_CHECK_NAMES) else "failed",
            check_count=len(checks),
            passed_check_count=passed,
            failed_check_count=len(checks) - passed,
            fixture_call_count=len(fixture.calls),
        )
        return result, tuple(checks)


def _validate_inputs(providers: Sequence[str] | None, model: str) -> tuple[str, ...]:
    selected = PROVIDER_CONFORMANCE_PROVIDERS if providers is None else providers
    if isinstance(selected, (str, bytes)) or not isinstance(selected, Sequence):
        raise ProviderError("provider conformance providers must be a sequence")
    selected_tuple = tuple(selected)
    if not selected_tuple or len(selected_tuple) > MAX_PROVIDER_CONFORMANCE_PROVIDERS:
        raise ProviderError("provider conformance provider count is outside its bounds")
    if len(set(selected_tuple)) != len(selected_tuple):
        raise ProviderError("provider conformance providers must be unique")
    if any(provider not in PROVIDER_CONFORMANCE_PROVIDERS for provider in selected_tuple):
        raise ProviderError("provider conformance provider is not a built-in adapter")
    if (
        not isinstance(model, str)
        or not model.strip()
        or len(model.encode("utf-8")) > 512
        or any(ord(character) < 32 for character in model)
    ):
        raise ProviderError("provider conformance model is not bounded")
    return selected_tuple


def run_provider_protocol_conformance(
    *,
    providers: Sequence[str] | None = None,
    model: str = _DEFAULT_MODEL,
) -> ProviderProtocolConformanceReport:
    """Run the all-provider, keyless protocol gate against a local loopback fixture."""

    selected = _validate_inputs(providers, model)
    provider_results: list[ProviderConformanceProviderResult] = []
    checks: list[ProviderConformanceCheck] = []
    for provider in selected:
        result, provider_checks = _run_provider(provider, model)
        provider_results.append(result)
        checks.extend(provider_checks)

    passed_providers = sum(result.status == "passed" for result in provider_results)
    passed_checks = sum(check.status == "passed" for check in checks)
    report_body: dict[str, Any] = {
        "schema": PROVIDER_PROTOCOL_CONFORMANCE_SCHEMA,
        "mode": PROVIDER_PROTOCOL_CONFORMANCE_MODE,
        "status": "passed" if passed_providers == len(provider_results) else "failed",
        "provider_count": len(provider_results),
        "passed_provider_count": passed_providers,
        "failed_provider_count": len(provider_results) - passed_providers,
        "check_count": len(checks),
        "passed_check_count": passed_checks,
        "failed_check_count": len(checks) - passed_checks,
        "providers": [result.to_dict() for result in provider_results],
        "checks": [check.to_dict() for check in checks],
        "transport": "local_loopback_fixture_never_external",
        "retention": "metadata_only;request_response_and_credentials_not_retained",
        "secret_material": "none;synthetic_fixture_credential_not_serialized",
    }
    return ProviderProtocolConformanceReport(
        schema=report_body["schema"],
        mode=report_body["mode"],
        status=report_body["status"],
        provider_count=report_body["provider_count"],
        passed_provider_count=report_body["passed_provider_count"],
        failed_provider_count=report_body["failed_provider_count"],
        check_count=report_body["check_count"],
        passed_check_count=report_body["passed_check_count"],
        failed_check_count=report_body["failed_check_count"],
        providers=tuple(provider_results),
        checks=tuple(checks),
        transport=report_body["transport"],
        retention=report_body["retention"],
        secret_material=report_body["secret_material"],
        report_digest=content_digest(report_body),
    )


def assert_provider_protocol_conformance(
    report: ProviderProtocolConformanceReport | Mapping[str, Any],
) -> None:
    """Raise a bounded error unless a report is internally consistent and fully green."""

    payload = report.to_dict() if isinstance(report, ProviderProtocolConformanceReport) else dict(report)
    if payload.get("schema") != PROVIDER_PROTOCOL_CONFORMANCE_SCHEMA:
        raise ProviderError("provider conformance report schema is invalid")
    if payload.get("mode") != PROVIDER_PROTOCOL_CONFORMANCE_MODE:
        raise ProviderError("provider conformance report mode is invalid")
    if payload.get("status") != "passed":
        raise ProviderError("provider conformance report is not passing")
    providers = payload.get("providers")
    checks = payload.get("checks")
    if not isinstance(providers, list) or not isinstance(checks, list):
        raise ProviderError("provider conformance report collections are invalid")
    if payload.get("provider_count") != len(providers) or payload.get("check_count") != len(checks):
        raise ProviderError("provider conformance report counters are inconsistent")
    if payload.get("failed_provider_count") != 0 or payload.get("failed_check_count") != 0:
        raise ProviderError("provider conformance report contains failures")
    if any(not isinstance(item, Mapping) or item.get("status") != "passed" for item in providers + checks):
        raise ProviderError("provider conformance report contains a non-passing row")
    digest = payload.get("report_digest")
    if not isinstance(digest, str) or not digest:
        raise ProviderError("provider conformance report digest is missing")
    digest_body = dict(payload)
    digest_body.pop("report_digest", None)
    if content_digest(digest_body) != digest:
        raise ProviderError("provider conformance report digest does not match")


__all__ = [
    "MAX_PROVIDER_CONFORMANCE_CHECKS",
    "MAX_PROVIDER_CONFORMANCE_PROVIDERS",
    "PROVIDER_CONFORMANCE_CHECK_NAMES",
    "PROVIDER_CONFORMANCE_PROVIDERS",
    "PROVIDER_PROTOCOL_CONFORMANCE_MODE",
    "PROVIDER_PROTOCOL_CONFORMANCE_SCHEMA",
    "ProviderConformanceCheck",
    "ProviderConformanceProviderResult",
    "ProviderProtocolConformanceReport",
    "assert_provider_protocol_conformance",
    "run_provider_protocol_conformance",
]
