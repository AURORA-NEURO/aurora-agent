"""Typed boundary for declared modality transports and loss ledgers."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .capability import _route_mapping, _route_text
from .errors import ArgumentError
from .modality import MODALITIES, MODALITY_CLAIMS, MODALITY_RESOLUTIONS


MODALITY_TRANSPORT_SCHEMA = "bioprism-mcp/modality-transport-check/0.1"
MODALITY_TRANSPORT_OUTCOME_KINDS = frozenset({"constructed", "refused"})
MODALITY_TRANSPORT_KINDS = frozenset({"aggregation", "deconvolution", "imputation"})
AGGREGATION_OPERATORS = frozenset({"sum", "max", "min", "mean", "existential", "universal", "statistical_estimate", "domain_specific"})


def _object(name: str, value: Any) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be an object")
    return dict(value)


def _rows(name: str, value: Any, *, maximum: int) -> tuple[dict[str, Any], ...]:
    if value is None:
        return ()
    if not isinstance(value, (list, tuple)):
        raise ArgumentError(f"{name} must be an array")
    if len(value) > maximum:
        raise ArgumentError(f"{name} exceeds the {maximum}-item safety bound")
    return tuple(_object(f"{name} item", item) if isinstance(item, Mapping) else _raise_row(name) for item in value)


def _raise_row(name: str) -> dict[str, Any]:
    raise ArgumentError(f"{name} items must be objects")


@dataclass(frozen=True)
class ModalityTransportCheckArgs:
    from_modality: str
    to: str
    axis: str
    transport: Mapping[str, Any]
    source_descriptor: Mapping[str, Any] | None = None
    claims: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        source = _route_text("source modality", self.from_modality)
        target = _route_text("destination modality", self.to)
        axis = _route_text("transport axis", self.axis)
        if source not in MODALITIES or target not in MODALITIES:
            raise ArgumentError("unknown source or destination modality")
        if axis not in MODALITY_RESOLUTIONS:
            raise ArgumentError(f"unknown transport axis: {axis!r}")
        transport = _object("transport kind", self.transport)
        kind = _route_text("transport kind", transport.get("kind"))
        if kind not in MODALITY_TRANSPORT_KINDS:
            raise ArgumentError(f"unknown modality transport kind: {kind!r}")
        if kind == "aggregation" and transport.get("operator") not in AGGREGATION_OPERATORS:
            raise ArgumentError("aggregation transport must declare a valid operator")
        if kind == "deconvolution":
            reference = _route_text("deconvolution reference", transport.get("reference"))
            if not reference.strip() or transport.get("recomposition") not in AGGREGATION_OPERATORS:
                raise ArgumentError("deconvolution transport must declare a reference and recomposition operator")
        if kind == "imputation" and not _route_text("imputation model", transport.get("model")).strip():
            raise ArgumentError("imputation transport must declare a model")
        descriptor = None if self.source_descriptor is None else _object("source descriptor", self.source_descriptor)
        claims = tuple(self.claims)
        if len(claims) > 20:
            raise ArgumentError("transport claims exceed the 20-item safety bound")
        for claim in claims:
            if _route_text("transport claim", claim) not in MODALITY_CLAIMS:
                raise ArgumentError(f"unknown transport claim: {claim!r}")
        object.__setattr__(self, "from_modality", source)
        object.__setattr__(self, "to", target)
        object.__setattr__(self, "axis", axis)
        object.__setattr__(self, "transport", transport)
        object.__setattr__(self, "source_descriptor", descriptor)
        object.__setattr__(self, "claims", claims)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ModalityTransportCheckArgs":
        raw = _object("modality transport arguments", value)
        claims = raw.get("claims")
        if claims is None:
            claim_values: tuple[str, ...] = ()
        elif isinstance(claims, (list, tuple)):
            claim_values = tuple(claims)
        else:
            raise ArgumentError("transport claims must be an array")
        return cls(raw.get("from"), raw.get("to"), raw.get("axis"), raw.get("transport"), raw.get("source_descriptor"), claim_values)

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "from": self.from_modality,
            "to": self.to,
            "axis": self.axis,
            "transport": dict(self.transport),
        }
        if self.source_descriptor is not None:
            result["source_descriptor"] = dict(self.source_descriptor)
        if self.claims:
            result["claims"] = list(self.claims)
        return result


@dataclass(frozen=True)
class ModalityTransportCheckReport:
    raw: dict[str, Any]
    ok: bool
    outcome_kind: str
    constructed: bool
    from_modality: str
    to: str
    axis: str
    transport: dict[str, Any]
    claims: tuple[dict[str, Any], ...]
    application: dict[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ModalityTransportCheckReport":
        raw = _object("modality transport report", value)
        if raw.get("ok") is not True:
            raise ArgumentError("modality transport report transport projection is not successful")
        if raw.get("schema") != MODALITY_TRANSPORT_SCHEMA:
            raise ArgumentError(f"unknown modality transport schema: {raw.get('schema')!r}")
        outcome_kind = _route_text("modality transport outcome kind", raw.get("outcome_kind"))
        if outcome_kind not in MODALITY_TRANSPORT_OUTCOME_KINDS:
            raise ArgumentError(f"unknown modality transport outcome kind: {outcome_kind!r}")
        constructed = raw.get("constructed")
        if not isinstance(constructed, bool) or constructed != (outcome_kind == "constructed"):
            raise ArgumentError("modality transport outcome and constructed flag do not reconcile")
        from_modality = _route_text("modality transport source", raw.get("from"))
        to = _route_text("modality transport destination", raw.get("to"))
        axis = _route_text("modality transport axis", raw.get("axis"))
        transport = _route_mapping("modality transport", raw.get("transport"))
        claims = _rows("modality transport claims", raw.get("claims", []), maximum=20)
        application = _route_mapping("modality transport application", raw.get("application"))
        if constructed and application.get("applied") is not True:
            raise ArgumentError("constructed modality transports must retain an applied descriptor")
        if not constructed and raw.get("transport_evidence") is None:
            raise ArgumentError("refused modality transports must retain typed transport evidence")
        return cls(raw, True, outcome_kind, constructed, from_modality, to, axis, transport, claims, application)


def modality_transport_check_report(value: Mapping[str, Any]) -> ModalityTransportCheckReport:
    return ModalityTransportCheckReport.from_wire(value)


__all__ = [
    "MODALITY_TRANSPORT_SCHEMA",
    "MODALITY_TRANSPORT_OUTCOME_KINDS",
    "MODALITY_TRANSPORT_KINDS",
    "AGGREGATION_OPERATORS",
    "ModalityTransportCheckArgs",
    "ModalityTransportCheckReport",
    "modality_transport_check_report",
]
