"""Typed projections for the executable FIBER decision contract.

`fiber-query/0.3` adds a decision-relative quotient summary, `fiber-query/0.4` adds a full bounded
rate-distortion summary, and `fiber-query/0.5` adds a recursive adaptive policy projection to
``fiber_compile``. This module validates those projections without pretending the
progressive-disclosure response contains the full certificate. The Rust compiler remains
authoritative; Python only makes the published MCP projections safe and convenient to consume.
"""

from __future__ import annotations

import json
import math
import re
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


FIBER_DECISION_QUOTIENT_SCHEMA = "bioprism-mcp/epistemic-decision-quotient/0.1"
FIBER_DECISION_QUOTIENT_BASIS = "permitted_loss_difference_profile"
FIBER_DECISION_MAX_ACTIONS = 1_000
FIBER_RATE_DISTORTION_SCHEMA = "bioprism-mcp/epistemic-context-audit/0.2"
FIBER_RATE_DISTORTION_MAX_EVIDENCE = 16
FIBER_ADAPTIVE_ACQUISITION_SCHEMA = "bioprism-mcp/fiber-adaptive-acquisition/0.1"
FIBER_ADAPTIVE_MAX_ACQUISITIONS = 16
FIBER_ADAPTIVE_MAX_STEPS = 16
FIBER_ADAPTIVE_MAX_NODES = 65_536
_DIGEST = re.compile(r"^[0-9a-f]{64}$")


def _finite(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


def _count(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _digest(name: str, value: Any) -> str:
    text = _route_text(name, value)
    if not _DIGEST.fullmatch(text):
        raise ArgumentError(f"{name} must be a lowercase 64-character SHA-256 digest")
    return text


def _candidate_payloads(value: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    raw = _route_mapping("fiber compile response", value)
    candidates: list[Mapping[str, Any]] = [raw]
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        candidates.append(mcp)
        result = mcp.get("result")
        if isinstance(result, Mapping):
            candidates.append(result)
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"fiber compile response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    return candidates


@dataclass(frozen=True)
class FiberDecisionQuotientSummary:
    """Validated L0 quotient summary returned by ``fiber_compile``."""

    raw: dict[str, Any]
    schema: str
    basis: str
    permitted_actions: tuple[str, ...]
    original_model_count: int
    quotient_model_count: int
    merged_model_count: int
    compressed: bool
    compression_fraction: float
    query_sha256: str
    certificate_sha256: str
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FiberDecisionQuotientSummary":
        summary: Mapping[str, Any] | None = None
        for candidate in _candidate_payloads(value):
            possible = candidate.get("decision_quotient")
            if isinstance(possible, Mapping):
                summary = possible
                break
        if summary is None:
            raise ArgumentError("response does not contain a fiber decision quotient summary")

        schema = _route_text("fiber decision quotient schema", summary.get("schema"))
        if schema != FIBER_DECISION_QUOTIENT_SCHEMA:
            raise ArgumentError("fiber decision quotient summary has an invalid schema")
        basis = _route_text("fiber decision quotient basis", summary.get("basis"))
        if basis != FIBER_DECISION_QUOTIENT_BASIS:
            raise ArgumentError("fiber decision quotient summary has an invalid basis")
        actions = _route_strings("fiber decision quotient permitted actions", summary.get("permitted_actions"))
        if not 1 <= len(actions) <= FIBER_DECISION_MAX_ACTIONS or tuple(actions) != tuple(sorted(actions)) or len(actions) != len(set(actions)):
            raise ArgumentError("fiber decision quotient permitted actions must be non-empty, unique, and canonical")
        original = _count("fiber decision quotient original model count", summary.get("original_model_count"))
        quotient = _count("fiber decision quotient model count", summary.get("quotient_model_count"))
        merged = _count("fiber decision quotient merged model count", summary.get("merged_model_count"))
        if original == 0 or quotient == 0 or quotient > original or merged != original - quotient:
            raise ArgumentError("fiber decision quotient counts do not reconcile")
        compressed = summary.get("compressed")
        if not isinstance(compressed, bool) or compressed != (quotient < original):
            raise ArgumentError("fiber decision quotient compressed flag does not reconcile")
        fraction = _finite("fiber decision quotient compression fraction", summary.get("compression_fraction"))
        if fraction != quotient / original:
            raise ArgumentError("fiber decision quotient compression fraction does not reconcile")
        binding = _route_mapping("fiber decision quotient certificate binding", summary.get("certificate_binding"))
        limitations = _route_strings("fiber decision quotient limitations", summary.get("limitations", []))
        return cls(
            dict(summary),
            schema,
            basis,
            tuple(actions),
            original,
            quotient,
            merged,
            compressed,
            fraction,
            _digest("fiber decision quotient query_sha256", binding.get("query_sha256")),
            _digest("fiber decision quotient certificate_sha256", binding.get("certificate_sha256")),
            tuple(limitations),
        )

    @property
    def refused(self) -> bool:
        """This projection is present only for an accepted compile."""

        return False

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def fiber_decision_quotient_summary(value: Mapping[str, Any]) -> FiberDecisionQuotientSummary:
    """Parse direct MCP output or an HTTP REST tool envelope from ``fiber_compile``."""

    return FiberDecisionQuotientSummary.from_wire(value)


@dataclass(frozen=True)
class FiberRateDistortionSummary:
    """Validated L0 observed-context rate-distortion summary from ``fiber_compile``."""

    raw: dict[str, Any]
    schema: str
    criterion: str
    tolerance: float
    compatibility_floor: float
    evidence_count: int
    full_rate: float
    identification: dict[str, Any]
    sufficiency: dict[str, Any]
    frontier: dict[str, Any]
    query_sha256: str
    certificate_sha256: str
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FiberRateDistortionSummary":
        summary: Mapping[str, Any] | None = None
        for candidate in _candidate_payloads(value):
            possible = candidate.get("rate_distortion")
            if isinstance(possible, Mapping):
                summary = possible
                break
        if summary is None:
            raise ArgumentError("response does not contain a fiber rate-distortion summary")

        schema = _route_text("fiber rate-distortion schema", summary.get("schema"))
        if schema != FIBER_RATE_DISTORTION_SCHEMA:
            raise ArgumentError("fiber rate-distortion summary has an invalid schema")
        criterion = _route_text("fiber rate-distortion criterion", summary.get("criterion"))
        if criterion not in {"bayes_regret", "minimax_regret"}:
            raise ArgumentError("fiber rate-distortion criterion is invalid")
        tolerance = _finite("fiber rate-distortion tolerance", summary.get("tolerance"))
        if tolerance < 0.0:
            raise ArgumentError("fiber rate-distortion tolerance must be non-negative")
        floor = _finite("fiber rate-distortion compatibility floor", summary.get("compatibility_floor"))
        if not 0.0 <= floor <= 1.0:
            raise ArgumentError("fiber rate-distortion compatibility floor must be between 0 and 1")
        evidence_count = _count("fiber rate-distortion evidence count", summary.get("evidence_count"))
        if evidence_count > FIBER_RATE_DISTORTION_MAX_EVIDENCE:
            raise ArgumentError("fiber rate-distortion evidence count exceeds the exhaustive bound")
        full_rate = _finite("fiber rate-distortion full rate", summary.get("full_rate"))
        if full_rate < 0.0:
            raise ArgumentError("fiber rate-distortion full rate must be non-negative")
        identification = dict(_route_mapping("fiber rate-distortion identification", summary.get("identification")))
        sufficiency = dict(_route_mapping("fiber rate-distortion sufficiency", summary.get("sufficiency")))
        frontier = dict(_route_mapping("fiber rate-distortion frontier", summary.get("frontier")))
        evaluated = _count("fiber rate-distortion evaluated contexts", frontier.get("evaluated"))
        if evaluated == 0 or evaluated > (1 << FIBER_RATE_DISTORTION_MAX_EVIDENCE):
            raise ArgumentError("fiber rate-distortion frontier evaluated count is outside the exhaustive bound")
        binding = _route_mapping("fiber rate-distortion certificate binding", summary.get("certificate_binding"))
        guarantees = _route_strings("fiber rate-distortion guarantees", summary.get("guarantees", []))
        limitations = _route_strings("fiber rate-distortion limitations", summary.get("limitations", []))
        return cls(
            dict(summary),
            schema,
            criterion,
            tolerance,
            floor,
            evidence_count,
            full_rate,
            identification,
            sufficiency,
            frontier,
            _digest("fiber rate-distortion query_sha256", binding.get("query_sha256")),
            _digest("fiber rate-distortion certificate_sha256", binding.get("certificate_sha256")),
            tuple(guarantees),
            tuple(limitations),
        )

    @property
    def refused(self) -> bool:
        return False

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def fiber_rate_distortion_summary(value: Mapping[str, Any]) -> FiberRateDistortionSummary:
    """Parse direct MCP output or an HTTP REST tool envelope from ``fiber_compile``."""

    return FiberRateDistortionSummary.from_wire(value)


@dataclass(frozen=True)
class FiberAdaptiveOutcomeSummary:
    """One validated outcome branch in an integrated FIBER adaptive policy."""

    raw: dict[str, Any]
    label: str
    probability: float
    posterior: tuple[float, ...]
    next: "FiberAdaptiveNodeSummary"


@dataclass(frozen=True)
class FiberAdaptiveNodeSummary:
    """A named stop/acquire node with recursively validated child branches."""

    raw: dict[str, Any]
    kind: str
    action_index: int | None
    action: str | None
    risk: float | None
    acquisition_index: int | None
    acquisition_id: str | None
    cost: float | None
    expected_total: float | None
    expected_terminal_risk: float | None
    expected_acquisition_cost: float | None
    outcomes: tuple[FiberAdaptiveOutcomeSummary, ...]


@dataclass(frozen=True)
class FiberAdaptiveAcquisitionSummary:
    """Validated certificate-bound `fiber-query/0.5` adaptive projection."""

    raw: dict[str, Any]
    schema: str
    budget: float
    max_steps: int
    prior: tuple[float, ...]
    problem: dict[str, Any]
    acquisitions: tuple[dict[str, Any], ...]
    expected_total: float
    expected_terminal_risk: float
    expected_acquisition_cost: float
    nodes_evaluated: int
    selected_depth: int
    root: FiberAdaptiveNodeSummary
    query_sha256: str
    certificate_sha256: str
    execution: str
    authorization: str
    provenance: dict[str, Any]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FiberAdaptiveAcquisitionSummary":
        summary: Mapping[str, Any] | None = None
        for candidate in _candidate_payloads(value):
            possible = candidate.get("adaptive_acquisition")
            if isinstance(possible, Mapping):
                summary = possible
                break
        if summary is None:
            raise ArgumentError("response does not contain a fiber adaptive-acquisition summary")

        schema = _route_text("fiber adaptive-acquisition schema", summary.get("schema"))
        if schema != FIBER_ADAPTIVE_ACQUISITION_SCHEMA:
            raise ArgumentError("fiber adaptive-acquisition summary has an invalid schema")
        budget = _finite("fiber adaptive-acquisition budget", summary.get("budget"))
        if budget < 0.0:
            raise ArgumentError("fiber adaptive-acquisition budget must be non-negative")
        max_steps = _count("fiber adaptive-acquisition max_steps", summary.get("max_steps"))
        if max_steps > FIBER_ADAPTIVE_MAX_STEPS:
            raise ArgumentError("fiber adaptive-acquisition max_steps exceeds the exact cap")

        raw_problem = _route_mapping("fiber adaptive-acquisition problem", summary.get("problem"))
        actions = _route_strings("fiber adaptive-acquisition actions", raw_problem.get("actions"))
        models = _route_strings("fiber adaptive-acquisition models", raw_problem.get("models"))
        if not actions or not models:
            raise ArgumentError("fiber adaptive-acquisition problem must have actions and models")
        if raw_problem.get("action_count") != len(actions) or raw_problem.get("model_count") != len(models):
            raise ArgumentError("fiber adaptive-acquisition problem counts do not reconcile")

        prior_values = summary.get("prior")
        if not isinstance(prior_values, Sequence) or isinstance(prior_values, (str, bytes)):
            raise ArgumentError("fiber adaptive-acquisition prior must be an array")
        if len(prior_values) != len(models):
            raise ArgumentError("fiber adaptive-acquisition prior shape does not match models")
        prior = tuple(_finite(f"fiber adaptive-acquisition prior[{index}]", item) for index, item in enumerate(prior_values))
        if any(item < 0.0 for item in prior) or not math.isclose(sum(prior), 1.0, rel_tol=1e-9, abs_tol=1e-9):
            raise ArgumentError("fiber adaptive-acquisition prior must be normalized and non-negative")

        raw_acquisitions = summary.get("acquisitions")
        if not isinstance(raw_acquisitions, Sequence) or isinstance(raw_acquisitions, (str, bytes)):
            raise ArgumentError("fiber adaptive-acquisition acquisitions must be an array")
        if not 1 <= len(raw_acquisitions) <= FIBER_ADAPTIVE_MAX_ACQUISITIONS:
            raise ArgumentError("fiber adaptive-acquisition acquisitions exceed the exact cap")
        acquisitions: list[dict[str, Any]] = []
        acquisition_ids: list[str] = []
        for index, raw_acquisition in enumerate(raw_acquisitions):
            acquisition = dict(_route_mapping(f"fiber adaptive-acquisition acquisitions[{index}]", raw_acquisition))
            identifier = _route_text(f"fiber adaptive-acquisition acquisitions[{index}].id", acquisition.get("id"))
            if not identifier.strip() or identifier in acquisition_ids:
                raise ArgumentError("fiber adaptive-acquisition acquisition IDs must be unique and non-empty")
            cost = _finite(f"fiber adaptive-acquisition acquisitions[{index}].cost", acquisition.get("cost"))
            if cost < 0.0:
                raise ArgumentError("fiber adaptive-acquisition acquisition costs must be non-negative")
            raw_outcomes = acquisition.get("outcomes")
            if not isinstance(raw_outcomes, Sequence) or isinstance(raw_outcomes, (str, bytes)) or not raw_outcomes:
                raise ArgumentError("fiber adaptive-acquisition acquisitions must expose outcomes")
            outcome_ids: set[str] = set()
            normalized_outcomes: list[dict[str, Any]] = []
            for outcome_index, raw_outcome in enumerate(raw_outcomes):
                outcome = dict(_route_mapping(f"fiber adaptive-acquisition acquisitions[{index}].outcomes[{outcome_index}]", raw_outcome))
                label = _route_text("fiber adaptive-acquisition outcome label", outcome.get("label"))
                if not label.strip() or label in outcome_ids:
                    raise ArgumentError("fiber adaptive-acquisition outcome labels must be unique and non-empty")
                likelihood_values = outcome.get("likelihood")
                if not isinstance(likelihood_values, Sequence) or isinstance(likelihood_values, (str, bytes)) or len(likelihood_values) != len(models):
                    raise ArgumentError("fiber adaptive-acquisition outcome likelihood shape does not match models")
                likelihood = tuple(_finite("fiber adaptive-acquisition likelihood", item) for item in likelihood_values)
                if any(item < 0.0 for item in likelihood):
                    raise ArgumentError("fiber adaptive-acquisition likelihoods must be non-negative")
                outcome_ids.add(label)
                normalized_outcomes.append({**outcome, "label": label, "likelihood": list(likelihood)})
            for model in range(len(models)):
                partition = sum(outcome["likelihood"][model] for outcome in normalized_outcomes)
                if not math.isclose(partition, 1.0, rel_tol=1e-9, abs_tol=1e-9):
                    raise ArgumentError("fiber adaptive-acquisition outcome likelihoods must partition each model")
            acquisition_ids.append(identifier)
            acquisitions.append({**acquisition, "id": identifier, "cost": cost, "outcomes": normalized_outcomes})

        raw_policy = _route_mapping("fiber adaptive-acquisition policy", summary.get("policy"))
        expected_total = _finite("fiber adaptive-acquisition expected_total", raw_policy.get("expected_total"))
        expected_terminal_risk = _finite("fiber adaptive-acquisition expected_terminal_risk", raw_policy.get("expected_terminal_risk"))
        expected_acquisition_cost = _finite("fiber adaptive-acquisition expected_acquisition_cost", raw_policy.get("expected_acquisition_cost"))
        nodes_evaluated = _count("fiber adaptive-acquisition nodes_evaluated", raw_policy.get("nodes_evaluated"))
        if nodes_evaluated == 0 or nodes_evaluated > FIBER_ADAPTIVE_MAX_NODES:
            raise ArgumentError("fiber adaptive-acquisition nodes_evaluated exceeds the exact cap")
        selected_depth = _count("fiber adaptive-acquisition selected_depth", raw_policy.get("selected_depth"))
        if selected_depth > max_steps:
            raise ArgumentError("fiber adaptive-acquisition selected depth exceeds max_steps")

        def parse_node(raw_node: Any, depth: int, used: tuple[int, ...], path_cost: float) -> FiberAdaptiveNodeSummary:
            if depth > FIBER_ADAPTIVE_MAX_STEPS:
                raise ArgumentError("fiber adaptive-acquisition policy tree exceeds the exact depth cap")
            node = _route_mapping("fiber adaptive-acquisition policy node", raw_node)
            kind = _route_text("fiber adaptive-acquisition policy node kind", node.get("kind"))
            if kind == "stop":
                action_index = _count("fiber adaptive-acquisition stop action_index", node.get("action_index"))
                if action_index >= len(actions) or node.get("action") != actions[action_index]:
                    raise ArgumentError("fiber adaptive-acquisition stop action identity does not reconcile")
                risk = _finite("fiber adaptive-acquisition stop risk", node.get("risk"))
                return FiberAdaptiveNodeSummary(dict(node), kind, action_index, actions[action_index], risk, None, None, None, None, None, None, ())
            if kind != "acquire":
                raise ArgumentError("fiber adaptive-acquisition policy node kind is invalid")
            acquisition_index = _count("fiber adaptive-acquisition acquisition_index", node.get("acquisition_index"))
            if acquisition_index >= len(acquisitions) or acquisition_index in used:
                raise ArgumentError("fiber adaptive-acquisition policy repeats or references an invalid acquisition")
            acquisition = acquisitions[acquisition_index]
            identifier = _route_text("fiber adaptive-acquisition policy acquisition id", node.get("id"))
            if identifier != acquisition["id"]:
                raise ArgumentError("fiber adaptive-acquisition policy acquisition identity does not reconcile")
            cost = _finite("fiber adaptive-acquisition policy acquisition cost", node.get("cost"))
            if not math.isclose(cost, acquisition["cost"], rel_tol=1e-9, abs_tol=1e-9):
                raise ArgumentError("fiber adaptive-acquisition policy acquisition cost does not reconcile")
            if path_cost + cost > budget + 1e-9:
                raise ArgumentError("fiber adaptive-acquisition policy exceeds the declared path budget")
            node_expected_total = _finite("fiber adaptive-acquisition node expected_total", node.get("expected_total"))
            node_terminal = _finite("fiber adaptive-acquisition node expected_terminal_risk", node.get("expected_terminal_risk"))
            node_cost = _finite("fiber adaptive-acquisition node expected_acquisition_cost", node.get("expected_acquisition_cost"))
            raw_outcomes_for_node = node.get("outcomes")
            if not isinstance(raw_outcomes_for_node, Sequence) or isinstance(raw_outcomes_for_node, (str, bytes)):
                raise ArgumentError("fiber adaptive-acquisition acquire nodes must contain outcomes")
            if len(raw_outcomes_for_node) != len(acquisition["outcomes"]):
                raise ArgumentError("fiber adaptive-acquisition policy outcome count does not reconcile")
            branches: list[FiberAdaptiveOutcomeSummary] = []
            probability_sum = 0.0
            labels: set[str] = set()
            for outcome_index, raw_branch in enumerate(raw_outcomes_for_node):
                branch = _route_mapping("fiber adaptive-acquisition policy outcome", raw_branch)
                label = _route_text("fiber adaptive-acquisition policy outcome label", branch.get("label"))
                if label in labels or label != acquisition["outcomes"][outcome_index]["label"]:
                    raise ArgumentError("fiber adaptive-acquisition policy outcome identity does not reconcile")
                probability = _finite("fiber adaptive-acquisition policy outcome probability", branch.get("probability"))
                if probability < 0.0 or probability > 1.0:
                    raise ArgumentError("fiber adaptive-acquisition outcome probabilities must be between 0 and 1")
                posterior_values = branch.get("posterior")
                if not isinstance(posterior_values, Sequence) or isinstance(posterior_values, (str, bytes)) or len(posterior_values) != len(models):
                    raise ArgumentError("fiber adaptive-acquisition posterior shape does not match models")
                posterior = tuple(_finite("fiber adaptive-acquisition posterior", item) for item in posterior_values)
                if any(item < 0.0 for item in posterior) or not math.isclose(sum(posterior), 1.0, rel_tol=1e-9, abs_tol=1e-9):
                    raise ArgumentError("fiber adaptive-acquisition posteriors must be normalized and non-negative")
                labels.add(label)
                probability_sum += probability
                branches.append(FiberAdaptiveOutcomeSummary(dict(branch), label, probability, posterior, parse_node(branch.get("next"), depth + 1, (*used, acquisition_index), path_cost + cost)))
            if not math.isclose(probability_sum, 1.0, rel_tol=1e-9, abs_tol=1e-9):
                raise ArgumentError("fiber adaptive-acquisition outcome probabilities must sum to one")
            return FiberAdaptiveNodeSummary(dict(node), kind, None, None, None, acquisition_index, identifier, cost, node_expected_total, node_terminal, node_cost, tuple(branches))

        root = parse_node(raw_policy.get("root"), 0, (), 0.0)
        if root.kind == "stop":
            if not math.isclose(expected_total, root.risk or 0.0, rel_tol=1e-9, abs_tol=1e-9) or not math.isclose(expected_terminal_risk, root.risk or 0.0, rel_tol=1e-9, abs_tol=1e-9) or not math.isclose(expected_acquisition_cost, 0.0, abs_tol=1e-9):
                raise ArgumentError("fiber adaptive-acquisition stop objective does not reconcile")
        else:
            assert root.expected_total is not None and root.expected_terminal_risk is not None and root.expected_acquisition_cost is not None
            if not math.isclose(expected_total, root.expected_total, rel_tol=1e-9, abs_tol=1e-9) or not math.isclose(expected_terminal_risk, root.expected_terminal_risk, rel_tol=1e-9, abs_tol=1e-9) or not math.isclose(expected_acquisition_cost, root.expected_acquisition_cost, rel_tol=1e-9, abs_tol=1e-9):
                raise ArgumentError("fiber adaptive-acquisition root objective does not reconcile")
        if expected_acquisition_cost < 0.0 or expected_total != expected_terminal_risk + expected_acquisition_cost and not math.isclose(expected_total, expected_terminal_risk + expected_acquisition_cost, rel_tol=1e-8, abs_tol=1e-8):
            raise ArgumentError("fiber adaptive-acquisition objective decomposition is invalid")

        binding = _route_mapping("fiber adaptive-acquisition certificate binding", summary.get("certificate_binding"))
        execution = _route_text("fiber adaptive-acquisition execution", summary.get("execution"))
        authorization = _route_text("fiber adaptive-acquisition authorization", summary.get("authorization"))
        if execution != "not_started" or authorization != "not_granted":
            raise ArgumentError("fiber adaptive-acquisition projection cannot claim execution or authorization")
        return cls(
            dict(summary), schema, budget, max_steps, prior, dict(raw_problem), tuple(acquisitions),
            expected_total, expected_terminal_risk, expected_acquisition_cost, nodes_evaluated,
            selected_depth, root,
            _digest("fiber adaptive-acquisition query_sha256", binding.get("query_sha256")),
            _digest("fiber adaptive-acquisition certificate_sha256", binding.get("certificate_sha256")),
            execution, authorization,
            dict(_route_mapping("fiber adaptive-acquisition provenance", summary.get("provenance"))),
            tuple(_route_strings("fiber adaptive-acquisition guarantees", summary.get("guarantees", []))),
            tuple(_route_strings("fiber adaptive-acquisition limitations", summary.get("limitations", []))),
        )

    @property
    def refused(self) -> bool:
        return False

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def fiber_adaptive_acquisition_summary(value: Mapping[str, Any]) -> FiberAdaptiveAcquisitionSummary:
    """Parse direct MCP output or an HTTP REST tool envelope from ``fiber_compile``."""

    return FiberAdaptiveAcquisitionSummary.from_wire(value)


__all__ = [
    "FIBER_DECISION_QUOTIENT_SCHEMA",
    "FIBER_DECISION_QUOTIENT_BASIS",
    "FIBER_RATE_DISTORTION_SCHEMA",
    "FIBER_RATE_DISTORTION_MAX_EVIDENCE",
    "FIBER_ADAPTIVE_ACQUISITION_SCHEMA",
    "FIBER_ADAPTIVE_MAX_ACQUISITIONS",
    "FIBER_ADAPTIVE_MAX_STEPS",
    "FIBER_ADAPTIVE_MAX_NODES",
    "FiberDecisionQuotientSummary",
    "FiberRateDistortionSummary",
    "FiberAdaptiveOutcomeSummary",
    "FiberAdaptiveNodeSummary",
    "FiberAdaptiveAcquisitionSummary",
    "fiber_decision_quotient_summary",
    "fiber_rate_distortion_summary",
    "fiber_adaptive_acquisition_summary",
]
