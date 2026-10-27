"""Bounded Ensembl TP53 gene and Entrez-reference metadata retrieval.

This repository-defined extension has no available source blueprint identifier. It reads two
fixed public annotation routes, never individual samples, sequences, variants or expanded gene
features. Cross-references remain source assertions, not a verified scientific identity join.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from datetime import datetime, timezone
import hashlib
import http.client
import ipaddress
import json
import math
import queue
import re
import secrets
import socket
import ssl
import threading
import time
from typing import Any

from .authoring import canonical_json, content_digest
from .workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult


ENSEMBL_GENE_BUNDLE_SCHEMA = "aurora-reviewed-ensembl-gene-references/0.1"
ENSEMBL_GENE_RECEIPT_SCHEMA = "aurora-reviewed-ensembl-gene-receipt/0.1"
ENSEMBL_GENE_OPERATION_ID = "ensembl.gene_references"
MAX_ENSEMBL_RESPONSE_BYTES = 1_000_000
MAX_ENSEMBL_TOTAL_BYTES = 2_000_000
MAX_ENSEMBL_TREE_DEPTH = 32
MAX_ENSEMBL_TREE_NODES = 50_000
MAX_ENSEMBL_XREFS = 64
_HOST = "rest.ensembl.org"
_BASE = f"https://{_HOST}"
_LOOKUP_URL = _BASE + "/lookup/symbol/homo_sapiens/TP53?expand=0"
_LOOKUP_FIELDS = ("id", "display_name", "species", "object_type", "db_type", "version",
                  "assembly_name", "biotype", "source", "description")
_XREF_FIELDS = ("primary_id", "display_id", "dbname", "version", "info_type", "description")
_ENSG = re.compile(r"^ENSG[0-9]{11}$")
_DIGEST = re.compile(r"^[a-f0-9]{64}$")
_UTC = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")
_DNS_SLOTS = threading.BoundedSemaphore(4)
_LIMITATIONS = [
    "Only the fixed TP53 human/core gene lookup and gene-level EntrezGene references are requested.",
    "Every retained Entrez reference remains a source-asserted candidate; no independently validated identity join is performed.",
    "The source does not report a total or pagination contract for these lookups; exhaustiveness remains unassessed.",
    "No variants, sequences, transcripts, exons, samples, individual records or clinical actions are requested or projected.",
    "Source annotations and free text are untrusted metadata, not scientific validation or treatment guidance.",
    "Digests bind retained bytes and projections; they are not provider signatures or independent provenance verification.",
]


class EnsemblGeneRetrievalError(ValueError):
    """A bounded failure without server payload, raw headers or exception details."""

    def __init__(self, failure_class: str, *, status_code: int | None = None,
                 retry_after_seconds: float | None = None) -> None:
        super().__init__(f"Ensembl gene retrieval refused: {failure_class}")
        self.failure_class = failure_class
        self.status_code = status_code
        self.retry_after_seconds = retry_after_seconds

    def to_dict(self) -> dict[str, Any]:
        return {"failure_class": self.failure_class, "status_code": self.status_code,
                "retry_after_seconds": self.retry_after_seconds, "automatic_retry": False}


def _fail(code: str) -> None:
    raise EnsemblGeneRetrievalError(code)


@dataclass(frozen=True)
class EnsemblGeneRetrievalConfig:
    gene_symbol: str = "TP53"
    timeout_ms: int = 30_000

    def __post_init__(self) -> None:
        if self.gene_symbol != "TP53" or type(self.gene_symbol) is not str:
            _fail("only_reviewed_TP53_symbol_is_admitted")
        if type(self.timeout_ms) is not int or not 100 <= self.timeout_ms <= 30_000:
            _fail("timeout_ms_outside_100_to_30000")

    def to_dict(self) -> dict[str, Any]:
        return {"gene_symbol": self.gene_symbol, "timeout_ms": self.timeout_ms}


@dataclass(frozen=True)
class EnsemblGeneHttpResponse:
    status: int
    body: bytes = field(repr=False)
    final_url: str
    headers: Mapping[str, str] = field(default_factory=dict, repr=False)


def _remaining(deadline: float) -> float:
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        _fail("total_deadline_exceeded")
    return remaining


def _resolve_public_address(deadline: float) -> tuple[Any, ...]:
    """Bound waiting and outstanding DNS jobs; a late resolver cannot initiate HTTP."""
    if not _DNS_SLOTS.acquire(timeout=_remaining(deadline)):
        _fail("dns_resolver_capacity_or_deadline")
    result: queue.Queue[Any] = queue.Queue(maxsize=1)

    def resolve() -> None:
        try:
            try:
                addresses = socket.getaddrinfo(_HOST, 443, type=socket.SOCK_STREAM)
            except Exception:
                addresses = None
            result.put_nowait(addresses)
        finally:
            _DNS_SLOTS.release()

    try:
        threading.Thread(target=resolve, name="aurora-ensembl-dns", daemon=True).start()
    except Exception:
        _DNS_SLOTS.release()
        raise EnsemblGeneRetrievalError("dns_resolver_unavailable") from None
    try:
        addresses = result.get(timeout=_remaining(deadline))
    except queue.Empty:
        raise EnsemblGeneRetrievalError("total_deadline_exceeded") from None
    _remaining(deadline)
    if not isinstance(addresses, list) or not 1 <= len(addresses) <= 64:
        _fail("dns_resolution_invalid")
    try:
        if any(not ipaddress.ip_address(address[4][0]).is_global
               or ipaddress.ip_address(address[4][0]).is_multicast for address in addresses):
            _fail("dns_address_is_not_public")
        selected = addresses[0]
        if selected[0] not in {socket.AF_INET, socket.AF_INET6} or selected[1] != socket.SOCK_STREAM:
            _fail("dns_address_shape_invalid")
    except EnsemblGeneRetrievalError:
        raise
    except (TypeError, ValueError, IndexError):
        raise EnsemblGeneRetrievalError("dns_address_shape_invalid") from None
    return selected


class _DeadlineReader:
    """HTTP parsing reads through finite recv calls, including slow header/body trickles."""

    def __init__(self, connection: Any, deadline: float) -> None:
        self.connection, self.deadline = connection, deadline
        self.buffer = bytearray()
        self.closed = False
        self.header_bytes = 0
        self.reading_headers = True

    def _receive(self) -> bool:
        self.connection.settimeout(_remaining(self.deadline))
        block = self.connection.recv(4096)
        _remaining(self.deadline)
        self.buffer.extend(block)
        return bool(block)

    def readline(self, limit: int = -1) -> bytes:
        _remaining(self.deadline)
        if limit < 0:
            limit = 65_537
        while True:
            newline = self.buffer.find(b"\n", 0, limit)
            if newline >= 0 or len(self.buffer) >= limit:
                end = newline + 1 if newline >= 0 else limit
                value = bytes(self.buffer[:end])
                del self.buffer[:end]
                if self.reading_headers:
                    self.header_bytes += len(value)
                    if self.header_bytes > 65_536:
                        _fail("response_header_bound")
                return value
            if not self._receive():
                value = bytes(self.buffer)
                self.buffer.clear()
                return value

    def read(self, size: int = -1) -> bytes:
        _remaining(self.deadline)
        if size < 0 or size > MAX_ENSEMBL_RESPONSE_BYTES + 1:
            _fail("transport_read_bound")
        while len(self.buffer) < size and self._receive():
            pass
        value = bytes(self.buffer[:size])
        del self.buffer[:size]
        return value

    def close(self) -> None:
        self.closed = True

    def flush(self) -> None:
        # HTTPResponse flushes its read-only file wrapper during early refusal cleanup.
        return None


class _ResponseSocket:
    def __init__(self, connection: Any, deadline: float) -> None:
        self.reader = _DeadlineReader(connection, deadline)

    def makefile(self, *_args: Any, **_kwargs: Any) -> _DeadlineReader:
        return self.reader


def _default_fetch(url: str, *, timeout_seconds: float,
                   max_response_bytes: int) -> EnsemblGeneHttpResponse:
    if (url != _LOOKUP_URL and not re.fullmatch(
            re.escape(_BASE) + r"/xrefs/id/ENSG[0-9]{11}\?external_db=EntrezGene&object_type=gene&species=homo_sapiens&all_levels=0", url)):
        _fail("transport_route_is_not_reviewed")
    if max_response_bytes != MAX_ENSEMBL_RESPONSE_BYTES:
        _fail("transport_response_bound_changed")
    deadline = time.monotonic() + timeout_seconds
    raw = tls = None
    try:
        address = _resolve_public_address(deadline)
        raw = socket.socket(address[0], address[1], address[2])
        raw.settimeout(_remaining(deadline))
        raw.connect(address[4])
        context = ssl.create_default_context()
        raw.settimeout(_remaining(deadline))
        tls = context.wrap_socket(raw, server_hostname=_HOST)
        target = url[len(_BASE):]
        request = (f"GET {target} HTTP/1.1\r\nHost: {_HOST}\r\nAccept: application/json\r\n"
                   "Content-Type: application/json\r\nUser-Agent: aurora-ensembl-metadata/0.1\r\n"
                   "Connection: close\r\n\r\n").encode("ascii")
        tls.settimeout(_remaining(deadline))
        tls.sendall(request)
        response_socket = _ResponseSocket(tls, deadline)
        response = http.client.HTTPResponse(response_socket)
        try:
            response.begin()
            response_socket.reader.reading_headers = False
            headers = {}
            for name, value in response.getheaders():
                normalized = name.lower()
                if normalized in headers:
                    _fail("ambiguous_response_headers")
                headers[normalized] = value
            if "content-length" in headers and "transfer-encoding" in headers:
                _fail("ambiguous_response_framing")
            body = response.read(max_response_bytes + 1)
            _remaining(deadline)
            return EnsemblGeneHttpResponse(response.status, body, url, headers)
        finally:
            response.close()
    except EnsemblGeneRetrievalError:
        raise
    except (OSError, http.client.HTTPException, ValueError):
        raise EnsemblGeneRetrievalError("transport_failure") from None
    finally:
        if tls is not None:
            tls.close()
        elif raw is not None:
            raw.close()


def _clock(value: str | None) -> str:
    result = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ") if value is None else value
    if not isinstance(result, str) or not _UTC.fullmatch(result):
        _fail("retrieved_at_must_be_whole_seconds_utc")
    try:
        datetime.strptime(result, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError:
        raise EnsemblGeneRetrievalError("retrieved_at_invalid") from None
    return result


def _unique(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result = {}
    for key, value in pairs:
        if key in result:
            _fail("duplicate_json_key")
        result[key] = value
    return result


def _tree(value: Any) -> None:
    count = 0

    def visit(item: Any, depth: int = 0) -> None:
        nonlocal count
        count += 1
        if depth > MAX_ENSEMBL_TREE_DEPTH or count > MAX_ENSEMBL_TREE_NODES:
            _fail("response_tree_bound")
        if item is None or type(item) in {str, bool, int}:
            return
        if type(item) is float and math.isfinite(item):
            return
        if isinstance(item, Mapping):
            for key, child in item.items():
                if not isinstance(key, str):
                    _fail("non_string_object_key")
                visit(key, depth + 1)
                visit(child, depth + 1)
        elif isinstance(item, list):
            for child in item:
                visit(child, depth + 1)
        else:
            _fail("non_finite_or_non_json_value")

    visit(value)


def _parse(body: bytes) -> Any:
    if type(body) is not bytes or len(body) > MAX_ENSEMBL_RESPONSE_BYTES:
        _fail("response_byte_bound")
    try:
        result = json.loads(body.decode("utf-8"), object_pairs_hook=_unique,
                            parse_constant=lambda _constant: _fail("non_finite_json"))
    except EnsemblGeneRetrievalError:
        raise
    except (ValueError, RecursionError, UnicodeDecodeError):
        raise EnsemblGeneRetrievalError("response_is_not_json") from None
    _tree(result)
    return result


def _text(value: Any, label: str, *, nullable: bool = False, maximum: int = 2000) -> str | None:
    if nullable and value is None:
        return None
    if (not isinstance(value, str) or not value.strip() or len(value.encode("utf-8")) > maximum
            or any(ord(char) < 32 or ord(char) == 127 for char in value)):
        _fail(label + "_invalid")
    return value


def _lookup(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict):
        _fail("lookup_object_required")
    for name, expected in {"display_name": "TP53", "species": "homo_sapiens",
                           "object_type": "Gene", "db_type": "core"}.items():
        if type(value.get(name)) is not str or value[name] != expected:
            _fail("lookup_" + name + "_mismatch")
    if not isinstance(value.get("id"), str) or not _ENSG.fullmatch(value["id"]):
        _fail("lookup_ensembl_gene_id_invalid")
    if type(value.get("version")) is not int or not 1 <= value["version"] <= 2**31 - 1:
        _fail("lookup_version_invalid")
    return {name: (_text(value.get(name), "lookup_" + name, nullable=True) if name in {
        "assembly_name", "biotype", "source", "description"} else value[name]) for name in _LOOKUP_FIELDS}


def _xrefs(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list) or len(value) > MAX_ENSEMBL_XREFS:
        _fail("xref_array_bound")
    result = []
    identities = set()
    for row in value:
        if not isinstance(row, dict) or row.get("dbname") != "EntrezGene":
            _fail("xref_database_or_row_invalid")
        primary = row.get("primary_id")
        if not isinstance(primary, str) or not re.fullmatch(r"[1-9][0-9]{0,18}", primary):
            _fail("xref_entrez_gene_id_invalid")
        if primary in identities:
            _fail("duplicate_or_ambiguous_xref_candidate")
        identities.add(primary)
        result.append({name: _text(row.get(name), "xref_" + name,
                                  nullable=name in {"version", "info_type", "description"})
                       for name in _XREF_FIELDS})
    return result


def _retry_after(headers: Mapping[str, str]) -> float | None:
    values = [value for key, value in headers.items() if isinstance(key, str) and key.lower() == "retry-after"]
    if len(values) != 1 or not isinstance(values[0], str) or len(values[0]) > 32:
        return None
    if not re.fullmatch(r"[0-9]{1,5}(?:\.[0-9]{1,6})?", values[0]):
        return None
    number = float(values[0])
    return number if 0 <= number <= 86_400 else None


def _response(response: Any, url: str) -> Any:
    if not isinstance(response, EnsemblGeneHttpResponse) or type(response.status) is not int:
        _fail("transport_response_invalid")
    if response.final_url != url or 300 <= response.status < 400:
        _fail("redirect_refused")
    if not isinstance(response.headers, Mapping) or len(response.headers) > 100:
        _fail("response_headers_invalid")
    if response.status != 200:
        code = {400: "source_lookup_refused", 403: "source_forbidden", 404: "source_route_not_found",
                408: "source_timeout", 429: "source_rate_limited", 503: "source_unavailable"}.get(response.status, "source_http_failure")
        raise EnsemblGeneRetrievalError(code, status_code=response.status,
                                       retry_after_seconds=_retry_after(response.headers) if response.status == 429 else None)
    content_types = [value for key, value in response.headers.items()
                     if isinstance(key, str) and key.lower() == "content-type"]
    if (len(content_types) != 1 or not isinstance(content_types[0], str)
            or content_types[0].split(";", 1)[0].strip().lower() != "application/json"):
        _fail("response_content_type_invalid")
    return _parse(response.body)


def _xref_url(gene_id: str) -> str:
    if not _ENSG.fullmatch(gene_id):
        _fail("derived_ensembl_gene_id_invalid")
    return (_BASE + "/xrefs/id/" + gene_id
            + "?external_db=EntrezGene&object_type=gene&species=homo_sapiens&all_levels=0")


def _transport(mode: str, binding: str) -> dict[str, Any]:
    return {"mode": mode, "binding_id": binding,
            "timing_authority": "adapter_total_deadline" if mode == "default_pinned_https" else "caller_owned_preemption;adapter_checks_elapsed",
            "automatic_retry": False}


def _bundle(config: EnsemblGeneRetrievalConfig, retrieved_at: str, source_inputs: list[dict[str, Any]],
            transport: dict[str, Any]) -> dict[str, Any]:
    if len(source_inputs) != 2:
        _fail("two_source_inputs_required")
    lookup_input, xref_input = source_inputs
    gene_input = _lookup(lookup_input["native_payload"])
    references = _xrefs(xref_input["native_payload"])
    if set(lookup_input["native_payload"]) != set(_LOOKUP_FIELDS):
        _fail("retained_lookup_fields_invalid")
    if any(set(row) != set(_XREF_FIELDS) for row in xref_input["native_payload"]):
        _fail("retained_xref_fields_invalid")
    expected = [("gene_lookup", _LOOKUP_URL), ("entrez_references", _xref_url(gene_input["id"]))]
    total = 0
    for row, (role, url) in zip(source_inputs, expected):
        if set(row) != {"role", "url", "http_status", "response_bytes", "response_digest", "native_payload", "payload_digest"}:
            _fail("source_input_fields_invalid")
        if row["role"] != role or row["url"] != url or type(row["http_status"]) is not int or row["http_status"] != 200:
            _fail("source_input_route_or_status_invalid")
        if type(row["response_bytes"]) is not int or not 1 <= row["response_bytes"] <= MAX_ENSEMBL_RESPONSE_BYTES:
            _fail("source_input_response_bound")
        total += row["response_bytes"]
        if not isinstance(row["response_digest"], str) or not _DIGEST.fullmatch(row["response_digest"]):
            _fail("source_input_response_digest_invalid")
        if row["payload_digest"] != content_digest(row["native_payload"]):
            _fail("source_input_payload_digest_mismatch")
    if total > MAX_ENSEMBL_TOTAL_BYTES:
        _fail("total_response_byte_bound")
    gene = {"requested_symbol": "TP53", "ensembl_gene_id": gene_input["id"],
            **{name: gene_input[name] for name in _LOOKUP_FIELDS if name != "id"}}
    candidates = [{"ensembl_gene_id": gene_input["id"], "entrez_gene_id": row["primary_id"],
                   **{name: row[name] for name in ("display_id", "version", "info_type", "description")}}
                  for row in references]
    receipts = [{"schema": ENSEMBL_GENE_RECEIPT_SCHEMA, "role": row["role"], "source_uri": row["url"],
                 "request_digest": content_digest({"method": "GET", "url": row["url"]}),
                 "response_digest": row["response_digest"], "response_bytes": row["response_bytes"],
                 "payload_digest": row["payload_digest"], "http_status": 200, "retrieved_at": retrieved_at,
                 "transport_mode": transport["mode"], "transport_binding_id": transport["binding_id"]}
                for row in source_inputs]
    result = {"schema": ENSEMBL_GENE_BUNDLE_SCHEMA, "source_id": "ensembl", "retrieved_at": retrieved_at,
              "query": config.to_dict(), "gene": gene, "entrez_candidates": candidates,
              "coverage": {"requested_symbol_count": 1, "returned_gene_count": 1,
                           "returned_candidate_count": len(candidates), "reported_total_count": None,
                           "exhaustiveness": "unassessed", "crosswalk_validation": "source_assertions_only",
                           "pagination": "not_documented", "candidate_state": "empty_success" if not candidates else "source_candidates",
                           "response_byte_count": total, "source_request_count": 2},
              "source_inputs": source_inputs, "receipts": receipts, "transport": transport,
              "limitations": list(_LIMITATIONS)}
    return {**result, "bundle_digest": content_digest(result)}


_DEFAULT_BINDING = content_digest({"adapter": ENSEMBL_GENE_BUNDLE_SCHEMA, "host": _HOST,
                                   "lookup_url": _LOOKUP_URL, "xref_database": "EntrezGene",
                                   "response_bound": MAX_ENSEMBL_RESPONSE_BYTES, "transport": "pinned_https_total_deadline/0.1"})


def retrieve_ensembl_gene_references(
    config: EnsemblGeneRetrievalConfig | None = None, *, approve_source_dispatch: bool = False,
    retrieved_at: str | None = None,
    fetcher: Callable[..., EnsemblGeneHttpResponse] | None = None,
) -> dict[str, Any]:
    """Retrieve exactly two credentialless TP53 annotation responses after explicit approval.

    An injected fetcher is trusted application code and owns network preemption. This boundary
    still rejects late results and never dispatches the second call after its overall deadline.
    """
    return _retrieve(config, approve_source_dispatch=approve_source_dispatch,
                     retrieved_at=retrieved_at, fetcher=fetcher,
                     binding=_DEFAULT_BINDING if fetcher is None else secrets.token_hex(32))


def _retrieve(config: EnsemblGeneRetrievalConfig | None, *, approve_source_dispatch: bool,
              retrieved_at: str | None, fetcher: Callable[..., EnsemblGeneHttpResponse] | None,
              binding: str) -> dict[str, Any]:
    config = EnsemblGeneRetrievalConfig() if config is None else config
    if not isinstance(config, EnsemblGeneRetrievalConfig):
        _fail("config_type_invalid")
    # Reconstruct frozen inputs to detect unsupported object mutation before any request.
    config = EnsemblGeneRetrievalConfig(**config.to_dict())
    if type(approve_source_dispatch) is not bool or approve_source_dispatch is not True:
        _fail("explicit_source_dispatch_approval_required")
    if fetcher is not None and not callable(fetcher):
        _fail("fetcher_must_be_application_owned_callable")
    stamp = _clock(retrieved_at)
    deadline = time.monotonic() + config.timeout_ms / 1000
    inputs = []
    url = _LOOKUP_URL
    for role, normalizer in (("gene_lookup", _lookup), ("entrez_references", _xrefs)):
        try:
            response = (fetcher or _default_fetch)(url, timeout_seconds=_remaining(deadline),
                                                   max_response_bytes=MAX_ENSEMBL_RESPONSE_BYTES)
        except EnsemblGeneRetrievalError:
            raise
        except Exception:
            raise EnsemblGeneRetrievalError("application_transport_failure") from None
        _remaining(deadline)
        native = normalizer(_response(response, url))
        _remaining(deadline)
        inputs.append({"role": role, "url": url, "http_status": 200,
                       "response_bytes": len(response.body), "response_digest": hashlib.sha256(response.body).hexdigest(),
                       "native_payload": native, "payload_digest": content_digest(native)})
        if role == "gene_lookup":
            url = _xref_url(native["id"])
    bundle = _bundle(config, stamp, inputs, _transport("default_pinned_https" if fetcher is None else "caller_supplied", binding))
    _remaining(deadline)
    return bundle


def validate_ensembl_gene_bundle(value: Mapping[str, Any]) -> dict[str, Any]:
    """Defensively replay normalization of retained projections and all derived metadata.

    The original HTTP bytes are not retained; their digests are provenance labels, not independently
    revalidated server observations. Consistently reauthored unkeyed bundles are not authenticated.
    """
    if not isinstance(value, Mapping):
        _fail("bundle_object_required")
    try:
        _tree(value)
        encoded = canonical_json(dict(value))
        if len(encoded.encode("utf-8")) > MAX_ENSEMBL_TOTAL_BYTES:
            _fail("bundle_byte_bound")
        safe = json.loads(encoded)
        transport = safe["transport"]
        if (set(transport) != {"mode", "binding_id", "timing_authority", "automatic_retry"}
                or transport["mode"] not in {"default_pinned_https", "caller_supplied"}
                or not isinstance(transport["binding_id"], str) or not _DIGEST.fullmatch(transport["binding_id"])):
            _fail("bundle_transport_invalid")
        if transport["mode"] == "default_pinned_https" and transport["binding_id"] != _DEFAULT_BINDING:
            _fail("default_transport_binding_mismatch")
        if transport != _transport(transport["mode"], transport["binding_id"]):
            _fail("bundle_transport_posture_mismatch")
        config = EnsemblGeneRetrievalConfig(**safe["query"])
        expected = _bundle(config, _clock(safe["retrieved_at"]), safe["source_inputs"], transport)
        if canonical_json(safe) != canonical_json(expected):
            _fail("bundle_replay_mismatch")
        return expected
    except EnsemblGeneRetrievalError:
        raise
    except (KeyError, TypeError, ValueError, RecursionError, OverflowError):
        raise EnsemblGeneRetrievalError("bundle_shape_invalid") from None


def register_ensembl_gene_operations(
    catalog: WorkflowOperationCatalog, *, fetcher: Callable[..., EnsemblGeneHttpResponse] | None = None,
) -> WorkflowOperationCatalog:
    """Explicitly register one operation; import and registration never contact Ensembl."""
    if not isinstance(catalog, WorkflowOperationCatalog):
        _fail("catalog_type_invalid")
    if fetcher is not None and not callable(fetcher):
        _fail("fetcher_must_be_application_owned_callable")
    binding = _DEFAULT_BINDING if fetcher is None else secrets.token_hex(32)
    description = ("Read fixed human/core TP53 gene identity and every bounded gene-level EntrezGene cross-reference candidate. "
                   "No identity join, patient records, sequences, variants, transcripts or clinical interpretation. "
                   f"Transport binding: {binding}; mode: {'default_pinned_https' if fetcher is None else 'caller_supplied'}.")
    def obj(properties: dict[str, Any]) -> dict[str, Any]:
        return {"type": "object", "properties": properties, "required": list(properties), "additionalProperties": False}

    text = {"type": "string"}
    nullable = {"type": ["string", "null"]}
    digest = {"type": "string", "pattern": "^[a-f0-9]{64}$"}
    gene_schema = obj({"requested_symbol": {"type": "string", "const": "TP53"},
        "ensembl_gene_id": {"type": "string", "pattern": "^ENSG[0-9]{11}$"},
        "display_name": {"type": "string", "const": "TP53"}, "species": {"type": "string", "const": "homo_sapiens"},
        "object_type": {"type": "string", "const": "Gene"}, "db_type": {"type": "string", "const": "core"},
        "version": {"type": "integer", "minimum": 1, "maximum": 2**31 - 1},
        **{name: nullable for name in ("assembly_name", "biotype", "source", "description")}})
    candidate_schema = obj({"ensembl_gene_id": {"type": "string", "pattern": "^ENSG[0-9]{11}$"},
        "entrez_gene_id": {"type": "string", "pattern": "^[1-9][0-9]{0,18}$"}, "display_id": text,
        **{name: nullable for name in ("version", "info_type", "description")}})
    coverage_schema = obj({"requested_symbol_count": {"type": "integer", "const": 1},
        "returned_gene_count": {"type": "integer", "const": 1},
        "returned_candidate_count": {"type": "integer", "minimum": 0, "maximum": MAX_ENSEMBL_XREFS},
        "reported_total_count": {"type": "null"},
        "exhaustiveness": {"type": "string", "const": "unassessed"},
        "crosswalk_validation": {"type": "string", "const": "source_assertions_only"},
        "pagination": {"type": "string", "const": "not_documented"},
        "candidate_state": {"type": "string", "enum": ["source_candidates", "empty_success"]},
        "response_byte_count": {"type": "integer", "minimum": 1, "maximum": MAX_ENSEMBL_TOTAL_BYTES},
        "source_request_count": {"type": "integer", "const": 2}})
    receipt_schema = obj({"schema": {"type": "string", "const": ENSEMBL_GENE_RECEIPT_SCHEMA},
        "role": {"type": "string", "enum": ["gene_lookup", "entrez_references"]}, "source_uri": text,
        "request_digest": digest, "response_digest": digest,
        "response_bytes": {"type": "integer", "minimum": 1, "maximum": MAX_ENSEMBL_RESPONSE_BYTES},
        "payload_digest": digest, "http_status": {"type": "integer", "const": 200}, "retrieved_at": text,
        "transport_mode": {"type": "string", "enum": ["default_pinned_https", "caller_supplied"]}, "transport_binding_id": digest})
    input_schema = obj({"role": {"type": "string", "enum": ["gene_lookup", "entrez_references"]},
        "url": text, "http_status": {"type": "integer", "const": 200},
        "response_bytes": {"type": "integer", "minimum": 1, "maximum": MAX_ENSEMBL_RESPONSE_BYTES},
        "response_digest": digest, "native_payload": {"anyOf": [{"type": "object"},
            {"type": "array", "items": {"type": "object"}, "maxItems": MAX_ENSEMBL_XREFS}]}, "payload_digest": digest})
    output = {"type": "object", "properties": {
        "schema": {"type": "string", "const": ENSEMBL_GENE_BUNDLE_SCHEMA},
        "source_id": {"type": "string", "const": "ensembl"},
        "retrieved_at": text, "query": obj({"gene_symbol": {"type": "string", "const": "TP53"},
            "timeout_ms": {"type": "integer", "minimum": 100, "maximum": 30_000}}), "gene": gene_schema,
        "entrez_candidates": {"type": "array", "items": candidate_schema, "maxItems": MAX_ENSEMBL_XREFS},
        "coverage": coverage_schema, "source_inputs": {"type": "array", "items": input_schema, "minItems": 2, "maxItems": 2},
        "receipts": {"type": "array", "items": receipt_schema, "minItems": 2, "maxItems": 2},
        "transport": obj({"mode": {"type": "string", "enum": ["default_pinned_https", "caller_supplied"]},
            "binding_id": digest, "timing_authority": text, "automatic_retry": {"type": "boolean", "const": False}}),
        "limitations": {"type": "array", "items": text},
        "bundle_digest": {"type": "string"}},
        "required": ["schema", "source_id", "retrieved_at", "query", "gene", "entrez_candidates", "coverage", "source_inputs", "receipts", "transport", "limitations", "bundle_digest"],
        "additionalProperties": False}
    operation = WorkflowOperation(ENSEMBL_GENE_OPERATION_ID, description,
                                  {"type": "object", "properties": {"timeout_ms": {"type": "integer", "minimum": 100, "maximum": 30_000, "default": 30_000}},
                                   "additionalProperties": False}, output, "ensembl", read_only=True)

    def execute(arguments: Mapping[str, Any], *, approve_source_dispatch: bool = False,
                retrieved_at: str | None = None) -> WorkflowOperationResult:
        bundle = _retrieve(EnsemblGeneRetrievalConfig(timeout_ms=arguments.get("timeout_ms", 30_000)),
                           approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at,
                           fetcher=fetcher, binding=binding)
        return WorkflowOperationResult(bundle, {"schema": ENSEMBL_GENE_RECEIPT_SCHEMA,
            "source_id": "ensembl", "operation_id": ENSEMBL_GENE_OPERATION_ID,
            "bundle_digest": bundle["bundle_digest"], "source_request_count": 2,
            "response_bytes": bundle["coverage"]["response_byte_count"],
            "candidate_count": bundle["coverage"]["returned_candidate_count"],
            "retrieved_at": bundle["retrieved_at"], "transport_mode": bundle["transport"]["mode"],
            "transport_binding_id": binding, "crosswalk_validation": "source_assertions_only"})

    catalog.register(operation, execute)
    return catalog
