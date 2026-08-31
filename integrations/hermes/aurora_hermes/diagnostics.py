"""Non-invasive readiness diagnostics for validated Hermes entries."""

from __future__ import annotations

import os
import shutil
from dataclasses import dataclass

from .config import ServerSpec
from .errors import ProbeError


@dataclass(frozen=True)
class Readiness:
    name: str
    kind: str
    status: str  # ready | unmeasured | unsupported | missing
    detail: str


def diagnose(spec: ServerSpec, *, probe: bool = False) -> Readiness:
    if not probe:
        return Readiness(spec.name, spec.kind, "unmeasured", "validated but no process or network probe was requested")
    if spec.kind == "stdio":
        if spec.command is None:
            raise ProbeError("stdio spec has no command")
        executable = spec.command if os.path.isabs(spec.command) else shutil.which(spec.command)
        if executable:
            return Readiness(spec.name, spec.kind, "ready", f"executable found at {executable}")
        return Readiness(spec.name, spec.kind, "missing", "command was not found; no process was started")
    return Readiness(spec.name, spec.kind, "unsupported", "HTTP live probing is not implemented; URL validation is not connectivity")


def diagnose_all(specs: tuple[ServerSpec, ...], *, probe: bool = False) -> tuple[Readiness, ...]:
    return tuple(diagnose(spec, probe=probe) for spec in specs)
