from __future__ import annotations

import asyncio
import threading

import pytest

from prism_sdk._async_callback_bridge import run_in_thread_and_drain


def test_repeated_cancellation_drains_the_worker_before_propagating() -> None:
    entered = threading.Event()
    release = threading.Event()
    settled = threading.Event()

    def blocking_worker() -> None:
        entered.set()
        try:
            assert release.wait(timeout=5)
            raise RuntimeError("worker failure after cancellation")
        finally:
            settled.set()

    async def exercise() -> None:
        task = asyncio.create_task(run_in_thread_and_drain(blocking_worker))
        try:
            assert await asyncio.to_thread(entered.wait, 2)
            task.cancel()
            await asyncio.sleep(0.02)
            assert not task.done(), "the worker must still be draining after first cancellation"

            task.cancel()
            await asyncio.sleep(0.02)
            assert not task.done(), "a second cancellation must not abandon the worker"
        finally:
            release.set()

        with pytest.raises(asyncio.CancelledError):
            await task

    asyncio.run(exercise())
    assert settled.is_set(), "the blocking worker must settle before cancellation returns"
