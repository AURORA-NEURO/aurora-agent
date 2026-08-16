"""Typed release-pipeline manifests and provenance-aware audit projections.

This module is an authoring and transport facade over the Rust release-pipeline contract.  It
validates shape, bounds, enum values, and digest syntax before transport, then preserves the
server's independent stage, artifact, attestation, promotion, and rollback evidence.  It does
not execute CI, verify cryptographic signatures, contact registries, or turn a coherent plan into
proof that a deployment happened.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Literal, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


RELEASE_PIPELINE_MANIFEST_SCHEMA = "bioprism-release-pipeline/0.1"
RELEASE_PIPELINE_AUDIT_SCHEMA = "bioprism-release-pipeline-audit/0.1"
RELEASE_PIPELINE_MAX_INPUT_BYTES = 20_000_000
RELEASE_PIPELINE_MAX_ENVIRONMENTS = 256
RELEASE_PIPELINE_MAX_STAGES = 4_096
RELEASE_PIPELINE_MAX_ARTIFACTS = 8_192
RELEASE_PIPELINE_MAX_ATTESTATIONS = 16_384
RELEASE_PIPELINE_MAX_PROMOTIONS = 4_096
RELEASE_PIPELINE_MAX_LIST_ITEMS = 16_384
RELEASE_PIPELINE_MAX_TEXT_BYTES = 4_096

EnvironmentClass = Literal["development", "staging", "production"]
StageKind = Literal["verify", "build", "test", "package", "sign", "publish", "deploy", "smoke", "rollback"]
ArtifactKind = Literal["source", "binary", "container", "package", "manifest", "sbom", "provenance"]
AttestationKind = Literal["test", "provenance", "signature", "approval"]
PromotionKind = Literal["advance", "rollback"]
IssueSeverity = Literal["warning", "blocking"]


def _mapping(name: str, value: Any) -> dict[str, Any]:
    return _route_mapping(name, value)


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _bounded(name: str, value: Any, limit: int) -> tuple[Any, ...]:
    result = _sequence(name, value)
    if len(result) > limit:
        raise ArgumentError(f"{name} is bounded at {limit} items")
    return result


def _text(name: str, value: Any, *, required: bool = True) -> str | None:
    if value is None and not required:
        return None
    result = _route_text(name, value)
    if required and not result.strip():
        raise ArgumentError(f"{name} must not be empty")
    if len(result.encode("utf-8")) > RELEASE_PIPELINE_MAX_TEXT_BYTES:
        raise ArgumentError(f"{name} exceeds {RELEASE_PIPELINE_MAX_TEXT_BYTES} UTF-8 bytes")
    return result


def _text_tuple(name: str, value: Any) -> tuple[str, ...]:
    return tuple(
        _text(f"{name}[{index}]", item)  # type: ignore[misc]
        for index, item in enumerate(_bounded(name, value, RELEASE_PIPELINE_MAX_LIST_ITEMS))
    )


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _integer(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _enum(name: str, value: Any, allowed: frozenset[str]) -> str:
    result = _text(name, value)
    assert result is not None
    if result not in allowed:
        raise ArgumentError(f"{name} must be one of {sorted(allowed)}")
    return result


def _digest(name: str, value: Any) -> str:
    result = _text(name, value)
    assert result is not None
    if len(result) != 64 or any(character not in "0123456789abcdefABCDEF" for character in result):
        raise ArgumentError(f"{name} must be 64 hexadecimal characters")
    return result


def _json_size(name: str, value: Any) -> None:
    try:
        encoded = json.dumps(value, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON serializable: {error}") from error
    if len(encoded) > RELEASE_PIPELINE_MAX_INPUT_BYTES:
        raise ArgumentError(f"{name} exceeds the {RELEASE_PIPELINE_MAX_INPUT_BYTES}-byte safety bound")


@dataclass(frozen=True)
class PipelineProjectArgs:
    id: str
    version: str
    repository: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PipelineProjectArgs":
        raw = _mapping("release pipeline project", value)
        return cls(_text("project.id", raw.get("id")), _text("project.version", raw.get("version")), _text("project.repository", raw.get("repository")))  # type: ignore[arg-type]

    def __post_init__(self) -> None:
        for name in ("id", "version", "repository"):
            _text(f"project.{name}", getattr(self, name))

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "version": self.version, "repository": self.repository}


@dataclass(frozen=True)
class PipelineSourceArgs:
    ref_name: str
    commit_digest: str
    workflow: str

    def __post_init__(self) -> None:
        _text("source.ref_name", self.ref_name)
        _digest("source.commit_digest", self.commit_digest)
        _text("source.workflow", self.workflow)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PipelineSourceArgs":
        raw = _mapping("release pipeline source", value)
        return cls(_text("source.ref_name", raw.get("ref_name")), _digest("source.commit_digest", raw.get("commit_digest")), _text("source.workflow", raw.get("workflow")))  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {"ref_name": self.ref_name, "commit_digest": self.commit_digest, "workflow": self.workflow}


@dataclass(frozen=True)
class PipelineEnvironmentArgs:
    id: str
    class_: EnvironmentClass
    protected: bool = False
    required_approvals: int = 0
    secrets_allowed: bool = False
    immutable_artifacts: bool = False

    def __post_init__(self) -> None:
        _text("environment.id", self.id)
        object.__setattr__(self, "class_", _enum("environment.class", self.class_, frozenset({"development", "staging", "production"})))
        object.__setattr__(self, "protected", _bool("environment.protected", self.protected))
        object.__setattr__(self, "required_approvals", _integer("environment.required_approvals", self.required_approvals))
        object.__setattr__(self, "secrets_allowed", _bool("environment.secrets_allowed", self.secrets_allowed))
        object.__setattr__(self, "immutable_artifacts", _bool("environment.immutable_artifacts", self.immutable_artifacts))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PipelineEnvironmentArgs":
        raw = _mapping("release pipeline environment", value)
        return cls(
            _text("environment.id", raw.get("id")),
            _enum("environment.class", raw.get("class"), frozenset({"development", "staging", "production"})),  # type: ignore[arg-type]
            raw.get("protected", False),
            raw.get("required_approvals", 0),
            raw.get("secrets_allowed", False),
            raw.get("immutable_artifacts", False),
        )  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "class": self.class_,
            "protected": self.protected,
            "required_approvals": self.required_approvals,
            "secrets_allowed": self.secrets_allowed,
            "immutable_artifacts": self.immutable_artifacts,
        }


@dataclass(frozen=True)
class PipelineStageArgs:
    id: str
    kind: StageKind
    environment: str
    depends_on: tuple[str, ...] = ()
    command: str | None = None
    produces: tuple[str, ...] = ()
    required: bool = True

    def __post_init__(self) -> None:
        _text("stage.id", self.id)
        object.__setattr__(self, "kind", _enum("stage.kind", self.kind, frozenset({"verify", "build", "test", "package", "sign", "publish", "deploy", "smoke", "rollback"})))
        _text("stage.environment", self.environment)
        object.__setattr__(self, "depends_on", _text_tuple("stage.depends_on", self.depends_on))
        object.__setattr__(self, "command", _text("stage.command", self.command, required=False))
        object.__setattr__(self, "produces", _text_tuple("stage.produces", self.produces))
        object.__setattr__(self, "required", _bool("stage.required", self.required))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PipelineStageArgs":
        raw = _mapping("release pipeline stage", value)
        return cls(
            _text("stage.id", raw.get("id")),
            _enum("stage.kind", raw.get("kind"), frozenset({"verify", "build", "test", "package", "sign", "publish", "deploy", "smoke", "rollback"})),  # type: ignore[arg-type]
            _text("stage.environment", raw.get("environment")),
            _text_tuple("stage.depends_on", raw.get("depends_on", [])),
            raw.get("command"),
            _text_tuple("stage.produces", raw.get("produces", [])),
            raw.get("required", True),
        )  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "kind": self.kind,
            "environment": self.environment,
            "depends_on": list(self.depends_on),
            "produces": list(self.produces),
            "required": self.required,
        }
        if self.command is not None:
            result["command"] = self.command
        return result


@dataclass(frozen=True)
class PipelineArtifactArgs:
    id: str
    kind: ArtifactKind
    digest: str
    produced_by: str
    inputs: tuple[str, ...] = ()
    attestations: tuple[str, ...] = ()
    immutable: bool = False

    def __post_init__(self) -> None:
        _text("artifact.id", self.id)
        object.__setattr__(self, "kind", _enum("artifact.kind", self.kind, frozenset({"source", "binary", "container", "package", "manifest", "sbom", "provenance"})))
        object.__setattr__(self, "digest", _digest("artifact.digest", self.digest))
        _text("artifact.produced_by", self.produced_by)
        object.__setattr__(self, "inputs", _text_tuple("artifact.inputs", self.inputs))
        object.__setattr__(self, "attestations", _text_tuple("artifact.attestations", self.attestations))
        object.__setattr__(self, "immutable", _bool("artifact.immutable", self.immutable))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PipelineArtifactArgs":
        raw = _mapping("release pipeline artifact", value)
        return cls(
            _text("artifact.id", raw.get("id")),
            _enum("artifact.kind", raw.get("kind"), frozenset({"source", "binary", "container", "package", "manifest", "sbom", "provenance"})),  # type: ignore[arg-type]
            _digest("artifact.digest", raw.get("digest")),
            _text("artifact.produced_by", raw.get("produced_by")),
            _text_tuple("artifact.inputs", raw.get("inputs", [])),
            _text_tuple("artifact.attestations", raw.get("attestations", [])),
            raw.get("immutable", False),
        )  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "kind": self.kind,
            "digest": self.digest,
            "produced_by": self.produced_by,
            "inputs": list(self.inputs),
            "attestations": list(self.attestations),
            "immutable": self.immutable,
        }


@dataclass(frozen=True)
class PipelineAttestationArgs:
    id: str
    kind: AttestationKind
    artifact: str
    digest: str
    issuer: str
    statement: str

    def __post_init__(self) -> None:
        _text("attestation.id", self.id)
        object.__setattr__(self, "kind", _enum("attestation.kind", self.kind, frozenset({"test", "provenance", "signature", "approval"})))
        _text("attestation.artifact", self.artifact)
        object.__setattr__(self, "digest", _digest("attestation.digest", self.digest))
        _text("attestation.issuer", self.issuer)
        _text("attestation.statement", self.statement)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PipelineAttestationArgs":
        raw = _mapping("release pipeline attestation", value)
        return cls(
            _text("attestation.id", raw.get("id")),
            _enum("attestation.kind", raw.get("kind"), frozenset({"test", "provenance", "signature", "approval"})),  # type: ignore[arg-type]
            _text("attestation.artifact", raw.get("artifact")),
            _digest("attestation.digest", raw.get("digest")),
            _text("attestation.issuer", raw.get("issuer")),
            _text("attestation.statement", raw.get("statement")),
        )  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "kind": self.kind, "artifact": self.artifact, "digest": self.digest, "issuer": self.issuer, "statement": self.statement}


@dataclass(frozen=True)
class PipelinePromotionArgs:
    id: str
    kind: PromotionKind
    from_environment: str
    to_environment: str
    artifacts: tuple[str, ...] = ()
    required_attestations: tuple[str, ...] = ()
    approvals: tuple[str, ...] = ()
    rollback_target: str | None = None

    def __post_init__(self) -> None:
        _text("promotion.id", self.id)
        object.__setattr__(self, "kind", _enum("promotion.kind", self.kind, frozenset({"advance", "rollback"})))
        _text("promotion.from", self.from_environment)
        _text("promotion.to", self.to_environment)
        object.__setattr__(self, "artifacts", _text_tuple("promotion.artifacts", self.artifacts))
        object.__setattr__(self, "required_attestations", _text_tuple("promotion.required_attestations", self.required_attestations))
        object.__setattr__(self, "approvals", _text_tuple("promotion.approvals", self.approvals))
        object.__setattr__(self, "rollback_target", _text("promotion.rollback_target", self.rollback_target, required=False))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PipelinePromotionArgs":
        raw = _mapping("release pipeline promotion", value)
        return cls(
            _text("promotion.id", raw.get("id")),
            _enum("promotion.kind", raw.get("kind"), frozenset({"advance", "rollback"})),  # type: ignore[arg-type]
            _text("promotion.from", raw.get("from")),
            _text("promotion.to", raw.get("to")),
            _text_tuple("promotion.artifacts", raw.get("artifacts", [])),
            _text_tuple("promotion.required_attestations", raw.get("required_attestations", [])),
            _text_tuple("promotion.approvals", raw.get("approvals", [])),
            raw.get("rollback_target"),
        )  # type: ignore[arg-type]

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "kind": self.kind,
            "from": self.from_environment,
            "to": self.to_environment,
            "artifacts": list(self.artifacts),
            "required_attestations": list(self.required_attestations),
            "approvals": list(self.approvals),
        }
        if self.rollback_target is not None:
            result["rollback_target"] = self.rollback_target
        return result


@dataclass(frozen=True)
class ReleasePipelinePoliciesArgs:
    require_stage_dag: bool = True
    require_provenance: bool = True
    require_production_signature: bool = True
    require_protected_production: bool = True
    require_rollback: bool = True
    require_approval: bool = True

    def __post_init__(self) -> None:
        for name in ("require_stage_dag", "require_provenance", "require_production_signature", "require_protected_production", "require_rollback", "require_approval"):
            _bool(f"policies.{name}", getattr(self, name))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any] | None) -> "ReleasePipelinePoliciesArgs":
        raw = {} if value is None else _mapping("release pipeline policies", value)
        return cls(raw.get("require_stage_dag", True), raw.get("require_provenance", True), raw.get("require_production_signature", True), raw.get("require_protected_production", True), raw.get("require_rollback", True), raw.get("require_approval", True))

    def to_wire(self) -> dict[str, Any]:
        return {
            "require_stage_dag": self.require_stage_dag,
            "require_provenance": self.require_provenance,
            "require_production_signature": self.require_production_signature,
            "require_protected_production": self.require_protected_production,
            "require_rollback": self.require_rollback,
            "require_approval": self.require_approval,
        }


@dataclass(frozen=True, init=False)
class ReleasePipelineManifestArgs:
    schema: str
    project: PipelineProjectArgs
    source: PipelineSourceArgs
    environments: tuple[PipelineEnvironmentArgs, ...]
    stages: tuple[PipelineStageArgs, ...]
    artifacts: tuple[PipelineArtifactArgs, ...]
    attestations: tuple[PipelineAttestationArgs, ...]
    promotions: tuple[PipelinePromotionArgs, ...]
    policies: ReleasePipelinePoliciesArgs

    def __init__(
        self,
        project: PipelineProjectArgs | Mapping[str, Any],
        source: PipelineSourceArgs | Mapping[str, Any],
        environments: Sequence[PipelineEnvironmentArgs | Mapping[str, Any]] = (),
        stages: Sequence[PipelineStageArgs | Mapping[str, Any]] = (),
        artifacts: Sequence[PipelineArtifactArgs | Mapping[str, Any]] = (),
        attestations: Sequence[PipelineAttestationArgs | Mapping[str, Any]] = (),
        promotions: Sequence[PipelinePromotionArgs | Mapping[str, Any]] = (),
        policies: ReleasePipelinePoliciesArgs | Mapping[str, Any] | None = None,
        schema: str = RELEASE_PIPELINE_MANIFEST_SCHEMA,
    ) -> None:
        normalized_schema = _text("release pipeline schema", schema)
        normalized_project = project if isinstance(project, PipelineProjectArgs) else PipelineProjectArgs.from_wire(project)
        normalized_source = source if isinstance(source, PipelineSourceArgs) else PipelineSourceArgs.from_wire(source)
        environment_values = _bounded("release pipeline environments", environments, RELEASE_PIPELINE_MAX_ENVIRONMENTS)
        stage_values = _bounded("release pipeline stages", stages, RELEASE_PIPELINE_MAX_STAGES)
        artifact_values = _bounded("release pipeline artifacts", artifacts, RELEASE_PIPELINE_MAX_ARTIFACTS)
        attestation_values = _bounded("release pipeline attestations", attestations, RELEASE_PIPELINE_MAX_ATTESTATIONS)
        promotion_values = _bounded("release pipeline promotions", promotions, RELEASE_PIPELINE_MAX_PROMOTIONS)
        normalized_policies = policies if isinstance(policies, ReleasePipelinePoliciesArgs) else ReleasePipelinePoliciesArgs.from_wire(policies)
        normalized_environments = tuple(item if isinstance(item, PipelineEnvironmentArgs) else PipelineEnvironmentArgs.from_wire(item) for item in environment_values)
        normalized_stages = tuple(item if isinstance(item, PipelineStageArgs) else PipelineStageArgs.from_wire(item) for item in stage_values)
        normalized_artifacts = tuple(item if isinstance(item, PipelineArtifactArgs) else PipelineArtifactArgs.from_wire(item) for item in artifact_values)
        normalized_attestations = tuple(item if isinstance(item, PipelineAttestationArgs) else PipelineAttestationArgs.from_wire(item) for item in attestation_values)
        normalized_promotions = tuple(item if isinstance(item, PipelinePromotionArgs) else PipelinePromotionArgs.from_wire(item) for item in promotion_values)
        wire = {
            "schema": normalized_schema,
            "project": normalized_project.to_wire(),
            "source": normalized_source.to_wire(),
            "environments": [item.to_wire() for item in normalized_environments],
            "stages": [item.to_wire() for item in normalized_stages],
            "artifacts": [item.to_wire() for item in normalized_artifacts],
            "attestations": [item.to_wire() for item in normalized_attestations],
            "promotions": [item.to_wire() for item in normalized_promotions],
            "policies": normalized_policies.to_wire(),
        }
        _json_size("release pipeline manifest", wire)
        object.__setattr__(self, "schema", normalized_schema)
        object.__setattr__(self, "project", normalized_project)
        object.__setattr__(self, "source", normalized_source)
        object.__setattr__(self, "environments", normalized_environments)
        object.__setattr__(self, "stages", normalized_stages)
        object.__setattr__(self, "artifacts", normalized_artifacts)
        object.__setattr__(self, "attestations", normalized_attestations)
        object.__setattr__(self, "promotions", normalized_promotions)
        object.__setattr__(self, "policies", normalized_policies)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ReleasePipelineManifestArgs":
        raw = _mapping("release pipeline manifest", value)
        return cls(
            raw.get("project"),
            raw.get("source"),
            _bounded("release pipeline environments", raw.get("environments", []), RELEASE_PIPELINE_MAX_ENVIRONMENTS),
            _bounded("release pipeline stages", raw.get("stages", []), RELEASE_PIPELINE_MAX_STAGES),
            _bounded("release pipeline artifacts", raw.get("artifacts", []), RELEASE_PIPELINE_MAX_ARTIFACTS),
            _bounded("release pipeline attestations", raw.get("attestations", []), RELEASE_PIPELINE_MAX_ATTESTATIONS),
            _bounded("release pipeline promotions", raw.get("promotions", []), RELEASE_PIPELINE_MAX_PROMOTIONS),
            raw.get("policies"),
            raw.get("schema", RELEASE_PIPELINE_MANIFEST_SCHEMA),
        )

    def to_wire(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "project": self.project.to_wire(),
            "source": self.source.to_wire(),
            "environments": [item.to_wire() for item in self.environments],
            "stages": [item.to_wire() for item in self.stages],
            "artifacts": [item.to_wire() for item in self.artifacts],
            "attestations": [item.to_wire() for item in self.attestations],
            "promotions": [item.to_wire() for item in self.promotions],
            "policies": self.policies.to_wire(),
        }

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"manifest": self.to_wire()}


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _mapping("release pipeline response", value)
    candidates: list[Mapping[str, Any]] = [raw]

    def add(container: Any) -> None:
        if not isinstance(container, Mapping):
            return
        candidates.append(container)
        nested = container.get("result")
        if isinstance(nested, Mapping):
            candidates.append(nested)
            add(nested.get("structuredContent"))
            content = nested.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"release pipeline response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
        add(container.get("structuredContent"))

    add(raw.get("mcp"))
    add(raw.get("result"))
    add(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("schema") == RELEASE_PIPELINE_AUDIT_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain a release pipeline audit projection")


@dataclass(frozen=True)
class ReleasePipelineIssueReport:
    code: str
    severity: IssueSeverity
    subject: str
    detail: str
    remediation: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ReleasePipelineIssueReport":
        raw = _mapping("release pipeline issue", value)
        return cls(_text("release issue code", raw.get("code")), _enum("release issue severity", raw.get("severity"), frozenset({"warning", "blocking"})), _text("release issue subject", raw.get("subject")), _text("release issue detail", raw.get("detail")), _text("release issue remediation", raw.get("remediation")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class PipelineStageReadinessReport:
    stage_id: str
    state: str
    dependency_ready: bool
    blocking_dependencies: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PipelineStageReadinessReport":
        raw = _mapping("release stage readiness", value)
        return cls(_text("stage readiness stage_id", raw.get("stage_id")), _text("stage readiness state", raw.get("state")), _bool("stage readiness dependency_ready", raw.get("dependency_ready")), _text_tuple("stage readiness blocking_dependencies", raw.get("blocking_dependencies", [])))  # type: ignore[arg-type]


@dataclass(frozen=True)
class PipelineArtifactAuditReport:
    artifact_id: str
    digest_valid: bool
    producer_valid: bool
    inputs_valid: bool
    attestations_valid: bool
    provenance_present: bool
    signature_present: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PipelineArtifactAuditReport":
        raw = _mapping("release artifact audit", value)
        return cls(_text("artifact audit artifact_id", raw.get("artifact_id")), _bool("artifact audit digest_valid", raw.get("digest_valid")), _bool("artifact audit producer_valid", raw.get("producer_valid")), _bool("artifact audit inputs_valid", raw.get("inputs_valid")), _bool("artifact audit attestations_valid", raw.get("attestations_valid")), _bool("artifact audit provenance_present", raw.get("provenance_present")), _bool("artifact audit signature_present", raw.get("signature_present")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class PipelinePromotionAuditReport:
    promotion_id: str
    from_environment: str
    to_environment: str
    valid: bool
    production: bool
    missing_attestations: tuple[str, ...]
    missing_approvals: tuple[str, ...]
    rollback_present: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PipelinePromotionAuditReport":
        raw = _mapping("release promotion audit", value)
        return cls(_text("promotion audit promotion_id", raw.get("promotion_id")), _text("promotion audit from", raw.get("from")), _text("promotion audit to", raw.get("to")), _bool("promotion audit valid", raw.get("valid")), _bool("promotion audit production", raw.get("production")), _text_tuple("promotion audit missing_attestations", raw.get("missing_attestations", [])), _text_tuple("promotion audit missing_approvals", raw.get("missing_approvals", [])), _bool("promotion audit rollback_present", raw.get("rollback_present")))  # type: ignore[arg-type]


@dataclass(frozen=True)
class ReleasePipelineAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    manifest_digest: str | None
    valid: bool | None
    release_ready_value: bool | None
    counts: Mapping[str, Any] | None
    stage_order: tuple[str, ...]
    cyclic_stages: tuple[tuple[str, ...], ...]
    stage_readiness: tuple[PipelineStageReadinessReport, ...]
    artifact_audits: tuple[PipelineArtifactAuditReport, ...]
    promotion_audits: tuple[PipelinePromotionAuditReport, ...]
    issues: tuple[ReleasePipelineIssueReport, ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ReleasePipelineAuditReport":
        raw = _payload(value)
        ok = raw.get("ok") is True
        if not ok:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("release pipeline refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), raw.get("manifest_digest"), False, False, None, (), (), (), (), (), (), _route_strings("release refusal guarantees", raw.get("guarantees", [])), _route_strings("release refusal limitations", raw.get("limitations", [])), raw.get("refusal") or raw.get("error"), True)
        if raw.get("schema") != RELEASE_PIPELINE_AUDIT_SCHEMA:
            raise ArgumentError("release pipeline projection has an invalid schema")
        audit = _mapping("release pipeline audit", raw.get("audit"))
        issues = tuple(ReleasePipelineIssueReport.from_wire(item) for item in _bounded("release audit issues", audit.get("issues", []), RELEASE_PIPELINE_MAX_LIST_ITEMS))
        readiness = tuple(PipelineStageReadinessReport.from_wire(item) for item in _bounded("release stage readiness", audit.get("stage_readiness", []), RELEASE_PIPELINE_MAX_STAGES))
        artifacts = tuple(PipelineArtifactAuditReport.from_wire(item) for item in _bounded("release artifact audits", audit.get("artifact_audits", []), RELEASE_PIPELINE_MAX_ARTIFACTS))
        promotions = tuple(PipelinePromotionAuditReport.from_wire(item) for item in _bounded("release promotion audits", audit.get("promotion_audits", []), RELEASE_PIPELINE_MAX_PROMOTIONS))
        cycles = tuple(_text_tuple(f"release cycle[{index}]", item) for index, item in enumerate(_bounded("release cyclic_stages", audit.get("cyclic_stages", []), RELEASE_PIPELINE_MAX_STAGES)))
        return cls(
            raw,
            True,
            RELEASE_PIPELINE_AUDIT_SCHEMA,
            _text("release workflow", raw.get("workflow")),
            _text("release manifest_digest", raw.get("manifest_digest"), required=False),
            _bool("release audit valid", audit.get("valid")),
            _bool("release_ready", raw.get("release_ready")),
            _mapping("release audit counts", audit.get("counts")),
            _text_tuple("release stage_order", audit.get("stage_order", [])),
            cycles,
            readiness,
            artifacts,
            promotions,
            issues,
            _route_strings("release guarantees", raw.get("guarantees", audit.get("guarantees", []))),
            _route_strings("release limitations", raw.get("limitations", audit.get("limitations", []))),
            None,
            False,
        )

    @property
    def accepted(self) -> bool:
        return self.ok and self.valid is True and self.release_ready_value is True

    @property
    def release_ready(self) -> bool:
        return self.release_ready_value is True

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def blocking_issues(self) -> tuple[ReleasePipelineIssueReport, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "blocking")

    @property
    def warning_issues(self) -> tuple[ReleasePipelineIssueReport, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "warning")

    @property
    def has_blockers(self) -> bool:
        return bool(self.blocking_issues)

    @property
    def production_promotions(self) -> tuple[PipelinePromotionAuditReport, ...]:
        return tuple(item for item in self.promotion_audits if item.production)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def release_pipeline_audit_report(value: Mapping[str, Any]) -> ReleasePipelineAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return ReleasePipelineAuditReport.from_wire(value)


__all__ = [
    "RELEASE_PIPELINE_MANIFEST_SCHEMA",
    "RELEASE_PIPELINE_AUDIT_SCHEMA",
    "RELEASE_PIPELINE_MAX_INPUT_BYTES",
    "RELEASE_PIPELINE_MAX_ENVIRONMENTS",
    "RELEASE_PIPELINE_MAX_STAGES",
    "RELEASE_PIPELINE_MAX_ARTIFACTS",
    "RELEASE_PIPELINE_MAX_ATTESTATIONS",
    "RELEASE_PIPELINE_MAX_PROMOTIONS",
    "PipelineProjectArgs",
    "PipelineSourceArgs",
    "PipelineEnvironmentArgs",
    "PipelineStageArgs",
    "PipelineArtifactArgs",
    "PipelineAttestationArgs",
    "PipelinePromotionArgs",
    "ReleasePipelinePoliciesArgs",
    "ReleasePipelineManifestArgs",
    "ReleasePipelineIssueReport",
    "PipelineStageReadinessReport",
    "PipelineArtifactAuditReport",
    "PipelinePromotionAuditReport",
    "ReleasePipelineAuditReport",
    "release_pipeline_audit_report",
]
