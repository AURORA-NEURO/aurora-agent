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
from typing import Any, Sequence
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
WORKFLOW_INTENT_EXTRACTION_PROTOCOL = "aurora-workflow-intent-extraction-protocol/0.1"
MAX_INTENT_EXTRACTION_ROWS = 32
MAX_INTENT_EXTRACTION_RESPONSE_BYTES = 128_000
MAX_INTENT_EXTRACTION_RESULT_BYTES = 512_000


class WorkflowIntentExtractionError(WorkflowValidationError):
    """Invalid local extraction configuration or a changed invocation boundary."""


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
        visible = [{key: operation.to_dict()[key] for key in
                    ("operation_id", "description", "input_schema", "read_only")} for operation in operations]
        schema = _response_schema(len(task))
        instructions = {
            "protocol": WORKFLOW_INTENT_EXTRACTION_PROTOCOL,
            "task": task, "catalog_digest": digest, "operations": visible,
            "rules": [
                "Propose mandatory task obligations, not a workflow or source observations. Nothing has executed.",
                "Retain every essential action, including unavailable capabilities. Do not omit a requested write just because only readers are available.",
                "Use exact operation identities when justified. Preserve unmapped, unavailable or ambiguous work as unresolved_action with a reason; never substitute a supported action for essential unavailable work.",
                "Preserve essential literal arguments as argument_equals even when they conflict with a native const or enum. Never silently replace a requested literal with a schema default or allowed alternative.",
                "An output=true required_operation means the user requires a final result from that operation; output=false leaves output selection unconstrained.",
                "For each span choose exact nonempty task text and its zero-based occurrence among overlapping exact matches. The host calculates character offsets. Do not normalize, paraphrase or trim excerpts.",
                "Give every row a distinct bounded ID. A repeated operation does not express distinct required invocation instances in this version.",
                "Use clarify or refuse when needed. Candidate rows are unreviewed model interpretations, never acceptance, authorization, scientific verification or proof of complete task coverage.",
                "No credentials, clinical actions, patient data, source dispatch, plan steps, replacement requirements or authority labels.",
            ],
        }
        messages = ({"role": "system", "content": canonical_json(instructions)},)
        request_value = {"model": model, "messages": list(messages), "response_schema": schema,
                         "require_json": True, "max_output_tokens": 4096, "temperature": 0}
        context_bytes = len(canonical_json(request_value).encode("utf-8"))
        if context_bytes > MAX_WORKFLOW_PLANNING_CONTEXT_BYTES:
            raise WorkflowIntentExtractionError("intent extraction context exceeds its byte bound")
        request = ProviderRequest(model=model, messages=messages, require_json=True, response_schema=schema,
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
            "protocol": WORKFLOW_INTENT_EXTRACTION_PROTOCOL, "visible_operations": [row["operation_id"] for row in visible],
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
            try:
                decision = json.loads(response.text, object_pairs_hook=_unique_object)
                if type(decision) is not dict:
                    raise WorkflowIntentObligationError("intent extraction decision must be an object")
                if decision.get("action") in {"clarify", "refuse"}:
                    if (set(decision) != {"action", "message"} or type(decision["message"]) is not str
                            or not decision["message"].strip() or len(decision["message"]) > 2000):
                        raise WorkflowIntentObligationError("intent extraction terminal fields are invalid")
                    value.update(status="needs_clarification" if decision["action"] == "clarify" else "refused",
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
                    value.update(status="candidate", obligations=obligations.to_dict(), span_resolution=resolution)
            except (ValueError, TypeError, KeyError, RecursionError, UnicodeError):
                value.update(status="rejected", reason="invalid_candidate_or_exact_task_binding")
        guard()
        value["extraction_digest"] = content_digest(value)
        encoded = canonical_json(value)
        if len(encoded.encode("utf-8")) > MAX_INTENT_EXTRACTION_RESULT_BYTES:
            raise WorkflowIntentExtractionError("intent extraction result exceeds its complete retention bound")
        return WorkflowIntentExtraction(encoded)
