"""Typed decision-relative value-of-information projections.

The epistemic gateway prices explicit evidence against an explicit decision problem.  This
module keeps the objects that define the calculation separate from the report that resulted from
it: a loss matrix is not a utility claim, a belief is not a hidden prior, gross risk reduction is
not acquisition-worthiness, and a non-adaptive bundle is not an adaptive policy.  The parser also
retains the server's fail-closed refusal projection, because an improper likelihood partition or
an outcome-space explosion is useful negative evidence rather than an ordinary transport error.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


EPISTEMIC_MAX_ACTIONS = 1_000
EPISTEMIC_MAX_MODELS = 1_000
EPISTEMIC_MAX_OUTCOMES = 1_000
EPISTEMIC_MAX_ACQUISITIONS = 64
EPISTEMIC_MAX_INPUT_BYTES = 20_000_000
EPISTEMIC_LOSS_EPSILON = 1e-12


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _finite(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ArgumentError(f"{name} must be a finite number")
    parsed = float(value)
    if not math.isfinite(parsed):
        raise ArgumentError(f"{name} must be a finite number")
    return parsed


def _finite_array(name: str, value: Any) -> tuple[float, ...]:
    return tuple(_finite(f"{name}[{index}]", item) for index, item in enumerate(_array(name, value)))


def _text_array(name: str, value: Any) -> tuple[str, ...]:
    return tuple(_route_text(f"{name}[{index}]", item) for index, item in enumerate(_array(name, value)))


def _index(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract direct JSON, MCP structured content, or a REST tool envelope."""

    raw = _route_mapping("epistemic VOI response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if not isinstance(candidate.get("ok"), bool):
            return False
        if candidate.get("ok") is True:
            return isinstance(candidate.get("value"), Mapping) and isinstance(candidate.get("actions"), Mapping)
        return isinstance(candidate.get("stage"), str) and isinstance(candidate.get("refusal"), str)

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
                        raise ArgumentError(f"epistemic VOI response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain an epistemic value-of-information projection")


@dataclass(frozen=True)
class EpistemicDecisionProblemArgs:
    """Explicit actions, models, and row-major loss matrix for one decision."""

    actions: tuple[str, ...]
    models: tuple[str, ...]
    loss: tuple[float, ...]

    def __post_init__(self) -> None:
        actions = tuple(_route_text(f"epistemic actions[{index}]", value) for index, value in enumerate(self.actions))
        models = tuple(_route_text(f"epistemic models[{index}]", value) for index, value in enumerate(self.models))
        loss = tuple(_finite(f"epistemic loss[{index}]", value) for index, value in enumerate(self.loss))
        if not actions or not models:
            raise ArgumentError("epistemic decision problems require at least one action and model")
        if len(actions) > EPISTEMIC_MAX_ACTIONS or len(models) > EPISTEMIC_MAX_MODELS:
            raise ArgumentError("epistemic decision problems are bounded at 1000 actions and models")
        if len(actions) != len(set(actions)) or len(models) != len(set(models)):
            raise ArgumentError("epistemic actions and models must be unique")
        expected = len(actions) * len(models)
        if len(loss) != expected:
            raise ArgumentError(f"epistemic loss must contain {expected} row-major values")
        object.__setattr__(self, "actions", actions)
        object.__setattr__(self, "models", models)
        object.__setattr__(self, "loss", loss)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicDecisionProblemArgs":
        raw = _route_mapping("epistemic decision problem", value)
        return cls(
            _route_strings("epistemic actions", raw.get("actions")),
            _route_strings("epistemic models", raw.get("models")),
            _finite_array("epistemic loss", raw.get("loss")),
        )

    def to_wire(self) -> dict[str, Any]:
        return {"actions": list(self.actions), "models": list(self.models), "loss": list(self.loss)}


@dataclass(frozen=True)
class EpistemicBeliefArgs:
    """A positive finite mass vector; the Rust boundary performs normalization."""

    mass: tuple[float, ...]

    def __post_init__(self) -> None:
        mass = tuple(_finite(f"epistemic belief mass[{index}]", value) for index, value in enumerate(self.mass))
        if not mass or len(mass) > EPISTEMIC_MAX_MODELS:
            raise ArgumentError("epistemic belief must contain between 1 and 1000 masses")
        if any(value < 0.0 for value in mass) or sum(mass) <= 0.0:
            raise ArgumentError("epistemic belief masses must be non-negative and non-degenerate")
        object.__setattr__(self, "mass", mass)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicBeliefArgs":
        raw = _route_mapping("epistemic belief", value)
        return cls(_finite_array("epistemic belief mass", raw.get("mass")))

    def to_wire(self) -> dict[str, Any]:
        return {"mass": list(self.mass)}


@dataclass(frozen=True)
class EpistemicOutcomeArgs:
    """One labelled result and its per-model likelihood vector."""

    label: str
    likelihood: tuple[float, ...]

    def __post_init__(self) -> None:
        label = _route_text("epistemic outcome label", self.label)
        likelihood = tuple(
            _finite(f"epistemic outcome {label!r} likelihood[{index}]", value)
            for index, value in enumerate(self.likelihood)
        )
        if not likelihood or len(likelihood) > EPISTEMIC_MAX_MODELS:
            raise ArgumentError("epistemic outcome likelihood must contain between 1 and 1000 values")
        if any(value < 0.0 for value in likelihood):
            raise ArgumentError("epistemic outcome likelihoods must be non-negative")
        object.__setattr__(self, "label", label)
        object.__setattr__(self, "likelihood", likelihood)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicOutcomeArgs":
        raw = _route_mapping("epistemic outcome", value)
        return cls(_route_text("epistemic outcome label", raw.get("label")), _finite_array("epistemic outcome likelihood", raw.get("likelihood")))

    def to_wire(self) -> dict[str, Any]:
        return {"label": self.label, "likelihood": list(self.likelihood)}


@dataclass(frozen=True)
class EpistemicAcquisitionArgs:
    """An assay/evidence action with an explicit scalarized burden and outcomes."""

    id: str
    cost: float
    outcomes: tuple[EpistemicOutcomeArgs, ...]

    def __post_init__(self) -> None:
        identifier = _route_text("epistemic acquisition id", self.id)
        if len(identifier.encode("utf-8")) > 256:
            raise ArgumentError("epistemic acquisition ids must contain at most 256 UTF-8 bytes")
        cost = _finite("epistemic acquisition cost", self.cost)
        if cost < 0.0:
            raise ArgumentError("epistemic acquisition cost must be non-negative")
        outcomes = tuple(
            item if isinstance(item, EpistemicOutcomeArgs) else EpistemicOutcomeArgs.from_wire(item)
            for item in self.outcomes
        )
        if not 1 <= len(outcomes) <= EPISTEMIC_MAX_OUTCOMES:
            raise ArgumentError("epistemic acquisitions must contain between 1 and 1000 outcomes")
        object.__setattr__(self, "id", identifier)
        object.__setattr__(self, "cost", cost)
        object.__setattr__(self, "outcomes", outcomes)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicAcquisitionArgs":
        raw = _route_mapping("epistemic acquisition", value)
        return cls(
            _route_text("epistemic acquisition id", raw.get("id")),
            _finite("epistemic acquisition cost", raw.get("cost")),
            tuple(EpistemicOutcomeArgs.from_wire(item) for item in _array("epistemic acquisition outcomes", raw.get("outcomes"))),
        )

    def to_wire(self) -> dict[str, Any]:
        return {"id": self.id, "cost": self.cost, "outcomes": [item.to_wire() for item in self.outcomes]}


@dataclass(frozen=True)
class EpistemicVoiArgs:
    """One acquisition or a bounded non-adaptive acquisition bundle."""

    problem: EpistemicDecisionProblemArgs
    belief: EpistemicBeliefArgs
    acquisition: EpistemicAcquisitionArgs | None = None
    acquisitions: tuple[EpistemicAcquisitionArgs, ...] = ()

    def __post_init__(self) -> None:
        problem = self.problem if isinstance(self.problem, EpistemicDecisionProblemArgs) else EpistemicDecisionProblemArgs.from_wire(self.problem)
        belief = self.belief if isinstance(self.belief, EpistemicBeliefArgs) else EpistemicBeliefArgs.from_wire(self.belief)
        acquisition = None if self.acquisition is None else (self.acquisition if isinstance(self.acquisition, EpistemicAcquisitionArgs) else EpistemicAcquisitionArgs.from_wire(self.acquisition))
        acquisitions = tuple(
            item if isinstance(item, EpistemicAcquisitionArgs) else EpistemicAcquisitionArgs.from_wire(item)
            for item in self.acquisitions
        )
        if belief.mass and len(belief.mass) != len(problem.models):
            raise ArgumentError("epistemic belief length must match the decision problem models")
        if acquisition is not None and acquisitions:
            raise ArgumentError("provide acquisition or acquisitions, not both")
        selected = (acquisition,) if acquisition is not None else acquisitions
        if not 1 <= len(selected) <= EPISTEMIC_MAX_ACQUISITIONS:
            raise ArgumentError("epistemic VOI requires between 1 and 64 acquisitions")
        for item in selected:
            for outcome in item.outcomes:
                if len(outcome.likelihood) != len(problem.models):
                    raise ArgumentError("epistemic outcome likelihood length must match problem models")
            for model in range(len(problem.models)):
                partition = sum(outcome.likelihood[model] for outcome in item.outcomes)
                if not math.isclose(partition, 1.0, rel_tol=0.0, abs_tol=1e-9):
                    raise ArgumentError(
                        f"epistemic acquisition {item.id!r} likelihoods must sum to one for every model"
                    )
        object.__setattr__(self, "problem", problem)
        object.__setattr__(self, "belief", belief)
        object.__setattr__(self, "acquisition", acquisition)
        object.__setattr__(self, "acquisitions", acquisitions)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicVoiArgs":
        raw = _route_mapping("epistemic VOI arguments", value)
        raw_acquisition = raw.get("acquisition")
        raw_acquisitions = raw.get("acquisitions", ())
        return cls(
            EpistemicDecisionProblemArgs.from_wire(raw.get("problem")),
            EpistemicBeliefArgs.from_wire(raw.get("belief")),
            None if raw_acquisition is None else EpistemicAcquisitionArgs.from_wire(raw_acquisition),
            tuple(EpistemicAcquisitionArgs.from_wire(item) for item in _array("epistemic acquisitions", raw_acquisitions)),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments = {"problem": self.problem.to_wire(), "belief": self.belief.to_wire()}
        if self.acquisition is not None:
            arguments["acquisition"] = self.acquisition.to_wire()
        else:
            arguments["acquisitions"] = [item.to_wire() for item in self.acquisitions]
        return arguments


@dataclass(frozen=True)
class EpistemicValueReport:
    """Gross value, declared cost, net value, and decision identities."""

    raw: dict[str, Any]
    gross: float
    cost: float
    net: float
    outcome_probabilities: tuple[float, ...]
    action_without: int
    action_after: tuple[int, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicValueReport":
        raw = _route_mapping("epistemic value", value)
        gross = _finite("epistemic value gross", raw.get("gross"))
        cost = _finite("epistemic value cost", raw.get("cost"))
        net = _finite("epistemic value net", raw.get("net"))
        if cost < 0.0:
            raise ArgumentError("epistemic value cost cannot be negative")
        if gross < -1e-9:
            raise ArgumentError("epistemic gross value violated its non-negative guarantee")
        if not math.isclose(net, gross - cost, rel_tol=1e-9, abs_tol=1e-9):
            raise ArgumentError("epistemic gross, cost, and net values do not reconcile")
        probabilities = _finite_array("epistemic outcome probabilities", raw.get("outcome_probabilities"))
        if any(value < -1e-9 or value > 1.0 + 1e-9 for value in probabilities):
            raise ArgumentError("epistemic outcome probabilities must lie between zero and one")
        action_after = tuple(_index(f"epistemic action_after[{index}]", item) for index, item in enumerate(_array("epistemic action_after", raw.get("action_after"))))
        return cls(
            raw,
            gross,
            cost,
            net,
            probabilities,
            _index("epistemic action_without", raw.get("action_without")),
            action_after,
        )

    @property
    def changes_the_action(self) -> bool:
        return any(action != self.action_without for action in self.action_after)

    @property
    def worth_acquiring(self) -> bool:
        return self.net > 0.0

    @property
    def gross_non_negative(self) -> bool:
        return self.gross >= -1e-9


@dataclass(frozen=True)
class EpistemicActionsReport:
    raw: dict[str, Any]
    without: str
    after: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any], *, expected_after: int) -> "EpistemicActionsReport":
        raw = _route_mapping("epistemic actions", value)
        after = _text_array("epistemic actions.after", raw.get("after"))
        if len(after) != expected_after:
            raise ArgumentError("epistemic action identities do not match outcome count")
        return cls(raw, _route_text("epistemic actions.without", raw.get("without")), after)


@dataclass(frozen=True)
class EpistemicComplementarityReport:
    raw: dict[str, Any]
    joint_gross: float
    sum_of_singletons: float
    excess: float

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicComplementarityReport":
        raw = _route_mapping("epistemic complementarity", value)
        joint = _finite("epistemic complementarity joint_gross", raw.get("joint_gross"))
        singletons = _finite("epistemic complementarity sum_of_singletons", raw.get("sum_of_singletons"))
        excess = _finite("epistemic complementarity excess", raw.get("excess"))
        if not math.isclose(excess, joint - singletons, rel_tol=1e-9, abs_tol=1e-9):
            raise ArgumentError("epistemic complementarity excess does not reconcile")
        return cls(raw, joint, singletons, excess)

    @property
    def is_complementary(self) -> bool:
        return self.excess > EPISTEMIC_LOSS_EPSILON


@dataclass(frozen=True)
class EpistemicRefusalReport:
    """A domain refusal returned as structured success by the fail-closed gateway."""

    raw: dict[str, Any]
    ok: bool
    stage: str | None
    refusal: str
    fail_closed: bool
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicRefusalReport":
        raw = _route_mapping("epistemic refusal", value)
        if raw.get("ok") is not False:
            raise ArgumentError("epistemic refusal must have ok=false")
        if raw.get("fail_closed") is not True:
            raise ArgumentError("epistemic refusals must be fail-closed")
        stage_raw = raw.get("stage")
        stage = None if stage_raw is None else _route_text("epistemic refusal stage", stage_raw)
        return cls(
            raw,
            False,
            stage,
            _route_text("epistemic refusal", raw.get("refusal")),
            True,
            _route_strings("epistemic refusal guarantees", raw.get("guarantees", [])),
        )


@dataclass(frozen=True)
class EpistemicVoiReport:
    """Validated single- or bundle-level value-of-information evidence."""

    raw: dict[str, Any]
    ok: bool
    mode: str | None
    value: EpistemicValueReport | None
    actions: EpistemicActionsReport | None
    complementarity: EpistemicComplementarityReport | EpistemicRefusalReport | None
    guarantees: tuple[str, ...]
    refusal: EpistemicRefusalReport | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EpistemicVoiReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            refusal = EpistemicRefusalReport.from_wire(raw)
            return cls(raw, False, None, None, None, None, refusal.guarantees, refusal)
        if raw.get("ok") is not True:
            raise ArgumentError("epistemic VOI projection must declare ok")
        mode = _route_text("epistemic VOI mode", raw.get("mode"))
        if mode not in {"single", "non_adaptive_joint_bundle"}:
            raise ArgumentError(f"unknown epistemic VOI mode {mode!r}")
        value_report = EpistemicValueReport.from_wire(raw.get("value"))
        actions = EpistemicActionsReport.from_wire(raw.get("actions"), expected_after=len(value_report.action_after))
        comp_raw = raw.get("complementarity")
        if comp_raw is None:
            complementarity = None
        elif isinstance(comp_raw, Mapping) and comp_raw.get("ok") is False:
            comp_refusal = dict(comp_raw)
            comp_refusal.setdefault("stage", "complementarity")
            complementarity = EpistemicRefusalReport.from_wire(comp_refusal)
        else:
            complementarity = EpistemicComplementarityReport.from_wire(comp_raw)
        if mode == "single" and complementarity is not None:
            raise ArgumentError("single epistemic VOI projections cannot include complementarity")
        return cls(
            raw,
            True,
            mode,
            value_report,
            actions,
            complementarity,
            _route_strings("epistemic VOI guarantees", raw.get("guarantees", [])),
            None,
        )

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def is_bundle(self) -> bool:
        return self.mode == "non_adaptive_joint_bundle"

    @property
    def non_adaptive(self) -> bool:
        return self.is_bundle

    @property
    def gross_value(self) -> float | None:
        return None if self.value is None else self.value.gross

    @property
    def declared_cost(self) -> float | None:
        return None if self.value is None else self.value.cost

    @property
    def net_value(self) -> float | None:
        return None if self.value is None else self.value.net

    @property
    def action_changed(self) -> bool | None:
        return None if self.value is None else self.value.changes_the_action

    @property
    def worth_acquiring(self) -> bool | None:
        return None if self.value is None else self.value.worth_acquiring

    @property
    def complementarity_detected(self) -> bool | None:
        if isinstance(self.complementarity, EpistemicComplementarityReport):
            return self.complementarity.is_complementary
        if isinstance(self.complementarity, EpistemicRefusalReport):
            return None
        return None

    @property
    def fail_closed(self) -> bool:
        return self.refusal is not None and self.refusal.fail_closed

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def epistemic_voi_report(value: Mapping[str, Any]) -> EpistemicVoiReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return EpistemicVoiReport.from_wire(value)


__all__ = [
    "EPISTEMIC_MAX_ACTIONS",
    "EPISTEMIC_MAX_MODELS",
    "EPISTEMIC_MAX_OUTCOMES",
    "EPISTEMIC_MAX_ACQUISITIONS",
    "EPISTEMIC_MAX_INPUT_BYTES",
    "EPISTEMIC_LOSS_EPSILON",
    "EpistemicDecisionProblemArgs",
    "EpistemicBeliefArgs",
    "EpistemicOutcomeArgs",
    "EpistemicAcquisitionArgs",
    "EpistemicVoiArgs",
    "EpistemicValueReport",
    "EpistemicActionsReport",
    "EpistemicComplementarityReport",
    "EpistemicRefusalReport",
    "EpistemicVoiReport",
    "epistemic_voi_report",
]
