"""Compact descriptor registry for large platform inventories."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum

from .errors import RegistryError, UnsupportedAdapterError


class AdapterState(str, Enum):
    SUPPORTED = "supported"
    PARTIAL = "partial"
    DESCRIPTOR_ONLY = "descriptor-only"
    REFUSED = "refused"


@dataclass(frozen=True)
class AdapterDescriptor:
    platform: str
    protocol: str
    state: AdapterState
    capabilities: tuple[str, ...] = ()
    notes: str = ""


class AdapterRegistry:
    def __init__(self, descriptors: tuple[AdapterDescriptor, ...] = ()) -> None:
        self._items: dict[tuple[str, str], AdapterDescriptor] = {}
        for descriptor in descriptors:
            self.register(descriptor)

    def register(self, descriptor: AdapterDescriptor) -> None:
        if not descriptor.platform or not descriptor.protocol:
            raise RegistryError("platform and protocol are required")
        if tuple(sorted(set(descriptor.capabilities))) != descriptor.capabilities:
            raise RegistryError(f"capabilities for {descriptor.platform}/{descriptor.protocol} must be sorted and unique")
        key = (descriptor.platform, descriptor.protocol)
        if key in self._items:
            raise RegistryError(f"duplicate adapter descriptor: {key}")
        self._items[key] = descriptor

    def get(self, platform: str, protocol: str) -> AdapterDescriptor:
        try:
            return self._items[(platform, protocol)]
        except KeyError as error:
            raise RegistryError(f"no descriptor for {platform}/{protocol}") from error

    def __len__(self) -> int:
        return len(self._items)

    def snapshot(self) -> tuple[AdapterDescriptor, ...]:
        return tuple(self._items[key] for key in sorted(self._items))

    def require_live(self, platform: str, protocol: str) -> AdapterDescriptor:
        descriptor = self.get(platform, protocol)
        if descriptor.state in {AdapterState.DESCRIPTOR_ONLY, AdapterState.REFUSED}:
            raise UnsupportedAdapterError(f"{platform}/{protocol} is {descriptor.state.value}: {descriptor.notes}")
        return descriptor


def default_registry(*, generated_platforms: int = 1024) -> AdapterRegistry:
    if generated_platforms < 0:
        raise RegistryError("generated_platforms cannot be negative")
    base = [
        AdapterDescriptor("aurora", "mcp-stdio", AdapterState.SUPPORTED, ("resources", "tools"), "local stdio contract"),
        AdapterDescriptor("aurora", "mcp-http", AdapterState.PARTIAL, ("tools",), "HTTP/1.1 Content-Length adapter; connectivity is external"),
        AdapterDescriptor("generic", "rest", AdapterState.DESCRIPTOR_ONLY, ("request",), "descriptor only; no live connector"),
        AdapterDescriptor("generic", "graphql", AdapterState.DESCRIPTOR_ONLY, ("query",), "descriptor only; no live connector"),
        AdapterDescriptor("generic", "webhook", AdapterState.DESCRIPTOR_ONLY, ("event",), "descriptor only; no live connector"),
        AdapterDescriptor("generic", "cli", AdapterState.DESCRIPTOR_ONLY, ("argv",), "argv shape only; no process launch"),
        AdapterDescriptor("generic", "archive", AdapterState.DESCRIPTOR_ONLY, ("import",), "archive shape only; no remote fetch"),
        AdapterDescriptor("generic", "a2a", AdapterState.REFUSED, (), "wire-shape compatibility is not a live A2A adapter"),
        AdapterDescriptor("generic", "acp", AdapterState.REFUSED, (), "ACP is intentionally not implemented"),
    ]
    for index in range(generated_platforms):
        base.append(AdapterDescriptor(f"platform-{index:04d}", "rest", AdapterState.DESCRIPTOR_ONLY, ("request",), "generated compact descriptor; integration not claimed"))
    return AdapterRegistry(tuple(base))
