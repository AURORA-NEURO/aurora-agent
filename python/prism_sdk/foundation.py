"""Typed foundation-contract gate projections.

``foundation_contract_check`` validates declarations at several different boundaries.  The
contract itself must be falsifiable and evaluable; an optional child must refine its parent; an
applicability envelope must be complete and mature enough for the requested presentation; a BioWorld
must license the requested counterfactual and reveal policy; and a transition must not confuse a
measurement or evaluation operation with a latent biological effect.  This module keeps those
gates independent and never turns a declared contract into evidence, treatment authority, or
execution.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


FOUNDATION_MAX_INPUT_BYTES = 20_000_000
COUNTERFACTUAL_CLAIMS = frozenset(
    {
        "associational",
        "analysis_fork",
        "injected_factor_effect",
        "simulated_intervention",
        "reveal_prediction",
        "specified_ground_truth",
        "real_treatment_effect",
    }
)
FOUNDATION_VERDICTS = frozenset({"admitted", "refused"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract direct JSON, MCP structured content, or a REST tool envelope."""

    raw = _route_mapping("foundation response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        return candidate.get("ok") is True and candidate.get("verdict") in FOUNDATION_VERDICTS and isinstance(candidate.get("contract"), Mapping)

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
            if isinstance(content, list):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"foundation response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a foundation-contract projection")


@dataclass(frozen=True)
class FoundationContractCheckArgs:
    """Serialized foundation inputs with explicit optional gate controls."""

    contract: Mapping[str, Any]
    parent: Mapping[str, Any] | None = None
    envelope: Mapping[str, Any] | None = None
    present_as_established: bool = False
    world: Mapping[str, Any] | None = None
    claim: str | None = None
    transition: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        contract = _route_mapping("foundation contract", self.contract)
        parent = None if self.parent is None else _route_mapping("foundation parent", self.parent)
        envelope = None if self.envelope is None else _route_mapping("foundation applicability envelope", self.envelope)
        world = None if self.world is None else _route_mapping("foundation world", self.world)
        transition = None if self.transition is None else _route_mapping("foundation transition", self.transition)
        present = _bool("foundation present_as_established", self.present_as_established)
        claim = _optional_text("foundation claim", self.claim)
        if claim is not None and claim not in COUNTERFACTUAL_CLAIMS:
            raise ArgumentError(f"unknown foundation counterfactual claim {claim!r}")
        arguments = {"contract": contract, "parent": parent, "envelope": envelope, "world": world, "claim": claim, "transition": transition}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"foundation arguments are not JSON serializable: {error}") from error
        if len(encoded) > FOUNDATION_MAX_INPUT_BYTES:
            raise ArgumentError("foundation input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "contract", contract)
        object.__setattr__(self, "parent", parent)
        object.__setattr__(self, "envelope", envelope)
        object.__setattr__(self, "present_as_established", present)
        object.__setattr__(self, "world", world)
        object.__setattr__(self, "claim", claim)
        object.__setattr__(self, "transition", transition)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FoundationContractCheckArgs":
        raw = _route_mapping("foundation arguments", value)
        return cls(
            _route_mapping("foundation contract", raw.get("contract")),
            None if raw.get("parent") is None else _route_mapping("foundation parent", raw.get("parent")),
            None if raw.get("envelope") is None else _route_mapping("foundation applicability envelope", raw.get("envelope")),
            raw.get("present_as_established", False),
            None if raw.get("world") is None else _route_mapping("foundation world", raw.get("world")),
            _optional_text("foundation claim", raw.get("claim")),
            None if raw.get("transition") is None else _route_mapping("foundation transition", raw.get("transition")),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {"contract": dict(self.contract), "present_as_established": self.present_as_established}
        for name, value in (("parent", self.parent), ("envelope", self.envelope), ("world", self.world), ("transition", self.transition)):
            if value is not None:
                arguments[name] = dict(value)
        if self.claim is not None:
            arguments["claim"] = self.claim
        return arguments


@dataclass(frozen=True)
class FoundationContractGateReport:
    raw: dict[str, Any]
    ok: bool
    contract_id: str | None
    intent: str | None
    falsifier_count: int | None
    action_count: int | None
    evidence_obligation_count: int | None
    minimum_reviewers: int | None
    uncertainty_required: bool | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FoundationContractGateReport":
        raw = _route_mapping("foundation contract gate", value)
        ok = _bool("foundation contract gate ok", raw.get("ok"))
        if not ok:
            return cls(raw, False, None, None, None, None, None, None, None, _route_text("foundation contract refusal", raw.get("refusal")), _bool("foundation contract fail_closed", raw.get("fail_closed")))
        return cls(
            raw,
            True,
            _route_text("foundation contract id", raw.get("id")),
            _route_text("foundation contract intent", raw.get("intent")),
            _route_count("foundation contract falsifier_count", raw.get("falsifier_count")),
            _route_count("foundation contract action_count", raw.get("action_count")),
            _route_count("foundation contract evidence_obligation_count", raw.get("evidence_obligation_count")),
            _route_count("foundation contract minimum_reviewers", raw.get("minimum_reviewers")),
            _bool("foundation contract uncertainty_required", raw.get("uncertainty_required")),
            None,
            False,
        )


@dataclass(frozen=True)
class FoundationParentRelationReport:
    raw: dict[str, Any]
    ok: bool
    relation: str
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FoundationParentRelationReport":
        raw = _route_mapping("foundation parent relation", value)
        ok = _bool("foundation parent relation ok", raw.get("ok"))
        relation = _route_text("foundation parent relation", raw.get("relation"))
        if ok and relation != "refines":
            raise ArgumentError("successful foundation parent relation must be refines")
        if not ok and relation != "refused":
            raise ArgumentError("refused foundation parent relation must be refused")
        return cls(raw, ok, relation, None if ok else _route_text("foundation parent refusal", raw.get("refusal")), False if ok else _bool("foundation parent fail_closed", raw.get("fail_closed")))


@dataclass(frozen=True)
class FoundationEnvelopeReport:
    raw: dict[str, Any]
    ok: bool
    structure: str
    maturity: str
    maturity_rung: str
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FoundationEnvelopeReport":
        raw = _route_mapping("foundation envelope check", value)
        return cls(
            raw,
            _bool("foundation envelope ok", raw.get("ok")),
            _route_text("foundation envelope structure", raw.get("structure")),
            _route_text("foundation envelope maturity", raw.get("maturity")),
            _route_text("foundation envelope maturity_rung", raw.get("maturity_rung")),
            _bool("foundation envelope fail_closed", raw.get("fail_closed")),
        )


@dataclass(frozen=True)
class FoundationWorldReport:
    raw: dict[str, Any]
    ok: bool
    world_id: str
    world_class: str
    counterfactual_strength: str
    reveal_policy: str
    claim: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FoundationWorldReport":
        raw = _route_mapping("foundation world check", value)
        return cls(
            raw,
            _bool("foundation world ok", raw.get("ok")),
            _route_text("foundation world id", raw.get("world_id")),
            _route_text("foundation world class", raw.get("class")),
            _route_text("foundation counterfactual strength", raw.get("counterfactual_strength")),
            _route_text("foundation reveal policy", raw.get("reveal_policy")),
            _optional_text("foundation world claim", raw.get("claim")),
            _bool("foundation world fail_closed", raw.get("fail_closed")),
        )


@dataclass(frozen=True)
class FoundationTransitionReport:
    raw: dict[str, Any]
    ok: bool
    verdict: str
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FoundationTransitionReport":
        raw = _route_mapping("foundation transition check", value)
        ok = _bool("foundation transition ok", raw.get("ok"))
        verdict = _route_text("foundation transition verdict", raw.get("verdict"))
        if ok and verdict != "plane_consistent":
            raise ArgumentError("successful foundation transition must be plane_consistent")
        if not ok and verdict != "plane_confusion":
            raise ArgumentError("refused foundation transition must be plane_confusion")
        return cls(raw, ok, verdict, None if ok else _route_text("foundation transition refusal", raw.get("refusal")), False if ok else _bool("foundation transition fail_closed", raw.get("fail_closed")))


@dataclass(frozen=True)
class FoundationContractCheckReport:
    """Independent foundation gates plus their aggregate verdict."""

    raw: dict[str, Any]
    ok: bool
    verdict: str
    contract: FoundationContractGateReport
    parent_relation: FoundationParentRelationReport | None
    envelope: FoundationEnvelopeReport | None
    world: FoundationWorldReport | None
    transition: FoundationTransitionReport | None
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FoundationContractCheckReport":
        raw = _payload(value)
        verdict = _route_text("foundation verdict", raw.get("verdict"))
        if verdict not in FOUNDATION_VERDICTS:
            raise ArgumentError(f"unknown foundation verdict {verdict!r}")
        parent_raw = raw.get("parent_relation")
        envelope_raw = raw.get("envelope")
        world_raw = raw.get("world")
        transition_raw = raw.get("transition")
        return cls(
            raw,
            _bool("foundation top-level ok", raw.get("ok")),
            verdict,
            FoundationContractGateReport.from_wire(raw.get("contract")),
            None if parent_raw is None else FoundationParentRelationReport.from_wire(parent_raw),
            None if envelope_raw is None else FoundationEnvelopeReport.from_wire(envelope_raw),
            None if world_raw is None else FoundationWorldReport.from_wire(world_raw),
            None if transition_raw is None else FoundationTransitionReport.from_wire(transition_raw),
            _route_strings("foundation guarantees", raw.get("guarantees", [])),
        )

    @property
    def contract_admissible(self) -> bool:
        return self.contract.ok

    @property
    def optional_gates_clear(self) -> bool:
        return all(gate is None or gate.ok for gate in (self.parent_relation, self.envelope, self.world, self.transition))

    @property
    def admitted(self) -> bool:
        return self.ok and self.verdict == "admitted" and self.contract_admissible and self.optional_gates_clear

    @property
    def refused(self) -> bool:
        return self.verdict == "refused" or not self.contract_admissible

    @property
    def fail_closed(self) -> bool:
        return self.refused or any(gate is not None and not gate.ok for gate in (self.parent_relation, self.envelope, self.world, self.transition))

    @property
    def world_claim_admitted(self) -> bool | None:
        if self.world is None or self.world.claim is None:
            return None
        return self.world.ok and self.world.claim == "admitted"

    @property
    def transition_plane_consistent(self) -> bool | None:
        return None if self.transition is None else self.transition.ok

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def foundation_contract_check_report(value: Mapping[str, Any]) -> FoundationContractCheckReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return FoundationContractCheckReport.from_wire(value)


__all__ = [
    "FOUNDATION_MAX_INPUT_BYTES",
    "COUNTERFACTUAL_CLAIMS",
    "FOUNDATION_VERDICTS",
    "FoundationContractCheckArgs",
    "FoundationContractGateReport",
    "FoundationParentRelationReport",
    "FoundationEnvelopeReport",
    "FoundationWorldReport",
    "FoundationTransitionReport",
    "FoundationContractCheckReport",
    "foundation_contract_check_report",
]
