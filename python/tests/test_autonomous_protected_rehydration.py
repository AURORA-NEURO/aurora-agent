from __future__ import annotations

import hashlib
import json
import unittest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousProtectedRehydrationBoundary,
    AutonomousProtectedRehydrationContext,
    AutonomousProtectedRehydrationError,
    AutonomousProtectedRehydrationPersistenceCoordinator,
    AutonomousMemoryConsolidationScheduler,
    AutonomousMemoryConsolidationSchedulerError,
    AutonomousMemoryConsolidator,
    JsonAutonomousProtectedRehydrationPersistence,
    TransactionalJsonAutonomousProtectedRehydrationPersistence,
    protected_value_digest,
    validate_autonomous_protected_rehydration_snapshot,
)


def _digest(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


class _CasStore:
    def __init__(self) -> None:
        self.value: str | None = None

    def read(self) -> str | None:
        return self.value

    def write(self, value: str) -> None:
        self.value = value

    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool:
        observed = None if self.value is None else json.loads(self.value)["snapshot_digest"]
        if observed != expected_snapshot_digest:
            return False
        self.value = value
        return True


class AutonomousProtectedRehydrationTests(unittest.TestCase):
    def _context(self, tenant: str = "tenant-a") -> AutonomousProtectedRehydrationContext:
        return AutonomousProtectedRehydrationContext(tenant, "actor-a", "session-a", _digest("authorization-a"))

    def test_all_domains_are_scoped_and_secret_values_are_transient(self) -> None:
        values: dict[str, str] = {}
        boundary = AutonomousProtectedRehydrationBoundary(
            self._context(),
            lambda reference, _context: values[reference.reference_id],
            authorizer=lambda _reference, _context: True,
            clock=lambda: 100.0,
        )
        for index, domain in enumerate(AUTONOMOUS_DOMAINS):
            reference_id = f"reference-{index}"
            value = f"user-provider-secret-{index}"
            values[reference_id] = value
            boundary.issue_for_value(
                reference_id,
                value,
                domain=domain,
                purpose="provider_credential",
                value_kind="credential",
                issued_at=100.0,
                expires_at=200.0,
            )

        snapshot = boundary.snapshot()
        encoded = json.dumps(snapshot)
        self.assertEqual([row["domain"] for row in snapshot["coverage"]], list(AUTONOMOUS_DOMAINS))
        self.assertEqual([row["reference_count"] for row in snapshot["coverage"]], [1] * len(AUTONOMOUS_DOMAINS))
        self.assertNotIn("user-provider-secret", encoded)
        self.assertNotIn('"value":', encoded.lower())
        result = boundary.resolve("reference-0", now=101.0)
        self.assertEqual(result.value, values["reference-0"])
        self.assertFalse(result.to_dict()["value_retained"])
        self.assertEqual(boundary.get("reference-0")["status"], "consumed")
        with self.assertRaises(AutonomousProtectedRehydrationError):
            boundary.resolve("reference-0", now=101.0)

    def test_context_authorization_expiry_and_digest_mismatch_are_fenced(self) -> None:
        boundary = AutonomousProtectedRehydrationBoundary(
            self._context(),
            lambda _reference, _context: "wrong-value",
            authorizer=lambda _reference, _context: False,
            clock=lambda: 100.0,
            max_attempts=2,
        )
        boundary.issue("auth", domain="enterprise", purpose="delegated_session", value_digest=protected_value_digest("expected"), issued_at=100.0, expires_at=102.0)
        with self.assertRaises(AutonomousProtectedRehydrationError):
            boundary.resolve("auth", now=100.5)
        self.assertEqual(boundary.get("auth")["last_error_class"], "authorization_denied")

        expired = AutonomousProtectedRehydrationBoundary(self._context(), lambda _reference, _context: "expected", clock=lambda: 100.0)
        expired.issue("expired", domain="operations", purpose="session", value_digest=protected_value_digest("expected"), issued_at=100.0, expires_at=101.0)
        with self.assertRaises(AutonomousProtectedRehydrationError):
            expired.resolve("expired", now=101.0)
        self.assertEqual(expired.get("expired")["status"], "expired")

        mismatch = AutonomousProtectedRehydrationBoundary(self._context(), lambda _reference, _context: "wrong", clock=lambda: 100.0)
        mismatch.issue("mismatch", domain="coding", purpose="credential", value_digest=protected_value_digest("expected"), issued_at=100.0, expires_at=200.0)
        with self.assertRaises(AutonomousProtectedRehydrationError):
            mismatch.resolve("mismatch", now=100.1)
        self.assertEqual(mismatch.get("mismatch")["last_error_class"], "value_digest_mismatch")

    def test_snapshot_restore_is_tenant_bound_and_cas_safe(self) -> None:
        source = AutonomousProtectedRehydrationBoundary(self._context(), lambda _reference, _context: "token", clock=lambda: 100.0, max_ttl_seconds=60.0)
        source.issue_for_value("persist", "token", domain="science", purpose="provider_credential", issued_at=100.0, expires_at=150.0)
        store = _CasStore()
        coordinator = AutonomousProtectedRehydrationPersistenceCoordinator(source, TransactionalJsonAutonomousProtectedRehydrationPersistence(store))
        snapshot = coordinator.flush()
        self.assertEqual(validate_autonomous_protected_rehydration_snapshot(snapshot)["snapshot_digest"], snapshot["snapshot_digest"])

        restored = AutonomousProtectedRehydrationBoundary(self._context(), lambda _reference, _context: "token", clock=lambda: 100.0, max_ttl_seconds=60.0)
        restored_coordinator = AutonomousProtectedRehydrationPersistenceCoordinator(restored, JsonAutonomousProtectedRehydrationPersistence(store))
        self.assertEqual(restored_coordinator.restore()["snapshot_digest"], snapshot["snapshot_digest"])
        self.assertEqual(restored.get("persist")["value_digest"], protected_value_digest("token"))

        other_tenant = AutonomousProtectedRehydrationBoundary(self._context("tenant-b"), lambda _reference, _context: "token", clock=lambda: 100.0, max_ttl_seconds=60.0)
        with self.assertRaises(AutonomousProtectedRehydrationError):
            other_tenant.restore(snapshot)
        source.issue_for_value("second", "token-2", domain="science", purpose="provider_credential", issued_at=100.0, expires_at=150.0)
        competing = AutonomousProtectedRehydrationBoundary(self._context(), lambda _reference, _context: "token", clock=lambda: 100.0, max_ttl_seconds=60.0)
        competing_coordinator = AutonomousProtectedRehydrationPersistenceCoordinator(competing, JsonAutonomousProtectedRehydrationPersistence(store))
        competing_coordinator.restore()
        competing.issue_for_value("competing", "token-3", domain="science", purpose="provider_credential", issued_at=100.0, expires_at=150.0)
        competing_coordinator.flush()
        with self.assertRaises(AutonomousProtectedRehydrationError):
            coordinator.flush()

    def test_memory_scheduler_snapshot_is_bound_to_the_same_execution_context(self) -> None:
        source = AutonomousMemoryConsolidationScheduler(
            AutonomousMemoryConsolidator(min_observations=1, min_support_lower_bound=0.0),
            execution_context=self._context(),
        )
        snapshot = source.snapshot()
        self.assertEqual(snapshot["policy"]["rehydration_context_digest"], self._context().context_digest)
        restored = AutonomousMemoryConsolidationScheduler(
            AutonomousMemoryConsolidator(min_observations=1, min_support_lower_bound=0.0),
            execution_context=self._context("tenant-b"),
        )
        with self.assertRaises(AutonomousMemoryConsolidationSchedulerError):
            restored.restore(snapshot)


if __name__ == "__main__":
    unittest.main()
