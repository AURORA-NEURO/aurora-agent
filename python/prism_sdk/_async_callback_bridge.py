"""Internal adapters for composing synchronous control kernels with async callers."""

from __future__ import annotations

import asyncio
import inspect
from collections.abc import Callable
from typing import Any


def is_async_callable(value: Any) -> bool:
    """Recognize coroutine functions and callable objects with async ``__call__`` methods."""

    return inspect.iscoroutinefunction(value) or inspect.iscoroutinefunction(
        getattr(value, "__call__", None)
    )


def bridge_callback_to_loop(
    callback: Callable[..., Any],
    loop: asyncio.AbstractEventLoop,
) -> Callable[..., Any]:
    """Return a blocking worker-thread wrapper that settles awaitables on ``loop``."""

    def invoke(*args: Any, **kwargs: Any) -> Any:
        result = callback(*args, **kwargs)
        if not inspect.isawaitable(result):
            return result

        async def resolve() -> Any:
            return await result

        coroutine = resolve()
        try:
            return asyncio.run_coroutine_threadsafe(coroutine, loop).result()
        except RuntimeError:
            coroutine.close()
            if inspect.iscoroutine(result):
                result.close()
            raise

    return invoke


async def run_in_thread_and_drain(
    function: Callable[..., Any],
    *args: Any,
    **kwargs: Any,
) -> Any:
    """Run blocking control work off-loop and let it settle before propagating cancellation."""

    running = asyncio.create_task(asyncio.to_thread(function, *args, **kwargs))
    try:
        return await asyncio.shield(running)
    except asyncio.CancelledError as cancellation:
        while not running.done():
            try:
                await asyncio.shield(running)
            except asyncio.CancelledError:
                # A second caller cancellation must not detach an unfenced worker.
                continue
            except BaseException:
                # Preserve caller cancellation even when the completed worker failed.
                break
        try:
            running.result()
        except BaseException:
            pass
        raise cancellation
