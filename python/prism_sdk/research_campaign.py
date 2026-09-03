"""Strict Python surface for the path-only offline research-campaign MCP tool.

The authoritative runner and native receipt verification remain in Rust.  This module transports
only workspace-relative locators and validates the metadata-only result projection; it never reads
campaign specifications, stage inputs, checkpoints, or research artifacts itself.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import re
from types import MappingProxyType
from typing import Any, Awaitable, Callable, Mapping

from .errors import ArgumentError, ProtocolError, ToolRefusal


RESEARCH_CAMPAIGN_OFFLINE_TOOL = "research_campaign_run_offline"
RESEARCH_CAMPAIGN_OFFLINE_RESULT_SCHEMA = "bioprism-mcp/research-campaign-offline-run/0.1"
RESEARCH_CAMPAIGN_CHECKPOINT_SCHEMA = "bioprism-research-campaign-checkpoint/0.1"
MAX_RESEARCH_CAMPAIGN_OFFLINE_STAGES = 8
MAX_RESEARCH_CAMPAIGN_OFFLINE_WRITTEN_PATHS = 20
MAX_RESEARCH_CAMPAIGN_OFFLINE_LIMITATIONS = 8
MAX_RESEARCH_CAMPAIGN_OFFLINE_RESPONSE_BYTES = 2_000_000
RESEARCH_CAMPAIGN_OFFLINE_LIMITATIONS = (
    "supports only synthetic_research and brain_plan campaign stages",
    "synthetic_research measures seeded repository fixtures and does not search external literature",
    "brain_plan validates and orders a plan but never executes its steps",
    "this first slice has no resume or execution-journal reconciliation; an interrupted output directory must be inspected rather than retried",
)

RESEARCH_CAMPAIGN_OFFLINE_EXECUTION_STATES = frozenset(
    {
        "not_started",
        "completed",
        "awaiting_human_review",
        "refused",
        "needs_input",
        "exhausted",
        "reconciliation_required",
    }
)
RESEARCH_CAMPAIGN_OFFLINE_STATUSES = frozenset(
    {
        "planned",
        "completed",
        "awaiting_human_review",
        "refused",
        "needs_input",
        "exhausted",
        "reconciliation_required",
    }
)
RESEARCH_CAMPAIGN_OFFLINE_STAGE_KINDS = frozenset({"synthetic_research", "brain_plan"})
RESEARCH_CAMPAIGN_OFFLINE_DISPOSITIONS = frozenset(
    {
        "succeeded",
        "completed_with_negative_findings",
        "missing_input",
        "unknown_completion",
        "awaiting_human_review",
        "exhausted",
        "refused",
    }
)

_DIGEST = re.compile(r"^[0-9a-f]{64}$")
_DRIVE_PATH = re.compile(r"^[A-Za-z]:")


def _object(name: str, value: Any) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be an object")
    return dict(value)


def _exact_keys(name: str, value: Mapping[str, Any], expected: set[str]) -> None:
    observed = set(value)
    missing = sorted(expected - observed)
    extra = sorted(observed - expected)
    if missing or extra:
        detail: list[str] = []
        if missing:
            detail.append(f"missing {missing!r}")
        if extra:
            detail.append(f"unsupported {extra!r}")
        raise ArgumentError(f"{name} has an invalid field set: {', '.join(detail)}")


def _text(name: str, value: Any, *, maximum: int = 4096) -> str:
    if not isinstance(value, str) or not value or any(mark in value for mark in ("\x00", "\r", "\n")):
        raise ArgumentError(f"{name} must be non-empty, line-safe text")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds its {maximum}-byte bound")
    return value


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or _DIGEST.fullmatch(value) is None:
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _integer(name: str, value: Any, minimum: int, maximum: int | None = None) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        raise ArgumentError(f"{name} must be an integer of at least {minimum}")
    if maximum is not None and value > maximum:
        raise ArgumentError(f"{name} must be an integer no greater than {maximum}")
    return value


def _relative_path(name: str, value: Any, *, maximum: int = 4096) -> str:
    path = _text(name, value, maximum=maximum)
    if "\\" in path or path.startswith("/") or _DRIVE_PATH.match(path):
        raise ArgumentError(f"{name} must be a portable workspace-relative path")
    if any(component in {"", ".", ".."} for component in path.split("/")):
        raise ArgumentError(
            f"{name} must not contain empty, current-directory, or parent-directory segments"
        )
    return path


def _array(name: str, value: Any, minimum: int, maximum: int) -> list[Any]:
    if not isinstance(value, list) or not minimum <= len(value) <= maximum:
        raise ArgumentError(f"{name} must be an array with {minimum}..{maximum} entries")
    return value


def _path_array(
    name: str,
    value: Any,
    minimum: int,
    maximum: int,
    *,
    path_maximum: int = 4096,
) -> tuple[str, ...]:
    return tuple(
        _relative_path(f"{name}[{index}]", item, maximum=path_maximum)
        for index, item in enumerate(_array(name, value, minimum, maximum))
    )


def _text_array(name: str, value: Any, minimum: int, maximum: int) -> tuple[str, ...]:
    return tuple(
        _text(f"{name}[{index}]", item)
        for index, item in enumerate(_array(name, value, minimum, maximum))
    )


def _under_directory(path: str, directory: str) -> bool:
    return path.startswith(f"{directory}/")


def _join_locator(directory: str, locator: str) -> str:
    return f"{directory}/{locator.split('#', 1)[0]}"


@dataclass(frozen=True, slots=True)
class ResearchCampaignOfflineRunRequest:
    """Exact path-only input accepted by ``research_campaign_run_offline``."""

    spec_path: str
    stage_input_paths: Mapping[str, str]
    output_dir: str
    confirm: bool = False

    def __init__(
        self,
        spec_path: str,
        stage_input_paths: Mapping[str, str],
        output_dir: str,
        confirm: bool = False,
    ) -> None:
        spec = _relative_path("research campaign spec_path", spec_path)
        output = _relative_path("research campaign output_dir", output_dir)
        if not isinstance(stage_input_paths, Mapping):
            raise ArgumentError("research campaign stage_input_paths must be an object")
        if not 1 <= len(stage_input_paths) <= MAX_RESEARCH_CAMPAIGN_OFFLINE_STAGES:
            raise ArgumentError("research campaign stage_input_paths must contain 1..8 entries")
        normalized: dict[str, str] = {}
        for raw_stage_id, raw_path in stage_input_paths.items():
            stage_id = _text("research campaign stage_id", raw_stage_id, maximum=256)
            if stage_id.strip() != stage_id:
                raise ArgumentError(
                    "research campaign stage_id must not have surrounding whitespace"
                )
            if stage_id in normalized:
                raise ArgumentError(f"research campaign stage_id {stage_id!r} is duplicated")
            normalized[stage_id] = _relative_path(
                f"research campaign stage_input_paths[{stage_id!r}]", raw_path
            )
        if not isinstance(confirm, bool):
            raise ArgumentError("research campaign confirm must be a boolean")
        object.__setattr__(self, "spec_path", spec)
        object.__setattr__(self, "stage_input_paths", MappingProxyType(normalized))
        object.__setattr__(self, "output_dir", output)
        object.__setattr__(self, "confirm", confirm)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ResearchCampaignOfflineRunRequest":
        raw = _object("research campaign offline request", value)
        allowed = {"spec_path", "stage_input_paths", "output_dir", "confirm"}
        required = {"spec_path", "stage_input_paths", "output_dir"}
        extra = sorted(set(raw) - allowed)
        missing = sorted(required - set(raw))
        if missing or extra:
            detail = []
            if missing:
                detail.append(f"missing {missing!r}")
            if extra:
                detail.append(f"unsupported {extra!r}")
            raise ArgumentError(
                f"research campaign offline request has an invalid field set: {', '.join(detail)}"
            )
        return cls(
            spec_path=raw["spec_path"],
            stage_input_paths=raw["stage_input_paths"],
            output_dir=raw["output_dir"],
            confirm=raw.get("confirm", False),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "spec_path": self.spec_path,
            "stage_input_paths": dict(self.stage_input_paths),
            "output_dir": self.output_dir,
            "confirm": self.confirm,
        }


# ``RunArgs`` matches the established tool-surface naming convention while ``RunRequest`` keeps
# the request terminology used by the autonomous SDK. They intentionally name the same type.
ResearchCampaignOfflineRunArgs = ResearchCampaignOfflineRunRequest


@dataclass(frozen=True, slots=True)
class ResearchCampaignOfflineExecution:
    state: str
    stage_id: str | None
    reason: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ResearchCampaignOfflineExecution":
        raw = _object("research campaign execution", value)
        state = _text("research campaign execution state", raw.get("state"), maximum=64)
        if state not in RESEARCH_CAMPAIGN_OFFLINE_EXECUTION_STATES:
            raise ArgumentError(f"unknown research campaign execution state {state!r}")
        if state in {"not_started", "completed"}:
            _exact_keys("research campaign execution", raw, {"state"})
            return cls(state, None, None)
        if state == "reconciliation_required":
            _exact_keys("research campaign execution", raw, {"state", "reason"})
            return cls(
                state,
                None,
                _text(
                    "research campaign reconciliation reason",
                    raw["reason"],
                    maximum=2048,
                ),
            )
        _exact_keys("research campaign execution", raw, {"state", "stage_id"})
        return cls(
            state,
            _text("research campaign execution stage_id", raw["stage_id"], maximum=256),
            None,
        )


@dataclass(frozen=True, slots=True)
class ResearchCampaignOfflineStage:
    state: str
    stage_id: str
    kind: str
    input_digest: str
    artifact_locator: str
    action_ordinal: int | None = None
    disposition: str | None = None
    artifact_digest: str | None = None
    receipt_digest: str | None = None
    file_sha256: str | None = None
    authorization_digest: str | None = None
    reason: str | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ResearchCampaignOfflineStage":
        raw = _object("research campaign stage", value)
        state = _text("research campaign stage state", raw.get("state"), maximum=32)
        common = {"state", "stage_id", "kind", "input_digest", "artifact_locator"}
        if state == "not_started":
            _exact_keys("research campaign not-started stage", raw, common)
        elif state == "settled":
            _exact_keys(
                "research campaign settled stage",
                raw,
                common
                | {
                    "action_ordinal",
                    "disposition",
                    "artifact_digest",
                    "receipt_digest",
                    "file_sha256",
                },
            )
        elif state == "reconciliation_required":
            _exact_keys(
                "research campaign reconciliation-required stage",
                raw,
                common | {"action_ordinal", "authorization_digest", "reason"},
            )
        else:
            raise ArgumentError(f"unknown research campaign stage state {state!r}")
        stage_id = _text("research campaign stage stage_id", raw["stage_id"], maximum=256)
        kind = _text("research campaign stage kind", raw["kind"], maximum=64)
        if kind not in RESEARCH_CAMPAIGN_OFFLINE_STAGE_KINDS:
            raise ArgumentError(f"unknown research campaign stage kind {kind!r}")
        input_digest = _digest("research campaign stage input_digest", raw["input_digest"])
        locator = _relative_path("research campaign stage artifact_locator", raw["artifact_locator"])
        if state == "not_started":
            return cls(state, stage_id, kind, input_digest, locator)
        if state == "reconciliation_required":
            return cls(
                state=state,
                stage_id=stage_id,
                kind=kind,
                input_digest=input_digest,
                artifact_locator=locator,
                action_ordinal=_integer(
                    "research campaign stage action_ordinal",
                    raw["action_ordinal"],
                    1,
                    MAX_RESEARCH_CAMPAIGN_OFFLINE_STAGES,
                ),
                authorization_digest=_digest(
                    "research campaign stage authorization_digest",
                    raw["authorization_digest"],
                ),
                reason=_text(
                    "research campaign stage reconciliation reason",
                    raw["reason"],
                    maximum=2048,
                ),
            )
        disposition = _text(
            "research campaign stage disposition", raw["disposition"], maximum=64
        )
        if disposition not in RESEARCH_CAMPAIGN_OFFLINE_DISPOSITIONS:
            raise ArgumentError(f"unknown research campaign stage disposition {disposition!r}")
        return cls(
            state=state,
            stage_id=stage_id,
            kind=kind,
            input_digest=input_digest,
            artifact_locator=locator,
            action_ordinal=_integer(
                "research campaign stage action_ordinal",
                raw["action_ordinal"],
                1,
                MAX_RESEARCH_CAMPAIGN_OFFLINE_STAGES,
            ),
            disposition=disposition,
            artifact_digest=_digest(
                "research campaign stage artifact_digest", raw["artifact_digest"]
            ),
            receipt_digest=_digest(
                "research campaign stage receipt_digest", raw["receipt_digest"]
            ),
            file_sha256=_digest("research campaign stage file_sha256", raw["file_sha256"]),
        )

    @property
    def settled(self) -> bool:
        return self.state == "settled"

    @property
    def reconciliation_required(self) -> bool:
        return self.state == "reconciliation_required"


@dataclass(frozen=True, slots=True)
class ResearchCampaignCheckpointMetadata:
    locator: str
    schema: str
    generation: int
    snapshot_digest: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ResearchCampaignCheckpointMetadata":
        raw = _object("research campaign checkpoint metadata", value)
        _exact_keys(
            "research campaign checkpoint metadata",
            raw,
            {"locator", "schema", "generation", "snapshot_digest"},
        )
        if raw["schema"] != RESEARCH_CAMPAIGN_CHECKPOINT_SCHEMA:
            raise ArgumentError("research campaign checkpoint schema is invalid")
        return cls(
            locator=_relative_path("research campaign checkpoint locator", raw["locator"]),
            schema=RESEARCH_CAMPAIGN_CHECKPOINT_SCHEMA,
            generation=_integer("research campaign checkpoint generation", raw["generation"], 1),
            snapshot_digest=_digest(
                "research campaign checkpoint snapshot_digest", raw["snapshot_digest"]
            ),
        )


@dataclass(frozen=True, slots=True)
class ResearchCampaignTrustedHeadMetadata:
    locator: str
    campaign_id: str
    spec_digest: str
    generation: int
    snapshot_digest: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ResearchCampaignTrustedHeadMetadata":
        raw = _object("research campaign trusted-head metadata", value)
        _exact_keys(
            "research campaign trusted-head metadata",
            raw,
            {"locator", "campaign_id", "spec_digest", "generation", "snapshot_digest"},
        )
        return cls(
            locator=_relative_path("research campaign trusted-head locator", raw["locator"]),
            campaign_id=_text(
                "research campaign trusted-head campaign_id", raw["campaign_id"], maximum=128
            ),
            spec_digest=_digest(
                "research campaign trusted-head spec_digest", raw["spec_digest"]
            ),
            generation=_integer("research campaign trusted-head generation", raw["generation"], 1),
            snapshot_digest=_digest(
                "research campaign trusted-head snapshot_digest", raw["snapshot_digest"]
            ),
        )


@dataclass(frozen=True, slots=True)
class ResearchCampaignManifestMetadata:
    locator: str
    digest: str
    file_sha256: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ResearchCampaignManifestMetadata":
        raw = _object("research campaign manifest metadata", value)
        _exact_keys(
            "research campaign manifest metadata", raw, {"locator", "digest", "file_sha256"}
        )
        digest = _digest("research campaign manifest digest", raw["digest"])
        file_sha256 = _digest(
            "research campaign manifest file_sha256", raw["file_sha256"]
        )
        if digest != file_sha256:
            raise ArgumentError(
                "research campaign manifest digest does not match its file SHA-256"
            )
        return cls(
            locator=_relative_path("research campaign manifest locator", raw["locator"]),
            digest=digest,
            file_sha256=file_sha256,
        )


@dataclass(frozen=True, slots=True)
class ResearchCampaignOfflineRunResult:
    schema: str
    workflow: str
    execution: ResearchCampaignOfflineExecution
    campaign_id: str
    spec_digest: str
    campaign_status: str
    actions_used: int
    stages: tuple[ResearchCampaignOfflineStage, ...]
    checkpoint: ResearchCampaignCheckpointMetadata | None
    trusted_head: ResearchCampaignTrustedHeadMetadata | None
    manifest: ResearchCampaignManifestMetadata | None
    written: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ResearchCampaignOfflineRunResult":
        """Parse a peer result and reject malformed positive responses as protocol errors."""

        try:
            return cls._from_wire(value)
        except (ArgumentError, KeyError, TypeError, ValueError) as error:
            raise ProtocolError(f"invalid research campaign offline result: {error}") from error

    @classmethod
    def _from_wire(cls, value: Mapping[str, Any]) -> "ResearchCampaignOfflineRunResult":
        raw = _result_payload(value)
        _exact_keys(
            "research campaign offline result",
            raw,
            {
                "schema",
                "workflow",
                "execution",
                "campaign_id",
                "spec_digest",
                "campaign_status",
                "actions_used",
                "stages",
                "checkpoint",
                "trusted_head",
                "manifest",
                "written",
                "limitations",
            },
        )
        if raw["schema"] != RESEARCH_CAMPAIGN_OFFLINE_RESULT_SCHEMA:
            raise ArgumentError("research campaign offline result schema is invalid")
        if raw["workflow"] != RESEARCH_CAMPAIGN_OFFLINE_TOOL:
            raise ArgumentError("research campaign offline result workflow is invalid")
        execution = ResearchCampaignOfflineExecution.from_wire(raw["execution"])
        campaign_id = _text("research campaign campaign_id", raw["campaign_id"], maximum=1024)
        spec_digest = _digest("research campaign spec_digest", raw["spec_digest"])
        campaign_status = _text(
            "research campaign campaign_status", raw["campaign_status"], maximum=64
        )
        if campaign_status not in RESEARCH_CAMPAIGN_OFFLINE_STATUSES:
            raise ArgumentError(f"unknown research campaign status {campaign_status!r}")
        actions_used = _integer(
            "research campaign actions_used",
            raw["actions_used"],
            0,
            MAX_RESEARCH_CAMPAIGN_OFFLINE_STAGES,
        )
        stages = tuple(
            ResearchCampaignOfflineStage.from_wire(item)
            for item in _array(
                "research campaign stages", raw["stages"], 1, MAX_RESEARCH_CAMPAIGN_OFFLINE_STAGES
            )
        )
        if len({stage.stage_id for stage in stages}) != len(stages):
            raise ArgumentError("research campaign stages contain duplicate stage_id values")
        checkpoint = (
            None
            if raw["checkpoint"] is None
            else ResearchCampaignCheckpointMetadata.from_wire(raw["checkpoint"])
        )
        trusted_head = (
            None
            if raw["trusted_head"] is None
            else ResearchCampaignTrustedHeadMetadata.from_wire(raw["trusted_head"])
        )
        manifest = (
            None
            if raw["manifest"] is None
            else ResearchCampaignManifestMetadata.from_wire(raw["manifest"])
        )
        written = _path_array(
            "research campaign written",
            raw["written"],
            0,
            MAX_RESEARCH_CAMPAIGN_OFFLINE_WRITTEN_PATHS,
            path_maximum=8192,
        )
        limitations = _text_array(
            "research campaign limitations",
            raw["limitations"],
            1,
            MAX_RESEARCH_CAMPAIGN_OFFLINE_LIMITATIONS,
        )
        if any(len(value.encode("utf-8")) > 2048 for value in limitations):
            raise ArgumentError("research campaign limitation exceeds its 2048-byte bound")
        if limitations != RESEARCH_CAMPAIGN_OFFLINE_LIMITATIONS:
            raise ArgumentError("research campaign limitations do not match the fixed offline boundary")
        result = cls(
            schema=RESEARCH_CAMPAIGN_OFFLINE_RESULT_SCHEMA,
            workflow=RESEARCH_CAMPAIGN_OFFLINE_TOOL,
            execution=execution,
            campaign_id=campaign_id,
            spec_digest=spec_digest,
            campaign_status=campaign_status,
            actions_used=actions_used,
            stages=stages,
            checkpoint=checkpoint,
            trusted_head=trusted_head,
            manifest=manifest,
            written=written,
            limitations=limitations,
        )
        result._validate_consistency()
        return result

    def _validate_consistency(self) -> None:
        expected_status = "planned" if self.execution.state == "not_started" else self.execution.state
        if self.campaign_status != expected_status:
            raise ArgumentError("research campaign execution state does not match campaign_status")
        for index, stage in enumerate(self.stages, start=1):
            suffix = (
                "research-dossier"
                if stage.kind == "synthetic_research"
                else "brain-plan-report"
            )
            if stage.artifact_locator != f"artifacts/{index:04d}-{suffix}.json":
                raise ArgumentError(
                    "research campaign stage artifact locator is not canonical"
                )
            if stage.action_ordinal is not None and stage.action_ordinal != index:
                raise ArgumentError(
                    "research campaign stage action ordinal does not match campaign order"
                )
        settled = tuple(stage for stage in self.stages if stage.settled)
        reconciliation = tuple(
            stage for stage in self.stages if stage.reconciliation_required
        )
        if len(reconciliation) > 1:
            raise ArgumentError(
                "research campaign contains more than one reconciliation-required stage"
            )
        ordinals = sorted(stage.action_ordinal for stage in settled if stage.action_ordinal is not None)
        if self.execution.state == "not_started":
            if self.actions_used != 0 or any(stage.state != "not_started" for stage in self.stages):
                raise ArgumentError("research campaign preview cannot contain authorized actions")
            if any(value is not None for value in (self.checkpoint, self.trusted_head, self.manifest)):
                raise ArgumentError("research campaign preview cannot contain persisted metadata")
            if self.written:
                raise ArgumentError("research campaign preview cannot report written paths")
            return
        if len(set(self.written)) != len(self.written):
            raise ArgumentError("research campaign written paths contain duplicates")

        durable_pair = self.checkpoint is not None and self.trusted_head is not None
        if (self.checkpoint is None) != (self.trusted_head is None):
            raise ArgumentError(
                "research campaign checkpoint and trusted head must be present together"
            )
        if durable_pair:
            assert self.checkpoint is not None and self.trusted_head is not None
            if (
                self.checkpoint.generation != self.trusted_head.generation
                or self.checkpoint.snapshot_digest != self.trusted_head.snapshot_digest
            ):
                raise ArgumentError("research campaign checkpoint does not match its trusted head")
            if (
                self.trusted_head.campaign_id != self.campaign_id
                or self.trusted_head.spec_digest != self.spec_digest
            ):
                raise ArgumentError("research campaign trusted head does not match the campaign")

        if self.execution.state == "reconciliation_required":
            if reconciliation:
                active = reconciliation[0]
                if self.actions_used != len(settled) + 1:
                    raise ArgumentError(
                        "research campaign reconciliation action count is inconsistent"
                    )
                if ordinals != list(range(1, self.actions_used)):
                    raise ArgumentError(
                        "research campaign settled action ordinals are not contiguous"
                    )
                if active.action_ordinal != self.actions_used:
                    raise ArgumentError(
                        "research campaign reconciliation stage is not the active action"
                    )
                active_index = self.stages.index(active)
                if any(not stage.settled for stage in self.stages[:active_index]) or any(
                    stage.state != "not_started" for stage in self.stages[active_index + 1 :]
                ):
                    raise ArgumentError(
                        "research campaign reconciliation stage has invalid surrounding progress"
                    )
                if any(
                    stage.disposition
                    not in {"succeeded", "completed_with_negative_findings"}
                    for stage in self.stages[:active_index]
                ):
                    raise ArgumentError(
                        "research campaign reconciliation follows a non-continuing receipt"
                    )
                if not durable_pair or not self.written:
                    raise ArgumentError(
                        "authorized reconciliation requires checkpoint/head metadata and writes"
                    )
                assert self.checkpoint is not None and self.trusted_head is not None
                if self.manifest is None:
                    checkpoint_file, checkpoint_marker, checkpoint_fragment = (
                        self.checkpoint.locator.partition("#")
                    )
                    head_file, head_marker, head_fragment = self.trusted_head.locator.partition("#")
                    if (
                        checkpoint_marker != "#"
                        or checkpoint_fragment != "/checkpoint"
                        or head_marker != "#"
                        or head_fragment != "/candidate_checkpoint_head"
                        or checkpoint_file != head_file
                        or checkpoint_file
                        != f"authority/{self.checkpoint.generation:04d}-authorization.json"
                        or self.checkpoint.generation != self.actions_used
                    ):
                        raise ArgumentError(
                            "research campaign partial reconciliation metadata is not bound to one authorization envelope"
                        )
                elif (
                    self.checkpoint.locator != "campaign.checkpoint.json"
                    or self.trusted_head.locator != "campaign.head.json"
                    or self.manifest.locator != "campaign.manifest.json"
                    or self.checkpoint.generation != self.actions_used + 1
                ):
                    raise ArgumentError(
                        "research campaign committed reconciliation metadata is not canonical"
                    )
            else:
                if (
                    self.actions_used != 0
                    or settled
                    or any(stage.state != "not_started" for stage in self.stages)
                    or any(
                        value is not None
                        for value in (self.checkpoint, self.trusted_head, self.manifest)
                    )
                    or self.written
                ):
                    raise ArgumentError(
                        "unestablished reconciliation cannot claim durable or executed work"
                    )
            return

        if reconciliation:
            raise ArgumentError(
                "research campaign reconciliation stage requires reconciliation execution"
            )
        if self.actions_used != len(settled):
            raise ArgumentError("research campaign actions_used does not match settled stage count")
        if ordinals != list(range(1, self.actions_used + 1)):
            raise ArgumentError("research campaign action ordinals are not contiguous")
        if not durable_pair or self.manifest is None or not self.written:
            raise ArgumentError(
                "confirmed terminal campaign requires checkpoint, head, manifest, and writes"
            )
        assert self.checkpoint is not None and self.trusted_head is not None
        if (
            self.checkpoint.locator != "campaign.checkpoint.json"
            or self.trusted_head.locator != "campaign.head.json"
            or self.manifest.locator != "campaign.manifest.json"
            or self.checkpoint.generation != self.actions_used + 1
        ):
            raise ArgumentError("research campaign terminal metadata locators are not canonical")
        if self.execution.stage_id is not None:
            target = next(
                (stage for stage in self.stages if stage.stage_id == self.execution.stage_id), None
            )
            if target is None or not target.settled:
                raise ArgumentError("research campaign execution stage is not a settled stage")
            expected_disposition = {
                "awaiting_human_review": "awaiting_human_review",
                "refused": "refused",
                "needs_input": "missing_input",
                "exhausted": "exhausted",
            }[self.execution.state]
            if target.disposition != expected_disposition:
                raise ArgumentError("research campaign execution state does not match stage disposition")
            if target.action_ordinal != self.actions_used:
                raise ArgumentError("research campaign execution stage is not the latest action")
            target_index = self.stages.index(target)
            if any(not stage.settled for stage in self.stages[:target_index]) or any(
                stage.state != "not_started" for stage in self.stages[target_index + 1 :]
            ):
                raise ArgumentError(
                    "research campaign terminal stage has invalid surrounding progress"
                )
            if any(
                stage.disposition not in {"succeeded", "completed_with_negative_findings"}
                for stage in self.stages[:target_index]
            ):
                raise ArgumentError(
                    "research campaign terminal stage follows a non-continuing receipt"
                )
        if self.execution.state == "completed" and any(
            not stage.settled
            or stage.disposition not in {"succeeded", "completed_with_negative_findings"}
            for stage in self.stages
        ):
            raise ArgumentError("completed research campaign contains an incomplete stage")

    def validate_request(self, request: ResearchCampaignOfflineRunRequest) -> None:
        if not isinstance(request, ResearchCampaignOfflineRunRequest):
            raise ArgumentError("research campaign request binding requires a typed request")
        if {stage.stage_id for stage in self.stages} != set(request.stage_input_paths):
            raise ProtocolError("research campaign result stages do not match requested stage ids")
        if request.confirm:
            if self.execution.state == "not_started":
                raise ProtocolError(
                    "research campaign execution request returned only a preview"
                )
            if any(not _under_directory(path, request.output_dir) for path in self.written):
                raise ProtocolError(
                    "research campaign result reported a write outside the requested output_dir"
                )
            internal_written = {
                path[len(request.output_dir) + 1 :] for path in self.written
            }
            expected_writes = {
                _join_locator(request.output_dir, stage.artifact_locator)
                for stage in self.stages
                if stage.settled
            }
            for metadata in (self.checkpoint, self.trusted_head, self.manifest):
                if metadata is not None:
                    expected_writes.add(_join_locator(request.output_dir, metadata.locator))
            if not expected_writes.issubset(set(self.written)):
                raise ProtocolError(
                    "research campaign written paths omit persisted result metadata"
                )
            if self.execution.state == "reconciliation_required" and self.manifest is None:
                expected_internal = {
                    f"authority/{ordinal:04d}-authorization.json"
                    for ordinal in range(1, self.actions_used + 1)
                }
                expected_internal.update(
                    stage.artifact_locator for stage in self.stages if stage.settled
                )
                if internal_written != expected_internal:
                    raise ProtocolError(
                        "research campaign partial reconciliation writes are not canonical"
                    )
            elif self.manifest is not None:
                assert self.checkpoint is not None
                expected_internal = {
                    f"authority/{ordinal:04d}-authorization.json"
                    for ordinal in range(1, self.actions_used + 1)
                }
                expected_internal.update(
                    stage.artifact_locator for stage in self.stages if stage.settled
                )
                expected_internal.update(
                    {
                        f"authority/{self.checkpoint.generation:04d}-terminal.json",
                        "campaign.checkpoint.json",
                        "campaign.head.json",
                        "campaign.manifest.json",
                    }
                )
                if internal_written != expected_internal:
                    raise ProtocolError(
                        "research campaign terminal writes are not canonical"
                    )
        elif self.execution.state != "not_started":
            raise ProtocolError("unconfirmed research campaign request returned an executed result")

    @property
    def preview(self) -> bool:
        return self.execution.state == "not_started"

    @property
    def completed(self) -> bool:
        return self.execution.state == "completed"

    def to_dict(self) -> dict[str, Any]:
        def stage_value(stage: ResearchCampaignOfflineStage) -> dict[str, Any]:
            value: dict[str, Any] = {
                "state": stage.state,
                "stage_id": stage.stage_id,
                "kind": stage.kind,
                "input_digest": stage.input_digest,
                "artifact_locator": stage.artifact_locator,
            }
            if stage.settled:
                value.update(
                    {
                        "action_ordinal": stage.action_ordinal,
                        "disposition": stage.disposition,
                        "artifact_digest": stage.artifact_digest,
                        "receipt_digest": stage.receipt_digest,
                        "file_sha256": stage.file_sha256,
                    }
                )
            elif stage.reconciliation_required:
                value.update(
                    {
                        "action_ordinal": stage.action_ordinal,
                        "authorization_digest": stage.authorization_digest,
                        "reason": stage.reason,
                    }
                )
            return value

        execution: dict[str, Any] = {"state": self.execution.state}
        if self.execution.stage_id is not None:
            execution["stage_id"] = self.execution.stage_id
        if self.execution.reason is not None:
            execution["reason"] = self.execution.reason
        checkpoint = None if self.checkpoint is None else {
            "locator": self.checkpoint.locator,
            "schema": self.checkpoint.schema,
            "generation": self.checkpoint.generation,
            "snapshot_digest": self.checkpoint.snapshot_digest,
        }
        trusted_head = None if self.trusted_head is None else {
            "locator": self.trusted_head.locator,
            "campaign_id": self.trusted_head.campaign_id,
            "spec_digest": self.trusted_head.spec_digest,
            "generation": self.trusted_head.generation,
            "snapshot_digest": self.trusted_head.snapshot_digest,
        }
        manifest = None if self.manifest is None else {
            "locator": self.manifest.locator,
            "digest": self.manifest.digest,
            "file_sha256": self.manifest.file_sha256,
        }
        return {
            "schema": self.schema,
            "workflow": self.workflow,
            "execution": execution,
            "campaign_id": self.campaign_id,
            "spec_digest": self.spec_digest,
            "campaign_status": self.campaign_status,
            "actions_used": self.actions_used,
            "stages": [stage_value(stage) for stage in self.stages],
            "checkpoint": checkpoint,
            "trusted_head": trusted_head,
            "manifest": manifest,
            "written": list(self.written),
            "limitations": list(self.limitations),
        }


def _result_payload(value: Mapping[str, Any]) -> dict[str, Any]:
    root = _object("research campaign response", value)
    try:
        root_json = json.dumps(
            root,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
    except (TypeError, ValueError) as error:
        raise ProtocolError("research campaign response is not JSON-safe") from error
    if len(root_json.encode("utf-8")) > MAX_RESEARCH_CAMPAIGN_OFFLINE_RESPONSE_BYTES:
        raise ProtocolError("research campaign response exceeds its response bound")

    def decode_tool_result(value: Any) -> dict[str, Any]:
        container = _object("research campaign MCP result", value)
        expected = {"content", "isError"}
        if "structuredContent" in container:
            expected.add("structuredContent")
        _exact_keys("research campaign MCP result", container, expected)
        if not isinstance(container["isError"], bool):
            raise ProtocolError("research campaign MCP result has a non-boolean isError field")
        content = container["content"]
        if not isinstance(content, list) or len(content) != 1:
            raise ProtocolError("research campaign MCP content must contain exactly one text block")
        block = _object("research campaign MCP content block", content[0])
        _exact_keys("research campaign MCP content block", block, {"type", "text"})
        if block["type"] != "text" or not isinstance(block["text"], str):
            raise ProtocolError("research campaign MCP content block must contain JSON text")
        if len(block["text"].encode("utf-8")) > MAX_RESEARCH_CAMPAIGN_OFFLINE_RESPONSE_BYTES:
            raise ProtocolError("research campaign MCP text projection exceeds its response bound")
        try:
            decoded = json.loads(block["text"])
        except json.JSONDecodeError as error:
            raise ProtocolError("research campaign MCP text projection is not JSON") from error
        if not isinstance(decoded, Mapping):
            raise ProtocolError("research campaign MCP text projection must be an object")
        payload = dict(decoded)
        structured = container.get("structuredContent")
        if container["isError"]:
            if structured is not None:
                raise ProtocolError(
                    "research campaign MCP refusal cannot contain structured success content"
                )
            _exact_keys("research campaign MCP refusal", payload, {"ok", "error"})
            if payload["ok"] is not False:
                raise ProtocolError("research campaign MCP refusal has an invalid ok field")
            _text("research campaign MCP refusal error", payload["error"], maximum=16_384)
            raise ToolRefusal(RESEARCH_CAMPAIGN_OFFLINE_TOOL, payload)
        if structured is not None:
            if not isinstance(structured, Mapping) or dict(structured) != payload:
                raise ProtocolError(
                    "research campaign MCP text and structured results disagree"
                )
        return payload

    if (
        root.get("schema") == RESEARCH_CAMPAIGN_OFFLINE_RESULT_SCHEMA
        and root.get("workflow") == RESEARCH_CAMPAIGN_OFFLINE_TOOL
    ):
        return root

    if "ok" in root:
        _exact_keys(
            "research campaign HTTP envelope",
            root,
            {"ok", "tool", "request_id", "mcp", "guarantee"},
        )
        if not isinstance(root["ok"], bool):
            raise ProtocolError("research campaign HTTP envelope has a non-boolean ok field")
        if root["tool"] != RESEARCH_CAMPAIGN_OFFLINE_TOOL:
            raise ProtocolError("research campaign HTTP envelope names a different tool")
        request_id = _text("research campaign HTTP request_id", root["request_id"], maximum=1024)
        if root["guarantee"] != "REST and MCP calls share the same in-process tool dispatcher":
            raise ProtocolError("research campaign HTTP envelope changed its dispatcher guarantee")
        if root["ok"] is False:
            raise ToolRefusal(
                RESEARCH_CAMPAIGN_OFFLINE_TOOL,
                {"ok": False, "error": "HTTP tool dispatch was refused"},
            )
        mcp = _object("research campaign JSON-RPC envelope", root["mcp"])
        _exact_keys("research campaign JSON-RPC envelope", mcp, {"jsonrpc", "id", "result"})
        if mcp["jsonrpc"] != "2.0" or mcp["id"] != request_id:
            raise ProtocolError("research campaign JSON-RPC identity does not match HTTP")
        return decode_tool_result(mcp["result"])

    if "jsonrpc" in root:
        _exact_keys("research campaign JSON-RPC envelope", root, {"jsonrpc", "id", "result"})
        if root["jsonrpc"] != "2.0":
            raise ProtocolError("research campaign JSON-RPC version is invalid")
        _text("research campaign JSON-RPC id", root["id"], maximum=1024)
        return decode_tool_result(root["result"])

    return decode_tool_result(root)


def research_campaign_offline_result(
    value: Mapping[str, Any],
) -> ResearchCampaignOfflineRunResult:
    """Parse a direct result, MCP content envelope, or HTTP tool envelope."""

    return ResearchCampaignOfflineRunResult.from_wire(value)


def _request(
    value: ResearchCampaignOfflineRunRequest | Mapping[str, Any],
) -> ResearchCampaignOfflineRunRequest:
    return (
        value
        if isinstance(value, ResearchCampaignOfflineRunRequest)
        else ResearchCampaignOfflineRunRequest.from_wire(value)
    )


class ResearchCampaignClient:
    """Synchronous facade over an existing HTTP or MCP ``call_tool`` transport."""

    def __init__(self, call_tool: Callable[[str, Mapping[str, Any]], Mapping[str, Any]]) -> None:
        if not callable(call_tool):
            raise ArgumentError("research campaign call_tool transport must be callable")
        self._call_tool = call_tool

    @classmethod
    def from_http(cls, client: Any) -> "ResearchCampaignClient":
        if not hasattr(client, "call_tool") or not callable(client.call_tool):
            raise ArgumentError("HTTP client must expose call_tool")
        return cls(client.call_tool)

    @classmethod
    def from_mcp(cls, client: Any) -> "ResearchCampaignClient":
        if not hasattr(client, "call_tool") or not callable(client.call_tool):
            raise ArgumentError("MCP client must expose call_tool")

        def invoke(name: str, arguments: Mapping[str, Any]) -> Mapping[str, Any]:
            result = client.call_tool(name, arguments)
            if not hasattr(result, "require_ok") or not callable(result.require_ok):
                raise ArgumentError("MCP transport returned an unrecognised tool result")
            payload = result.require_ok()
            if not isinstance(payload, Mapping):
                raise ArgumentError("MCP transport returned a non-object tool result")
            return payload

        return cls(invoke)

    def run_offline(
        self, request: ResearchCampaignOfflineRunRequest | Mapping[str, Any]
    ) -> ResearchCampaignOfflineRunResult:
        normalized = _request(request)
        payload = self._call_tool(RESEARCH_CAMPAIGN_OFFLINE_TOOL, normalized.to_mcp_arguments())
        if not isinstance(payload, Mapping):
            raise ArgumentError("research campaign transport returned a non-object response")
        result = ResearchCampaignOfflineRunResult.from_wire(payload)
        result.validate_request(normalized)
        return result


class AsyncResearchCampaignClient:
    """Async facade over an existing HTTP or MCP ``call_tool`` transport."""

    def __init__(
        self,
        call_tool: Callable[[str, Mapping[str, Any]], Awaitable[Mapping[str, Any]]],
    ) -> None:
        if not callable(call_tool):
            raise ArgumentError("async research campaign call_tool transport must be callable")
        self._call_tool = call_tool

    @classmethod
    def from_http(cls, client: Any) -> "AsyncResearchCampaignClient":
        if not hasattr(client, "call_tool") or not callable(client.call_tool):
            raise ArgumentError("async HTTP client must expose call_tool")
        return cls(client.call_tool)

    @classmethod
    def from_mcp(cls, client: Any) -> "AsyncResearchCampaignClient":
        if not hasattr(client, "call_tool") or not callable(client.call_tool):
            raise ArgumentError("async MCP client must expose call_tool")

        async def invoke(name: str, arguments: Mapping[str, Any]) -> Mapping[str, Any]:
            result = await client.call_tool(name, arguments)
            if not hasattr(result, "require_ok") or not callable(result.require_ok):
                raise ArgumentError("async MCP transport returned an unrecognised tool result")
            payload = result.require_ok()
            if not isinstance(payload, Mapping):
                raise ArgumentError("async MCP transport returned a non-object tool result")
            return payload

        return cls(invoke)

    async def run_offline(
        self, request: ResearchCampaignOfflineRunRequest | Mapping[str, Any]
    ) -> ResearchCampaignOfflineRunResult:
        normalized = _request(request)
        payload = await self._call_tool(
            RESEARCH_CAMPAIGN_OFFLINE_TOOL, normalized.to_mcp_arguments()
        )
        if not isinstance(payload, Mapping):
            raise ArgumentError("async research campaign transport returned a non-object response")
        result = ResearchCampaignOfflineRunResult.from_wire(payload)
        result.validate_request(normalized)
        return result


__all__ = [
    "RESEARCH_CAMPAIGN_OFFLINE_TOOL",
    "RESEARCH_CAMPAIGN_OFFLINE_RESULT_SCHEMA",
    "RESEARCH_CAMPAIGN_CHECKPOINT_SCHEMA",
    "MAX_RESEARCH_CAMPAIGN_OFFLINE_STAGES",
    "MAX_RESEARCH_CAMPAIGN_OFFLINE_WRITTEN_PATHS",
    "MAX_RESEARCH_CAMPAIGN_OFFLINE_LIMITATIONS",
    "MAX_RESEARCH_CAMPAIGN_OFFLINE_RESPONSE_BYTES",
    "RESEARCH_CAMPAIGN_OFFLINE_LIMITATIONS",
    "RESEARCH_CAMPAIGN_OFFLINE_EXECUTION_STATES",
    "RESEARCH_CAMPAIGN_OFFLINE_STATUSES",
    "RESEARCH_CAMPAIGN_OFFLINE_STAGE_KINDS",
    "RESEARCH_CAMPAIGN_OFFLINE_DISPOSITIONS",
    "ResearchCampaignOfflineRunRequest",
    "ResearchCampaignOfflineRunArgs",
    "ResearchCampaignOfflineExecution",
    "ResearchCampaignOfflineStage",
    "ResearchCampaignCheckpointMetadata",
    "ResearchCampaignTrustedHeadMetadata",
    "ResearchCampaignManifestMetadata",
    "ResearchCampaignOfflineRunResult",
    "ResearchCampaignClient",
    "AsyncResearchCampaignClient",
    "research_campaign_offline_result",
]
