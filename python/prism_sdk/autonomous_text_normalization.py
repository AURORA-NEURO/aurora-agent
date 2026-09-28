"""Shared ASCII normalization for deterministic autonomous routing and tool ranking."""

from __future__ import annotations

import re


_NON_TOKEN_CHARACTERS = re.compile(r"[^a-z0-9]+")


def normalize_route_text(value: str) -> str:
    """Normalize transient text for deterministic catalogue matching."""

    normalized = _NON_TOKEN_CHARACTERS.sub(" ", value.lower())
    return " ".join(normalized.split())


def term_matches(normalized_task: str, term: str) -> bool:
    """Check a bounded catalogue term against already normalized task text."""

    normalized_term = normalize_route_text(term)
    if not normalized_term:
        return False
    return f" {normalized_term} " in f" {normalized_task} "


__all__ = ["normalize_route_text", "term_matches"]
