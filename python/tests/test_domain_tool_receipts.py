from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousDomainToolReceipt,
    AutonomousDomainToolReceiptJournal,
    AutonomousDomainToolReceiptJournalEntry,
    builtin_autonomous_domain_tool_profiles,
    content_digest,
)
from prism_sdk.errors import ArgumentError


def _receipt(domain: str, *, status: str = "executed", call_id: str | None = None) -> AutonomousDomainToolReceipt:
    return AutonomousDomainToolReceipt(
        call_id=call_id or f"call-{domain}",
        tool=f"{domain}_observe",
        status=status,
        schema_digest=content_digest({"type": "object"}),
        arguments_digest=content_digest({"domain": domain}),
        output_digest=content_digest({"status": status}),
        execution_id=f"execution-{domain}",
        domain=domain,
        capability="observation",
        risk_class="read_only",
    )


def test_receipt_journal_is_hash_chained_restart_safe_and_idempotent(tmp_path) -> None:
    path = tmp_path / "domain-receipts.jsonl"
    journal = AutonomousDomainToolReceiptJournal(path)
    receipt = _receipt("operations")

    first = journal.append(receipt)
    duplicate = journal.append(receipt)
    assert isinstance(first, AutonomousDomainToolReceiptJournalEntry)
    assert duplicate == first
    assert journal.find(execution_id="execution-operations", call_id="call-operations", tool="operations_observe") == receipt
    assert journal.verify_integrity()["head_digest"] == first.entry_digest
    assert "operations" in str(journal.receipts())
    encoded = json.dumps(first.to_dict())
    assert '"arguments":' not in encoded
    assert '"outputs":' not in encoded

    reopened = AutonomousDomainToolReceiptJournal(path)
    assert reopened.receipts() == (first,)
    assert reopened.verify_integrity()["verified"] is True


def test_receipt_journal_rejects_identity_conflicts_tampering_and_private_fields(tmp_path) -> None:
    path = tmp_path / "domain-receipts.jsonl"
    journal = AutonomousDomainToolReceiptJournal(path)
    journal.append(_receipt("coding"))

    conflicting = _receipt("coding", status="schema_refused")
    with pytest.raises(ArgumentError, match="identity conflict"):
        journal.append(conflicting)

    raw = path.read_text(encoding="utf-8")
    path.write_text(raw.replace('"entry_digest":"', '"entry_digest":"0', 1), encoding="utf-8")
    with pytest.raises(ArgumentError, match="digest"):
        AutonomousDomainToolReceiptJournal(path)

    with pytest.raises(ArgumentError):
        journal.append({**_receipt("data").to_dict(), "arguments": {"secret": "private"}})


def test_receipt_journal_covers_every_builtin_domain_and_respects_capacity(tmp_path) -> None:
    profiles = builtin_autonomous_domain_tool_profiles()
    assert {profile.domain for profile in profiles} == set(AUTONOMOUS_DOMAINS)
    journal = AutonomousDomainToolReceiptJournal(tmp_path / "all-domains.jsonl", max_entries=len(profiles))

    for profile in profiles:
        journal.append(_receipt(profile.domain))

    assert len(journal.receipts(limit=len(profiles))) == len(AUTONOMOUS_DOMAINS)
    assert {entry.receipt.domain for entry in journal.receipts(limit=len(profiles))} == set(AUTONOMOUS_DOMAINS)
    with pytest.raises(ArgumentError, match="capacity"):
        journal.append(_receipt("cross_domain", call_id="capacity-overflow"))
