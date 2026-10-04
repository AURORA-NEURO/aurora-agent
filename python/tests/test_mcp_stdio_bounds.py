from __future__ import annotations

import asyncio
from io import BytesIO
from pathlib import Path
import sys

import pytest

from prism_sdk.async_client import AsyncClient
from prism_sdk.client import _LineReader
from prism_sdk.errors import ProtocolError


def test_sync_reader_limits_frame_memory_before_a_line_terminator() -> None:
    stream = BytesIO(b"x" * 1_000_000)
    reader = _LineReader(stream, max_frame_bytes=1_024)
    reader.start()

    result = reader.next(timeout=1)
    reader._thread.join(timeout=1)

    assert isinstance(result, ProtocolError)
    assert stream.tell() == 1_025
    assert not reader._thread.is_alive()


def test_sync_reader_keeps_only_a_bounded_stderr_tail() -> None:
    reader = _LineReader(BytesIO(b"d" * 200_000), max_frame_bytes=1_024, stderr=True)
    reader.start()
    reader._thread.join(timeout=1)

    assert not reader._thread.is_alive()
    assert len(reader.stderr().encode("utf-8")) == 64_000
    assert reader.stderr() == "d" * 64_000


def test_async_client_accepts_configured_frames_above_the_default_stream_limit(tmp_path: Path) -> None:
    peer = tmp_path / "large_frame_peer.py"
    peer.write_text(
        "import json, sys\n"
        "for raw in sys.stdin:\n"
        " request = json.loads(raw)\n"
        " if request.get('method') == 'initialize':\n"
        "  sys.stderr.write('d' * 200000); sys.stderr.flush()\n"
        "  response = {'jsonrpc':'2.0','id':request['id'],'result':{'protocolVersion':'2025-06-18','capabilities':{},'serverInfo':{'name':'n' * 70000}}}\n"
        "  sys.stdout.write(json.dumps(response, separators=(',', ':')) + '\\n'); sys.stdout.flush()\n"
        " elif request.get('method') == 'notifications/initialized':\n"
        "  break\n",
        encoding="utf-8",
    )

    async def exercise() -> tuple[str, str]:
        client = AsyncClient([sys.executable, "-u", str(peer)], timeout=2, max_frame_bytes=100_000)
        await client.connect()
        name = client.session.server_info["name"]
        await client.close()
        return name, client.stderr()

    name, stderr = asyncio.run(exercise())

    assert len(name) == 70_000
    assert stderr == "d" * 64_000


def test_async_client_rejects_an_unterminated_oversized_frame(tmp_path: Path) -> None:
    peer = tmp_path / "oversized_frame_peer.py"
    peer.write_text(
        "import json, sys\n"
        "json.loads(sys.stdin.readline())\n"
        "sys.stdout.write('x' * 1000000); sys.stdout.flush()\n"
        "sys.stdin.readline()\n",
        encoding="utf-8",
    )

    async def exercise() -> None:
        client = AsyncClient([sys.executable, "-u", str(peer)], timeout=2, max_frame_bytes=1_024)
        with pytest.raises(ProtocolError, match="frame.*bound"):
            await client.connect()
        await client.close()

    asyncio.run(exercise())
