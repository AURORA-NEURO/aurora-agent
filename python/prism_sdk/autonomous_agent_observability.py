"""Focused autonomous-agent observability methods."""

from __future__ import annotations

from .autonomy import (
    AUTONOMOUS_DOMAIN_NAMES,
    Any,
    AutonomousAuthorizationGate,
    AutonomousAuthorizationLedger,
    AutonomousRunAnalyticsController,
    AutonomousRunAnalyticsLedger,
    AutonomousRunAnalyticsLedgerPolicy,
    AutonomousRunObservabilityController,
    AutonomousRunTraceAnalyticsPolicy,
    AutonomousRunTraceAnalyticsReport,
    AutonomousRunTraceRegistryController,
    AutonomousRunTraceSession,
    AutonomousRunTraceStore,
    AutonomousTracedRunResult,
    BrainRunError,
    Callable,
    CredentialHandle,
    CredentialSession,
    Mapping,
    ModelCandidate,
    Sequence,
    _authorize_launch_admission_domains,
    _cross_domain_subtask_domains_for_launch_admission,
    analyze_autonomous_run_trace,
    autonomous_run_trace_status,
    content_digest,
    uuid,
)

class AutonomousAgentObservabilityMixin:
    @staticmethod
    def _trace_store(store: Any) -> AutonomousRunTraceStore:
        if not all(callable(getattr(store, name, None)) for name in ("append", "events", "snapshot", "restore")):
            raise BrainRunError("autonomous run trace store must implement append, events, snapshot, and restore")
        return store

    def run_with_trace(
        self,
        *,
        task: str,
        domain: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        trace_store: AutonomousRunTraceStore,
        run_id: str | None = None,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        **kwargs: Any,
    ) -> AutonomousTracedRunResult:
        """Run one domain and return the live result with a durable metadata-only trace.

        ``run_id`` is also used as the execution-controller id so provider accounting and the
        trace share one stable identity.  A caller may persist the trace independently through
        :class:`AutonomousRunTracePersistenceCoordinator`; no transcript is written by this
        helper.
        """

        self._trace_store(trace_store)
        resolved_run_id = run_id or kwargs.pop("execution_id", None) or f"trace-{uuid.uuid4().hex}"
        task_digest = content_digest({"task": task})
        session = AutonomousRunTraceSession(
            trace_store,
            run_id=resolved_run_id,
            task_digest=task_digest,
            domains=(domain,),
            authorization_context=kwargs.get("authorization_context"),
        )
        session.started()
        live_observer = session.provider_observer()
        try:
            result = self.run(
                task=task,
                domain=domain,
                credentials=credentials,
                model_candidates=model_candidates,
                execution_id=resolved_run_id,
                invocation_observer=live_observer,
                trace_event_callback=session.record,
                **kwargs,
            )
            metadata = self._trace_execution_metadata(result)
            if not trace_store.events({"run_id": resolved_run_id, "phase": "plan_compiled"}):
                session.record(
                    phase="plan_compiled",
                    status="running",
                    route_digest=metadata["route_digest"],
                    plan_digest=metadata["plan_digest"],
                    selection_digest=metadata["selection_digest"],
                )
            if not trace_store.events({"run_id": resolved_run_id, "phase": "provider_invocation_finished"}):
                session.record_provider_receipts(metadata["receipts"])
            session.complete(
                status=autonomous_run_trace_status(getattr(result, "status", "unknown")),
                route_digest=metadata["route_digest"],
                plan_digest=metadata["plan_digest"],
                selection_digest=metadata["selection_digest"],
                detail_digest=metadata["detail_digest"],
            )
        except Exception as error:
            session.fail(failure_class=type(error).__name__, failure_code="execution_error")
            raise
        return AutonomousTracedRunResult(result=result, trace=session.summary())

    def run_with_trace_and_launch_admission(
        self,
        *,
        launch_admission: Mapping[str, Any],
        **kwargs: Any,
    ) -> AutonomousTracedRunResult:
        """Run a traced task only when its domain was approved at the launch boundary."""

        domain = kwargs.get("domain")
        _authorize_launch_admission_domains(launch_admission, (domain,))
        return self.run_with_trace(**kwargs)

    @staticmethod
    def analyze_run_trace(
        snapshot: Mapping[str, Any] | Any,
        policy: AutonomousRunTraceAnalyticsPolicy | Mapping[str, Any] | None = None,
    ) -> AutonomousRunTraceAnalyticsReport:
        """Aggregate a verified metadata-only trace into conservative health observations.

        This is deliberately separate from execution: it never invokes a provider, reads a
        credential, or treats absent latency/token observations as zero. Callers can persist
        the returned digest-bound report or re-run the analysis with different thresholds.
        """

        return analyze_autonomous_run_trace(snapshot, policy)

    @staticmethod
    def create_run_analytics_ledger(
        policy: AutonomousRunAnalyticsLedgerPolicy | Mapping[str, Any] | None = None,
        *,
        clock: Callable[[], float] | None = None,
    ) -> AutonomousRunAnalyticsLedger:
        """Create a bounded longitudinal ledger for already-verified trace reports.

        The ledger is deliberately analytics-only: it never invokes a provider, reads a
        credential, or retains prompts, responses, tool payloads, or cost claims.  Callers own
        persistence and may pass a mapping when restoring configuration from JSON.
        """

        if isinstance(policy, AutonomousRunAnalyticsLedgerPolicy) or policy is None:
            normalized_policy = policy
        else:
            policy_mapping = dict(policy)
            policy_mapping.setdefault("expected_domains", list(AUTONOMOUS_DOMAIN_NAMES))
            policy_mapping.setdefault("max_reports", AutonomousRunAnalyticsLedgerPolicy().max_reports)
            normalized_policy = AutonomousRunAnalyticsLedgerPolicy.from_dict(policy_mapping)
        return AutonomousRunAnalyticsLedger(normalized_policy) if clock is None else AutonomousRunAnalyticsLedger(normalized_policy, clock=clock)

    def create_run_analytics_controller(
        self,
        ledger: AutonomousRunAnalyticsLedger,
        persistence: Any,
    ) -> AutonomousRunAnalyticsController:
        """Bind a metadata-only analytics ledger to this agent's application lifecycle.

        The controller is restore-before-read, re-entrancy fenced, and persistence-aware.  It
        never gains provider authority and accepts only a caller-owned ledger and JSON/CAS
        persistence adapter.
        """

        return AutonomousRunAnalyticsController(self, ledger, persistence)

    @staticmethod
    def create_authorization_ledger(
        *,
        max_grants: int = 4_096,
        max_events: int = 32_768,
    ) -> AutonomousAuthorizationLedger:
        """Create the caller-owned tenant/actor/session authorization boundary.

        The ledger is intentionally separate from task execution: applications issue grants
        from their identity system, call ``authorize`` immediately before a provider/source/tool
        or effect boundary, and persist the metadata-only snapshot through the authorization
        persistence coordinator. No task, prompt, credential, or result is accepted here.
        """

        return AutonomousAuthorizationLedger(max_grants=max_grants, max_events=max_events)

    @staticmethod
    def create_authorization_gate(
        ledger: AutonomousAuthorizationLedger,
    ) -> AutonomousAuthorizationGate:
        """Create the fail-closed gate used immediately before caller-owned dispatch."""

        return AutonomousAuthorizationGate(ledger)

    def create_trace_registry_controller(
        self,
        registry: Any,
        persistence: Any,
    ) -> AutonomousRunTraceRegistryController:
        """Bind the metadata-only trace registry to this agent's application lifecycle.

        The controller is restore-before-read and persistence-aware. It does not authorize
        provider calls, replay a run, or retain source values.
        """

        return AutonomousRunTraceRegistryController(self, registry, persistence)

    def create_run_observability_controller(
        self,
        trace_registry: AutonomousRunTraceRegistryController,
        run_analytics: AutonomousRunAnalyticsController,
        alert_sink: Any | None = None,
    ) -> AutonomousRunObservabilityController:
        """Coordinate one source snapshot across trace indexing and longitudinal analytics."""

        return AutonomousRunObservabilityController(self, trace_registry, run_analytics, alert_sink)

    def run_cross_domain_with_trace(
        self,
        *,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        trace_store: AutonomousRunTraceStore,
        run_id: str | None = None,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        **kwargs: Any,
    ) -> AutonomousTracedRunResult:
        """Run a bounded fan-out/fan-in task with one trace spanning every specialist domain."""

        self._trace_store(trace_store)
        requested_domains = [
            value.get("domain")
            for value in subtasks
            if isinstance(value, Mapping) and isinstance(value.get("domain"), str)
        ]
        trace_domains = tuple(dict.fromkeys(["cross_domain", *requested_domains]))
        resolved_run_id = run_id or kwargs.pop("execution_id", None) or f"trace-{uuid.uuid4().hex}"
        session = AutonomousRunTraceSession(
            trace_store,
            run_id=resolved_run_id,
            task_digest=content_digest({"task": task}),
            domains=trace_domains,
            authorization_context=kwargs.get("authorization_context"),
        )
        session.started()
        live_observer = session.provider_observer()
        try:
            result = self.run_cross_domain(
                task=task,
                subtasks=subtasks,
                credentials=credentials,
                model_candidates=model_candidates,
                execution_id=resolved_run_id,
                invocation_observer=live_observer,
                trace_event_callback=session.record,
                **kwargs,
            )
            metadata = self._trace_execution_metadata(result)
            if not trace_store.events({"run_id": resolved_run_id, "phase": "plan_compiled"}):
                session.record(
                    phase="plan_compiled",
                    status="running",
                    domains=trace_domains,
                    route_digest=metadata["route_digest"],
                    plan_digest=metadata["plan_digest"],
                    selection_digest=metadata["selection_digest"],
                )
            if not trace_store.events({"run_id": resolved_run_id, "phase": "provider_invocation_finished"}):
                session.record_provider_receipts(metadata["receipts"])
            session.complete(
                status=autonomous_run_trace_status(result.status),
                domains=trace_domains,
                route_digest=metadata["route_digest"],
                plan_digest=metadata["plan_digest"],
                selection_digest=metadata["selection_digest"],
                detail_digest=metadata["detail_digest"],
            )
        except Exception as error:
            session.fail(failure_class=type(error).__name__, failure_code="execution_error")
            raise
        return AutonomousTracedRunResult(result=result, trace=session.summary())

    def run_cross_domain_with_trace_and_launch_admission(
        self,
        *,
        launch_admission: Mapping[str, Any],
        **kwargs: Any,
    ) -> AutonomousTracedRunResult:
        """Run one traced cross-domain fan-out only after every specialist is admitted."""

        requested_domains = _cross_domain_subtask_domains_for_launch_admission(kwargs.get("subtasks", ()))
        _authorize_launch_admission_domains(launch_admission, requested_domains)
        return self.run_cross_domain_with_trace(**kwargs)

# Keep method introspection compatible with the public AutonomousAgent class.
for _name, _descriptor in vars(AutonomousAgentObservabilityMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, (staticmethod, classmethod)) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousAgent.{_name}"
