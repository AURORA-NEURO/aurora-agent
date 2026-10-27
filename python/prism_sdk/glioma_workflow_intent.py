"""Compile a bounded, model-free glioma request grammar into the reviewed evidence graph.

This is a repository-defined application grammar, not a general natural-language interpreter
or a blueprint implementation. Every nonempty clause must match an audited form. Unsupported
clauses survive as original-text spans and prevent compilation; a valid graph never silently
stands in for a more specific request. Interpretation grants no provider or source authority.
"""
from __future__ import annotations

from dataclasses import dataclass
import json
import re
from typing import Any, Mapping

from .authoring import content_digest
from .errors import ArgumentError
from .glioma_workflow import (
    GLIOMA_WORKFLOW_SCOPE, glioma_evidence_workflow_proposal,
)
from .workflow_builder import WorkflowPlan, WorkflowValidationError, compile_workflow_plan
from .workflow_integrations import WorkflowOperationCatalog
from .workflow_requirements import compile_workflow_requirements


GLIOMA_WORKFLOW_INTENT_SCHEMA = "aurora-glioma-workflow-intent/0.1"
MAX_GLIOMA_INTENT_TASK_BYTES = 8_000
MAX_GLIOMA_INTENT_CLAUSES = 32
_SOURCE_ORDER = ("pubmed.search", "clinical_trials.search", "gdc.projects")
_ATOM = r"\b(?:pubmed|clinicaltrials\.gov|clinical\s+trials|trial\s+registry|trials|tcga(?:\s+projects)?|gdc(?:\s+projects)?)\b"
_LIST = rf"{_ATOM}(?:\s*(?:,\s*(?:and\s+)?|and\s+|\+\s*){_ATOM})*"
_NOUNS = r"(?:\s+(?:public|research|evidence|metadata|sources|workflow|review|handoff))*"
_HEAD = rf"(?:review|retrieve|find|collect|gather|research|compile|build|prepare)\s+(?:the\s+)?(?:public\s+)?glioma{_NOUNS}"
_MAIN = re.compile(rf"{_HEAD}(?:\s+(?:using|from|with)\s+(?P<sources>{_LIST})(?P<only>\s+only)?)?(?:\s+(?:without|excluding)\s+(?P<exclude>{_LIST}))?", re.I)
_REVERSED_MAIN = re.compile(rf"(?:review|retrieve|find|collect|gather)\s+(?P<sources>{_LIST})(?P<only>\s+only)?\s+(?:for|on|about)\s+(?:public\s+)?glioma{_NOUNS}", re.I)
_INCLUDE = re.compile(rf"(?P<prefix>use|using|include|include\s+only|only\s+use|sources:)\s+(?P<sources>{_LIST})(?P<only>\s+only)?", re.I)
_EXCLUDE = re.compile(rf"(?:exclude|without|no|do\s+not\s+use|don't\s+use)\s+(?P<sources>{_LIST})", re.I)
_WORDS = {word: index for index, word in enumerate(("zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"))}
_NUMBER = r"(?:[0-9]+|zero|one|two|three|four|five|six|seven|eight|nine|ten)"
_PUBMED_LIMIT = re.compile(rf"(?:limit\s+)?pubmed\s+(?:(?:record\s+)?limit(?:\s+to)?\s+|to\s+)(?P<number>{_NUMBER})(?:\s+(?:records|papers|citations))?", re.I)
_PUBMED_COUNT = re.compile(rf"(?P<number>{_NUMBER})\s+pubmed\s+(?:records|papers|citations)", re.I)
_TRIAL_ATOM = r"(?:clinicaltrials\.gov|clinical\s+trials|trial\s+registry|trials)"
_TRIAL_SIZE = re.compile(rf"{_TRIAL_ATOM}\s+page\s+size\s+(?P<number>{_NUMBER})", re.I)
_TRIAL_PAGES = re.compile(rf"{_TRIAL_ATOM}\s+(?:max(?:imum)?\s+pages|page\s+limit)\s+(?P<number>{_NUMBER})", re.I)
_TRIAL_LIMIT = re.compile(rf"(?:limit\s+)?{_TRIAL_ATOM}\s+(?:limit(?:\s+to)?\s+|to\s+)(?P<number>{_NUMBER})(?:\s+records)?", re.I)
_TRIAL_COUNT = re.compile(rf"(?P<number>{_NUMBER})\s+(?:trial\s+records|clinical\s+trial\s+records)", re.I)
_HANDOFF = re.compile(r"(?:reconcile(?:\s+(?:the\s+)?(?:sources|evidence|metadata))?(?:\s+(?:and|then)\s+(?:prepare\s+)?(?:a\s+)?(?:human\s+)?review(?:\s+handoff)?)?|(?:prepare\s+)?(?:a\s+)?(?:human\s+)?review\s+handoff|review)", re.I)
_BOUNDARY = re.compile(r"(?:public\s+metadata\s+only|no\s+patient\s+data|no\s+clinical\s+(?:claims|recommendations)|human\s+review\s+required)", re.I)
_INTERPRETATION = "bounded_clause_grammar_only;fixed_glioma_lanes;question_not_used_as_source_query;scientific_validity_unassessed"


@dataclass(frozen=True, slots=True)
class GliomaWorkflowIntent:
    """An immutable interpretation receipt retaining the exact caller task for review."""

    _encoded: str

    def to_dict(self) -> dict[str, Any]:
        return json.loads(self._encoded)

    @property
    def intent_digest(self) -> str:
        return self.to_dict()["intent_digest"]

    @property
    def status(self) -> str:
        return self.to_dict()["status"]


def _source_ids(text: str) -> list[str]:
    result = []
    for match in re.finditer(_ATOM, text, re.I):
        token = match.group().lower()
        result.append("pubmed.search" if token == "pubmed" else "gdc.projects" if token.startswith(("tcga", "gdc")) else "clinical_trials.search")
    return result


def _clauses(task: str) -> list[tuple[int, int, str]]:
    spans = []
    start = 0
    # The dot inside ClinicalTrials.gov stays part of its alias.
    for delimiter in re.finditer(r"[;\n]|\.(?=\s|$)", task):
        spans.append((start, delimiter.start()))
        start = delimiter.end()
    spans.append((start, len(task)))
    result = []
    for left, right in spans:
        while left < right and task[left].isspace():
            left += 1
        while right > left and task[right - 1].isspace():
            right -= 1
        if left < right:
            result.append((left, right, task[left:right]))
    return result


def _unresolved_reason(text: str) -> str:
    checks = (
        (r"\b(?:send|email|publish|upload|delete|enroll|enrol|schedule|purchase|deploy)\b", "effectful_request"),
        (r"\b(?:diagnos\w*|prescri\w*|dose|dosage|patient|treat(?:ment)?\s+(?:me|my|a\s+patient))\b", "clinical_or_patient_request"),
        (r"\b(?:19[0-9]{2}|20[0-9]{2}|since|before|after|recent|latest|year|years|month|months|date|dates)\b", "unsupported_date_filter"),
        (r"\b(?:idh[12]?|mgmt|h3|egfr|tert|glioblastoma|astrocytoma|oligodendroglioma|pediatric|paediatric|adult)\b|1p\s*/\s*19q", "unsupported_subtype_filter"),
        (r"\b(?:intervention|drug|temozolomide|radiotherapy|immunotherapy|vaccine|treatment|therapy)\b", "unsupported_intervention_filter"),
        (r"\b(?:no|not|without|except|exclude|only)\b|don't", "unsupported_negation_or_scope"),
    )
    return next((reason for pattern, reason in checks if re.search(pattern, text, re.I)), "unsupported_clause")


def interpret_glioma_workflow_request(task: str) -> GliomaWorkflowIntent:
    """Interpret only the documented clause grammar; unresolved text always prevents readiness."""
    try:
        valid = isinstance(task, str) and bool(task.strip()) and "\x00" not in task and len(task.encode("utf-8")) <= MAX_GLIOMA_INTENT_TASK_BYTES
    except UnicodeError:
        valid = False
    if not valid:
        raise ArgumentError("glioma intent requires a nonempty task within 8000 UTF-8 bytes")
    clauses = _clauses(task)
    if len(clauses) > MAX_GLIOMA_INTENT_CLAUSES:
        raise ArgumentError("glioma intent exceeds its 32-clause bound")
    unresolved: list[dict[str, Any]] = []
    included: set[str] = set()
    excluded: set[str] = set()
    exclusive: set[str] | None = None
    specified: dict[str, int] = {}
    limits = {"literature_limit": 10, "trial_page_size": 25, "trial_max_pages": 2}
    main_seen = False

    def issue(span: tuple[int, int, str], reason: str) -> None:
        row = {"start": span[0], "end": span[1], "text": span[2], "reason": reason}
        if row not in unresolved:
            unresolved.append(row)

    def select(text: str, span: tuple[int, int, str], *, exclusion: bool = False, only: bool = False) -> None:
        nonlocal exclusive
        identities = _source_ids(text)
        if len(identities) != len(set(identities)):
            issue(span, "duplicate_source")
        values = set(identities)
        if exclusion:
            if values & included:
                issue(span, "conflicting_source_selection")
            excluded.update(values)
        else:
            if values & excluded or (exclusive is not None and values - exclusive):
                issue(span, "conflicting_source_selection")
            if only:
                if included - values or (exclusive is not None and values != exclusive):
                    issue(span, "conflicting_source_selection")
                exclusive = values
            included.update(values)

    def set_limit(field: str, number: str | int, span: tuple[int, int, str]) -> None:
        if isinstance(number, str) and number.isdecimal() and len(number) > 3:
            issue(span, "limit_outside_adapter_bound")
            return
        value = _WORDS[number.lower()] if isinstance(number, str) and number.lower() in _WORDS else int(number)
        if field in specified and specified[field] != value:
            issue(span, "conflicting_limit")
        maximum = {"literature_limit": 50, "trial_page_size": 100, "trial_max_pages": 5}[field]
        if not 1 <= value <= maximum:
            issue(span, "limit_outside_adapter_bound")
        specified[field] = value

    for span in clauses:
        text = span[2]
        match = _MAIN.fullmatch(text) or _REVERSED_MAIN.fullmatch(text)
        if match:
            main_seen = True
            values = match.groupdict()
            if values.get("sources"):
                select(values["sources"], span, only=bool(values.get("only")))
            if values.get("exclude"):
                select(values["exclude"], span, exclusion=True)
            continue
        match = _INCLUDE.fullmatch(text)
        if match:
            select(match["sources"], span, only=bool(match["only"]) or "only" in match["prefix"].lower())
            continue
        match = _EXCLUDE.fullmatch(text)
        if match:
            select(match["sources"], span, exclusion=True)
            continue
        matched_limit = False
        for pattern, field in ((_PUBMED_LIMIT, "literature_limit"), (_PUBMED_COUNT, "literature_limit"),
                               (_TRIAL_SIZE, "trial_page_size"), (_TRIAL_PAGES, "trial_max_pages"),
                               (_TRIAL_LIMIT, "trial_page_size"), (_TRIAL_COUNT, "trial_page_size")):
            match = pattern.fullmatch(text)
            if match:
                set_limit(field, match["number"], span)
                if pattern in (_TRIAL_LIMIT, _TRIAL_COUNT):
                    set_limit("trial_max_pages", 1, span)
                matched_limit = True
                break
        if matched_limit or _HANDOFF.fullmatch(text) or _BOUNDARY.fullmatch(text):
            continue
        issue(span, _unresolved_reason(text))
    if not main_seen:
        issue((0, len(task), task), "missing_supported_glioma_request")
    selected = (included if included else set(_SOURCE_ORDER)) - excluded
    if not selected:
        issue((0, len(task), task), "empty_source_selection")
    for field in specified:
        operation = "pubmed.search" if field == "literature_limit" else "clinical_trials.search"
        if operation not in selected:
            issue((0, len(task), task), "limit_for_unselected_source")
    limits.update(specified)
    defaults: list[dict[str, Any]] = []
    if not included:
        defaults.append({"field": "source_operations", "value": [op for op in _SOURCE_ORDER if op in selected]})
    for field, value in limits.items():
        active = "pubmed.search" in selected if field == "literature_limit" else "clinical_trials.search" in selected
        if active and field not in specified:
            defaults.append({"field": field, "value": value})
    defaults.extend([{"field": "base_output", "value": "human_review_handoff"},
                     {"field": "dispatch_attempts_per_operation", "value": 1}])
    descriptor = {"schema": GLIOMA_WORKFLOW_INTENT_SCHEMA, "task": task,
        "task_digest": content_digest({"task": task}), "status": "needs_clarification" if unresolved else "ready",
        "scope": GLIOMA_WORKFLOW_SCOPE, "source_operations": [op for op in _SOURCE_ORDER if op in selected],
        **limits, "explicit_defaults": defaults, "unresolved_spans": unresolved,
        "interpretation": _INTERPRETATION, "source_dispatch": False, "provider_calls": 0}
    return GliomaWorkflowIntent(json.dumps({**descriptor, "intent_digest": content_digest(descriptor)},
                                          ensure_ascii=False, sort_keys=True, separators=(",", ":")))


def compile_glioma_workflow_request(
    task: str, catalog: WorkflowOperationCatalog, *,
    intent: GliomaWorkflowIntent | Mapping[str, Any] | None = None, grounded: bool = False,
) -> WorkflowPlan:
    """Compile an accepted interpretation with a fixed exact-operation/argument/flow contract.

    Reinterpretation validates a supplied receipt against the exact task and grammar, rather than
    trusting recomputable hashes as authority. No provider invocation or source dispatch occurs.
    """
    if type(grounded) is not bool or not isinstance(catalog, WorkflowOperationCatalog):
        raise WorkflowValidationError("glioma intent compilation requires a catalogue and boolean grounded option")
    expected = interpret_glioma_workflow_request(task)
    if intent is not None:
        try:
            supplied = intent.to_dict() if isinstance(intent, GliomaWorkflowIntent) else dict(intent) if isinstance(intent, Mapping) else None
            encoded = json.dumps(supplied, ensure_ascii=False, allow_nan=False, sort_keys=True, separators=(",", ":"))
            valid = len(encoded.encode("utf-8")) <= 128_000 and encoded == expected._encoded
        except (ValueError, TypeError, RecursionError, UnicodeError):
            valid = False
        if not valid:
            raise WorkflowValidationError("glioma interpretation receipt does not match the exact task and grammar")
    receipt = expected.to_dict()
    if receipt["status"] != "ready":
        raise WorkflowValidationError("glioma request needs clarification; unresolved clauses cannot compile")
    proposal_builder = glioma_evidence_workflow_proposal
    if grounded:
        from .glioma_workflow_synthesis import glioma_grounded_workflow_proposal
        proposal_builder = glioma_grounded_workflow_proposal
    proposal = proposal_builder(task, source_operations=receipt["source_operations"],
        literature_limit=receipt["literature_limit"], trial_page_size=receipt["trial_page_size"],
        trial_max_pages=receipt["trial_max_pages"])
    # Bind catalogue defaults as well as caller-specified values into the fixed contract.
    normalized = compile_workflow_plan(proposal, catalog, task=task).to_dict()
    proposal = {key: normalized[key] for key in ("name", "steps", "outputs")}
    required = [step["operation"] for step in proposal["steps"]]
    links = [{"source_operation": source, "target_operation": "glioma.evidence.reconcile",
              "source_pointer": "", "target_path": f"/sources/{index}/bundle"}
             for index, source in enumerate(receipt["source_operations"])]
    if grounded:
        links.extend([{"source_operation": "glioma.evidence.reconcile", "target_operation": "glioma.evidence.synthesize",
                       "source_pointer": "", "target_path": "/reconciliation"},
                      {"source_operation": "glioma.evidence.synthesize", "target_operation": "glioma.evidence.audit_draft",
                       "source_pointer": "", "target_path": "/synthesis"}])
    else:
        links.append({"source_operation": "glioma.evidence.reconcile", "target_operation": "glioma.evidence.review_handoff",
                      "source_pointer": "", "target_path": "/reconciliation"})
    rules = []
    for step in proposal["steps"]:
        for key, value in step["arguments"].items():
            if key in {"sources", "reconciliation", "synthesis"}:
                continue
            rules.append({"operation": step["operation"], "path": "/" + key, "comparison": "equals", "value": value})
        if step["operation"] == "glioma.evidence.reconcile":
            for index, source in enumerate(receipt["source_operations"]):
                rules.append({"operation": step["operation"], "path": f"/sources/{index}/operation_id", "comparison": "equals", "value": source})
    output_id = proposal["outputs"][0]
    output_operation = next(step["operation"] for step in proposal["steps"] if step["id"] == output_id)
    contract = compile_workflow_requirements(task, required_operations=required,
        forbidden_operations=[op for op in _SOURCE_ORDER if op not in required], output_operations=[output_operation],
        argument_rules=rules, data_links=links, max_steps=len(proposal["steps"]),
        max_calls_by_operation={operation: 1 for operation in required})
    return compile_workflow_plan(proposal, catalog, task=task, requirements=contract)


__all__ = ["GLIOMA_WORKFLOW_INTENT_SCHEMA", "MAX_GLIOMA_INTENT_TASK_BYTES", "MAX_GLIOMA_INTENT_CLAUSES",
           "GliomaWorkflowIntent", "interpret_glioma_workflow_request", "compile_glioma_workflow_request"]
