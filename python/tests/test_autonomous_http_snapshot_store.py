from __future__ import annotations

import json

import pytest

from prism_sdk import AUTONOMOUS_DOMAINS, AutonomousHttpSnapshotTextStore
from prism_sdk.authoring import content_digest
from prism_sdk.errors import ArgumentError, TransportError


class _Response:
    def __init__(self, payload: bytes = b"", *, status: int = 200) -> None:
        self.status = status
        self.headers = {"Content-Type": "application/json"}
        self._payload = payload
        self._offset = 0

    def __enter__(self) -> "_Response":
        return self

    def __exit__(self, *_args: object) -> None:
        return None

    def getcode(self) -> int:
        return self.status

    def read(self, size: int = -1) -> bytes:
        if size < 0:
            size = len(self._payload) - self._offset
        chunk = self._payload[self._offset : self._offset + size]
        self._offset += len(chunk)
        return chunk


def _snapshot(domain: str, version: str) -> str:
    body = {"schema": "test-snapshot/0.1", "domain": domain, "version": version, "metadata_only": True}
    return json.dumps({**body, "snapshot_digest": content_digest(body)}, sort_keys=True, separators=(",", ":"))


def _header(request, name: str) -> str | None:
    for key, value in request.headers.items():
        if key.lower() == name.lower():
            return value
    return None


def test_http_snapshot_store_supports_all_domains_transient_headers_and_cas() -> None:
    values: dict[str, str] = {}
    contexts: list[dict[str, object]] = []
    requests: list[tuple[str, str | None, str | None]] = []

    def opener(request, _timeout):
        resource = _header(request, "X-Aurora-Snapshot-Resource")
        assert resource is not None
        requests.append((request.get_method(), _header(request, "If-Match"), _header(request, "If-None-Match")))
        if request.get_method() == "GET":
            return _Response(values[resource].encode("utf-8")) if resource in values else _Response(status=404)
        current = values.get(resource)
        if _header(request, "If-None-Match") == "*" and current is not None:
            return _Response(status=412)
        expected = _header(request, "If-Match")
        if expected is not None:
            expected = expected.strip('"')
            if current is None or json.loads(current)["snapshot_digest"] != expected:
                return _Response(status=412)
        values[resource] = request.data.decode("utf-8")
        return _Response(status=204)

    for domain in AUTONOMOUS_DOMAINS:
        store = AutonomousHttpSnapshotTextStore(
            "https://state.test/snapshots",
            f"{domain}/state",
            allowed_hosts=("state.test",),
            header_resolver=lambda context: (contexts.append(dict(context)) or {"Authorization": "transient-test-credential"}),
            opener=opener,
        )
        assert store.read() is None
        assert store.write_if_unchanged(None, _snapshot(domain, "one"))
        assert not store.write_if_unchanged(None, _snapshot(domain, "two"))
        current = json.loads(store.read())
        assert current["domain"] == domain
        assert store.write_if_unchanged(current["snapshot_digest"], _snapshot(domain, "three"))
        assert store.describe()["credentials"] == "transient_header_resolver;never_returned"

    assert len(contexts) == len(AUTONOMOUS_DOMAINS) * 5
    assert all(context["expected_snapshot_digest"] is None or len(context["expected_snapshot_digest"]) == 64 for context in contexts)
    assert len(requests) == len(AUTONOMOUS_DOMAINS) * 5
    assert all("transient-test-credential" not in json.dumps(values) for _ in [0])


def test_http_snapshot_store_enforces_endpoint_snapshot_and_response_bounds() -> None:
    with pytest.raises(ArgumentError, match="HTTPS"):
        AutonomousHttpSnapshotTextStore("http://state.test/snapshots", "state", allowed_hosts=("state.test",))
    with pytest.raises(ArgumentError, match="allowlist"):
        AutonomousHttpSnapshotTextStore("https://state.test/snapshots", "state", allowed_hosts=("other.test",))
    with pytest.raises(ArgumentError, match="resource"):
        AutonomousHttpSnapshotTextStore("https://state.test/snapshots", "unsafe resource", allowed_hosts=("state.test",))
    oversized = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "state",
        allowed_hosts=("state.test",),
        max_response_bytes=8,
        opener=lambda _request, _timeout: _Response(b"x" * 9),
    )
    with pytest.raises(TransportError, match="exceeded"):
        oversized.read()
    malformed = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "state",
        allowed_hosts=("state.test",),
        opener=lambda _request, _timeout: _Response(b"[]"),
    )
    with pytest.raises(ArgumentError, match="JSON object"):
        malformed.read()


def test_http_snapshot_store_separates_cas_conflicts_from_transport_failures_and_rejects_headers() -> None:
    conflict = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "state",
        allowed_hosts=("state.test",),
        opener=lambda _request, _timeout: _Response(status=409),
    )
    assert not conflict.write_if_unchanged("a" * 64, _snapshot("coding", "one"))
    with pytest.raises(ArgumentError, match="digest"):
        conflict.write_if_unchanged("bad", _snapshot("coding", "one"))
    timeout = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "state",
        allowed_hosts=("state.test",),
        opener=lambda _request, _timeout: (_ for _ in ()).throw(TimeoutError()),
    )
    with pytest.raises(TransportError, match="transport"):
        timeout.read()
    unsafe = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "state",
        allowed_hosts=("state.test",),
        header_resolver=lambda _context: {"X-Test": "line\nbreak"},
        opener=lambda _request, _timeout: _Response(status=404),
    )
    with pytest.raises(ArgumentError, match="header value"):
        unsafe.read()
