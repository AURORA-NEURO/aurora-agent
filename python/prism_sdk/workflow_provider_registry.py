"""Versioned reviewed service/reader inventory bound to executable workflow catalogues.

This application layer has no blueprint module identifier. It implements the provider/evidence
ledger prerequisite in WORKFLOW_DEVELOPMENT_PLAN, not a generated list of integrations. A service
identity is an explicitly reviewed API grouping, not an independently operated organization.
Registration, schema conformance, a runtime result and a referenced live report are distinct facts.
None establishes continuous availability, remote network authenticity or scientific support.

The registry never discovers providers, obtains credentials, imports request-selected code, or
upgrades an externally supplied evidence label into a verified live measurement. Its execution
entry point uses the existing concrete catalogue and the same literal source approval gate.
"""
from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from datetime import datetime, timezone
import hashlib
from importlib import import_module
import ipaddress
import json
from pathlib import Path
import re
from types import FunctionType, MappingProxyType
from typing import Any
from urllib.parse import urlsplit

from .authoring import canonical_json, content_digest
from .errors import ArgumentError
from .workflow_integrations import (
    WorkflowOperationCatalog, WorkflowOperationResult, builtin_workflow_operation_catalog,
)

WORKFLOW_PROVIDER_REGISTRY_SCHEMA = "aurora-workflow-provider-registry/0.1"
WORKFLOW_PROVIDER_MANIFEST_VERSION = "reviewed-public-readers/2026-10-27.2"
WORKFLOW_PROVIDER_REQUEST_SCHEMA = "aurora-workflow-provider-request/0.1"
WORKFLOW_PROVIDER_OBSERVATION_SCHEMA = "aurora-workflow-provider-observation/0.1"
MAX_WORKFLOW_PROVIDER_RECORDS = 1_024
MAX_WORKFLOW_PROVIDER_READERS = 4_096
MAX_WORKFLOW_PROVIDER_EVIDENCE_REFERENCES = 1_024
_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}\Z")
_SHA = re.compile(r"[a-f0-9]{64}\Z")


def _identifier(value: Any) -> str:
    if type(value) is not str or not _ID.fullmatch(value):
        raise ArgumentError("provider registry identity must be a bounded identifier")
    return value


def _text(value: Any, maximum: int = 2_048) -> str:
    if type(value) is not str or not value.strip() or len(value.encode("utf-8")) > maximum or any(ord(char) < 32 for char in value):
        raise ArgumentError("provider registry text is outside its bound")
    return value


def _digest(value: Any) -> str:
    if type(value) is not str or not _SHA.fullmatch(value):
        raise ArgumentError("provider registry digest must be SHA-256 hex")
    return value


def _json(value: Any, maximum: int = 8_000_000) -> str:
    try:
        result = canonical_json(value)
    except (ValueError, TypeError, RecursionError, OverflowError):
        raise ArgumentError("provider registry value must be finite bounded JSON") from None
    if len(result.encode("utf-8")) > maximum:
        raise ArgumentError("provider registry value exceeds its byte bound")
    return result


def _sequence(value: Any, maximum: int) -> tuple[Any, ...]:
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence) or len(value) > maximum:
        raise ArgumentError("provider registry sequence is outside its bound")
    return tuple(value)


def _freeze(value: Any) -> Any:
    if type(value) is dict:
        return MappingProxyType({key: _freeze(child) for key, child in value.items()})
    if type(value) is list:
        return tuple(_freeze(child) for child in value)
    return value


def _origin(value: Any) -> str:
    value = _text(value, 512)
    try:
        parsed = urlsplit(value)
        port = parsed.port
    except ValueError:
        raise ArgumentError("provider registry origin must be a credentialless HTTPS authority") from None
    if (parsed.scheme != "https" or not parsed.hostname or port not in (None, 443)
            or parsed.username is not None or parsed.password is not None or parsed.query
            or parsed.fragment or parsed.path not in ("", "/") or parsed.hostname != parsed.hostname.lower()
            or ":" in parsed.hostname or not re.fullmatch(r"[a-z0-9]+(?:[.-][a-z0-9]+)+", parsed.hostname)):
        raise ArgumentError("provider registry origin must be a credentialless HTTPS authority")
    try:
        ipaddress.ip_address(parsed.hostname)
    except ValueError:
        pass
    else:
        raise ArgumentError("provider registry service identity requires a DNS authority, not an IP literal")
    return "https://" + parsed.hostname


@dataclass(frozen=True)
class WorkflowProviderIdentity:
    """A reviewed API service grouping; origins do not establish operator independence."""

    provider_id: str
    display_name: str
    origins: tuple[str, ...]
    review_basis: str
    _document: str = field(init=False, repr=False)

    def __post_init__(self) -> None:
        origins = tuple(_origin(value) for value in _sequence(self.origins, 16))
        if not origins or len(set(origins)) != len(origins):
            raise ArgumentError("provider registry origins must be nonempty and unique")
        value = {"schema": "aurora-workflow-provider-identity/0.1",
                 "provider_id": _identifier(self.provider_id), "display_name": _text(self.display_name),
                 "origins": sorted(origins), "review_basis": _text(self.review_basis),
                 "identity_unit": "reviewed_api_service_group", "review_authority": "application_supplied_review_record",
                 "operator_independence": "unassessed", "provider_verified_identity": "not_established_by_registry"}
        object.__setattr__(self, "origins", origins)
        object.__setattr__(self, "_document", _json(value))

    def to_dict(self) -> dict[str, Any]:
        value = json.loads(self._document)
        return value | {"identity_digest": content_digest(value)}


@dataclass(frozen=True)
class WorkflowReaderContract:
    """One concrete reader path with exact normalized contract and component source digests.

    Source digests identify retained component bytes. They do not attest a Python interpreter,
    arbitrary foreign module globals, upstream server behavior or an official API version.
    """

    operation_id: str
    provider_id: str
    operation: Mapping[str, Any]
    components: Mapping[str, str]
    review_document: str
    _document: str = field(init=False, repr=False)

    def __post_init__(self) -> None:
        operation = json.loads(_json(self.operation, 2_000_000))
        if (operation.get("operation_id") != self.operation_id or operation.get("auth_mode") != "none"
                or operation.get("read_only") is not True or operation.get("implementation") != "live_adapter"):
            raise ArgumentError("reviewed public reader requires a credentialless read-only concrete adapter")
        if not isinstance(self.components, Mapping) or not self.components or len(self.components) > 16:
            raise ArgumentError("reader component digests are outside their bound")
        components = {_identifier(name): _digest(digest) for name, digest in self.components.items()}
        document = _text(self.review_document, 256)
        if not re.fullmatch(r"docs/[A-Z0-9_]+\.md", document):
            raise ArgumentError("reader review document must name a repository document")
        value = {"schema": "aurora-workflow-reader-contract/0.1",
                 "operation_id": _identifier(self.operation_id), "provider_id": _identifier(self.provider_id),
                 "operation": operation, "operation_digest": content_digest(operation),
                 "component_sha256": components, "review_document": document,
                 "contract_origin": "repository_reviewed_normalized_adapter",
                 "upstream_contract_version": {"state": "not_established", "value": None},
                 "auth_requirement": "credentialless_public", "data_class": "public_research_metadata_or_aggregate",
                 "private_data_retention": "unsupported", "remote_writes": "unsupported",
                 "scientific_support": "unassessed"}
        object.__setattr__(self, "_document", _json(value))
        object.__setattr__(self, "operation", _freeze(operation))
        object.__setattr__(self, "components", _freeze(components))

    def to_dict(self) -> dict[str, Any]:
        value = json.loads(self._document)
        return value | {"contract_digest": content_digest(value)}


@dataclass(frozen=True)
class WorkflowProviderEvidenceReference:
    """An external report pointer, always unverified by this registry.

    The SHA-256 is caller-declared and is not checked by dereferencing paths or URLs. Even a
    matching report digest would establish byte consistency, not acquisition authenticity.
    """

    operation_id: str
    evidence_kind: str
    reference_id: str
    artifact_sha256: str
    _document: str = field(init=False, repr=False)

    def __post_init__(self) -> None:
        if self.evidence_kind not in {"acquisition", "offline_replay", "fixture", "failed_attempt", "unjudged_attempt"}:
            raise ArgumentError("provider evidence reference kind is unsupported")
        value = {"schema": "aurora-workflow-provider-evidence-reference/0.1",
                 "operation_id": _identifier(self.operation_id), "evidence_kind": self.evidence_kind,
                 "reference_id": _identifier(self.reference_id), "artifact_sha256": _digest(self.artifact_sha256),
                 "verification_state": "unverified_external_reference", "artifact_opened": False,
                 "supports_live_coverage": False}
        object.__setattr__(self, "_document", _json(value))

    def to_dict(self) -> dict[str, Any]:
        return json.loads(self._document)


@dataclass(frozen=True)
class WorkflowProviderRequest:
    """An exact normalized request; creating it grants no source dispatch authority."""

    _document: str = field(repr=False)

    def to_dict(self) -> dict[str, Any]:
        if type(self._document) is not str or len(self._document.encode("utf-8")) > 2_100_000:
            raise ArgumentError("provider prepared request exceeds its JSON byte bound")
        try:
            value = json.loads(self._document)
            if type(value) is not dict:
                raise ValueError
            _json(value, 2_100_000)
            return value
        except (ValueError, TypeError, RecursionError):
            raise ArgumentError("provider prepared request must be a bounded JSON object") from None


@dataclass(frozen=True)
class WorkflowProviderExecution:
    """Native operation result and a metadata-only in-process observation."""

    result: WorkflowOperationResult = field(repr=False)
    _observation: str = field(repr=False)

    def observation(self) -> dict[str, Any]:
        return json.loads(self._observation)


class WorkflowProviderDispatchError(ArgumentError):
    """An entered adapter failed; its sanitized observation is retained separately."""

    def __init__(self, observation: Mapping[str, Any]) -> None:
        super().__init__("provider operation failed; inspect the retained bounded attempt observation")
        self._observation = _json(observation, 64_000)

    def observation(self) -> dict[str, Any]:
        return json.loads(self._observation)


def _executor_shape(executor: Any) -> tuple[Any, ...] | None:
    """Only exact finite literal closures qualify as a repository default executor.

    In particular, a caller fetcher hidden in default keyword arguments must not acquire a live
    label merely by reusing an operation ID and JSON schema.
    """
    def literal(value: Any) -> Any:
        if value is None or type(value) in (str, int, bool):
            return (type(value), value)
        if type(value) is tuple:
            return (tuple, tuple(literal(child) for child in value))
        if value is WorkflowOperationResult:
            return ("reviewed_result_type", id(value))
        raise ValueError

    if type(executor) is not FunctionType:
        return None
    try:
        return (id(executor.__code__), id(executor.__globals__), literal(executor.__defaults__),
                tuple(sorted((key, literal(value)) for key, value in (executor.__kwdefaults__ or {}).items())),
                tuple(literal(cell.cell_contents) for cell in (executor.__closure__ or ())))
    except (ValueError, TypeError):
        return None


def _binding_shape(executor: Any) -> tuple[Any, ...]:
    shape = _executor_shape(executor)
    if shape is not None:
        return (id(executor), shape)
    # Non-default executors remain visible but cannot enter the reviewed dispatch path. Their
    # identity is still anchored so replacing an unreviewed callable invalidates inspection.
    return (id(executor),)


class WorkflowProviderRegistry:
    """Immutable reviewed inventory and a checked binding to one live catalogue object.

    Changed catalogue, descriptor, captured default controls, executor identity or component
    source bytes require a new registry binding. Catalogue mutation is refused before dispatch.
    This is an application consistency check, not a sandbox against arbitrary in-process code.
    """

    def __init__(self, catalog: WorkflowOperationCatalog, *, providers: Sequence[WorkflowProviderIdentity],
                 readers: Sequence[WorkflowReaderContract], reviewed_catalog: WorkflowOperationCatalog,
                 evidence_references: Sequence[WorkflowProviderEvidenceReference] = (),
                 manifest_version: str = WORKFLOW_PROVIDER_MANIFEST_VERSION) -> None:
        if type(catalog) is not WorkflowOperationCatalog or type(reviewed_catalog) is not WorkflowOperationCatalog:
            raise ArgumentError("provider registry requires concrete workflow catalogues")
        providers = _sequence(providers, MAX_WORKFLOW_PROVIDER_RECORDS)
        readers = _sequence(readers, MAX_WORKFLOW_PROVIDER_READERS)
        references = _sequence(evidence_references, MAX_WORKFLOW_PROVIDER_EVIDENCE_REFERENCES)
        if any(type(value) is not WorkflowProviderIdentity for value in providers) or any(type(value) is not WorkflowReaderContract for value in readers):
            raise ArgumentError("provider registry requires typed identity and reader records")
        if any(type(value) is not WorkflowProviderEvidenceReference for value in references):
            raise ArgumentError("provider registry requires typed evidence reference records")
        provider_records = [value.to_dict() for value in providers]
        reader_records = [value.to_dict() for value in readers]
        provider_ids = {value["provider_id"] for value in provider_records}
        reader_ids = {value["operation_id"] for value in reader_records}
        if len(provider_ids) != len(providers) or len(reader_ids) != len(readers):
            raise ArgumentError("provider registry identity or reader is duplicated")
        # The same authority can expose separately reviewed APIs (EBI is one example), so origins
        # cannot prove either provider equality or provider independence.
        if any(value["provider_id"] not in provider_ids for value in reader_records):
            raise ArgumentError("provider registry reader names an unknown service identity")
        if any(value.to_dict()["operation_id"] not in reader_ids for value in references):
            raise ArgumentError("provider evidence reference names an unknown reader")
        reference_keys = {(value.to_dict()["operation_id"], value.to_dict()["reference_id"]) for value in references}
        if len(reference_keys) != len(references):
            raise ArgumentError("provider evidence reference is duplicated")
        if {operation.operation_id for operation in reviewed_catalog.operations()} != reader_ids:
            raise ArgumentError("reviewed catalogue must contain exactly the declared reader contracts")
        for value in reader_records:
            if _json(reviewed_catalog.resolve(value["operation_id"]).to_dict()) != _json(value["operation"]):
                raise ArgumentError("reviewed reader contract does not match its concrete operation")
        self._catalog = catalog
        self._catalog_digest = catalog.catalog_digest
        self._executors = {name: (executor, _binding_shape(executor)) for name, executor in catalog._executors.items()}
        self._readers = {value["operation_id"]: _json(value) for value in reader_records}
        self._default_shapes = {name: _executor_shape(executor) for name, executor in reviewed_catalog._executors.items()}
        self._components = {}
        for value in reader_records:
            for name, digest in value["component_sha256"].items():
                if name in self._components and self._components[name] != digest:
                    raise ArgumentError("provider reader contracts declare conflicting component digests")
                self._components[name] = digest
        self._manifest = _json({"manifest_version": _text(manifest_version, 256),
            "providers": sorted(provider_records, key=lambda value: value["provider_id"]),
            "readers": sorted(reader_records, key=lambda value: value["operation_id"]),
            "evidence_references": sorted((value.to_dict() for value in references), key=lambda value: (value["operation_id"], value["reference_id"]))})
        self._manifest_digest = content_digest(json.loads(self._manifest))
        self._binding_digest = content_digest({"manifest_digest": self._manifest_digest, "catalog_digest": self._catalog_digest})
        self._check_binding()

    def _check_binding(self) -> None:
        try:
            self._inspect_binding()
        except ArgumentError:
            raise
        except Exception:
            # Corrupted/foreign descriptors can make catalogue hashing itself fail. That is an
            # uninspectable binding, never evidence that no admitted attempt occurred.
            raise ArgumentError("provider registry binding inspection failed; binding is not established") from None

    def _inspect_binding(self) -> None:
        if self._catalog.catalog_digest != self._catalog_digest or set(self._catalog._executors) != set(self._executors):
            raise ArgumentError("provider registry catalogue binding changed; construct a new reviewed registry")
        for name, (executor, shape) in self._executors.items():
            if self._catalog._executors[name] is not executor or _binding_shape(executor) != shape:
                raise ArgumentError("provider registry executor binding changed; construct a new reviewed registry")
        for name, digest in self._components.items():
            if _component_digest(name) != digest:
                raise ArgumentError("provider registry component source changed; construct a new reviewed registry")

    def _reader_state(self, record: Mapping[str, Any]) -> str:
        operation_id = record["operation_id"]
        if operation_id not in self._executors:
            return "not_registered"
        if _json(self._catalog.resolve(operation_id).to_dict()) != _json(record["operation"]):
            return "registered_contract_differs"
        actual = _executor_shape(self._catalog._executors[operation_id])
        if actual is None or actual != self._default_shapes[operation_id]:
            return "registered_executor_not_reviewed_default"
        return "registered_reviewed_default"

    def to_dict(self) -> dict[str, Any]:
        self._check_binding()
        manifest = json.loads(self._manifest)
        readers = [record | {"registration_state": self._reader_state(record),
            "live_observation": {"state": "unmeasured_by_registry", "value": None},
            "continuous_availability": "unmeasured", "provider_account": "not_applicable_credentialless",
            "dispatch_authorization": "separate_explicit_approval_required"} for record in manifest["readers"]]
        registered = [record for record in readers if record["registration_state"] == "registered_reviewed_default"]
        value = {"schema": WORKFLOW_PROVIDER_REGISTRY_SCHEMA, "manifest_version": manifest["manifest_version"],
            "manifest_digest": self._manifest_digest, "catalog_digest": self._catalog_digest,
            "binding_digest": self._binding_digest, "providers": manifest["providers"], "readers": readers,
            "evidence_references": manifest["evidence_references"],
            "counts": {"declared_reviewed_service_groups": len(manifest["providers"]),
                "implemented_reader_paths": len(readers), "registered_reviewed_default_readers": len(registered),
                "supported_readers_not_registered": sum(record["registration_state"] == "not_registered" for record in readers),
                "registered_unmatched_readers": sum(record["registration_state"] in {"registered_contract_differs", "registered_executor_not_reviewed_default"} for record in readers),
                "registered_reviewed_service_groups": len({record["provider_id"] for record in registered}),
                "external_evidence_references": len(manifest["evidence_references"]),
                "independently_live_verified_providers": {"state": "not_established_by_registry", "value": None}},
            "unreviewed_operation_ids": sorted(set(self._executors) - set(self._readers)),
            "unreviewed_operation_binding_scope": "executor_identity_only; provider/configuration_semantics_unchecked",
            "network_requests_on_inspection": 0,
            "limitations": ["Service groups are reviewed API identities, not independent organizations or verified live providers.",
                "Registration and exact schema/source binding do not establish provider availability, network authenticity or scientific support.",
                "Referenced acquisition, replay and fixture reports remain separate unverified external references.",
                "No OAuth/account onboarding, private payload retention, quota scheduler or remote-write support is added.",
                "Component hashes identify source bytes; arbitrary in-process global mutation and interpreter authenticity are outside this check."]}
        return value | {"registry_digest": content_digest(value)}

    def inspect_request(self, operation_id: str, arguments: Mapping[str, Any]) -> dict[str, Any]:
        """Validate and normalize a reader request without entering its executor."""
        request = self.prepare(operation_id, arguments).to_dict()
        return {"state": "ready_for_separately_approved_bounded_dispatch", "request": request,
                "argument_schema_conformance": "checked", "executor_entered": False,
                "source_requests": 0, "remote_response_behavior": "unmeasured",
                "failure_and_outage_behavior": "unmeasured_by_this_check"}

    def prepare(self, operation_id: str, arguments: Mapping[str, Any]) -> WorkflowProviderRequest:
        self._check_binding()
        operation_id = _identifier(operation_id)
        if operation_id not in self._readers:
            raise ArgumentError("operation has no reviewed provider reader contract")
        reader = json.loads(self._readers[operation_id])
        if self._reader_state(reader) != "registered_reviewed_default":
            raise ArgumentError("reader is not bound to the exact registered reviewed default executor")
        safe = self._catalog.validate_arguments(operation_id, arguments)
        self._check_binding()
        value = {"schema": WORKFLOW_PROVIDER_REQUEST_SCHEMA, "operation_id": operation_id,
                 "provider_id": reader["provider_id"], "contract_digest": reader["contract_digest"],
                 "registry_binding_digest": self._binding_digest, "catalog_digest": self._catalog_digest,
                 "arguments": safe, "arguments_digest": content_digest(safe)}
        return WorkflowProviderRequest(_json(value | {"request_digest": content_digest(value)}))

    def execute(self, request: WorkflowProviderRequest, *, approve_source_dispatch: bool = False,
                retrieved_at: str | None = None) -> WorkflowProviderExecution:
        """Execute one existing reader and retain result/failure without inventing HTTP counts."""
        if type(request) is not WorkflowProviderRequest:
            raise ArgumentError("provider dispatch requires a typed prepared request")
        document = request.to_dict()
        if set(document) != {"schema", "operation_id", "provider_id", "contract_digest", "registry_binding_digest", "catalog_digest", "arguments", "arguments_digest", "request_digest"}:
            raise ArgumentError("provider prepared request fields are invalid")
        current = self.prepare(document["operation_id"], document["arguments"]).to_dict()
        if _json(current) != _json(document):
            raise ArgumentError("provider prepared request differs from its exact current binding")
        if approve_source_dispatch is not True or type(approve_source_dispatch) is not bool:
            raise ArgumentError("provider dispatch requires explicit literal source approval")
        started_at = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
        observation = {"schema": WORKFLOW_PROVIDER_OBSERVATION_SCHEMA,
            "request_digest": current["request_digest"], "registry_binding_digest": self._binding_digest,
            "catalog_digest": self._catalog_digest, "contract_digest": current["contract_digest"],
            "operation_id": current["operation_id"], "provider_id": current["provider_id"],
            "started_at": started_at, "evidence_kind": "in_process_default_adapter_attempt",
            "catalogue_execute_invocation_count": 1,
            "actual_http_request_count": {"state": "not_independently_observed", "value": None},
            "network_authenticity": "unassessed", "scientific_support": "unassessed",
            "continuous_availability": "unmeasured", "supports_live_provider_coverage": False}
        result = None
        try:
            result = self._catalog.execute(current["operation_id"], current["arguments"],
                approve_source_dispatch=True, retrieved_at=retrieved_at)
        except Exception:
            observation |= {"outcome": "adapter_or_schema_failed", "result": None,
                            "failure_detail": "sanitized; inspect application-owned source failure evidence separately"}
        else:
            observation |= {"outcome": "schema_validated_result", "result": {
                "value_digest": content_digest(result.value), "receipt_digest": content_digest(result.receipt),
                "empty_result_semantics": "source_specific; not_inferred_from_operation_success"}}
        # A foreign adapter can mutate the binding and then succeed OR fail. Refusing its result
        # must not erase the catalogue invocation and result/failure measurements already made.
        try:
            self._check_binding()
        except ArgumentError:
            observation |= {"adapter_outcome": observation["outcome"],
                            "outcome": "binding_invalidated_after_dispatch", "result_admitted": False,
                            "failure_detail": "binding_changed; retained_result_digests_are_not_an_admitted_result"}
            raise WorkflowProviderDispatchError(observation | {"observation_digest": content_digest(observation)}) from None
        if result is None:
            raise WorkflowProviderDispatchError(observation | {"observation_digest": content_digest(observation)}) from None
        return WorkflowProviderExecution(result, _json(observation | {"observation_digest": content_digest(observation)}))

    def verify_observation(self, observation: Mapping[str, Any], request: WorkflowProviderRequest, *,
                           result: WorkflowOperationResult | None = None) -> dict[str, Any]:
        """Check retained metadata/value consistency; never reacquire or authenticate a source."""
        self._check_binding()
        if type(request) is not WorkflowProviderRequest or not isinstance(observation, Mapping):
            raise ArgumentError("provider observation verification requires typed retained inputs")
        safe = json.loads(_json(dict(observation), 64_000))
        unsigned = {key: value for key, value in safe.items() if key != "observation_digest"}
        if safe.get("observation_digest") != content_digest(unsigned):
            raise ArgumentError("provider observation digest is inconsistent")
        document = request.to_dict()
        if _json(document) != _json(self.prepare(document.get("operation_id"), document.get("arguments")).to_dict()):
            raise ArgumentError("provider observation request is not currently bound")
        common = {"schema": WORKFLOW_PROVIDER_OBSERVATION_SCHEMA,
            "request_digest": document["request_digest"], "registry_binding_digest": self._binding_digest,
            "catalog_digest": self._catalog_digest, "contract_digest": document["contract_digest"],
            "operation_id": document["operation_id"], "provider_id": document["provider_id"],
            "evidence_kind": "in_process_default_adapter_attempt", "catalogue_execute_invocation_count": 1,
            "actual_http_request_count": {"state": "not_independently_observed", "value": None},
            "network_authenticity": "unassessed", "scientific_support": "unassessed",
            "continuous_availability": "unmeasured", "supports_live_provider_coverage": False}
        if any(_json(safe.get(key)) != _json(value) for key, value in common.items()):
            raise ArgumentError("provider observation binding or evidence state is inconsistent")
        try:
            stamp = safe["started_at"]
            if not isinstance(stamp, str) or not stamp.endswith("Z") or datetime.fromisoformat(stamp.replace("Z", "+00:00")).utcoffset().total_seconds() != 0:
                raise ValueError
        except (ValueError, KeyError, AttributeError):
            raise ArgumentError("provider observation time is invalid") from None
        outcome = safe.get("outcome")
        if outcome == "schema_validated_result":
            if type(result) is not WorkflowOperationResult:
                raise ArgumentError("successful provider observation requires its native retained result")
            self._catalog._validate(document["operation_id"], result.value,
                                    self._catalog.resolve(document["operation_id"]).to_dict()["output_schema"])
            expected = {"value_digest": content_digest(result.value), "receipt_digest": content_digest(result.receipt),
                        "empty_result_semantics": "source_specific; not_inferred_from_operation_success"}
            if _json(safe.get("result")) != _json(expected) or set(safe) != set(common) | {"started_at", "outcome", "result", "observation_digest"}:
                raise ArgumentError("provider observation result or fields are inconsistent")
        elif outcome == "adapter_or_schema_failed":
            if result is not None or safe.get("result") is not None or safe.get("failure_detail") != "sanitized; inspect application-owned source failure evidence separately" or set(safe) != set(common) | {"started_at", "outcome", "result", "failure_detail", "observation_digest"}:
                raise ArgumentError("failed provider observation must not invent a result")
        else:
            raise ArgumentError("provider observation outcome is unsupported")
        self._check_binding()
        return {"state": "retained_metadata_and_value_digest_consistent", "outcome": outcome,
                "observation_digest": safe["observation_digest"], "source_calls": 0, "model_calls": 0,
                "network_authenticity": "unassessed", "live_acquisition_reverified": False,
                "scientific_support": "unassessed", "continuous_availability": "unmeasured"}


def _component_digest(name: str) -> str:
    _identifier(name)
    try:
        module = import_module("." + name, __package__)
        path = Path(module.__file__)
        with path.open("rb") as stream:
            raw = stream.read(4_000_001)
        if len(raw) > 4_000_000:
            raise OSError
        return hashlib.sha256(raw).hexdigest()
    except (OSError, TypeError, AttributeError, ImportError):
        raise ArgumentError("reviewed provider component source is unavailable or exceeds its bound") from None


def _reviewed_catalog() -> WorkflowOperationCatalog:
    from .reviewed_research_sources import register_research_source_operations
    from .reviewed_gdc_gene_mutation_retrieval import register_gdc_gene_mutation_operations
    from .reviewed_gdc_ssm_coverage_retrieval import register_gdc_ssm_coverage_operations
    from .reviewed_ensembl_gene_retrieval import register_ensembl_gene_operations
    from .reviewed_uniprot_retrieval import register_uniprot_operations
    from .reviewed_reactome_retrieval import register_reactome_operations
    from .reviewed_hgnc_gene_retrieval import register_hgnc_gene_operations
    from .reviewed_nci_thesaurus_retrieval import register_nci_thesaurus_operations
    catalog = builtin_workflow_operation_catalog()
    for register in (register_research_source_operations, register_gdc_gene_mutation_operations, register_gdc_ssm_coverage_operations,
                     register_ensembl_gene_operations, register_uniprot_operations, register_reactome_operations,
                     register_hgnc_gene_operations, register_nci_thesaurus_operations):
        register(catalog)
    return catalog


# These entries name reviewed code paths; adding an endpoint, alias or generated descriptor never
# creates an entry. Grouping PubMed/Gene and both GDC readers prevents their double-counting.
_SERVICE_RECORDS = (
    ("ncbi_eutils", "NCBI E-utilities", ("https://eutils.ncbi.nlm.nih.gov",), "PubMed and NCBI Gene use the same reviewed E-utilities authority; one API service group."),
    ("europe_pmc", "Europe PMC", ("https://www.ebi.ac.uk",), "Europe PMC API path reviewed separately from other EBI-hosted APIs; organizational independence unassessed."),
    ("clinical_trials", "ClinicalTrials.gov", ("https://clinicaltrials.gov",), "Fixed ClinicalTrials.gov v2 public registry API; no enrollment or clinical decisions."),
    ("gdc", "NCI Genomic Data Commons", ("https://api.gdc.cancer.gov",), "Project summaries, fixed TP53 mutation counts and project SSM-data coverage share the GDC API; one API service group."),
    ("cbioportal", "cBioPortal", ("https://www.cbioportal.org",), "Fixed public study/profile catalogue API; no individual or assay values."),
    ("open_targets", "Open Targets Platform", ("https://api.platform.opentargets.org",), "Fixed public association metadata GraphQL API; read-only request despite HTTP POST."),
    ("gwas_catalog", "GWAS Catalog", ("https://www.ebi.ac.uk",), "GWAS API path reviewed separately from Europe PMC; common authority does not establish independent operation."),
    ("openalex", "OpenAlex", ("https://api.openalex.org",), "Fixed public works first-page metadata reader."),
    ("crossref", "Crossref", ("https://api.crossref.org",), "Fixed public bibliographic works first-page metadata reader."),
    ("zenodo", "Zenodo", ("https://zenodo.org",), "Fixed published record first-page metadata reader; no repository files."),
    ("ensembl", "Ensembl REST", ("https://rest.ensembl.org",), "Fixed public human/core TP53 identity and Entrez candidates; no validated biological crosswalk."),
    ("uniprot", "UniProt", ("https://rest.uniprot.org",), "Fixed public reviewed human P04637 name annotations; no sequence/variant data."),
    ("reactome", "Reactome", ("https://reactome.org",), "Fixed public human P04637 lower-level pathway annotations; no glioma activity inference."),
    ("hgnc", "HGNC REST", ("https://rest.genenames.org",), "Fixed public TP53 native gene identifier/alias metadata; no validated scientific equivalence."),
    ("nci_evs", "NCI EVS Thesaurus", ("https://api-evsrest.nci.nih.gov",), "Fixed NCIt glioma concept API, reviewed separately from GDC; shared NCI affiliation does not establish independence."),
)
_READER_RECORDS = (
    ("pubmed.search", "ncbi_eutils", "reviewed_pubmed_retrieval", "GLIOMA_WORKFLOW_BUILDER"),
    ("ncbi_gene.summary", "ncbi_eutils", "reviewed_ncbi_gene_retrieval", "GLIOMA_MOLECULAR_WORKFLOW"),
    ("europe_pmc.search", "europe_pmc", "reviewed_europe_pmc_retrieval", "NLP_WORKFLOW_BUILDER"),
    ("clinical_trials.search", "clinical_trials", "reviewed_clinical_trials_retrieval", "GLIOMA_WORKFLOW_BUILDER"),
    ("gdc.projects", "gdc", "reviewed_gdc_project_retrieval", "GLIOMA_WORKFLOW_BUILDER"),
    ("gdc.gene_mutation_counts", "gdc", "reviewed_gdc_gene_mutation_retrieval", "GDC_GENE_MUTATION_COUNTS"),
    ("gdc.ssm_project_coverage", "gdc", "reviewed_gdc_ssm_coverage_retrieval", "GDC_SSM_COVERAGE"),
    ("cbioportal.studies", "cbioportal", "reviewed_cbioportal_study_retrieval", "GLIOMA_MOLECULAR_WORKFLOW"),
    ("open_targets.associations", "open_targets", "reviewed_open_targets_retrieval", "GLIOMA_MOLECULAR_WORKFLOW"),
    ("gwas_catalog.associations", "gwas_catalog", "reviewed_gwas_catalog_retrieval", "GLIOMA_MOLECULAR_WORKFLOW"),
    ("openalex.search_works", "openalex", "reviewed_research_sources", "RESEARCH_SOURCE_INTEGRATIONS"),
    ("crossref.search_works", "crossref", "reviewed_research_sources", "RESEARCH_SOURCE_INTEGRATIONS"),
    ("zenodo.search_records", "zenodo", "reviewed_research_sources", "RESEARCH_SOURCE_INTEGRATIONS"),
    ("ensembl.gene_references", "ensembl", "reviewed_ensembl_gene_retrieval", "ENSEMBL_GENE_REFERENCES"),
    ("uniprot.protein_annotation", "uniprot", "reviewed_uniprot_retrieval", "UNIPROT_ANNOTATIONS"),
    ("reactome.pathway_annotations", "reactome", "reviewed_reactome_retrieval", "REACTOME_PATHWAYS"),
    ("hgnc.gene_references", "hgnc", "reviewed_hgnc_gene_retrieval", "HGNC_GENE_REFERENCES"),
    ("nci_thesaurus.glioma_ontology", "nci_evs", "reviewed_nci_thesaurus_retrieval", "NCI_THESAURUS"),
)


def reviewed_workflow_provider_registry(catalog: WorkflowOperationCatalog, *,
        evidence_references: Sequence[WorkflowProviderEvidenceReference] = ()) -> WorkflowProviderRegistry:
    """Bind eighteen actual reviewed public readers without enabling or dispatching any.

    Optional readers stay unregistered in the caller's catalogue. Unknown OpenAPI/MCP/local
    operations remain explicitly unreviewed rather than inheriting provider identity from an ID.
    """
    reviewed = _reviewed_catalog()
    providers = tuple(WorkflowProviderIdentity(*record) for record in _SERVICE_RECORDS)
    shared = {name: _component_digest(name) for name in (
        "workflow_provider_registry", "workflow_integrations", "authoring", "tooling", "domain_tools")}
    readers = []
    for operation_id, provider_id, module_name, document in _READER_RECORDS:
        components = shared | {module_name: _component_digest(module_name)}
        if provider_id in {"uniprot", "reactome", "hgnc", "nci_evs"} or operation_id == "gdc.ssm_project_coverage":
            components["reviewed_public_http"] = _component_digest("reviewed_public_http")
        readers.append(WorkflowReaderContract(operation_id, provider_id, reviewed.resolve(operation_id).to_dict(),
                       components, "docs/" + document + ".md"))
    return WorkflowProviderRegistry(catalog, providers=providers, readers=readers, reviewed_catalog=reviewed,
                                    evidence_references=evidence_references)


__all__ = ["WORKFLOW_PROVIDER_REGISTRY_SCHEMA", "WORKFLOW_PROVIDER_MANIFEST_VERSION",
    "WORKFLOW_PROVIDER_REQUEST_SCHEMA", "WORKFLOW_PROVIDER_OBSERVATION_SCHEMA",
    "WorkflowProviderIdentity", "WorkflowReaderContract", "WorkflowProviderEvidenceReference",
    "WorkflowProviderRequest", "WorkflowProviderExecution", "WorkflowProviderDispatchError",
    "WorkflowProviderRegistry", "reviewed_workflow_provider_registry"]
