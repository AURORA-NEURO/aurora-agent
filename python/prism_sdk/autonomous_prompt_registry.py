"""Digest-bound prompt selection and transient rendering for the autonomous brain.

Prompt callbacks used to be the last untyped seam in the provider path.  That made it
possible for a caller to select a model from one reviewed plan while silently changing the
prompt implementation at invocation time.  This module gives prompts the same control-plane
properties as models, adapters, and provider contracts:

* templates are explicitly scoped to a built-in domain, stage, and capability set;
* selection plans carry the registry digest and selected manifest digests;
* a changed, removed, or tampered template fails closed before provider dispatch; and
* rendered messages and context remain transient while only bounded digests and metadata are
  serializable.

The registry does not store task text, credentials, provider responses, or renderer output.
Renderers remain caller-owned functions and are invoked only after a selection plan has been
verified against the current registry.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import json
from typing import Any, Callable, Mapping, Sequence

from .authoring import content_digest
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_PROMPT_REGISTRY_SCHEMA = "bioprism-python-autonomous-prompt-registry/0.1"
AUTONOMOUS_PROMPT_MANIFEST_SCHEMA = "bioprism-python-autonomous-prompt-manifest/0.1"
AUTONOMOUS_PROMPT_SELECTION_SCHEMA = "bioprism-python-autonomous-prompt-selection/0.1"
AUTONOMOUS_PROMPT_SELECTION_ROW_SCHEMA = "bioprism-python-autonomous-prompt-selection-row/0.1"
AUTONOMOUS_PROMPT_RENDER_SCHEMA = "bioprism-python-autonomous-prompt-render/0.1"
AUTONOMOUS_PROMPT_SELECTION_POLICY = "deterministic_specificity_v1"
MAX_AUTONOMOUS_PROMPT_TEMPLATES = 1_024
MAX_AUTONOMOUS_PROMPT_CAPABILITIES = 64
MAX_AUTONOMOUS_PROMPT_STAGES = 64
MAX_AUTONOMOUS_PROMPT_SELECTIONS = 128
MAX_AUTONOMOUS_PROMPT_MESSAGES = 64
MAX_AUTONOMOUS_PROMPT_BYTES = 1_000_000
MAX_AUTONOMOUS_PROMPT_IDENTIFIER_BYTES = 256
MAX_AUTONOMOUS_PROMPT_VERSION_BYTES = 128

PromptRenderer = Callable[[Mapping[str, Any]], Sequence[Mapping[str, Any]]]
_PROMPT_ROLES = frozenset({"system", "developer", "user", "assistant", "tool"})
_SAFE_IDENTIFIER_CHARS = frozenset(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-+ /"
)
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
        "clientsecret",
    }
)


def _text(name: str, value: Any, maximum: int) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds its bounded size")
    return value.strip()


def _identifier(name: str, value: Any) -> str:
    result = _text(name, value, MAX_AUTONOMOUS_PROMPT_IDENTIFIER_BYTES)
    if any(character not in _SAFE_IDENTIFIER_CHARS for character in result):
        raise ArgumentError(f"{name} contains unsupported identifier characters")
    return result


def _digest(name: str, value: Any, *, optional: bool = False) -> str | None:
    if optional and value is None:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _items(name: str, value: Any, *, maximum: int, allow_wildcard: bool = False, allow_empty: bool = False) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be a sequence")
    if (not allow_empty and not 1 <= len(value)) or len(value) > maximum:
        raise ArgumentError(f"{name} must contain between 1 and {maximum} entries")
    result: list[str] = []
    for item in value:
        item_text = _identifier(f"{name} entry", item)
        if item_text == "*" and not allow_wildcard:
            raise ArgumentError(f"{name} does not allow wildcard entries")
        result.append(item_text)
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} contains duplicate entries")
    return tuple(result)


def _positive_integer(name: str, value: Any, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
        raise ArgumentError(f"{name} must be an integer between 1 and {maximum}")
    return value


def _secret_safe_json(value: Any, name: str, *, depth: int = 0) -> Any:
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
            if normalized in _SECRET_FIELD_MARKERS or any(marker in normalized for marker in ("token", "secret", "credential")):
                raise ArgumentError(f"{name} contains credential-shaped fields")
            result[key] = _secret_safe_json(child, f"{name}.{key}", depth=depth + 1)
        return result
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        return [_secret_safe_json(item, f"{name}[{index}]", depth=depth + 1) for index, item in enumerate(value)]
    raise ArgumentError(f"{name} must be JSON-safe")


def _context_domain_stage(context: Mapping[str, Any]) -> tuple[str, str]:
    requirement = context.get("requirement")
    if isinstance(requirement, Mapping):
        domain = requirement.get("domain")
        stage = requirement.get("stage_id")
    else:
        domain = context.get("domain")
        stage = context.get("stage_id")
    domain = _identifier("prompt context domain", domain)
    stage = _identifier("prompt context stage_id", stage)
    if domain not in AUTONOMOUS_DOMAIN_NAMES:
        raise ArgumentError(f"prompt context domain is unsupported: {domain}")
    return domain, stage


@dataclass(frozen=True, slots=True)
class AutonomousPromptManifest:
    """Serializable, renderer-free identity for one prompt implementation."""

    prompt_id: str
    version: str
    domain: str
    capabilities: tuple[str, ...]
    stages: tuple[str, ...]
    template_digest: str
    output_contract_digest: str | None = None
    max_messages: int = MAX_AUTONOMOUS_PROMPT_MESSAGES
    max_prompt_bytes: int = MAX_AUTONOMOUS_PROMPT_BYTES

    def __post_init__(self) -> None:
        object.__setattr__(self, "prompt_id", _identifier("prompt manifest prompt_id", self.prompt_id))
        object.__setattr__(self, "version", _text("prompt manifest version", self.version, MAX_AUTONOMOUS_PROMPT_VERSION_BYTES))
        object.__setattr__(self, "domain", _identifier("prompt manifest domain", self.domain))
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError(f"prompt manifest domain is unsupported: {self.domain}")
        object.__setattr__(self, "capabilities", _items("prompt manifest capabilities", self.capabilities, maximum=MAX_AUTONOMOUS_PROMPT_CAPABILITIES))
        object.__setattr__(self, "stages", _items("prompt manifest stages", self.stages, maximum=MAX_AUTONOMOUS_PROMPT_STAGES, allow_wildcard=True))
        object.__setattr__(self, "template_digest", _digest("prompt manifest template_digest", self.template_digest))
        object.__setattr__(self, "output_contract_digest", _digest("prompt manifest output_contract_digest", self.output_contract_digest, optional=True))
        object.__setattr__(self, "max_messages", _positive_integer("prompt manifest max_messages", self.max_messages, MAX_AUTONOMOUS_PROMPT_MESSAGES))
        object.__setattr__(self, "max_prompt_bytes", _positive_integer("prompt manifest max_prompt_bytes", self.max_prompt_bytes, MAX_AUTONOMOUS_PROMPT_BYTES))

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_PROMPT_MANIFEST_SCHEMA,
            "prompt_id": self.prompt_id,
            "version": self.version,
            "domain": self.domain,
            "capabilities": list(self.capabilities),
            "stages": list(self.stages),
            "template_digest": self.template_digest,
            "output_contract_digest": self.output_contract_digest,
            "max_messages": self.max_messages,
            "max_prompt_bytes": self.max_prompt_bytes,
            "retention": "renderer_and_rendered_messages_transient;manifest_metadata_only",
            "secret_material": "never_returned",
        }

    @property
    def manifest_digest(self) -> str:
        return content_digest(self.to_dict())


@dataclass(frozen=True, slots=True)
class AutonomousPromptRenderResult:
    """Transient rendered messages plus value-only identity metadata."""

    prompt_id: str
    version: str
    domain: str
    stage: str
    manifest_digest: str
    rendered_prompt_digest: str
    messages: tuple[Mapping[str, Any], ...] = field(repr=False)
    selection_plan_digest: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_PROMPT_RENDER_SCHEMA,
            "prompt_id": self.prompt_id,
            "version": self.version,
            "domain": self.domain,
            "stage": self.stage,
            "manifest_digest": self.manifest_digest,
            "rendered_prompt_digest": self.rendered_prompt_digest,
            "selection_plan_digest": self.selection_plan_digest,
            "message_count": len(self.messages),
            "retention": "rendered_messages_transient;digest_only_projection",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousPromptTemplate:
    """Caller-owned prompt renderer bound to a reviewed prompt manifest."""

    prompt_id: str
    version: str
    domain: str
    capabilities: tuple[str, ...]
    stages: tuple[str, ...]
    template_digest: str
    render: PromptRenderer
    output_contract_digest: str | None = None
    max_messages: int = MAX_AUTONOMOUS_PROMPT_MESSAGES
    max_prompt_bytes: int = MAX_AUTONOMOUS_PROMPT_BYTES

    def __post_init__(self) -> None:
        if not callable(self.render):
            raise ArgumentError("prompt template render must be callable")
        self.manifest  # force validation through the property

    @property
    def manifest(self) -> AutonomousPromptManifest:
        return AutonomousPromptManifest(
            prompt_id=self.prompt_id,
            version=self.version,
            domain=self.domain,
            capabilities=self.capabilities,
            stages=self.stages,
            template_digest=self.template_digest,
            output_contract_digest=self.output_contract_digest,
            max_messages=self.max_messages,
            max_prompt_bytes=self.max_prompt_bytes,
        )

    def render_transient(self, context: Mapping[str, Any], *, selection_plan_digest: str | None = None) -> AutonomousPromptRenderResult:
        if not isinstance(context, Mapping):
            raise ArgumentError("prompt render context must be a mapping")
        domain, stage = _context_domain_stage(context)
        manifest = self.manifest
        if domain != manifest.domain:
            raise ArgumentError("prompt template domain does not match render context")
        if stage not in manifest.stages and "*" not in manifest.stages:
            raise ArgumentError("prompt template does not cover render context stage")
        try:
            messages = self.render(context)
        except ArgumentError:
            raise
        except Exception as error:
            raise ArgumentError("prompt template renderer failed") from error
        if isinstance(messages, (str, bytes, bytearray)) or not isinstance(messages, Sequence):
            raise ArgumentError("prompt renderer must return a message sequence")
        if not 1 <= len(messages) <= manifest.max_messages:
            raise ArgumentError("prompt renderer returned an unsupported message count")
        normalized: list[Mapping[str, Any]] = []
        for index, message in enumerate(messages):
            if not isinstance(message, Mapping):
                raise ArgumentError(f"prompt message {index} must be a mapping")
            role = message.get("role")
            if role not in _PROMPT_ROLES:
                raise ArgumentError(f"prompt message {index} has an unsupported role")
            if "content" not in message:
                raise ArgumentError(f"prompt message {index} is missing content")
            safe = _secret_safe_json(dict(message), f"prompt message {index}")
            normalized.append(safe)
        try:
            encoded = json.dumps(normalized, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
        except (TypeError, ValueError) as error:
            raise ArgumentError("rendered prompt must be JSON-safe") from error
        if len(encoded.encode("utf-8")) > manifest.max_prompt_bytes:
            raise ArgumentError("rendered prompt exceeds its bounded size")
        return AutonomousPromptRenderResult(
            prompt_id=manifest.prompt_id,
            version=manifest.version,
            domain=manifest.domain,
            stage=stage,
            manifest_digest=manifest.manifest_digest,
            rendered_prompt_digest=content_digest(normalized),
            messages=tuple(normalized),
            selection_plan_digest=selection_plan_digest,
        )


@dataclass(frozen=True, slots=True)
class AutonomousPromptSelectionRow:
    domain: str
    stage: str
    required_capabilities: tuple[str, ...]
    selected_prompt_id: str
    selected_version: str
    selected_manifest_digest: str
    candidate_prompt_ids: tuple[str, ...]
    selection_reason: str = "stage_specificity_then_capability_fit_then_lexical_identity"

    def __post_init__(self) -> None:
        object.__setattr__(self, "domain", _identifier("prompt selection row domain", self.domain))
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("prompt selection row domain is unsupported")
        object.__setattr__(self, "stage", _identifier("prompt selection row stage", self.stage))
        object.__setattr__(self, "required_capabilities", _items("prompt selection row required_capabilities", self.required_capabilities, maximum=MAX_AUTONOMOUS_PROMPT_CAPABILITIES, allow_empty=True))
        object.__setattr__(self, "selected_prompt_id", _identifier("prompt selection row selected_prompt_id", self.selected_prompt_id))
        object.__setattr__(self, "selected_version", _text("prompt selection row selected_version", self.selected_version, MAX_AUTONOMOUS_PROMPT_VERSION_BYTES))
        object.__setattr__(self, "selected_manifest_digest", _digest("prompt selection row selected_manifest_digest", self.selected_manifest_digest))
        object.__setattr__(self, "candidate_prompt_ids", _items("prompt selection row candidate_prompt_ids", self.candidate_prompt_ids, maximum=MAX_AUTONOMOUS_PROMPT_TEMPLATES))
        object.__setattr__(self, "selection_reason", _text("prompt selection row selection_reason", self.selection_reason, 512))

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_PROMPT_SELECTION_ROW_SCHEMA,
            "domain": self.domain,
            "stage": self.stage,
            "required_capabilities": list(self.required_capabilities),
            "selected_prompt_id": self.selected_prompt_id,
            "selected_version": self.selected_version,
            "selected_manifest_digest": self.selected_manifest_digest,
            "candidate_prompt_ids": list(self.candidate_prompt_ids),
            "selection_reason": self.selection_reason,
        }


@dataclass(frozen=True, slots=True)
class AutonomousPromptSelectionPlan:
    registry_digest: str
    rows: tuple[AutonomousPromptSelectionRow, ...]
    selection_policy: str = AUTONOMOUS_PROMPT_SELECTION_POLICY

    def __post_init__(self) -> None:
        object.__setattr__(self, "registry_digest", _digest("prompt selection plan registry_digest", self.registry_digest))
        if not isinstance(self.rows, Sequence) or isinstance(self.rows, (str, bytes)) or not 1 <= len(self.rows) <= MAX_AUTONOMOUS_PROMPT_SELECTIONS:
            raise ArgumentError("prompt selection plan rows are outside their bounds")
        if any(not isinstance(row, AutonomousPromptSelectionRow) for row in self.rows):
            raise ArgumentError("prompt selection plan rows are malformed")
        keys = [(row.domain, row.stage, row.required_capabilities) for row in self.rows]
        if len(set(keys)) != len(keys):
            raise ArgumentError("prompt selection plan rows contain duplicates")
        if self.selection_policy != AUTONOMOUS_PROMPT_SELECTION_POLICY:
            raise ArgumentError("unsupported prompt selection policy")

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_PROMPT_SELECTION_SCHEMA,
            "registry_digest": self.registry_digest,
            "selection_policy": self.selection_policy,
            "rows": [row.to_dict() for row in self.rows],
        }

    @property
    def plan_digest(self) -> str:
        return content_digest(self._descriptor())

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._descriptor(),
            "plan_digest": self.plan_digest,
            "execution": "selection_only;render_and_provider_invocation_remain_transient_caller_boundaries",
            "retention": "registry_and_selection_metadata_only",
            "secret_material": "never_returned",
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousPromptSelectionPlan":
        if not isinstance(value, Mapping):
            raise ArgumentError("prompt selection plan must be a mapping")
        rows_value = value.get("rows")
        if not isinstance(rows_value, Sequence) or isinstance(rows_value, (str, bytes)):
            raise ArgumentError("prompt selection plan rows are malformed")
        rows = tuple(
            AutonomousPromptSelectionRow(
                domain=row["domain"],
                stage=row["stage"],
                required_capabilities=tuple(row["required_capabilities"]),
                selected_prompt_id=row["selected_prompt_id"],
                selected_version=row["selected_version"],
                selected_manifest_digest=row["selected_manifest_digest"],
                candidate_prompt_ids=tuple(row["candidate_prompt_ids"]),
                selection_reason=row.get("selection_reason", "stage_specificity_then_capability_fit_then_lexical_identity"),
            )
            for row in rows_value
            if isinstance(row, Mapping)
        )
        if len(rows) != len(rows_value):
            raise ArgumentError("prompt selection plan contains malformed rows")
        plan = cls(
            registry_digest=value.get("registry_digest"),
            rows=rows,
            selection_policy=value.get("selection_policy", AUTONOMOUS_PROMPT_SELECTION_POLICY),
        )
        supplied_digest = value.get("plan_digest")
        if supplied_digest is not None and supplied_digest != plan.plan_digest:
            raise ArgumentError("prompt selection plan digest does not match its contents")
        return plan


class AutonomousPromptRegistry:
    """Registry and fail-closed selector for versioned autonomous prompt templates."""

    def __init__(self, templates: Sequence[AutonomousPromptTemplate] = ()) -> None:
        self._templates: dict[str, AutonomousPromptTemplate] = {}
        for template in templates:
            self.register(template)

    def register(self, template: AutonomousPromptTemplate, *, replace: bool = False) -> AutonomousPromptManifest:
        if not isinstance(template, AutonomousPromptTemplate):
            raise ArgumentError("prompt registry requires an AutonomousPromptTemplate")
        if not isinstance(replace, bool):
            raise ArgumentError("prompt registry replace must be a boolean")
        key = template.manifest.prompt_id
        if key in self._templates and not replace:
            raise ArgumentError(f"prompt registry already contains prompt: {key}")
        if key not in self._templates and len(self._templates) >= MAX_AUTONOMOUS_PROMPT_TEMPLATES:
            raise ArgumentError("prompt registry exceeds its template bound")
        self._templates[key] = template
        return template.manifest

    @property
    def templates(self) -> tuple[AutonomousPromptTemplate, ...]:
        return tuple(self._templates[key] for key in sorted(self._templates))

    @property
    def manifests(self) -> tuple[AutonomousPromptManifest, ...]:
        return tuple(template.manifest for template in self.templates)

    @property
    def registry_digest(self) -> str:
        return content_digest({"schema": AUTONOMOUS_PROMPT_REGISTRY_SCHEMA, "manifests": [manifest.to_dict() for manifest in self.manifests]})

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_PROMPT_REGISTRY_SCHEMA,
            "registry_digest": self.registry_digest,
            "templates": [manifest.to_dict() for manifest in self.manifests],
            "template_count": len(self._templates),
            "retention": "renderer_and_rendered_messages_transient;manifest_metadata_only",
            "secret_material": "never_returned",
        }

    def template_for(self, prompt_id: str) -> AutonomousPromptTemplate:
        prompt_id = _identifier("prompt registry prompt_id", prompt_id)
        try:
            return self._templates[prompt_id]
        except KeyError as error:
            raise ArgumentError(f"prompt registry has no template: {prompt_id}") from error

    def candidates(self, domain: str, stage: str, required_capabilities: Sequence[str] = ()) -> tuple[AutonomousPromptTemplate, ...]:
        domain = _identifier("prompt candidate domain", domain)
        stage = _identifier("prompt candidate stage", stage)
        if domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("prompt candidate domain is unsupported")
        required = _items("prompt candidate required_capabilities", required_capabilities, maximum=MAX_AUTONOMOUS_PROMPT_CAPABILITIES, allow_empty=True)
        result = [
            template
            for template in self.templates
            if template.manifest.domain == domain
            and (stage in template.manifest.stages or "*" in template.manifest.stages)
            and set(required).issubset(template.manifest.capabilities)
        ]
        return tuple(
            sorted(
                result,
                key=lambda template: (
                    0 if stage in template.manifest.stages else 1,
                    len(template.manifest.capabilities) - len(required),
                    template.manifest.prompt_id,
                    template.manifest.version,
                ),
            )
        )

    def select_for(self, requests: Sequence[Mapping[str, Any]]) -> AutonomousPromptSelectionPlan:
        if not isinstance(requests, Sequence) or isinstance(requests, (str, bytes, bytearray)) or not 1 <= len(requests) <= MAX_AUTONOMOUS_PROMPT_SELECTIONS:
            raise ArgumentError("prompt selection requests are outside their bounds")
        rows: list[AutonomousPromptSelectionRow] = []
        for index, request in enumerate(requests):
            if not isinstance(request, Mapping):
                raise ArgumentError(f"prompt selection request {index} is malformed")
            domain = _identifier(f"prompt selection request {index} domain", request.get("domain"))
            stage = _identifier(f"prompt selection request {index} stage", request.get("stage", request.get("stage_id")))
            required = _items(f"prompt selection request {index} required_capabilities", request.get("required_capabilities", ()), maximum=MAX_AUTONOMOUS_PROMPT_CAPABILITIES, allow_empty=True)
            candidates = self.candidates(domain, stage, required)
            if not candidates:
                raise ArgumentError(f"no prompt template satisfies {domain}/{stage}")
            selected = candidates[0]
            manifest = selected.manifest
            rows.append(
                AutonomousPromptSelectionRow(
                    domain=domain,
                    stage=stage,
                    required_capabilities=required,
                    selected_prompt_id=manifest.prompt_id,
                    selected_version=manifest.version,
                    selected_manifest_digest=manifest.manifest_digest,
                    candidate_prompt_ids=tuple(item.manifest.prompt_id for item in candidates),
                )
            )
        return AutonomousPromptSelectionPlan(registry_digest=self.registry_digest, rows=tuple(rows))

    def verify_selection(self, plan: AutonomousPromptSelectionPlan | Mapping[str, Any]) -> AutonomousPromptSelectionPlan:
        if isinstance(plan, Mapping):
            plan = AutonomousPromptSelectionPlan.from_dict(plan)
        if not isinstance(plan, AutonomousPromptSelectionPlan):
            raise ArgumentError("prompt registry selection plan is malformed")
        if plan.registry_digest != self.registry_digest:
            raise ArgumentError("prompt selection plan is stale for the current registry")
        for row in plan.rows:
            template = self.template_for(row.selected_prompt_id)
            manifest = template.manifest
            if manifest.domain != row.domain or manifest.version != row.selected_version or manifest.manifest_digest != row.selected_manifest_digest:
                raise ArgumentError("prompt selection plan selected manifest is stale or tampered")
            if template not in self.candidates(row.domain, row.stage, row.required_capabilities):
                raise ArgumentError("prompt selection plan selected template no longer satisfies its request")
        return plan

    def render(self, plan: AutonomousPromptSelectionPlan | Mapping[str, Any], context: Mapping[str, Any]) -> AutonomousPromptRenderResult:
        verified = self.verify_selection(plan)
        domain, stage = _context_domain_stage(context)
        matching = [row for row in verified.rows if row.domain == domain and row.stage == stage]
        if len(matching) != 1:
            raise ArgumentError("prompt selection plan has no unique row for render context")
        row = matching[0]
        return self.template_for(row.selected_prompt_id).render_transient(context, selection_plan_digest=verified.plan_digest)


__all__ = [
    "AUTONOMOUS_PROMPT_REGISTRY_SCHEMA",
    "AUTONOMOUS_PROMPT_MANIFEST_SCHEMA",
    "AUTONOMOUS_PROMPT_SELECTION_SCHEMA",
    "AUTONOMOUS_PROMPT_SELECTION_ROW_SCHEMA",
    "AUTONOMOUS_PROMPT_RENDER_SCHEMA",
    "AUTONOMOUS_PROMPT_SELECTION_POLICY",
    "MAX_AUTONOMOUS_PROMPT_TEMPLATES",
    "MAX_AUTONOMOUS_PROMPT_CAPABILITIES",
    "MAX_AUTONOMOUS_PROMPT_STAGES",
    "MAX_AUTONOMOUS_PROMPT_SELECTIONS",
    "MAX_AUTONOMOUS_PROMPT_MESSAGES",
    "MAX_AUTONOMOUS_PROMPT_BYTES",
    "AutonomousPromptManifest",
    "AutonomousPromptRenderResult",
    "AutonomousPromptTemplate",
    "AutonomousPromptSelectionRow",
    "AutonomousPromptSelectionPlan",
    "AutonomousPromptRegistry",
]
