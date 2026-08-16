"""Typed developer-platform readiness projections.

The developer-platform check is deliberately more expressive than a health boolean.  It
distinguishes repository-checkable walkthrough claims from foreign artifacts, clean cookbook
verification from warning-bearing diagnostics, and declared change impact from a live dependency
watcher.  This module keeps those distinctions intact at the SDK boundary and reconciles the
bounded counts returned by the Rust kernel.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


DEVELOPER_PLATFORM_MAX_ITEMS = 1_000
WALKTHROUGH_STANDINGS = frozenset({"checkable_here", "partly_outside", "entirely_outside"})
WALKTHROUGH_STANDING_TEXT = {
    "checkable_here": "checkable here",
    "partly_outside": "partly outside",
    "entirely_outside": "entirely outside",
}


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _bounded_count(name: str, value: Any) -> int:
    return _route_count(name, value)


def _bounded_rows(name: str, value: Any) -> tuple[dict[str, Any], ...]:
    return tuple(_route_mapping(f"{name}[{index}]", row) for index, row in enumerate(_array(name, value)))


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract the projection from direct MCP output or an HTTP REST envelope."""

    raw = _route_mapping("developer platform response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        return candidate.get("ok") is True and isinstance(candidate.get("devplat"), Mapping)

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
                        raise ArgumentError(
                            f"developer platform response text is not JSON: {error}"
                        ) from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a developer platform projection")


@dataclass(frozen=True)
class DeveloperPlatformStatusArgs:
    """Bounded request for the in-repository developer-platform contract."""

    include_details: bool = False
    max_items: int = 100

    def __post_init__(self) -> None:
        if not isinstance(self.include_details, bool):
            raise ArgumentError("developer platform include_details must be a boolean")
        if (
            isinstance(self.max_items, bool)
            or not isinstance(self.max_items, int)
            or not 1 <= self.max_items <= DEVELOPER_PLATFORM_MAX_ITEMS
        ):
            raise ArgumentError(
                f"developer platform max_items must be between 1 and {DEVELOPER_PLATFORM_MAX_ITEMS}"
            )

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeveloperPlatformStatusArgs":
        raw = _route_mapping("developer platform arguments", value)
        return cls(raw.get("include_details", False), raw.get("max_items", 100))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"include_details": self.include_details, "max_items": self.max_items}


@dataclass(frozen=True)
class WalkthroughStatusReport:
    """One flattened walkthrough with claim-standing reconciliation."""

    raw: dict[str, Any]
    id: str
    goal: str
    standing: str
    standing_text: str
    steps: int
    claims: int
    guarded_claims: int
    unguarded_claims: int
    documents_absent_artifact: bool
    refuted_claims: int
    narration_permille: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WalkthroughStatusReport":
        raw = _route_mapping("developer walkthrough", value)
        standing = _route_mapping("developer walkthrough standing", raw.get("standing"))
        standing_kind = _route_text("developer walkthrough standing.standing", standing.get("standing"))
        if standing_kind not in WALKTHROUGH_STANDINGS:
            raise ArgumentError(f"unknown developer walkthrough standing {standing_kind!r}")
        standing_text = _route_text("developer walkthrough standing_text", raw.get("standing_text"))
        if standing_text != WALKTHROUGH_STANDING_TEXT[standing_kind]:
            raise ArgumentError("developer walkthrough standing_text does not match standing")
        steps = _bounded_count("developer walkthrough steps", raw.get("steps"))
        claims = _bounded_count("developer walkthrough claims", raw.get("claims"))
        guarded = _bounded_count("developer walkthrough guarded_claims", raw.get("guarded_claims"))
        unguarded = _bounded_count("developer walkthrough unguarded_claims", raw.get("unguarded_claims"))
        if standing_kind == "checkable_here":
            expected = (claims, 0)
        elif standing_kind == "partly_outside":
            expected = (
                _bounded_count("developer walkthrough standing.here", standing.get("here")),
                _bounded_count("developer walkthrough standing.outside", standing.get("outside")),
            )
        else:
            expected = (
                0,
                _bounded_count("developer walkthrough standing.claims", standing.get("claims")),
            )
        if (guarded, unguarded) != expected or guarded + unguarded != claims:
            raise ArgumentError("developer walkthrough claim counts do not reconcile with standing")
        if claims > steps:
            raise ArgumentError("developer walkthrough claims cannot exceed steps")
        documents_absent = _bool(
            "developer walkthrough documents_absent_artifact", raw.get("documents_absent_artifact")
        )
        if documents_absent != (standing_kind == "entirely_outside"):
            raise ArgumentError("developer walkthrough absent-artifact flag does not reconcile with standing")
        refuted = _bounded_count("developer walkthrough refuted_claims", raw.get("refuted_claims"))
        if refuted > claims:
            raise ArgumentError("developer walkthrough refuted_claims cannot exceed claims")
        narration = _bounded_count("developer walkthrough narration_permille", raw.get("narration_permille"))
        if narration > 1_000:
            raise ArgumentError("developer walkthrough narration_permille cannot exceed 1000")
        return cls(
            raw,
            _route_text("developer walkthrough id", raw.get("id")),
            _route_text("developer walkthrough goal", raw.get("goal")),
            standing_kind,
            standing_text,
            steps,
            claims,
            guarded,
            unguarded,
            documents_absent,
            refuted,
            narration,
        )

    @property
    def claims_guarded(self) -> bool:
        return self.unguarded_claims == 0


@dataclass(frozen=True)
class DeveloperPlatformSummaryReport:
    """The compact classification ledger returned in every status report."""

    raw: dict[str, Any]
    digest: str
    verdict_counts: tuple[int, int, int, int]
    modules_classified: int
    implemented_count: int
    not_implemented_count: int
    foreign_subject_count: int
    walkthrough_count: int
    guarded_claims: int
    unguarded_claims: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeveloperPlatformSummaryReport":
        raw = _route_mapping("developer platform summary", value)
        verdict_counts = tuple(
            _bounded_count(f"developer platform verdict_counts[{index}]", item)
            for index, item in enumerate(_array("developer platform verdict_counts", raw.get("verdict_counts")))
        )
        if len(verdict_counts) != 4:
            raise ArgumentError("developer platform verdict_counts must contain four buckets")
        modules = _bounded_count("developer platform modules_classified", raw.get("modules_classified"))
        if modules != sum(verdict_counts):
            raise ArgumentError("developer platform module counts do not reconcile")
        implemented = _bounded_count("developer platform implemented_count", raw.get("implemented_count"))
        not_implemented = _bounded_count(
            "developer platform not_implemented_count", raw.get("not_implemented_count")
        )
        if implemented + not_implemented != modules:
            raise ArgumentError("developer platform implemented and unimplemented counts do not reconcile")
        return cls(
            raw,
            _route_text("developer platform digest", raw.get("digest")),
            verdict_counts,  # type: ignore[arg-type]
            modules,
            implemented,
            not_implemented,
            _bounded_count("developer platform foreign_subject_count", raw.get("foreign_subject_count")),
            _bounded_count("developer platform walkthrough_count", raw.get("walkthrough_count")),
            _bounded_count("developer platform guarded_claims", raw.get("guarded_claims")),
            _bounded_count("developer platform unguarded_claims", raw.get("unguarded_claims")),
        )


@dataclass(frozen=True)
class CookbookVerificationReport:
    raw: dict[str, Any]
    clean: bool
    crates_checked: int
    entry_points_checked: int
    tests_checked: int
    quotes_checked: int
    defect_count: int
    defects_returned: tuple[Any, ...]
    omitted_defects: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CookbookVerificationReport":
        raw = _route_mapping("cookbook verification", value)
        defects = _array("cookbook defects_returned", raw.get("defects_returned", []))
        defect_count = _bounded_count("cookbook defect_count", raw.get("defect_count"))
        omitted = _bounded_count("cookbook omitted_defects", raw.get("omitted_defects"))
        if defect_count != len(defects) + omitted:
            raise ArgumentError("cookbook defect counts do not reconcile")
        return cls(
            raw,
            _bool("cookbook verification clean", raw.get("clean")),
            _bounded_count("cookbook crates_checked", raw.get("crates_checked")),
            _bounded_count("cookbook entry_points_checked", raw.get("entry_points_checked")),
            _bounded_count("cookbook tests_checked", raw.get("tests_checked")),
            _bounded_count("cookbook quotes_checked", raw.get("quotes_checked")),
            defect_count,
            defects,
            omitted,
        )


@dataclass(frozen=True)
class CookbookStatusReport:
    raw: dict[str, Any]
    recipes: int
    anti_recipes: int
    crates: tuple[str, ...]
    enforcing_tests: int
    quotes: int
    verification: CookbookVerificationReport

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CookbookStatusReport":
        raw = _route_mapping("developer cookbook", value)
        return cls(
            raw,
            _bounded_count("cookbook recipes", raw.get("recipes")),
            _bounded_count("cookbook anti_recipes", raw.get("anti_recipes")),
            _route_strings("cookbook crates", raw.get("crates", [])),
            _bounded_count("cookbook enforcing_tests", raw.get("enforcing_tests")),
            _bounded_count("cookbook quotes", raw.get("quotes")),
            CookbookVerificationReport.from_wire(raw.get("verification", {})),
        )


@dataclass(frozen=True)
class DeveloperContractSurfaceReport:
    raw: dict[str, Any]
    id: str
    owns_count: int
    invalidates_count: int
    rationale: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeveloperContractSurfaceReport":
        raw = _route_mapping("developer contract surface", value)
        return cls(
            raw,
            _route_text("developer contract surface id", raw.get("id")),
            _bounded_count("developer contract owns_count", raw.get("owns_count")),
            _bounded_count("developer contract invalidates_count", raw.get("invalidates_count")),
            _route_text("developer contract rationale", raw.get("rationale")),
        )


@dataclass(frozen=True)
class DeveloperContractSummaryReport:
    raw: dict[str, Any]
    surface_count: int
    surfaces_returned: tuple[DeveloperContractSurfaceReport, ...]
    omitted_surfaces: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeveloperContractSummaryReport":
        raw = _route_mapping("developer contract summary", value)
        surfaces = tuple(
            DeveloperContractSurfaceReport.from_wire(row)
            for row in _array("developer contract surfaces_returned", raw.get("surfaces_returned", []))
        )
        count = _bounded_count("developer contract surface_count", raw.get("surface_count"))
        omitted = _bounded_count("developer contract omitted_surfaces", raw.get("omitted_surfaces"))
        if count != len(surfaces) + omitted:
            raise ArgumentError("developer contract surface counts do not reconcile")
        return cls(raw, count, surfaces, omitted)


@dataclass(frozen=True)
class DiagnosticCatalogueReport:
    raw: dict[str, Any]
    clean: bool
    checked: int
    errors: int
    warnings: int
    finding_count: int
    findings_returned: tuple[dict[str, Any], ...]
    omitted_findings: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DiagnosticCatalogueReport":
        raw = _route_mapping("diagnostic catalogue", value)
        clean = _bool("diagnostic catalogue clean", raw.get("clean"))
        checked = _bounded_count("diagnostic catalogue checked", raw.get("checked"))
        errors = _bounded_count("diagnostic catalogue errors", raw.get("errors"))
        warnings = _bounded_count("diagnostic catalogue warnings", raw.get("warnings"))
        if errors > checked or clean != (errors == 0):
            raise ArgumentError("diagnostic catalogue health does not reconcile")
        findings = _bounded_rows("diagnostic catalogue findings_returned", raw.get("findings_returned", []))
        finding_count = _bounded_count("diagnostic catalogue finding_count", raw.get("finding_count"))
        omitted = _bounded_count("diagnostic catalogue omitted_findings", raw.get("omitted_findings"))
        if finding_count != len(findings) + omitted:
            raise ArgumentError("diagnostic catalogue finding counts do not reconcile")
        return cls(raw, clean, checked, errors, warnings, finding_count, findings, omitted)


@dataclass(frozen=True)
class ExitCodeAuditReport:
    raw: dict[str, Any]
    clean: bool
    retry_decision_recoverable_from_code_alone: bool
    divergence_count: int
    divergences_returned: tuple[dict[str, Any], ...]
    omitted_divergences: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ExitCodeAuditReport":
        raw = _route_mapping("exit code audit", value)
        divergences = _bounded_rows("exit code divergences_returned", raw.get("divergences_returned", []))
        divergence_count = _bounded_count("exit code divergence_count", raw.get("divergence_count"))
        omitted = _bounded_count("exit code omitted_divergences", raw.get("omitted_divergences"))
        if divergence_count != len(divergences) + omitted:
            raise ArgumentError("exit code divergence counts do not reconcile")
        return cls(
            raw,
            _bool("exit code audit clean", raw.get("clean")),
            _bool(
                "exit code retry_decision_recoverable_from_code_alone",
                raw.get("retry_decision_recoverable_from_code_alone"),
            ),
            divergence_count,
            divergences,
            omitted,
        )


@dataclass(frozen=True)
class DeveloperPlatformDetailsReport:
    """Full-detail evidence, retained with typed top-level collections."""

    raw: dict[str, Any]
    devplat: dict[str, Any]
    cookbook_verification: dict[str, Any]
    developer_contract: tuple[dict[str, Any], ...]
    diagnostic_findings: tuple[dict[str, Any], ...]
    exit_code_divergences: tuple[dict[str, Any], ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeveloperPlatformDetailsReport":
        raw = _route_mapping("developer platform details", value)
        devplat = _route_mapping("developer platform details.devplat", raw.get("devplat"))
        _route_text("developer platform details.devplat.digest", devplat.get("digest"))
        _route_count("developer platform details.devplat.guarded_claims", devplat.get("guarded_claims"))
        _route_count("developer platform details.devplat.unguarded_claims", devplat.get("unguarded_claims"))
        return cls(
            raw,
            devplat,
            _route_mapping("developer platform details.cookbook_verification", raw.get("cookbook_verification")),
            _bounded_rows("developer platform details.developer_contract", raw.get("developer_contract", [])),
            _bounded_rows("developer platform details.diagnostic_findings", raw.get("diagnostic_findings", [])),
            _bounded_rows("developer platform details.exit_code_divergences", raw.get("exit_code_divergences", [])),
        )


@dataclass(frozen=True)
class DeveloperPlatformStatusReport:
    """Validated developer-platform status with explicit readiness properties."""

    raw: dict[str, Any]
    ok: bool
    root: str
    detail_mode: str
    max_items: int
    devplat: DeveloperPlatformSummaryReport
    walkthroughs: tuple[WalkthroughStatusReport, ...]
    cookbook: CookbookStatusReport
    developer_contract: DeveloperContractSummaryReport
    diagnostic_catalogue: DiagnosticCatalogueReport
    exit_code_audit: ExitCodeAuditReport
    limitations: tuple[str, ...]
    details: DeveloperPlatformDetailsReport | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeveloperPlatformStatusReport":
        raw = _payload(value)
        ok = _bool("developer platform ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("developer platform status projection must be successful")
        mode = _route_text("developer platform detail_mode", raw.get("detail_mode"))
        if mode not in {"summary", "full"}:
            raise ArgumentError("developer platform detail_mode must be summary or full")
        max_items = _bounded_count("developer platform max_items", raw.get("max_items"))
        if not 1 <= max_items <= DEVELOPER_PLATFORM_MAX_ITEMS:
            raise ArgumentError("developer platform max_items is outside the protocol bound")
        devplat = DeveloperPlatformSummaryReport.from_wire(raw.get("devplat", {}))
        walkthroughs = tuple(
            WalkthroughStatusReport.from_wire(row)
            for row in _array("developer platform walkthroughs", raw.get("walkthroughs", []))
        )
        if devplat.walkthrough_count != len(walkthroughs):
            raise ArgumentError("developer platform walkthrough count does not reconcile")
        if devplat.guarded_claims != sum(row.guarded_claims for row in walkthroughs):
            raise ArgumentError("developer platform guarded claim total does not reconcile")
        if devplat.unguarded_claims != sum(row.unguarded_claims for row in walkthroughs):
            raise ArgumentError("developer platform unguarded claim total does not reconcile")
        details_raw = raw.get("details")
        details = None if details_raw is None else DeveloperPlatformDetailsReport.from_wire(details_raw)
        if mode == "full" and details is None:
            raise ArgumentError("full developer platform reports must include details")
        if details is not None and details.devplat.get("digest") != devplat.digest:
            raise ArgumentError("developer platform detail digest does not match summary digest")
        return cls(
            raw,
            ok,
            _route_text("developer platform root", raw.get("root")),
            mode,
            max_items,
            devplat,
            walkthroughs,
            CookbookStatusReport.from_wire(raw.get("cookbook", {})),
            DeveloperContractSummaryReport.from_wire(raw.get("developer_contract", {})),
            DiagnosticCatalogueReport.from_wire(raw.get("diagnostic_catalogue", {})),
            ExitCodeAuditReport.from_wire(raw.get("exit_code_audit", {})),
            _route_strings("developer platform limitations", raw.get("limitations", [])),
            details,
        )

    @property
    def platform_checks_clean(self) -> bool:
        return (
            self.cookbook.verification.clean
            and self.diagnostic_catalogue.clean
            and self.exit_code_audit.clean
        )

    @property
    def claims_guarded(self) -> bool:
        return self.devplat.unguarded_claims == 0

    @property
    def foreign_artifacts_present(self) -> bool:
        return self.devplat.foreign_subject_count > 0 or any(
            walkthrough.documents_absent_artifact for walkthrough in self.walkthroughs
        )

    @property
    def complete_summary(self) -> bool:
        return (
            self.cookbook.verification.omitted_defects == 0
            and self.developer_contract.omitted_surfaces == 0
            and self.diagnostic_catalogue.omitted_findings == 0
            and self.exit_code_audit.omitted_divergences == 0
        )

    @property
    def details_available(self) -> bool:
        return self.details is not None

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def developer_platform_status_report(value: Mapping[str, Any]) -> DeveloperPlatformStatusReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return DeveloperPlatformStatusReport.from_wire(value)


__all__ = [
    "DEVELOPER_PLATFORM_MAX_ITEMS",
    "WALKTHROUGH_STANDINGS",
    "DeveloperPlatformStatusArgs",
    "WalkthroughStatusReport",
    "DeveloperPlatformSummaryReport",
    "CookbookVerificationReport",
    "CookbookStatusReport",
    "DeveloperContractSurfaceReport",
    "DeveloperContractSummaryReport",
    "DiagnosticCatalogueReport",
    "ExitCodeAuditReport",
    "DeveloperPlatformDetailsReport",
    "DeveloperPlatformStatusReport",
    "developer_platform_status_report",
]
