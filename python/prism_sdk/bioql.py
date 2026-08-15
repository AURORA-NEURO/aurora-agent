"""Typed, bounded request model for the fail-closed BioQL compiler bridge."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .authoring import canonical_json
from .errors import ArgumentError


BIOQL_SCHEMA = "bioprism-python-bioql/0.1"
MAX_BIOQL_QUERY_BYTES = 1_000_000
MAX_BIOQL_SCHEMA_BYTES = 10_000_000


@dataclass(frozen=True)
class BioQlCompileRequest:
    """A caller-supplied BioQL source and explicit biological query schema."""

    query: str
    schema: Mapping[str, Any]

    def __post_init__(self) -> None:
        if not isinstance(self.query, str) or not self.query.strip():
            raise ArgumentError("query must be a non-empty string")
        query_bytes = len(self.query.encode("utf-8"))
        if query_bytes > MAX_BIOQL_QUERY_BYTES:
            raise ArgumentError(f"query exceeds the {MAX_BIOQL_QUERY_BYTES}-byte limit")
        if not isinstance(self.schema, Mapping):
            raise ArgumentError("schema must be a mapping")
        try:
            schema_json = canonical_json(dict(self.schema))
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"schema is not canonical JSON-safe: {error}") from error
        if len(schema_json.encode("utf-8")) > MAX_BIOQL_SCHEMA_BYTES:
            raise ArgumentError(f"schema exceeds the {MAX_BIOQL_SCHEMA_BYTES}-byte limit")
        object.__setattr__(self, "schema", dict(self.schema))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"query": self.query, "schema": dict(self.schema)}


__all__ = [
    "BIOQL_SCHEMA",
    "MAX_BIOQL_QUERY_BYTES",
    "MAX_BIOQL_SCHEMA_BYTES",
    "BioQlCompileRequest",
]
