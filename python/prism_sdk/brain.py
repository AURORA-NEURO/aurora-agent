"""High-level autonomous decision loop over the Rust brain and :mod:`llm_runtime`.

This facade is intentionally bounded but real: it selects a model, assembles a bounded prompt,
validates a plan, requires explicit approval for the provider effect, and invokes the model with a
caller-owned credential handle. A structured model decision can then be proposed to the existing
mission executor for server-side preflight and a separate caller approval; the model never grants
itself tools, side effects, or credentials.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
import hashlib
import json
import math
import os
from pathlib import Path
import re
import threading
import uuid
from typing import TYPE_CHECKING, Any, Callable, Mapping, Protocol, Sequence

from .llm_runtime import (
    CredentialError,
    CredentialHandle,
    CompositeProviderInvocationObserver,
    LLMRuntime,
    ProviderContentPart,
    ProviderRequest,
    ProviderResponse,
    ProviderError,
    ProviderInvocationObserver,
    ProviderTool,
    ProviderToolCall,
    ProviderToolLoopResult,
    ProviderToolResult,
    normalize_provider_content_parts,
)
from .errors import ArgumentError
from .mission import MissionPolicy, MissionRequest
from .memory import BrainEpisodicMemory, BrainMemoryError, MemoryQuery, task_facet_digests
from .tooling import ToolCatalogue, ToolSchemaError
from .autonomy_persistence import AutonomousExecutionController
from .autonomy_provider import AutonomousProviderInvocationSession
from .autonomous_selection_lab import (
    normalize_autonomous_model_observations,
    normalize_autonomous_selection_weights,
)

if TYPE_CHECKING:
    from .jobs import BrainJobStore


class BrainRunError(RuntimeError):
    """The bounded autonomous loop could not reach a provider invocation."""


DEFAULT_MISSION_RESPONSE_SCHEMA: dict[str, Any] = {
    "type": "object",
    "required": ["mission"],
    "properties": {
        "mission": {
            "type": "object",
            "required": ["steps"],
            "properties": {
                "steps": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 128,
                    "items": {
                        "type": "object",
                        "required": [
                            "id",
                            "domain",
                            "capability",
                            "objective",
                            "tool",
                            "arguments",
                        ],
                        "properties": {
                            "id": {"type": "string", "minLength": 1, "maxLength": 256},
                            "domain": {"type": "string", "minLength": 1, "maxLength": 256},
                            "capability": {"type": "string", "minLength": 1, "maxLength": 256},
                            "objective": {"type": "string", "minLength": 1, "maxLength": 4096},
                            "tool": {"type": "string", "minLength": 1, "maxLength": 256},
                            "arguments": {"type": "object"},
                            "depends_on": {"type": "array", "items": {"type": "string"}, "maxItems": 128},
                            "required": {"type": "boolean"},
                            "bindings": {"type": "array", "maxItems": 128},
                        },
                    },
                }
            },
        }
    },
    "additionalProperties": False,
}

MAX_ROUTE_REQUEST_BYTES = 2_000_000
MAX_ROUTE_PROMPT_BYTES = 750_000
MAX_ROUTE_PROMPT_SCHEMAS = 128
MAX_MISSION_AUTHORIZATION_CALLS = 128
MAX_MISSION_AUTHORIZATION_RESULT_BYTES = 750_000
MAX_MISSION_AUTHORIZATION_STEP_OUTPUT_BYTES = 350_000
MAX_ADAPTIVE_ROUTE_LABEL_BYTES = 256
MAX_BRAIN_EVALUATOR_ID_BYTES = 128
MAX_BRAIN_EVALUATOR_EVIDENCE_BYTES = 350_000
MAX_BRAIN_EVALUATOR_INPUT_BYTES = 500_000
MAX_BRAIN_REPLAY_BYTES = 16_000
MAX_BRAIN_REPLAN_INSTRUCTION_BYTES = 4_096
MAX_BRAIN_LEARNING_EPISODE_BYTES = 64_000
MAX_BRAIN_LEARNING_TRAJECTORY_STEPS = 32
MAX_BRAIN_LEARNING_TRAJECTORY_BYTES = 256_000
MAX_BRAIN_CREDITED_OUTCOMES = 4096
MAX_BRAIN_LEARNING_SNAPSHOT_BYTES = 32_000_000
MAX_MODEL_SELECTION_AUDIT_RANKING = 64
MAX_MODEL_SELECTION_AUDIT_INPUT_RANKING = 512
MAX_MODEL_SELECTION_AUDIT_REASON_BYTES = 512
MODEL_SELECTION_AUDIT_SCHEMA = "bioprism-brain-selection-audit/0.1"
MODEL_CONTINUATION_SCHEMA = "bioprism-autonomous-model-continuation/0.1"
MODEL_CONTINUATION_STATE_SCHEMA = "bioprism-autonomous-model-continuation-state/0.1"
MAX_MODEL_CONTINUATION_FAILOVERS = 8
MAX_MODEL_CONTINUATION_STEPS = MAX_MODEL_CONTINUATION_FAILOVERS + 1
BRAIN_EVALUATOR_REPLAY_SCHEMA = "bioprism-brain-evaluator-replay/0.1"
BRAIN_EVALUATOR_MESH_SCHEMA = "bioprism-python-autonomous-evaluator-mesh/0.1"
AUTONOMOUS_EVALUATOR_MESH_SCHEMA = BRAIN_EVALUATOR_MESH_SCHEMA
BRAIN_LEARNING_EPISODE_SCHEMA = "bioprism-brain-learning-episode/0.1"
BRAIN_LEARNING_TRAJECTORY_SCHEMA = "bioprism-brain-learning-trajectory/0.1"
BRAIN_CONTEXT_LEARNING_STATE_SCHEMA = "bioprism-brain-context-learning-state/0.1"
_LEGACY_BRAIN_LEARNING_SNAPSHOT_SCHEMA = "bioprism-brain-learning-snapshot/0.1"
BRAIN_LEARNING_SNAPSHOT_SCHEMA = "bioprism-brain-learning-snapshot/0.2"
_REPLAN_SECRET_PATTERNS = (
    re.compile(
        r"(?i)\b(?:api[_ -]?key|access[_ -]?token|refresh[_ -]?token|password|authorization|secret)\b\s*[:=]\s*\S+"
    ),
    re.compile(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{16,}"),
    re.compile(r"\b(?:sk|rk|pk)-[A-Za-z0-9_-]{16,}\b"),
)


def _bounded_route_prompt_context(route: Mapping[str, Any]) -> dict[str, Any]:
    """Project route evidence into a bounded, model-readable context packet.

    The capability route is authoritative evidence about the live catalogue, but it can contain
    many schemas. The model needs the candidate contract, not an unbounded registry dump. Schemas
    are admitted in deterministic order until the packet bound is reached; omitted schemas remain
    explicit so a model cannot mistake a truncated route for a complete catalogue.
    """

    recommended = route.get("recommended_tools", [])
    if not isinstance(recommended, list) or any(not isinstance(tool, str) for tool in recommended):
        raise BrainRunError("capability route returned malformed recommended_tools")
    needs = route.get("needs", [])
    if not isinstance(needs, list) or any(not isinstance(need, Mapping) for need in needs):
        raise BrainRunError("capability route returned malformed needs")
    raw_schemas = route.get("tool_schemas", [])
    if not isinstance(raw_schemas, list) or any(not isinstance(schema, Mapping) for schema in raw_schemas):
        raise BrainRunError("capability route returned malformed tool_schemas")

    compact_needs: list[dict[str, Any]] = []
    for need in needs:
        compact_needs.append(
            {
                "id": need.get("id"),
                "resolution": need.get("resolution"),
                "candidate_groups": need.get("candidate_groups", []),
                "candidate_domains": need.get("candidate_domains", []),
                "candidate_tools": need.get("candidate_tools", []),
            }
        )
    packet: dict[str, Any] = {
        "workflow": "capability_route_context",
        "route_id": route.get("route_id"),
        "catalog_digest": route.get("catalog_digest"),
        "goal": route.get("goal"),
        "needs": compact_needs,
        "recommended_tools": recommended,
        "schema_attachment": route.get("schema_attachment", {}),
        "tool_schemas": [],
        "tool_schemas_omitted": 0,
        "does_not_authorize": [
            "candidate ranking is routing evidence, not permission",
            "the caller mission policy remains the only tool allow-list",
            "tool schemas describe inputs but do not establish domain validity or readiness",
        ],
    }
    for schema in raw_schemas[:MAX_ROUTE_PROMPT_SCHEMAS]:
        candidate = dict(packet)
        candidate["tool_schemas"] = [*packet["tool_schemas"], dict(schema)]
        encoded = json.dumps(candidate, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        if len(encoded.encode("utf-8")) > MAX_ROUTE_PROMPT_BYTES:
            break
        packet["tool_schemas"] = candidate["tool_schemas"]
    packet["tool_schemas_omitted"] = len(raw_schemas) - len(packet["tool_schemas"])
    return packet


def _route_provider_tool_surface(
    provider_tools: Sequence[ProviderTool],
    recommended_tools: Sequence[str],
) -> tuple[ProviderTool, ...]:
    """Narrow provider-visible schemas to the live route recommendation.

    Routing is still not authorization: this only controls what the provider sees in the next
    turn. The caller-owned callback, mission policy, or domain runtime remains the only execution
    authority. Route order is preserved so deterministic ranking remains visible to the model.
    """

    if not isinstance(provider_tools, Sequence) or isinstance(provider_tools, (str, bytes)):
        raise BrainRunError("provider tool surface must be a sequence")
    if not isinstance(recommended_tools, Sequence) or isinstance(recommended_tools, (str, bytes)):
        raise BrainRunError("route recommended_tools must be a sequence")
    recommended = tuple(tool for tool in recommended_tools if isinstance(tool, str) and tool.strip())
    by_name = {tool.name: tool for tool in provider_tools}
    narrowed = tuple(by_name[name] for name in recommended if name in by_name)
    if provider_tools and not narrowed:
        raise BrainRunError("route has no overlap with the caller provider tool surface")
    return narrowed


def _provider_messages_with_content_parts(
    messages: Sequence[Mapping[str, Any]],
    content_parts: Sequence[Mapping[str, Any]],
) -> tuple[dict[str, Any], ...]:
    """Project a text-only prompt into one transient multimodal provider request."""

    rows: list[dict[str, Any]] = []
    task_index = -1
    for index, message in enumerate(messages):
        if (
            not isinstance(message, Mapping)
            or not isinstance(message.get("role"), str)
            or not isinstance(message.get("content"), str)
        ):
            raise BrainRunError("prompt assembly returned malformed provider messages")
        rows.append({"role": message["role"], "content": message["content"]})
        if message.get("role") == "user":
            task_index = index
        if message.get("source_id") == "task" and message.get("role") == "user":
            task_index = index
    if content_parts:
        if task_index < 0:
            raise BrainRunError("prompt assembly has no user task message for content parts")
        text = rows[task_index]["content"]
        rows[task_index] = {
            **rows[task_index],
            "content": tuple(({"type": "text", "text": text}, *(dict(part) for part in content_parts))),
        }
    return tuple(rows)


def _adaptive_route_context(
    route: Mapping[str, Any],
    *,
    task: str,
    route_request: Mapping[str, Any],
) -> dict[str, Any]:
    """Derive bounded contextual-selection labels from one authoritative live route."""

    if route.get("workflow") != "capability_route":
        raise BrainRunError("adaptive route must be a capability_route report")
    if route.get("goal") != task:
        raise BrainRunError("adaptive route goal must match the task")
    unresolved = route.get("unresolved_needs", [])
    if not isinstance(unresolved, list) or any(not isinstance(item, str) for item in unresolved):
        raise BrainRunError("adaptive route returned malformed unresolved_needs")
    if unresolved:
        raise BrainRunError("adaptive route contains unresolved needs: " + ", ".join(unresolved))
    needs = route.get("needs", [])
    if not isinstance(needs, list) or any(not isinstance(need, Mapping) for need in needs):
        raise BrainRunError("adaptive route returned malformed needs")
    domains: set[str] = set()
    capabilities: set[str] = set()
    for need in needs:
        for key, target in (("candidate_domains", domains), ("candidate_groups", capabilities)):
            values = need.get(key, [])
            if not isinstance(values, list) or any(not isinstance(value, str) for value in values):
                raise BrainRunError(f"adaptive route need {key} must be a string list")
            target.update(value for value in values if value.strip())
    coverage = route.get("route_coverage")
    if isinstance(coverage, Mapping):
        for value in coverage.get("candidate_domains", []):
            if isinstance(value, str) and value.strip():
                domains.add(value)
        for value in coverage.get("candidate_groups", []):
            if isinstance(value, str) and value.strip():
                capabilities.add(value)
    if not domains:
        domains.add("cross_domain")
    if not capabilities:
        capabilities.add("cross_domain")
    risk_class = route_request.get("risk_class", "routed_standard")
    task_family = route_request.get("task_family", "routed_task")
    if not isinstance(risk_class, str) or not risk_class.strip():
        raise BrainRunError("route_request.risk_class must be a non-empty string")
    if not isinstance(task_family, str) or not task_family.strip():
        raise BrainRunError("route_request.task_family must be a non-empty string")
    context = {
        "domain": "cross_domain:" + ",".join(sorted(domains)),
        "capability": "route:" + ",".join(sorted(capabilities)),
        "risk_class": risk_class,
        "task_family": task_family,
    }
    for name, value in context.items():
        if len(value.encode("utf-8")) > MAX_ADAPTIVE_ROUTE_LABEL_BYTES:
            raise BrainRunError(f"adaptive route context {name} exceeds the bounded label size")
    BrainLearningLedger._assert_safe(context)
    return context


class BrainLearningLedger:
    """Append-only, value-only persistence for evaluator judgments and bandit state.

    The ledger is deliberately separate from :class:`CredentialStore`: it accepts only the Rust
    learning report and the returned next state, rejects secret-shaped field names, bounds both
    record count and file size, and fsyncs each append. It never stores provider response text.
    """

    _SCHEMA = "bioprism-brain-learning-ledger/0.1"
    _FORBIDDEN_FIELDS = {
        "api_key",
        "apikey",
        "authorization",
        "credential",
        "password",
        "secret",
        "access_token",
        "refresh_token",
    }
    _FORBIDDEN_NORMALIZED_FIELDS = {
        "".join(character for character in field if character.isalnum())
        for field in _FORBIDDEN_FIELDS
    }

    def __init__(
        self,
        path: str | os.PathLike[str],
        *,
        max_records: int = 4096,
        max_bytes: int = 32_000_000,
    ) -> None:
        if max_records <= 0 or max_bytes <= 0:
            raise BrainRunError("learning ledger bounds must be positive")
        self.path = Path(path)
        self.max_records = max_records
        self.max_bytes = max_bytes
        self._lock = threading.RLock()
        self._snapshot_generation = 0
        self._previous_snapshot_digest: str | None = None
        self._snapshot_cache: dict[str, Any] | None = None
        self._snapshot_cache_record_digests: tuple[str, ...] | None = None

    def _invalidate_snapshot_cache(self) -> None:
        self._snapshot_cache = None
        self._snapshot_cache_record_digests = None

    def _snapshot_for_rows(self, rows: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
        normalized_rows = [_validate_learning_ledger_row(row) for row in rows]
        record_digests = tuple(
            hashlib.sha256(_canonical_learning_json(row).encode("utf-8")).hexdigest()
            for row in normalized_rows
        )
        if self._snapshot_cache is not None and self._snapshot_cache_record_digests == record_digests:
            return json.loads(_canonical_learning_json(self._snapshot_cache))
        snapshot = _build_learning_snapshot(
            normalized_rows,
            max_records=self.max_records,
            max_bytes=self.max_bytes,
            snapshot_generation=self._snapshot_generation + 1,
            previous_snapshot_digest=self._previous_snapshot_digest if self._snapshot_generation else None,
        )
        self._snapshot_generation = snapshot["snapshot_generation"]
        self._previous_snapshot_digest = snapshot["snapshot_digest"]
        self._snapshot_cache = snapshot
        self._snapshot_cache_record_digests = record_digests
        return json.loads(_canonical_learning_json(snapshot))

    def append(
        self,
        report: Mapping[str, Any],
        *,
        context_digest: str | None = None,
        replay: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        if not isinstance(report, Mapping):
            raise BrainRunError("learning ledger report must be an object")
        evidence = report.get("learning_evidence")
        next_state = report.get("next_state")
        if not isinstance(evidence, Mapping) or not isinstance(next_state, Mapping):
            raise BrainRunError("learning ledger report must contain evidence and next_state")
        if context_digest is not None and not _valid_digest(context_digest):
            raise BrainRunError("context_digest must be a lowercase SHA-256 digest")
        self._assert_safe(report)
        if replay is not None:
            if not isinstance(replay, Mapping):
                raise BrainRunError("learning ledger replay must be an object")
            self._assert_safe(replay)
            try:
                encoded_replay = json.dumps(
                    dict(replay),
                    ensure_ascii=False,
                    sort_keys=True,
                    separators=(",", ":"),
                    allow_nan=False,
                ).encode("utf-8")
            except (TypeError, ValueError) as error:
                raise BrainRunError("learning ledger replay must be JSON-safe") from error
            if len(encoded_replay) > MAX_BRAIN_REPLAY_BYTES:
                raise BrainRunError("learning ledger replay exceeds the bounded size")
        try:
            encoded_report = json.dumps(
                {"learning_evidence": evidence, "next_state": next_state},
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            ).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise BrainRunError("learning ledger report must be JSON-safe") from error
        if len(encoded_report) > self.max_bytes:
            raise BrainRunError("learning ledger record exceeds max_bytes")
        record: dict[str, Any] = {
            "learning_evidence": evidence,
            "next_state": next_state,
        }
        if context_digest is not None:
            record["context_digest"] = context_digest
        if replay is not None:
            record["replay"] = dict(replay)
        line = json.dumps(
            {"schema": self._SCHEMA, "record": record},
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8") + b"\n"
        with self._lock:
            existing_size = self.path.stat().st_size if self.path.exists() else 0
            if existing_size + len(line) > self.max_bytes:
                raise BrainRunError("learning ledger capacity is exhausted")
            existing_records = self._read_records_locked()
            if len(existing_records) >= self.max_records:
                raise BrainRunError("learning ledger record capacity is exhausted")
            self.path.parent.mkdir(parents=True, exist_ok=True)
            with self.path.open("ab") as handle:
                handle.write(line)
                handle.flush()
                os.fsync(handle.fileno())
            self._invalidate_snapshot_cache()
            record_digest = hashlib.sha256(line.rstrip(b"\n")).hexdigest()
            return {
                "schema": self._SCHEMA,
                "record_index": len(existing_records),
                "record_digest": record_digest,
                "evidence_digest": evidence.get("evidence_digest"),
                "replay_digest": None if replay is None else _json_digest(dict(replay)),
            }

    def begin_episode(
        self,
        episode: BrainLearningEpisode | Mapping[str, Any],
    ) -> dict[str, Any]:
        """Persist a pending, metadata-only episode before delayed evaluation.

        The operation is idempotent for the same episode identity and content. A different
        episode with an already-used identity is rejected so a delayed evaluator cannot silently
        credit a different provider run.
        """

        normalized = episode if isinstance(episode, BrainLearningEpisode) else BrainLearningEpisode.from_mapping(episode)
        payload = normalized.to_dict()
        self._assert_safe(payload)
        record = {
            "record_type": "pending_episode",
            "episode": payload,
        }
        encoded_record = json.dumps(
            record,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
        if len(encoded_record) > MAX_BRAIN_LEARNING_EPISODE_BYTES:
            raise BrainRunError("learning episode record exceeds the bounded size")
        line = json.dumps(
            {"schema": self._SCHEMA, "record": record},
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8") + b"\n"
        with self._lock:
            existing_size = self.path.stat().st_size if self.path.exists() else 0
            existing_records = self._read_records_locked()
            for row in existing_records:
                prior = row.get("record")
                prior_episode = prior.get("episode") if isinstance(prior, Mapping) else None
                if not isinstance(prior_episode, Mapping) or prior_episode.get("episode_id") != normalized.episode_id:
                    continue
                if prior_episode != payload:
                    raise BrainRunError("learning episode identity is already bound to different content")
                prior_line = json.dumps(row, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
                return {
                    "schema": self._SCHEMA,
                    "record_index": existing_records.index(row),
                    "record_digest": hashlib.sha256(prior_line).hexdigest(),
                    "episode_id": normalized.episode_id,
                    "idempotent": True,
                }
            if existing_size + len(line) > self.max_bytes:
                raise BrainRunError("learning ledger capacity is exhausted")
            if len(existing_records) >= self.max_records:
                raise BrainRunError("learning ledger record capacity is exhausted")
            self.path.parent.mkdir(parents=True, exist_ok=True)
            with self.path.open("ab") as handle:
                handle.write(line)
                handle.flush()
                os.fsync(handle.fileno())
            self._invalidate_snapshot_cache()
            record_digest = hashlib.sha256(line.rstrip(b"\n")).hexdigest()
            return {
                "schema": self._SCHEMA,
                "record_index": len(existing_records),
                "record_digest": record_digest,
                "episode_id": normalized.episode_id,
                "idempotent": False,
            }

    def records(self) -> list[dict[str, Any]]:
        with self._lock:
            return self._read_records_locked()

    def snapshot(self) -> dict[str, Any]:
        """Return a verified, portable projection of the value-only learning ledger.

        The JSONL file is intentionally an append target, not a cross-process transport format.
        A snapshot binds every record to a digest and includes only evaluator evidence, bandit
        state, episode identity, and replay metadata; provider prompts, responses, credentials,
        and tool arguments are never added by this boundary.
        """

        with self._lock:
            rows = self._read_records_locked()
            return self._snapshot_for_rows(rows)

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        """Atomically replace the JSONL ledger with a strictly validated snapshot."""

        normalized = _normalize_learning_snapshot(
            snapshot,
            max_records=self.max_records,
            max_bytes=self.max_bytes,
        )
        lines = b"".join(
            _canonical_learning_json(row).encode("utf-8") + b"\n"
            for row in normalized["records"]
        )
        temporary = self.path.with_name(f".{self.path.name}.{uuid.uuid4().hex}.tmp")
        with self._lock:
            try:
                self.path.parent.mkdir(parents=True, exist_ok=True)
                with temporary.open("wb") as handle:
                    handle.write(lines)
                    handle.flush()
                    os.fsync(handle.fileno())
                os.replace(temporary, self.path)
                self._snapshot_generation = int(normalized.get("snapshot_generation", 0))
                self._previous_snapshot_digest = normalized["snapshot_digest"] if self._snapshot_generation else None
                if normalized.get("schema") == BRAIN_LEARNING_SNAPSHOT_SCHEMA:
                    self._snapshot_cache = normalized
                    self._snapshot_cache_record_digests = tuple(normalized["record_digests"])
                else:
                    # A legacy image is a migration input, not a new chain link. The next
                    # snapshot write emits a current-schema generation-one root.
                    self._invalidate_snapshot_cache()
            except (OSError, ValueError) as error:
                try:
                    temporary.unlink(missing_ok=True)
                except OSError:
                    pass
                raise BrainRunError("learning ledger snapshot could not be restored") from error

    def pending_episodes(self, *, limit: int = 128) -> list[BrainLearningEpisode]:
        """Return unsettled delayed-learning episodes without loading provider content."""

        if not isinstance(limit, int) or isinstance(limit, bool) or not 1 <= limit <= self.max_records:
            raise BrainRunError("pending episode limit must be within the ledger record bound")
        rows = self.records()
        episodes: dict[str, BrainLearningEpisode] = {}
        settled: set[str] = set()
        for row in rows:
            record = row.get("record")
            if not isinstance(record, Mapping):
                continue
            episode_raw = record.get("episode")
            if record.get("record_type") == "pending_episode" and isinstance(episode_raw, Mapping):
                episode = BrainLearningEpisode.from_mapping(episode_raw)
                episodes[episode.episode_id] = episode
            replay = record.get("replay")
            if isinstance(replay, Mapping) and isinstance(replay.get("episode_id"), str):
                settled.add(replay["episode_id"])
        return [
            episode
            for episode_id, episode in list(episodes.items())[-limit:]
            if episode_id not in settled
        ]

    def episode(self, episode_id: str) -> BrainLearningEpisode | None:
        """Look up one pending or settled episode by identity."""

        if not isinstance(episode_id, str) or not episode_id.strip():
            raise BrainRunError("episode_id must be a non-empty string")
        for row in reversed(self.records()):
            record = row.get("record")
            raw = record.get("episode") if isinstance(record, Mapping) else None
            if isinstance(raw, Mapping) and raw.get("episode_id") == episode_id:
                return BrainLearningEpisode.from_mapping(raw)
        return None

    def latest_state(self, context_digest: str | None = None) -> dict[str, Any] | None:
        if context_digest is not None and not _valid_digest(context_digest):
            raise BrainRunError("context_digest must be a lowercase SHA-256 digest")
        for row in reversed(self.records()):
            record = row.get("record")
            if not isinstance(record, Mapping):
                continue
            if context_digest is not None and record.get("context_digest") != context_digest:
                continue
            state = record.get("next_state")
            if isinstance(state, Mapping):
                return dict(state)
        return None

    def contextual_state(self, context: Mapping[str, Any]) -> dict[str, Any]:
        """Return evaluator-linked bandit state for one domain/capability/risk context.

        The context is normalized to the same four routing identity fields used by contextual
        model selection. Rich task text and provider payloads are not part of the lookup key.
        This makes the result safe to request before a run (first-run exploration) and useful to
        feed into the next domain-scoped selection without requiring callers to know the digest.
        """

        normalized_context = _normalize_learning_context(context)
        context_digest = _context_identity_digest(normalized_context)
        state: Mapping[str, Any] | None = None
        evaluation_count = 0
        last_evaluator_id: str | None = None
        last_evaluator_version: str | None = None
        for row in reversed(self.records()):
            record = row.get("record")
            if not isinstance(record, Mapping) or record.get("context_digest") != context_digest:
                continue
            candidate_state = record.get("next_state")
            if state is None and isinstance(candidate_state, Mapping):
                state = dict(candidate_state)
            evidence = record.get("learning_evidence")
            if isinstance(evidence, Mapping):
                evaluation_count += 1
                if last_evaluator_id is None and isinstance(evidence.get("evaluator_id"), str):
                    last_evaluator_id = evidence["evaluator_id"]
                if last_evaluator_version is None and isinstance(evidence.get("evaluator_version"), str):
                    last_evaluator_version = evidence["evaluator_version"]
        if state is None:
            state = {
                "schema": "bioprism-brain-bandit/0.1",
                "generation": 0,
                "arms": [],
            }
        result = {
            "schema": BRAIN_CONTEXT_LEARNING_STATE_SCHEMA,
            "context": normalized_context,
            "context_digest": context_digest,
            "bandit_state": {
                **dict(state),
                "arms": _bandit_observations(state, context_digest=context_digest),
            },
            "observed": evaluation_count > 0,
            "evaluation_count": evaluation_count,
            "last_evaluator_id": last_evaluator_id,
            "last_evaluator_version": last_evaluator_version,
            "retention": "context_identity_and_evaluator_bandit_metadata_only",
        }
        self._assert_safe(result)
        return result

    def replays(
        self,
        *,
        run_id: str | None = None,
        evaluator_id: str | None = None,
        limit: int = 128,
    ) -> list[dict[str, Any]]:
        """Return bounded evaluator replay metadata without loading provider/evidence content."""

        if not isinstance(limit, int) or isinstance(limit, bool) or not 1 <= limit <= self.max_records:
            raise BrainRunError("replay limit must be within the ledger record bound")
        for name, value in (("run_id", run_id), ("evaluator_id", evaluator_id)):
            if value is not None and (not isinstance(value, str) or not value.strip()):
                raise BrainRunError(f"{name} must be a non-empty string when supplied")
        matches: list[dict[str, Any]] = []
        for row in reversed(self.records()):
            record = row.get("record")
            replay = record.get("replay") if isinstance(record, Mapping) else None
            if not isinstance(replay, Mapping):
                continue
            if run_id is not None and replay.get("run_id") != run_id:
                continue
            if evaluator_id is not None and replay.get("evaluator_id") != evaluator_id:
                continue
            matches.append(dict(replay))
            if len(matches) >= limit:
                break
        matches.reverse()
        return matches

    def _read_records_locked(self) -> list[dict[str, Any]]:
        if not self.path.exists():
            return []
        if self.path.stat().st_size > self.max_bytes:
            raise BrainRunError("learning ledger exceeds max_bytes")
        rows: list[dict[str, Any]] = []
        with self.path.open("rb") as handle:
            for raw_line in handle:
                if len(rows) >= self.max_records:
                    raise BrainRunError("learning ledger exceeds max_records")
                try:
                    row = json.loads(raw_line.decode("utf-8"))
                except (UnicodeDecodeError, json.JSONDecodeError) as error:
                    raise BrainRunError("learning ledger contains invalid JSON") from error
                validated = _validate_learning_ledger_row(row)
                if raw_line.rstrip(b"\r\n") != _canonical_learning_json(validated).encode("utf-8"):
                    raise BrainRunError("learning ledger contains non-canonical JSON")
                rows.append(validated)
        return rows

    @classmethod
    def _assert_safe(cls, value: Any) -> None:
        if isinstance(value, Mapping):
            for key, child in value.items():
                normalized_key = (
                    "".join(character for character in key.lower() if character.isalnum())
                    if isinstance(key, str)
                    else ""
                )
                if normalized_key in cls._FORBIDDEN_NORMALIZED_FIELDS:
                    raise BrainRunError("learning evidence contains a forbidden secret field")
                cls._assert_safe(child)
        elif isinstance(value, (list, tuple)):
            for child in value:
                cls._assert_safe(child)


def _canonical_learning_json(value: Any) -> str:
    try:
        return json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
    except (TypeError, ValueError) as error:
        raise BrainRunError("learning ledger value must be canonical JSON") from error


def _validate_learning_ledger_row(value: Any) -> dict[str, Any]:
    """Validate one durable row before it can influence a future bandit decision."""

    if not isinstance(value, Mapping) or set(value) != {"schema", "record"}:
        raise BrainRunError("learning ledger contains an invalid record envelope")
    if value.get("schema") != BrainLearningLedger._SCHEMA:
        raise BrainRunError("learning ledger contains an invalid schema")
    record = value.get("record")
    if not isinstance(record, Mapping):
        raise BrainRunError("learning ledger record is missing its projection")
    BrainLearningLedger._assert_safe(value)
    if record.get("record_type") == "pending_episode":
        episode = record.get("episode")
        if not isinstance(episode, Mapping):
            raise BrainRunError("learning ledger pending episode is malformed")
        BrainLearningEpisode.from_mapping(episode)
    elif not isinstance(record.get("learning_evidence"), Mapping) or not isinstance(record.get("next_state"), Mapping):
        raise BrainRunError("learning ledger outcome record is malformed")
    replay = record.get("replay")
    if replay is not None:
        if not isinstance(replay, Mapping):
            raise BrainRunError("learning ledger replay is malformed")
        if len(_canonical_learning_json(dict(replay)).encode("utf-8")) > MAX_BRAIN_REPLAY_BYTES:
            raise BrainRunError("learning ledger replay exceeds the bounded size")
    context_digest = record.get("context_digest")
    if context_digest is not None and not _valid_digest(context_digest):
        raise BrainRunError("learning ledger context_digest is invalid")
    normalized = {
        "schema": BrainLearningLedger._SCHEMA,
        "record": json.loads(_canonical_learning_json(dict(record))),
    }
    # Canonicalization also rejects NaN, non-JSON values, and values that could not survive a
    # portable snapshot round trip.
    _canonical_learning_json(normalized)
    return normalized


def _build_learning_snapshot(
    rows: Sequence[Mapping[str, Any]],
    *,
    max_records: int,
    max_bytes: int,
    snapshot_generation: int = 1,
    previous_snapshot_digest: str | None = None,
) -> dict[str, Any]:
    if isinstance(snapshot_generation, bool) or not isinstance(snapshot_generation, int) or snapshot_generation < 1:
        raise BrainRunError("learning snapshot_generation must start at one")
    if snapshot_generation == 1 and previous_snapshot_digest is not None:
        raise BrainRunError("learning snapshot generation and previous_snapshot_digest are inconsistent")
    if snapshot_generation > 1 and not _valid_digest(previous_snapshot_digest):
        raise BrainRunError("learning previous_snapshot_digest is required after generation one")
    if len(rows) > max_records:
        raise BrainRunError("learning ledger snapshot exceeds max_records")
    normalized_rows = [_validate_learning_ledger_row(row) for row in rows]
    encoded_rows = [_canonical_learning_json(row).encode("utf-8") for row in normalized_rows]
    if sum(len(row) + 1 for row in encoded_rows) > max_bytes:
        raise BrainRunError("learning ledger snapshot exceeds max_bytes")
    record_digests = [hashlib.sha256(row).hexdigest() for row in encoded_rows]
    descriptor = {
        "schema": BRAIN_LEARNING_SNAPSHOT_SCHEMA,
        "snapshot_generation": snapshot_generation,
        "previous_snapshot_digest": previous_snapshot_digest,
        "records": normalized_rows,
        "record_digests": record_digests,
        "head_digest": record_digests[-1] if record_digests else "",
        "retention": "value_only_evaluator_bandit_and_replay_metadata",
        "secret_material": "never_returned",
    }
    snapshot = {**descriptor, "snapshot_digest": _json_digest(descriptor)}
    if len(_canonical_learning_json(snapshot).encode("utf-8")) > min(max_bytes, MAX_BRAIN_LEARNING_SNAPSHOT_BYTES):
        raise BrainRunError("learning ledger snapshot exceeds its byte capacity")
    return snapshot


def _normalize_learning_snapshot(
    value: Mapping[str, Any],
    *,
    max_records: int,
    max_bytes: int,
) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise BrainRunError("learning ledger snapshot is malformed")
    legacy = value.get("schema") == _LEGACY_BRAIN_LEARNING_SNAPSHOT_SCHEMA
    expected_keys = {
        "schema",
        "records",
        "record_digests",
        "head_digest",
        "retention",
        "secret_material",
        "snapshot_digest",
    }
    if not legacy:
        expected_keys.update({"snapshot_generation", "previous_snapshot_digest"})
    if set(value) != expected_keys:
        raise BrainRunError("learning ledger snapshot is malformed")
    if value.get("schema") not in {_LEGACY_BRAIN_LEARNING_SNAPSHOT_SCHEMA, BRAIN_LEARNING_SNAPSHOT_SCHEMA}:
        raise BrainRunError("learning ledger snapshot schema is unsupported")
    if (
        value.get("retention") != "value_only_evaluator_bandit_and_replay_metadata"
        or value.get("secret_material") != "never_returned"
    ):
        raise BrainRunError("learning ledger snapshot retention is invalid")
    raw_rows = value.get("records")
    raw_digests = value.get("record_digests")
    if not isinstance(raw_rows, Sequence) or isinstance(raw_rows, (str, bytes, bytearray)) or len(raw_rows) > max_records:
        raise BrainRunError("learning ledger snapshot record count is outside its bound")
    if not isinstance(raw_digests, Sequence) or isinstance(raw_digests, (str, bytes, bytearray)) or len(raw_digests) != len(raw_rows):
        raise BrainRunError("learning ledger snapshot record digest count is invalid")
    rows: list[dict[str, Any]] = []
    digests: list[str] = []
    total_bytes = 0
    for raw_row, raw_digest in zip(raw_rows, raw_digests):
        row = _validate_learning_ledger_row(raw_row)
        encoded = _canonical_learning_json(row).encode("utf-8")
        total_bytes += len(encoded) + 1
        if total_bytes > max_bytes:
            raise BrainRunError("learning ledger snapshot exceeds max_bytes")
        if not isinstance(raw_digest, str) or not _valid_digest(raw_digest) or hashlib.sha256(encoded).hexdigest() != raw_digest:
            raise BrainRunError("learning ledger snapshot record digest does not match its row")
        rows.append(row)
        digests.append(raw_digest)
    head_digest = value.get("head_digest")
    expected_head = digests[-1] if digests else ""
    if not isinstance(head_digest, str) or (head_digest and not _valid_digest(head_digest)) or head_digest != expected_head:
        raise BrainRunError("learning ledger snapshot head_digest is invalid")
    snapshot_generation = value.get("snapshot_generation")
    previous_snapshot_digest = value.get("previous_snapshot_digest")
    if not legacy:
        if isinstance(snapshot_generation, bool) or not isinstance(snapshot_generation, int) or snapshot_generation < 1:
            raise BrainRunError("learning snapshot_generation must start at one")
        if previous_snapshot_digest is not None and not _valid_digest(previous_snapshot_digest):
            raise BrainRunError("learning previous_snapshot_digest is invalid")
        if (snapshot_generation == 1) != (previous_snapshot_digest is None):
            raise BrainRunError("learning snapshot generation and previous_snapshot_digest are inconsistent")
    descriptor = {
        "schema": value["schema"],
        "records": rows,
        "record_digests": digests,
        "head_digest": head_digest,
        "retention": "value_only_evaluator_bandit_and_replay_metadata",
        "secret_material": "never_returned",
    }
    if not legacy:
        descriptor = {
            **descriptor,
            "snapshot_generation": snapshot_generation,
            "previous_snapshot_digest": previous_snapshot_digest,
        }
    snapshot_digest = value.get("snapshot_digest")
    if not isinstance(snapshot_digest, str) or not _valid_digest(snapshot_digest) or _json_digest(descriptor) != snapshot_digest:
        raise BrainRunError("learning ledger snapshot digest does not match its metadata")
    normalized = {**descriptor, "snapshot_digest": snapshot_digest}
    if len(_canonical_learning_json(normalized).encode("utf-8")) > min(max_bytes, MAX_BRAIN_LEARNING_SNAPSHOT_BYTES):
        raise BrainRunError("learning ledger snapshot exceeds its byte capacity")
    return normalized


def validate_brain_learning_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    """Public strict validator for value-only evaluator and bandit snapshots."""

    return _normalize_learning_snapshot(
        value,
        max_records=4096,
        max_bytes=MAX_BRAIN_LEARNING_SNAPSHOT_BYTES,
    )


class BrainLearningSnapshotTextStore(Protocol):
    """Portable text persistence for evaluator outcomes and bandit state."""

    def read(self) -> str | None: ...

    def write(self, value: str) -> None: ...


class TransactionalBrainLearningSnapshotTextStore(BrainLearningSnapshotTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class JsonBrainLearningSnapshotPersistence:
    """Canonical JSON learning persistence over a caller-owned text store."""

    def __init__(
        self,
        store: BrainLearningSnapshotTextStore,
        *,
        max_bytes: int = MAX_BRAIN_LEARNING_SNAPSHOT_BYTES,
    ) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise BrainRunError("learning JSON persistence requires a text store")
        if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or not 1 <= max_bytes <= MAX_BRAIN_LEARNING_SNAPSHOT_BYTES:
            raise BrainRunError("learning JSON persistence max_bytes is outside its bound")
        self.store = store
        self.max_bytes = max_bytes

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            raise BrainRunError("learning JSON snapshot exceeds its byte bound")
        try:
            raw = json.loads(encoded)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise BrainRunError("learning JSON snapshot is invalid") from error
        if not isinstance(raw, Mapping):
            raise BrainRunError("learning JSON snapshot must be an object")
        normalized = _normalize_learning_snapshot(
            raw,
            max_records=4096,
            max_bytes=self.max_bytes,
        )
        if encoded != _canonical_learning_json(normalized):
            raise BrainRunError("learning JSON snapshot is not canonical")
        return normalized

    def write(self, snapshot: Mapping[str, Any]) -> None:
        normalized = _normalize_learning_snapshot(
            snapshot,
            max_records=4096,
            max_bytes=self.max_bytes,
        )
        encoded = _canonical_learning_json(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise BrainRunError("learning JSON snapshot exceeds its byte bound")
        self.store.write(encoded)


class TransactionalJsonBrainLearningSnapshotPersistence(JsonBrainLearningSnapshotPersistence):
    """Canonical JSON learning persistence with stale-writer fencing."""

    def __init__(
        self,
        store: TransactionalBrainLearningSnapshotTextStore,
        *,
        max_bytes: int = MAX_BRAIN_LEARNING_SNAPSHOT_BYTES,
    ) -> None:
        super().__init__(store, max_bytes=max_bytes)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise BrainRunError("transactional learning persistence requires write_if_unchanged")
        self.store = store

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        if expected_snapshot_digest is not None and not _valid_digest(expected_snapshot_digest):
            raise BrainRunError("learning expected snapshot digest is invalid")
        normalized = _normalize_learning_snapshot(
            snapshot,
            max_records=4096,
            max_bytes=self.max_bytes,
        )
        encoded = _canonical_learning_json(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise BrainRunError("learning JSON snapshot exceeds its byte bound")
        return self.store.write_if_unchanged(expected_snapshot_digest, encoded)


class BrainLearningPersistenceCoordinator:
    """Flush and restore the bandit/evaluator ledger through caller-owned storage."""

    def __init__(self, store: BrainLearningLedger, persistence: Any) -> None:
        if not isinstance(store, BrainLearningLedger):
            raise BrainRunError("learning persistence requires a BrainLearningLedger")
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise BrainRunError("learning persistence adapter is malformed")
        self.store = store
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None

    def restore(self) -> dict[str, Any] | None:
        raw = self.persistence.read()
        if raw is None:
            self._expected_snapshot_digest = None
            return None
        snapshot = _normalize_learning_snapshot(
            raw,
            max_records=self.store.max_records,
            max_bytes=self.store.max_bytes,
        )
        self.store.restore(snapshot)
        self._expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot

    def flush(self) -> dict[str, Any]:
        snapshot = self.store.snapshot()
        write_if_unchanged = getattr(self.persistence, "write_if_unchanged", None)
        if callable(write_if_unchanged):
            if not write_if_unchanged(self._expected_snapshot_digest, snapshot):
                raise BrainRunError("learning persistence compare-and-swap conflict")
        else:
            self.persistence.write(snapshot)
        self._expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot


class BrainWorkspace(Protocol):
    def tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]: ...


def _valid_digest(value: Any) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(
        character in "0123456789abcdef" for character in value
    )


def _normalize_learning_context(context: Mapping[str, Any]) -> dict[str, Any]:
    """Normalize the stable context identity shared by routing and learning persistence."""

    if not isinstance(context, Mapping):
        raise BrainRunError("learning context must be a mapping")
    normalized: dict[str, Any] = {}
    for field in ("domain", "capability", "risk_class"):
        value = context.get(field)
        if not isinstance(value, str) or not value.strip():
            raise BrainRunError(f"learning context.{field} must be a non-empty string")
        normalized[field] = value
    task_family = context.get("task_family")
    if task_family is not None and (not isinstance(task_family, str) or not task_family.strip()):
        raise BrainRunError("learning context.task_family must be a non-empty string when supplied")
    normalized["task_family"] = task_family
    return normalized


def _context_identity_digest(context: Mapping[str, Any]) -> str:
    """Match the Rust contextual-selection digest without retaining arbitrary task text."""

    normalized = _normalize_learning_context(context)
    encoded = json.dumps(
        normalized,
        ensure_ascii=False,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _selection_context_binding(
    selection: Mapping[str, Any],
) -> tuple[str | None, dict[str, Any] | None]:
    """Return a validated contextual binding while retaining legacy digest-only metadata."""

    context_digest = selection.get("context_digest")
    if context_digest is not None and not _valid_digest(context_digest):
        raise BrainRunError("selection context_digest must be a lowercase SHA-256 digest")
    raw_context = selection.get("context")
    if raw_context is None:
        return context_digest, None
    if not isinstance(raw_context, Mapping):
        raise BrainRunError("selection context must be a mapping")
    context = _normalize_learning_context(raw_context)
    expected = _context_identity_digest(context)
    if context_digest != expected:
        raise BrainRunError("selection context_digest does not match its context identity")
    return context_digest, context


def _bandit_observations(
    state: Mapping[str, Any] | None,
    *,
    context_digest: str | None = None,
) -> list[dict[str, Any]]:
    """Project caller-persisted global or context-scoped state into model observations.

    Contextual rows are nested in the canonical Rust state. A missing scoped row deliberately
    falls back to the top-level global arms so first-run exploration remains possible without
    allowing a different context's rewards to leak into this request.
    """

    if state is None:
        return []
    if not isinstance(state, Mapping):
        raise BrainRunError("bandit state must be a mapping")
    BrainLearningLedger._assert_safe(state)
    if context_digest is not None:
        if not _valid_digest(context_digest):
            raise BrainRunError("context_digest must be a lowercase SHA-256 digest")
        contextual_states = state.get("contextual_states", [])
        if not isinstance(contextual_states, list):
            raise BrainRunError("bandit state contextual_states must be a list")
        matching: list[Mapping[str, Any]] = []
        for row in contextual_states:
            if not isinstance(row, Mapping):
                raise BrainRunError("bandit state contextual_states must contain mappings")
            row_digest = row.get("context_digest")
            if row_digest == context_digest:
                row_context = row.get("context")
                if not isinstance(row_context, Mapping):
                    raise BrainRunError("bandit state contextual state identity is malformed")
                normalized_row_context = _normalize_learning_context(row_context)
                if _context_identity_digest(normalized_row_context) != row_digest:
                    raise BrainRunError("bandit state contextual state digest does not match its context")
                matching.append(row)
        if len(matching) > 1:
            raise BrainRunError("bandit state contains duplicate contextual state")
        global_observations = _bandit_observations({"arms": state.get("arms", [])})
        if not matching:
            return global_observations
        contextual_observations = _bandit_observations({"arms": matching[0].get("arms", [])})
        merged = {observation["arm_id"]: observation for observation in global_observations}
        merged.update({observation["arm_id"]: observation for observation in contextual_observations})
        return list(merged.values())
    else:
        arms = state.get("arms", [])
    if not isinstance(arms, list):
        raise BrainRunError("bandit state arms must be a list")
    observations: list[dict[str, Any]] = []
    for arm in arms:
        if not isinstance(arm, Mapping):
            raise BrainRunError("bandit state arms must contain mappings")
        arm_id = arm.get("arm_id")
        pulls = arm.get("pulls", 0)
        reward_sum = arm.get("reward_sum", 0.0)
        failures = arm.get("failures", 0)
        disabled = arm.get("disabled", False)
        if (
            not isinstance(arm_id, str)
            or not arm_id.strip()
            or not isinstance(pulls, int)
            or isinstance(pulls, bool)
            or pulls < 0
            or not isinstance(reward_sum, (int, float))
            or isinstance(reward_sum, bool)
            or not isinstance(failures, int)
            or isinstance(failures, bool)
            or failures < 0
            or not isinstance(disabled, bool)
        ):
            raise BrainRunError("bandit state contains malformed arm statistics")
        observation = {
            "arm_id": arm_id,
            "pulls": pulls,
            "reward_sum": reward_sum,
            "failures": failures,
            "disabled": disabled,
        }
        try:
            json.dumps(observation, ensure_ascii=False, allow_nan=False)
        except (TypeError, ValueError) as error:
            raise BrainRunError("bandit state contains non-finite arm statistics") from error
        observations.append(observation)
    return observations


def _normalize_bandit_arm_collection(
    raw_arms: Any,
    *,
    field: str,
    ensure_arm_id: str | None = None,
) -> list[dict[str, Any]]:
    if not isinstance(raw_arms, list):
        raise BrainRunError(f"{field} must be a list")
    if len(raw_arms) > 512:
        raise BrainRunError(f"{field} contains too many arms")
    copied: list[dict[str, Any]] = []
    seen: set[str] = set()
    for raw_arm in raw_arms:
        if not isinstance(raw_arm, Mapping):
            raise BrainRunError(f"{field} must contain mappings")
        current = dict(raw_arm)
        current_id = current.get("arm_id")
        current.setdefault("pulls", 0)
        current.setdefault("reward_sum", 0.0)
        current.setdefault("failures", 0)
        current.setdefault("disabled", False)
        pulls = current.get("pulls")
        reward_sum = current.get("reward_sum")
        failures = current.get("failures")
        if (
            not isinstance(current_id, str)
            or not current_id.strip()
            or not isinstance(pulls, int)
            or isinstance(pulls, bool)
            or pulls < 0
            or not isinstance(reward_sum, (int, float))
            or isinstance(reward_sum, bool)
            or not math.isfinite(float(reward_sum))
            or not -float(pulls) <= float(reward_sum) <= float(pulls)
            or not isinstance(failures, int)
            or isinstance(failures, bool)
            or failures < 0
            or failures > pulls
            or not isinstance(current.get("disabled"), bool)
        ):
            raise BrainRunError(f"{field} contains malformed arm statistics")
        if current_id in seen:
            raise BrainRunError(f"{field} contains duplicate arm {current_id!r}")
        seen.add(current_id)
        current["reward_sum"] = float(reward_sum)
        copied.append(current)
    if ensure_arm_id is not None and ensure_arm_id not in seen:
        copied.append(
            {
                "arm_id": ensure_arm_id,
                "pulls": 0,
                "reward_sum": 0.0,
                "failures": 0,
                "disabled": False,
            }
        )
    return copied


def _ensure_bandit_arm(
    state: Mapping[str, Any],
    arm_id: str,
    *,
    context_digest: str | None = None,
    context: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Return a safe state that contains the selected arm for first-run learning.

    A brand-new application quite reasonably starts with ``arms=[]``.  Selection can explore
    candidates without an arm record, but the later evaluator update needs a concrete arm to
    credit.  Hydrating only the selected provider/model here keeps that bootstrap deterministic,
    bounded, and independent of any provider payload.
    """

    if not isinstance(state, Mapping):
        raise BrainRunError("bandit_state must be a mapping")
    if not isinstance(arm_id, str) or not arm_id.strip() or len(arm_id.encode("utf-8")) > 512:
        raise BrainRunError("bandit arm_id must be a bounded non-empty string")
    if (context_digest is None) != (context is None):
        raise BrainRunError("context_digest and context must be supplied together")
    normalized_context: dict[str, Any] | None = None
    if context_digest is not None and context is not None:
        normalized_context = _normalize_learning_context(context)
        if _context_identity_digest(normalized_context) != context_digest:
            raise BrainRunError("context_digest does not match its context identity")
    BrainLearningLedger._assert_safe(state)
    normalized = dict(state)
    normalized.setdefault("schema", "bioprism-brain-bandit/0.1")
    generation = normalized.get("generation", 0)
    if not isinstance(generation, int) or isinstance(generation, bool) or generation < 0:
        raise BrainRunError("bandit_state generation must be a non-negative integer")
    normalized["generation"] = generation
    credited_outcomes = normalized.get("credited_outcomes", [])
    if not isinstance(credited_outcomes, list):
        raise BrainRunError("bandit_state credited_outcomes must be a list")
    if len(credited_outcomes) > MAX_BRAIN_CREDITED_OUTCOMES:
        raise BrainRunError("bandit_state credited_outcomes exceed their bounded size")
    seen_outcomes: set[str] = set()
    normalized_outcomes: list[dict[str, Any]] = []
    for raw_outcome in credited_outcomes:
        if not isinstance(raw_outcome, Mapping):
            raise BrainRunError("bandit_state credited_outcomes must contain objects")
        outcome = dict(raw_outcome)
        outcome_digest = outcome.get("outcome_digest")
        outcome_arm = outcome.get("arm_id")
        outcome_reward = outcome.get("reward")
        outcome_failed = outcome.get("failed", False)
        outcome_contract = outcome.get("contract_digest")
        outcome_context = outcome.get("context_digest")
        if (
            not _valid_digest(outcome_digest)
            or not isinstance(outcome_arm, str)
            or not outcome_arm.strip()
            or isinstance(outcome_reward, bool)
            or not isinstance(outcome_reward, (int, float))
            or not math.isfinite(float(outcome_reward))
            or not 0.0 <= float(outcome_reward) <= 1.0
            or not isinstance(outcome_failed, bool)
            or (outcome_contract is not None and not _valid_digest(outcome_contract))
            or (outcome_context is not None and not _valid_digest(outcome_context))
        ):
            raise BrainRunError("bandit_state credited_outcomes contain malformed receipts")
        if outcome_digest in seen_outcomes:
            raise BrainRunError("bandit_state credited_outcomes contain a duplicate digest")
        seen_outcomes.add(outcome_digest)
        normalized_outcomes.append(
            {
                "outcome_digest": outcome_digest,
                "arm_id": outcome_arm,
                "reward": float(outcome_reward),
                "failed": outcome_failed,
                "contract_digest": outcome_contract,
                "context_digest": outcome_context,
            }
        )
    normalized["credited_outcomes"] = normalized_outcomes
    normalized["arms"] = _normalize_bandit_arm_collection(
        normalized.get("arms", []),
        field="bandit_state arms",
        ensure_arm_id=None if normalized_context is not None else arm_id,
    )
    raw_contextual_states = normalized.get("contextual_states", [])
    if not isinstance(raw_contextual_states, list):
        raise BrainRunError("bandit_state contextual_states must be a list")
    if len(raw_contextual_states) > 64:
        raise BrainRunError("bandit_state contains too many contextual states")
    contextual_states: list[dict[str, Any]] = []
    seen_contexts: set[str] = set()
    for raw_contextual in raw_contextual_states:
        if not isinstance(raw_contextual, Mapping):
            raise BrainRunError("bandit_state contextual_states must contain mappings")
        contextual = dict(raw_contextual)
        row_digest = contextual.get("context_digest")
        row_context = contextual.get("context")
        if not isinstance(row_digest, str) or not _valid_digest(row_digest) or not isinstance(row_context, Mapping):
            raise BrainRunError("bandit_state contextual state identity is malformed")
        normalized_row_context = _normalize_learning_context(row_context)
        if _context_identity_digest(normalized_row_context) != row_digest:
            raise BrainRunError("bandit_state contextual state digest does not match its context")
        if row_digest in seen_contexts:
            raise BrainRunError("bandit_state contains duplicate contextual state")
        seen_contexts.add(row_digest)
        row_generation = contextual.get("generation", 0)
        if not isinstance(row_generation, int) or isinstance(row_generation, bool) or row_generation < 0:
            raise BrainRunError("bandit_state contextual generation must be a non-negative integer")
        observed = contextual.get("observed", False)
        if not isinstance(observed, bool):
            raise BrainRunError("bandit_state contextual observed must be boolean")
        contextual_states.append(
            {
                "context_digest": row_digest,
                "context": normalized_row_context,
                "generation": row_generation,
                "arms": _normalize_bandit_arm_collection(
                    contextual.get("arms", []),
                    field="bandit_state contextual arms",
                ),
                "observed": observed,
            }
        )
    if normalized_context is not None and context_digest is not None:
        matching = [row for row in contextual_states if row["context_digest"] == context_digest]
        if matching:
            row_arms = matching[0]["arms"]
            if not any(row["arm_id"] == arm_id for row in row_arms):
                row_arms.append(
                    {
                        "arm_id": arm_id,
                        "pulls": 0,
                        "reward_sum": 0.0,
                        "failures": 0,
                        "disabled": False,
                    }
                )
        else:
            contextual_states.append(
                {
                    "context_digest": context_digest,
                    "context": normalized_context,
                    "generation": 0,
                    "arms": _normalize_bandit_arm_collection([], field="bandit_state contextual arms", ensure_arm_id=arm_id),
                    "observed": False,
                }
            )
    normalized["contextual_states"] = contextual_states
    BrainLearningLedger._assert_safe(normalized)
    try:
        encoded = json.dumps(normalized, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise BrainRunError("bandit_state must be JSON-safe") from error
    if len(encoded) > MAX_BRAIN_LEARNING_EPISODE_BYTES:
        raise BrainRunError("bandit_state exceeds the bounded learning state size")
    return json.loads(encoded.decode("utf-8"))


@dataclass(frozen=True, slots=True)
class BrainRunResult:
    run_id: str
    status: str
    selection: Mapping[str, Any]
    prompt: Mapping[str, Any]
    plan: Mapping[str, Any]
    response: ProviderResponse | None
    outcome_digest: str
    provider_failover: Mapping[str, Any] | None = None
    provider_invocations: tuple[Mapping[str, Any], ...] = ()
    continuation_plan: Mapping[str, Any] | None = None
    # Optional structural feedback for the opt-in autonomous domain response contract.  The
    # provider response remains caller-owned; this field contains only value-only evaluation
    # metadata and is omitted from legacy projections when unused.
    response_evaluation: Mapping[str, Any] | None = None
    # A redacted provider-boundary failure projection used when a parent fan-out contains a
    # failed child.  The exception message, request, credential, response, and wire payload are
    # deliberately never retained here.
    failure: Mapping[str, Any] | None = None

    def to_dict(self) -> dict[str, Any]:
        custom_prompt = self.prompt.get("autonomous_prompt") is not None
        prompt_projection = (
            {
                key: value
                for key, value in self.prompt.items()
                if key not in {"messages", "_provider_messages_override"}
            }
            if custom_prompt
            else dict(self.prompt)
        )
        if custom_prompt and "messages" in self.prompt:
            raw_messages = self.prompt.get("messages")
            prompt_projection["message_count"] = (
                len(raw_messages)
                if isinstance(raw_messages, Sequence) and not isinstance(raw_messages, (str, bytes))
                else None
            )
            prompt_projection.setdefault(
                "retention",
                "provider_messages_transient;digest_only_projection",
            )
        result = {
            "run_id": self.run_id,
            "status": self.status,
            "selection": dict(self.selection),
            "prompt": prompt_projection,
            "plan": dict(self.plan),
            "response": None if self.response is None else self.response.to_dict(),
            "outcome_digest": self.outcome_digest,
            "provider_failover": None if self.provider_failover is None else dict(self.provider_failover),
            "provider_invocations": [dict(receipt) for receipt in self.provider_invocations],
            "continuation_plan": None if self.continuation_plan is None else dict(self.continuation_plan),
            "credential_posture": "handle_only_not_serialized",
            "execution": "provider_call_only",
            "tool_execution": "not_started",
        }
        if self.response_evaluation is not None:
            result["response_evaluation"] = dict(self.response_evaluation)
        if self.failure is not None:
            result["failure"] = dict(self.failure)
        return result


@dataclass(frozen=True, slots=True)
class BrainToolLoopResult:
    """Brain-level envelope for a provider continuation loop.

    The first decision is still planned and approved through the brain kernel. Subsequent native
    tool turns are represented by the runtime's bounded loop; caller code remains the sole effect
    authority through its authorization callback.
    """

    brain_run: BrainRunResult
    status: str
    provider_loop: ProviderToolLoopResult | None
    route: Mapping[str, Any] | None = None
    authorization_receipts: tuple[Mapping[str, Any], ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status,
            "brain_run": self.brain_run.to_dict(),
            "provider_loop": None if self.provider_loop is None else self.provider_loop.to_dict(),
            "route": None if self.route is None else dict(self.route),
            "authorization_receipts": [dict(receipt) for receipt in self.authorization_receipts],
            "authorization": {
                "provider_call": "caller_approved_brain_plan",
                "tool_execution": "caller_callback_only",
            },
        }


@dataclass(frozen=True, slots=True)
class BrainMissionResult:
    """The outcome of proposing and optionally executing one model-authored mission.

    ``preflight`` is always the non-executing server response. ``execution`` is present only
    after the caller explicitly authorizes mission dispatch. The normalized mission carries the
    caller's policy, not a policy selected by the model.
    """

    brain_run: BrainRunResult
    status: str
    mission: Mapping[str, Any] | None
    preflight: Mapping[str, Any] | None
    execution: Mapping[str, Any] | None
    route: Mapping[str, Any] | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status,
            "brain_run": self.brain_run.to_dict(),
            "mission": None if self.mission is None else dict(self.mission),
            "preflight": None if self.preflight is None else dict(self.preflight),
            "execution": None if self.execution is None else dict(self.execution),
            "route": None if self.route is None else dict(self.route),
            "authorization": {
                "provider_call": "recorded_in_brain_run",
                "mission_dispatch": "caller_approved_only",
            },
            "tool_execution": "bounded_agent_mission_executor",
        }


@dataclass(frozen=True, slots=True)
class BrainLearningCycleResult:
    """A bounded mission/evaluation/memory/replan cycle.

    Each attempt is evaluated independently and contributes a separate append-only memory
    episode.  Replanning is proposal-only after a failed attempt unless the caller explicitly
    supplied a mission option that dispatches it; the cycle refuses to replay after a dispatched
    mission because a transport failure is not proof that an external effect did not happen.
    """

    status: str
    final_result: BrainMissionResult
    attempts: tuple[BrainMissionResult, ...]
    evaluations: tuple[Mapping[str, Any], ...]
    memory_receipts: tuple[Mapping[str, Any], ...]
    recalled_memory: tuple[Mapping[str, Any], ...]
    replan_count: int
    trajectory_result: "BrainLearningTrajectoryResult" | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "bioprism-brain-learning-cycle/0.1",
            "status": self.status,
            "final_result": self.final_result.to_dict(),
            "attempts": [attempt.to_dict() for attempt in self.attempts],
            "evaluations": [dict(evaluation) for evaluation in self.evaluations],
            "memory_receipts": [dict(receipt) for receipt in self.memory_receipts],
            "recalled_memory": [dict(episode) for episode in self.recalled_memory],
            "replan_count": self.replan_count,
            "trajectory_result": None if self.trajectory_result is None else self.trajectory_result.to_dict(),
            "authorization": {
                "memory": "value_only_hash_chained",
                "mission_dispatch": "caller_approved_only",
            },
        }


@dataclass(frozen=True, slots=True)
class BrainJobRunResult:
    """Result envelope for one claimed, resolver-backed durable brain job."""

    status: str
    job: Mapping[str, Any]
    cycle: BrainLearningCycleResult | None
    error_class: str | None = None
    workflow: Any | None = None
    effect_reconciliation: Mapping[str, Any] | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "bioprism-brain-job-run/0.1",
            "status": self.status,
            "job": dict(self.job),
            "cycle": None if self.cycle is None else self.cycle.to_dict(),
            "workflow": None if self.workflow is None else self.workflow.to_dict(),
            "error_class": self.error_class,
            **({"effect_reconciliation": dict(self.effect_reconciliation)} if self.effect_reconciliation is not None else {}),
            "retention": "job_metadata_and_learning_digests_only; workflow_checkpoint_caller_owned",
        }


def _mission_tool_identifier(value: Any) -> bool:
    return isinstance(value, str) and bool(value) and all(
        character.isalnum() or character == "_" for character in value
    )


@dataclass(frozen=True, slots=True)
class BrainLearningEpisode:
    """A restart-safe, value-only handle for delayed evaluator feedback.

    The episode stores the evaluator projection rather than a provider transcript. Applications
    may persist this object and later supply their separately retained evidence packet to
    :meth:`BrainOutcomeEvaluator.evaluate_episode`. The episode itself contains no task text,
    prompt, credential, response, tool argument, or tool output.
    """

    episode_id: str
    evaluation_input: Mapping[str, Any]
    arm_id: str
    evidence_digest: str | None = None
    status: str = "pending"

    def __post_init__(self) -> None:
        if not isinstance(self.episode_id, str) or not self.episode_id.strip() or len(self.episode_id.encode("utf-8")) > 512:
            raise BrainRunError("learning episode_id must be a bounded non-empty string")
        if not isinstance(self.arm_id, str) or not self.arm_id.strip() or len(self.arm_id.encode("utf-8")) > 512:
            raise BrainRunError("learning episode arm_id must be a bounded non-empty string")
        if self.status not in {"pending", "settled"}:
            raise BrainRunError("learning episode status must be pending or settled")
        if not isinstance(self.evaluation_input, Mapping):
            raise BrainRunError("learning episode evaluation_input must be a mapping")
        if self.evaluation_input.get("schema") != "bioprism-brain-evaluator-input/0.1":
            raise BrainRunError("learning episode evaluation_input has an invalid schema")
        BrainLearningLedger._assert_safe(self.evaluation_input)
        try:
            encoded = json.dumps(
                dict(self.evaluation_input),
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            ).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise BrainRunError("learning episode evaluation_input must be JSON-safe") from error
        if len(encoded) > MAX_BRAIN_LEARNING_EPISODE_BYTES:
            raise BrainRunError("learning episode exceeds the bounded size")
        if self.evidence_digest is not None and not _valid_digest(self.evidence_digest):
            raise BrainRunError("learning episode evidence_digest must be a lowercase SHA-256 digest")
        input_digest = self.evaluation_input.get("evidence_digest")
        if input_digest is not None and not _valid_digest(input_digest):
            raise BrainRunError("learning episode evaluation_input evidence_digest is malformed")
        if self.evidence_digest != input_digest:
            raise BrainRunError("learning episode evidence_digest must match evaluation_input")
        object.__setattr__(self, "evaluation_input", json.loads(encoded.decode("utf-8")))

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "BrainLearningEpisode":
        if not isinstance(value, Mapping) or value.get("schema") != BRAIN_LEARNING_EPISODE_SCHEMA:
            raise BrainRunError("learning episode has an invalid schema")
        return cls(
            episode_id=value.get("episode_id"),
            evaluation_input=value.get("evaluation_input"),
            arm_id=value.get("arm_id"),
            evidence_digest=value.get("evidence_digest"),
            status=value.get("status", "pending"),
        )

    @property
    def run_id(self) -> str:
        value = self.evaluation_input.get("run_id")
        if not isinstance(value, str) or not value.strip():
            raise BrainRunError("learning episode evaluation_input is missing run_id")
        return value

    @property
    def result_kind(self) -> str:
        value = self.evaluation_input.get("result_kind")
        if not isinstance(value, str) or not value.strip():
            raise BrainRunError("learning episode evaluation_input is missing result_kind")
        return value

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": BRAIN_LEARNING_EPISODE_SCHEMA,
            "episode_id": self.episode_id,
            "evaluation_input": dict(self.evaluation_input),
            "arm_id": self.arm_id,
            "evidence_digest": self.evidence_digest,
            "status": self.status,
            "retention": "value_only_evaluator_projection_and_digests",
        }


@dataclass(frozen=True, slots=True)
class BrainLearningTrajectory:
    """An ordered, delayed-feedback group of value-only episodes.

    A trajectory is the smallest durable unit that lets the brain assign credit across a
    multi-step workflow, mission attempt sequence, or cross-domain fan-out. It contains only
    episode projections and a bounded discount factor; the provider transcript, prompt, task
    text, tool arguments, credentials, and evaluator evidence remain outside this object.
    """

    trajectory_id: str
    episodes: tuple[BrainLearningEpisode, ...]
    discount: float = 0.90
    terminal_reward: float | None = None

    def __post_init__(self) -> None:
        if (
            not isinstance(self.trajectory_id, str)
            or not self.trajectory_id.strip()
            or len(self.trajectory_id.encode("utf-8")) > 512
        ):
            raise BrainRunError("learning trajectory_id must be a bounded non-empty string")
        if not isinstance(self.episodes, Sequence) or isinstance(self.episodes, (str, bytes)):
            raise BrainRunError("learning trajectory episodes must be a sequence")
        if not 1 <= len(self.episodes) <= MAX_BRAIN_LEARNING_TRAJECTORY_STEPS:
            raise BrainRunError(
                "learning trajectory must contain between 1 and "
                f"{MAX_BRAIN_LEARNING_TRAJECTORY_STEPS} episodes"
            )
        if any(not isinstance(episode, BrainLearningEpisode) for episode in self.episodes):
            raise BrainRunError("learning trajectory episodes are malformed")
        if any(episode.status != "pending" for episode in self.episodes):
            raise BrainRunError("learning trajectory can contain only pending episodes")
        episode_ids = [episode.episode_id for episode in self.episodes]
        run_ids = [episode.run_id for episode in self.episodes]
        if len(set(episode_ids)) != len(episode_ids):
            raise BrainRunError("learning trajectory contains duplicate episode ids")
        if len(set(run_ids)) != len(run_ids):
            raise BrainRunError("learning trajectory contains duplicate run ids")
        if (
            isinstance(self.discount, bool)
            or not isinstance(self.discount, (int, float))
            or not math.isfinite(float(self.discount))
            or not 0.0 < float(self.discount) <= 1.0
        ):
            raise BrainRunError("learning trajectory discount must be within (0, 1]")
        if self.terminal_reward is not None and (
            isinstance(self.terminal_reward, bool)
            or not isinstance(self.terminal_reward, (int, float))
            or not math.isfinite(float(self.terminal_reward))
            or not -1.0 <= float(self.terminal_reward) <= 1.0
        ):
            raise BrainRunError("learning trajectory terminal_reward must be within [-1, 1]")
        object.__setattr__(self, "episodes", tuple(self.episodes))
        object.__setattr__(self, "discount", float(self.discount))
        if self.terminal_reward is not None:
            object.__setattr__(self, "terminal_reward", float(self.terminal_reward))
        try:
            encoded = json.dumps(
                self.to_dict(),
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            ).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise BrainRunError("learning trajectory must be JSON-safe") from error
        if len(encoded) > MAX_BRAIN_LEARNING_TRAJECTORY_BYTES:
            raise BrainRunError("learning trajectory exceeds the bounded size")

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "BrainLearningTrajectory":
        if not isinstance(value, Mapping) or value.get("schema") != BRAIN_LEARNING_TRAJECTORY_SCHEMA:
            raise BrainRunError("learning trajectory has an invalid schema")
        raw_episodes = value.get("episodes")
        if not isinstance(raw_episodes, Sequence) or isinstance(raw_episodes, (str, bytes)):
            raise BrainRunError("learning trajectory episodes must be a sequence")
        return cls(
            trajectory_id=value.get("trajectory_id"),
            episodes=tuple(
                item if isinstance(item, BrainLearningEpisode) else BrainLearningEpisode.from_mapping(item)
                for item in raw_episodes
            ),
            discount=value.get("discount", 0.90),
            terminal_reward=value.get("terminal_reward"),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": BRAIN_LEARNING_TRAJECTORY_SCHEMA,
            "trajectory_id": self.trajectory_id,
            "episodes": [episode.to_dict() for episode in self.episodes],
            "discount": self.discount,
            "terminal_reward": self.terminal_reward,
            "retention": "ordered_value_only_episode_projections_and_digests",
        }


@dataclass(frozen=True, slots=True)
class BrainLearningTrajectoryResult:
    """Settled trajectory credit and the caller-owned next bandit state."""

    status: str
    trajectory: BrainLearningTrajectory
    decisions: tuple["BrainEvaluatorDecision", ...]
    recordings: tuple[Mapping[str, Any], ...]
    credited_rewards: tuple[float, ...]
    bandit_state: Mapping[str, Any]

    def __post_init__(self) -> None:
        if self.status not in {"settled", "partially_settled"}:
            raise BrainRunError("learning trajectory result has an invalid status")
        if not isinstance(self.trajectory, BrainLearningTrajectory):
            raise BrainRunError("learning trajectory result trajectory is malformed")
        count = len(self.trajectory.episodes)
        if len(self.decisions) != count or len(self.recordings) != count or len(self.credited_rewards) != count:
            raise BrainRunError("learning trajectory result lengths do not match the trajectory")
        if any(not isinstance(decision, BrainEvaluatorDecision) for decision in self.decisions):
            raise BrainRunError("learning trajectory result decisions are malformed")
        if any(
            isinstance(reward, bool)
            or not isinstance(reward, (int, float))
            or not math.isfinite(float(reward))
            or not -1.0 <= float(reward) <= 1.0
            for reward in self.credited_rewards
        ):
            raise BrainRunError("learning trajectory credited rewards are malformed")
        if any(not isinstance(recording, Mapping) for recording in self.recordings):
            raise BrainRunError("learning trajectory recordings are malformed")
        if not isinstance(self.bandit_state, Mapping):
            raise BrainRunError("learning trajectory bandit_state must be a mapping")
        BrainLearningLedger._assert_safe(self.bandit_state)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "bioprism-brain-learning-trajectory-result/0.1",
            "status": self.status,
            "trajectory": self.trajectory.to_dict(),
            "decisions": [decision.to_dict() for decision in self.decisions],
            "recordings": [dict(recording) for recording in self.recordings],
            "credited_rewards": list(self.credited_rewards),
            "bandit_state": dict(self.bandit_state),
            "credit_assignment": "discounted_return_to_go_with_optional_terminal_reward",
            "retention": "value_only_evaluator_and_bandit_metadata",
        }


def _json_digest(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _prompt_bound_idempotency_key(
    caller_key: str | None,
    *,
    prompt_digest: str,
    metadata: Mapping[str, Any] | None,
) -> str:
    """Bind provider request identity to the reviewed prompt implementation.

    A caller key alone is not sufficient once a versioned prompt can change the provider
    request. The digest keeps retries deterministic while ensuring a prompt rollout cannot
    accidentally reuse a prior request identity. Only prompt metadata and digests cross this
    boundary; rendered messages remain process-local.
    """

    return _json_digest(
        {
            "schema": "bioprism-python-autonomous-prompt-request/0.1",
            "caller_key": caller_key,
            "prompt_digest": prompt_digest,
            "manifest_digest": None if metadata is None else metadata.get("manifest_digest"),
            "selection_plan_digest": None if metadata is None else metadata.get("selection_plan_digest"),
        }
    )


def _learning_outcome_digest(
    result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
) -> str:
    """Bind evaluator credit to the complete bounded execution shape."""

    if isinstance(result, BrainRunResult):
        return result.outcome_digest
    if isinstance(result, BrainToolLoopResult):
        final_response = None if result.provider_loop is None else result.provider_loop.final_response
        return _json_digest(
            {
                "brain_outcome_digest": result.brain_run.outcome_digest,
                "status": result.status,
                "provider_loop_status": None
                if result.provider_loop is None
                else result.provider_loop.status,
                "turns": None if result.provider_loop is None else result.provider_loop.turns,
                "tool_calls": None
                if result.provider_loop is None
                else result.provider_loop.tool_calls,
                "final_provider": None if final_response is None else final_response.provider,
                "final_model": None if final_response is None else final_response.model,
                "final_request_id": None if final_response is None else final_response.request_id,
            }
        )
    if isinstance(result, BrainMissionResult):
        execution = result.execution or {}
        return _json_digest(
            {
                "brain_outcome_digest": result.brain_run.outcome_digest,
                "status": result.status,
                "mission_status": execution.get("mission_status"),
                "execution": execution.get("execution"),
                "result_digest": execution.get("result_digest"),
            }
        )
    raise BrainRunError("result must be a BrainRunResult, BrainToolLoopResult, or BrainMissionResult")


def build_model_selection_audit(selection: Mapping[str, Any]) -> dict[str, Any]:
    """Project the Rust model-selection report into bounded routing evidence.

    The Rust kernel remains authoritative for eligibility and ordering.  This projection makes
    that decision inspectable at every Python execution boundary without copying the task,
    prompts, credentials, or provider payloads.  ``routing_confidence`` is deliberately a
    heuristic about selection stability (score margin plus observed coverage); it is not a
    probability that the selected model will answer correctly and it never becomes reward by
    itself.
    """

    if not isinstance(selection, Mapping):
        raise BrainRunError("model selection must be a mapping")
    raw_ranking = selection.get("ranking", [])
    if not isinstance(raw_ranking, Sequence) or isinstance(raw_ranking, (str, bytes)):
        raise BrainRunError("model selection ranking must be a sequence")
    if len(raw_ranking) > MAX_MODEL_SELECTION_AUDIT_INPUT_RANKING:
        raise BrainRunError("model selection ranking exceeds its bounded input size")

    decision_digest = selection.get("decision_digest")
    if decision_digest is not None and not _valid_digest(decision_digest):
        raise BrainRunError("model selection decision_digest must be a lowercase SHA-256 digest")

    ranking: list[dict[str, Any]] = []
    rejection_counts: dict[str, int] = {}
    eligible_count = 0
    selected_id: str | None = None
    selected_model = selection.get("selected_model")
    if isinstance(selected_model, Mapping):
        provider = selected_model.get("provider")
        model = selected_model.get("model")
        if isinstance(provider, str) and isinstance(model, str) and provider and model:
            selected_id = f"{provider}/{model}"

    def finite_number(value: Any, field: str) -> float:
        if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
            raise BrainRunError(f"model selection ranking {field} must be finite")
        return float(value)

    for raw in raw_ranking:
        if not isinstance(raw, Mapping):
            raise BrainRunError("model selection ranking must contain mappings")
        model_id = raw.get("model_id")
        eligible = raw.get("eligible")
        reasons = raw.get("reasons", [])
        pulls = raw.get("observed_pulls", 0)
        if not isinstance(model_id, str) or not model_id.strip() or len(model_id.encode("utf-8")) > 512:
            raise BrainRunError("model selection ranking model_id is malformed")
        if not isinstance(eligible, bool):
            raise BrainRunError("model selection ranking eligible flag is malformed")
        if not isinstance(reasons, Sequence) or isinstance(reasons, (str, bytes)) or any(
            not isinstance(reason, str)
            or not reason.strip()
            or len(reason.encode("utf-8")) > MAX_MODEL_SELECTION_AUDIT_REASON_BYTES
            or any(ord(character) < 32 for character in reason)
            for reason in reasons
        ):
            raise BrainRunError("model selection ranking reasons are malformed")
        if not isinstance(pulls, int) or isinstance(pulls, bool) or pulls < 0:
            raise BrainRunError("model selection ranking observed_pulls is malformed")
        candidate = {
            "model_id": model_id,
            "eligible": eligible,
            "reasons": list(reasons),
            "base_score": finite_number(raw.get("base_score", 0.0), "base_score"),
            "exploration_bonus": finite_number(raw.get("exploration_bonus", 0.0), "exploration_bonus"),
            "score": finite_number(raw.get("score", 0.0), "score"),
            "observed_pulls": pulls,
        }
        ranking.append(candidate)
        if eligible:
            eligible_count += 1
        else:
            for reason in reasons:
                rejection_counts[reason] = rejection_counts.get(reason, 0) + 1

    omitted = max(0, len(ranking) - MAX_MODEL_SELECTION_AUDIT_RANKING)
    retained = ranking[:MAX_MODEL_SELECTION_AUDIT_RANKING]
    selected = next((item for item in ranking if item["model_id"] == selected_id), None)
    eligible_scores = [item for item in ranking if item["eligible"]]
    eligible_scores.sort(key=lambda item: (-item["score"], item["model_id"]))
    runner_up = next((item for item in eligible_scores if item["model_id"] != selected_id), None)
    margin = None
    if selected is not None and runner_up is not None:
        margin = max(0.0, selected["score"] - runner_up["score"])
    selected_pulls = 0 if selected is None else selected["observed_pulls"]
    total_pulls = sum(item["observed_pulls"] for item in ranking)
    observation_coverage = selected_pulls / (selected_pulls + 4.0)
    margin_scale = 0.0 if selected is None or margin is None else margin / (abs(selected["score"]) + 1.0)
    routing_confidence = max(0.0, min(1.0, 0.55 * margin_scale + 0.45 * observation_coverage))
    exploration_bonus = None if selected is None else selected["exploration_bonus"]
    selection_status = selection.get("selection_status")
    if not isinstance(selection_status, str) or not selection_status.strip():
        selection_status = "selected" if selected is not None else "refused_no_eligible_model"
    kernel_selection_confidence = selection.get("selection_confidence")
    if kernel_selection_confidence is not None:
        if (
            isinstance(kernel_selection_confidence, bool)
            or not isinstance(kernel_selection_confidence, (int, float))
            or not math.isfinite(float(kernel_selection_confidence))
            or not 0.0 <= float(kernel_selection_confidence) <= 1.0
        ):
            raise BrainRunError("model selection selection_confidence must be within [0, 1]")
        kernel_selection_confidence = float(kernel_selection_confidence)
    kernel_threshold = selection.get("min_selection_confidence")
    if kernel_threshold is not None:
        if (
            isinstance(kernel_threshold, bool)
            or not isinstance(kernel_threshold, (int, float))
            or not math.isfinite(float(kernel_threshold))
            or not 0.0 <= float(kernel_threshold) <= 1.0
        ):
            raise BrainRunError("model selection min_selection_confidence must be within [0, 1]")
        kernel_threshold = float(kernel_threshold)

    audit_without_digest: dict[str, Any] = {
        "schema": MODEL_SELECTION_AUDIT_SCHEMA,
        "selection_status": selection_status,
        "selected_model": None
        if selected_id is None
        else {"model_id": selected_id, "provider": selected_model.get("provider"), "model": selected_model.get("model")}
        if isinstance(selected_model, Mapping)
        else {"model_id": selected_id},
        "decision_digest": decision_digest,
        "ranking": retained,
        "ranking_omitted": omitted,
        "eligibility": {
            "eligible_count": eligible_count,
            "rejected_count": len(ranking) - eligible_count,
            "rejection_counts": {key: rejection_counts[key] for key in sorted(rejection_counts)},
        },
        "exploration": {
            "selected_bonus": exploration_bonus,
            "selected_observed_pulls": selected_pulls,
            "total_observed_pulls": total_pulls,
            "unseen_eligible_count": sum(1 for item in eligible_scores if item["observed_pulls"] == 0),
        },
        "stability": {
            "runner_up_model_id": None if runner_up is None else runner_up["model_id"],
            "score_margin": margin,
            "observation_coverage": observation_coverage,
            "routing_confidence": routing_confidence,
            "kernel_selection_confidence": kernel_selection_confidence,
            "kernel_selection_confidence_floor": kernel_threshold,
            "confidence_basis": "score_margin_and_observation_coverage_heuristic",
        },
        "does_not_claim": [
            "routing confidence is not answer correctness probability",
            "transport success is not task reward",
            "selection does not authenticate a provider or redeem a credential",
        ],
        "retention": "metadata_only_no_task_or_provider_payloads",
    }
    audit_without_digest["audit_digest"] = _json_digest(audit_without_digest)
    BrainLearningLedger._assert_safe(audit_without_digest)
    try:
        encoded = json.dumps(audit_without_digest, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise BrainRunError("model selection audit is not JSON-safe") from error
    if len(encoded) > 150_000:
        raise BrainRunError("model selection audit exceeds its bounded size")
    return audit_without_digest


def _selection_attempt_metadata(audit: Mapping[str, Any]) -> dict[str, Any]:
    """Return the small audit join carried by bounded failover metadata."""

    stability = audit.get("stability")
    eligibility = audit.get("eligibility")
    exploration = audit.get("exploration")
    return {
        "selection_audit_digest": audit.get("audit_digest"),
        "routing_confidence": stability.get("routing_confidence") if isinstance(stability, Mapping) else None,
        "eligible_count": eligibility.get("eligible_count") if isinstance(eligibility, Mapping) else None,
        "selected_exploration_bonus": exploration.get("selected_bonus") if isinstance(exploration, Mapping) else None,
    }


def _continuation_identifier(value: Any, *, field: str, maximum: int) -> str:
    if (
        not isinstance(value, str)
        or not value.strip()
        or len(value) > maximum
        or any(ord(character) < 32 for character in value)
    ):
        raise BrainRunError(f"{field} is outside its continuation bounds")
    return value


def _continuation_status_code(value: Any) -> int | None:
    if value is None:
        return None
    if not isinstance(value, int) or isinstance(value, bool) or not 0 <= value <= 599:
        raise BrainRunError("model continuation status code is invalid")
    return value


def _continuation_candidate_digest(candidate: Mapping[str, Any]) -> str:
    return _json_digest(
        {
            "provider": candidate.get("provider"),
            "model": candidate.get("model"),
            "capabilities": sorted(candidate.get("capabilities", [])),
            "context_window_tokens": candidate.get("context_window_tokens"),
            "max_output_tokens": candidate.get("max_output_tokens"),
            "quality": candidate.get("quality"),
            "latency_ms": candidate.get("latency_ms"),
            "cost_per_million_tokens": candidate.get("cost_per_million_tokens"),
            "reliability": candidate.get("reliability"),
            "requires_credential": candidate.get("requires_credential"),
            "enabled": candidate.get("enabled", True),
        }
    )


def build_model_continuation_plan(
    selection: Mapping[str, Any],
    model_candidates: Sequence[Mapping[str, Any]],
    *,
    max_failovers: int = 0,
) -> dict[str, Any]:
    """Compile a fixed, metadata-only model fallback ladder from one selection decision.

    Adaptive execution may still update transport health after a failure, but it must not let
    that update silently reorder the current run. The returned plan and its cursor are safe to
    persist across a worker restart; task text, prompts, credentials, and provider responses are
    deliberately excluded.
    """

    if not isinstance(selection, Mapping):
        raise BrainRunError("model continuation selection must be a mapping")
    if (
        not isinstance(max_failovers, int)
        or isinstance(max_failovers, bool)
        or not 0 <= max_failovers <= MAX_MODEL_CONTINUATION_FAILOVERS
    ):
        raise BrainRunError(
            f"model continuation max_failovers must be within [0, {MAX_MODEL_CONTINUATION_FAILOVERS}]"
        )
    selected_model = selection.get("selected_model")
    if not isinstance(selected_model, Mapping):
        raise BrainRunError("model continuation requires a selected model")
    selected_provider = _continuation_identifier(
        selected_model.get("provider"), field="selected provider", maximum=256
    )
    selected_name = _continuation_identifier(
        selected_model.get("model"), field="selected model", maximum=512
    )
    selected_id = f"{selected_provider}/{selected_name}"
    if not isinstance(model_candidates, Sequence) or isinstance(model_candidates, (str, bytes)):
        raise BrainRunError("model continuation candidates must be a sequence")
    candidates: dict[str, Mapping[str, Any]] = {}
    for candidate in model_candidates:
        if not isinstance(candidate, Mapping):
            raise BrainRunError("model continuation candidates must contain mappings")
        provider = _continuation_identifier(
            candidate.get("provider"), field="continuation candidate provider", maximum=256
        )
        model = _continuation_identifier(
            candidate.get("model"), field="continuation candidate model", maximum=512
        )
        arm_id = f"{provider}/{model}"
        if arm_id in candidates:
            raise BrainRunError(f"model continuation contains duplicate model {arm_id}")
        candidates[arm_id] = candidate
    if selected_id not in candidates:
        raise BrainRunError("selected model is absent from model continuation candidates")

    ranking = selection.get("ranking", [])
    if not isinstance(ranking, Sequence) or isinstance(ranking, (str, bytes)):
        raise BrainRunError("model continuation selection ranking must be a sequence")
    eligible: list[tuple[str, int]] = []
    seen: set[str] = set()
    selected_ranked = False
    if len(ranking) == 0:
        # Older caller-owned workspaces may return a selected_model without a ranking. Preserve
        # compatibility while making the fallback order explicit and deterministic from the
        # already-admitted candidate sequence.
        ranking = [
            {
                "model_id": f"{candidate.get('provider')}/{candidate.get('model')}",
                "eligible": candidate.get("enabled", True) is True,
            }
            for candidate in model_candidates
            if isinstance(candidate, Mapping)
        ]
        # Some older workspace adapters return an authoritative selected_model but mark their
        # catalogue row disabled because readiness was evaluated outside the adapter. Preserve
        # that explicit selection as the first step; it is still bounded by the supplied model
        # candidate identity and never grants a new provider or credential.
        if not any(row.get("model_id") == selected_id and row.get("eligible") is True for row in ranking):
            ranking.insert(0, {"model_id": selected_id, "eligible": True})
    for ranking_index, row in enumerate(ranking):
        if not isinstance(row, Mapping) or row.get("eligible") is not True:
            continue
        model_id = row.get("model_id")
        if not isinstance(model_id, str) or "/" not in model_id:
            provider = row.get("provider")
            model = row.get("model")
            if isinstance(provider, str) and isinstance(model, str):
                model_id = f"{provider}/{model}"
        if not isinstance(model_id, str) or model_id in seen or model_id not in candidates:
            continue
        seen.add(model_id)
        eligible.append((model_id, ranking_index))
        selected_ranked = selected_ranked or model_id == selected_id
    if not selected_ranked:
        raise BrainRunError("selected model is not eligible in the continuation ranking")

    ordered_ids = [selected_id] + [model_id for model_id, _ in eligible]
    ranking_indices = {model_id: index for model_id, index in eligible}
    steps: list[dict[str, Any]] = []
    for model_id in ordered_ids:
        if model_id not in candidates or model_id in {step["model_id"] for step in steps}:
            continue
        # Retain the whole bounded ladder: a provider-scoped outage may skip several sibling
        # arms while consuming only one failover transition.
        if len(steps) >= MAX_MODEL_CONTINUATION_STEPS:
            break
        provider, model = model_id.split("/", 1)
        steps.append(
            {
                "order": len(steps),
                "provider": provider,
                "model": model,
                "model_id": model_id,
                "candidate_digest": _continuation_candidate_digest(candidates[model_id]),
                "ranking_index": ranking_indices[model_id],
                "failure_policy": {
                    "timeout_with_closed_circuit": "exclude_model",
                    "retryable_provider_error": "exclude_provider",
                },
            }
        )
    if not steps or steps[0]["model_id"] != selected_id:
        raise BrainRunError("model continuation could not place the selected model first")
    body = {
        "schema": MODEL_CONTINUATION_SCHEMA,
        "selection_digest": _json_digest(selection),
        "strategy": "fixed_selection_snapshot",
        "max_failovers": max_failovers,
        "steps": steps,
        "omitted_eligible_candidates": max(0, len(eligible) - len(steps)),
        "retention": "selection_metadata_only_no_task_prompt_provider_payloads",
        "secret_material": "never_returned",
    }
    return {**body, "plan_digest": _json_digest(body)}


def _seal_model_continuation_state(body: Mapping[str, Any]) -> dict[str, Any]:
    result = dict(body)
    result["state_digest"] = _json_digest(body)
    return result


def create_model_continuation_state(plan: Mapping[str, Any]) -> dict[str, Any]:
    validate_model_continuation_plan(plan)
    return _seal_model_continuation_state(
        {
            "schema": MODEL_CONTINUATION_STATE_SCHEMA,
            "plan_digest": plan["plan_digest"],
            "next_step_index": 0,
            "failovers_used": 0,
            "excluded_providers": [],
            "excluded_models": [],
            "attempts": [],
            "status": "ready",
            "retention": "selection_metadata_only_no_task_prompt_provider_payloads",
            "secret_material": "never_returned",
        }
    )


def validate_model_continuation_plan(plan: Mapping[str, Any]) -> None:
    if not isinstance(plan, Mapping) or plan.get("schema") != MODEL_CONTINUATION_SCHEMA:
        raise BrainRunError("model continuation plan has an invalid schema")
    if set(plan.keys()) != {
        "schema",
        "selection_digest",
        "strategy",
        "max_failovers",
        "steps",
        "omitted_eligible_candidates",
        "retention",
        "secret_material",
        "plan_digest",
    }:
        raise BrainRunError("model continuation plan contains unsupported fields")
    plan_digest = plan.get("plan_digest")
    if not _valid_digest(plan_digest):
        raise BrainRunError("model continuation plan digest is malformed")
    body = {key: value for key, value in plan.items() if key != "plan_digest"}
    if _json_digest(body) != plan_digest:
        raise BrainRunError("model continuation plan digest mismatch")
    steps = plan.get("steps")
    if not isinstance(steps, list) or not 0 < len(steps) <= MAX_MODEL_CONTINUATION_STEPS:
        raise BrainRunError("model continuation plan steps are outside their bounds")
    if plan.get("strategy") != "fixed_selection_snapshot":
        raise BrainRunError("model continuation plan strategy is invalid")
    if not _valid_digest(plan.get("selection_digest")):
        raise BrainRunError("model continuation selection digest is malformed")
    if (
        not isinstance(plan.get("max_failovers"), int)
        or isinstance(plan.get("max_failovers"), bool)
        or not 0 <= plan["max_failovers"] <= MAX_MODEL_CONTINUATION_FAILOVERS
    ):
        raise BrainRunError("model continuation plan failover budget is invalid")
    if (
        not isinstance(plan.get("omitted_eligible_candidates"), int)
        or isinstance(plan.get("omitted_eligible_candidates"), bool)
        or plan["omitted_eligible_candidates"] < 0
    ):
        raise BrainRunError("model continuation omitted candidate count is invalid")
    if plan.get("retention") != "selection_metadata_only_no_task_prompt_provider_payloads":
        raise BrainRunError("model continuation plan retention contract is invalid")
    if plan.get("secret_material") != "never_returned":
        raise BrainRunError("model continuation plan secret-material contract is invalid")
    seen_model_ids: set[str] = set()
    for index, step in enumerate(steps):
        if not isinstance(step, Mapping):
            raise BrainRunError("model continuation plan step is malformed")
        if set(step.keys()) != {
            "order",
            "provider",
            "model",
            "model_id",
            "candidate_digest",
            "ranking_index",
            "failure_policy",
        } or step.get("order") != index:
            raise BrainRunError("model continuation plan step ordering is invalid")
        provider = _continuation_identifier(
            step.get("provider"), field="continuation step provider", maximum=256
        )
        model = _continuation_identifier(
            step.get("model"), field="continuation step model", maximum=512
        )
        model_id = f"{provider}/{model}"
        if step.get("model_id") != model_id or model_id in seen_model_ids:
            raise BrainRunError("model continuation plan step identity is invalid")
        seen_model_ids.add(model_id)
        if (
            not isinstance(step.get("ranking_index"), int)
            or isinstance(step.get("ranking_index"), bool)
            or step["ranking_index"] < 0
            or not _valid_digest(step.get("candidate_digest"))
        ):
            raise BrainRunError("model continuation plan step metadata is invalid")
        if step.get("failure_policy") != {
            "timeout_with_closed_circuit": "exclude_model",
            "retryable_provider_error": "exclude_provider",
        }:
            raise BrainRunError("model continuation plan failure policy is invalid")


def _validate_model_continuation_state(
    plan: Mapping[str, Any], state: Mapping[str, Any]
) -> None:
    validate_model_continuation_plan(plan)
    if (
        not isinstance(state, Mapping)
        or state.get("schema") != MODEL_CONTINUATION_STATE_SCHEMA
        or state.get("plan_digest") != plan.get("plan_digest")
    ):
        raise BrainRunError("model continuation state is not bound to the supplied plan")
    if set(state.keys()) != {
        "schema",
        "plan_digest",
        "next_step_index",
        "failovers_used",
        "excluded_providers",
        "excluded_models",
        "attempts",
        "status",
        "retention",
        "secret_material",
        "state_digest",
    }:
        raise BrainRunError("model continuation state contains unsupported fields")
    state_digest = state.get("state_digest")
    if not _valid_digest(state_digest):
        raise BrainRunError("model continuation state digest is malformed")
    body = {key: value for key, value in state.items() if key != "state_digest"}
    if _json_digest(body) != state_digest:
        raise BrainRunError("model continuation state digest mismatch")
    failovers = state.get("failovers_used")
    if not isinstance(failovers, int) or isinstance(failovers, bool) or not 0 <= failovers <= plan["max_failovers"]:
        raise BrainRunError("model continuation state failover count is invalid")
    if not isinstance(state.get("attempts"), list) or len(state["attempts"]) > len(plan["steps"]):
        raise BrainRunError("model continuation state attempts are outside their bounds")
    if state.get("status") not in {"ready", "completed", "exhausted"}:
        raise BrainRunError("model continuation state status is invalid")
    next_index = state.get("next_step_index")
    if next_index is not None and (
        not isinstance(next_index, int)
        or isinstance(next_index, bool)
        or not 0 <= next_index < len(plan["steps"])
    ):
        raise BrainRunError("model continuation state next step is invalid")
    if state.get("retention") != "selection_metadata_only_no_task_prompt_provider_payloads":
        raise BrainRunError("model continuation state retention contract is invalid")
    if state.get("secret_material") != "never_returned":
        raise BrainRunError("model continuation state secret-material contract is invalid")
    if not isinstance(state.get("excluded_providers"), list) or not isinstance(state.get("excluded_models"), list):
        raise BrainRunError("model continuation state exclusions are invalid")

    plan_steps = plan["steps"]
    allowed_providers = {step["provider"] for step in plan_steps}
    allowed_models = {step["model_id"] for step in plan_steps}
    for field, values, allowed in (
        ("excluded_providers", state["excluded_providers"], allowed_providers),
        ("excluded_models", state["excluded_models"], allowed_models),
    ):
        if len(values) > len(plan_steps):
            raise BrainRunError(f"model continuation {field} exceed their bounds")
        normalized = [
            _continuation_identifier(value, field=f"model continuation {field}", maximum=768)
            for value in values
        ]
        if len(set(normalized)) != len(normalized) or normalized != sorted(normalized):
            raise BrainRunError(f"model continuation {field} must be sorted and unique")
        if not set(normalized).issubset(allowed):
            raise BrainRunError(f"model continuation {field} references an unknown arm")

    attempts = state["attempts"]
    expected_attempt_keys = {
        "order",
        "provider",
        "model",
        "outcome",
        "failure_scope",
        "failure_code",
        "status_code",
    }
    previous_order = -1
    expected_excluded_providers: set[str] = set()
    expected_excluded_models: set[str] = set()
    failure_count = 0
    success_count = 0
    for attempt in attempts:
        if not isinstance(attempt, Mapping) or set(attempt.keys()) != expected_attempt_keys:
            raise BrainRunError("model continuation state attempt is malformed")
        order = attempt.get("order")
        if (
            not isinstance(order, int)
            or isinstance(order, bool)
            or not 0 <= order < len(plan_steps)
            or order <= previous_order
        ):
            raise BrainRunError("model continuation state attempt ordering is invalid")
        previous_order = order
        step = plan_steps[order]
        if attempt.get("provider") != step["provider"] or attempt.get("model") != step["model"]:
            raise BrainRunError("model continuation state attempt identity is invalid")
        outcome = attempt.get("outcome")
        failure_scope = attempt.get("failure_scope")
        if outcome == "failure":
            if failure_scope not in {"model", "provider"}:
                raise BrainRunError("model continuation state failure scope is invalid")
            failure_count += 1
            if failure_scope == "provider":
                expected_excluded_providers.add(step["provider"])
            else:
                expected_excluded_models.add(step["model_id"])
        elif outcome == "success":
            success_count += 1
            if failure_scope is not None or attempt.get("failure_code") is not None:
                raise BrainRunError("model continuation successful attempt contains failure metadata")
        else:
            raise BrainRunError("model continuation state attempt outcome is invalid")
        failure_code = attempt.get("failure_code")
        if failure_code is not None:
            _continuation_identifier(
                failure_code, field="continuation failure code", maximum=128
            )
        _continuation_status_code(attempt.get("status_code"))
    if failure_count != failovers:
        raise BrainRunError("model continuation state failover count does not match attempts")
    if success_count > 1 or (success_count and attempts[-1]["outcome"] != "success"):
        raise BrainRunError("model continuation state has an invalid terminal attempt")
    if set(state["excluded_providers"]) != expected_excluded_providers or set(state["excluded_models"]) != expected_excluded_models:
        raise BrainRunError("model continuation state exclusions do not match attempts")
    if attempts and attempts[0]["order"] != 0:
        raise BrainRunError("model continuation state must begin with the selected model")
    expected_next_index = next(
        (
            index
            for index, step in enumerate(plan_steps)
            if index > previous_order
            and step["provider"] not in expected_excluded_providers
            and step["model_id"] not in expected_excluded_models
        ),
        None,
    )
    if state["status"] == "ready":
        if success_count or state["next_step_index"] != expected_next_index or expected_next_index is None:
            raise BrainRunError("model continuation ready cursor is inconsistent")
    elif state["status"] == "completed":
        if state["next_step_index"] is not None or success_count != 1:
            raise BrainRunError("model continuation completed cursor is inconsistent")
    elif state["next_step_index"] is not None or expected_next_index is not None or not attempts:
        raise BrainRunError("model continuation exhausted cursor is inconsistent")


def validate_model_continuation_state(
    plan: Mapping[str, Any], state: Mapping[str, Any]
) -> None:
    """Validate a restored continuation cursor before accepting worker progress."""

    _validate_model_continuation_state(plan, state)


def advance_model_continuation_state(
    plan: Mapping[str, Any],
    state: Mapping[str, Any],
    *,
    provider: str,
    model: str,
    failure_scope: str,
    failure_code: str | None = None,
    status_code: int | None = None,
) -> dict[str, Any]:
    _validate_model_continuation_state(plan, state)
    if state.get("status") != "ready" or not isinstance(state.get("next_step_index"), int):
        raise BrainRunError("model continuation is not ready for another failure")
    if state["failovers_used"] >= plan["max_failovers"]:
        raise BrainRunError("model continuation failover budget is exhausted")
    if failure_scope not in {"model", "provider"}:
        raise BrainRunError("model continuation failure scope is invalid")
    current = plan["steps"][state["next_step_index"]]
    if current["provider"] != provider or current["model"] != model:
        raise BrainRunError("model continuation failure does not match the current step")
    excluded_providers = set(state.get("excluded_providers", []))
    excluded_models = set(state.get("excluded_models", []))
    if failure_scope == "provider":
        excluded_providers.add(provider)
    else:
        excluded_models.add(current["model_id"])
    if failure_code is not None:
        _continuation_identifier(failure_code, field="continuation failure code", maximum=128)
    _continuation_status_code(status_code)
    attempts = [*state["attempts"], {
        "order": current["order"],
        "provider": provider,
        "model": model,
        "outcome": "failure",
        "failure_scope": failure_scope,
        "failure_code": failure_code,
        "status_code": status_code,
    }]
    next_index = next(
        (
            index
            for index, step in enumerate(plan["steps"])
            if index > current["order"]
            and step["provider"] not in excluded_providers
            and step["model_id"] not in excluded_models
        ),
        None,
    )
    return _seal_model_continuation_state(
        {
            "schema": MODEL_CONTINUATION_STATE_SCHEMA,
            "plan_digest": plan["plan_digest"],
            "next_step_index": next_index,
            "failovers_used": state["failovers_used"] + 1,
            "excluded_providers": sorted(excluded_providers),
            "excluded_models": sorted(excluded_models),
            "attempts": attempts,
            "status": "exhausted" if next_index is None else "ready",
            "retention": "selection_metadata_only_no_task_prompt_provider_payloads",
            "secret_material": "never_returned",
        }
    )


def complete_model_continuation_state(
    plan: Mapping[str, Any],
    state: Mapping[str, Any],
    *,
    provider: str,
    model: str,
    status_code: int | None = None,
) -> dict[str, Any]:
    _validate_model_continuation_state(plan, state)
    if state.get("status") != "ready" or not isinstance(state.get("next_step_index"), int):
        raise BrainRunError("model continuation is not ready for completion")
    current = plan["steps"][state["next_step_index"]]
    if current["provider"] != provider or current["model"] != model:
        raise BrainRunError("model continuation success does not match the current step")
    _continuation_status_code(status_code)
    body = {key: value for key, value in state.items() if key != "state_digest"}
    body.update(
        {
            "next_step_index": None,
            "attempts": [
                *state["attempts"],
                {
                    "order": current["order"],
                    "provider": provider,
                    "model": model,
                    "outcome": "success",
                    "failure_scope": None,
                    "failure_code": None,
                    "status_code": status_code,
                },
            ],
            "status": "completed",
        }
    )
    return _seal_model_continuation_state(body)


def _emit_model_selection_trace(
    callback: Callable[..., Any] | None,
    *,
    phase: str,
    status: str,
    attempt: int,
    selection: Mapping[str, Any],
    audit: Mapping[str, Any] | None = None,
    selected: Mapping[str, Any] | None = None,
    failure_code: str | None = None,
) -> None:
    """Project one adaptive selection transition without exposing task or provider payloads."""

    if callback is None:
        return
    models = selection.get("models", [])
    candidate_count = len(models) if isinstance(models, Sequence) and not isinstance(models, (str, bytes)) else 0
    eligibility = audit.get("eligibility") if isinstance(audit, Mapping) else None
    eligible_count = eligibility.get("eligible_count") if isinstance(eligibility, Mapping) else None
    if not isinstance(eligible_count, int) or isinstance(eligible_count, bool) or eligible_count < 0:
        eligible_count = None
    selected_provider = selected.get("provider") if isinstance(selected, Mapping) else None
    selected_model = selected.get("model") if isinstance(selected, Mapping) else None
    if not isinstance(selected_provider, str) or not selected_provider.strip():
        selected_provider = None
    if not isinstance(selected_model, str) or not selected_model.strip():
        selected_model = None
    selection_digest = selection.get("decision_digest")
    if not _valid_digest(selection_digest):
        selection_digest = None
    detail = {
        "candidate_count": candidate_count,
        "eligible_candidate_count": eligible_count,
        "selection_status": audit.get("selection_status") if isinstance(audit, Mapping) else None,
        "selection_confidence": selection.get("selection_confidence"),
        "min_selection_confidence": selection.get("min_selection_confidence"),
        "failover": attempt > 1,
        "selection_audit_digest": audit.get("audit_digest") if isinstance(audit, Mapping) else None,
    }
    callback(
        phase=phase,
        status=status,
        provider=selected_provider,
        model=selected_model,
        attempt=attempt,
        selection_digest=selection_digest,
        detail_digest=_json_digest(detail),
        failure_code=failure_code,
    )


def _prepare_fixed_selection_attempt(
    selection: Mapping[str, Any],
    continuation_plan: Mapping[str, Any] | None,
    continuation_state: Mapping[str, Any] | None,
    *,
    attempt: int,
    trace_event_callback: Callable[..., Any] | None,
    scope: str,
) -> tuple[dict[str, Any], Mapping[str, Any], dict[str, Any]]:
    """Project one fixed continuation step without invoking the selector again.

    The first selection is already authoritative for this run.  A fallback is a cursor move
    over the sealed ladder, not a new optimization problem: provider health may be refreshed for
    future runs, but it must not reorder or silently replace the current run's reviewed choice.
    ``selection_override`` is consumed by the low-level provider bridge below and therefore
    keeps the selector tool out of the fallback path entirely.
    """

    if continuation_plan is None:
        projected = dict(selection)
    else:
        if not isinstance(continuation_state, Mapping):
            raise BrainRunError(f"adaptive {scope} continuation state is missing")
        next_index = continuation_state.get("next_step_index")
        steps = continuation_plan.get("steps")
        if not isinstance(next_index, int) or isinstance(next_index, bool) or not isinstance(steps, list):
            raise BrainRunError(f"adaptive {scope} continuation cursor is malformed")
        if not 0 <= next_index < len(steps) or not isinstance(steps[next_index], Mapping):
            raise BrainRunError(f"adaptive {scope} continuation cursor points outside its plan")
        step = steps[next_index]
        provider = step.get("provider")
        model = step.get("model")
        if not isinstance(provider, str) or not isinstance(model, str):
            raise BrainRunError(f"adaptive {scope} continuation step identity is malformed")
        projected = dict(selection)
        projected["selected_model"] = {"provider": provider, "model": model}
        projected["selection_status"] = "selected"

    selected = projected.get("selected_model")
    if not isinstance(selected, Mapping):
        raise BrainRunError(f"adaptive {scope} selection has no eligible provider")
    provider = selected.get("provider")
    model = selected.get("model")
    if not isinstance(provider, str) or not provider or not isinstance(model, str) or not model:
        raise BrainRunError(f"adaptive {scope} selection returned malformed provider metadata")
    audit = build_model_selection_audit(projected)
    projected["selection_audit"] = audit
    _emit_model_selection_trace(
        trace_event_callback,
        phase="model_selection_started",
        status="running",
        attempt=attempt,
        selection=projected,
    )
    _emit_model_selection_trace(
        trace_event_callback,
        phase="model_selection_finished",
        status="completed",
        attempt=attempt,
        selection=projected,
        audit=audit,
        selected=selected,
    )
    return projected, selected, audit


def _routing_health_evidence(subject: str, health: Mapping[str, Any]) -> dict[str, Any] | None:
    """Validate and project bounded transport evidence for one routing subject.

    Historical health is preferred when present because it can represent more attempts than the
    current process. A small confidence cap below prevents a short outage or a tiny sample from
    completely overriding the caller's model prior.
    """

    historical = health.get("historical")
    source: Mapping[str, Any] = historical if isinstance(historical, Mapping) else health
    attempts = source.get("attempts", 0)
    if not isinstance(attempts, int) or isinstance(attempts, bool) or attempts < 0:
        raise BrainRunError(f"routing health attempts are invalid for {subject!r}")
    if attempts == 0:
        return None
    success_rate = source.get("success_rate")
    if (
        isinstance(success_rate, bool)
        or not isinstance(success_rate, (int, float))
        or not math.isfinite(float(success_rate))
        or not 0.0 <= float(success_rate) <= 1.0
    ):
        raise BrainRunError(f"routing health success_rate is invalid for {subject!r}")
    latency = source.get("last_latency_ms")
    if (
        isinstance(latency, bool)
        or not isinstance(latency, (int, float))
        or not math.isfinite(float(latency))
        or float(latency) < 0.0
    ):
        raise BrainRunError(f"routing health last_latency_ms is invalid for {subject!r}")
    return {
        "attempts": attempts,
        "success_rate": float(success_rate),
        "last_latency_ms": float(latency),
        "confidence": min(0.75, attempts / 12.0),
    }


def _provider_health_evidence(provider: str, health: Mapping[str, Any]) -> dict[str, Any] | None:
    """Compatibility wrapper for provider-level routing evidence."""

    return _routing_health_evidence(provider, health)


def _refresh_failover_provider_health(
    runtime: LLMRuntime,
    attempt_selection: dict[str, Any],
    *,
    provider: str,
    error: ProviderError,
    failed_providers: set[str],
) -> dict[str, Any]:
    """Refresh the live provider gate after a failed arm before the next selection.

    A model-level refusal only disables that arm. Once the runtime reports an open circuit (or
    the provider error explicitly carries ``circuit_open``), every remaining arm for that
    provider is disabled for this bounded failover sequence. The returned receipt is scalar
    metadata only and never contains provider payloads or credential material.
    """

    raw_health = attempt_selection.get("provider_health", {})
    if not isinstance(raw_health, Mapping):
        raise BrainRunError("adaptive failover provider health must be a mapping")
    provider_health = {
        key: dict(value)
        for key, value in raw_health.items()
        if isinstance(key, str) and isinstance(value, Mapping)
    }
    current = dict(provider_health.get(provider, {}))
    try:
        status = runtime.provider_status(provider)
    except ProviderError:
        status = {}
    for field in (
        "circuit",
        "consecutive_failures",
        "opened_until",
        "attempts",
        "successes",
        "failures",
        "success_rate",
        "mean_latency_ms",
        "last_latency_ms",
        "last_model",
        "last_outcome",
        "observed_at",
    ):
        if field in status:
            current[field] = status[field]
    circuit = current.get("circuit", "closed")
    provider_circuit_open = error.circuit_open or circuit == "open"
    if provider_circuit_open:
        circuit = "open"
        current["circuit"] = "open"
        current["eligible"] = False
        failed_providers.add(provider)
    provider_health[provider] = current
    attempt_selection["provider_health"] = provider_health
    return {
        "provider_circuit_after_failure": circuit,
        "provider_consecutive_failures": current.get("consecutive_failures", 0),
        "provider_health_attempts": current.get("attempts", 0),
        "provider_health_success_rate": current.get("success_rate", 0.0),
        "provider_health_gate": "closed" if not provider_circuit_open else "provider_disabled",
    }


def _mission_wire_output(value: Any) -> Any:
    """Extract structured tool output while dropping opaque wire envelopes."""

    if not isinstance(value, Mapping):
        return None
    result = value.get("result")
    if not isinstance(result, Mapping):
        return None
    structured = result.get("structuredContent")
    if structured is not None:
        return structured
    content = result.get("content")
    if isinstance(content, list) and content:
        first = content[0]
        if isinstance(first, Mapping) and isinstance(first.get("text"), str):
            try:
                return json.loads(first["text"])
            except (TypeError, ValueError):
                return None
    return None


def _bounded_mission_report_projection(
    report: Mapping[str, Any],
    *,
    include_outputs: bool,
) -> dict[str, Any]:
    """Project an agent_mission report for continuation without replaying opaque envelopes."""

    if not isinstance(report, Mapping):
        raise BrainRunError("agent_mission returned a non-object report")
    if report.get("workflow") not in (None, "agent_mission"):
        raise BrainRunError("agent_mission returned the wrong workflow")
    projection: dict[str, Any] = {
        "workflow": "agent_mission",
        "ok": report.get("ok", True),
        "execution": report.get("execution", "unknown"),
        "mission_status": report.get("mission_status", "unknown"),
        "dispatch": report.get("dispatch", "unknown"),
        "preflight": report.get("preflight", False),
        "plan_digest": None,
        "succeeded": report.get("succeeded", 0),
        "refused": report.get("refused", 0),
        "blocked": report.get("blocked", 0),
        "cancelled": report.get("cancelled", 0),
        "required_failures": report.get("required_failures", 0),
        "returned_bytes": report.get("returned_bytes", 0),
        "results": [],
        "result_digest": _json_digest(dict(report)),
        "retention": "structured_step_outputs_only",
    }
    plan = report.get("plan")
    if isinstance(plan, Mapping):
        projection["plan_digest"] = plan.get("digest") or plan.get("plan_digest")
        projection["mission_id"] = plan.get("mission_id")
    raw_results = report.get("results", [])
    if isinstance(raw_results, list):
        for raw in raw_results[:MAX_MISSION_AUTHORIZATION_CALLS]:
            if not isinstance(raw, Mapping):
                continue
            row: dict[str, Any] = {
                "id": raw.get("id"),
                "tool": raw.get("tool"),
                "status": raw.get("status"),
                "required": raw.get("required"),
                "arguments_digest": raw.get("arguments_digest"),
                "bytes": raw.get("bytes", 0),
            }
            if raw.get("error") is not None:
                row["error_digest"] = _json_digest({"error": raw.get("error")})
            if include_outputs:
                output = _mission_wire_output(raw.get("wire"))
                if output is not None:
                    encoded_output = json.dumps(
                        output,
                        ensure_ascii=False,
                        sort_keys=True,
                        separators=(",", ":"),
                        allow_nan=False,
                    ).encode("utf-8")
                    if len(encoded_output) <= MAX_MISSION_AUTHORIZATION_STEP_OUTPUT_BYTES:
                        row["output"] = output
                    else:
                        row["output_digest"] = hashlib.sha256(encoded_output).hexdigest()
                elif raw.get("wire") is not None:
                    row["output_digest"] = _json_digest(raw.get("wire"))
            projection["results"].append(row)
    BrainLearningLedger._assert_safe(projection)
    encoded = json.dumps(
        projection,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    if len(encoded) > MAX_MISSION_AUTHORIZATION_RESULT_BYTES:
        raise BrainRunError("agent_mission continuation result exceeds the bounded size")
    return projection


@dataclass(frozen=True, slots=True)
class MissionAuthorizationReceipt:
    """One caller-owned tool-intent authorization attempt and its bounded evidence."""

    mission_id: str
    call_ids: tuple[str, ...]
    status: str
    preflight: Mapping[str, Any]
    execution: Mapping[str, Any] | None
    result: Mapping[str, Any]

    def __post_init__(self) -> None:
        if self.status not in {
            "preflight_refused",
            "approval_required",
            "executed",
            "execution_refused",
            "execution_failed",
        }:
            raise BrainRunError("mission authorization receipt has an invalid status")
        if not self.mission_id or not self.call_ids:
            raise BrainRunError("mission authorization receipt is missing identity")

    def to_dict(self) -> dict[str, Any]:
        return {
            "mission_id": self.mission_id,
            "call_ids": list(self.call_ids),
            "status": self.status,
            "preflight": dict(self.preflight),
            "execution": None if self.execution is None else dict(self.execution),
            "result": dict(self.result),
            "authorization": "caller_owned",
        }


class MissionToolAuthorizer:
    """Route, preflight, and optionally dispatch native provider tool intents.

    The object is intentionally a callable so it can be passed directly to
    :meth:`LLMRuntime.invoke_tool_loop` or :meth:`AutonomousBrain.run_tool_loop`. It never treats
    a route recommendation as permission: every call must pass the caller policy, the route
    candidate set, the local route schema when available, and the authoritative ``agent_mission``
    preflight. Dispatch remains disabled unless ``approve_mission_dispatch`` is true.
    """

    def __init__(
        self,
        workspace: BrainWorkspace,
        *,
        task: str,
        mission_policy: MissionPolicy | Mapping[str, Any],
        route: Mapping[str, Any] | None = None,
        approve_mission_dispatch: bool = False,
        mission_id_prefix: str = "brain-tool",
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
        operations_gate_acceptance: Mapping[str, Any] | None = None,
    ) -> None:
        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("mission authorizer task must be non-empty")
        if not hasattr(workspace, "tool") or not callable(getattr(workspace, "tool")):
            raise BrainRunError("mission authorizer requires a workspace tool boundary")
        if not isinstance(mission_policy, (MissionPolicy, Mapping)):
            raise BrainRunError("mission authorizer policy must be a MissionPolicy or mapping")
        normalized_policy = (
            mission_policy.to_dict()
            if isinstance(mission_policy, MissionPolicy)
            else dict(mission_policy)
        )
        allowed = normalized_policy.get("allowed_tools")
        if not isinstance(allowed, Sequence) or isinstance(allowed, (str, bytes)) or not allowed:
            raise BrainRunError("mission authorizer requires an explicit allowed_tools policy")
        if any(not _mission_tool_identifier(tool) for tool in allowed):
            raise BrainRunError("mission authorizer policy contains an unsafe tool name")
        BrainLearningLedger._assert_safe(normalized_policy)
        if not isinstance(approve_mission_dispatch, bool):
            raise BrainRunError("approve_mission_dispatch must be a boolean")
        if not isinstance(claim_requests, Sequence) or isinstance(claim_requests, (str, bytes)):
            raise BrainRunError("mission authorizer claim_requests must be a sequence")
        if any(not isinstance(value, Mapping) for value in claim_requests):
            raise BrainRunError("mission authorizer claim_requests must contain mappings")
        if evaluator_review is not None and not isinstance(evaluator_review, Mapping):
            raise BrainRunError("mission authorizer evaluator_review must be a mapping")
        if workflow_binding is not None and not isinstance(workflow_binding, Mapping):
            raise BrainRunError("mission authorizer workflow_binding must be a mapping")
        if operations_gate_acceptance is not None and not isinstance(operations_gate_acceptance, Mapping):
            raise BrainRunError("mission authorizer operations_gate_acceptance must be a mapping")
        BrainLearningLedger._assert_safe(
            {
                "claim_requests": list(claim_requests),
                "evaluator_review": evaluator_review,
                "workflow_binding": workflow_binding,
                "operations_gate_acceptance": operations_gate_acceptance,
            }
        )
        self.workspace = workspace
        self.task = task
        self.policy = normalized_policy
        self.policy["execute"] = False
        self.route = None if route is None else dict(route)
        self.approve_mission_dispatch = approve_mission_dispatch
        self.mission_id_prefix = mission_id_prefix
        self.claim_requests = tuple(dict(value) for value in claim_requests)
        self.evaluator_review = None if evaluator_review is None else dict(evaluator_review)
        self.workflow_binding = None if workflow_binding is None else dict(workflow_binding)
        self.operations_gate_acceptance = (
            None if operations_gate_acceptance is None else dict(operations_gate_acceptance)
        )
        self._receipts: list[MissionAuthorizationReceipt] = []
        self._invocation = 0
        self._route_recommended: set[str] | None = None
        self._route_candidates: dict[str, tuple[str, ...]] = {}
        self._route_metadata: dict[str, tuple[str, str, str]] = {}
        self._schema_catalogue: ToolCatalogue | None = None
        if self.route is not None:
            self._configure_route(self.route)

    @property
    def receipts(self) -> tuple[MissionAuthorizationReceipt, ...]:
        return tuple(self._receipts)

    def __call__(
        self,
        calls: tuple[ProviderToolCall, ...],
    ) -> tuple[ProviderToolResult, ...]:
        if not isinstance(calls, tuple) or not calls or len(calls) > MAX_MISSION_AUTHORIZATION_CALLS:
            raise BrainRunError("mission authorizer received an invalid tool-call batch")
        if any(not isinstance(call, ProviderToolCall) for call in calls):
            raise BrainRunError("mission authorizer received malformed tool calls")
        self._invocation += 1
        call_ids = tuple(call.call_id for call in calls)
        if len(set(call_ids)) != len(call_ids):
            return self._refuse(calls, "duplicate provider tool call ids")
        validation_error = self._validate_calls(calls)
        if validation_error is not None:
            return self._refuse(calls, validation_error)
        mission_id = self._mission_id(calls)
        steps = []
        for index, call in enumerate(calls):
            domain, capability, objective = self._route_metadata.get(
                call.name,
                ("cross_domain", call.name, f"Execute caller-authorized tool intent {call.name}"),
            )
            steps.append(
                {
                    "id": f"provider-tool-{self._invocation}-{index}",
                    "domain": domain,
                    "capability": capability,
                    "objective": objective,
                    "tool": call.name,
                    "arguments": dict(call.arguments),
                    "required": True,
                    "depends_on": [],
                    "bindings": [],
                }
            )
        request = MissionRequest(
            mission_id=mission_id,
            goal=self.task,
            steps=steps,
            policy=dict(self.policy),
            claim_requests=self.claim_requests,
            evaluator_review=self.evaluator_review,
            workflow_binding=self.workflow_binding,
            operations_gate_acceptance=self.operations_gate_acceptance,
        )
        try:
            preflight_raw = self.workspace.tool("agent_mission", request.to_mcp_arguments())
            preflight = _bounded_mission_report_projection(preflight_raw, include_outputs=False)
        except Exception:
            return self._refuse(calls, "mission preflight transport or validation failed", mission_id=mission_id)
        if not self._preflight_ready(preflight_raw):
            self._record_receipt(
                mission_id,
                calls,
                "preflight_refused",
                preflight,
                None,
                preflight,
            )
            return tuple(
                ProviderToolResult(call.call_id, preflight, approved=False, is_error=True)
                for call in calls
            )
        if not self.approve_mission_dispatch:
            self._record_receipt(
                mission_id,
                calls,
                "approval_required",
                preflight,
                None,
                preflight,
            )
            return tuple(
                ProviderToolResult(call.call_id, preflight, approved=False, is_error=True)
                for call in calls
            )
        execute_policy = dict(self.policy)
        execute_policy["execute"] = True
        execute_request = MissionRequest(
            mission_id=mission_id,
            goal=self.task,
            steps=steps,
            policy=execute_policy,
            claim_requests=self.claim_requests,
            evaluator_review=self.evaluator_review,
            workflow_binding=self.workflow_binding,
            operations_gate_acceptance=self.operations_gate_acceptance,
        )
        try:
            execution_raw = self.workspace.tool("agent_mission", execute_request.to_mcp_arguments())
            execution = _bounded_mission_report_projection(execution_raw, include_outputs=True)
        except Exception:
            execution = {
                "workflow": "agent_mission",
                "ok": False,
                "execution": "refused",
                "mission_status": "failed",
                "result_digest": _json_digest({"mission_id": mission_id, "status": "transport_failed"}),
                "retention": "structured_step_outputs_only",
            }
            self._record_receipt(
                mission_id,
                calls,
                "execution_failed",
                preflight,
                execution,
                execution,
            )
            return tuple(
                ProviderToolResult(call.call_id, execution, approved=False, is_error=True)
                for call in calls
            )
        mission_status = execution.get("mission_status")
        status = "executed" if mission_status == "succeeded" else (
            "execution_refused" if mission_status in {"refused", "blocked", "cancelled"} else "execution_failed"
        )
        self._record_receipt(mission_id, calls, status, preflight, execution, execution)
        return tuple(
            ProviderToolResult(
                call.call_id,
                execution,
                approved=True,
                is_error=status != "executed",
            )
            for call in calls
        )

    def _configure_route(self, route: Mapping[str, Any]) -> None:
        if route.get("workflow") != "capability_route":
            raise BrainRunError("mission authorizer route must be a capability_route report")
        if route.get("goal") != self.task:
            raise BrainRunError("mission authorizer route goal must match the task")
        unresolved = route.get("unresolved_needs", [])
        if not isinstance(unresolved, list) or unresolved:
            raise BrainRunError("mission authorizer route contains unresolved needs")
        recommended = route.get("recommended_tools")
        needs = route.get("needs")
        if not isinstance(recommended, list) or any(not isinstance(tool, str) for tool in recommended):
            raise BrainRunError("mission authorizer route has malformed recommended_tools")
        if not isinstance(needs, list) or any(not isinstance(need, Mapping) for need in needs):
            raise BrainRunError("mission authorizer route has malformed needs")
        self._route_recommended = set(recommended)
        for need in needs:
            need_id = need.get("id")
            candidate_tools = need.get("candidate_tools", [])
            if not isinstance(need_id, str) or not isinstance(candidate_tools, list):
                raise BrainRunError("mission authorizer route need is malformed")
            domains = need.get("candidate_domains", [])
            groups = need.get("candidate_groups", [])
            domain = domains[0] if isinstance(domains, list) and domains and isinstance(domains[0], str) else "cross_domain"
            capability = groups[0] if isinstance(groups, list) and groups and isinstance(groups[0], str) else need_id
            objective = need.get("query") if isinstance(need.get("query"), str) else f"Resolve routed need {need_id}"
            self._route_metadata.update(
                {tool: (domain, capability, objective) for tool in candidate_tools if isinstance(tool, str)}
            )
        raw_schemas = route.get("tool_schemas", [])
        omitted = route.get("tool_schemas_omitted", 0)
        if isinstance(raw_schemas, list) and raw_schemas and omitted == 0:
            try:
                self._schema_catalogue = ToolCatalogue.from_definitions(raw_schemas)
            except (ArgumentError, ToolSchemaError, TypeError, ValueError) as error:
                raise BrainRunError("mission authorizer route schemas are invalid") from error

    def _validate_calls(self, calls: Sequence[ProviderToolCall]) -> str | None:
        allowed = set(self.policy["allowed_tools"])
        for call in calls:
            if not _mission_tool_identifier(call.name):
                return f"tool {call.name!r} is not an executable mission tool identifier"
            if call.name not in allowed:
                return f"tool {call.name!r} is not in the caller mission policy"
            if self._route_recommended is not None and call.name not in self._route_recommended:
                return f"tool {call.name!r} is not recommended by the live route"
            if self._route_recommended is not None and call.name not in self._route_metadata:
                return f"tool {call.name!r} is not attached to a resolved route need"
            if self._schema_catalogue is not None:
                try:
                    report = self._schema_catalogue.validate(call.name, call.arguments)
                except ToolSchemaError:
                    return f"tool {call.name!r} is absent from the retained route schema set"
                if not report.ok:
                    return f"tool {call.name!r} failed route schema preflight"
        return None

    def _mission_id(self, calls: Sequence[ProviderToolCall]) -> str:
        digest = _json_digest([call.to_dict() for call in calls])[:32]
        prefix = self.mission_id_prefix if _mission_tool_identifier(self.mission_id_prefix) else "brain_tool"
        return f"{prefix}-{self._invocation}-{digest}"

    def _preflight_ready(self, report: Mapping[str, Any]) -> bool:
        if not isinstance(report, Mapping) or report.get("ok") is False:
            return False
        if report.get("workflow") not in (None, "agent_mission"):
            return False
        if report.get("dispatch") in {"executed", "started"}:
            return False
        return report.get("mission_status") in {None, "planned", "succeeded"} or report.get("execution") == "planned"

    def _record_receipt(
        self,
        mission_id: str,
        calls: Sequence[ProviderToolCall],
        status: str,
        preflight: Mapping[str, Any],
        execution: Mapping[str, Any] | None,
        result: Mapping[str, Any],
    ) -> None:
        self._receipts.append(
            MissionAuthorizationReceipt(
                mission_id=mission_id,
                call_ids=tuple(call.call_id for call in calls),
                status=status,
                preflight=preflight,
                execution=execution,
                result=result,
            )
        )

    def _refuse(
        self,
        calls: Sequence[ProviderToolCall],
        reason: str,
        *,
        mission_id: str | None = None,
    ) -> tuple[ProviderToolResult, ...]:
        projection = {
            "workflow": "agent_mission",
            "ok": False,
            "execution": "not_started",
            "mission_status": "refused",
            "refusal": reason,
            "result_digest": _json_digest({"reason": reason}),
            "retention": "structured_step_outputs_only",
        }
        self._receipts.append(
            MissionAuthorizationReceipt(
                mission_id=mission_id or f"{self.mission_id_prefix}-refused-{self._invocation}",
                call_ids=tuple(call.call_id for call in calls),
                status="preflight_refused",
                preflight=projection,
                execution=None,
                result=projection,
            )
        )
        return tuple(
            ProviderToolResult(call.call_id, projection, approved=False, is_error=True)
            for call in calls
        )


def _compose_provider_observers(
    policy_observer: ProviderInvocationObserver | None,
    external_observer: ProviderInvocationObserver | None,
) -> ProviderInvocationObserver | None:
    """Keep execution admission and caller telemetry attached to the same provider turn."""

    observers = tuple(observer for observer in (policy_observer, external_observer) if observer is not None)
    if not observers:
        return None
    if len(observers) == 1:
        return observers[0]
    return CompositeProviderInvocationObserver(observers)


class AutonomousBrain:
    """Coordinate the value-only Rust kernel with a real caller-approved provider invocation."""

    def __init__(
        self,
        workspace: BrainWorkspace,
        runtime: LLMRuntime,
        memory: BrainEpisodicMemory | None = None,
    ) -> None:
        self.workspace = workspace
        self.runtime = runtime
        if memory is not None and not isinstance(memory, BrainEpisodicMemory):
            raise BrainRunError("memory must be a BrainEpisodicMemory or None")
        self.memory = memory

    def prepare_autonomous(self, **kwargs: Any) -> Any:
        """Build a domain-aware task blueprint without contacting a provider.

        The import is local to keep the low-level brain kernel independent from the convenience
        orchestration layer. The returned blueprint contains only transient task material and
        value-only public metadata; credentials are never accepted by this preparation method.
        """

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).prepare(**kwargs)

    def prepare_cross_domain(self, **kwargs: Any) -> Any:
        """Build bounded fan-out/fan-in domain work without contacting a provider."""

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).prepare_cross_domain(**kwargs)

    def domain_operating_kit(self, domain: str) -> Any:
        """Return one provider-free, digest-bound operating contract for a built-in domain."""

        from .autonomous_domain_operating_kit import build_autonomous_domain_operating_kit

        return build_autonomous_domain_operating_kit(domain)

    def domain_operating_kits(self, domains: Sequence[str] | None = None) -> tuple[Any, ...]:
        """Return deterministic operating contracts for the requested built-in domains."""

        from .autonomous_domain_operating_kit import build_autonomous_domain_operating_kits

        return build_autonomous_domain_operating_kits(domains)

    def validate_domain_operating_kit(self, value: Mapping[str, Any] | Any) -> Any:
        """Rebuild and validate caller-held domain metadata against current reviewed contracts."""

        from .autonomous_domain_operating_kit import validate_autonomous_domain_operating_kit

        return validate_autonomous_domain_operating_kit(value)

    def select_execution_policy(
        self,
        *,
        task: str,
        candidates: Sequence[Mapping[str, Any] | Any],
        domain: str | None = None,
        hints: Sequence[str] = (),
        allow_cross_domain: bool = True,
        policy: Any | None = None,
        required_capabilities: Sequence[str] = (),
        preferred_capabilities: Sequence[str] = (),
        required_path: str | None = None,
        evidence_required: bool | None = None,
        structured_output_required: bool | None = None,
        effects_requested: bool = False,
        effects_approved: bool = False,
        approval_granted: bool = False,
        max_cost_units: float | None = None,
        max_latency_ms: float | None = None,
        max_risk: float | None = None,
        min_score: float | None = None,
    ) -> dict[str, Any]:
        """Choose a joint execution arm after route admission, without dispatching anything.

        The returned route and policy decision contain only digests and bounded candidate
        metadata. The caller keeps the policy instance, executes the selected arm through its
        existing approval boundary, and later settles explicit evaluator credit with that same
        policy instance. Provider success is never inferred as reward.
        """

        from .autonomous_execution_policy import AutonomousExecutionPolicy
        from .autonomy import AutonomousTaskOrchestrator

        orchestrator = AutonomousTaskOrchestrator(self)
        route = orchestrator.route_task(task=task, hints=tuple(hints) + ((domain,) if domain is not None else ()), allow_cross_domain=False if domain is not None else allow_cross_domain)
        if route.abstained or not route.selected_domains:
            raise BrainRunError("execution policy selection requires an admitted route")
        if domain is not None and route.primary_domain != domain:
            raise BrainRunError("execution policy explicit domain did not win deterministic route admission")
        selected_policy = policy if isinstance(policy, AutonomousExecutionPolicy) else AutonomousExecutionPolicy()
        domain_policy = orchestrator.domain_policy(route.primary_domain or route.selected_domains[0])
        decision = selected_policy.select(
            {
                "context_digest": route.task_digest,
                "requested_domains": list(route.selected_domains),
                "required_capabilities": list(required_capabilities),
                "preferred_capabilities": list(preferred_capabilities),
                "required_path": required_path,
                "evidence_required": domain_policy.evidence_mode == "required_before_provider" if evidence_required is None else evidence_required,
                "structured_output_required": domain_policy.response_mode == "structured_required" if structured_output_required is None else structured_output_required,
                "effects_requested": effects_requested,
                "effects_approved": effects_approved,
                "approval_granted": approval_granted,
                "max_cost_units": domain_policy.max_total_cost_units if max_cost_units is None else max_cost_units,
                "max_latency_ms": 86_400_000 if max_latency_ms is None else max_latency_ms,
                "max_risk": 1.0 if max_risk is None else max_risk,
                "min_score": 0.0 if min_score is None else min_score,
            },
            candidates,
        )
        descriptor = {"schema": "bioprism-python-autonomous-brain-execution-policy/0.1", "route_digest": route.route_digest, "decision_digest": decision.decision_digest}
        return {
            "schema": "bioprism-python-autonomous-brain-execution-policy/0.1",
            "route": route.to_dict(),
            "decision": decision.to_dict(),
            "policy_plan_digest": _json_digest(descriptor),
            "retention": "route_and_policy_metadata_only;task_prompt_response_tool_and_credential_values_not_retained",
            "secret_material": "never_returned",
        }

    def run_autonomous(self, **kwargs: Any) -> Any:
        """Run a domain-aware task through adaptive selection and bounded provider execution.

        Use ``learn=True`` to require explicit evaluator evidence, update caller-owned bandit
        state, and append a metadata-only episodic record. Provider and mission approval flags are
        deliberately forwarded unchanged; this convenience method does not widen authority.
        """

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).run(**kwargs)

    def run_workflow(self, **kwargs: Any) -> Any:
        """Execute a prepared domain workflow as a resumable stage dependency graph.

        Stage outputs are structured and checkpointable; approval, malformed evidence, and
        model-declared uncertainty stop the graph without replaying completed stages.
        """

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).run_workflow(**kwargs)

    def run_workflow_learning(self, **kwargs: Any) -> Any:
        """Execute workflow stages and apply explicit per-stage evaluator updates."""

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).run_workflow_learning(**kwargs)

    def run_workflow_cycle(self, **kwargs: Any) -> Any:
        """Run an explicit bounded evaluator-guided workflow recovery cycle."""

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).run_workflow_cycle(**kwargs)

    def run_workflow_trajectory_learning(self, **kwargs: Any) -> Any:
        """Execute workflow stages and assign delayed discounted credit across the trajectory."""

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).run_workflow_trajectory_learning(**kwargs)

    def run_cross_domain(self, **kwargs: Any) -> Any:
        """Run bounded domain specialists and an optional cross-domain synthesis."""

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).run_cross_domain(**kwargs)

    def run_cross_domain_learning(self, **kwargs: Any) -> Any:
        """Run cross-domain specialists with sequential evaluator and bandit updates."""

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).run_cross_domain_learning(**kwargs)

    def run_cross_domain_trajectory_learning(self, **kwargs: Any) -> Any:
        """Run cross-domain specialists and synthesis with delayed trajectory credit."""

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).run_cross_domain_trajectory_learning(**kwargs)

    def run_cross_domain_replan_learning(self, **kwargs: Any) -> Any:
        """Run bounded cross-domain replan attempts with evaluator-guided delayed credit."""

        from .autonomy import AutonomousTaskOrchestrator

        return AutonomousTaskOrchestrator(self).run_cross_domain_replan_learning(**kwargs)

    def recall_memory(
        self,
        query: MemoryQuery | Mapping[str, Any] | None = None,
        *,
        limit: int | None = None,
        memory: BrainEpisodicMemory | None = None,
    ) -> list[dict[str, Any]]:
        """Recall bounded metadata/lessons from the configured episodic memory."""

        store = memory if memory is not None else self.memory
        if store is None:
            raise BrainRunError("episodic memory is not configured")
        if not isinstance(store, BrainEpisodicMemory):
            raise BrainRunError("memory must be a BrainEpisodicMemory")
        try:
            return store.retrieve(query, limit=limit)
        except BrainMemoryError as error:
            raise BrainRunError("episodic memory retrieval failed") from error

    @staticmethod
    def _result_memory_kind(
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
    ) -> tuple[BrainRunResult, str, str]:
        if isinstance(result, BrainRunResult):
            return result, "run", result.status
        if isinstance(result, BrainToolLoopResult):
            return result.brain_run, "tool_loop", result.status
        if isinstance(result, BrainMissionResult):
            return result.brain_run, "mission", result.status
        raise BrainRunError("result must be a BrainRunResult, BrainToolLoopResult, or BrainMissionResult")

    def remember_result(
        self,
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
        *,
        task: str,
        episode_id: str | None = None,
        context: Mapping[str, Any] | None = None,
        tags: Sequence[str] = (),
        lesson: str | None = None,
        provenance: Mapping[str, Any] | None = None,
        memory: BrainEpisodicMemory | None = None,
    ) -> dict[str, Any]:
        """Persist one run as metadata-only episodic memory.

        The task is immediately reduced to a digest.  The provider response, prompt, tool
        arguments, and credentials are never passed to the memory store.
        """

        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("task must be a non-empty string")
        if context is not None and not isinstance(context, Mapping):
            raise BrainRunError("memory context must be a mapping or None")
        if not isinstance(tags, Sequence) or isinstance(tags, (str, bytes)):
            raise BrainRunError("memory tags must be a string sequence")
        if any(not isinstance(tag, str) or not tag.strip() for tag in tags):
            raise BrainRunError("memory tags must contain non-empty strings")
        if provenance is not None and not isinstance(provenance, Mapping):
            raise BrainRunError("memory provenance must be a mapping or None")
        store = memory if memory is not None else self.memory
        if store is None:
            raise BrainRunError("episodic memory is not configured")
        if not isinstance(store, BrainEpisodicMemory):
            raise BrainRunError("memory must be a BrainEpisodicMemory")
        brain_result, result_kind, status = self._result_memory_kind(result)
        selected = brain_result.selection.get("selected_model")
        if not isinstance(selected, Mapping):
            raise BrainRunError("cannot remember a result without selected model metadata")
        selected_model = {
            "provider": selected.get("provider"),
            "model": selected.get("model"),
        }
        if not isinstance(selected_model["provider"], str) or not isinstance(selected_model["model"], str):
            raise BrainRunError("selected model metadata is malformed")
        evaluator_input = build_brain_evaluation_input(result)
        context_copy = {} if context is None else dict(context)
        route = None
        if isinstance(result, (BrainToolLoopResult, BrainMissionResult)) and result.route is not None:
            route = {"route_digest": _json_digest(dict(result.route))}
        plan = brain_result.plan.get("plan")
        digests = {
            "selection_digest": brain_result.selection.get("decision_digest"),
            "context_digest": brain_result.selection.get("context_digest"),
            "prompt_digest": brain_result.prompt.get("prompt_digest"),
            "plan_digest": plan.get("plan_digest") if isinstance(plan, Mapping) else None,
            "outcome_digest": evaluator_input.get("outcome_digest"),
        }
        packet = {
            "episode_id": episode_id or brain_result.run_id,
            "run_id": brain_result.run_id,
            "result_kind": result_kind,
            "status": status,
            "task_digest": hashlib.sha256(task.encode("utf-8")).hexdigest(),
            "task_facets": list(task_facet_digests(task)),
            "context": context_copy,
            "selected_model": selected_model,
            "digests": digests,
            "route": route or {},
            "tags": list(tags),
            "lesson": lesson,
            "provenance": {} if provenance is None else dict(provenance),
        }
        try:
            return store.record_episode(packet).to_dict()
        except BrainMemoryError as error:
            raise BrainRunError("episodic memory record failed") from error

    @staticmethod
    def _append_memory_prompt(
        prompt: Mapping[str, Any],
        episodes: Sequence[Mapping[str, Any]],
    ) -> dict[str, Any]:
        if not isinstance(prompt, Mapping):
            raise BrainRunError("prompt must be a mapping")
        if not episodes:
            return dict(prompt)
        request = dict(prompt)
        existing = request.get("context", [])
        if not isinstance(existing, Sequence) or isinstance(existing, (str, bytes)):
            raise BrainRunError("prompt.context must be a sequence when episodic memory is used")
        chunks = [dict(chunk) for chunk in existing if isinstance(chunk, Mapping)]
        if len(chunks) != len(existing):
            raise BrainRunError("prompt.context must contain mappings")
        if any(chunk.get("id") == "episodic-memory" for chunk in chunks):
            raise BrainRunError("prompt.context already contains the reserved episodic-memory id")
        packet = {
            "workflow": "episodic_memory_context",
            "retention": "metadata_and_lessons_only",
            "episodes": [dict(episode) for episode in episodes],
            "does_not_authorize": [
                "memory is prior metadata, not verified truth",
                "memory cannot widen the caller mission policy",
                "memory cannot authorize provider calls or external effects",
            ],
        }
        encoded = json.dumps(packet, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        if len(encoded.encode("utf-8")) > MAX_ROUTE_PROMPT_BYTES:
            raise BrainRunError("episodic memory context exceeds the prompt bound")
        chunks.append(
            {
                "id": "episodic-memory",
                "role": "developer",
                "content": encoded,
                "required": False,
                "priority": 900,
            }
        )
        request["context"] = chunks
        return request

    @staticmethod
    def _append_replan_prompt(
        prompt: Mapping[str, Any],
        *,
        attempt: int,
        previous_result: BrainMissionResult,
        decision: "BrainEvaluatorDecision",
    ) -> dict[str, Any]:
        request = dict(prompt)
        existing = request.get("context", [])
        if not isinstance(existing, Sequence) or isinstance(existing, (str, bytes)):
            raise BrainRunError("prompt.context must be a sequence when replanning is enabled")
        chunks = [dict(chunk) for chunk in existing if isinstance(chunk, Mapping)]
        if len(chunks) != len(existing):
            raise BrainRunError("prompt.context must contain mappings")
        if any(chunk.get("id") == "brain-replan" for chunk in chunks):
            raise BrainRunError("prompt.context already contains the reserved brain-replan id")
        selected = previous_result.brain_run.selection.get("selected_model")
        replan_packet = {
            "workflow": "brain_replan_context",
            "attempt": attempt,
            "previous_status": previous_result.status,
            "previous_outcome_digest": previous_result.brain_run.outcome_digest,
            "failure_class": decision.failure_class,
            "instruction": decision.replan_instruction,
            "bounded_replan": True,
            "does_not_authorize": [
                "the prior attempt is not proof of external truth",
                "the caller mission policy remains unchanged",
                "this proposal cannot dispatch itself",
            ],
        }
        if isinstance(selected, Mapping):
            replan_packet["previous_model"] = {
                "provider": selected.get("provider"),
                "model": selected.get("model"),
            }
        encoded = json.dumps(replan_packet, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        if len(encoded.encode("utf-8")) > MAX_ROUTE_PROMPT_BYTES:
            raise BrainRunError("replan context exceeds the prompt bound")
        chunks.append(
            {
                "id": "brain-replan",
                "role": "developer",
                "content": encoded,
                "required": True,
                "priority": 950,
            }
        )
        request["context"] = chunks
        return request

    def build_adaptive_model_selection(
        self,
        *,
        task: str,
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        ledger: BrainLearningLedger | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        context: Mapping[str, Any] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        required_capabilities: Sequence[str] = (),
        input_tokens: int = 4_096,
        requested_output_tokens: int = 2_048,
        max_cost_per_million_tokens: int | None = None,
        max_latency_ms: int | None = None,
        min_quality: float | None = None,
        min_selection_confidence: float | None = None,
        selection_overrides: Mapping[str, Any] | None = None,
        selection_weights: Mapping[str, Any] | None = None,
        selection_observations: Sequence[Mapping[str, Any]] | None = None,
    ) -> dict[str, Any]:
        """Build a live model-selection request from registered transports and learned state.

        Applications own the model catalogue because provider model availability and pricing are
        deployment-specific. The brain owns the decision: it removes candidates whose transport
        is not registered or whose required user credential handle is absent, projects persisted
        bandit state into the Rust selector, and scopes observations to an optional domain /
        capability / risk context. No provider secret enters this request.
        """

        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("task must be a non-empty string")
        if not isinstance(model_candidates, Sequence) or isinstance(model_candidates, (str, bytes)):
            raise BrainRunError("model_candidates must be a sequence")
        if not model_candidates:
            raise BrainRunError("model_candidates must not be empty")
        if not isinstance(credentials, Mapping):
            raise BrainRunError("credentials must be a mapping")
        if not isinstance(required_capabilities, Sequence) or isinstance(
            required_capabilities, (str, bytes)
        ):
            raise BrainRunError("required_capabilities must be a sequence")
        if any(not isinstance(capability, str) or not capability.strip() for capability in required_capabilities):
            raise BrainRunError("required_capabilities must contain non-empty strings")
        if not isinstance(input_tokens, int) or isinstance(input_tokens, bool) or input_tokens < 1:
            raise BrainRunError("input_tokens must be a positive integer")
        if not isinstance(requested_output_tokens, int) or isinstance(requested_output_tokens, bool) or requested_output_tokens < 1:
            raise BrainRunError("requested_output_tokens must be a positive integer")
        for name, value in (("max_cost_per_million_tokens", max_cost_per_million_tokens), ("max_latency_ms", max_latency_ms)):
            if value is not None and (not isinstance(value, int) or isinstance(value, bool) or value < 0):
                raise BrainRunError(f"{name} must be a non-negative integer or None")
        if min_quality is not None and (
            not isinstance(min_quality, (int, float))
            or isinstance(min_quality, bool)
            or not 0 <= min_quality <= 1
        ):
            raise BrainRunError("min_quality must be within [0, 1] or None")
        if min_selection_confidence is not None and (
            not isinstance(min_selection_confidence, (int, float))
            or isinstance(min_selection_confidence, bool)
            or not math.isfinite(float(min_selection_confidence))
            or not 0.0 <= float(min_selection_confidence) <= 1.0
        ):
            raise BrainRunError("min_selection_confidence must be within [0, 1] or None")
        if ledger is not None and not isinstance(ledger, BrainLearningLedger):
            raise BrainRunError("ledger must be a BrainLearningLedger or None")
        if bandit_state is not None:
            if not isinstance(bandit_state, Mapping):
                raise BrainRunError("bandit_state must be a mapping or None")
            BrainLearningLedger._assert_safe(bandit_state)
        if context is not None and not isinstance(context, Mapping):
            raise BrainRunError("context must be a mapping or None")
        if not isinstance(contextual_observations, Sequence) or isinstance(
            contextual_observations, (str, bytes)
        ):
            raise BrainRunError("contextual_observations must be a sequence")
        if any(not isinstance(observation, Mapping) for observation in contextual_observations):
            raise BrainRunError("contextual_observations must contain mappings")
        if selection_overrides is not None and not isinstance(selection_overrides, Mapping):
            raise BrainRunError("selection_overrides must be a mapping or None")
        if selection_overrides is not None:
            BrainLearningLedger._assert_safe(selection_overrides)
        # Make the multi-objective policy first-class while retaining the older override escape
        # hatch for callers that persist complete selector requests.  Both paths normalize to the
        # same bounded value-only contract before health and bandit state are joined below.
        try:
            override_weights = (
                None if selection_overrides is None else selection_overrides.get("weights")
            )
            normalized_weights = normalize_autonomous_selection_weights(
                selection_weights if selection_weights is not None else override_weights
            )
            if selection_weights is not None and override_weights is not None:
                normalized_override_weights = normalize_autonomous_selection_weights(override_weights)
                if normalized_override_weights != normalized_weights:
                    raise BrainRunError(
                        "selection_weights conflicts with selection_overrides.weights"
                    )
        except ArgumentError as error:
            raise BrainRunError(str(error)) from error
        try:
            normalized_selection_observations = (
                None
                if selection_observations is None
                else normalize_autonomous_model_observations(selection_observations)
            )
            override_observations = (
                None
                if selection_overrides is None
                else selection_overrides.get("observations")
            )
            normalized_override_observations = (
                None
                if override_observations is None
                else normalize_autonomous_model_observations(override_observations)
            )
        except ArgumentError as error:
            raise BrainRunError(str(error)) from error
        if (
            normalized_selection_observations is not None
            and normalized_override_observations is not None
            and normalized_selection_observations != normalized_override_observations
        ):
            raise BrainRunError(
                "selection_observations conflicts with selection_overrides.observations"
            )
        effective_selection_observations = (
            normalized_selection_observations
            if normalized_selection_observations is not None
            else normalized_override_observations
            or []
        )
        health_overrides: Mapping[str, Any] = {}
        if selection_overrides is not None and selection_overrides.get("provider_health") is not None:
            raw_health_overrides = selection_overrides.get("provider_health")
            if not isinstance(raw_health_overrides, Mapping):
                raise BrainRunError("selection_overrides.provider_health must be a mapping")
            health_overrides = raw_health_overrides
        model_health_overrides: Mapping[str, Any] = {}
        if selection_overrides is not None and selection_overrides.get("model_health") is not None:
            raw_model_health_overrides = selection_overrides.get("model_health")
            if not isinstance(raw_model_health_overrides, Mapping):
                raise BrainRunError("selection_overrides.model_health must be a mapping")
            model_health_overrides = raw_model_health_overrides

        provider_metadata = {
            row.get("provider"): row
            for row in self.runtime.provider_metadata()
            if isinstance(row, Mapping) and isinstance(row.get("provider"), str)
        }
        provider_health: dict[str, dict[str, Any]] = {}
        for provider, metadata in provider_metadata.items():
            status = self.runtime.provider_status(provider)
            provider_health[provider] = {
                "registered": True,
                "circuit": status.get("circuit"),
                "consecutive_failures": status.get("consecutive_failures", 0),
                "attempts": status.get("attempts", 0),
                "successes": status.get("successes", 0),
                "failures": status.get("failures", 0),
                "success_rate": status.get("success_rate", 0.0),
                "mean_latency_ms": status.get("mean_latency_ms"),
                "last_latency_ms": status.get("last_latency_ms"),
                "last_model": status.get("last_model"),
                "last_outcome": status.get("last_outcome"),
                "observed_at": status.get("observed_at"),
                "credential_ready": (
                    not bool(metadata.get("requires_credential", True))
                    or (
                        isinstance(credentials.get(provider), CredentialHandle)
                        and credentials[provider].provider == provider
                    )
                ),
            }
        # Process-local model observations are an immediate routing prior. They deliberately do
        # not become independent hard gates: only the provider circuit can disable every arm on
        # that transport.
        model_health: dict[str, dict[str, Any]] = {
            arm_id: dict(health)
            for arm_id, health in self.runtime.model_health_snapshot().items()
        }
        normalized_models: list[dict[str, Any]] = []
        for candidate in model_candidates:
            if not isinstance(candidate, Mapping):
                raise BrainRunError("model_candidates must contain mappings")
            BrainLearningLedger._assert_safe(candidate)
            model = dict(candidate)
            for field in (
                "provider",
                "model",
                "context_window_tokens",
                "max_output_tokens",
                "quality",
                "latency_ms",
                "cost_per_million_tokens",
            ):
                if field not in model:
                    raise BrainRunError(f"model candidate is missing {field}")
            provider = model.get("provider")
            model_name = model.get("model")
            if not isinstance(provider, str) or not provider.strip() or not isinstance(model_name, str) or not model_name.strip():
                raise BrainRunError("model candidate provider and model must be non-empty strings")
            capabilities = model.get("capabilities", [])
            if not isinstance(capabilities, Sequence) or isinstance(capabilities, (str, bytes)) or any(
                not isinstance(capability, str) for capability in capabilities
            ):
                raise BrainRunError("model candidate capabilities must be a string sequence")
            model["capabilities"] = list(capabilities)
            model.setdefault("requires_credential", True)
            model.setdefault("enabled", True)
            if not isinstance(model["requires_credential"], bool) or not isinstance(model["enabled"], bool):
                raise BrainRunError("model candidate requires_credential and enabled must be booleans")
            registered = provider_metadata.get(provider)
            runtime_requires_credential = True if registered is None else bool(
                registered.get("requires_credential", True)
            )
            requires_credential = bool(model["requires_credential"]) or runtime_requires_credential
            model["requires_credential"] = requires_credential
            health = provider_health.get(provider)
            if health is None:
                health = provider_health[provider] = {
                    "registered": False,
                    "circuit": "unconfigured",
                    "consecutive_failures": 0,
                    "credential_ready": False,
                    "eligible": False,
                }
            if registered is None:
                model["enabled"] = False
            elif requires_credential:
                handle = credentials.get(provider)
                credential_ready = (
                    isinstance(handle, CredentialHandle)
                    and handle.provider == provider
                )
                if credential_ready:
                    try:
                        # Resolve only metadata here. This verifies that the handle belongs to
                        # this runtime and has not expired or been revoked without exposing the
                        # underlying value to the selector or Rust kernel.
                        self.runtime.credentials.metadata(handle)  # type: ignore[arg-type]
                    except CredentialError:
                        credential_ready = False
                health["credential_ready"] = credential_ready
                if not credential_ready:
                    model["enabled"] = False
            health = provider_health[provider]
            if health["circuit"] == "open":
                model["enabled"] = False
            health["eligible"] = bool(model["enabled"]) and bool(health["credential_ready"])
            normalized_models.append(model)

        # A durable health snapshot may add historical evidence to the live provider gate. It can
        # never make an unregistered or credential-ineligible provider eligible; an explicit open
        # historical circuit only narrows the candidate set until an operator resets it.
        for provider, historical in health_overrides.items():
            if not isinstance(provider, str) or not isinstance(historical, Mapping):
                raise BrainRunError("selection_overrides.provider_health must map provider names to objects")
            current = provider_health.setdefault(
                provider,
                {
                    "registered": False,
                    "circuit": "unconfigured",
                    "consecutive_failures": 0,
                    "credential_ready": False,
                    "eligible": False,
                },
            )
            current["historical"] = dict(historical)
            if historical.get("circuit") == "open":
                current["circuit"] = "open"
                for model in normalized_models:
                    if model.get("provider") == provider:
                        model["enabled"] = False
                        current["eligible"] = False

        # Durable model evidence is joined by the unambiguous provider/model arm id. It can
        # influence reliability and latency but cannot override provider registration,
        # credential, capability, or circuit gates.
        for arm_id, historical in model_health_overrides.items():
            if (
                not isinstance(arm_id, str)
                or "/" not in arm_id
                or not arm_id.split("/", 1)[0].strip()
                or not arm_id.split("/", 1)[1].strip()
                or not isinstance(historical, Mapping)
            ):
                raise BrainRunError("selection_overrides.model_health must map provider/model ids to objects")
            provider, model_name = arm_id.split("/", 1)
            current = model_health.setdefault(
                arm_id,
                {
                    "provider": provider,
                    "model": model_name,
                    "attempts": 0,
                    "successes": 0,
                    "failures": 0,
                    "success_rate": 0.0,
                    "last_latency_ms": None,
                    "circuit": "closed",
                },
            )
            current["historical"] = dict(historical)

        # Transport outcomes are a bounded prior update, not task reward. Prefer model-level
        # evidence when an arm has it, then fall back to provider-level evidence. A capped
        # confidence keeps sparse evidence from making routing brittle.
        for model in normalized_models:
            provider = model.get("provider")
            model_name = model.get("model")
            if not isinstance(provider, str) or not isinstance(model_name, str):
                continue
            health = provider_health.get(provider)
            arm_id = f"{provider}/{model_name}"
            model_evidence = model_health.get(arm_id)
            evidence = (
                _routing_health_evidence(arm_id, model_evidence)
                if isinstance(model_evidence, Mapping)
                else None
            )
            evidence_source = "model"
            if evidence is None and isinstance(health, Mapping):
                evidence = _provider_health_evidence(provider, health)
                evidence_source = "provider"
            if evidence is None:
                continue
            model["health_evidence"] = evidence_source
            if evidence_source == "model":
                # The Python façade applies the model evidence below before forwarding the
                # request to the Rust kernel. Keep the evidence in the request for auditability,
                # while marking this local projection so the kernel does not blend it twice.
                forwarded_health = model_health.get(arm_id)
                if isinstance(forwarded_health, dict):
                    forwarded_health["prior_adjustment_applied"] = True
            confidence = float(evidence["confidence"])
            prior_reliability = model.get("reliability", 0.5)
            if (
                isinstance(prior_reliability, (int, float))
                and not isinstance(prior_reliability, bool)
                and math.isfinite(float(prior_reliability))
                and 0.0 <= float(prior_reliability) <= 1.0
            ):
                model["reliability"] = round(
                    (1.0 - confidence) * float(prior_reliability)
                    + confidence * float(evidence["success_rate"]),
                    6,
                )
            prior_latency = model.get("latency_ms")
            if (
                isinstance(prior_latency, int)
                and not isinstance(prior_latency, bool)
                and prior_latency >= 0
            ):
                model["latency_ms"] = max(
                    0,
                    int(
                        round(
                            (1.0 - confidence) * prior_latency
                            + confidence * float(evidence["last_latency_ms"])
                        )
                    ),
                )

        global_state = (
            dict(bandit_state)
            if bandit_state is not None
            else None if ledger is None else ledger.latest_state()
        )
        observations = _bandit_observations(global_state)
        explicit_by_arm = {
            observation["arm_id"]: observation
            for observation in effective_selection_observations
        }
        merged_global_observations = {
            observation["arm_id"]: observation for observation in observations
        }
        merged_global_observations.update(explicit_by_arm)
        observations = [
            merged_global_observations[arm_id]
            for arm_id in sorted(merged_global_observations)
        ]
        scoped_observations: list[dict[str, Any]] = []
        if context is not None:
            context_digest = _context_identity_digest(context)
            scoped_state = (
                dict(bandit_state)
                if bandit_state is not None
                else None if ledger is None else ledger.latest_state(context_digest)
            )
            scoped_by_arm = {
                observation["arm_id"]: observation
                for observation in _bandit_observations(scoped_state, context_digest=context_digest)
            }
            scoped_by_arm.update(explicit_by_arm)
            supplied = _bandit_observations({"arms": list(contextual_observations)})
            scoped_by_arm.update({observation["arm_id"]: observation for observation in supplied})
            scoped_observations = [
                {**observation, "context_digest": context_digest}
                for observation in scoped_by_arm.values()
            ]
        elif contextual_observations:
            raise BrainRunError("contextual_observations require context")

        request: dict[str, Any] = dict(selection_overrides or {})
        request.update(
            {
                "task": task,
                "required_capabilities": list(required_capabilities),
                "input_tokens": input_tokens,
                "requested_output_tokens": requested_output_tokens,
                "models": normalized_models,
                "observations": observations,
                "provider_health": provider_health,
                "model_health": model_health,
                "weights": normalized_weights,
            }
        )
        if max_cost_per_million_tokens is not None:
            request["max_cost_per_million_tokens"] = max_cost_per_million_tokens
        if max_latency_ms is not None:
            request["max_latency_ms"] = max_latency_ms
        if min_quality is not None:
            request["min_quality"] = min_quality
        if min_selection_confidence is not None:
            request["min_selection_confidence"] = float(min_selection_confidence)
        if context is not None:
            request["context"] = dict(context)
            request["contextual_observations"] = scoped_observations
        BrainLearningLedger._assert_safe(request)
        try:
            json.dumps(request, ensure_ascii=False, allow_nan=False)
        except (TypeError, ValueError) as error:
            raise BrainRunError("adaptive model-selection request must be JSON-safe") from error
        return request

    def _prepare_adaptive_route(
        self,
        *,
        task: str,
        route_request: Mapping[str, Any],
    ) -> tuple[dict[str, Any], dict[str, Any]]:
        if not isinstance(route_request, Mapping):
            raise BrainRunError("route_request must be a mapping")
        BrainLearningLedger._assert_safe(route_request)
        arguments = dict(route_request)
        supplied_goal = arguments.get("goal")
        if supplied_goal is not None and supplied_goal != task:
            raise BrainRunError("route_request.goal must match the adaptive task")
        arguments["goal"] = task
        arguments.setdefault("needs", [{"id": "task", "query": task}])
        arguments.setdefault("include_tools", True)
        arguments.setdefault("max_tools", 128)
        try:
            encoded = json.dumps(
                arguments,
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            ).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise BrainRunError("route_request must be JSON-safe") from error
        if len(encoded) > MAX_ROUTE_REQUEST_BYTES:
            raise BrainRunError("route_request exceeds the bounded size")
        response = self.workspace.tool("capability_route", arguments)
        if not isinstance(response, Mapping):
            raise BrainRunError("capability route returned a non-object")
        if response.get("ok") is False or response.get("workflow") != "capability_route":
            raise BrainRunError("capability route was refused")
        route = dict(response)
        BrainLearningLedger._assert_safe(route)
        context = _adaptive_route_context(route, task=task, route_request=arguments)
        return route, context

    def _preview_adaptive_selection(
        self,
        *,
        task: str,
        selection: Mapping[str, Any],
        context: Mapping[str, Any] | None,
        trace_event_callback: Callable[..., Any] | None = None,
        attempt: int = 1,
    ) -> dict[str, Any]:
        _emit_model_selection_trace(
            trace_event_callback,
            phase="model_selection_started",
            status="running",
            attempt=attempt,
            selection=selection,
        )
        try:
            arguments = dict(selection)
            arguments["task"] = task
            if context is None:
                report = self.workspace.tool("brain_model_select", arguments)
                if not isinstance(report, Mapping):
                    raise BrainRunError("adaptive model selection preview returned a non-object")
                result = dict(report)
            else:
                observations = selection.get("contextual_observations", [])
                if not isinstance(observations, list):
                    raise BrainRunError("adaptive contextual selection observations are malformed")
                report = self.workspace.tool(
                    "brain_model_select_contextual",
                    {
                        "context": dict(context),
                        "base": arguments,
                        "observations": [dict(observation) for observation in observations],
                    },
                )
                if not isinstance(report, Mapping):
                    raise BrainRunError("adaptive contextual selection preview returned a non-object")
                nested = report.get("selection")
                if not isinstance(nested, Mapping):
                    raise BrainRunError("adaptive contextual selection preview omitted selection")
                result = dict(nested)
                normalized_context = _normalize_learning_context(context)
                context_digest = report.get("context_digest")
                expected_context_digest = _context_identity_digest(normalized_context)
                if not _valid_digest(context_digest) or context_digest != expected_context_digest:
                    raise BrainRunError(
                        "adaptive contextual selection returned a context digest that does not match its identity"
                    )
                # Preserve the exact contextual binding that the low-level run path would have
                # attached. Fallbacks consume this snapshot directly and must not lose the
                # learning-domain identity merely because they skip selector re-entry.
                result["context_digest"] = context_digest
                result["context"] = normalized_context
                result["contextual_selection_status"] = report.get("selection_status")
            if trace_event_callback is not None:
                audit = build_model_selection_audit(result)
                selected = result.get("selected_model")
                _emit_model_selection_trace(
                    trace_event_callback,
                    phase="model_selection_finished",
                    status="completed" if isinstance(selected, Mapping) else "refused",
                    attempt=attempt,
                    selection=result,
                    audit=audit,
                    selected=selected if isinstance(selected, Mapping) else None,
                    failure_code=None if isinstance(selected, Mapping) else "selection_abstained",
                )
            return result
        except Exception:
            _emit_model_selection_trace(
                trace_event_callback,
                phase="model_selection_finished",
                status="failed",
                attempt=attempt,
                selection=selection,
                failure_code="selection_error",
            )
            raise

    def run_adaptive(
        self,
        *,
        task: str,
        model_candidates: Sequence[Mapping[str, Any]],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        ledger: BrainLearningLedger | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        context: Mapping[str, Any] | None = None,
        content_parts: Sequence[ProviderContentPart | Mapping[str, Any]] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        required_capabilities: Sequence[str] = (),
        input_tokens: int = 4_096,
        requested_output_tokens: int = 2_048,
        max_cost_per_million_tokens: int | None = None,
        max_latency_ms: int | None = None,
        min_quality: float | None = None,
        min_selection_confidence: float | None = None,
        selection_overrides: Mapping[str, Any] | None = None,
        approve_provider_call: bool = False,
        run_id: str | None = None,
        max_output_tokens: int = 2_048,
        temperature: float | None = None,
        require_json: bool = False,
        response_schema: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        tools: Sequence[ProviderTool] = (),
        tool_choice: str | None = None,
        max_provider_failovers: int = 2,
        execution_controller: AutonomousExecutionController | None = None,
        invocation_observer: ProviderInvocationObserver | None = None,
        trace_event_callback: Callable[..., Any] | None = None,
    ) -> BrainRunResult:
        """Select, plan, and invoke from live providers using caller-persisted learning state."""

        if not isinstance(max_provider_failovers, int) or isinstance(max_provider_failovers, bool) or not 0 <= max_provider_failovers <= 8:
            raise BrainRunError("max_provider_failovers must be within [0, 8]")

        selection = self.build_adaptive_model_selection(
            task=task,
            model_candidates=model_candidates,
            credentials=credentials,
            ledger=ledger,
            bandit_state=bandit_state,
            context=context,
            contextual_observations=contextual_observations,
            required_capabilities=required_capabilities,
            input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            min_selection_confidence=min_selection_confidence,
            selection_overrides=selection_overrides,
        )
        effective_contextual_observations = (
            selection.get("contextual_observations", contextual_observations)
            if context is not None
            else contextual_observations
        )
        attempt_selection = dict(selection)
        failed_providers: set[str] = set()
        failover_attempts: list[dict[str, Any]] = []
        invocation_receipts: list[Mapping[str, Any]] = []
        continuation_plan: dict[str, Any] | None = None
        continuation_state: dict[str, Any] | None = None
        selection_snapshot: dict[str, Any] | None = None
        for attempt in range(max_provider_failovers + 1):
            if continuation_plan is None:
                preview = self._preview_adaptive_selection(
                    task=task,
                    selection=selection,
                    context=context,
                    trace_event_callback=trace_event_callback,
                    attempt=attempt + 1,
                )
                attempt_selection = {**selection, **preview}
                selected = attempt_selection.get("selected_model")
                if not isinstance(selected, Mapping):
                    raise BrainRunError("adaptive model selection has no eligible provider")
                attempt_audit = build_model_selection_audit(attempt_selection)
                selection_snapshot = dict(attempt_selection)
            else:
                attempt_selection, selected, attempt_audit = _prepare_fixed_selection_attempt(
                    selection_snapshot if selection_snapshot is not None else selection,
                    continuation_plan,
                    continuation_state,
                    attempt=attempt + 1,
                    trace_event_callback=trace_event_callback,
                    scope="model",
                )
            provider = selected.get("provider")
            model = selected.get("model")
            if not isinstance(provider, str) or not isinstance(model, str):
                raise BrainRunError("adaptive selection returned malformed provider metadata")
            if continuation_plan is None:
                continuation_plan = build_model_continuation_plan(
                    attempt_selection,
                    [candidate for candidate in selection.get("models", []) if isinstance(candidate, Mapping)],
                    max_failovers=max_provider_failovers,
                )
                continuation_state = create_model_continuation_state(continuation_plan)
            selected_id = f"{provider}/{model}"
            policy_observer = None
            if execution_controller is not None:
                policy_observer = AutonomousProviderInvocationSession(
                    controller=execution_controller,
                    provider=provider,
                    model=model,
                    selection_digest=selection.get("decision_digest"),
                    cost_per_million_tokens=selected.get("cost_per_million_tokens", 0.0),
                    attempt=attempt,
                    kind="provider_call",
                )
            effective_observer = _compose_provider_observers(policy_observer, invocation_observer)
            try:
                result = self.run(
                    task=task,
                    model_selection=attempt_selection,
                    selection_override=attempt_selection,
                    prompt=prompt,
                    plan=plan,
                    credentials=credentials,
                    approve_provider_call=approve_provider_call,
                    run_id=run_id,
                    max_output_tokens=max_output_tokens,
                    temperature=temperature,
                    require_json=require_json,
                    response_schema=response_schema,
                    idempotency_key=idempotency_key,
                    context=context,
                    content_parts=content_parts,
                    contextual_observations=effective_contextual_observations,
                    tools=tools,
                    tool_choice=tool_choice,
                    invocation_observer=effective_observer,
                )
                if policy_observer is not None:
                    result = replace(result, provider_invocations=policy_observer.evidence())
                if continuation_plan is None or continuation_state is None:
                    raise BrainRunError("adaptive model continuation was not initialized")
                continuation_state = complete_model_continuation_state(
                    continuation_plan,
                    continuation_state,
                    provider=provider,
                    model=model,
                )
                if not failover_attempts:
                    return replace(result, continuation_plan=continuation_plan)
                invocation_receipts.extend(result.provider_invocations)
                failover_attempts.append(
                    {
                        "attempt": attempt,
                        "provider": provider,
                        "model": model,
                        "arm_id": selected_id,
                        "status": "completed",
                        **_selection_attempt_metadata(attempt_audit),
                    }
                )
                return replace(
                    result,
                    provider_failover={
                        "strategy": "deterministic_model_selector_with_provider_health_gating",
                        "attempts": list(failover_attempts),
                        "fallback_count": len(failover_attempts) - 1,
                        "continuation_plan_digest": continuation_plan["plan_digest"],
                        "continuation_plan": dict(continuation_plan),
                        "continuation_state_digest": continuation_state["state_digest"],
                        "retention": "metadata_only",
                    },
                    provider_invocations=tuple(invocation_receipts),
                    continuation_plan=continuation_plan,
                )
            except ProviderError as error:
                if policy_observer is not None:
                    invocation_receipts.extend(policy_observer.evidence())
                health_after_failure = _refresh_failover_provider_health(
                    self.runtime,
                    attempt_selection,
                    provider=provider,
                    error=error,
                    failed_providers=failed_providers,
                )
                failover_attempts.append(
                    {
                        "attempt": attempt,
                        "provider": provider,
                        "model": model,
                        "arm_id": selected_id,
                        "status": "provider_refused",
                        "reason": "circuit_open" if error.circuit_open else "provider_error",
                        "status_code": error.status_code,
                        **health_after_failure,
                        **_selection_attempt_metadata(attempt_audit),
                    }
                )
                if attempt >= max_provider_failovers:
                    raise
                if continuation_plan is None or continuation_state is None:
                    raise BrainRunError("adaptive model continuation was not initialized")
                continuation_state = advance_model_continuation_state(
                    continuation_plan,
                    continuation_state,
                    provider=provider,
                    model=model,
                    failure_scope=(
                        "model"
                        if error.status_code == 408 and not error.circuit_open
                        else "provider"
                    ),
                    failure_code="circuit_open" if error.circuit_open else "provider_error",
                    status_code=error.status_code,
                )
                if continuation_state["status"] != "ready":
                    raise
        raise BrainRunError("adaptive provider failover exhausted")

    def run_adaptive_tool_loop(
        self,
        *,
        task: str,
        model_candidates: Sequence[Mapping[str, Any]],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        ledger: BrainLearningLedger | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        context: Mapping[str, Any] | None = None,
        content_parts: Sequence[ProviderContentPart | Mapping[str, Any]] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        required_capabilities: Sequence[str] = (),
        input_tokens: int = 4_096,
        requested_output_tokens: int = 2_048,
        max_cost_per_million_tokens: int | None = None,
        max_latency_ms: int | None = None,
        min_quality: float | None = None,
        selection_overrides: Mapping[str, Any] | None = None,
        tool_loop_options: Mapping[str, Any] | None = None,
        max_provider_failovers: int = 2,
        execution_controller: AutonomousExecutionController | None = None,
        invocation_observer: ProviderInvocationObserver | None = None,
        trace_event_callback: Callable[..., Any] | None = None,
    ) -> BrainToolLoopResult:
        """Select adaptively, then enter the bounded route-aware native tool loop.

        ``tool_loop_options`` carries the explicit continuation/authorization options accepted by
        :meth:`run_tool_loop` (for example ``mission_policy``, ``route_request``,
        ``approve_mission_dispatch``, and ``provider_tools``). It intentionally cannot override
        the task, credentials, context, or learned selection assembled by this method.
        """

        if (
            not isinstance(max_provider_failovers, int)
            or isinstance(max_provider_failovers, bool)
            or not 0 <= max_provider_failovers <= 8
        ):
            raise BrainRunError("max_provider_failovers must be within [0, 8]")
        if not isinstance(tool_loop_options, (Mapping, type(None))):
            raise BrainRunError("tool_loop_options must be a mapping or None")
        options = {} if tool_loop_options is None else dict(tool_loop_options)
        allowed_options = {
            "authorize_and_execute",
            "approve_provider_call",
            "run_id",
            "max_output_tokens",
            "temperature",
            "require_json",
            "response_schema",
            "idempotency_key",
            "provider_tools",
            "tool_choice",
            "max_turns",
            "max_tool_calls",
            "stream",
            "mission_policy",
            "approve_mission_dispatch",
            "route_request",
            "enforce_route_tools",
            "require_resolved_route",
            "claim_requests",
            "evaluator_review",
            "workflow_binding",
            "operations_gate_acceptance",
        }
        unknown = sorted(set(options).difference(allowed_options))
        if unknown:
            raise BrainRunError(f"tool_loop_options contains unsupported fields: {', '.join(unknown)}")
        effective_context = context
        route_report: dict[str, Any] | None = None
        if "route_request" in options:
            route_report, route_context = self._prepare_adaptive_route(
                task=task,
                route_request=options["route_request"],
            )
            if effective_context is None:
                effective_context = route_context
            options["route_report"] = route_report
        selection = self.build_adaptive_model_selection(
            task=task,
            model_candidates=model_candidates,
            credentials=credentials,
            ledger=ledger,
            bandit_state=bandit_state,
            context=effective_context,
            contextual_observations=contextual_observations,
            required_capabilities=required_capabilities,
            input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            selection_overrides=selection_overrides,
        )
        effective_contextual_observations = (
            selection.get("contextual_observations", contextual_observations)
            if effective_context is not None
            else contextual_observations
        )
        attempt_selection = dict(selection)
        failed_providers: set[str] = set()
        failover_attempts: list[dict[str, Any]] = []
        invocation_receipts: list[Mapping[str, Any]] = []
        continuation_plan: dict[str, Any] | None = None
        continuation_state: dict[str, Any] | None = None
        selection_snapshot: dict[str, Any] | None = None
        for attempt in range(max_provider_failovers + 1):
            if continuation_plan is None:
                preview = self._preview_adaptive_selection(
                    task=task,
                    selection=selection,
                    context=effective_context,
                    trace_event_callback=trace_event_callback,
                    attempt=attempt + 1,
                )
                attempt_selection = {**selection, **preview}
                selected = attempt_selection.get("selected_model")
                if not isinstance(selected, Mapping):
                    raise BrainRunError("adaptive tool-loop selection has no eligible provider")
                attempt_audit = build_model_selection_audit(attempt_selection)
                selection_snapshot = dict(attempt_selection)
            else:
                attempt_selection, selected, attempt_audit = _prepare_fixed_selection_attempt(
                    selection_snapshot if selection_snapshot is not None else selection,
                    continuation_plan,
                    continuation_state,
                    attempt=attempt + 1,
                    trace_event_callback=trace_event_callback,
                    scope="tool-loop",
                )
            provider = selected.get("provider")
            model = selected.get("model")
            if not isinstance(provider, str) or not isinstance(model, str):
                raise BrainRunError("adaptive tool-loop selection returned malformed provider metadata")
            if continuation_plan is None:
                continuation_plan = build_model_continuation_plan(
                    attempt_selection,
                    [candidate for candidate in selection.get("models", []) if isinstance(candidate, Mapping)],
                    max_failovers=max_provider_failovers,
                )
                continuation_state = create_model_continuation_state(continuation_plan)
            selected_id = f"{provider}/{model}"
            attempt_state: dict[str, Any] = {}
            attempt_options = dict(options)
            attempt_options["attempt_state"] = attempt_state
            policy_observer = None
            if execution_controller is not None:
                policy_observer = AutonomousProviderInvocationSession(
                    controller=execution_controller,
                    provider=provider,
                    model=model,
                    selection_digest=selection.get("decision_digest"),
                    cost_per_million_tokens=selected.get("cost_per_million_tokens", 0.0),
                    attempt=attempt,
                    kind="tool_loop_turn",
                )
            effective_observer = _compose_provider_observers(policy_observer, invocation_observer)
            try:
                result = self.run_tool_loop(
                    task=task,
                    model_selection=attempt_selection,
                    selection_override=attempt_selection,
                    prompt=prompt,
                    plan=plan,
                    credentials=credentials,
                    context=effective_context,
                    content_parts=content_parts,
                    contextual_observations=effective_contextual_observations,
                    invocation_observer=effective_observer,
                    **attempt_options,
                )
                if policy_observer is not None:
                    result = replace(
                        result,
                        brain_run=replace(
                            result.brain_run,
                            provider_invocations=policy_observer.evidence(),
                        ),
                    )
                if continuation_plan is None or continuation_state is None:
                    raise BrainRunError("adaptive tool-loop continuation was not initialized")
                continuation_state = complete_model_continuation_state(
                    continuation_plan,
                    continuation_state,
                    provider=provider,
                    model=model,
                )
                if not failover_attempts:
                    return replace(
                        result,
                        brain_run=replace(result.brain_run, continuation_plan=continuation_plan),
                    )
                invocation_receipts.extend(result.brain_run.provider_invocations)
                failover_attempts.append(
                    {
                        "attempt": attempt,
                        "provider": provider,
                        "model": model,
                        "arm_id": selected_id,
                        "status": "completed",
                        **_selection_attempt_metadata(attempt_audit),
                    }
                )
                return replace(
                    result,
                    brain_run=replace(
                        result.brain_run,
                        provider_failover={
                            "strategy": "deterministic_tool_loop_selector_before_side_effects",
                            "attempts": list(failover_attempts),
                            "fallback_count": len(failover_attempts) - 1,
                            "continuation_plan_digest": continuation_plan["plan_digest"],
                            "continuation_plan": dict(continuation_plan),
                            "continuation_state_digest": continuation_state["state_digest"],
                            "retention": "metadata_only",
                        },
                        provider_invocations=tuple(invocation_receipts),
                        continuation_plan=continuation_plan,
                    ),
                )
            except ProviderError as error:
                if attempt_state.get("tool_authorization_started"):
                    raise
                if policy_observer is not None:
                    invocation_receipts.extend(policy_observer.evidence())
                health_after_failure = _refresh_failover_provider_health(
                    self.runtime,
                    attempt_selection,
                    provider=provider,
                    error=error,
                    failed_providers=failed_providers,
                )
                failover_attempts.append(
                    {
                        "attempt": attempt,
                        "provider": provider,
                        "model": model,
                        "arm_id": selected_id,
                        "status": "provider_refused",
                        "reason": "circuit_open" if error.circuit_open else "provider_error",
                        "status_code": error.status_code,
                        **health_after_failure,
                        **_selection_attempt_metadata(attempt_audit),
                    }
                )
                if attempt >= max_provider_failovers:
                    raise
                if continuation_plan is None or continuation_state is None:
                    raise BrainRunError("adaptive tool-loop continuation was not initialized")
                continuation_state = advance_model_continuation_state(
                    continuation_plan,
                    continuation_state,
                    provider=provider,
                    model=model,
                    failure_scope=(
                        "model"
                        if error.status_code == 408 and not error.circuit_open
                        else "provider"
                    ),
                    failure_code="circuit_open" if error.circuit_open else "provider_error",
                    status_code=error.status_code,
                )
                if continuation_state["status"] != "ready":
                    raise
        raise BrainRunError("adaptive tool-loop provider failover exhausted")

    def run_adaptive_mission(
        self,
        *,
        task: str,
        model_candidates: Sequence[Mapping[str, Any]],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        mission_policy: MissionPolicy | Mapping[str, Any],
        ledger: BrainLearningLedger | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        context: Mapping[str, Any] | None = None,
        content_parts: Sequence[ProviderContentPart | Mapping[str, Any]] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        required_capabilities: Sequence[str] = (),
        input_tokens: int = 4_096,
        requested_output_tokens: int = 2_048,
        max_cost_per_million_tokens: int | None = None,
        max_latency_ms: int | None = None,
        min_quality: float | None = None,
        selection_overrides: Mapping[str, Any] | None = None,
        approve_provider_call: bool = False,
        approve_mission_dispatch: bool = False,
        run_id: str | None = None,
        max_output_tokens: int = 2_048,
        temperature: float | None = None,
        response_schema: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
        route_review: Mapping[str, Any] | None = None,
        operations_gate_acceptance: Mapping[str, Any] | None = None,
        route_request: Mapping[str, Any] | None = None,
        enforce_route_tools: bool = True,
        require_resolved_route: bool = True,
        provider_tools: Sequence[ProviderTool] = (),
        tool_choice: str | None = None,
        max_provider_failovers: int = 2,
        execution_controller: AutonomousExecutionController | None = None,
        invocation_observer: ProviderInvocationObserver | None = None,
        trace_event_callback: Callable[..., Any] | None = None,
    ) -> BrainMissionResult:
        """Select, route, plan, and execute one bounded cross-domain mission.

        The route is resolved once and reused for contextual model selection, prompt assembly,
        tool narrowing, and mission authorization. Provider failover is allowed only while the
        model is still producing the mission proposal; once the proposal reaches ``agent_mission``
        this method never replays it against another provider.
        """

        if (
            not isinstance(max_provider_failovers, int)
            or isinstance(max_provider_failovers, bool)
            or not 0 <= max_provider_failovers <= 8
        ):
            raise BrainRunError("max_provider_failovers must be within [0, 8]")
        if route_request is not None and not isinstance(route_request, Mapping):
            raise BrainRunError("route_request must be a mapping or None")

        effective_context = context
        route_report: dict[str, Any] | None = None
        if route_request is not None:
            route_report, route_context = self._prepare_adaptive_route(
                task=task,
                route_request=route_request,
            )
            if effective_context is None:
                effective_context = route_context

        selection = self.build_adaptive_model_selection(
            task=task,
            model_candidates=model_candidates,
            credentials=credentials,
            ledger=ledger,
            bandit_state=bandit_state,
            context=effective_context,
            contextual_observations=contextual_observations,
            required_capabilities=required_capabilities,
            input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            selection_overrides=selection_overrides,
        )
        effective_contextual_observations = (
            selection.get("contextual_observations", contextual_observations)
            if effective_context is not None
            else contextual_observations
        )
        attempt_selection = dict(selection)
        failed_providers: set[str] = set()
        failover_attempts: list[dict[str, Any]] = []
        invocation_receipts: list[Mapping[str, Any]] = []
        continuation_plan: dict[str, Any] | None = None
        continuation_state: dict[str, Any] | None = None
        selection_snapshot: dict[str, Any] | None = None
        for attempt in range(max_provider_failovers + 1):
            if continuation_plan is None:
                preview = self._preview_adaptive_selection(
                    task=task,
                    selection=selection,
                    context=effective_context,
                    trace_event_callback=trace_event_callback,
                    attempt=attempt + 1,
                )
                attempt_selection = {**selection, **preview}
                selected = attempt_selection.get("selected_model")
                if not isinstance(selected, Mapping):
                    raise BrainRunError("adaptive mission selection has no eligible provider")
                attempt_audit = build_model_selection_audit(attempt_selection)
                selection_snapshot = dict(attempt_selection)
            else:
                attempt_selection, selected, attempt_audit = _prepare_fixed_selection_attempt(
                    selection_snapshot if selection_snapshot is not None else selection,
                    continuation_plan,
                    continuation_state,
                    attempt=attempt + 1,
                    trace_event_callback=trace_event_callback,
                    scope="mission",
                )
            provider = selected.get("provider")
            model = selected.get("model")
            if not isinstance(provider, str) or not isinstance(model, str):
                raise BrainRunError("adaptive mission selection returned malformed provider metadata")
            if continuation_plan is None:
                continuation_plan = build_model_continuation_plan(
                    attempt_selection,
                    [candidate for candidate in selection.get("models", []) if isinstance(candidate, Mapping)],
                    max_failovers=max_provider_failovers,
                )
                continuation_state = create_model_continuation_state(continuation_plan)
            selected_id = f"{provider}/{model}"
            attempt_state: dict[str, Any] = {}
            policy_observer = None
            if execution_controller is not None:
                policy_observer = AutonomousProviderInvocationSession(
                    controller=execution_controller,
                    provider=provider,
                    model=model,
                    selection_digest=selection.get("decision_digest"),
                    cost_per_million_tokens=selected.get("cost_per_million_tokens", 0.0),
                    attempt=attempt,
                    kind="mission_proposal",
                )
            effective_observer = _compose_provider_observers(policy_observer, invocation_observer)
            try:
                result = self.run_mission(
                    task=task,
                    model_selection=attempt_selection,
                    selection_override=attempt_selection,
                    prompt=prompt,
                    plan=plan,
                    credentials=credentials,
                    mission_policy=mission_policy,
                    approve_provider_call=approve_provider_call,
                    approve_mission_dispatch=approve_mission_dispatch,
                    run_id=run_id,
                    max_output_tokens=max_output_tokens,
                    temperature=temperature,
                    response_schema=response_schema,
                    idempotency_key=idempotency_key,
                    claim_requests=claim_requests,
                    context=effective_context,
                    content_parts=content_parts,
                    contextual_observations=effective_contextual_observations,
                    evaluator_review=evaluator_review,
                    workflow_binding=workflow_binding,
                    route_review=route_review,
                    operations_gate_acceptance=operations_gate_acceptance,
                    route_request=route_request,
                    route_report=route_report,
                    enforce_route_tools=enforce_route_tools,
                    require_resolved_route=require_resolved_route,
                    provider_tools=provider_tools,
                    tool_choice=tool_choice,
                    attempt_state=attempt_state,
                    invocation_observer=effective_observer,
                )
                if policy_observer is not None:
                    result = replace(
                        result,
                        brain_run=replace(
                            result.brain_run,
                            provider_invocations=policy_observer.evidence(),
                        ),
                    )
                if continuation_plan is None or continuation_state is None:
                    raise BrainRunError("adaptive mission continuation was not initialized")
                continuation_state = complete_model_continuation_state(
                    continuation_plan,
                    continuation_state,
                    provider=provider,
                    model=model,
                )
                if not failover_attempts:
                    return replace(
                        result,
                        brain_run=replace(result.brain_run, continuation_plan=continuation_plan),
                    )
                invocation_receipts.extend(result.brain_run.provider_invocations)
                failover_attempts.append(
                    {
                        "attempt": attempt,
                        "provider": provider,
                        "model": model,
                        "arm_id": selected_id,
                        "status": "completed",
                        **_selection_attempt_metadata(attempt_audit),
                    }
                )
                return replace(
                    result,
                    brain_run=replace(
                        result.brain_run,
                        provider_failover={
                            "strategy": "deterministic_mission_selector_before_dispatch",
                            "attempts": list(failover_attempts),
                            "fallback_count": len(failover_attempts) - 1,
                            "continuation_plan_digest": continuation_plan["plan_digest"],
                            "continuation_plan": dict(continuation_plan),
                            "continuation_state_digest": continuation_state["state_digest"],
                            "retention": "metadata_only",
                        },
                        provider_invocations=tuple(invocation_receipts),
                        continuation_plan=continuation_plan,
                    ),
                )
            except ProviderError as error:
                if attempt_state.get("mission_dispatch_started"):
                    raise
                if policy_observer is not None:
                    invocation_receipts.extend(policy_observer.evidence())
                health_after_failure = _refresh_failover_provider_health(
                    self.runtime,
                    attempt_selection,
                    provider=provider,
                    error=error,
                    failed_providers=failed_providers,
                )
                failover_attempts.append(
                    {
                        "attempt": attempt,
                        "provider": provider,
                        "model": model,
                        "arm_id": selected_id,
                        "status": "provider_refused",
                        "reason": "circuit_open" if error.circuit_open else "provider_error",
                        "status_code": error.status_code,
                        **health_after_failure,
                        **_selection_attempt_metadata(attempt_audit),
                    }
                )
                if attempt >= max_provider_failovers:
                    raise
                if continuation_plan is None or continuation_state is None:
                    raise BrainRunError("adaptive mission continuation was not initialized")
                continuation_state = advance_model_continuation_state(
                    continuation_plan,
                    continuation_state,
                    provider=provider,
                    model=model,
                    failure_scope=(
                        "model"
                        if error.status_code == 408 and not error.circuit_open
                        else "provider"
                    ),
                    failure_code="circuit_open" if error.circuit_open else "provider_error",
                    status_code=error.status_code,
                )
                if continuation_state["status"] != "ready":
                    raise
        raise BrainRunError("adaptive mission provider failover exhausted")

    def run_adaptive_mission_learning_cycle(
        self,
        *,
        task: str,
        model_candidates: Sequence[Mapping[str, Any]],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        mission_policy: MissionPolicy | Mapping[str, Any],
        evaluator: "BrainOutcomeEvaluator",
        bandit_state: Mapping[str, Any],
        provider_health: Mapping[str, Any] | None = None,
        model_health: Mapping[str, Any] | None = None,
        ledger: BrainLearningLedger | None = None,
        memory: BrainEpisodicMemory | None = None,
        memory_query: MemoryQuery | Mapping[str, Any] | None = None,
        memory_limit: int = 8,
        memory_tags: Sequence[str] = (),
        evidence: Mapping[str, Any] | None = None,
        max_replans: int = 1,
        trajectory_discount: float | None = None,
        trajectory_terminal_reward: float | None = None,
        mission_options: Mapping[str, Any] | None = None,
        execution_controller: AutonomousExecutionController | None = None,
        invocation_observer: ProviderInvocationObserver | None = None,
        trace_event_callback: Callable[..., Any] | None = None,
    ) -> BrainLearningCycleResult:
        """Run, evaluate, remember, and boundedly replan a cross-domain mission.

        This is the high-level learning seam for applications that want the agent to improve
        across calls.  Recalled episodes are inserted as non-authorizing developer context.  Each
        outcome is sent through the explicit evaluator and Rust bandit recorder, then persisted as
        a separate memory evaluation event.  A replan can happen only when the evaluator requests
        one and the prior mission has not crossed the external-effect dispatch boundary.

        ``mission_options`` contains the optional keyword arguments accepted by
        :meth:`run_adaptive_mission`; keeping them in one mapping makes this orchestration API
        forward-compatible while rejecting accidental task/credential/policy overrides.
        """

        if not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("evaluator must be a BrainOutcomeEvaluator")
        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("bandit_state must be a mapping")
        if provider_health is not None:
            if not isinstance(provider_health, Mapping):
                raise BrainRunError("provider_health must be a mapping or None")
            BrainLearningLedger._assert_safe(provider_health)
        if model_health is not None:
            if not isinstance(model_health, Mapping):
                raise BrainRunError("model_health must be a mapping or None")
            BrainLearningLedger._assert_safe(model_health)
        BrainLearningLedger._assert_safe(bandit_state)
        if not isinstance(max_replans, int) or isinstance(max_replans, bool) or not 0 <= max_replans <= 3:
            raise BrainRunError("max_replans must be within [0, 3]")
        trajectory_mode = trajectory_discount is not None
        if trajectory_mode and (
            isinstance(trajectory_discount, bool)
            or not isinstance(trajectory_discount, (int, float))
            or not math.isfinite(float(trajectory_discount))
            or not 0.0 < float(trajectory_discount) <= 1.0
        ):
            raise BrainRunError("trajectory_discount must be within (0, 1] or None")
        if trajectory_terminal_reward is not None and (
            isinstance(trajectory_terminal_reward, bool)
            or not isinstance(trajectory_terminal_reward, (int, float))
            or not math.isfinite(float(trajectory_terminal_reward))
            or not -1.0 <= float(trajectory_terminal_reward) <= 1.0
        ):
            raise BrainRunError("trajectory_terminal_reward must be within [-1, 1] or None")
        if not isinstance(memory_limit, int) or isinstance(memory_limit, bool) or not 1 <= memory_limit <= 32:
            raise BrainRunError("memory_limit must be within [1, 32]")
        if not isinstance(memory_tags, Sequence) or isinstance(memory_tags, (str, bytes)):
            raise BrainRunError("memory_tags must be a string sequence")
        if any(not isinstance(tag, str) or not tag.strip() for tag in memory_tags):
            raise BrainRunError("memory_tags must contain non-empty strings")
        if evidence is not None:
            if not isinstance(evidence, Mapping):
                raise BrainRunError("evidence must be a mapping or None")
            BrainLearningLedger._assert_safe(evidence)
        store = memory if memory is not None else self.memory
        if store is None:
            raise BrainRunError("episodic memory is required for a learning cycle")
        if not isinstance(store, BrainEpisodicMemory):
            raise BrainRunError("memory must be a BrainEpisodicMemory")
        if mission_options is not None and not isinstance(mission_options, Mapping):
            raise BrainRunError("mission_options must be a mapping or None")
        options = {} if mission_options is None else dict(mission_options)
        if provider_health is not None:
            overrides = options.get("selection_overrides", {})
            if not isinstance(overrides, Mapping):
                raise BrainRunError("mission_options.selection_overrides must be a mapping")
            overrides = dict(overrides)
            prior_health = overrides.get("provider_health", {})
            if not isinstance(prior_health, Mapping):
                raise BrainRunError("mission_options.provider_health must be a mapping")
            merged_health = dict(prior_health)
            for provider, snapshot in provider_health.items():
                if not isinstance(provider, str) or not isinstance(snapshot, Mapping):
                    raise BrainRunError("provider_health must map provider names to objects")
                merged_health[provider] = dict(snapshot)
            overrides["provider_health"] = merged_health
            options["selection_overrides"] = overrides
        if model_health is not None:
            overrides = options.get("selection_overrides", {})
            if not isinstance(overrides, Mapping):
                raise BrainRunError("mission_options.selection_overrides must be a mapping")
            overrides = dict(overrides)
            prior_health = overrides.get("model_health", {})
            if not isinstance(prior_health, Mapping):
                raise BrainRunError("mission_options.model_health must be a mapping")
            merged_health = dict(prior_health)
            for arm_id, snapshot in model_health.items():
                if not isinstance(arm_id, str) or not isinstance(snapshot, Mapping):
                    raise BrainRunError("model_health must map provider/model ids to objects")
                merged_health[arm_id] = dict(snapshot)
            overrides["model_health"] = merged_health
            options["selection_overrides"] = overrides
        allowed_options = {
            "context",
            "content_parts",
            "contextual_observations",
            "required_capabilities",
            "input_tokens",
            "requested_output_tokens",
            "max_cost_per_million_tokens",
            "max_latency_ms",
            "min_quality",
            "selection_overrides",
            "approve_provider_call",
            "approve_mission_dispatch",
            "run_id",
            "max_output_tokens",
            "temperature",
            "response_schema",
            "idempotency_key",
            "claim_requests",
            "evaluator_review",
            "workflow_binding",
            "route_review",
            "operations_gate_acceptance",
            "route_request",
            "enforce_route_tools",
            "require_resolved_route",
            "provider_tools",
            "tool_choice",
            "max_provider_failovers",
        }
        unknown = sorted(set(options).difference(allowed_options))
        if unknown:
            raise BrainRunError("mission_options contains unsupported fields: " + ", ".join(unknown))
        context = options.get("context")
        if context is not None and not isinstance(context, Mapping):
            raise BrainRunError("mission_options.context must be a mapping")
        if memory_query is None and isinstance(context, Mapping):
            derived_query = {
                field: context[field]
                for field in ("domain", "capability", "risk_class")
                if isinstance(context.get(field), str) and context[field].strip()
            }
            resolved_query: MemoryQuery | Mapping[str, Any] | None = derived_query
        else:
            resolved_query = memory_query
        try:
            recalled = tuple(store.retrieve(resolved_query, limit=memory_limit))
        except BrainMemoryError as error:
            raise BrainRunError("episodic memory retrieval failed") from error
        base_prompt = self._append_memory_prompt(prompt, recalled)
        current_prompt = base_prompt
        current_bandit_state: Mapping[str, Any] = dict(bandit_state)
        attempts: list[BrainMissionResult] = []
        evaluations: list[dict[str, Any]] = []
        memory_receipts: list[dict[str, Any]] = []
        trajectory_episodes: list[BrainLearningEpisode] = []
        trajectory_decisions: list[BrainEvaluatorDecision] = []
        trajectory_evidence: list[Mapping[str, Any] | None] = []
        final_status = "completed"
        replan_count = 0

        for attempt in range(max_replans + 1):
            result = self.run_adaptive_mission(
                task=task,
                model_candidates=model_candidates,
                prompt=current_prompt,
                plan=plan,
                credentials=credentials,
                mission_policy=mission_policy,
                bandit_state=current_bandit_state,
                execution_controller=execution_controller,
                invocation_observer=invocation_observer,
                trace_event_callback=trace_event_callback,
                **options,
            )
            attempts.append(result)
            if trajectory_mode:
                episode_id = f"{result.brain_run.run_id}-attempt-{attempt}"
                if len(episode_id.encode("utf-8")) > 256:
                    episode_id = "episode-" + hashlib.sha256(episode_id.encode("utf-8")).hexdigest()
                episode = self.prepare_learning_episode(
                    result,
                    evidence=evidence,
                    episode_id=episode_id,
                    ledger=ledger,
                )
                decision = evaluator.assess(result, evidence=evidence)
                trajectory_episodes.append(episode)
                trajectory_decisions.append(decision)
                trajectory_evidence.append(evidence)
                report = {
                    "status": "deferred_trajectory_reward",
                    "next_state": current_bandit_state,
                    "learning_evidence": None,
                }
            else:
                decision, report = evaluator.evaluate_and_record_with_decision(
                    self,
                    result,
                    bandit_state=current_bandit_state,
                    evidence=evidence,
                    ledger=ledger,
                )
                next_state = report.get("next_state")
                if isinstance(next_state, Mapping):
                    current_bandit_state = dict(next_state)
            BrainLearningLedger._assert_safe(report)
            episode_id = f"{result.brain_run.run_id}-attempt-{attempt}"
            if len(episode_id.encode("utf-8")) > 256:
                episode_id = "episode-" + hashlib.sha256(episode_id.encode("utf-8")).hexdigest()
            episode_receipt = self.remember_result(
                result,
                task=task,
                episode_id=episode_id,
                context=context,
                tags=[*memory_tags, f"attempt:{attempt}"],
                lesson=decision.replan_instruction if decision.replan_requested else None,
                provenance={
                    "evaluator_id": decision.evaluator_id,
                    "evaluator_version": decision.evaluator_version,
                    "replan_requested": decision.replan_requested,
                },
                memory=store,
            )
            if trajectory_mode:
                memory_receipts.append(episode_receipt)
            else:
                try:
                    evaluation_receipt = store.record_evaluation(
                        episode_id,
                        {
                            **decision.to_dict(),
                            "decision_digest": _json_digest(decision.to_dict()),
                        },
                    ).to_dict()
                except BrainMemoryError as error:
                    raise BrainRunError("episodic evaluation record failed") from error
                memory_receipts.extend((episode_receipt, evaluation_receipt))
            evaluations.append(
                {
                    "decision": decision.to_dict(),
                    "recording": {
                        "status": report.get("status"),
                        "next_state": report.get("next_state"),
                        "learning_evidence": report.get("learning_evidence"),
                    },
                }
            )
            if not decision.failed or not decision.replan_requested:
                final_status = "completed" if decision.passed else "completed_without_replan"
                break
            if result.status == "mission_dispatched" or result.execution is not None:
                final_status = "replan_blocked_after_dispatch"
                break
            if attempt >= max_replans:
                final_status = "replan_limit_reached"
                break
            replan_count += 1
            current_prompt = self._append_replan_prompt(
                base_prompt,
                attempt=attempt + 1,
                previous_result=result,
                decision=decision,
            )
        else:
            final_status = "replan_limit_reached"

        trajectory_result: BrainLearningTrajectoryResult | None = None
        if trajectory_mode and trajectory_episodes:
            trajectory = BrainLearningTrajectory(
                trajectory_id="mission-trajectory-" + _json_digest(
                    {"runs": [episode.run_id for episode in trajectory_episodes]}
                ),
                episodes=tuple(trajectory_episodes),
                discount=float(trajectory_discount),
                terminal_reward=trajectory_terminal_reward,
            )
            trajectory_result = evaluator.settle_trajectory(
                self,
                trajectory,
                decisions=trajectory_decisions,
                bandit_state=bandit_state,
                evidence_by_step=trajectory_evidence,
                ledger=ledger,
            )
            current_bandit_state = dict(trajectory_result.bandit_state)
            for index, decision in enumerate(trajectory_result.decisions):
                recording = trajectory_result.recordings[index]
                evaluations[index] = {
                    "decision": decision.to_dict(),
                    "recording": {
                        "status": recording.get("status"),
                        "next_state": recording.get("next_state"),
                        "learning_evidence": recording.get("learning_evidence"),
                        "trajectory_id": trajectory.trajectory_id,
                        "trajectory_step": index,
                        "credited_reward": trajectory_result.credited_rewards[index],
                    },
                }
                try:
                    evaluation_receipt = store.record_evaluation(
                        trajectory.episodes[index].episode_id,
                        {
                            **decision.to_dict(),
                            "decision_digest": _json_digest(decision.to_dict()),
                        },
                    ).to_dict()
                except BrainMemoryError as error:
                    raise BrainRunError("trajectory evaluation memory record failed") from error
                memory_receipts.append(evaluation_receipt)

        return BrainLearningCycleResult(
            status=final_status,
            final_result=attempts[-1],
            attempts=tuple(attempts),
            evaluations=tuple(evaluations),
            memory_receipts=tuple(memory_receipts),
            recalled_memory=recalled,
            replan_count=replan_count,
            trajectory_result=trajectory_result,
        )

    def run_resumable_learning_job(
        self,
        store: "BrainJobStore",
        *,
        job_id: str,
        worker_id: str,
        resolver: Callable[[Mapping[str, Any]], Mapping[str, Any]],
        evaluator: "BrainOutcomeEvaluator",
        bandit_state: Mapping[str, Any],
        provider_health: Mapping[str, Any] | None = None,
        model_health: Mapping[str, Any] | None = None,
        lease_seconds: float = 60.0,
        ledger: BrainLearningLedger | None = None,
        memory: BrainEpisodicMemory | None = None,
        approval_router: Any | None = None,
        approval_scope: str | None = None,
        required_approval_role: str = "operator",
    ) -> BrainJobRunResult:
        """Claim and execute one restart-safe learning job through a caller resolver.

        The persisted job never contains the task, prompt, plan, provider response, evaluator
        evidence, or credential handle. ``resolver`` receives only the public job metadata and
        rehydrates those values in-process (typically by resolving a secret-manager reference and
        collecting a fresh BYOK handle). Any exception during the cycle is conservatively marked
        as reconciliation-required because the process cannot prove whether a side effect began.
        A mission that reaches an approval boundary is durably parked in ``waiting_approval``;
        it is never reported as completed merely because its proposal was generated.
        """

        from .jobs import BrainJobError, BrainJobStore
        from .control_plane import BrainApprovalRouter

        if not isinstance(store, BrainJobStore):
            raise BrainRunError("store must be a BrainJobStore")
        if not callable(resolver):
            raise BrainRunError("resolver must be callable")
        if not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("evaluator must be a BrainOutcomeEvaluator")
        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("bandit_state must be a mapping")
        if provider_health is not None:
            if not isinstance(provider_health, Mapping):
                raise BrainRunError("provider_health must be a mapping or None")
            BrainLearningLedger._assert_safe(provider_health)
        if model_health is not None:
            if not isinstance(model_health, Mapping):
                raise BrainRunError("model_health must be a mapping or None")
            BrainLearningLedger._assert_safe(model_health)
        if not isinstance(lease_seconds, (int, float)) or isinstance(lease_seconds, bool) or not 1 <= lease_seconds <= 86_400:
            raise BrainRunError("lease_seconds must be within [1, 86400]")
        if approval_router is None:
            approval_router = BrainApprovalRouter(store)
        elif not isinstance(approval_router, BrainApprovalRouter):
            raise BrainRunError("approval_router must be a BrainApprovalRouter or None")
        if approval_scope is not None and (
            not isinstance(approval_scope, str)
            or not approval_scope.strip()
            or len(approval_scope.encode("utf-8")) > 512
        ):
            raise BrainRunError("approval_scope must be a bounded non-empty string or None")
        if (
            not isinstance(required_approval_role, str)
            or not required_approval_role.strip()
            or len(required_approval_role.encode("utf-8")) > 128
        ):
            raise BrainRunError("required_approval_role must be a bounded non-empty string")
        try:
            job = store.claim(job_id, worker_id, lease_seconds=lease_seconds)
        except BrainJobError as error:
            raise BrainRunError("brain job claim failed") from error
        if job.terminal:
            return BrainJobRunResult(
                status="already_terminal",
                job=job.to_dict(),
                cycle=None,
                error_class=None,
            )
        execution_started = False
        try:
            approval_released = job.checkpoint.get("phase") == "approval_released"
            job = store.checkpoint(
                job.job_id,
                worker_id,
                phase="resolving_spec",
                checkpoint={
                    **dict(job.checkpoint),
                    "spec_digest": job.spec_digest,
                    "attempt": job.attempts,
                },
                side_effect_boundary="not_started",
            )
            resolved = resolver(job.to_dict())
            if not isinstance(resolved, Mapping):
                raise BrainRunError("job resolver must return a mapping")
            allowed = {
                "task",
                "model_candidates",
                "prompt",
                "plan",
                "credentials",
                "mission_policy",
                "memory_query",
                "memory_limit",
                "memory_tags",
                "evidence",
                "max_replans",
                "mission_options",
            }
            unknown = sorted(set(resolved).difference(allowed))
            if unknown:
                raise BrainRunError("job resolver returned unsupported fields: " + ", ".join(unknown))
            required = {"task", "model_candidates", "prompt", "plan", "credentials", "mission_policy"}
            missing = sorted(required.difference(resolved))
            if missing:
                raise BrainRunError("job resolver omitted required fields: " + ", ".join(missing))
            store.checkpoint(
                job.job_id,
                worker_id,
                phase="learning_cycle_started",
                checkpoint={
                    **dict(job.checkpoint),
                    "spec_digest": job.spec_digest,
                    "attempt": job.attempts,
                },
                side_effect_boundary="not_started",
            )
            execution_started = True
            resolved_for_cycle = dict(resolved)
            if approval_released:
                options = resolved_for_cycle.get("mission_options", {})
                if not isinstance(options, Mapping):
                    raise BrainRunError("approved job mission_options must be a mapping")
                options = dict(options)
                # The durable approval router is the authorization boundary for this rehydrated
                # dispatch. The resolver still owns every private prompt/tool argument, but it
                # cannot accidentally discard the operator's decision by returning False here.
                options["approve_mission_dispatch"] = True
                resolved_for_cycle["mission_options"] = options
            if provider_health is not None:
                options = resolved_for_cycle.get("mission_options", {})
                if not isinstance(options, Mapping):
                    raise BrainRunError("job mission_options must be a mapping")
                options = dict(options)
                overrides = options.get("selection_overrides", {})
                if not isinstance(overrides, Mapping):
                    raise BrainRunError("job mission_options.selection_overrides must be a mapping")
                overrides = dict(overrides)
                prior_health = overrides.get("provider_health", {})
                if not isinstance(prior_health, Mapping):
                    raise BrainRunError("job mission_options.provider_health must be a mapping")
                merged_health = dict(prior_health)
                for provider, snapshot in provider_health.items():
                    if not isinstance(provider, str) or not isinstance(snapshot, Mapping):
                        raise BrainRunError("provider_health must map provider names to objects")
                    merged_health[provider] = dict(snapshot)
                overrides["provider_health"] = merged_health
                if model_health is not None:
                    prior_model_health = overrides.get("model_health", {})
                    if not isinstance(prior_model_health, Mapping):
                        raise BrainRunError("job mission_options.model_health must be a mapping")
                    merged_model_health = dict(prior_model_health)
                    for arm_id, snapshot in model_health.items():
                        if not isinstance(arm_id, str) or not isinstance(snapshot, Mapping):
                            raise BrainRunError("model_health must map provider/model ids to objects")
                        merged_model_health[arm_id] = dict(snapshot)
                    overrides["model_health"] = merged_model_health
                options["selection_overrides"] = overrides
                resolved_for_cycle["mission_options"] = options
            elif model_health is not None:
                options = resolved_for_cycle.get("mission_options", {})
                if not isinstance(options, Mapping):
                    raise BrainRunError("job mission_options must be a mapping")
                options = dict(options)
                overrides = options.get("selection_overrides", {})
                if not isinstance(overrides, Mapping):
                    raise BrainRunError("job mission_options.selection_overrides must be a mapping")
                overrides = dict(overrides)
                prior_model_health = overrides.get("model_health", {})
                if not isinstance(prior_model_health, Mapping):
                    raise BrainRunError("job mission_options.model_health must be a mapping")
                merged_model_health = dict(prior_model_health)
                for arm_id, snapshot in model_health.items():
                    if not isinstance(arm_id, str) or not isinstance(snapshot, Mapping):
                        raise BrainRunError("model_health must map provider/model ids to objects")
                    merged_model_health[arm_id] = dict(snapshot)
                overrides["model_health"] = merged_model_health
                options["selection_overrides"] = overrides
                resolved_for_cycle["mission_options"] = options
            cycle = self.run_adaptive_mission_learning_cycle(
                **resolved_for_cycle,
                evaluator=evaluator,
                bandit_state=bandit_state,
                ledger=ledger,
                memory=memory if memory is not None else self.memory,
            )
            final_result = cycle.final_result
            requires_approval = getattr(final_result, "status", None) in {
                "mission_approval_required",
                "approval_required",
            }
            if requires_approval:
                request_digest = final_result.brain_run.outcome_digest
                effective_scope = approval_scope or (
                    f"{job.domain}:{job.capability}:{job.risk_class}:mission_dispatch"
                )
                approval_router.request(
                    job.job_id,
                    worker_id,
                    approval_scope=effective_scope,
                    request_digest=request_digest,
                    required_role=required_approval_role,
                )
                waiting = store.get(job.job_id)
                if waiting is None:
                    raise BrainRunError("approval-waiting job disappeared from the durable store")
                return BrainJobRunResult(
                    status="waiting_approval",
                    job=waiting.to_dict(),
                    cycle=cycle,
                )
            boundary = "dispatched" if (
                final_result.status == "mission_dispatched"
                or final_result.execution is not None
            ) else "preflight"
            store.checkpoint(
                job.job_id,
                worker_id,
                phase="learning_cycle_completed",
                checkpoint={
                    "cycle_status": cycle.status,
                    "attempt_count": len(cycle.attempts),
                    "replan_count": cycle.replan_count,
                    "final_outcome_digest": cycle.final_result.brain_run.outcome_digest,
                },
                side_effect_boundary=boundary,
            )
            completed = store.complete(
                job.job_id,
                worker_id,
                result_metadata={
                    "cycle_status": cycle.status,
                    "attempt_count": len(cycle.attempts),
                    "replan_count": cycle.replan_count,
                    "final_outcome_digest": cycle.final_result.brain_run.outcome_digest,
                },
            )
            return BrainJobRunResult(status=completed.state, job=completed.to_dict(), cycle=cycle)
        except Exception as error:
            error_class = type(error).__name__
            try:
                boundary = "unknown" if execution_started else "not_started"
                store.checkpoint(
                    job.job_id,
                    worker_id,
                    phase="execution_error",
                    checkpoint={
                        **dict(job.checkpoint),
                        "error_class": error_class,
                    },
                    side_effect_boundary=boundary,
                )
                failed = store.fail(
                    job.job_id,
                    worker_id,
                    reason=(
                        "execution failed before the cycle started"
                        if not execution_started
                        else "execution outcome is uncertain; reconciliation required"
                    ),
                    retryable=False,
                )
            except (BrainJobError, BrainRunError) as persistence_error:
                raise BrainRunError("brain job failure could not be durably recorded") from persistence_error
            return BrainJobRunResult(
                status=failed.state,
                job=failed.to_dict(),
                cycle=None,
                error_class=error_class,
            )

    def run_resumable_workflow_job(
        self,
        store: "BrainJobStore",
        *,
        job_id: str,
        worker_id: str,
        resolver: Callable[[Mapping[str, Any]], Mapping[str, Any]],
        evaluator: "BrainOutcomeEvaluator | None" = None,
        bandit_state: Mapping[str, Any],
        provider_health: Mapping[str, Any] | None = None,
        model_health: Mapping[str, Any] | None = None,
        lease_seconds: float = 60.0,
        ledger: BrainLearningLedger | None = None,
        memory: BrainEpisodicMemory | None = None,
        approval_router: Any | None = None,
        approval_scope: str | None = None,
        required_approval_role: str = "operator",
        checkpoint_sink: Callable[[str, Any], Any] | None = None,
    ) -> BrainJobRunResult:
        """Execute exactly one bounded workflow continuation under a durable job lease.

        The resolver is the BYOK/process-restart boundary. It receives only the public job
        record and must rehydrate a prepared ``AutonomousTaskBlueprint``, model candidates, and
        live credential handles in memory. The job journal stores workflow identifiers, digests,
        completed stage ids, value-only bandit state, and either a bounded inline checkpoint or
        a reference posture for a caller-owned checkpoint sink. It never stores the raw task,
        prompt, provider response, credential handle, or evaluator evidence.

        One worker invocation runs at most one provider-backed stage. A successful non-terminal
        stage is checkpointed and cooperatively requeued, which makes process restart and worker
        hand-off ordinary control-plane events rather than implicit replay. Provider approval is
        parked in ``waiting_approval`` and the approval release is enforced on the rehydrated
        options before the next stage can run.
        """

        from .autonomy import (
            AutonomousPlanRefinementResult,
            AutonomousTaskBlueprint,
            AutonomousWorkflowCheckpoint,
        )
        from .control_plane import BrainApprovalRouter
        from .jobs import BrainJobError, BrainJobStore, MAX_JOB_CHECKPOINT_BYTES

        if not isinstance(store, BrainJobStore):
            raise BrainRunError("store must be a BrainJobStore")
        if not callable(resolver):
            raise BrainRunError("resolver must be callable")
        if evaluator is not None and not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("evaluator must be a BrainOutcomeEvaluator or None")
        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("bandit_state must be a mapping")
        BrainLearningLedger._assert_safe(bandit_state)
        if provider_health is not None:
            if not isinstance(provider_health, Mapping):
                raise BrainRunError("provider_health must be a mapping or None")
            BrainLearningLedger._assert_safe(provider_health)
        if model_health is not None:
            if not isinstance(model_health, Mapping):
                raise BrainRunError("model_health must be a mapping or None")
            BrainLearningLedger._assert_safe(model_health)
        if not isinstance(lease_seconds, (int, float)) or isinstance(lease_seconds, bool) or not 1 <= lease_seconds <= 86_400:
            raise BrainRunError("lease_seconds must be within [1, 86400]")
        if approval_router is None:
            approval_router = BrainApprovalRouter(store)
        elif not isinstance(approval_router, BrainApprovalRouter):
            raise BrainRunError("approval_router must be a BrainApprovalRouter or None")
        if approval_scope is not None and (
            not isinstance(approval_scope, str)
            or not approval_scope.strip()
            or len(approval_scope.encode("utf-8")) > 512
        ):
            raise BrainRunError("approval_scope must be a bounded non-empty string or None")
        if (
            not isinstance(required_approval_role, str)
            or not required_approval_role.strip()
            or len(required_approval_role.encode("utf-8")) > 128
        ):
            raise BrainRunError("required_approval_role must be a bounded non-empty string")
        if checkpoint_sink is not None and not callable(checkpoint_sink):
            raise BrainRunError("checkpoint_sink must be callable or None")

        try:
            job = store.claim(job_id, worker_id, lease_seconds=lease_seconds)
        except BrainJobError as error:
            raise BrainRunError("brain workflow job claim failed") from error
        if job.terminal:
            return BrainJobRunResult(status="already_terminal", job=job.to_dict(), cycle=None, workflow=None)

        execution_started = False
        workflow_result: Any | None = None
        current_boundary = job.side_effect_boundary

        def _checkpoint_digest(value: Mapping[str, Any]) -> str:
            return _json_digest(value)

        def _persist_workflow_state(
            current_job: Any,
            result: Any,
            *,
            phase: str,
            side_effect_boundary: str = "preflight",
        ) -> Any:
            workflow_run = result.workflow
            checkpoint = workflow_run.checkpoint
            checkpoint_dict = checkpoint.to_dict()
            checkpoint_digest = checkpoint.checkpoint_digest
            state = result.bandit_state
            if not isinstance(state, Mapping):
                raise BrainRunError("workflow learning returned a non-mapping bandit state")
            BrainLearningLedger._assert_safe(state)
            metadata: dict[str, Any] = {
                "job_kind": "autonomous_workflow",
                "workflow_id": workflow_run.blueprint.workflow.workflow_id,
                "workflow_digest": workflow_run.blueprint.workflow.workflow_digest,
                "workflow_run_id": workflow_run.run_id,
                "workflow_checkpoint_digest": checkpoint_digest,
                "completed_stage_ids": list(checkpoint.completed_stage_ids),
                "next_stage_ids": list(workflow_run.next_stage_ids),
                "workflow_status": result.status,
                "bandit_state": dict(state),
                "stage_evaluation_count": len(result.evaluations),
                "accepted_plan_refinement_digest": checkpoint.plan_refinement_digest,
            }
            inline_candidate = {**metadata, "checkpoint_storage": "inline", "workflow_checkpoint": checkpoint_dict}
            encoded_size = len(
                json.dumps(inline_candidate, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
            )
            # Approval and cooperative-release transitions append a small amount of metadata to
            # the same job record. Keep headroom so a valid inline checkpoint cannot become
            # unpersistable merely because an operator approves it or a worker releases it.
            inline_limit = MAX_JOB_CHECKPOINT_BYTES - 8_192
            if encoded_size <= inline_limit:
                persisted = inline_candidate
            else:
                if checkpoint_sink is None:
                    raise BrainRunError(
                        "workflow checkpoint exceeds the job journal bound; configure checkpoint_sink for caller-owned persistence"
                    )
                checkpoint_sink(current_job.job_id, checkpoint)
                persisted = {**metadata, "checkpoint_storage": "caller_owned"}
            return store.checkpoint(
                current_job.job_id,
                worker_id,
                phase=phase,
                checkpoint=persisted,
                side_effect_boundary=side_effect_boundary,
            )

        try:
            previous_checkpoint = job.checkpoint
            previous_kind = previous_checkpoint.get("job_kind")
            if previous_kind is not None and previous_kind != "autonomous_workflow":
                raise BrainRunError("job checkpoint belongs to a different execution kind")
            approval_released = previous_checkpoint.get("phase") == "approval_released"
            resolving = store.checkpoint(
                job.job_id,
                worker_id,
                phase="resolving_workflow",
                checkpoint={
                    **dict(previous_checkpoint),
                    "job_kind": "autonomous_workflow",
                    "spec_digest": job.spec_digest,
                    "attempt": job.attempts,
                },
                side_effect_boundary=current_boundary,
            )
            resolved = resolver(resolving.to_dict())
            if not isinstance(resolved, Mapping):
                raise BrainRunError("workflow job resolver must return a mapping")
            allowed = {"blueprint", "model_candidates", "credentials", "checkpoint", "workflow_options"}
            unknown = sorted(set(resolved).difference(allowed))
            if unknown:
                raise BrainRunError("workflow job resolver returned unsupported fields: " + ", ".join(unknown))
            required = {"blueprint", "model_candidates", "credentials"}
            missing = sorted(required.difference(resolved))
            if missing:
                raise BrainRunError("workflow job resolver omitted required fields: " + ", ".join(missing))
            blueprint = resolved["blueprint"]
            if not isinstance(blueprint, AutonomousTaskBlueprint):
                raise BrainRunError("workflow job blueprint must be an AutonomousTaskBlueprint")
            options = resolved.get("workflow_options", {})
            if not isinstance(options, Mapping):
                raise BrainRunError("workflow_options must be a mapping")
            options = dict(options)
            allowed_options = {
                "retry_blocked", "stage_execution_mode", "memory_query", "memory_limit",
                "contextual_observations", "content_parts", "input_tokens", "requested_output_tokens", "max_cost_per_million_tokens",
                "max_latency_ms", "min_quality", "selection_overrides", "approve_provider_call",
                "approve_mission_dispatch", "run_id", "max_output_tokens", "temperature", "idempotency_key",
                "mission_policy", "mission_options", "route_request", "auto_route", "enforce_route_tools",
                "require_resolved_route", "provider_tools", "tool_choice", "max_provider_failovers",
                "tool_loop_options", "stage_evidence", "memory_tags", "resume_after_replan", "max_stage_calls",
                "accepted_plan_refinement",
            }
            unknown_options = sorted(set(options).difference(allowed_options))
            if unknown_options:
                raise BrainRunError("workflow_options contains unsupported fields: " + ", ".join(unknown_options))
            resume_after_replan = bool(options.pop("resume_after_replan", False))
            if previous_checkpoint.get("workflow_status") == "learning_replan_requested" and not resume_after_replan:
                raise BrainRunError(
                    "workflow learning requested a replan; resolver must explicitly set resume_after_replan"
                )
            checkpoint_value = resolved.get("checkpoint")
            if checkpoint_value is None:
                checkpoint_value = previous_checkpoint.get("workflow_checkpoint")
            if checkpoint_value is None and previous_checkpoint.get("checkpoint_storage") == "caller_owned":
                raise BrainRunError("resolver did not rehydrate the caller-owned workflow checkpoint")
            if checkpoint_value is not None and not isinstance(checkpoint_value, AutonomousWorkflowCheckpoint):
                if not isinstance(checkpoint_value, Mapping):
                    raise BrainRunError("workflow resolver checkpoint must be a checkpoint or mapping")
                checkpoint_value = AutonomousWorkflowCheckpoint.from_dict(checkpoint_value)
            expected_checkpoint_digest = previous_checkpoint.get("workflow_checkpoint_digest")
            if checkpoint_value is not None and expected_checkpoint_digest is not None:
                if checkpoint_value.checkpoint_digest != expected_checkpoint_digest:
                    raise BrainRunError("rehydrated workflow checkpoint digest does not match the job journal")
            if checkpoint_value is not None:
                options["checkpoint"] = checkpoint_value
            else:
                options.setdefault("run_id", f"job-{job.job_id}")
            if options.get("max_stage_calls") not in (None, 1):
                raise BrainRunError("durable workflow jobs execute at most one stage per lease")
            options["max_stage_calls"] = 1
            accepted_plan = options.get("accepted_plan_refinement")
            if accepted_plan is not None and not isinstance(accepted_plan, AutonomousPlanRefinementResult):
                raise BrainRunError(
                    "workflow_options.accepted_plan_refinement must be an AutonomousPlanRefinementResult"
                )
            if approval_released:
                options["approve_provider_call"] = True
                if str(previous_checkpoint.get("approval_scope", "")).endswith(":mission_dispatch"):
                    options["approve_mission_dispatch"] = True
            if provider_health is not None:
                overrides = options.get("selection_overrides", {})
                if not isinstance(overrides, Mapping):
                    raise BrainRunError("workflow_options.selection_overrides must be a mapping")
                merged_overrides = dict(overrides)
                prior_health = merged_overrides.get("provider_health", {})
                if not isinstance(prior_health, Mapping):
                    raise BrainRunError("workflow_options.provider_health must be a mapping")
                merged_health = dict(prior_health)
                for provider, snapshot in provider_health.items():
                    if not isinstance(provider, str) or not isinstance(snapshot, Mapping):
                        raise BrainRunError("provider_health must map provider names to objects")
                    merged_health[provider] = dict(snapshot)
                merged_overrides["provider_health"] = merged_health
                if model_health is not None:
                    prior_model_health = merged_overrides.get("model_health", {})
                    if not isinstance(prior_model_health, Mapping):
                        raise BrainRunError("workflow_options.model_health must be a mapping")
                    merged_model_health = dict(prior_model_health)
                    for arm_id, snapshot in model_health.items():
                        if not isinstance(arm_id, str) or not isinstance(snapshot, Mapping):
                            raise BrainRunError("model_health must map provider/model ids to objects")
                        merged_model_health[arm_id] = dict(snapshot)
                    merged_overrides["model_health"] = merged_model_health
                options["selection_overrides"] = merged_overrides
            elif model_health is not None:
                merged_overrides = dict(options.get("selection_overrides", {}))
                prior_model_health = merged_overrides.get("model_health", {})
                if not isinstance(prior_model_health, Mapping):
                    raise BrainRunError("workflow_options.model_health must be a mapping")
                merged_model_health = dict(prior_model_health)
                for arm_id, snapshot in model_health.items():
                    if not isinstance(arm_id, str) or not isinstance(snapshot, Mapping):
                        raise BrainRunError("model_health must map provider/model ids to objects")
                    merged_model_health[arm_id] = dict(snapshot)
                merged_overrides["model_health"] = merged_model_health
                options["selection_overrides"] = merged_overrides
            store.checkpoint(
                job.job_id,
                worker_id,
                phase="workflow_stage_started",
                checkpoint={
                    **dict(resolving.checkpoint),
                    "workflow_id": blueprint.workflow.workflow_id,
                    "workflow_digest": blueprint.workflow.workflow_digest,
                    "accepted_plan_refinement_digest": None
                    if accepted_plan is None
                    else _json_digest(accepted_plan.to_dict()),
                    "workflow_run_id": options.get("run_id") or (
                        checkpoint_value.run_id if checkpoint_value is not None else f"job-{job.job_id}"
                    ),
                },
                side_effect_boundary="preflight",
            )
            execution_started = True
            workflow_result = self.run_workflow_learning(
                blueprint=blueprint,
                model_candidates=resolved["model_candidates"],
                credentials=resolved["credentials"],
                bandit_state=previous_checkpoint.get("bandit_state", bandit_state),
                evaluator=evaluator,
                ledger=ledger,
                memory=memory if memory is not None else self.memory,
                **options,
            )
            _persist_workflow_state(
                job,
                workflow_result,
                phase="workflow_stage_checkpointed",
                side_effect_boundary="preflight",
            )
            workflow_run = workflow_result.workflow
            if workflow_run.status == "approval_required":
                request_digest = _checkpoint_digest(
                    {
                        "workflow_id": workflow_run.blueprint.workflow.workflow_id,
                        "run_id": workflow_run.run_id,
                        "checkpoint_digest": workflow_run.checkpoint.checkpoint_digest,
                        "next_stage_ids": list(workflow_run.next_stage_ids),
                    }
                )
                stage_result = workflow_run.stage_results[-1] if workflow_run.stage_results else None
                raw_status = None if stage_result is None or stage_result.result is None else stage_result.result.status
                scope_suffix = "mission_dispatch" if raw_status == "mission_approval_required" else "provider_call"
                effective_scope = approval_scope or f"{job.domain}:{job.capability}:{job.risk_class}:{scope_suffix}"
                approval_router.request(
                    job.job_id,
                    worker_id,
                    approval_scope=effective_scope,
                    request_digest=request_digest,
                    required_role=required_approval_role,
                )
                waiting = store.get(job.job_id)
                if waiting is None:
                    raise BrainRunError("workflow approval-waiting job disappeared from the durable store")
                return BrainJobRunResult(
                    status="waiting_approval",
                    job=waiting.to_dict(),
                    cycle=None,
                    workflow=workflow_result,
                )
            if workflow_result.status == "completed" and workflow_run.status == "completed":
                completed = store.complete(
                    job.job_id,
                    worker_id,
                    result_metadata={
                        "job_kind": "autonomous_workflow",
                        "workflow_id": workflow_run.blueprint.workflow.workflow_id,
                        "workflow_run_id": workflow_run.run_id,
                        "workflow_status": workflow_result.status,
                        "workflow_checkpoint_digest": workflow_run.checkpoint.checkpoint_digest,
                        "completed_stage_ids": list(workflow_run.checkpoint.completed_stage_ids),
                        "stage_evaluation_count": len(workflow_result.evaluations),
                        "accepted_plan_refinement_digest": workflow_run.checkpoint.plan_refinement_digest,
                    },
                )
                return BrainJobRunResult(
                    status=completed.state,
                    job=completed.to_dict(),
                    cycle=None,
                    workflow=workflow_result,
                )
            released = store.release(
                job.job_id,
                worker_id,
                reason=(
                    "workflow learning requested explicit replan"
                    if workflow_result.status == "learning_replan_requested"
                    else "workflow stage checkpoint persisted"
                ),
            )
            return BrainJobRunResult(
                status=workflow_result.status if workflow_result.status == "learning_replan_requested" else "queued",
                job=released.to_dict(),
                cycle=None,
                workflow=workflow_result,
            )
        except Exception as error:
            error_class = type(error).__name__
            try:
                boundary = "unknown" if execution_started else current_boundary
                current = store.get(job.job_id)
                if current is not None and current.lease_owner == worker_id and current.state in {"leased", "running"}:
                    store.checkpoint(
                        job.job_id,
                        worker_id,
                        phase="workflow_execution_error",
                        checkpoint={
                            **dict(current.checkpoint),
                            "error_class": error_class,
                        },
                        side_effect_boundary=boundary,
                    )
                    failed = store.fail(
                        job.job_id,
                        worker_id,
                        reason=(
                            "workflow execution failed before provider dispatch"
                            if not execution_started
                            else "workflow execution outcome is uncertain; reconciliation required"
                        ),
                        retryable=False,
                    )
                    return BrainJobRunResult(
                        status=failed.state,
                        job=failed.to_dict(),
                        cycle=None,
                        error_class=error_class,
                        workflow=workflow_result,
                    )
            except (BrainJobError, BrainRunError) as persistence_error:
                raise BrainRunError("workflow job failure could not be durably recorded") from persistence_error
            raise BrainRunError("workflow job execution failed") from error

    def run_resumable_cross_domain_job(
        self,
        store: "BrainJobStore",
        *,
        job_id: str,
        worker_id: str,
        resolver: Callable[[Mapping[str, Any]], Mapping[str, Any]],
        evaluator: "BrainOutcomeEvaluator | None" = None,
        bandit_state: Mapping[str, Any],
        provider_health: Mapping[str, Any] | None = None,
        model_health: Mapping[str, Any] | None = None,
        lease_seconds: float = 60.0,
        ledger: BrainLearningLedger | None = None,
        memory: BrainEpisodicMemory | None = None,
        approval_router: Any | None = None,
        approval_scope: str | None = None,
        required_approval_role: str = "operator",
    ) -> BrainJobRunResult:
        """Execute one provider-only child or synthesis step under a durable job lease.

        The resolver is the process-restart and BYOK boundary. It rehydrates the cross-domain
        blueprint, live credential handles, and caller-owned completed child results. The job
        journal stores only ordered IDs, outcome digests, plan identity, and synthesis state.
        Provider approval is parked without advancing the next item; an approved retry therefore
        cannot skip a child or replay a completed result.
        """

        from .autonomy import (
            AutonomousCrossDomainBlueprint,
            AutonomousCrossDomainCheckpoint,
            AutonomousCrossDomainPlanRefinementResult,
            AutonomousCrossDomainStepResult,
            AutonomousTaskOrchestrator,
            _cross_domain_plan_digest,
            _autonomous_result_digest,
        )
        from .control_plane import BrainApprovalRouter
        from .jobs import BrainJobError, BrainJobStore

        if not isinstance(store, BrainJobStore):
            raise BrainRunError("store must be a BrainJobStore")
        if not callable(resolver):
            raise BrainRunError("resolver must be callable")
        if evaluator is not None and not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("cross-domain durable evaluator must be a BrainOutcomeEvaluator or None")
        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("cross-domain durable bandit_state must be a mapping")
        BrainLearningLedger._assert_safe(bandit_state)
        if provider_health is not None:
            if not isinstance(provider_health, Mapping):
                raise BrainRunError("provider_health must be a mapping or None")
            BrainLearningLedger._assert_safe(provider_health)
        if model_health is not None:
            if not isinstance(model_health, Mapping):
                raise BrainRunError("model_health must be a mapping or None")
            BrainLearningLedger._assert_safe(model_health)
        if not isinstance(lease_seconds, (int, float)) or isinstance(lease_seconds, bool) or not 1 <= lease_seconds <= 86_400:
            raise BrainRunError("lease_seconds must be within [1, 86400]")
        if approval_router is None:
            approval_router = BrainApprovalRouter(store)
        elif not isinstance(approval_router, BrainApprovalRouter):
            raise BrainRunError("approval_router must be a BrainApprovalRouter or None")
        if approval_scope is not None and (
            not isinstance(approval_scope, str)
            or not approval_scope.strip()
            or len(approval_scope.encode("utf-8")) > 512
        ):
            raise BrainRunError("approval_scope must be a bounded non-empty string or None")
        if (
            not isinstance(required_approval_role, str)
            or not required_approval_role.strip()
            or len(required_approval_role.encode("utf-8")) > 128
        ):
            raise BrainRunError("required_approval_role must be a bounded non-empty string")

        try:
            job = store.claim(job_id, worker_id, lease_seconds=lease_seconds)
        except BrainJobError as error:
            raise BrainRunError("brain cross-domain job claim failed") from error
        if job.terminal:
            return BrainJobRunResult(status="already_terminal", job=job.to_dict(), cycle=None, workflow=None)

        execution_started = False
        step_result: AutonomousCrossDomainStepResult | None = None
        current_boundary = job.side_effect_boundary

        def checkpoint_metadata(
            checkpoint: AutonomousCrossDomainCheckpoint,
            *,
            phase: str,
            step: AutonomousCrossDomainStepResult | None = None,
        ) -> dict[str, Any]:
            return {
                "job_kind": "autonomous_cross_domain",
                "cross_domain_checkpoint": checkpoint.to_dict(),
                "cross_domain_checkpoint_digest": checkpoint.checkpoint_digest,
                "task_digest": checkpoint.task_digest,
                "base_plan_digest": checkpoint.base_plan_digest,
                "execution_child_ids": list(checkpoint.execution_child_ids),
                "completed_child_ids": list(checkpoint.completed_child_ids),
                "next_child_id": checkpoint.next_child_id,
                "plan_refinement_digest": checkpoint.plan_refinement_digest,
                "synthesis_result_digest": checkpoint.synthesis_result_digest,
                "cross_domain_status": checkpoint.status,
                "last_item_id": checkpoint.last_item_id if step is None else step.item_id,
                "last_item_phase": checkpoint.last_item_phase if step is None else step.phase,
                "last_item_status": checkpoint.last_item_status if step is None else step.status,
                "failure_class": checkpoint.failure_class,
                "phase": phase,
            }

        current: AutonomousCrossDomainCheckpoint | None = None
        try:
            previous_checkpoint = job.checkpoint
            previous_kind = previous_checkpoint.get("job_kind")
            if previous_kind is not None and previous_kind != "autonomous_cross_domain":
                raise BrainRunError("job checkpoint belongs to a different execution kind")
            approval_released = previous_checkpoint.get("phase") == "approval_released"
            resolving = store.checkpoint(
                job.job_id,
                worker_id,
                phase="resolving_cross_domain",
                checkpoint={
                    **dict(previous_checkpoint),
                    "job_kind": "autonomous_cross_domain",
                    "spec_digest": job.spec_digest,
                    "attempt": job.attempts,
                },
                side_effect_boundary=current_boundary,
            )
            resolved = resolver(resolving.to_dict())
            if not isinstance(resolved, Mapping):
                raise BrainRunError("cross-domain job resolver must return a mapping")
            allowed = {
                "blueprint",
                "model_candidates",
                "credentials",
                "completed_child_results",
                "checkpoint",
                "cross_domain_options",
            }
            unknown = sorted(set(resolved).difference(allowed))
            if unknown:
                raise BrainRunError("cross-domain job resolver returned unsupported fields: " + ", ".join(unknown))
            required = {"blueprint", "model_candidates", "credentials"}
            missing = sorted(required.difference(resolved))
            if missing:
                raise BrainRunError("cross-domain job resolver omitted required fields: " + ", ".join(missing))
            blueprint = resolved["blueprint"]
            if not isinstance(blueprint, AutonomousCrossDomainBlueprint):
                raise BrainRunError("cross-domain job blueprint must be an AutonomousCrossDomainBlueprint")
            if any(
                item.spec.execution_mode == "mission"
                for item in (*blueprint.child_blueprints, blueprint.synthesis_blueprint)
            ):
                raise BrainRunError(
                    "durable cross-domain jobs currently require provider or tool-loop execution; "
                    "mission effects need reconciliation-aware continuation"
                )
            options = resolved.get("cross_domain_options", {})
            if not isinstance(options, Mapping):
                raise BrainRunError("cross_domain_options must be a mapping")
            options = dict(options)
            allowed_options = {
                "ledger", "memory", "memory_query", "memory_limit", "contextual_observations", "content_parts",
                "input_tokens", "requested_output_tokens", "max_cost_per_million_tokens",
                "max_latency_ms", "min_quality", "selection_overrides", "approve_provider_call",
                "approve_mission_dispatch", "run_id", "max_output_tokens", "temperature",
                "idempotency_key", "mission_policy", "mission_options", "route_request", "auto_route",
                "enforce_route_tools", "require_resolved_route", "provider_tools", "tool_choice",
                "max_provider_failovers", "tool_loop_options", "bandit_state",
                "accepted_plan_refinement", "response_alignments", "require_response_alignment",
                "minimum_response_reward", "minimum_response_alignment_confidence",
                "response_contradiction_confidence_threshold", "retry_synthesis_after_response_review",
                "completed_synthesis_result",
            }
            unknown_options = sorted(set(options).difference(allowed_options))
            if unknown_options:
                raise BrainRunError("cross_domain_options contains unsupported fields: " + ", ".join(unknown_options))
            accepted_plan = options.get("accepted_plan_refinement")
            if accepted_plan is not None and not isinstance(
                accepted_plan,
                AutonomousCrossDomainPlanRefinementResult,
            ):
                raise BrainRunError(
                    "cross_domain_options.accepted_plan_refinement must be an AutonomousCrossDomainPlanRefinementResult"
                )
            orchestrator = AutonomousTaskOrchestrator(self)
            plan_priority, plan_digest, _ = orchestrator._accepted_cross_domain_plan(blueprint, accepted_plan)
            execution_child_ids = tuple(
                sorted(blueprint.child_ids, key=lambda child_id: plan_priority.get(child_id, len(plan_priority)))
            )
            base_plan_digest = _cross_domain_plan_digest(blueprint)
            previous_wire = previous_checkpoint.get("cross_domain_checkpoint")
            if previous_wire is None:
                current = AutonomousCrossDomainCheckpoint(
                    run_id=options.get("run_id") or f"job-{job.job_id}",
                    task_digest=blueprint.task_digest,
                    base_plan_digest=base_plan_digest,
                    execution_child_ids=execution_child_ids,
                    next_child_id=execution_child_ids[0],
                    plan_refinement_digest=plan_digest,
                )
            else:
                if not isinstance(previous_wire, Mapping):
                    raise BrainRunError("persisted cross-domain checkpoint is malformed")
                current = AutonomousCrossDomainCheckpoint.from_dict(previous_wire)
                if current.task_digest != blueprint.task_digest or current.base_plan_digest != base_plan_digest:
                    raise BrainRunError("rehydrated cross-domain blueprint does not match the job checkpoint")
                if current.execution_child_ids != execution_child_ids:
                    raise BrainRunError("rehydrated cross-domain ordering does not match the job checkpoint")
                if current.plan_refinement_digest != plan_digest:
                    raise BrainRunError("rehydrated cross-domain accepted plan does not match the job checkpoint")
            supplied_checkpoint = resolved.get("checkpoint")
            if supplied_checkpoint is not None:
                if isinstance(supplied_checkpoint, AutonomousCrossDomainCheckpoint):
                    supplied = supplied_checkpoint
                elif isinstance(supplied_checkpoint, Mapping):
                    supplied = AutonomousCrossDomainCheckpoint.from_dict(supplied_checkpoint)
                else:
                    raise BrainRunError("cross-domain resolver checkpoint must be a checkpoint or mapping")
                if supplied.checkpoint_digest != current.checkpoint_digest:
                    raise BrainRunError("rehydrated cross-domain checkpoint does not match the job journal")

            raw_results = resolved.get("completed_child_results", {})
            if not isinstance(raw_results, Mapping):
                raise BrainRunError("completed_child_results must be a mapping")
            completed_results = dict(raw_results)
            if set(completed_results) != set(current.completed_child_ids):
                raise BrainRunError("resolver must rehydrate exactly the checkpointed completed children")
            for child_id, result in completed_results.items():
                if not isinstance(result, (BrainRunResult, BrainToolLoopResult, BrainMissionResult)):
                    raise BrainRunError("completed_child_results contains an unsupported result")
                if not result.status.startswith("completed"):
                    raise BrainRunError("completed_child_results contains an incomplete result")
                if _autonomous_result_digest(result) != current.child_result_digests[child_id]:
                    raise BrainRunError(
                        f"rehydrated child result digest does not match the checkpoint for {child_id}"
                    )
            retry_synthesis_after_review = options.get("retry_synthesis_after_response_review", False)
            if not isinstance(retry_synthesis_after_review, bool):
                raise BrainRunError("cross_domain_options.retry_synthesis_after_response_review must be a boolean")
            raw_synthesis_result = options.get("completed_synthesis_result")
            if retry_synthesis_after_review:
                if current.status != "synthesis_response_review_required":
                    raise BrainRunError("retry_synthesis_after_response_review requires a post-synthesis review checkpoint")
                if raw_synthesis_result is not None:
                    raise BrainRunError("retry_synthesis_after_response_review cannot combine with a rehydrated synthesis result")
                completed_synthesis_result = None
            elif raw_synthesis_result is not None:
                if current.status != "synthesis_response_review_required":
                    raise BrainRunError("completed_synthesis_result is only valid for post-synthesis response review")
                if not isinstance(raw_synthesis_result, (BrainRunResult, BrainToolLoopResult, BrainMissionResult)):
                    raise BrainRunError("completed_synthesis_result contains an unsupported result")
                if not raw_synthesis_result.status.startswith("completed"):
                    raise BrainRunError("completed_synthesis_result contains an incomplete result")
                if current.synthesis_result_digest != _autonomous_result_digest(raw_synthesis_result):
                    raise BrainRunError("rehydrated synthesis result digest does not match the checkpoint")
                completed_synthesis_result = raw_synthesis_result
            elif current.status == "synthesis_response_review_required":
                raise BrainRunError("post-synthesis response review requires completed_synthesis_result or explicit retry")
            else:
                completed_synthesis_result = None
            options["accepted_plan_refinement"] = accepted_plan
            options["run_id"] = current.run_id
            options["bandit_state"] = bandit_state
            options["ledger"] = ledger
            options["memory"] = memory if memory is not None else self.memory
            options["completed_child_results"] = completed_results
            options["completed_synthesis_result"] = completed_synthesis_result
            if approval_released:
                options["approve_provider_call"] = True
            if provider_health is not None:
                overrides = options.get("selection_overrides", {})
                if not isinstance(overrides, Mapping):
                    raise BrainRunError("cross_domain_options.selection_overrides must be a mapping")
                merged_overrides = dict(overrides)
                prior_health = merged_overrides.get("provider_health", {})
                if not isinstance(prior_health, Mapping):
                    raise BrainRunError("cross_domain_options.provider_health must be a mapping")
                merged_health = dict(prior_health)
                for provider, snapshot in provider_health.items():
                    if not isinstance(provider, str) or not isinstance(snapshot, Mapping):
                        raise BrainRunError("provider_health must map provider names to objects")
                    merged_health[provider] = dict(snapshot)
                merged_overrides["provider_health"] = merged_health
                if model_health is not None:
                    prior_model_health = merged_overrides.get("model_health", {})
                    if not isinstance(prior_model_health, Mapping):
                        raise BrainRunError("cross_domain_options.model_health must be a mapping")
                    merged_model_health = dict(prior_model_health)
                    for arm_id, snapshot in model_health.items():
                        if not isinstance(arm_id, str) or not isinstance(snapshot, Mapping):
                            raise BrainRunError("model_health must map provider/model ids to objects")
                        merged_model_health[arm_id] = dict(snapshot)
                    merged_overrides["model_health"] = merged_model_health
                options["selection_overrides"] = merged_overrides
            elif model_health is not None:
                merged_overrides = dict(options.get("selection_overrides", {}))
                prior_model_health = merged_overrides.get("model_health", {})
                if not isinstance(prior_model_health, Mapping):
                    raise BrainRunError("cross_domain_options.model_health must be a mapping")
                merged_model_health = dict(prior_model_health)
                for arm_id, snapshot in model_health.items():
                    if not isinstance(arm_id, str) or not isinstance(snapshot, Mapping):
                        raise BrainRunError("model_health must map provider/model ids to objects")
                    merged_model_health[arm_id] = dict(snapshot)
                merged_overrides["model_health"] = merged_model_health
                options["selection_overrides"] = merged_overrides
            if current.status == "synthesis_response_review_required" and retry_synthesis_after_review:
                retry_checkpoint = AutonomousCrossDomainCheckpoint(
                    run_id=current.run_id,
                    task_digest=current.task_digest,
                    base_plan_digest=current.base_plan_digest,
                    execution_child_ids=current.execution_child_ids,
                    completed_child_ids=current.completed_child_ids,
                    child_result_digests=current.child_result_digests,
                    next_child_id=None,
                    plan_refinement_digest=current.plan_refinement_digest,
                    synthesis_result_digest=None,
                    response_assessment_digest=None,
                    status="synthesis_pending",
                    generation=current.generation + 1,
                    previous_checkpoint_digest=current.checkpoint_digest,
                )
                store.checkpoint(
                    job.job_id,
                    worker_id,
                    phase="cross_domain_synthesis_response_retry_authorized",
                    checkpoint=checkpoint_metadata(retry_checkpoint, phase="cross_domain_synthesis_response_retry_authorized"),
                    side_effect_boundary="preflight",
                )
                current = retry_checkpoint
            store.checkpoint(
                job.job_id,
                worker_id,
                phase="cross_domain_step_started",
                checkpoint=checkpoint_metadata(current, phase="cross_domain_step_started"),
                side_effect_boundary="preflight",
            )
            execution_started = True
            step_result = orchestrator.run_cross_domain_step(
                blueprint=blueprint,
                model_candidates=resolved["model_candidates"],
                credentials=resolved["credentials"],
                next_child_id=current.next_child_id,
                **options,
            )
            if not isinstance(step_result, AutonomousCrossDomainStepResult):
                raise BrainRunError("cross-domain durable execution returned an unsupported step")
            if step_result.status == "response_review_required":
                assessment = step_result.response_assessment
                if assessment is None:
                    raise BrainRunError("cross-domain response review did not return an assessment")
                if current.status == "response_review_required" and current.response_assessment_digest == assessment.assessment_digest:
                    review_checkpoint = current
                else:
                    review_checkpoint = AutonomousCrossDomainCheckpoint(
                        run_id=current.run_id,
                        task_digest=current.task_digest,
                        base_plan_digest=current.base_plan_digest,
                        execution_child_ids=current.execution_child_ids,
                        completed_child_ids=step_result.completed_child_ids,
                        child_result_digests=step_result.child_result_digests,
                        next_child_id=None,
                        plan_refinement_digest=current.plan_refinement_digest,
                        response_assessment_digest=assessment.assessment_digest,
                        status="response_review_required",
                        generation=current.generation + 1,
                        previous_checkpoint_digest=current.checkpoint_digest,
                    )
                    store.checkpoint(
                        job.job_id,
                        worker_id,
                        phase="cross_domain_response_review_required",
                        checkpoint=checkpoint_metadata(review_checkpoint, phase="cross_domain_response_review_required", step=step_result),
                        side_effect_boundary="preflight",
                    )
                released = store.release(job.job_id, worker_id, reason="response admission requires explicit review before synthesis")
                return BrainJobRunResult(status="queued", job=released.to_dict(), cycle=None, workflow=step_result)
            if step_result.status == "synthesis_response_review_required":
                assessment = step_result.response_assessment
                synthesis_result = step_result.result
                if assessment is None or not isinstance(synthesis_result, (BrainRunResult, BrainToolLoopResult, BrainMissionResult)):
                    raise BrainRunError("post-synthesis response review did not return an assessment and synthesis result")
                if not synthesis_result.status.startswith("completed"):
                    raise BrainRunError("post-synthesis response review returned an incomplete synthesis result")
                synthesis_digest = _autonomous_result_digest(synthesis_result)
                if current.status == "synthesis_response_review_required" and (
                    current.synthesis_result_digest == synthesis_digest
                    and current.response_assessment_digest == assessment.assessment_digest
                ):
                    review_checkpoint = current
                else:
                    review_checkpoint = AutonomousCrossDomainCheckpoint(
                        run_id=current.run_id,
                        task_digest=current.task_digest,
                        base_plan_digest=current.base_plan_digest,
                        execution_child_ids=current.execution_child_ids,
                        completed_child_ids=step_result.completed_child_ids,
                        child_result_digests=step_result.child_result_digests,
                        next_child_id=None,
                        plan_refinement_digest=current.plan_refinement_digest,
                        synthesis_result_digest=synthesis_digest,
                        response_assessment_digest=assessment.assessment_digest,
                        status="synthesis_response_review_required",
                        generation=current.generation + 1,
                        previous_checkpoint_digest=current.checkpoint_digest,
                    )
                    store.checkpoint(
                        job.job_id,
                        worker_id,
                        phase="cross_domain_synthesis_response_review_required",
                        checkpoint=checkpoint_metadata(review_checkpoint, phase="cross_domain_synthesis_response_review_required", step=step_result),
                        side_effect_boundary="preflight",
                    )
                released = store.release(job.job_id, worker_id, reason="post-synthesis response review requires explicit resolution or retry")
                return BrainJobRunResult(status="queued", job=released.to_dict(), cycle=None, workflow=step_result)
            if step_result.status in {"approval_required", "mission_approval_required"}:
                approval_checkpoint = AutonomousCrossDomainCheckpoint(
                    run_id=current.run_id,
                    task_digest=current.task_digest,
                    base_plan_digest=current.base_plan_digest,
                    execution_child_ids=current.execution_child_ids,
                    completed_child_ids=step_result.completed_child_ids,
                    child_result_digests=step_result.child_result_digests,
                    next_child_id=current.next_child_id,
                    plan_refinement_digest=current.plan_refinement_digest,
                    status="approval_required",
                    generation=current.generation + 1,
                    previous_checkpoint_digest=current.checkpoint_digest,
                )
                request_digest = _json_digest(
                    {
                        "job_kind": "autonomous_cross_domain",
                        "checkpoint_digest": approval_checkpoint.checkpoint_digest,
                        "item_id": step_result.item_id,
                        "phase": step_result.phase,
                    }
                )
                store.checkpoint(
                    job.job_id,
                    worker_id,
                    phase="cross_domain_approval_required",
                    checkpoint=checkpoint_metadata(
                        approval_checkpoint,
                        phase="cross_domain_approval_required",
                        step=step_result,
                    ),
                    side_effect_boundary="preflight",
                )
                effective_scope = approval_scope or f"{job.domain}:{job.capability}:{job.risk_class}:provider_call"
                approval_router.request(
                    job.job_id,
                    worker_id,
                    approval_scope=effective_scope,
                    request_digest=request_digest,
                    required_role=required_approval_role,
                )
                waiting = store.get(job.job_id)
                if waiting is None:
                    raise BrainRunError("cross-domain approval-waiting job disappeared from the durable store")
                return BrainJobRunResult(status="waiting_approval", job=waiting.to_dict(), cycle=None, workflow=step_result)
            if step_result.status == "reconciliation_required":
                item_phase = step_result.phase
                reconciliation_checkpoint = AutonomousCrossDomainCheckpoint(
                    run_id=current.run_id,
                    task_digest=current.task_digest,
                    base_plan_digest=current.base_plan_digest,
                    execution_child_ids=current.execution_child_ids,
                    completed_child_ids=current.completed_child_ids,
                    child_result_digests=current.child_result_digests,
                    next_child_id=current.next_child_id,
                    plan_refinement_digest=current.plan_refinement_digest,
                    status="reconciliation_required",
                    last_item_id=step_result.item_id,
                    last_item_phase=item_phase,
                    last_item_status=step_result.status,
                    failure_class="result_reconciliation_required",
                    generation=current.generation + 1,
                    previous_checkpoint_digest=current.checkpoint_digest,
                )
                store.checkpoint(
                    job.job_id,
                    worker_id,
                    phase="cross_domain_reconciliation_required",
                    checkpoint=checkpoint_metadata(
                        reconciliation_checkpoint,
                        phase="cross_domain_reconciliation_required",
                        step=step_result,
                    ),
                    side_effect_boundary="unknown",
                )
                failed = store.fail(
                    job.job_id,
                    worker_id,
                    reason=f"cross-domain {step_result.phase} {step_result.item_id} requires reconciliation",
                    retryable=False,
                )
                return BrainJobRunResult(
                    status=failed.state,
                    job=failed.to_dict(),
                    cycle=None,
                    error_class="reconciliation_required",
                    workflow=step_result,
                )
            if not step_result.status.startswith("completed"):
                failed = store.fail(
                    job.job_id,
                    worker_id,
                    reason=f"cross-domain {step_result.phase} {step_result.item_id} did not complete",
                    retryable=False,
                )
                return BrainJobRunResult(
                    status=failed.state,
                    job=failed.to_dict(),
                    cycle=None,
                    workflow=step_result,
                )
            if step_result.phase == "child":
                is_last_child = len(step_result.completed_child_ids) == len(current.execution_child_ids)
                next_child = None if is_last_child else current.execution_child_ids[len(step_result.completed_child_ids)]
                next_checkpoint = AutonomousCrossDomainCheckpoint(
                    run_id=current.run_id,
                    task_digest=current.task_digest,
                    base_plan_digest=current.base_plan_digest,
                    execution_child_ids=current.execution_child_ids,
                    completed_child_ids=step_result.completed_child_ids,
                    child_result_digests=step_result.child_result_digests,
                    next_child_id=next_child,
                    plan_refinement_digest=current.plan_refinement_digest,
                    response_assessment_digest=None,
                    status="synthesis_pending" if is_last_child else "children_pending",
                    generation=current.generation + 1,
                    previous_checkpoint_digest=current.checkpoint_digest,
                )
                store.checkpoint(
                    job.job_id,
                    worker_id,
                    phase="cross_domain_child_checkpointed",
                    checkpoint=checkpoint_metadata(next_checkpoint, phase="cross_domain_child_checkpointed", step=step_result),
                    side_effect_boundary="preflight",
                )
                released = store.release(job.job_id, worker_id)
                return BrainJobRunResult(
                    status="queued",
                    job=released.to_dict(),
                    cycle=None,
                    workflow=step_result,
                )
            synthesis_checkpoint = AutonomousCrossDomainCheckpoint(
                run_id=current.run_id,
                task_digest=current.task_digest,
                base_plan_digest=current.base_plan_digest,
                execution_child_ids=current.execution_child_ids,
                completed_child_ids=current.completed_child_ids,
                child_result_digests=current.child_result_digests,
                next_child_id=None,
                plan_refinement_digest=current.plan_refinement_digest,
                synthesis_result_digest=_autonomous_result_digest(step_result.result),
                response_assessment_digest=None if step_result.response_assessment is None else step_result.response_assessment.assessment_digest,
                status="completed",
                generation=current.generation + 1,
                previous_checkpoint_digest=current.checkpoint_digest,
            )
            completed = store.complete(
                job.job_id,
                worker_id,
                result_metadata=checkpoint_metadata(synthesis_checkpoint, phase="completed", step=step_result),
            )
            return BrainJobRunResult(status=completed.state, job=completed.to_dict(), cycle=None, workflow=step_result)
        except Exception as error:
            error_class = type(error).__name__
            try:
                boundary = "unknown" if execution_started else current_boundary
                current_job = store.get(job.job_id)
                if current_job is not None and current_job.lease_owner == worker_id and current_job.state in {"leased", "running"}:
                    checkpoint_phase = "cross_domain_execution_error"
                    if execution_started and current is not None:
                        item_id = current.next_child_id
                        item_phase = "child"
                        if item_id is None and len(current.completed_child_ids) == len(current.execution_child_ids):
                            item_id = "synthesis"
                            item_phase = "synthesis"
                        if item_id is None:
                            raise BrainRunError("cross-domain uncertain boundary has no identifiable next item")
                        reconciliation_checkpoint = AutonomousCrossDomainCheckpoint(
                            run_id=current.run_id,
                            task_digest=current.task_digest,
                            base_plan_digest=current.base_plan_digest,
                            execution_child_ids=current.execution_child_ids,
                            completed_child_ids=current.completed_child_ids,
                            child_result_digests=current.child_result_digests,
                            next_child_id=current.next_child_id,
                            plan_refinement_digest=current.plan_refinement_digest,
                            status="reconciliation_required",
                            last_item_id=item_id,
                            last_item_phase=item_phase,
                            last_item_status="execution_uncertain",
                            failure_class=error_class,
                            generation=current.generation + 1,
                            previous_checkpoint_digest=current.checkpoint_digest,
                        )
                        reconciliation_metadata = checkpoint_metadata(
                            reconciliation_checkpoint,
                            phase="cross_domain_reconciliation_required",
                            step=step_result,
                        )
                        checkpoint_phase = "cross_domain_reconciliation_required"
                    else:
                        reconciliation_metadata = {
                            **dict(current_job.checkpoint),
                            "phase": "cross_domain_execution_error",
                            "error_class": error_class,
                        }
                    store.checkpoint(
                        job.job_id,
                        worker_id,
                        phase=checkpoint_phase,
                        checkpoint=reconciliation_metadata,
                        side_effect_boundary="unknown" if execution_started else boundary,
                    )
                    failed = store.fail(
                        job.job_id,
                        worker_id,
                        reason=(
                            "cross-domain execution failed before provider dispatch"
                            if not execution_started
                            else "cross-domain execution outcome is uncertain; reconciliation required"
                        ),
                        retryable=False,
                    )
                    return BrainJobRunResult(
                        status=failed.state,
                        job=failed.to_dict(),
                        cycle=None,
                        error_class=error_class,
                        workflow=step_result,
                    )
            except (BrainJobError, BrainRunError) as persistence_error:
                raise BrainRunError("cross-domain job failure could not be durably recorded") from persistence_error
            raise BrainRunError("cross-domain job execution failed") from error

    def run(
        self,
        *,
        task: str,
        model_selection: Mapping[str, Any],
        selection_override: Mapping[str, Any] | None = None,
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        approve_provider_call: bool = False,
        run_id: str | None = None,
        max_output_tokens: int = 1024,
        temperature: float | None = None,
        require_json: bool = False,
        response_schema: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        context: Mapping[str, Any] | None = None,
        content_parts: Sequence[ProviderContentPart | Mapping[str, Any]] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        tools: Sequence[ProviderTool] = (),
        tool_choice: str | None = None,
        invocation_observer: ProviderInvocationObserver | None = None,
    ) -> BrainRunResult:
        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("task must be a non-empty string")
        if not isinstance(tools, Sequence) or isinstance(tools, (str, bytes)):
            raise BrainRunError("tools must be a sequence")
        if any(not isinstance(tool, ProviderTool) for tool in tools):
            raise BrainRunError("tools must contain ProviderTool values")
        normalized_content_parts = (
            () if content_parts is None else normalize_provider_content_parts(content_parts)
        )
        resolved_run_id = run_id or f"brain-{uuid.uuid4().hex}"
        if not isinstance(resolved_run_id, str) or not resolved_run_id.strip() or len(resolved_run_id) > 256:
            raise BrainRunError("run_id must be a bounded non-empty string")
        selection_args = dict(model_selection)
        selection_args["task"] = task
        if selection_override is not None:
            if not isinstance(selection_override, Mapping):
                raise BrainRunError("selection_override must be a mapping")
            selection = dict(selection_override)
            BrainLearningLedger._assert_safe(selection)
            override_selected = selection.get("selected_model")
            if not isinstance(override_selected, Mapping):
                raise BrainRunError("selection_override must contain selected_model metadata")
            override_provider = override_selected.get("provider")
            override_model = override_selected.get("model")
            if not isinstance(override_provider, str) or not override_provider.strip() or not isinstance(override_model, str) or not override_model.strip():
                raise BrainRunError("selection_override selected_model metadata is malformed")
            override_models = selection.get("models")
            if not isinstance(override_models, Sequence) or isinstance(override_models, (str, bytes)):
                raise BrainRunError("selection_override must contain a model catalogue")
            if not any(
                isinstance(candidate, Mapping)
                and candidate.get("provider") == override_provider
                and candidate.get("model") == override_model
                for candidate in override_models
            ):
                raise BrainRunError("selection_override selected model is absent from its catalogue")
        elif context is None:
            if contextual_observations:
                raise BrainRunError("contextual_observations require a context mapping")
            selection = self.workspace.tool("brain_model_select", selection_args)
        else:
            if not isinstance(context, Mapping):
                raise BrainRunError("context must be a mapping")
            BrainLearningLedger._assert_safe(context)
            if not isinstance(contextual_observations, Sequence) or isinstance(
                contextual_observations, (str, bytes)
            ):
                raise BrainRunError("contextual_observations must be a sequence")
            if any(not isinstance(observation, Mapping) for observation in contextual_observations):
                raise BrainRunError("contextual_observations must contain mappings")
            BrainLearningLedger._assert_safe(list(contextual_observations))
            contextual_report = self.workspace.tool(
                "brain_model_select_contextual",
                {
                    "context": dict(context),
                    "base": selection_args,
                    "observations": [dict(observation) for observation in contextual_observations],
                },
            )
            nested_selection = contextual_report.get("selection")
            if not isinstance(nested_selection, Mapping):
                raise BrainRunError("contextual model selection did not produce a selection report")
            context_digest = contextual_report.get("context_digest")
            if not _valid_digest(context_digest):
                raise BrainRunError("contextual model selection returned an invalid context digest")
            normalized_context = _normalize_learning_context(context)
            expected_context_digest = _context_identity_digest(normalized_context)
            if context_digest != expected_context_digest:
                raise BrainRunError(
                    "contextual model selection returned a context digest that does not match its identity"
                )
            selection = dict(nested_selection)
            selection["context_digest"] = context_digest
            # The caller may provide a rich autonomy blueprint here, but the durable learning
            # contract must carry only the canonical four-field identity. Rich metadata remains
            # in the blueprint and is never allowed to become part of a replay key.
            selection["context"] = normalized_context
            selection["contextual_selection_status"] = contextual_report.get("selection_status")
        selection = dict(selection)
        selection["selection_audit"] = build_model_selection_audit(selection)
        if isinstance(selection_args.get("provider_health"), Mapping):
            selection["provider_health"] = dict(selection_args["provider_health"])
        selected = selection.get("selected_model")
        if not isinstance(selected, Mapping):
            raise BrainRunError("model selection did not produce an eligible model")
        provider = selected.get("provider")
        model = selected.get("model")
        if not isinstance(provider, str) or not provider or not isinstance(model, str) or not model:
            raise BrainRunError("model selection returned malformed provider/model metadata")

        prompt_args = dict(prompt)
        prompt_args["task"] = task
        prompt_override = prompt_args.pop("_provider_messages_override", None)
        override_metadata: Mapping[str, Any] | None = None
        if prompt_override is not None:
            if not isinstance(prompt_override, Mapping):
                raise BrainRunError("provider prompt override must be a mapping")
            raw_messages = prompt_override.get("messages")
            if not isinstance(raw_messages, Sequence) or isinstance(raw_messages, (str, bytes)) or not raw_messages:
                raise BrainRunError("provider prompt override must contain a non-empty message sequence")
            if any(not isinstance(message, Mapping) for message in raw_messages):
                raise BrainRunError("provider prompt override messages must contain mappings")
            override_metadata_value = prompt_override.get("metadata", {})
            if not isinstance(override_metadata_value, Mapping):
                raise BrainRunError("provider prompt override metadata must be a mapping")
            try:
                messages = [dict(message) for message in raw_messages]
                prompt_digest = _json_digest(messages)
            except (TypeError, ValueError) as error:
                raise BrainRunError("provider prompt override must be JSON-safe") from error
            override_metadata = dict(override_metadata_value)
            prompt_report = {
                "schema": "bioprism-python-autonomous-prompt-override/0.1",
                "messages": messages,
                "prompt_digest": prompt_digest,
                "autonomous_prompt": override_metadata,
                "retention": "prompt_messages_transient;digest_only_projection",
                "secret_material": "never_returned",
            }
        else:
            prompt_report = self.workspace.tool("brain_prompt_assemble", prompt_args)
            messages = prompt_report.get("messages")
            if not isinstance(messages, list) or not messages:
                raise BrainRunError("prompt assembly did not produce messages")

        plan_args = dict(plan)
        plan_args.setdefault("objective", task)
        plan_report = self.workspace.tool("brain_plan", plan_args)
        if not plan_report.get("ok", False):
            return self._result(resolved_run_id, "plan_refused", selection, prompt_report, plan_report, None)
        planned = plan_report.get("plan")
        if not isinstance(planned, Mapping):
            raise BrainRunError("brain plan reported success without a plan")
        if planned.get("requires_approval", False) and not approve_provider_call:
            return self._result(resolved_run_id, "approval_required", selection, prompt_report, plan_report, None)
        if not approve_provider_call and any(
            isinstance(step, Mapping) and step.get("effect") == "provider_call"
            for step in planned.get("steps", [])
        ):
            return self._result(resolved_run_id, "approval_required", selection, prompt_report, plan_report, None)

        handle = credentials.get(provider)
        if self.runtime.provider_requires_credential(provider) and handle is None:
            raise BrainRunError(f"no user credential handle was supplied for provider {provider!r}")
        if handle is not None and handle.provider != provider:
            raise BrainRunError(f"credential handle does not belong to provider {provider!r}")
        provider_messages = _provider_messages_with_content_parts(messages, normalized_content_parts)
        effective_idempotency_key = idempotency_key
        if prompt_override is not None:
            prompt_report = {
                **prompt_report,
                "prompt_digest": _json_digest(provider_messages),
            }
            effective_idempotency_key = _prompt_bound_idempotency_key(
                idempotency_key,
                prompt_digest=prompt_report["prompt_digest"],
                metadata=override_metadata,
            )
        request = ProviderRequest(
            model=model,
            messages=provider_messages,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            require_json=require_json,
            response_schema=response_schema,
            idempotency_key=effective_idempotency_key,
            tools=tuple(tools),
            tool_choice=tool_choice,
        )
        response = self.runtime.invoke(
            provider,
            request,
            credential=handle,
            invocation_observer=invocation_observer,
            invocation_kind="provider_call",
        )
        invocations = ()
        if isinstance(invocation_observer, AutonomousProviderInvocationSession):
            invocations = tuple(invocation_observer.evidence())
        return self._result(
            resolved_run_id,
            "completed_provider_call",
            selection,
            prompt_report,
            plan_report,
            response,
            provider_invocations=invocations,
        )

    def run_tool_loop(
        self,
        *,
        task: str,
        model_selection: Mapping[str, Any],
        selection_override: Mapping[str, Any] | None = None,
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        authorize_and_execute: Callable[[tuple[ProviderToolCall, ...]], Sequence[ProviderToolResult]] | None = None,
        approve_provider_call: bool = False,
        run_id: str | None = None,
        max_output_tokens: int = 2048,
        temperature: float | None = None,
        require_json: bool = False,
        response_schema: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        context: Mapping[str, Any] | None = None,
        content_parts: Sequence[ProviderContentPart | Mapping[str, Any]] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        provider_tools: Sequence[ProviderTool] = (),
        tool_choice: str | None = None,
        max_turns: int = 4,
        max_tool_calls: int = 128,
        stream: bool = False,
        mission_policy: MissionPolicy | Mapping[str, Any] | None = None,
        approve_mission_dispatch: bool = False,
        route_request: Mapping[str, Any] | None = None,
        enforce_route_tools: bool = True,
        require_resolved_route: bool = True,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
        operations_gate_acceptance: Mapping[str, Any] | None = None,
        route_report: Mapping[str, Any] | None = None,
        attempt_state: dict[str, Any] | None = None,
        invocation_observer: ProviderInvocationObserver | None = None,
    ) -> BrainToolLoopResult:
        """Run the planned provider call and continue only through caller-approved tool results.

        This method is the high-level bridge for applications that want native function calling
        without converting every turn into a mission. The initial model decision still passes
        through ``brain_plan`` and provider approval. The callback is intentionally typed and
        explicit: it may invoke a caller-owned mission/executor, but the brain and provider
        runtime never do so implicitly.
        """

        if authorize_and_execute is not None and not callable(authorize_and_execute):
            raise BrainRunError("authorize_and_execute must be callable")
        if attempt_state is not None and not isinstance(attempt_state, dict):
            raise BrainRunError("attempt_state must be a mutable mapping")
        if attempt_state is not None:
            attempt_state["tool_authorization_started"] = False
        normalized_content_parts = (
            () if content_parts is None else normalize_provider_content_parts(content_parts)
        )
        if not isinstance(provider_tools, Sequence) or isinstance(provider_tools, (str, bytes)):
            raise BrainRunError("provider_tools must be a sequence")
        if any(not isinstance(tool, ProviderTool) for tool in provider_tools):
            raise BrainRunError("provider_tools must contain ProviderTool values")
        if not isinstance(stream, bool):
            raise BrainRunError("stream must be a boolean")
        if not isinstance(enforce_route_tools, bool) or not isinstance(require_resolved_route, bool):
            raise BrainRunError("route enforcement flags must be booleans")
        if route_report is not None:
            if route_request is None:
                raise BrainRunError("route_report requires route_request")
            if not isinstance(route_report, Mapping):
                raise BrainRunError("route_report must be a mapping")
            BrainLearningLedger._assert_safe(route_report)
        prompt_request = dict(prompt)
        route: dict[str, Any] | None = None
        raw_route: dict[str, Any] | None = None
        if route_request is not None:
            if not isinstance(route_request, Mapping):
                raise BrainRunError("route_request must be a mapping")
            BrainLearningLedger._assert_safe(route_request)
            route_arguments = dict(route_request)
            supplied_goal = route_arguments.get("goal")
            if supplied_goal is not None and supplied_goal != task:
                raise BrainRunError("route_request.goal must match the tool-loop task")
            route_arguments["goal"] = task
            route_arguments.setdefault("needs", [{"id": "task", "query": task}])
            route_arguments.setdefault("include_tools", True)
            route_arguments.setdefault("max_tools", 128)
            try:
                encoded_route_request = json.dumps(
                    route_arguments,
                    ensure_ascii=False,
                    sort_keys=True,
                    separators=(",", ":"),
                    allow_nan=False,
                ).encode("utf-8")
            except (TypeError, ValueError) as error:
                raise BrainRunError("route_request must be JSON-safe") from error
            if len(encoded_route_request) > MAX_ROUTE_REQUEST_BYTES:
                raise BrainRunError("route_request exceeds the bounded size")
            route_response = (
                dict(route_report)
                if route_report is not None
                else self.workspace.tool("capability_route", route_arguments)
            )
            if not isinstance(route_response, Mapping):
                raise BrainRunError("capability route returned a non-object")
            if route_response.get("ok") is False or route_response.get("workflow") != "capability_route":
                raise BrainRunError("capability route was refused")
            raw_route = dict(route_response)
            BrainLearningLedger._assert_safe(raw_route)
            unresolved = raw_route.get("unresolved_needs", [])
            if not isinstance(unresolved, list) or any(not isinstance(item, str) for item in unresolved):
                raise BrainRunError("capability route returned malformed unresolved_needs")
            if unresolved and require_resolved_route:
                raise BrainRunError("capability route contains unresolved needs: " + ", ".join(unresolved))
            route_context = _bounded_route_prompt_context(raw_route)
            route = dict(route_context)
            route.update(
                {
                    "ok": True,
                    "workflow": "capability_route",
                    "evidence_digest": raw_route.get("evidence_digest"),
                    "unresolved_needs": list(unresolved),
                    "route_coverage": raw_route.get("route_coverage", {}),
                    "execution": raw_route.get("execution", "not_started"),
                }
            )
            existing_context = prompt_request.get("context", [])
            if not isinstance(existing_context, Sequence) or isinstance(existing_context, (str, bytes)):
                raise BrainRunError("prompt.context must be a sequence when routing is enabled")
            context_chunks = [dict(chunk) for chunk in existing_context if isinstance(chunk, Mapping)]
            if len(context_chunks) != len(existing_context):
                raise BrainRunError("prompt.context must contain mappings")
            if any(chunk.get("id") == "capability-route" for chunk in context_chunks):
                raise BrainRunError("prompt.context already contains the reserved capability-route id")
            context_chunks.append(
                {
                    "id": "capability-route",
                    "role": "developer",
                    "content": json.dumps(route_context, ensure_ascii=False, sort_keys=True, separators=(",", ":")),
                    "required": True,
                    "priority": 1_000,
                }
            )
            prompt_request["context"] = context_chunks
            if not provider_tools and route_context["tool_schemas"] and not route_context["tool_schemas_omitted"]:
                provider_tools = tuple(
                    ProviderTool.from_mcp_schema(schema) for schema in route_context["tool_schemas"]
                )
            if enforce_route_tools:
                recommended_tools = route.get("recommended_tools")
                if not isinstance(recommended_tools, list) or any(not isinstance(tool, str) for tool in recommended_tools):
                    raise BrainRunError("capability route returned malformed recommended_tools")
                provider_tools = _route_provider_tool_surface(provider_tools, recommended_tools)
                if mission_policy is not None:
                    policy_for_route = (
                        mission_policy.to_dict()
                        if isinstance(mission_policy, MissionPolicy)
                        else dict(mission_policy)
                    )
                    allowed_tools = policy_for_route.get("allowed_tools")
                    if not isinstance(allowed_tools, Sequence) or isinstance(allowed_tools, (str, bytes)):
                        raise BrainRunError(
                            "enforce_route_tools requires an explicit mission policy allowed_tools list"
                        )
                    narrowed = [tool for tool in allowed_tools if tool in set(recommended_tools)]
                    if not narrowed:
                        raise BrainRunError("route has no overlap with the caller mission policy allowed_tools")
                    policy_for_route["allowed_tools"] = narrowed
                    mission_policy = policy_for_route
        if authorize_and_execute is None:
            if mission_policy is None:
                raise BrainRunError("provide authorize_and_execute or mission_policy for the built-in mission authorizer")
            if not provider_tools:
                raise BrainRunError("the built-in mission authorizer requires provider_tools")
            authorizer = MissionToolAuthorizer(
                self.workspace,
                task=task,
                mission_policy=mission_policy,
                route=raw_route,
                approve_mission_dispatch=approve_mission_dispatch,
                claim_requests=claim_requests,
                evaluator_review=evaluator_review,
                workflow_binding=workflow_binding,
                operations_gate_acceptance=operations_gate_acceptance,
            )
            authorize_and_execute = authorizer
        else:
            authorizer = None
        if attempt_state is not None:
            original_authorizer = authorize_and_execute
            if original_authorizer is None:
                raise BrainRunError("tool authorization callback was not initialized")

            def tracked_authorizer(
                calls: tuple[ProviderToolCall, ...],
            ) -> Sequence[ProviderToolResult]:
                if calls:
                    attempt_state["tool_authorization_started"] = True
                return original_authorizer(calls)

            authorize_and_execute = tracked_authorizer
        first = self.run(
            task=task,
            model_selection=model_selection,
            selection_override=selection_override,
            prompt=prompt_request,
            plan=plan,
            credentials=credentials,
            approve_provider_call=approve_provider_call,
            run_id=run_id,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            require_json=require_json,
            response_schema=response_schema,
            idempotency_key=idempotency_key,
            context=context,
            content_parts=None if content_parts is None else normalized_content_parts,
            contextual_observations=contextual_observations,
            tools=provider_tools,
            tool_choice=tool_choice,
            invocation_observer=invocation_observer,
        )
        if first.status != "completed_provider_call" or first.response is None:
            return BrainToolLoopResult(brain_run=first, status=first.status, provider_loop=None, route=route)
        selected = first.selection.get("selected_model")
        if not isinstance(selected, Mapping):
            raise BrainRunError("model selection did not produce a continuation model")
        provider = selected.get("provider")
        model = selected.get("model")
        if not isinstance(provider, str) or not isinstance(model, str):
            raise BrainRunError("continuation model metadata is malformed")
        prompt_messages = first.prompt.get("messages")
        if not isinstance(prompt_messages, list) or not prompt_messages:
            raise BrainRunError("brain prompt did not retain bounded provider messages")
        provider_messages = _provider_messages_with_content_parts(prompt_messages, normalized_content_parts)
        handle = credentials.get(provider)
        if self.runtime.provider_requires_credential(provider) and handle is None:
            raise BrainRunError(f"no user credential handle was supplied for provider {provider!r}")
        if handle is not None and handle.provider != provider:
            raise BrainRunError(f"credential handle does not belong to provider {provider!r}")
        continuation_idempotency_key = idempotency_key
        if prompt_request.get("_provider_messages_override") is not None:
            prompt_metadata = first.prompt.get("autonomous_prompt")
            continuation_idempotency_key = _prompt_bound_idempotency_key(
                idempotency_key,
                prompt_digest=first.prompt.get("prompt_digest"),
                metadata=prompt_metadata if isinstance(prompt_metadata, Mapping) else None,
            )
        request = ProviderRequest(
            model=model,
            messages=provider_messages,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            require_json=require_json,
            response_schema=response_schema,
            idempotency_key=continuation_idempotency_key,
            tools=tuple(provider_tools),
            tool_choice=tool_choice,
        )
        loop = self.runtime.invoke_tool_loop(
            provider,
            request,
            credential=handle,
            authorize_and_execute=authorize_and_execute,
            max_turns=max_turns,
            max_tool_calls=max_tool_calls,
            stream=stream,
            initial_response=first.response,
            invocation_observer=invocation_observer,
            invocation_kind="tool_loop_turn",
        )
        if isinstance(invocation_observer, AutonomousProviderInvocationSession):
            first = replace(first, provider_invocations=tuple(invocation_observer.evidence()))
        status = {
            "completed": "completed_provider_tool_loop",
            "authorization_required": "tool_authorization_required",
            "turn_limit_reached": "tool_turn_limit_reached",
        }[loop.status]
        receipts = () if authorizer is None else tuple(receipt.to_dict() for receipt in authorizer.receipts)
        return BrainToolLoopResult(
            brain_run=first,
            status=status,
            provider_loop=loop,
            route=route,
            authorization_receipts=receipts,
        )

    def record_evaluator_outcome(
        self,
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
        *,
        bandit_state: Mapping[str, Any],
        evaluator_id: str,
        evaluator_version: str,
        reward: float,
        passed: bool,
        arm_id: str | None = None,
        failed: bool = False,
        feedback_digest: str | None = None,
        failure_class: str | None = None,
        evidence_digest: str | None = None,
        ledger: BrainLearningLedger | None = None,
        replay_metadata: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
    ) -> dict[str, Any]:
        """Submit one explicit evaluator judgment for a run, loop, or mission.

        The evaluator remains the only reward authority. For continuation results, the identity
        digest is extended with bounded response metadata without retaining provider text or tool
        wire envelopes in the learning ledger.
        """

        if isinstance(result, BrainRunResult):
            brain_result = result
            outcome_digest = result.outcome_digest
            outcome_request_id = result.response.request_id if result.response is not None else None
        elif isinstance(result, BrainToolLoopResult):
            brain_result = result.brain_run
            final_response = None if result.provider_loop is None else result.provider_loop.final_response
            outcome_digest = _json_digest(
                {
                    "brain_outcome_digest": brain_result.outcome_digest,
                    "status": result.status,
                    "provider_loop_status": None
                    if result.provider_loop is None
                    else result.provider_loop.status,
                    "turns": None if result.provider_loop is None else result.provider_loop.turns,
                    "tool_calls": None
                    if result.provider_loop is None
                    else result.provider_loop.tool_calls,
                    "final_provider": None if final_response is None else final_response.provider,
                    "final_model": None if final_response is None else final_response.model,
                    "final_request_id": None if final_response is None else final_response.request_id,
                }
            )
            outcome_request_id = None if final_response is None else final_response.request_id
        elif isinstance(result, BrainMissionResult):
            brain_result = result.brain_run
            execution = result.execution or {}
            outcome_digest = _json_digest(
                {
                    "brain_outcome_digest": brain_result.outcome_digest,
                    "status": result.status,
                    "mission_status": execution.get("mission_status"),
                    "execution": execution.get("execution"),
                    "result_digest": execution.get("result_digest"),
                }
            )
            outcome_request_id = brain_result.response.request_id if brain_result.response is not None else None
        else:
            raise BrainRunError("result must be a BrainRunResult, BrainToolLoopResult, or BrainMissionResult")

        selected = brain_result.selection.get("selected_model")
        if not isinstance(selected, Mapping):
            raise BrainRunError("cannot record an outcome without selected model metadata")
        provider = selected.get("provider")
        model = selected.get("model")
        if not isinstance(provider, str) or not isinstance(model, str):
            raise BrainRunError("selected model metadata is malformed")
        selection_digest = brain_result.selection.get("decision_digest")
        prompt_digest = brain_result.prompt.get("prompt_digest")
        plan_digest = (brain_result.plan.get("plan") or {}).get("plan_digest") if isinstance(brain_result.plan.get("plan"), Mapping) else None
        for name, value in (("selection_digest", selection_digest), ("prompt_digest", prompt_digest), ("plan_digest", plan_digest)):
            if not isinstance(value, str) or len(value) != 64:
                raise BrainRunError(f"{name} is missing or is not a SHA-256 digest")
        context_digest, context = _selection_context_binding(brain_result.selection)
        contextual_digest = context_digest if context is not None else None
        effective_arm_id = arm_id or f"{provider}/{model}"
        normalized_bandit_state = _ensure_bandit_arm(
            bandit_state,
            effective_arm_id,
            context_digest=contextual_digest,
            context=context,
        )
        resolved_idempotency_key = idempotency_key or f"run:{brain_result.run_id}"
        if not isinstance(resolved_idempotency_key, str) or not resolved_idempotency_key.strip() or len(resolved_idempotency_key.encode("utf-8")) > 512:
            raise BrainRunError("evaluator idempotency_key must be a bounded non-empty string")
        report = self.workspace.tool(
            "brain_outcome_record",
            {
                "run": {
                    "run_id": brain_result.run_id,
                    "selection_digest": selection_digest,
                    "prompt_digest": prompt_digest,
                    "plan_digest": plan_digest,
                    "provider": provider,
                    "model": model,
                    "outcome_digest": outcome_digest,
                    "request_id": outcome_request_id,
                },
                "assessment": {
                    "evaluator_id": evaluator_id,
                    "evaluator_version": evaluator_version,
                    "reward": reward,
                    "passed": passed,
                    "failed": failed,
                    "feedback_digest": feedback_digest,
                    "failure_class": failure_class,
                    "evidence_digest": evidence_digest,
                },
                "bandit_state": normalized_bandit_state,
                "arm_id": effective_arm_id,
                **({"context_digest": contextual_digest, "context": context} if contextual_digest is not None else {}),
                "idempotency_key": resolved_idempotency_key,
            },
        )
        if not isinstance(report, Mapping) or not report.get("ok"):
            raise BrainRunError("brain outcome recording returned a refusal")
        if ledger is not None:
            ledger.append(
                report,
                context_digest=context_digest if isinstance(context_digest, str) else None,
                replay=replay_metadata,
            )
        return dict(report)

    def prepare_learning_episode(
        self,
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
        *,
        evidence: Mapping[str, Any] | None = None,
        arm_id: str | None = None,
        episode_id: str | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> BrainLearningEpisode:
        """Create and optionally persist a delayed-feedback episode.

        Only the value-only evaluator projection is retained. If the caller already has a
        bounded evidence packet, its digest is bound now while the packet itself remains the
        caller's responsibility to retain and re-submit at settlement time.
        """

        metadata = build_brain_evaluation_input(result)
        evidence_digest: str | None = None
        if evidence is not None:
            with_evidence = build_brain_evaluation_input(result, evidence=evidence)
            evidence_digest = with_evidence.get("evidence_digest")
            if not isinstance(evidence_digest, str) or not _valid_digest(evidence_digest):
                raise BrainRunError("learning episode evidence digest was not generated")
            metadata["evidence_digest"] = evidence_digest
        selected = metadata.get("selected_model")
        if not isinstance(selected, Mapping):
            raise BrainRunError("cannot create a learning episode without selected model metadata")
        provider = selected.get("provider")
        model = selected.get("model")
        if not isinstance(provider, str) or not isinstance(model, str) or not provider or not model:
            raise BrainRunError("learning episode selected model metadata is malformed")
        effective_arm_id = arm_id or f"{provider}/{model}"
        if not isinstance(effective_arm_id, str) or not effective_arm_id.strip():
            raise BrainRunError("learning episode arm_id must be a non-empty string")
        resolved_episode_id = episode_id
        if resolved_episode_id is None:
            resolved_episode_id = "episode-" + _json_digest(
                {
                    "run_id": metadata.get("run_id"),
                    "result_kind": metadata.get("result_kind"),
                    "learning_outcome_digest": metadata.get("learning_outcome_digest"),
                    "arm_id": effective_arm_id,
                }
            )
        episode = BrainLearningEpisode(
            episode_id=resolved_episode_id,
            evaluation_input=metadata,
            arm_id=effective_arm_id,
            evidence_digest=evidence_digest,
        )
        if ledger is not None:
            ledger.begin_episode(episode)
        return episode

    def prepare_learning_trajectory(
        self,
        results: Sequence[BrainRunResult | BrainToolLoopResult | BrainMissionResult],
        *,
        evidence_by_step: Sequence[Mapping[str, Any] | None] | None = None,
        arm_ids: Sequence[str | None] | None = None,
        trajectory_id: str | None = None,
        discount: float = 0.90,
        terminal_reward: float | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> BrainLearningTrajectory:
        """Prepare an ordered delayed-feedback trajectory for workflow or mission learning.

        Episodes are built before any ledger write, then registered in order. This makes the
        trajectory identity deterministic while allowing a caller to persist it before a human,
        benchmark, or downstream synthesis evaluator supplies the eventual reward packet.
        """

        if not isinstance(results, Sequence) or isinstance(results, (str, bytes)):
            raise BrainRunError("learning trajectory results must be a sequence")
        if not 1 <= len(results) <= MAX_BRAIN_LEARNING_TRAJECTORY_STEPS:
            raise BrainRunError(
                "learning trajectory results must contain between 1 and "
                f"{MAX_BRAIN_LEARNING_TRAJECTORY_STEPS} items"
            )
        if any(not isinstance(result, (BrainRunResult, BrainToolLoopResult, BrainMissionResult)) for result in results):
            raise BrainRunError("learning trajectory results contain an unsupported brain result")
        if evidence_by_step is not None:
            if not isinstance(evidence_by_step, Sequence) or isinstance(evidence_by_step, (str, bytes)):
                raise BrainRunError("learning trajectory evidence_by_step must be a sequence or None")
            if len(evidence_by_step) != len(results) or any(
                item is not None and not isinstance(item, Mapping) for item in evidence_by_step
            ):
                raise BrainRunError("learning trajectory evidence_by_step must match results")
        if arm_ids is not None:
            if not isinstance(arm_ids, Sequence) or isinstance(arm_ids, (str, bytes)):
                raise BrainRunError("learning trajectory arm_ids must be a sequence or None")
            if len(arm_ids) != len(results) or any(item is not None and (not isinstance(item, str) or not item.strip()) for item in arm_ids):
                raise BrainRunError("learning trajectory arm_ids must match results")
        if trajectory_id is None:
            trajectory_id = "trajectory-" + _json_digest(
                {
                    "runs": [_learning_outcome_digest(result) for result in results],
                    "discount": discount,
                    "terminal_reward": terminal_reward,
                }
            )
        if not isinstance(trajectory_id, str) or not trajectory_id.strip():
            raise BrainRunError("learning trajectory_id must be a non-empty string or None")
        episodes: list[BrainLearningEpisode] = []
        for index, result in enumerate(results):
            episode_id = f"{trajectory_id}-step-{index}"
            if len(episode_id.encode("utf-8")) > 512:
                episode_id = "trajectory-episode-" + _json_digest(
                    {"trajectory_id": trajectory_id, "index": index}
                )
            episodes.append(
                self.prepare_learning_episode(
                    result,
                    evidence=None if evidence_by_step is None else evidence_by_step[index],
                    arm_id=None if arm_ids is None else arm_ids[index],
                    episode_id=episode_id,
                )
            )
        trajectory = BrainLearningTrajectory(
            trajectory_id=trajectory_id,
            episodes=tuple(episodes),
            discount=discount,
            terminal_reward=terminal_reward,
        )
        if ledger is not None:
            for episode in trajectory.episodes:
                ledger.begin_episode(episode)
        return trajectory

    def record_value_only_evaluator_outcome(
        self,
        episode: BrainLearningEpisode | Mapping[str, Any],
        *,
        bandit_state: Mapping[str, Any],
        evaluator_id: str,
        evaluator_version: str,
        reward: float,
        passed: bool,
        failed: bool = False,
        feedback_digest: str | None = None,
        failure_class: str | None = None,
        evidence: Mapping[str, Any] | None = None,
        ledger: BrainLearningLedger | None = None,
        replay_metadata: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Settle delayed evaluator feedback using only a persisted episode projection.

        This is the restart-safe counterpart to :meth:`record_evaluator_outcome`. It never needs
        the original provider response or credential handle, and it still routes the reward through
        the Rust kernel's explicit bandit update.
        """

        normalized_episode = episode if isinstance(episode, BrainLearningEpisode) else BrainLearningEpisode.from_mapping(episode)
        if normalized_episode.status != "pending":
            raise BrainRunError("learning episode is already settled")
        if ledger is not None and normalized_episode.episode_id not in {
            item.episode_id for item in ledger.pending_episodes(limit=ledger.max_records)
        }:
            raise BrainRunError("learning episode is already settled or was not registered")
        evaluation_input = build_brain_evaluation_input_from_metadata(
            normalized_episode.evaluation_input,
            evidence=evidence,
        )
        selected = evaluation_input.get("selected_model")
        if not isinstance(selected, Mapping):
            raise BrainRunError("learning episode selected model metadata is malformed")
        provider = selected.get("provider")
        model = selected.get("model")
        if not isinstance(provider, str) or not isinstance(model, str) or not provider or not model:
            raise BrainRunError("learning episode selected model metadata is malformed")
        context_digest, context = _selection_context_binding(evaluation_input)
        contextual_digest = context_digest if context is not None else None
        run_id = evaluation_input.get("run_id")
        selection_digest = evaluation_input.get("selection_digest")
        prompt_digest = evaluation_input.get("prompt_digest")
        plan_digest = evaluation_input.get("plan_digest")
        outcome_digest = evaluation_input.get("learning_outcome_digest", evaluation_input.get("outcome_digest"))
        for name, value in (
            ("run_id", run_id),
            ("selection_digest", selection_digest),
            ("prompt_digest", prompt_digest),
            ("plan_digest", plan_digest),
            ("outcome_digest", outcome_digest),
        ):
            if not isinstance(value, str) or not value.strip() or (name != "run_id" and not _valid_digest(value)):
                raise BrainRunError(f"learning episode {name} is missing or malformed")
        response = evaluation_input.get("response")
        request_id = response.get("request_id") if isinstance(response, Mapping) else None
        if request_id is None:
            loop = evaluation_input.get("tool_loop")
            request_id = loop.get("final_request_id") if isinstance(loop, Mapping) else None
        if request_id is not None and (not isinstance(request_id, str) or not request_id.strip()):
            raise BrainRunError("learning episode request_id is malformed")
        normalized_bandit_state = _ensure_bandit_arm(
            bandit_state,
            normalized_episode.arm_id,
            context_digest=contextual_digest,
            context=context,
        )
        report = self.workspace.tool(
            "brain_outcome_record",
            {
                "run": {
                    "run_id": run_id,
                    "selection_digest": selection_digest,
                    "prompt_digest": prompt_digest,
                    "plan_digest": plan_digest,
                    "provider": provider,
                    "model": model,
                    "outcome_digest": outcome_digest,
                    "request_id": request_id,
                },
                "assessment": {
                    "evaluator_id": evaluator_id,
                    "evaluator_version": evaluator_version,
                    "reward": reward,
                    "passed": passed,
                    "failed": failed,
                    "feedback_digest": feedback_digest,
                    "failure_class": failure_class,
                    "evidence_digest": evaluation_input.get("evidence_digest"),
                },
                "bandit_state": normalized_bandit_state,
                "arm_id": normalized_episode.arm_id,
                **({"context_digest": contextual_digest, "context": context} if contextual_digest is not None else {}),
                "idempotency_key": f"episode:{normalized_episode.episode_id}",
            },
        )
        if not isinstance(report, Mapping) or not report.get("ok"):
            raise BrainRunError("brain outcome recording returned a refusal")
        replay = dict(replay_metadata or {})
        replay.setdefault("schema", BRAIN_EVALUATOR_REPLAY_SCHEMA)
        replay.update(
            {
                "episode_id": normalized_episode.episode_id,
                "result_kind": evaluation_input.get("result_kind"),
                "run_id": run_id,
                "outcome_digest": outcome_digest,
                "evaluation_input_digest": _json_digest(evaluation_input),
                "evidence_digest": evaluation_input.get("evidence_digest"),
                "evaluator_id": evaluator_id,
                "evaluator_version": evaluator_version,
                "retention": "metadata_and_digests_only",
            }
        )
        if ledger is not None:
            context_digest = evaluation_input.get("context_digest")
            ledger.append(
                report,
                context_digest=context_digest if isinstance(context_digest, str) else None,
                replay=replay,
            )
        return dict(report)

    def run_mission(
        self,
        *,
        task: str,
        model_selection: Mapping[str, Any],
        selection_override: Mapping[str, Any] | None = None,
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle],
        mission_policy: MissionPolicy | Mapping[str, Any],
        approve_provider_call: bool = False,
        approve_mission_dispatch: bool = False,
        run_id: str | None = None,
        max_output_tokens: int = 2048,
        temperature: float | None = None,
        response_schema: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        claim_requests: Sequence[Mapping[str, Any]] = (),
        context: Mapping[str, Any] | None = None,
        content_parts: Sequence[ProviderContentPart | Mapping[str, Any]] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        evaluator_review: Mapping[str, Any] | None = None,
        workflow_binding: Mapping[str, Any] | None = None,
        route_review: Mapping[str, Any] | None = None,
        operations_gate_acceptance: Mapping[str, Any] | None = None,
        route_request: Mapping[str, Any] | None = None,
        route_report: Mapping[str, Any] | None = None,
        attempt_state: dict[str, Any] | None = None,
        enforce_route_tools: bool = False,
        require_resolved_route: bool = True,
        provider_tools: Sequence[ProviderTool] = (),
        tool_choice: str | None = None,
        invocation_observer: ProviderInvocationObserver | None = None,
    ) -> BrainMissionResult:
        """Run a model decision through the existing bounded mission executor.

        The model supplies only step data. The caller supplies the mission policy and therefore
        the tool allow-list, output budgets, parallelism, and side-effect posture. The server
        receives a preview with ``execute=false`` first; dispatch is a separate request after
        ``approve_mission_dispatch=True``. Claims/evaluator bindings are caller-owned metadata and
        are not accepted from the model response.
        """

        if not isinstance(mission_policy, (MissionPolicy, Mapping)):
            raise BrainRunError("mission_policy must be a MissionPolicy or mapping")
        policy = (
            mission_policy.to_dict()
            if isinstance(mission_policy, MissionPolicy)
            else dict(mission_policy)
        )
        if not isinstance(claim_requests, Sequence) or isinstance(claim_requests, (str, bytes)):
            raise BrainRunError("claim_requests must be a sequence")
        if not isinstance(enforce_route_tools, bool) or not isinstance(require_resolved_route, bool):
            raise BrainRunError("route enforcement flags must be booleans")
        if not isinstance(provider_tools, Sequence) or isinstance(provider_tools, (str, bytes)):
            raise BrainRunError("provider_tools must be a sequence")
        if any(not isinstance(tool, ProviderTool) for tool in provider_tools):
            raise BrainRunError("provider_tools must contain ProviderTool values")
        if attempt_state is not None and not isinstance(attempt_state, dict):
            raise BrainRunError("attempt_state must be a mutable mapping")
        if attempt_state is not None:
            attempt_state["mission_dispatch_started"] = False
        if route_report is not None:
            if route_request is None:
                raise BrainRunError("route_report requires route_request")
            if not isinstance(route_report, Mapping):
                raise BrainRunError("route_report must be a mapping")
            BrainLearningLedger._assert_safe(route_report)

        route: dict[str, Any] | None = None
        prompt_request = dict(prompt)
        if route_request is not None:
            if not isinstance(route_request, Mapping):
                raise BrainRunError("route_request must be a mapping")
            BrainLearningLedger._assert_safe(route_request)
            route_arguments = dict(route_request)
            supplied_goal = route_arguments.get("goal")
            if supplied_goal is not None and supplied_goal != task:
                raise BrainRunError("route_request.goal must match the mission task")
            route_arguments["goal"] = task
            route_arguments.setdefault(
                "needs",
                [{"id": "task", "query": task}],
            )
            route_arguments.setdefault("include_tools", True)
            route_arguments.setdefault("max_tools", 128)
            try:
                encoded_route_request = json.dumps(
                    route_arguments,
                    ensure_ascii=False,
                    sort_keys=True,
                    separators=(",", ":"),
                    allow_nan=False,
                ).encode("utf-8")
            except (TypeError, ValueError) as error:
                raise BrainRunError("route_request must be JSON-safe") from error
            if len(encoded_route_request) > MAX_ROUTE_REQUEST_BYTES:
                raise BrainRunError("route_request exceeds the bounded size")
            route_response = (
                dict(route_report)
                if route_report is not None
                else self.workspace.tool("capability_route", route_arguments)
            )
            if not isinstance(route_response, Mapping):
                raise BrainRunError("capability route returned a non-object")
            if route_response.get("ok") is False or route_response.get("workflow") != "capability_route":
                raise BrainRunError("capability route was refused")
            raw_route = dict(route_response)
            BrainLearningLedger._assert_safe(raw_route)
            unresolved = raw_route.get("unresolved_needs", [])
            if not isinstance(unresolved, list) or any(not isinstance(item, str) for item in unresolved):
                raise BrainRunError("capability route returned malformed unresolved_needs")
            if unresolved and require_resolved_route:
                raise BrainRunError(
                    "capability route contains unresolved needs: " + ", ".join(unresolved)
                )
            route_context = _bounded_route_prompt_context(raw_route)
            route = dict(route_context)
            route.update(
                {
                    "ok": True,
                    "workflow": "capability_route",
                    "evidence_digest": raw_route.get("evidence_digest"),
                    "unresolved_needs": list(unresolved),
                    "route_coverage": raw_route.get("route_coverage", {}),
                    "execution": raw_route.get("execution", "not_started"),
                }
            )
            existing_context = prompt_request.get("context", [])
            if not isinstance(existing_context, Sequence) or isinstance(existing_context, (str, bytes)):
                raise BrainRunError("prompt.context must be a sequence when routing is enabled")
            context_chunks = [dict(chunk) for chunk in existing_context if isinstance(chunk, Mapping)]
            if len(context_chunks) != len(existing_context):
                raise BrainRunError("prompt.context must contain mappings")
            route_chunk_id = "capability-route"
            if any(chunk.get("id") == route_chunk_id for chunk in context_chunks):
                raise BrainRunError("prompt.context already contains the reserved capability-route id")
            context_chunks.append(
                {
                    "id": route_chunk_id,
                    "role": "developer",
                    "content": json.dumps(
                        route_context,
                        ensure_ascii=False,
                        sort_keys=True,
                        separators=(",", ":"),
                    ),
                    "required": True,
                    "priority": 1_000,
                }
            )
            prompt_request["context"] = context_chunks

            if not provider_tools and route_context["tool_schemas"] and not route_context["tool_schemas_omitted"]:
                provider_tools = tuple(
                    ProviderTool.from_mcp_schema(schema)
                    for schema in route_context["tool_schemas"]
                )

            if enforce_route_tools:
                recommended_tools = route.get("recommended_tools")
                if not isinstance(recommended_tools, list) or any(
                    not isinstance(tool, str) for tool in recommended_tools
                ):
                    raise BrainRunError("capability route returned malformed recommended_tools")
                provider_tools = _route_provider_tool_surface(provider_tools, recommended_tools)
                recommended_set = set(recommended_tools)
                allowed_tools = policy.get("allowed_tools")
                if not isinstance(allowed_tools, Sequence) or isinstance(allowed_tools, (str, bytes)):
                    raise BrainRunError(
                        "enforce_route_tools requires an explicit mission policy allowed_tools list"
                    )
                narrowed = [tool for tool in allowed_tools if tool in recommended_set]
                if not narrowed:
                    raise BrainRunError(
                        "route has no overlap with the caller mission policy allowed_tools"
                    )
                policy["allowed_tools"] = narrowed
        policy["execute"] = False
        brain_run = self.run(
            task=task,
            model_selection=model_selection,
            selection_override=selection_override,
            prompt=prompt_request,
            plan=plan,
            credentials=credentials,
            approve_provider_call=approve_provider_call,
            run_id=run_id,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            require_json=True,
            response_schema=response_schema or DEFAULT_MISSION_RESPONSE_SCHEMA,
            idempotency_key=idempotency_key,
            context=context,
            content_parts=content_parts,
            contextual_observations=contextual_observations,
            tools=provider_tools,
            tool_choice=tool_choice,
            invocation_observer=invocation_observer,
        )
        if brain_run.status != "completed_provider_call" or brain_run.response is None:
            return BrainMissionResult(
                brain_run=brain_run,
                status="brain_run_not_completed",
                mission=None,
                preflight=None,
                execution=None,
                route=route,
            )
        if brain_run.response.tool_calls:
            raw_steps = []
            for index, call in enumerate(brain_run.response.tool_calls):
                domain = "cross_domain"
                if route is not None:
                    for need in route.get("needs", []):
                        if not isinstance(need, Mapping):
                            continue
                        candidate_tools = need.get("candidate_tools", [])
                        if call.name in candidate_tools:
                            domains = need.get("candidate_domains", [])
                            if isinstance(domains, list) and domains and isinstance(domains[0], str):
                                domain = domains[0]
                            break
                raw_steps.append(
                    {
                        "id": f"provider-tool-{index}",
                        "domain": domain,
                        "capability": call.name,
                        "objective": f"Execute the caller-authorized provider tool intent {call.name}",
                        "tool": call.name,
                        "arguments": dict(call.arguments),
                        "required": True,
                        "depends_on": [],
                        "bindings": [],
                    }
                )
        else:
            structured = brain_run.response.structured
            if not isinstance(structured, Mapping):
                raise BrainRunError("structured brain response did not contain a JSON object")
            proposed = structured.get("mission")
            if not isinstance(proposed, Mapping):
                raise BrainRunError("structured brain response did not contain a mission object")
            raw_steps = proposed.get("steps")
            if not isinstance(raw_steps, list) or not raw_steps:
                raise BrainRunError("model mission must contain a non-empty steps array")

        mission_id = f"{brain_run.run_id}-mission"
        preview_request = MissionRequest(
            mission_id=mission_id,
            goal=task,
            steps=raw_steps,
            policy=policy,
            claim_requests=claim_requests,
            evaluator_review=evaluator_review,
            workflow_binding=workflow_binding,
            route_review=route_review,
            operations_gate_acceptance=operations_gate_acceptance,
        )
        preview_arguments = preview_request.to_mcp_arguments()
        preflight = self.workspace.tool("agent_mission", preview_arguments)
        if not isinstance(preflight, Mapping):
            raise BrainRunError("agent mission preflight returned a non-object")
        if preflight.get("workflow") not in (None, "agent_mission"):
            raise BrainRunError("agent mission preflight returned the wrong workflow")
        mission = dict(preview_arguments)
        if not approve_mission_dispatch:
            return BrainMissionResult(
                brain_run=brain_run,
                status="mission_approval_required",
                mission=mission,
                preflight=dict(preflight),
                execution=None,
                route=route,
            )

        execute_policy = dict(policy)
        execute_policy["execute"] = True
        execute_request = MissionRequest(
            mission_id=mission_id,
            goal=task,
            steps=raw_steps,
            policy=execute_policy,
            claim_requests=claim_requests,
            evaluator_review=evaluator_review,
            workflow_binding=workflow_binding,
            route_review=route_review,
            operations_gate_acceptance=operations_gate_acceptance,
        )
        if attempt_state is not None:
            attempt_state["mission_dispatch_started"] = True
        execution = self.workspace.tool("agent_mission", execute_request.to_mcp_arguments())
        if not isinstance(execution, Mapping):
            raise BrainRunError("agent mission execution returned a non-object")
        return BrainMissionResult(
            brain_run=brain_run,
            status="mission_dispatched",
            mission=mission,
            preflight=dict(preflight),
            execution=dict(execution),
            route=route,
        )

    @staticmethod
    def _result(
        run_id: str,
        status: str,
        selection: Mapping[str, Any],
        prompt: Mapping[str, Any],
        plan: Mapping[str, Any],
        response: ProviderResponse | None,
        *,
        provider_invocations: Sequence[Mapping[str, Any]] = (),
        continuation_plan: Mapping[str, Any] | None = None,
        provider_failover: Mapping[str, Any] | None = None,
    ) -> BrainRunResult:
        digest_input = {
            "status": status,
            "selection": selection,
            "prompt_digest": prompt.get("prompt_digest"),
            "plan_digest": (plan.get("plan") or {}).get("plan_digest")
            if isinstance(plan.get("plan"), Mapping)
            else None,
            "response": None
            if response is None
            else {
                "provider": response.provider,
                "model": response.model,
                "text": response.text,
                "request_id": response.request_id,
                "usage": dict(response.usage),
            },
        }
        encoded = json.dumps(digest_input, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
        return BrainRunResult(
            run_id=run_id,
            status=status,
            selection=selection,
            prompt=prompt,
            plan=plan,
            response=response,
            outcome_digest=hashlib.sha256(encoded).hexdigest(),
            provider_failover=None if provider_failover is None else dict(provider_failover),
            provider_invocations=tuple(dict(receipt) for receipt in provider_invocations),
            continuation_plan=None if continuation_plan is None else dict(continuation_plan),
        )


@dataclass(frozen=True, slots=True)
class BrainEvaluatorDecision:
    """A validated, value-only evaluator judgment for one brain outcome.

    The evaluator is intentionally separate from the provider. A provider response can be
    inspected by a caller-owned evaluator, but only this compact decision crosses the learning
    boundary. ``evidence_digest`` binds the decision to the optional caller-supplied evidence
    packet without copying that packet into the learning ledger.
    """

    evaluator_id: str
    evaluator_version: str
    reward: float
    passed: bool
    failed: bool = False
    feedback_digest: str | None = None
    failure_class: str | None = None
    evidence_digest: str | None = None
    replan_requested: bool = False
    replan_instruction: str | None = None

    def __post_init__(self) -> None:
        for field_name, value in (
            ("evaluator_id", self.evaluator_id),
            ("evaluator_version", self.evaluator_version),
        ):
            if (
                not isinstance(value, str)
                or not value.strip()
                or len(value.encode("utf-8")) > MAX_BRAIN_EVALUATOR_ID_BYTES
            ):
                raise BrainRunError(f"{field_name} must be a bounded non-empty string")
        if (
            not isinstance(self.reward, (int, float))
            or isinstance(self.reward, bool)
            or not isinstance(self.passed, bool)
            or not isinstance(self.failed, bool)
        ):
            raise BrainRunError("evaluator decision has malformed reward or status fields")
        try:
            json.dumps(self.reward, allow_nan=False)
        except (TypeError, ValueError) as error:
            raise BrainRunError("evaluator reward must be finite") from error
        if self.passed and self.failed:
            raise BrainRunError("evaluator decision cannot be both passed and failed")
        for field_name, value in (
            ("feedback_digest", self.feedback_digest),
            ("evidence_digest", self.evidence_digest),
        ):
            if value is not None and not _valid_digest(value):
                raise BrainRunError(f"{field_name} must be a lowercase SHA-256 digest")
        if self.failure_class is not None and (
            not isinstance(self.failure_class, str)
            or not self.failure_class.strip()
            or len(self.failure_class.encode("utf-8")) > MAX_BRAIN_EVALUATOR_ID_BYTES
        ):
            raise BrainRunError("failure_class must be a bounded non-empty string")
        if not isinstance(self.replan_requested, bool):
            raise BrainRunError("replan_requested must be boolean")
        if self.replan_instruction is not None and (
            not isinstance(self.replan_instruction, str)
            or not self.replan_instruction.strip()
            or len(self.replan_instruction.encode("utf-8")) > MAX_BRAIN_REPLAN_INSTRUCTION_BYTES
        ):
            raise BrainRunError("replan_instruction must be a bounded non-empty string")
        if self.replan_instruction is not None and any(
            pattern.search(self.replan_instruction) for pattern in _REPLAN_SECRET_PATTERNS
        ):
            raise BrainRunError("replan_instruction resembles secret material")
        if self.replan_requested and self.failed and self.replan_instruction is None and self.failure_class is None:
            raise BrainRunError("a requested replan must include an instruction or failure_class")

    def to_dict(self) -> dict[str, Any]:
        return {
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "reward": self.reward,
            "passed": self.passed,
            "failed": self.failed,
            "feedback_digest": self.feedback_digest,
            "failure_class": self.failure_class,
            "evidence_digest": self.evidence_digest,
            "replan_requested": self.replan_requested,
            "replan_instruction": self.replan_instruction,
        }


def _evaluator_metadata_projection(result: BrainRunResult) -> dict[str, Any]:
    """Project the common run identity without exposing prompt or provider response content."""

    selected = result.selection.get("selected_model")
    selected_model = (
        {"provider": selected.get("provider"), "model": selected.get("model")}
        if isinstance(selected, Mapping)
        else None
    )
    plan = result.plan.get("plan")
    plan_digest = plan.get("plan_digest") if isinstance(plan, Mapping) else None
    projection: dict[str, Any] = {
        "run_id": result.run_id,
        "status": result.status,
        "selected_model": selected_model,
        "selection_digest": result.selection.get("decision_digest"),
        "context_digest": result.selection.get("context_digest"),
        "context": dict(result.selection.get("context"))
        if isinstance(result.selection.get("context"), Mapping)
        else None,
        "selection_audit": dict(result.selection.get("selection_audit", {}))
        if isinstance(result.selection.get("selection_audit"), Mapping)
        else None,
        "prompt_digest": result.prompt.get("prompt_digest"),
        "plan_digest": plan_digest,
        "outcome_digest": result.outcome_digest,
        "provider_failover": None
        if result.provider_failover is None
        else {
            "strategy": result.provider_failover.get("strategy"),
            "fallback_count": result.provider_failover.get("fallback_count"),
            "attempt_count": len(result.provider_failover.get("attempts", []))
            if isinstance(result.provider_failover.get("attempts"), list)
            else None,
            "retention": result.provider_failover.get("retention"),
        },
        "provider_invocations": [dict(receipt) for receipt in result.provider_invocations],
    }
    if result.response is not None:
        projection["response"] = {
            "provider": result.response.provider,
            "model": result.response.model,
            "request_id": result.response.request_id,
            "usage": dict(result.response.usage),
            "structured": result.response.structured is not None,
            "tool_call_count": len(result.response.tool_calls),
        }
    else:
        projection["response"] = None
    return projection


def build_brain_evaluation_input(
    result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
    *,
    evidence: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Build a bounded evaluator input for any brain execution shape.

    Only identities, digests, status/count metadata, route identity, and caller-supplied bounded
    evidence are exposed. Provider text, prompt text, credentials, and opaque tool wire envelopes
    are deliberately absent. The returned value is JSON round-tripped so an evaluator cannot
    mutate the caller's original mappings through shared references.
    """

    if evidence is not None:
        if not isinstance(evidence, Mapping):
            raise BrainRunError("evaluator evidence must be a mapping or None")
        BrainLearningLedger._assert_safe(evidence)
        try:
            encoded_evidence = json.dumps(
                dict(evidence),
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            ).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise BrainRunError("evaluator evidence must be JSON-safe") from error
        if len(encoded_evidence) > MAX_BRAIN_EVALUATOR_EVIDENCE_BYTES:
            raise BrainRunError("evaluator evidence exceeds the bounded size")
        evidence_copy = json.loads(encoded_evidence.decode("utf-8"))
        evidence_digest = hashlib.sha256(encoded_evidence).hexdigest()
    else:
        evidence_copy = None
        evidence_digest = None

    if isinstance(result, BrainRunResult):
        projection = _evaluator_metadata_projection(result)
        result_kind = "run"
    elif isinstance(result, BrainToolLoopResult):
        projection = _evaluator_metadata_projection(result.brain_run)
        loop = result.provider_loop
        final_response = None if loop is None else loop.final_response
        receipt_statuses: dict[str, int] = {}
        for receipt in result.authorization_receipts:
            if not isinstance(receipt, Mapping):
                continue
            status = receipt.get("status")
            if isinstance(status, str):
                receipt_statuses[status] = receipt_statuses.get(status, 0) + 1
        projection.update(
            {
                "result_kind": "tool_loop",
                "status": result.status,
                "route": None
                if result.route is None
                else {
                    "route_digest": _json_digest(dict(result.route)),
                    "evidence_digest": result.route.get("evidence_digest"),
                    "execution": result.route.get("execution"),
                },
                "tool_loop": None
                if loop is None
                else {
                    "status": loop.status,
                    "turns": loop.turns,
                    "tool_calls": loop.tool_calls,
                    "final_provider": None if final_response is None else final_response.provider,
                    "final_model": None if final_response is None else final_response.model,
                    "final_request_id": None
                    if final_response is None
                    else final_response.request_id,
                },
                "tool_receipts": {
                    "receipt_count": len(result.authorization_receipts),
                    "status_counts": receipt_statuses,
                },
            }
        )
        result_kind = "tool_loop"
    elif isinstance(result, BrainMissionResult):
        projection = _evaluator_metadata_projection(result.brain_run)
        execution = result.execution if isinstance(result.execution, Mapping) else None
        preflight = result.preflight if isinstance(result.preflight, Mapping) else None
        projection.update(
            {
                "result_kind": "mission",
                "status": result.status,
                "route": None
                if result.route is None
                else {
                    "route_digest": _json_digest(dict(result.route)),
                    "evidence_digest": result.route.get("evidence_digest"),
                    "execution": result.route.get("execution"),
                },
                "mission": {
                    "preflight": None
                    if preflight is None
                    else _bounded_mission_report_projection(preflight, include_outputs=False),
                    "execution": None
                    if execution is None
                    else _bounded_mission_report_projection(execution, include_outputs=False),
                },
            }
        )
        result_kind = "mission"
    else:
        raise BrainRunError("result must be a BrainRunResult, BrainToolLoopResult, or BrainMissionResult")

    projection["schema"] = "bioprism-brain-evaluator-input/0.1"
    projection["result_kind"] = result_kind
    projection["learning_outcome_digest"] = _learning_outcome_digest(result)
    projection["evidence_digest"] = evidence_digest
    projection["evidence"] = evidence_copy
    BrainLearningLedger._assert_safe(projection)
    try:
        encoded_projection = json.dumps(
            projection,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise BrainRunError("evaluator input must be JSON-safe") from error
    if len(encoded_projection) > MAX_BRAIN_EVALUATOR_INPUT_BYTES:
        raise BrainRunError("evaluator input exceeds the bounded size")
    return json.loads(encoded_projection.decode("utf-8"))


def build_brain_evaluation_input_from_metadata(
    metadata: Mapping[str, Any],
    *,
    evidence: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Rehydrate a value-only evaluator input without a live provider result.

    ``metadata`` is normally read from :class:`BrainLearningEpisode`. It may contain only the
    redacted projection produced by :func:`build_brain_evaluation_input`; the optional evidence
    packet is caller-owned and is digest-checked before it reaches the evaluator.
    """

    if not isinstance(metadata, Mapping) or metadata.get("schema") != "bioprism-brain-evaluator-input/0.1":
        raise BrainRunError("value-only evaluator metadata has an invalid schema")
    if metadata.get("evidence") not in (None, {}):
        raise BrainRunError("value-only evaluator metadata must not contain retained evidence")
    BrainLearningLedger._assert_safe(metadata)
    try:
        normalized = json.loads(
            json.dumps(
                dict(metadata),
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            )
        )
    except (TypeError, ValueError) as error:
        raise BrainRunError("value-only evaluator metadata must be JSON-safe") from error
    if not isinstance(normalized, dict):
        raise BrainRunError("value-only evaluator metadata must be an object")
    expected_digest = normalized.get("evidence_digest")
    if expected_digest is not None and not _valid_digest(expected_digest):
        raise BrainRunError("value-only evaluator evidence_digest is malformed")
    if evidence is None:
        normalized["evidence"] = None
    else:
        if not isinstance(evidence, Mapping):
            raise BrainRunError("value-only evaluator evidence must be a mapping or None")
        BrainLearningLedger._assert_safe(evidence)
        try:
            encoded_evidence = json.dumps(
                dict(evidence),
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            ).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise BrainRunError("value-only evaluator evidence must be JSON-safe") from error
        if len(encoded_evidence) > MAX_BRAIN_EVALUATOR_EVIDENCE_BYTES:
            raise BrainRunError("value-only evaluator evidence exceeds the bounded size")
        actual_digest = hashlib.sha256(encoded_evidence).hexdigest()
        if expected_digest is not None and expected_digest != actual_digest:
            raise BrainRunError("value-only evaluator evidence does not match its episode digest")
        normalized["evidence_digest"] = actual_digest
        normalized["evidence"] = json.loads(encoded_evidence.decode("utf-8"))
    BrainLearningLedger._assert_safe(normalized)
    try:
        encoded = json.dumps(
            normalized,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise BrainRunError("value-only evaluator input must be JSON-safe") from error
    if len(encoded) > MAX_BRAIN_EVALUATOR_INPUT_BYTES:
        raise BrainRunError("value-only evaluator input exceeds the bounded size")
    return json.loads(encoded.decode("utf-8"))


class BrainOutcomeEvaluator:
    """Adapt a caller-owned evaluator into the value-only learning boundary.

    The callback receives :func:`build_brain_evaluation_input`, never a raw provider response or
    runtime credential. It may return a :class:`BrainEvaluatorDecision` or a mapping containing
    only ``reward``, ``passed``, ``failed``, ``feedback_digest``, and ``failure_class``. The
    adapter computes and binds the evidence digest, then delegates persistence to the brain.
    """

    _ALLOWED_DECISION_FIELDS = {
        "reward",
        "passed",
        "failed",
        "feedback_digest",
        "failure_class",
        "evidence_digest",
        "replan_requested",
        "replan_instruction",
    }

    def __init__(
        self,
        evaluator: Callable[[Mapping[str, Any]], Mapping[str, Any] | BrainEvaluatorDecision],
        *,
        evaluator_id: str,
        evaluator_version: str,
    ) -> None:
        if not callable(evaluator):
            raise BrainRunError("evaluator must be callable")
        self.evaluator = evaluator
        self.evaluator_id = evaluator_id
        self.evaluator_version = evaluator_version
        BrainEvaluatorDecision(
            evaluator_id=evaluator_id,
            evaluator_version=evaluator_version,
            reward=0.0,
            passed=False,
        )

    def assess(
        self,
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
        *,
        evidence: Mapping[str, Any] | None = None,
    ) -> BrainEvaluatorDecision:
        evaluation_input = build_brain_evaluation_input(result, evidence=evidence)
        return self._assess_input(evaluation_input)

    def _assess_input(self, evaluation_input: Mapping[str, Any]) -> BrainEvaluatorDecision:
        try:
            raw_decision = self.evaluator(evaluation_input)
        except Exception as error:
            raise BrainRunError("evaluator callback failed") from error
        if isinstance(raw_decision, BrainEvaluatorDecision):
            if (
                raw_decision.evaluator_id != self.evaluator_id
                or raw_decision.evaluator_version != self.evaluator_version
            ):
                raise BrainRunError("evaluator decision identity does not match the adapter")
            decision = raw_decision
        else:
            if not isinstance(raw_decision, Mapping):
                raise BrainRunError("evaluator callback must return a decision object")
            BrainLearningLedger._assert_safe(raw_decision)
            unknown_fields = set(raw_decision) - self._ALLOWED_DECISION_FIELDS
            if unknown_fields:
                raise BrainRunError("evaluator decision contains unsupported fields")
            if "reward" not in raw_decision or "passed" not in raw_decision:
                raise BrainRunError("evaluator decision requires reward and passed")
            passed = raw_decision["passed"]
            if not isinstance(passed, bool):
                raise BrainRunError("evaluator decision passed must be boolean")
            failed = raw_decision.get("failed", not passed)
            if not isinstance(failed, bool):
                raise BrainRunError("evaluator decision failed must be boolean")
            decision = BrainEvaluatorDecision(
                evaluator_id=self.evaluator_id,
                evaluator_version=self.evaluator_version,
                reward=raw_decision["reward"],
                passed=passed,
                failed=failed,
                feedback_digest=raw_decision.get("feedback_digest"),
                failure_class=raw_decision.get("failure_class"),
                evidence_digest=raw_decision.get("evidence_digest"),
                replan_requested=raw_decision.get("replan_requested", False),
                replan_instruction=raw_decision.get("replan_instruction"),
            )
        expected_evidence_digest = evaluation_input.get("evidence_digest")
        if decision.evidence_digest is not None and decision.evidence_digest != expected_evidence_digest:
            raise BrainRunError("evaluator decision evidence_digest does not match evidence")
        if decision.evidence_digest is None and expected_evidence_digest is not None:
            decision = replace(decision, evidence_digest=expected_evidence_digest)
        return decision

    def assess_value_only_input(self, evaluation_input: Mapping[str, Any]) -> BrainEvaluatorDecision:
        """Assess a replayed, already-projected input without requiring a live provider result.

        Offline replay may retain only a caller-owned evidence packet and its digest. This public
        seam applies the same decision validation as a live run while keeping prompts, responses,
        credentials, and tool envelopes out of the replay path.
        """

        if not isinstance(evaluation_input, Mapping):
            raise BrainRunError("value-only evaluator input must be a mapping")
        BrainLearningLedger._assert_safe(evaluation_input)
        try:
            encoded = json.dumps(
                dict(evaluation_input),
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            ).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise BrainRunError("value-only evaluator input must be JSON-safe") from error
        if len(encoded) > MAX_BRAIN_EVALUATOR_INPUT_BYTES:
            raise BrainRunError("value-only evaluator input exceeds the bounded size")
        return self._assess_input(json.loads(encoded.decode("utf-8")))

    def evaluate_episode(
        self,
        brain: AutonomousBrain,
        episode: BrainLearningEpisode | Mapping[str, Any],
        *,
        bandit_state: Mapping[str, Any],
        evidence: Mapping[str, Any] | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> tuple[BrainEvaluatorDecision, dict[str, Any]]:
        """Evaluate and settle a delayed episode after a restart or later human review."""

        if not isinstance(brain, AutonomousBrain):
            raise BrainRunError("brain must be an AutonomousBrain")
        normalized_episode = episode if isinstance(episode, BrainLearningEpisode) else BrainLearningEpisode.from_mapping(episode)
        if normalized_episode.status != "pending":
            raise BrainRunError("learning episode is already settled")
        if ledger is not None and normalized_episode.episode_id not in {
            item.episode_id for item in ledger.pending_episodes(limit=ledger.max_records)
        }:
            raise BrainRunError("learning episode is already settled or was not registered")
        evaluation_input = build_brain_evaluation_input_from_metadata(
            normalized_episode.evaluation_input,
            evidence=evidence,
        )
        decision = self._assess_input(evaluation_input)
        replay = {
            "schema": BRAIN_EVALUATOR_REPLAY_SCHEMA,
            "episode_id": normalized_episode.episode_id,
            "result_kind": evaluation_input.get("result_kind"),
            "run_id": evaluation_input.get("run_id"),
            "outcome_digest": evaluation_input.get("learning_outcome_digest", evaluation_input.get("outcome_digest")),
            "evaluation_input_digest": _json_digest(evaluation_input),
            "evidence_digest": evaluation_input.get("evidence_digest"),
            "evaluator_id": decision.evaluator_id,
            "evaluator_version": decision.evaluator_version,
            "decision_digest": _json_digest(decision.to_dict()),
            "retention": "metadata_and_digests_only",
        }
        report = brain.record_value_only_evaluator_outcome(
            normalized_episode,
            bandit_state=bandit_state,
            evaluator_id=decision.evaluator_id,
            evaluator_version=decision.evaluator_version,
            reward=decision.reward,
            passed=decision.passed,
            failed=decision.failed,
            feedback_digest=decision.feedback_digest,
            failure_class=decision.failure_class,
            evidence=evidence,
            ledger=ledger,
            replay_metadata=replay,
        )
        return decision, report

    def settle_episode(
        self,
        brain: AutonomousBrain,
        episode: BrainLearningEpisode | Mapping[str, Any],
        *,
        decision: BrainEvaluatorDecision,
        bandit_state: Mapping[str, Any],
        ledger: BrainLearningLedger | None = None,
    ) -> tuple[BrainEvaluatorDecision, dict[str, Any]]:
        """Settle one already-validated value-only decision after a process restart.

        This seam is for evaluator workers that have already inspected their caller-owned
        evidence and retained only the bounded decision projection. It deliberately does not
        accept an evidence packet or invoke an evaluator callback. The episode identity,
        evaluator identity, evidence digest, and replay envelope remain bound before the Rust
        kernel receives the reward.
        """

        if not isinstance(brain, AutonomousBrain):
            raise BrainRunError("brain must be an AutonomousBrain")
        normalized_episode = episode if isinstance(episode, BrainLearningEpisode) else BrainLearningEpisode.from_mapping(episode)
        if normalized_episode.status != "pending":
            raise BrainRunError("learning episode is already settled")
        if not isinstance(decision, BrainEvaluatorDecision):
            raise BrainRunError("learning decision must be a BrainEvaluatorDecision")
        if (
            decision.evaluator_id != self.evaluator_id
            or decision.evaluator_version != self.evaluator_version
        ):
            raise BrainRunError("learning decision identity does not match the adapter")
        if not -1.0 <= float(decision.reward) <= 1.0:
            raise BrainRunError("learning decision reward must be within [-1, 1]")
        if ledger is not None and normalized_episode.episode_id not in {
            item.episode_id for item in ledger.pending_episodes(limit=ledger.max_records)
        }:
            raise BrainRunError("learning episode is already settled or was not registered")
        evaluation_input = build_brain_evaluation_input_from_metadata(normalized_episode.evaluation_input)
        expected_evidence_digest = evaluation_input.get("evidence_digest")
        if decision.evidence_digest != expected_evidence_digest and not (
            decision.evidence_digest is None and expected_evidence_digest is None
        ):
            raise BrainRunError("learning decision evidence_digest does not match the episode")
        replay = {
            "schema": BRAIN_EVALUATOR_REPLAY_SCHEMA,
            "episode_id": normalized_episode.episode_id,
            "result_kind": evaluation_input.get("result_kind"),
            "run_id": evaluation_input.get("run_id"),
            "outcome_digest": evaluation_input.get(
                "learning_outcome_digest", evaluation_input.get("outcome_digest")
            ),
            "evaluation_input_digest": _json_digest(evaluation_input),
            "evidence_digest": expected_evidence_digest,
            "evaluator_id": decision.evaluator_id,
            "evaluator_version": decision.evaluator_version,
            "decision_digest": _json_digest(decision.to_dict()),
            "retention": "metadata_and_digests_only",
        }
        report = brain.record_value_only_evaluator_outcome(
            normalized_episode,
            bandit_state=bandit_state,
            evaluator_id=decision.evaluator_id,
            evaluator_version=decision.evaluator_version,
            reward=decision.reward,
            passed=decision.passed,
            failed=decision.failed,
            feedback_digest=decision.feedback_digest,
            failure_class=decision.failure_class,
            ledger=ledger,
            replay_metadata=replay,
        )
        return decision, report

    def evaluate_trajectory(
        self,
        brain: AutonomousBrain,
        trajectory: BrainLearningTrajectory | Mapping[str, Any],
        *,
        bandit_state: Mapping[str, Any],
        evidence_by_step: Sequence[Mapping[str, Any] | None] | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> BrainLearningTrajectoryResult:
        """Evaluate and settle an ordered trajectory with bounded discounted return-to-go credit.

        Every step is assessed through the same value-only evaluator contract. The resulting
        credit for step ``i`` is ``reward_i + discount * return_(i+1)`` (clamped to ``[-1, 1]``),
        optionally seeded by ``terminal_reward``. This lets a late synthesis or human judgment
        influence earlier model choices while preserving per-step evaluator identity and ledger
        idempotency. It is deliberately not a claim that transport success or a provider text is
        intrinsically rewarding.
        """

        if not isinstance(brain, AutonomousBrain):
            raise BrainRunError("brain must be an AutonomousBrain")
        normalized = trajectory if isinstance(trajectory, BrainLearningTrajectory) else BrainLearningTrajectory.from_mapping(trajectory)
        if evidence_by_step is not None:
            if not isinstance(evidence_by_step, Sequence) or isinstance(evidence_by_step, (str, bytes)):
                raise BrainRunError("trajectory evidence_by_step must be a sequence or None")
            if len(evidence_by_step) != len(normalized.episodes) or any(
                item is not None and not isinstance(item, Mapping) for item in evidence_by_step
            ):
                raise BrainRunError("trajectory evidence_by_step must match the trajectory")
        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("trajectory bandit_state must be a mapping")
        BrainLearningLedger._assert_safe(bandit_state)
        if ledger is not None:
            pending_ids = {item.episode_id for item in ledger.pending_episodes(limit=ledger.max_records)}
            missing = [episode.episode_id for episode in normalized.episodes if episode.episode_id not in pending_ids]
            if missing:
                raise BrainRunError("trajectory contains an episode that is already settled or was not registered")

        decisions: list[BrainEvaluatorDecision] = []
        evaluation_inputs: list[Mapping[str, Any]] = []
        for index, episode in enumerate(normalized.episodes):
            evaluation_input = build_brain_evaluation_input_from_metadata(
                episode.evaluation_input,
                evidence=None if evidence_by_step is None else evidence_by_step[index],
            )
            decision = self._assess_input(evaluation_input)
            if not -1.0 <= float(decision.reward) <= 1.0:
                raise BrainRunError("trajectory evaluator rewards must be within [-1, 1]")
            decisions.append(decision)
            evaluation_inputs.append(evaluation_input)

        return self.settle_trajectory(
            brain,
            normalized,
            decisions=decisions,
            bandit_state=bandit_state,
            evidence_by_step=evidence_by_step,
            ledger=ledger,
        )

    def settle_trajectory(
        self,
        brain: AutonomousBrain,
        trajectory: BrainLearningTrajectory | Mapping[str, Any],
        *,
        decisions: Sequence[BrainEvaluatorDecision],
        bandit_state: Mapping[str, Any],
        evidence_by_step: Sequence[Mapping[str, Any] | None] | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> BrainLearningTrajectoryResult:
        """Settle precomputed decisions without invoking the evaluator callback again.

        Online re-planning can inspect a decision immediately while deferring its bandit write
        until the attempt sequence is complete. This seam makes that safe: the callback runs
        once, but the eventual discounted credit is still the only value written to the ledger.
        """

        if not isinstance(brain, AutonomousBrain):
            raise BrainRunError("brain must be an AutonomousBrain")
        normalized = trajectory if isinstance(trajectory, BrainLearningTrajectory) else BrainLearningTrajectory.from_mapping(trajectory)
        if not isinstance(decisions, Sequence) or isinstance(decisions, (str, bytes)):
            raise BrainRunError("trajectory decisions must be a sequence")
        if len(decisions) != len(normalized.episodes) or any(
            not isinstance(decision, BrainEvaluatorDecision) for decision in decisions
        ):
            raise BrainRunError("trajectory decisions must match the trajectory")
        if any(
            decision.evaluator_id != self.evaluator_id or decision.evaluator_version != self.evaluator_version
            for decision in decisions
        ):
            raise BrainRunError("trajectory decision identity does not match the adapter")
        if evidence_by_step is not None:
            if not isinstance(evidence_by_step, Sequence) or isinstance(evidence_by_step, (str, bytes)):
                raise BrainRunError("trajectory evidence_by_step must be a sequence or None")
            if len(evidence_by_step) != len(normalized.episodes) or any(
                item is not None and not isinstance(item, Mapping) for item in evidence_by_step
            ):
                raise BrainRunError("trajectory evidence_by_step must match the trajectory")
        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("trajectory bandit_state must be a mapping")
        BrainLearningLedger._assert_safe(bandit_state)
        if ledger is not None:
            pending_ids = {item.episode_id for item in ledger.pending_episodes(limit=ledger.max_records)}
            missing = [episode.episode_id for episode in normalized.episodes if episode.episode_id not in pending_ids]
            if missing:
                raise BrainRunError("trajectory contains an episode that is already settled or was not registered")
        evaluation_inputs: list[Mapping[str, Any]] = []
        for index, (episode, decision) in enumerate(zip(normalized.episodes, decisions)):
            evaluation_input = build_brain_evaluation_input_from_metadata(
                episode.evaluation_input,
                evidence=None if evidence_by_step is None else evidence_by_step[index],
            )
            expected_evidence_digest = evaluation_input.get("evidence_digest")
            if decision.evidence_digest != expected_evidence_digest and not (
                decision.evidence_digest is None and expected_evidence_digest is None
            ):
                raise BrainRunError("trajectory decision evidence_digest does not match evidence")
            if not -1.0 <= float(decision.reward) <= 1.0:
                raise BrainRunError("trajectory evaluator rewards must be within [-1, 1]")
            evaluation_inputs.append(evaluation_input)

        credited_rewards = [0.0] * len(decisions)
        running = 0.0 if normalized.terminal_reward is None else normalized.terminal_reward
        for index in range(len(decisions) - 1, -1, -1):
            running = max(-1.0, min(1.0, float(decisions[index].reward) + normalized.discount * running))
            credited_rewards[index] = running

        state: Mapping[str, Any] = dict(bandit_state)
        recordings: list[Mapping[str, Any]] = []
        for index, (episode, decision, evaluation_input, credited_reward) in enumerate(
            zip(normalized.episodes, decisions, evaluation_inputs, credited_rewards)
        ):
            replay = {
                "schema": BRAIN_EVALUATOR_REPLAY_SCHEMA,
                "episode_id": episode.episode_id,
                "trajectory_id": normalized.trajectory_id,
                "trajectory_step": index,
                "trajectory_length": len(normalized.episodes),
                "discount": normalized.discount,
                "terminal_reward": normalized.terminal_reward,
                "raw_reward": decision.reward,
                "credited_reward": credited_reward,
                "result_kind": evaluation_input.get("result_kind"),
                "run_id": evaluation_input.get("run_id"),
                "outcome_digest": evaluation_input.get(
                    "learning_outcome_digest", evaluation_input.get("outcome_digest")
                ),
                "evaluation_input_digest": _json_digest(evaluation_input),
                "evidence_digest": evaluation_input.get("evidence_digest"),
                "evaluator_id": decision.evaluator_id,
                "evaluator_version": decision.evaluator_version,
                "decision_digest": _json_digest(decision.to_dict()),
                "retention": "metadata_and_digests_only",
            }
            report = brain.record_value_only_evaluator_outcome(
                episode,
                bandit_state=state,
                evaluator_id=decision.evaluator_id,
                evaluator_version=decision.evaluator_version,
                reward=credited_reward,
                passed=decision.passed,
                failed=decision.failed,
                feedback_digest=decision.feedback_digest,
                failure_class=decision.failure_class,
                evidence=None if evidence_by_step is None else evidence_by_step[index],
                ledger=ledger,
                replay_metadata=replay,
            )
            next_state = report.get("next_state")
            if isinstance(next_state, Mapping):
                state = dict(next_state)
            recordings.append(dict(report))
        return BrainLearningTrajectoryResult(
            status="settled",
            trajectory=normalized,
            decisions=tuple(decisions),
            recordings=tuple(recordings),
            credited_rewards=tuple(credited_rewards),
            bandit_state=state,
        )

    def evaluate_and_record(
        self,
        brain: AutonomousBrain,
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
        *,
        bandit_state: Mapping[str, Any],
        evidence: Mapping[str, Any] | None = None,
        arm_id: str | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> dict[str, Any]:
        """Evaluate and persist an outcome, preserving the historical report-only API."""

        _decision, report = self.evaluate_and_record_with_decision(
            brain,
            result,
            bandit_state=bandit_state,
            evidence=evidence,
            arm_id=arm_id,
            ledger=ledger,
        )
        return report

    def evaluate_and_record_with_decision(
        self,
        brain: AutonomousBrain,
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
        *,
        bandit_state: Mapping[str, Any],
        evidence: Mapping[str, Any] | None = None,
        arm_id: str | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> tuple[BrainEvaluatorDecision, dict[str, Any]]:
        """Return the compact evaluator decision alongside the persisted Rust report."""

        if not isinstance(brain, AutonomousBrain):
            raise BrainRunError("brain must be an AutonomousBrain")
        evaluation_input = build_brain_evaluation_input(result, evidence=evidence)
        decision = self._assess_input(evaluation_input)
        replay = {
            "schema": BRAIN_EVALUATOR_REPLAY_SCHEMA,
            "result_kind": evaluation_input["result_kind"],
            "run_id": evaluation_input["run_id"],
            "outcome_digest": evaluation_input.get("learning_outcome_digest", evaluation_input["outcome_digest"]),
            "learning_outcome_digest": evaluation_input.get("learning_outcome_digest"),
            "evaluation_input_digest": _json_digest(evaluation_input),
            "evidence_digest": evaluation_input.get("evidence_digest"),
            "evaluator_id": decision.evaluator_id,
            "evaluator_version": decision.evaluator_version,
            "decision_digest": _json_digest(decision.to_dict()),
            "retention": "metadata_and_digests_only",
        }
        report = brain.record_evaluator_outcome(
            result,
            bandit_state=bandit_state,
            evaluator_id=decision.evaluator_id,
            evaluator_version=decision.evaluator_version,
            reward=decision.reward,
            passed=decision.passed,
            arm_id=arm_id,
            failed=decision.failed,
            feedback_digest=decision.feedback_digest,
            failure_class=decision.failure_class,
            evidence_digest=decision.evidence_digest,
            ledger=ledger,
            replay_metadata=replay,
        )
        return decision, report


@dataclass(frozen=True, slots=True)
class AutonomousEvaluatorMeshResult:
    """Value-only quorum result for independent evaluator decisions.

    A mesh result intentionally contains no evaluator exception, instruction, provider output, or
    evidence packet.  It is an audit projection: disagreement and member failure are first-class
    outcomes and cannot be averaged into a reward that would train the bandit.
    """

    schema: str
    status: str
    evaluator_id: str
    evaluator_version: str
    reward: float | None
    passed: bool | None
    failed: bool
    replan_requested: bool
    feedback_digest: str | None
    evidence_digest: str | None
    failure_class: str | None
    reward_spread: float | None
    max_reward_spread: float
    member_results: tuple[Mapping[str, Any], ...]
    member_results_digest: str
    mesh_digest: str
    retention: str = "value_only_evaluator_mesh"
    secret_material: str = "never_returned"

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "status": self.status,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "reward": self.reward,
            "passed": self.passed,
            "failed": self.failed,
            "replan_requested": self.replan_requested,
            "feedback_digest": self.feedback_digest,
            "evidence_digest": self.evidence_digest,
            "failure_class": self.failure_class,
            "reward_spread": self.reward_spread,
            "max_reward_spread": self.max_reward_spread,
            "member_results": [dict(member) for member in self.member_results],
            "member_results_digest": self.member_results_digest,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }

    def __post_init__(self) -> None:
        if self.schema != BRAIN_EVALUATOR_MESH_SCHEMA:
            raise BrainRunError("evaluator mesh result has an invalid schema")
        if self.status not in {"accepted", "disagreement", "member_error"}:
            raise BrainRunError("evaluator mesh result has an invalid status")
        BrainEvaluatorDecision(
            evaluator_id=self.evaluator_id,
            evaluator_version=self.evaluator_version,
            reward=0.0 if self.reward is None else self.reward,
            passed=False if self.passed is None else self.passed,
            failed=self.failed,
            feedback_digest=self.feedback_digest,
            failure_class=self.failure_class,
            evidence_digest=self.evidence_digest,
            replan_requested=self.replan_requested,
            replan_instruction=None,
        )
        if self.reward is not None and not -1.0 <= float(self.reward) <= 1.0:
            raise BrainRunError("evaluator mesh reward must be within [-1, 1]")
        if self.reward_spread is not None and not 0.0 <= float(self.reward_spread) <= 2.0:
            raise BrainRunError("evaluator mesh reward_spread must be within [0, 2]")
        if not isinstance(self.max_reward_spread, (int, float)) or isinstance(self.max_reward_spread, bool) or not 0.0 <= float(self.max_reward_spread) <= 1.0:
            raise BrainRunError("evaluator mesh max_reward_spread must be within [0, 1]")
        if not isinstance(self.member_results, Sequence) or isinstance(self.member_results, (str, bytes)) or not 2 <= len(self.member_results) <= 8:
            raise BrainRunError("evaluator mesh result must contain between 2 and 8 member results")
        BrainLearningLedger._assert_safe(self.member_results)
        if self.status == "accepted" and (self.reward is None or self.passed is None):
            raise BrainRunError("accepted evaluator mesh result must contain reward and passed")
        if self.status != "accepted" and (self.reward is not None or self.passed is not None):
            raise BrainRunError("refused evaluator mesh result cannot contain learning credit")
        member_ids: set[str] = set()
        allowed_member_fields = {
            "evaluator_id",
            "evaluator_version",
            "reward",
            "passed",
            "failed",
            "replan_requested",
            "feedback_digest",
            "evidence_digest",
            "failure_class",
            "decision_digest",
        }
        for member in self.member_results:
            if not isinstance(member, Mapping) or set(member).difference(allowed_member_fields):
                raise BrainRunError("evaluator mesh member result contains unsupported fields")
            member_id = member.get("evaluator_id")
            member_version = member.get("evaluator_version")
            if not isinstance(member_id, str) or not member_id.strip() or not isinstance(member_version, str) or not member_version.strip() or member_id in member_ids:
                raise BrainRunError("evaluator mesh member identity is malformed or duplicated")
            member_ids.add(member_id)
            reward = member.get("reward")
            passed = member.get("passed")
            if reward is None or passed is None:
                if member.get("failure_class") not in {"evaluator_member_error", "evaluator_member_invalid"}:
                    raise BrainRunError("evaluator mesh member refusal is missing its failure class")
            else:
                BrainEvaluatorDecision(
                    evaluator_id=member_id,
                    evaluator_version=member_version,
                    reward=reward,
                    passed=passed,
                    failed=member.get("failed", not passed),
                    feedback_digest=member.get("feedback_digest"),
                    failure_class=member.get("failure_class"),
                    evidence_digest=member.get("evidence_digest"),
                    replan_requested=member.get("replan_requested", False),
                )
            decision_digest = member.get("decision_digest")
            if decision_digest is not None:
                without_digest = dict(member)
                without_digest.pop("decision_digest", None)
                if decision_digest != _json_digest(without_digest):
                    raise BrainRunError("evaluator mesh member decision_digest is invalid")
        if not _valid_digest(self.member_results_digest) or _json_digest(list(self.member_results)) != self.member_results_digest:
            raise BrainRunError("evaluator mesh member_results_digest is invalid")
        if self.retention != "value_only_evaluator_mesh" or self.secret_material != "never_returned":
            raise BrainRunError("evaluator mesh retention markers are invalid")
        if self.mesh_digest != _json_digest(self._descriptor()):
            raise BrainRunError("evaluator mesh mesh_digest is invalid")

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "mesh_digest": self.mesh_digest}


class AutonomousEvaluatorMesh(BrainOutcomeEvaluator):
    """Gate Python learning credit on agreement from bounded independent evaluators.

    The class is a normal :class:`BrainOutcomeEvaluator`, so it can be passed directly to
    ``evaluate_and_record``, workflow learning, trajectory settlement, mission learning, or
    offline value-only replay.  Each member receives the same projected evaluator input.  A
    member exception or disagreement raises at the learning boundary; callers can inspect the
    value-only refusal with :meth:`evaluate_detailed` without granting credit.
    """

    def __init__(
        self,
        members: Sequence[BrainOutcomeEvaluator],
        *,
        evaluator_id: str = "python-evaluator-mesh",
        evaluator_version: str = "0.1",
        max_reward_spread: float = 0.1,
    ) -> None:
        if not isinstance(members, Sequence) or isinstance(members, (str, bytes)) or not 2 <= len(members) <= 8:
            raise BrainRunError("evaluator mesh requires between 2 and 8 independent members")
        if not isinstance(max_reward_spread, (int, float)) or isinstance(max_reward_spread, bool) or not 0.0 <= float(max_reward_spread) <= 1.0:
            raise BrainRunError("evaluator mesh max_reward_spread must be within [0, 1]")
        normalized = tuple(members)
        if any(not isinstance(member, BrainOutcomeEvaluator) for member in normalized):
            raise BrainRunError("evaluator mesh members must be BrainOutcomeEvaluator instances")
        identities = [(member.evaluator_id, member.evaluator_version) for member in normalized]
        if len({identity[0] for identity in identities}) != len(identities):
            raise BrainRunError("evaluator mesh member evaluator_id values must be unique")
        self.members = normalized
        self.max_reward_spread = float(max_reward_spread)
        super().__init__(
            lambda _input: {"reward": 0.0, "passed": False},
            evaluator_id=evaluator_id,
            evaluator_version=evaluator_version,
        )

    @staticmethod
    def _projection(member: BrainOutcomeEvaluator, decision: BrainEvaluatorDecision) -> dict[str, Any]:
        projection = {
            "evaluator_id": member.evaluator_id,
            "evaluator_version": member.evaluator_version,
            "reward": float(decision.reward),
            "passed": decision.passed,
            "failed": decision.failed,
            "replan_requested": decision.replan_requested,
            "feedback_digest": decision.feedback_digest,
            "evidence_digest": decision.evidence_digest,
            "failure_class": decision.failure_class,
        }
        projection["decision_digest"] = _json_digest(projection)
        return projection

    def _evaluate_input(self, evaluation_input: Mapping[str, Any]) -> AutonomousEvaluatorMeshResult:
        if not isinstance(evaluation_input, Mapping):
            raise BrainRunError("evaluator mesh input must be a mapping")
        expected_evidence_digest = evaluation_input.get("evidence_digest")
        if expected_evidence_digest is not None and not _valid_digest(expected_evidence_digest):
            raise BrainRunError("evaluator mesh input evidence_digest is malformed")
        projections: list[dict[str, Any]] = []
        for member in self.members:
            try:
                decision = member._assess_input(evaluation_input)
                projections.append(self._projection(member, decision))
            except Exception:
                projections.append(
                    {
                        "evaluator_id": member.evaluator_id,
                        "evaluator_version": member.evaluator_version,
                        "reward": None,
                        "passed": None,
                        "failed": True,
                        "replan_requested": True,
                        "feedback_digest": None,
                        "evidence_digest": None,
                        "failure_class": "evaluator_member_error",
                        "decision_digest": None,
                    }
                )
        member_results_digest = _json_digest(projections)
        member_error = any(item["failure_class"] == "evaluator_member_error" for item in projections)
        status = "accepted"
        reward: float | None = None
        passed: bool | None = None
        failed = False
        replan_requested = False
        failure_class: str | None = None
        reward_spread: float | None = None
        if member_error:
            status = "member_error"
            failed = True
            replan_requested = True
            failure_class = "evaluator_mesh_member_error"
        else:
            rewards = [float(item["reward"]) for item in projections]
            reward_spread = round(max(rewards) - min(rewards), 12)
            first = projections[0]
            agreement = all(
                item["passed"] == first["passed"]
                and item["failed"] == first["failed"]
                and item["replan_requested"] == first["replan_requested"]
                and item["failure_class"] == first["failure_class"]
                for item in projections
            ) and reward_spread <= self.max_reward_spread
            if agreement:
                reward = round(sum(rewards) / len(rewards), 12)
                passed = bool(first["passed"])
                failed = bool(first["failed"])
                replan_requested = bool(first["replan_requested"])
                failure_class = first["failure_class"]
            else:
                status = "disagreement"
                failed = True
                replan_requested = True
                failure_class = "evaluator_disagreement"
        descriptor = {
            "schema": BRAIN_EVALUATOR_MESH_SCHEMA,
            "status": status,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "reward": reward,
            "passed": passed,
            "failed": failed,
            "replan_requested": replan_requested,
            "feedback_digest": member_results_digest,
            "evidence_digest": expected_evidence_digest,
            "failure_class": failure_class,
            "reward_spread": reward_spread,
            "max_reward_spread": self.max_reward_spread,
            "member_results": projections,
            "member_results_digest": member_results_digest,
            "retention": "value_only_evaluator_mesh",
            "secret_material": "never_returned",
        }
        return AutonomousEvaluatorMeshResult(
            **descriptor,
            mesh_digest=_json_digest(descriptor),
        )

    def evaluate_detailed(
        self,
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
        *,
        evidence: Mapping[str, Any] | None = None,
    ) -> AutonomousEvaluatorMeshResult:
        """Return the bounded mesh projection without granting or recording learning credit."""

        return self._evaluate_input(build_brain_evaluation_input(result, evidence=evidence))

    def evaluate_detailed_value_only_input(self, evaluation_input: Mapping[str, Any]) -> AutonomousEvaluatorMeshResult:
        """Evaluate a caller-rehydrated projected input without provider access."""

        if not isinstance(evaluation_input, Mapping):
            raise BrainRunError("evaluator mesh value-only input must be a mapping")
        BrainLearningLedger._assert_safe(evaluation_input)
        try:
            encoded = json.dumps(
                dict(evaluation_input),
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            ).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise BrainRunError("evaluator mesh value-only input must be JSON-safe") from error
        if len(encoded) > MAX_BRAIN_EVALUATOR_INPUT_BYTES:
            raise BrainRunError("evaluator mesh value-only input exceeds the bounded size")
        return self._evaluate_input(json.loads(encoded.decode("utf-8")))

    def _assess_input(self, evaluation_input: Mapping[str, Any]) -> BrainEvaluatorDecision:
        mesh = self._evaluate_input(evaluation_input)
        if mesh.status != "accepted" or mesh.reward is None or mesh.passed is None:
            raise BrainRunError(f"evaluator mesh refused learning credit: {mesh.failure_class or mesh.status}")
        return BrainEvaluatorDecision(
            evaluator_id=self.evaluator_id,
            evaluator_version=self.evaluator_version,
            reward=mesh.reward,
            passed=mesh.passed,
            failed=mesh.failed,
            feedback_digest=mesh.feedback_digest,
            failure_class=mesh.failure_class,
            evidence_digest=mesh.evidence_digest,
            replan_requested=mesh.replan_requested,
        )
