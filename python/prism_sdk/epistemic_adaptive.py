"""Typed reports for exact finite-horizon adaptive acquisition policies.

This surface deliberately separates a policy calculation from acquisition execution.  A policy is
an explicit tree: each outcome carries its probability and posterior, and each child can stop or
choose a different unused acquisition.  The parser validates the tree's local accounting and
decision references so callers cannot accidentally treat a non-adaptive VOI number, a sampled
rollout, or an unverified observation as an adaptive plan.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError
from .epistemic import (
    EpistemicAcquisitionArgs,
    EpistemicBeliefArgs,
    EpistemicDecisionProblemArgs,
    EpistemicOutcomeArgs,
    EpistemicRefusalReport,
    EPISTEMIC_MAX_ACTIONS,
    EPISTEMIC_MAX_INPUT_BYTES,
    EPISTEMIC_MAX_MODELS,
    EPISTEMIC_MAX_OUTCOMES,
    _array,
    _finite,
    _finite_array,
    _index,
)


EPISTEMIC_ADAPTIVE_SCHEMA = "bioprism-mcp/epistemic-adaptive-acquisition/0.1"
EPISTEMIC_ADAPTIVE_MAX_ACQUISITIONS = 16
EPISTEMIC_ADAPTIVE_MAX_STEPS = 16
EPISTEMIC_ADAPTIVE_MAX_POLICY_NODES = 65_536
EPISTEMIC_ADAPTIVE_EPSILON = 1e-9


def _adaptive_payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract direct JSON, MCP structured content, or a REST tool envelope."""

    raw = _route_mapping("epistemic adaptive response", value)

    def candidates_for(candidate: Mapping[str, Any]) -> list[Mapping[str, Any]]:
        found = [candidate]
        mcp = candidate.get("mcp")
        if isinstance(mcp, Mapping):
            found.append(mcp)
            result = mcp.get("result")
            if isinstance(result, Mapping):
                found.append(result)
                structured = result.get("structuredContent")
                if isinstance(structured, Mapping):
                    found.append(structured)
                content = result.get("content")
                if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                    for block in content:
                        if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                            continue
                        try:
                            decoded = json.loads(block["text"])
                        except json.JSONDecodeError as error:
                            raise ArgumentError(
                                f"epistemic adaptive response text is not JSON: {error}"
                            ) from error
                        if isinstance(decoded, Mapping):
                            found.append(decoded)
        return found

    for candidate in candidates_for(raw):
        schema = candidate.get("schema")
        if candidate.get("ok") is True and schema == EPISTEMIC_ADAPTIVE_SCHEMA:
            if isinstance(candidate.get("policy"), Mapping):
                return dict(candidate)
        if candidate.get("ok") is False and candidate.get("fail_closed") is True:
            if isinstance(candidate.get("refusal"), str):
                return dict(candidate)
    raise ArgumentError("response does not contain an epistemic adaptive policy projection")


@dataclass(frozen=True)
class EpistemicAdaptiveArgs:
    """Explicit inputs for one bounded adaptive policy calculation."""

    problem: EpistemicDecisionProblemArgs
    belief: EpistemicBeliefArgs
    acquisitions: tuple[EpistemicAcquisitionArgs, ...]
    budget: float
    max_steps: int

    def __post_init__(self) -> None:
        problem = self.problem if isinstance(self.problem, EpistemicDecisionProblemArgs) else EpistemicDecisionProblemArgs.from_wire(self.problem)
        belief = self.belief if isinstance(self.belief, EpistemicBeliefArgs) else EpistemicBeliefArgs.from_wire(self.belief)
        acquisitions = tuple(
            item if isinstance(item, EpistemicAcquisitionArgs) else EpistemicAcquisitionArgs.from_wire(item)
            for item in self.acquisitions
        )
        budget = _finite("epistemic adaptive budget", self.budget)
        if budget < 0.0:
            raise ArgumentError("epistemic adaptive budget must be non-negative")
        if isinstance(self.max_steps, bool) or not isinstance(self.max_steps, int):
            raise ArgumentError("epistemic adaptive max_steps must be an integer")
        if not 0 <= self.max_steps <= EPISTEMIC_ADAPTIVE_MAX_STEPS:
            raise ArgumentError(
                f"epistemic adaptive max_steps must be between 0 and {EPISTEMIC_ADAPTIVE_MAX_STEPS}"
            )
        if len(belief.mass) != len(problem.models):
            raise ArgumentError("epistemic adaptive belief length must match problem models")
        if not 1 <= len(acquisitions) <= EPISTEMIC_ADAPTIVE_MAX_ACQUISITIONS:
            raise ArgumentError(
                "epistemic adaptive acquisitions must contain between 1 and 16 actions"
            )
        identifiers = [item.id for item in acquisitions]
        if len(identifiers) != len(set(identifiers)):
            raise ArgumentError("epistemic adaptive acquisition ids must be unique")
        for item in acquisitions:
            for outcome in item.outcomes:
                if len(outcome.likelihood) != len(problem.models):
                    raise ArgumentError("epistemic adaptive likelihood length must match models")
            for model in range(len(problem.models)):
                partition = sum(outcome.likelihood[model] for outcome in item.outcomes)
                if not math.isclose(partition, 1.0, rel_tol=0.0, abs_tol=EPISTEMIC_ADAPTIVE_EPSILON):
                    raise ArgumentError(
                        f"epistemic adaptive acquisition {item.id!r} likelihoods must sum to one"
                    )
        wire = {
            "problem": problem.to_wire(),
            "belief": belief.to_wire(),
            "acquisitions": [item.to_wire() for item in acquisitions],
            "budget": budget,
            "max_steps": self.max_steps,
        }
        try:
            encoded_size = len(json.dumps(wire, separators=(",", ":"), allow_nan=False).encode("utf-8"))
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"epistemic adaptive arguments are not JSON encodable: {error}") from error
        if encoded_size > EPISTEMIC_MAX_INPUT_BYTES:
            raise ArgumentError("epistemic adaptive input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "problem", problem)
        object.__setattr__(self, "belief", belief)
        object.__setattr__(self, "acquisitions", acquisitions)
        object.__setattr__(self, "budget", budget)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicAdaptiveArgs":
        raw = _route_mapping("epistemic adaptive arguments", value)
        return cls(
            EpistemicDecisionProblemArgs.from_wire(raw.get("problem")),
            EpistemicBeliefArgs.from_wire(raw.get("belief")),
            tuple(
                EpistemicAcquisitionArgs.from_wire(item)
                for item in _array("epistemic adaptive acquisitions", raw.get("acquisitions"))
            ),
            _finite("epistemic adaptive budget", raw.get("budget")),
            _index("epistemic adaptive max_steps", raw.get("max_steps")),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "problem": self.problem.to_wire(),
            "belief": self.belief.to_wire(),
            "acquisitions": [item.to_wire() for item in self.acquisitions],
            "budget": self.budget,
            "max_steps": self.max_steps,
        }


@dataclass(frozen=True)
class EpistemicAdaptiveOutcomeReport:
    """One observed-outcome branch in a policy tree."""

    label: str
    probability: float
    posterior: tuple[float, ...]
    next: "EpistemicAdaptiveNodeReport"

    @classmethod
    def from_wire(cls, value: Mapping[str, Any], *, depth: int) -> "EpistemicAdaptiveOutcomeReport":
        raw = _route_mapping("epistemic adaptive outcome", value)
        probability = _finite("epistemic adaptive outcome probability", raw.get("probability"))
        if not -EPISTEMIC_ADAPTIVE_EPSILON <= probability <= 1.0 + EPISTEMIC_ADAPTIVE_EPSILON:
            raise ArgumentError("epistemic adaptive outcome probabilities must lie between zero and one")
        posterior = _finite_array("epistemic adaptive posterior", raw.get("posterior"))
        if any(item < -EPISTEMIC_ADAPTIVE_EPSILON for item in posterior):
            raise ArgumentError("epistemic adaptive posteriors must be non-negative")
        if not math.isclose(sum(posterior), 1.0, rel_tol=0.0, abs_tol=EPISTEMIC_ADAPTIVE_EPSILON):
            raise ArgumentError("epistemic adaptive posterior masses must sum to one")
        return cls(
            _route_text("epistemic adaptive outcome label", raw.get("label")),
            max(0.0, probability),
            posterior,
            EpistemicAdaptiveNodeReport.from_wire(raw.get("next"), depth=depth),
        )


@dataclass(frozen=True)
class EpistemicAdaptiveNodeReport:
    """A stop or acquisition node, normalized without erasing the original wire object."""

    raw: dict[str, Any]
    kind: str
    action_index: int | None
    action: str | None
    risk: float | None
    acquisition_index: int | None
    id: str | None
    cost: float | None
    expected_total: float | None
    expected_terminal_risk: float | None
    expected_acquisition_cost: float | None
    outcomes: tuple[EpistemicAdaptiveOutcomeReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any], *, depth: int = 0) -> "EpistemicAdaptiveNodeReport":
        if depth > EPISTEMIC_ADAPTIVE_MAX_STEPS:
            raise ArgumentError("epistemic adaptive policy exceeds its 16-level tree cap")
        raw = _route_mapping("epistemic adaptive policy node", value)
        kind = _route_text("epistemic adaptive node kind", raw.get("kind"))
        if kind == "stop":
            action_index = _index("epistemic adaptive stop action_index", raw.get("action_index"))
            action = _route_text("epistemic adaptive stop action", raw.get("action"))
            risk = _finite("epistemic adaptive stop risk", raw.get("risk"))
            if risk < -EPISTEMIC_ADAPTIVE_EPSILON:
                raise ArgumentError("epistemic adaptive stop risk must be non-negative")
            return cls(dict(raw), kind, action_index, action, max(0.0, risk), None, None, None, None, None, None, ())
        if kind != "acquire":
            raise ArgumentError(f"unknown epistemic adaptive node kind {kind!r}")
        outcomes_raw = _array("epistemic adaptive node outcomes", raw.get("outcomes"))
        if not 1 <= len(outcomes_raw) <= EPISTEMIC_MAX_OUTCOMES:
            raise ArgumentError("epistemic adaptive node must contain between 1 and 1000 outcomes")
        cost = _finite("epistemic adaptive node cost", raw.get("cost"))
        expected_total = _finite("epistemic adaptive expected_total", raw.get("expected_total"))
        expected_terminal_risk = _finite("epistemic adaptive expected_terminal_risk", raw.get("expected_terminal_risk"))
        expected_acquisition_cost = _finite("epistemic adaptive expected_acquisition_cost", raw.get("expected_acquisition_cost"))
        if cost < -EPISTEMIC_ADAPTIVE_EPSILON or expected_terminal_risk < -EPISTEMIC_ADAPTIVE_EPSILON or expected_acquisition_cost < -EPISTEMIC_ADAPTIVE_EPSILON:
            raise ArgumentError("epistemic adaptive costs and risks must be non-negative")
        if not math.isclose(expected_total, expected_terminal_risk + expected_acquisition_cost, rel_tol=1e-8, abs_tol=EPISTEMIC_ADAPTIVE_EPSILON):
            raise ArgumentError("epistemic adaptive total does not reconcile terminal risk and cost")
        outcomes = tuple(
            EpistemicAdaptiveOutcomeReport.from_wire(item, depth=depth + 1)
            for item in outcomes_raw
        )
        if not math.isclose(sum(item.probability for item in outcomes), 1.0, rel_tol=0.0, abs_tol=EPISTEMIC_ADAPTIVE_EPSILON):
            raise ArgumentError("epistemic adaptive outcome probabilities must sum to one")
        child_total = sum(item.probability * _node_total(item.next) for item in outcomes)
        child_risk = sum(item.probability * _node_terminal_risk(item.next) for item in outcomes)
        child_cost = sum(item.probability * _node_acquisition_cost(item.next) for item in outcomes)
        if not math.isclose(expected_total, cost + child_total, rel_tol=1e-8, abs_tol=EPISTEMIC_ADAPTIVE_EPSILON):
            raise ArgumentError("epistemic adaptive node total does not reconcile its outcome branches")
        if not math.isclose(expected_terminal_risk, child_risk, rel_tol=1e-8, abs_tol=EPISTEMIC_ADAPTIVE_EPSILON):
            raise ArgumentError("epistemic adaptive node terminal risk does not reconcile its outcome branches")
        if not math.isclose(expected_acquisition_cost, cost + child_cost, rel_tol=1e-8, abs_tol=EPISTEMIC_ADAPTIVE_EPSILON):
            raise ArgumentError("epistemic adaptive node cost does not reconcile its outcome branches")
        return cls(
            dict(raw),
            kind,
            None,
            None,
            None,
            _index("epistemic adaptive acquisition_index", raw.get("acquisition_index")),
            _route_text("epistemic adaptive acquisition id", raw.get("id")),
            max(0.0, cost),
            expected_total,
            max(0.0, expected_terminal_risk),
            max(0.0, expected_acquisition_cost),
            outcomes,
        )

    @property
    def is_stop(self) -> bool:
        return self.kind == "stop"

    @property
    def is_acquisition(self) -> bool:
        return self.kind == "acquire"

    @property
    def max_depth(self) -> int:
        if self.is_stop:
            return 0
        return 1 + max(outcome.next.max_depth for outcome in self.outcomes)


@dataclass(frozen=True)
class EpistemicAdaptivePolicyReport:
    """Validated objective decomposition and explicit adaptive tree."""

    raw: dict[str, Any]
    expected_total: float
    expected_terminal_risk: float
    expected_acquisition_cost: float
    nodes_evaluated: int
    selected_depth: int
    root: EpistemicAdaptiveNodeReport

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicAdaptivePolicyReport":
        raw = _route_mapping("epistemic adaptive policy", value)
        expected_total = _finite("epistemic adaptive policy expected_total", raw.get("expected_total"))
        expected_terminal_risk = _finite("epistemic adaptive policy expected_terminal_risk", raw.get("expected_terminal_risk"))
        expected_acquisition_cost = _finite("epistemic adaptive policy expected_acquisition_cost", raw.get("expected_acquisition_cost"))
        if expected_terminal_risk < -EPISTEMIC_ADAPTIVE_EPSILON or expected_acquisition_cost < -EPISTEMIC_ADAPTIVE_EPSILON:
            raise ArgumentError("epistemic adaptive policy risk and cost must be non-negative")
        if not math.isclose(expected_total, expected_terminal_risk + expected_acquisition_cost, rel_tol=1e-8, abs_tol=EPISTEMIC_ADAPTIVE_EPSILON):
            raise ArgumentError("epistemic adaptive policy total does not reconcile")
        nodes_evaluated = _index("epistemic adaptive nodes_evaluated", raw.get("nodes_evaluated"))
        if not 1 <= nodes_evaluated <= EPISTEMIC_ADAPTIVE_MAX_POLICY_NODES:
            raise ArgumentError("epistemic adaptive nodes_evaluated exceeds the exact state cap")
        selected_depth = _index("epistemic adaptive selected_depth", raw.get("selected_depth"))
        if selected_depth > EPISTEMIC_ADAPTIVE_MAX_STEPS:
            raise ArgumentError("epistemic adaptive selected_depth exceeds the exact horizon cap")
        root = EpistemicAdaptiveNodeReport.from_wire(raw.get("root"))
        if root.max_depth != selected_depth:
            raise ArgumentError("epistemic adaptive selected_depth does not match the returned policy tree")
        if not math.isclose(_node_total(root), expected_total, rel_tol=1e-8, abs_tol=EPISTEMIC_ADAPTIVE_EPSILON):
            raise ArgumentError("epistemic adaptive root total does not match policy total")
        return cls(
            dict(raw),
            expected_total,
            max(0.0, expected_terminal_risk),
            max(0.0, expected_acquisition_cost),
            nodes_evaluated,
            selected_depth,
            root,
        )

    @property
    def exact_within_caps(self) -> bool:
        return self.nodes_evaluated <= EPISTEMIC_ADAPTIVE_MAX_POLICY_NODES


@dataclass(frozen=True)
class EpistemicAdaptiveReport:
    """Accepted adaptive policy or a fail-closed refusal."""

    raw: dict[str, Any]
    ok: bool
    schema: str
    budget: float | None
    max_steps: int | None
    actions: tuple[str, ...]
    models: tuple[str, ...]
    acquisition_ids: tuple[str, ...]
    policy: EpistemicAdaptivePolicyReport | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    refusal: EpistemicRefusalReport | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicAdaptiveReport":
        raw = _adaptive_payload(value)
        schema = _route_text("epistemic adaptive schema", raw.get("schema", EPISTEMIC_ADAPTIVE_SCHEMA))
        if schema != EPISTEMIC_ADAPTIVE_SCHEMA:
            raise ArgumentError(f"unexpected epistemic adaptive schema {schema!r}")
        guarantees = _route_strings("epistemic adaptive guarantees", raw.get("guarantees", []))
        limitations = _route_strings("epistemic adaptive limitations", raw.get("limitations", []))
        if raw.get("ok") is False:
            refusal = EpistemicRefusalReport.from_wire(raw)
            return cls(dict(raw), False, schema, None, None, (), (), (), None, guarantees, limitations, refusal)
        if raw.get("ok") is not True:
            raise ArgumentError("epistemic adaptive projection must declare ok")
        budget = _finite("epistemic adaptive report budget", raw.get("budget"))
        if budget < 0.0:
            raise ArgumentError("epistemic adaptive report budget must be non-negative")
        max_steps = _index("epistemic adaptive report max_steps", raw.get("max_steps"))
        problem = _route_mapping("epistemic adaptive report problem", raw.get("problem"))
        actions = _route_strings("epistemic adaptive report actions", problem.get("actions"))
        models = _route_strings("epistemic adaptive report models", problem.get("models"))
        if len(actions) > EPISTEMIC_MAX_ACTIONS or len(models) > EPISTEMIC_MAX_MODELS:
            raise ArgumentError("epistemic adaptive report problem exceeds action/model caps")
        acquisition_values = _array("epistemic adaptive report acquisitions", raw.get("acquisitions"))
        if not 1 <= len(acquisition_values) <= EPISTEMIC_ADAPTIVE_MAX_ACQUISITIONS:
            raise ArgumentError("epistemic adaptive report acquisition count exceeds cap")
        acquisition_ids: list[str] = []
        for index, item in enumerate(acquisition_values):
            acquisition = _route_mapping(f"epistemic adaptive report acquisition[{index}]", item)
            identifier = _route_text("epistemic adaptive report acquisition id", acquisition.get("id"))
            if identifier in acquisition_ids:
                raise ArgumentError("epistemic adaptive report acquisition ids must be unique")
            acquisition_ids.append(identifier)
        policy = EpistemicAdaptivePolicyReport.from_wire(raw.get("policy"))
        _validate_tree(policy.root, actions, acquisition_ids, depth=0)
        return cls(
            dict(raw),
            True,
            schema,
            budget,
            max_steps,
            actions,
            models,
            tuple(acquisition_ids),
            policy,
            guarantees,
            limitations,
            None,
        )

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def fail_closed(self) -> bool:
        return self.refusal is not None and self.refusal.fail_closed

    @property
    def expected_total(self) -> float | None:
        return None if self.policy is None else self.policy.expected_total

    @property
    def branch_dependent(self) -> bool | None:
        if self.policy is None:
            return None
        return any(node.is_acquisition for node in _walk_nodes(self.policy.root) if node is not self.policy.root)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def _walk_nodes(root: EpistemicAdaptiveNodeReport) -> tuple[EpistemicAdaptiveNodeReport, ...]:
    nodes = [root]
    for outcome in root.outcomes:
        nodes.extend(_walk_nodes(outcome.next))
    return tuple(nodes)


def _node_total(node: EpistemicAdaptiveNodeReport) -> float:
    return node.risk if node.is_stop else node.expected_total  # type: ignore[return-value]


def _node_terminal_risk(node: EpistemicAdaptiveNodeReport) -> float:
    return node.risk if node.is_stop else node.expected_terminal_risk  # type: ignore[return-value]


def _node_acquisition_cost(node: EpistemicAdaptiveNodeReport) -> float:
    return 0.0 if node.is_stop else node.expected_acquisition_cost  # type: ignore[return-value]


def _validate_tree(
    node: EpistemicAdaptiveNodeReport,
    actions: tuple[str, ...],
    acquisition_ids: list[str],
    *,
    depth: int,
    used: tuple[str, ...] = (),
) -> None:
    if node.is_stop:
        if node.action_index is None or node.action_index >= len(actions) or node.action != actions[node.action_index]:
            raise ArgumentError("epistemic adaptive stop node action does not match the problem")
        return
    assert node.acquisition_index is not None and node.id is not None
    if node.acquisition_index >= len(acquisition_ids) or node.id != acquisition_ids[node.acquisition_index]:
        raise ArgumentError("epistemic adaptive acquisition node does not match the declared acquisitions")
    if node.id in used:
        raise ArgumentError("epistemic adaptive policy repeats an acquisition on one branch")
    if depth >= EPISTEMIC_ADAPTIVE_MAX_STEPS:
        raise ArgumentError("epistemic adaptive policy exceeds its 16-step tree cap")
    for outcome in node.outcomes:
        _validate_tree(outcome.next, actions, acquisition_ids, depth=depth + 1, used=used + (node.id,))


def epistemic_adaptive_report(value: Mapping[str, Any]) -> EpistemicAdaptiveReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return EpistemicAdaptiveReport.from_wire(value)


__all__ = [
    "EPISTEMIC_ADAPTIVE_SCHEMA",
    "EPISTEMIC_ADAPTIVE_MAX_ACQUISITIONS",
    "EPISTEMIC_ADAPTIVE_MAX_STEPS",
    "EPISTEMIC_ADAPTIVE_MAX_POLICY_NODES",
    "EpistemicAdaptiveArgs",
    "EpistemicAdaptiveOutcomeReport",
    "EpistemicAdaptiveNodeReport",
    "EpistemicAdaptivePolicyReport",
    "EpistemicAdaptiveReport",
    "epistemic_adaptive_report",
]
