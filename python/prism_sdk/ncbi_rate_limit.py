"""Shared runtime-local pacing for reviewed NCBI E-utilities adapters."""

from __future__ import annotations

import threading
import time


_MIN_REQUEST_INTERVAL_SECONDS = 0.34
_RATE_LOCK = threading.Lock()
_LAST_DISPATCH: float | None = None


def acquire_ncbi_request_slot() -> None:
    """Wait for a request slot shared by reviewed NCBI adapters in this runtime."""

    global _LAST_DISPATCH
    with _RATE_LOCK:
        now = time.monotonic()
        if _LAST_DISPATCH is not None:
            remaining = _MIN_REQUEST_INTERVAL_SECONDS - (now - _LAST_DISPATCH)
            if remaining > 0:
                time.sleep(remaining)
        _LAST_DISPATCH = time.monotonic()
