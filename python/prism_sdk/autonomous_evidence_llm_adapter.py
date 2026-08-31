"""Provider-backed evidence acquisition for the reviewed autonomous brain.

The evidence runtime intentionally knows nothing about providers.  This module supplies the
missing application boundary: it converts one reviewed evidence requirement into a bounded
provider request, invokes the existing :class:`LLMRuntime`, and returns a JSON-safe transient
value for the runtime's caller-owned projector.

The adapter never places credentials, prompt payloads, or provider responses in a durable
projection.  It also has no implicit provider, model, or credential fallback.  An embedding
application must register a provider and explicitly supply either a model or a model resolver.
The router makes domain coverage explicit when one evidence run spans multiple built-in domains.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Callable, Mapping, Sequence

from .authoring import content_digest
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError
from .llm_runtime import (
    CredentialHandle,
    LLMRuntime,
    ProviderInvocationObserver,
    ProviderRequest,
    ProviderResponse,
)
from .autonomous_prompt_registry import (
    AutonomousPromptRegistry,
    AutonomousPromptSelectionPlan,
    AutonomousPromptTemplate,
)


AUTONOMOUS_LLM_EVIDENCE_ADAPTER_SCHEMA = "bioprism-python-autonomous-llm-evidence-adapter/0.1"
MAX_AUTONOMOUS_LLM_EVIDENCE_PROMPT_MESSAGES = 64
MAX_AUTONOMOUS_LLM_EVIDENCE_OUTPUT_TOKENS = 32_000
MAX_AUTONOMOUS_LLM_EVIDENCE_MODEL_BYTES = 256
MAX_AUTONOMOUS_LLM_EVIDENCE_ADAPTER_TEXT_BYTES = 512
MAX_AUTONOMOUS_LLM_EVIDENCE_RESPONSE_BYTES = 2_000_000

_SECRET_FIELD_MARKERS = frozenset(
    {
        "apikey",
        "authorization",
        "bearer",
        "credential",
        "credentials",
        "password",
        "secret",
        "secretkey",
        "token",
        "accesstoken",
        "refreshtoken",
        "privatekey",
    }
)

PromptForContext = Callable[[Mapping[str, Any]], Sequence[Mapping[str, Any]]]
ModelForContext = Callable[[Mapping[str, Any]], str]
CredentialForContext = Callable[[str, Mapping[str, Any]], CredentialHandle | None]
ParseResponse = Callable[[ProviderResponse, Mapping[str, Any]], Any]
ProjectValue = Callable[[Any, Mapping[str, Any]], Sequence[Mapping[str, Any]]]


def _text(name: str, value: Any, maximum: int) -> str:
    if (
        not isinstance(value, str)
        or not value.strip()
        or "\x00" in value
        or len(value.encode("utf-8")) > maximum
    ):
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value.strip()


def _identifier(name: str, value: Any, maximum: int = MAX_AUTONOMOUS_LLM_EVIDENCE_ADAPTER_TEXT_BYTES) -> str:
    result = _text(name, value, maximum)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-+ /" for character in result):
        raise ArgumentError(f"{name} contains unsupported identifier characters")
    return result


def _positive_integer(name: str, value: Any, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
        raise ArgumentError(f"{name} must be an integer between 1 and {maximum}")
    return value


def _safe_json(value: Any, name: str, *, depth: int = 0) -> Any:
    """Validate a provider-derived value without retaining credential-shaped fields."""

    if depth > 32:
        raise ArgumentError(f"{name} is too deeply nested")
    if value is None or isinstance(value, (str, bool, int)):
        return value
    if isinstance(value, float):
        if value != value or value in {float("inf"), float("-inf")}:
            raise ArgumentError(f"{name} contains a non-finite number")
        return value
    if isinstance(value, Mapping):
        result: dict[str, Any] = {}
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip() or "\x00" in key:
                raise ArgumentError(f"{name} contains an invalid object field")
            normalized = "".join(character for character in key.lower() if character.isalnum())
            if normalized in _SECRET_FIELD_MARKERS or any(
                marker in normalized for marker in ("token", "secret", "credential")
            ):
                raise ArgumentError(f"{name} contains credential-shaped response fields")
            result[key] = _safe_json(child, f"{name}.{key}", depth=depth + 1)
        try:
            encoded = json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"{name} must be JSON-safe") from error
        if len(encoded.encode("utf-8")) > MAX_AUTONOMOUS_LLM_EVIDENCE_RESPONSE_BYTES:
            raise ArgumentError(f"{name} exceeds its response byte bound")
        return result
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        result = [_safe_json(item, f"{name}[{index}]", depth=depth + 1) for index, item in enumerate(value)]
        try:
            encoded = json.dumps(result, ensure_ascii=False, separators=(",", ":"), allow_nan=False)
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"{name} must be JSON-safe") from error
        if len(encoded.encode("utf-8")) > MAX_AUTONOMOUS_LLM_EVIDENCE_RESPONSE_BYTES:
            raise ArgumentError(f"{name} exceeds its response byte bound")
        return result
    raise ArgumentError(f"{name} must be JSON-safe")


def _field(value: Any, name: str, default: Any = None) -> Any:
    if isinstance(value, Mapping):
        return value.get(name, default)
    return getattr(value, name, default)


def _request_identity(context: Mapping[str, Any], *, rendered_prompt_digest: str | None = None) -> str:
    request = context.get("request")
    if not isinstance(request, Mapping):
        raise ArgumentError("LLM evidence adapter context request is malformed")
    requirement = context.get("requirement")
    identity: dict[str, Any] = {
            "schema": AUTONOMOUS_LLM_EVIDENCE_ADAPTER_SCHEMA,
            "plan_digest": context.get("plan_digest"),
            "requirement_id": _field(requirement, "requirement_id"),
            "source_id": request.get("source_id"),
            "source_digest": request.get("source_digest"),
            "request_id": request.get("request_id"),
            "metadata": request.get("metadata", {}),
        }
    if rendered_prompt_digest is not None:
        identity["rendered_prompt_digest"] = rendered_prompt_digest
    return content_digest(identity)


def _default_parse_response(response: ProviderResponse, _context: Mapping[str, Any]) -> Any:
    return response.structured if response.structured is not None else response.text


@dataclass(frozen=True, slots=True)
class AutonomousLLMEvidenceAdapter:
    """One explicitly configured LLM evidence source for one autonomous domain."""

    adapter_id: str
    version: str
    domain: str
    provider: str
    runtime: LLMRuntime
    capabilities: tuple[str, ...]
    prompt_for_context: PromptForContext | None = None
    model: str | None = None
    model_for_context: ModelForContext | None = None
    source_kinds: tuple[str, ...] = ("llm_structured",)
    credential: CredentialHandle | None = None
    credential_for: CredentialForContext | None = None
    parse_response: ParseResponse | None = None
    project: ProjectValue | None = None
    max_output_tokens: int = 1_024
    temperature: float | None = None
    require_json: bool = False
    response_schema: Mapping[str, Any] | None = None
    invocation_observer: ProviderInvocationObserver | None = None
    invocation_kind: str = "autonomous_evidence_acquisition"
    prompt_template: AutonomousPromptTemplate | None = None
    prompt_registry: AutonomousPromptRegistry | None = None
    prompt_selection: AutonomousPromptSelectionPlan | Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        object.__setattr__(self, "adapter_id", _identifier("LLM evidence adapter adapter_id", self.adapter_id))
        object.__setattr__(self, "version", _identifier("LLM evidence adapter version", self.version))
        object.__setattr__(self, "domain", _identifier("LLM evidence adapter domain", self.domain))
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError(f"LLM evidence adapter domain is not supported: {self.domain}")
        object.__setattr__(self, "provider", _identifier("LLM evidence adapter provider", self.provider))
        if not isinstance(self.runtime, LLMRuntime):
            raise ArgumentError("LLM evidence adapter requires an LLMRuntime")
        if self.prompt_for_context is not None and not callable(self.prompt_for_context):
            raise ArgumentError("LLM evidence adapter prompt_for_context is malformed")
        if self.prompt_template is not None and not isinstance(self.prompt_template, AutonomousPromptTemplate):
            raise ArgumentError("LLM evidence adapter prompt_template is malformed")
        if self.prompt_template is not None and self.prompt_template.manifest.domain != self.domain:
            raise ArgumentError("LLM evidence adapter prompt_template domain does not match adapter domain")
        if self.prompt_registry is not None and not isinstance(self.prompt_registry, AutonomousPromptRegistry):
            raise ArgumentError("LLM evidence adapter prompt_registry is malformed")
        if self.prompt_selection is not None and not isinstance(self.prompt_selection, (AutonomousPromptSelectionPlan, Mapping)):
            raise ArgumentError("LLM evidence adapter prompt_selection is malformed")
        if self.prompt_template is not None and (self.prompt_registry is not None or self.prompt_selection is not None):
            raise ArgumentError("LLM evidence adapter cannot combine prompt_template with prompt registry selection")
        if (self.prompt_registry is None) != (self.prompt_selection is None):
            raise ArgumentError("LLM evidence adapter prompt_registry and prompt_selection must be supplied together")
        if self.prompt_for_context is not None and (self.prompt_template is not None or self.prompt_registry is not None):
            raise ArgumentError("LLM evidence adapter cannot combine prompt_for_context with a prompt registry template")
        if self.prompt_for_context is None and self.prompt_template is None and self.prompt_registry is None:
            raise ArgumentError("LLM evidence adapter requires prompt_for_context or a verified prompt template selection")
        if self.prompt_registry is not None:
            object.__setattr__(self, "prompt_selection", self.prompt_registry.verify_selection(self.prompt_selection))
        if self.model is None and self.model_for_context is None:
            raise ArgumentError("LLM evidence adapter requires model or model_for_context")
        if self.model is not None and self.model_for_context is not None:
            raise ArgumentError("LLM evidence adapter cannot configure both model and model_for_context")
        if self.model is not None:
            object.__setattr__(self, "model", _text("LLM evidence adapter model", self.model, MAX_AUTONOMOUS_LLM_EVIDENCE_MODEL_BYTES))
        if self.model_for_context is not None and not callable(self.model_for_context):
            raise ArgumentError("LLM evidence adapter model_for_context is malformed")
        if self.credential is not None and self.credential_for is not None:
            raise ArgumentError("LLM evidence adapter cannot configure both credential and credential_for")
        if self.credential is not None and not isinstance(self.credential, CredentialHandle):
            raise ArgumentError("LLM evidence adapter credential is malformed")
        if self.credential_for is not None and not callable(self.credential_for):
            raise ArgumentError("LLM evidence adapter credential_for is malformed")
        if self.parse_response is not None and not callable(self.parse_response):
            raise ArgumentError("LLM evidence adapter parse_response is malformed")
        if self.project is not None and not callable(self.project):
            raise ArgumentError("LLM evidence adapter project is malformed")
        if isinstance(self.capabilities, (str, bytes)) or not isinstance(self.capabilities, Sequence) or not self.capabilities:
            raise ArgumentError("LLM evidence adapter capabilities must be a non-empty sequence")
        capabilities = tuple(_identifier("LLM evidence adapter capability", item) for item in self.capabilities)
        if len(set(capabilities)) != len(capabilities):
            raise ArgumentError("LLM evidence adapter capabilities must be unique")
        object.__setattr__(self, "capabilities", capabilities)
        if isinstance(self.source_kinds, (str, bytes)) or not isinstance(self.source_kinds, Sequence) or not self.source_kinds:
            raise ArgumentError("LLM evidence adapter source_kinds must be a non-empty sequence")
        source_kinds = tuple(_identifier("LLM evidence adapter source kind", item) for item in self.source_kinds)
        if len(set(source_kinds)) != len(source_kinds):
            raise ArgumentError("LLM evidence adapter source_kinds must be unique")
        object.__setattr__(self, "source_kinds", source_kinds)
        object.__setattr__(self, "max_output_tokens", _positive_integer("LLM evidence adapter max_output_tokens", self.max_output_tokens, MAX_AUTONOMOUS_LLM_EVIDENCE_OUTPUT_TOKENS))
        if self.temperature is not None and (
            isinstance(self.temperature, bool)
            or not isinstance(self.temperature, (int, float))
            or not 0 <= float(self.temperature) <= 2
        ):
            raise ArgumentError("LLM evidence adapter temperature must be between 0 and 2")
        if not isinstance(self.require_json, bool):
            raise ArgumentError("LLM evidence adapter require_json must be a boolean")
        if self.response_schema is not None:
            if not self.require_json or not isinstance(self.response_schema, Mapping):
                raise ArgumentError("LLM evidence adapter response_schema requires require_json and a mapping")
            _safe_json(self.response_schema, "LLM evidence adapter response_schema")
            object.__setattr__(self, "response_schema", dict(self.response_schema))
        object.__setattr__(self, "invocation_kind", _text("LLM evidence adapter invocation_kind", self.invocation_kind, 128))

    def _assert_context_domain(self, context: Mapping[str, Any]) -> None:
        requirement = context.get("requirement")
        requirement_domain = _field(requirement, "domain")
        if requirement_domain != self.domain:
            raise ArgumentError("LLM evidence adapter requirement domain does not match its configured domain")

    def acquire(self, context: Mapping[str, Any]) -> Any:
        """Build and invoke one provider request for the reviewed requirement."""

        if not isinstance(context, Mapping):
            raise ArgumentError("LLM evidence adapter context must be a mapping")
        self._assert_context_domain(context)
        model = self.model if self.model is not None else self.model_for_context(context)  # type: ignore[misc]
        model = _text("LLM evidence adapter resolved model", model, MAX_AUTONOMOUS_LLM_EVIDENCE_MODEL_BYTES)
        rendered_prompt = None
        if self.prompt_for_context is not None:
            messages = self.prompt_for_context(context)
        elif self.prompt_template is not None:
            rendered_prompt = self.prompt_template.render_transient(context)
            messages = rendered_prompt.messages
        else:
            rendered_prompt = self.prompt_registry.render(self.prompt_selection, context)  # type: ignore[union-attr]
            messages = rendered_prompt.messages
        if isinstance(messages, (str, bytes, bytearray)) or not isinstance(messages, Sequence) or not 1 <= len(messages) <= MAX_AUTONOMOUS_LLM_EVIDENCE_PROMPT_MESSAGES:
            raise ArgumentError("LLM evidence adapter prompt must contain between 1 and 64 messages")
        normalized_messages: list[Mapping[str, Any]] = []
        for index, message in enumerate(messages):
            if not isinstance(message, Mapping):
                raise ArgumentError(f"LLM evidence adapter prompt message {index} must be a mapping")
            try:
                json.dumps(message, ensure_ascii=False, allow_nan=False)
            except (TypeError, ValueError) as error:
                raise ArgumentError(f"LLM evidence adapter prompt message {index} must be JSON-safe") from error
            normalized_messages.append(dict(message))
        request = ProviderRequest(
            model=model,
            messages=tuple(normalized_messages),
            max_output_tokens=self.max_output_tokens,
            temperature=self.temperature,
            require_json=self.require_json,
            response_schema=self.response_schema,
            idempotency_key=_request_identity(
                context,
                rendered_prompt_digest=None if rendered_prompt is None else rendered_prompt.rendered_prompt_digest,
            ),
        )
        selected_credential = self.credential
        if self.credential_for is not None:
            selected_credential = self.credential_for(self.provider, context)
            if selected_credential is not None and not isinstance(selected_credential, CredentialHandle):
                raise ArgumentError("LLM evidence adapter credential_for returned a malformed handle")
        response = self.runtime.invoke(
            self.provider,
            request,
            credential=selected_credential,
            invocation_observer=self.invocation_observer,
            invocation_kind=self.invocation_kind,
        )
        parser = self.parse_response or _default_parse_response
        try:
            parsed = parser(response, context)
        except ArgumentError:
            raise
        except Exception as error:
            raise ArgumentError("LLM evidence adapter response parser failed") from error
        return _safe_json(parsed, "LLM evidence adapter parsed response")

    def project_value(self, value: Any, context: Mapping[str, Any]) -> Sequence[Mapping[str, Any]]:
        """Project a provider value into runtime observations without retaining the value."""

        if self.project is None:
            return ()
        result = self.project(value, context)
        if isinstance(result, (str, bytes, bytearray)) or not isinstance(result, Sequence):
            raise ArgumentError("LLM evidence adapter project must return a sequence")
        projected: list[Mapping[str, Any]] = []
        for index, item in enumerate(result):
            if not isinstance(item, Mapping):
                raise ArgumentError(f"LLM evidence adapter projection {index} must be a mapping")
            projected.append(_safe_json(dict(item), f"LLM evidence adapter projection {index}"))
        return tuple(projected)

    def to_dict(self) -> dict[str, Any]:
        prompt_metadata: dict[str, Any]
        if self.prompt_for_context is not None:
            prompt_metadata = {"mode": "caller_callback"}
        elif self.prompt_template is not None:
            prompt_metadata = {
                "mode": "versioned_template",
                "prompt_id": self.prompt_template.manifest.prompt_id,
                "prompt_version": self.prompt_template.manifest.version,
                "prompt_manifest_digest": self.prompt_template.manifest.manifest_digest,
            }
        else:
            prompt_metadata = {
                "mode": "registry_selection",
                "registry_digest": self.prompt_registry.registry_digest,  # type: ignore[union-attr]
                "selection_plan_digest": self.prompt_selection.plan_digest if isinstance(self.prompt_selection, AutonomousPromptSelectionPlan) else None,
            }
        return {
            "schema": AUTONOMOUS_LLM_EVIDENCE_ADAPTER_SCHEMA,
            "adapter_id": self.adapter_id,
            "version": self.version,
            "domain": self.domain,
            "provider": self.provider,
            "capabilities": list(self.capabilities),
            "source_kinds": list(self.source_kinds),
            "model": self.model,
            "model_resolution": "context_resolver" if self.model_for_context is not None else "static",
            "structured_output": {
                "required": self.require_json,
                "response_schema": self.response_schema is not None,
            },
            "credential_posture": "caller_owned_opaque_handle" if self.credential is not None or self.credential_for is not None else "provider_must_be_credentialless_or_fail_closed",
            "projector_configured": self.project is not None,
            "prompt": prompt_metadata,
            "retention": "provider_messages_and_responses_transient;metadata_only_projection",
            "secret_material": "never_returned",
        }


class AutonomousLLMEvidenceAdapterRouter:
    """Route evidence requests to explicit per-domain LLM adapters."""

    def __init__(self, adapters: Mapping[str, AutonomousLLMEvidenceAdapter], *, require_all_domains: bool = False) -> None:
        if not isinstance(adapters, Mapping) or not adapters:
            raise ArgumentError("LLM evidence adapter router requires at least one adapter")
        if not isinstance(require_all_domains, bool):
            raise ArgumentError("LLM evidence adapter router require_all_domains must be a boolean")
        normalized: dict[str, AutonomousLLMEvidenceAdapter] = {}
        for domain, adapter in adapters.items():
            domain_name = _identifier("LLM evidence adapter router domain", domain)
            if domain_name not in AUTONOMOUS_DOMAIN_NAMES:
                raise ArgumentError(f"LLM evidence adapter router domain is not supported: {domain_name}")
            if not isinstance(adapter, AutonomousLLMEvidenceAdapter) or adapter.domain != domain_name:
                raise ArgumentError("LLM evidence adapter router entries must match their adapter domain")
            normalized[domain_name] = adapter
        if require_all_domains and set(normalized) != set(AUTONOMOUS_DOMAIN_NAMES):
            raise ArgumentError("LLM evidence adapter router does not cover every autonomous domain")
        self._adapters = dict(sorted(normalized.items()))
        self.require_all_domains = require_all_domains

    @property
    def domains(self) -> tuple[str, ...]:
        return tuple(self._adapters)

    def adapter_for(self, context: Mapping[str, Any]) -> AutonomousLLMEvidenceAdapter:
        if not isinstance(context, Mapping):
            raise ArgumentError("LLM evidence adapter router context must be a mapping")
        domain = _field(context.get("requirement"), "domain")
        if domain not in self._adapters:
            raise ArgumentError(f"LLM evidence adapter router has no adapter for domain: {domain}")
        return self._adapters[domain]

    def acquire(self, context: Mapping[str, Any]) -> Any:
        return self.adapter_for(context).acquire(context)

    def project(self, value: Any, context: Mapping[str, Any]) -> Sequence[Mapping[str, Any]]:
        return self.adapter_for(context).project_value(value, context)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_LLM_EVIDENCE_ADAPTER_SCHEMA,
            "router": True,
            "domains": list(self.domains),
            "require_all_domains": self.require_all_domains,
            "adapters": [self._adapters[domain].to_dict() for domain in self.domains],
            "retention": "provider_messages_and_responses_transient;metadata_only_projection",
            "secret_material": "never_returned",
        }


def create_autonomous_llm_evidence_adapter(**kwargs: Any) -> AutonomousLLMEvidenceAdapter:
    """Construct a validated provider-backed evidence adapter from application-owned options."""

    return AutonomousLLMEvidenceAdapter(**kwargs)


def create_autonomous_llm_evidence_adapter_router(
    adapters: Mapping[str, AutonomousLLMEvidenceAdapter],
    *,
    require_all_domains: bool = False,
) -> AutonomousLLMEvidenceAdapterRouter:
    """Construct an explicit domain router; no provider or model is selected implicitly."""

    return AutonomousLLMEvidenceAdapterRouter(adapters, require_all_domains=require_all_domains)
