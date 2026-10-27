"""Single-call local NLP proposals for mandatory workflow declarations.

This application extension has no blueprint module ID. A model chooses task excerpts,
actions and literals; the host resolves exact excerpt occurrences to character offsets.
Resolution is mechanical provenance, not semantic repair or proof of complete extraction.
Every candidate remains model_candidate and requires separate caller acceptance. No plan,
source invocation, automatic approval, retry or fallback is available in this module.
"""
from __future__ import annotations

from dataclasses import dataclass
import json
import re
from typing import Any, Mapping, Sequence
from urllib.parse import urlsplit

from .authoring import canonical_json, content_digest
from .llm_runtime import LLMRuntime, ProviderRequest
from .workflow_builder import (
    MAX_WORKFLOW_PLANNING_CONTEXT_BYTES, WorkflowValidationError,
    _assert_workflow_provider_binding, _capture_workflow_provider_binding,
    _WorkflowProviderInvocationGuard,
)
from .workflow_integrations import WorkflowOperationCatalog
from .workflow_intent_obligations import (
    WorkflowIntentObligations, WorkflowIntentObligationError,
    compile_workflow_intent_obligations, validate_workflow_intent_obligations,
    _task, _unique_object,
)

WORKFLOW_INTENT_EXTRACTION_SCHEMA = "aurora-workflow-intent-extraction/0.1"
WORKFLOW_INTENT_EXTRACTION_PROTOCOL = "aurora-workflow-intent-extraction-protocol/0.2"
_LEGACY_EXTRACTION_PROTOCOL = "aurora-workflow-intent-extraction-protocol/0.1"
MAX_INTENT_EXTRACTION_ROWS = 32
MAX_INTENT_EXTRACTION_RESPONSE_BYTES = 128_000
MAX_INTENT_EXTRACTION_RESULT_BYTES = 512_000


class WorkflowIntentExtractionError(WorkflowValidationError):
    """Invalid local extraction configuration or a changed invocation boundary."""


def _request_value(task: str, catalog_digest: str, model: str, operations: Sequence[Any],
                   protocol: str) -> dict[str, Any]:
    """Reconstruct each version's actual contract exposure without upgrading old records."""
    if protocol not in (_LEGACY_EXTRACTION_PROTOCOL, WORKFLOW_INTENT_EXTRACTION_PROTOCOL):
        raise WorkflowIntentExtractionError("intent extraction protocol is unsupported")
    descriptors = [operation.to_dict() for operation in sorted(operations, key=lambda item: item.operation_id)]
    visible = ([{key: descriptor[key] for key in ("operation_id", "description", "input_schema", "read_only")}
                for descriptor in descriptors] if protocol == _LEGACY_EXTRACTION_PROTOCOL else descriptors)
    rules = [
        "Propose mandatory task obligations, not a workflow or source observations. Nothing has executed.",
        "Retain every essential action, including unavailable capabilities. Do not omit a requested write just because only readers are available.",
        "Use exact operation identities when justified. Preserve unmapped, unavailable or ambiguous work as unresolved_action with a reason; never substitute a supported action for essential unavailable work.",
        "Preserve essential literal arguments as argument_equals even when they conflict with a native const or enum. Never silently replace a requested literal with a schema default or allowed alternative.",
        "An output=true required_operation means the user requires a final result from that operation; output=false leaves output selection unconstrained.",
        "For each span choose exact nonempty task text and its zero-based occurrence among overlapping exact matches. The host calculates character offsets. Do not normalize, paraphrase or trim excerpts.",
        "Give every row a distinct bounded ID. A repeated operation does not express distinct required invocation instances in this version.",
        "Use clarify or refuse when needed. Candidate rows are unreviewed model interpretations, never acceptance, authorization, scientific verification or proof of complete task coverage.",
        "No credentials, clinical actions, patient data, source dispatch, plan steps, replacement requirements or authority labels.",
    ]
    if protocol == WORKFLOW_INTENT_EXTRACTION_PROTOCOL:
        rules[5:5] = [
            "Each complete native operation descriptor includes input_schema for arguments and output_schema for its possible future returned value. An output contract is not a source result or proof that a value exists.",
            "An output=true required_operation requests the operation's whole future returned value as a final plan output. A requested returned artifact does not require an input flag unless the native input contract defines one. Do not invent arguments from output fields or assume an implicit result wrapper.",
            "Descriptor implementation and auth_mode are catalogue declarations, not execution grants, measured source availability or scientific validity. Requested output details not established by the contract remain unresolved; do not invent observed values or repair the task.",
        ]
    instructions = {"protocol": protocol, "task": task, "catalog_digest": catalog_digest,
                    "operations": visible, "rules": rules}
    return {"model": model, "messages": [{"role": "system", "content": canonical_json(instructions)}],
            "response_schema": _response_schema(len(task)), "require_json": True,
            "max_output_tokens": 4096, "temperature": 0}


def _object(properties: dict[str, Any]) -> dict[str, Any]:
    return {"type": "object", "properties": properties,
            "required": list(properties), "additionalProperties": False}


def _response_schema(task_length: int) -> dict[str, Any]:
    identity = {"type": "string", "pattern": "^[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}$"}
    span = _object({"text": {"type": "string", "minLength": 1, "maxLength": task_length},
                    "occurrence": {"type": "integer", "minimum": 0, "maximum": task_length - 1}})
    common = {"id": identity, "span": span}
    required = _object({**common, "kind": {"const": "required_operation"},
                        "operation": identity, "output": {"type": "boolean"}})
    equality = _object({**common, "kind": {"const": "argument_equals"}, "operation": identity,
                        "path": {"type": "string", "maxLength": 512}, "value": {}})
    unresolved = _object({**common, "kind": {"const": "unresolved_action"},
                          "reason": {"type": "string", "minLength": 1, "maxLength": 512}})
    candidate = _object({"action": {"const": "candidate"}, "obligations": {
        "type": "array", "minItems": 1, "maxItems": MAX_INTENT_EXTRACTION_ROWS,
        "items": {"anyOf": [required, equality, unresolved]}}})
    terminal = _object({"action": {"enum": ["clarify", "refuse"]},
                        "message": {"type": "string", "minLength": 1, "maxLength": 2000}})
    return {"anyOf": [candidate, terminal]}


def _interpret_response(task: str, text: str) -> dict[str, Any]:
    result: dict[str, Any] = {"obligations": None, "span_resolution": []}
    try:
        decision = json.loads(text, object_pairs_hook=_unique_object)
        if type(decision) is not dict:
            raise WorkflowIntentObligationError("intent extraction decision must be an object")
        if decision.get("action") in {"clarify", "refuse"}:
            if (set(decision) != {"action", "message"} or type(decision["message"]) is not str
                    or not decision["message"].strip() or len(decision["message"]) > 2000):
                raise WorkflowIntentObligationError("intent extraction terminal fields are invalid")
            result.update(status="needs_clarification" if decision["action"] == "clarify" else "refused",
                          message=decision["message"])
        else:
            if (set(decision) != {"action", "obligations"} or decision["action"] != "candidate"
                    or type(decision["obligations"]) is not list
                    or not 1 <= len(decision["obligations"]) <= MAX_INTENT_EXTRACTION_ROWS):
                raise WorkflowIntentObligationError("intent extraction candidate fields are invalid")
            rows, resolution = [], []
            for row in decision["obligations"]:
                if type(row) is not dict or type(row.get("span")) is not dict:
                    raise WorkflowIntentObligationError("intent extraction row needs an exact span selector")
                selector = row["span"]
                if (set(selector) != {"text", "occurrence"} or type(selector["text"]) is not str
                        or not selector["text"].strip() or type(selector["occurrence"]) is not int
                        or not 0 <= selector["occurrence"] < len(task)):
                    raise WorkflowIntentObligationError("intent extraction span selector is invalid")
                start = -1
                for _ in range(selector["occurrence"] + 1):
                    start = task.find(selector["text"], start + 1)
                    if start == -1:
                        raise WorkflowIntentObligationError("intent extraction excerpt occurrence is absent")
                span = {"start": start, "end": start + len(selector["text"]), "text": selector["text"]}
                rows.append({**row, "span": span})
                resolution.append({"obligation_id": row.get("id"), "selector": selector, "span": span})
            obligations = compile_workflow_intent_obligations(task, rows, interpretation_basis="model_candidate")
            result.update(status="candidate", obligations=obligations.to_dict(), span_resolution=resolution)
    except (ValueError, TypeError, KeyError, RecursionError, UnicodeError):
        result.update(status="rejected", reason="invalid_candidate_or_exact_task_binding")
    return result


@dataclass(frozen=True, slots=True)
class WorkflowIntentExtraction:
    """Unreviewed candidate or explicit noncandidate outcome; never executable authority."""

    _encoded: str

    def to_dict(self) -> dict[str, Any]:
        return json.loads(self._encoded)

    @property
    def status(self) -> str:
        return self.to_dict()["status"]

    @property
    def obligations(self) -> WorkflowIntentObligations | None:
        data = self.to_dict()
        if data["status"] != "candidate":
            return None
        return validate_workflow_intent_obligations(data["obligations"], task=data["task"])


def validate_workflow_intent_extraction(
    value: WorkflowIntentExtraction | Mapping[str, Any], *, task: str,
    catalog: WorkflowOperationCatalog | None = None, operation_ids: Sequence[str] | None = None,
) -> WorkflowIntentExtraction:
    """Recompute retained-text interpretation, not model authorship or task completeness.

    A restamped result cannot pair different model text with native declarations/offsets.
    With a catalogue, request bindings use the exact retained protocol's context rules.
    Without one, request digests remain declarations. Neither proves model authorship.
    Omitted oversized text cannot be replayed; its omission is retained, never upgraded.
    """
    try:
        task = _task(task)
        if operation_ids is not None and catalog is None:
            raise ValueError("selection validation requires its catalogue")
        data = value.to_dict() if isinstance(value, WorkflowIntentExtraction) else dict(value)
        encoded = canonical_json(data)
        if len(encoded.encode("utf-8")) > MAX_INTENT_EXTRACTION_RESULT_BYTES:
            raise ValueError("result bound")
        data = json.loads(encoded)
        base = {"schema", "task", "task_digest", "catalog_digest", "provider", "model", "provider_config_digest",
                "request_digest", "response_schema_digest", "protocol", "visible_operations", "context_bytes_including_schema",
                "provider_invocations", "transport_attempt_policy", "actual_transport_attempts", "source_dispatches",
                "plan_created", "accepted", "task_interpretation_completeness", "scientific_validity", "raw_response",
                "obligations", "span_resolution", "status", "extraction_digest"}
        status = data["status"]
        expected_fields = base | ({"reason"} if status == "rejected" else {"message"} if status in {"needs_clarification", "refused"} else set())
        if set(data) != expected_fields or status not in {"candidate", "needs_clarification", "refused", "rejected"}:
            raise ValueError("result fields")
        protocol = data["protocol"]
        if (data["schema"] != WORKFLOW_INTENT_EXTRACTION_SCHEMA
                or protocol not in (_LEGACY_EXTRACTION_PROTOCOL, WORKFLOW_INTENT_EXTRACTION_PROTOCOL)
                or data["task"] != task or data["task_digest"] != content_digest({"task": task})
                or data["response_schema_digest"] != content_digest(_response_schema(len(task)))
                or data["extraction_digest"] != content_digest({key: child for key, child in data.items() if key != "extraction_digest"})
                or data["accepted"] is not False or data["plan_created"] is not False
                or type(data["source_dispatches"]) is not int or data["source_dispatches"] != 0
                or any(type(data[key]) is not int or data[key] != 1 for key in ("provider_invocations", "transport_attempt_policy"))
                or data["actual_transport_attempts"] != "not_independently_observed"
                or any(data[key] != "unassessed" for key in ("task_interpretation_completeness", "scientific_validity"))
                or type(data["context_bytes_including_schema"]) is not int
                or not 1 <= data["context_bytes_including_schema"] <= MAX_WORKFLOW_PLANNING_CONTEXT_BYTES
                or any(type(data[key]) is not str or not re.fullmatch(r"[a-f0-9]{64}", data[key])
                       for key in ("task_digest", "catalog_digest", "provider_config_digest", "request_digest", "response_schema_digest"))
                or type(data["model"]) is not str or not data["model"].strip() or len(data["model"]) > 200
                or type(data["provider"]) is not str or not data["provider"].strip() or len(data["provider"]) > 256):
            raise ValueError("result binding")
        visible = data["visible_operations"]
        if (type(visible) is not list or not 1 <= len(visible) <= 128
                or any(type(identity) is not str or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}", identity) for identity in visible)
                or visible != sorted(set(visible))):
            raise ValueError("visible operations")
        if catalog is not None:
            if not isinstance(catalog, WorkflowOperationCatalog) or catalog.catalog_digest != data["catalog_digest"]:
                raise ValueError("catalogue binding")
            if operation_ids is not None and (not isinstance(operation_ids, (list, tuple)) or not 1 <= len(operation_ids) <= 128
                    or any(type(identity) is not str for identity in operation_ids) or len(set(operation_ids)) != len(operation_ids)):
                raise ValueError("selection binding")
            expected = sorted(operation.operation_id for operation in catalog.operations()) if operation_ids is None else sorted(operation_ids)
            if expected != visible:
                raise ValueError("visibility binding")
            for identity in visible:
                catalog.resolve(identity)
            request = _request_value(task, catalog.catalog_digest, data["model"],
                                     [catalog.resolve(identity) for identity in visible], protocol)
            if (data["request_digest"] != content_digest(request)
                    or data["context_bytes_including_schema"] != len(canonical_json(request).encode("utf-8"))):
                raise ValueError("versioned request binding")
        raw = data["raw_response"]
        if (type(raw) is not dict or set(raw) != {"text", "text_digest", "utf8_bytes", "retention"}
                or type(raw["utf8_bytes"]) is not int or raw["utf8_bytes"] < 0
                or type(raw["text_digest"]) is not str or not re.fullmatch(r"[a-f0-9]{64}", raw["text_digest"])):
            raise ValueError("raw response")
        if raw["retention"] == "complete":
            if (type(raw["text"]) is not str or len(raw["text"].encode("utf-8")) != raw["utf8_bytes"]
                    or raw["utf8_bytes"] > MAX_INTENT_EXTRACTION_RESPONSE_BYTES
                    or raw["text_digest"] != content_digest({"text": raw["text"]})):
                raise ValueError("raw response binding")
            interpretation = _interpret_response(task, raw["text"])
        elif raw["retention"] == "omitted_exceeds_bound" and raw["text"] is None and raw["utf8_bytes"] > MAX_INTENT_EXTRACTION_RESPONSE_BYTES:
            interpretation = {"status": "rejected", "reason": "response_text_exceeds_retention_bound",
                              "obligations": None, "span_resolution": []}
        else:
            raise ValueError("raw retention")
        actual = {key: data[key] for key in interpretation}
        if canonical_json(actual) != canonical_json(interpretation):
            raise ValueError("raw response and native interpretation differ")
        return WorkflowIntentExtraction(encoded)
    except (ValueError, TypeError, KeyError, RecursionError, UnicodeError) as error:
        raise WorkflowIntentExtractionError("retained extraction does not match its exact task, text and declaration bindings") from error


class LocalWorkflowIntentExtractor:
    """Ask an explicitly registered credentialless local model for mandatory task rows.

    The selected catalogue is shown without silent ranking or truncation. Unavailable and
    unresolved work must remain visible. This instruction is not a completeness guarantee;
    a valid candidate can still omit or misinterpret a task and must remain unreviewed.
    """

    def __init__(self, catalog: WorkflowOperationCatalog, runtime: LLMRuntime, *, model: str,
                 provider: str = "ollama", max_visible_operations: int = 128) -> None:
        if not isinstance(catalog, WorkflowOperationCatalog) or not isinstance(runtime, LLMRuntime):
            raise WorkflowIntentExtractionError("intent extraction needs a catalogue and LLMRuntime")
        if not isinstance(model, str) or not model.strip() or len(model) > 200:
            raise WorkflowIntentExtractionError("intent extraction requires a bounded local model identity")
        if type(max_visible_operations) is not int or not 1 <= max_visible_operations <= 128:
            raise WorkflowIntentExtractionError("intent extraction visibility bound must be one to 128")
        self.catalog, self.runtime, self.model, self.provider = catalog, runtime, model, provider
        self.max_visible_operations = max_visible_operations
        self._local_provider_metadata()
        self._binding = _capture_workflow_provider_binding(runtime, provider)

    def _local_provider_metadata(self) -> None:
        metadata = next((row for row in self.runtime.provider_metadata() if row.get("provider") == self.provider), None)
        if (metadata is None or metadata.get("requires_credential") is not False
                or type(metadata.get("max_attempts")) is not int or metadata["max_attempts"] != 1):
            raise WorkflowIntentExtractionError("intent extraction requires a credentialless one-attempt local provider")
        if metadata.get("transport") != "in_memory":
            endpoint = metadata.get("base_url")
            if (metadata.get("transport") != "http" or not isinstance(endpoint, str)
                    or urlsplit(endpoint).hostname not in {"127.0.0.1", "localhost", "::1"}):
                raise WorkflowIntentExtractionError("intent extraction provider must be loopback or in-memory")

    def extract(self, task: str, *, approve_provider_call: bool = False,
                operation_ids: Sequence[str] | None = None) -> WorkflowIntentExtraction:
        """One approved SDK invocation; runtime/transport failures propagate without repair.

        Only a response actually returned by LLMRuntime is retained here. Runtime JSON/schema
        failures may happen before a response is returned and are not labelled candidate
        rejections. Oversized returned text retains its digest/size, never a truncated body.
        """
        if approve_provider_call is not True:
            raise WorkflowIntentExtractionError("intent extraction requires explicit provider approval")
        task = _task(task)
        catalog, runtime, provider, model = self.catalog, self.runtime, self.provider, self.model
        binding = self._binding
        maximum = self.max_visible_operations
        digest = catalog.catalog_digest

        def guard() -> None:
            if (self.catalog is not catalog or self.runtime is not runtime or self.provider != provider
                    or self.model != model or type(self.max_visible_operations) is not int
                    or self.max_visible_operations != maximum or self._binding is not binding
                    or catalog.catalog_digest != digest):
                raise WorkflowIntentExtractionError("intent extraction catalogue or configuration changed")
            self._local_provider_metadata()
            _assert_workflow_provider_binding(runtime, provider, binding)

        guard()
        if operation_ids is None:
            operations = list(catalog.operations())
        else:
            if (not isinstance(operation_ids, (list, tuple)) or not 1 <= len(operation_ids) <= 128
                    or any(type(value) is not str for value in operation_ids)
                    or len(set(operation_ids)) != len(operation_ids)):
                raise WorkflowIntentExtractionError("intent extraction requires a unique bounded operation selection")
            operations = [catalog.resolve(value) for value in operation_ids]
        if not operations or len(operations) > maximum:
            raise WorkflowIntentExtractionError("intent extraction refuses an empty or oversized visible catalogue")
        operations.sort(key=lambda operation: operation.operation_id)
        protocol = WORKFLOW_INTENT_EXTRACTION_PROTOCOL
        request_value = _request_value(task, digest, model, operations, protocol)
        context_bytes = len(canonical_json(request_value).encode("utf-8"))
        if context_bytes > MAX_WORKFLOW_PLANNING_CONTEXT_BYTES:
            raise WorkflowIntentExtractionError("intent extraction context exceeds its byte bound")
        schema = request_value["response_schema"]
        request = ProviderRequest(model=model, messages=tuple(request_value["messages"]), require_json=True, response_schema=schema,
                                  max_output_tokens=4096, temperature=0)
        class InvocationGuard(_WorkflowProviderInvocationGuard):
            # Runtime quota/effect callbacks occur after extract() preflight. Recheck the
            # whole selected boundary immediately before transport as well as afterwards.
            def before(self, metadata: Any) -> None:
                guard()
                super().before(metadata)

            def before_transport(self, metadata: Any) -> None:
                guard()
                super().before_transport(metadata)

            def after(self, metadata: Any, returned: Any, error: Any, elapsed_ms: float) -> None:
                guard()
                super().after(metadata, returned, error, elapsed_ms)

        response = runtime.invoke(provider, request, invocation_observer=InvocationGuard(runtime, provider, binding))
        guard()
        if response.provider != provider or response.model != model or response.tool_calls:
            raise WorkflowIntentExtractionError("intent extraction response changed its model/provider/tool boundary")
        if not isinstance(response.text, str):
            raise WorkflowIntentExtractionError("intent extraction response text must be a string")
        encoded_response = response.text.encode("utf-8")
        retained = len(encoded_response) <= MAX_INTENT_EXTRACTION_RESPONSE_BYTES
        value = {"schema": WORKFLOW_INTENT_EXTRACTION_SCHEMA, "task": task,
            "task_digest": content_digest({"task": task}), "catalog_digest": digest,
            "provider": provider, "model": model, "provider_config_digest": binding.config_digest,
            "request_digest": content_digest(request_value), "response_schema_digest": content_digest(schema),
            "protocol": protocol, "visible_operations": [operation.operation_id for operation in operations],
            "context_bytes_including_schema": context_bytes, "provider_invocations": 1,
            "transport_attempt_policy": 1, "actual_transport_attempts": "not_independently_observed",
            "source_dispatches": 0, "plan_created": False, "accepted": False,
            "task_interpretation_completeness": "unassessed", "scientific_validity": "unassessed",
            "raw_response": {"text": response.text if retained else None,
                "text_digest": content_digest({"text": response.text}), "utf8_bytes": len(encoded_response),
                "retention": "complete" if retained else "omitted_exceeds_bound"},
            "obligations": None, "span_resolution": []}
        if not retained:
            value.update(status="rejected", reason="response_text_exceeds_retention_bound")
        else:
            value.update(_interpret_response(task, response.text))
        guard()
        value["extraction_digest"] = content_digest(value)
        encoded = canonical_json(value)
        if len(encoded.encode("utf-8")) > MAX_INTENT_EXTRACTION_RESULT_BYTES:
            raise WorkflowIntentExtractionError("intent extraction result exceeds its complete retention bound")
        return WorkflowIntentExtraction(encoded)
