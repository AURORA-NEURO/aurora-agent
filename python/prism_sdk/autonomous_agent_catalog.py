"""Focused autonomous-agent catalog methods."""

from __future__ import annotations

from .autonomy import (
    AUTONOMOUS_DOMAINS,
    Any,
    ArgumentError,
    AutonomousActivationError,
    AutonomousAgentPersistenceLifecycleCoordinator,
    AutonomousCapabilityActivationStore,
    AutonomousCapabilityJournalPersistenceCoordinator,
    AutonomousDecisionCyclePersistenceCoordinator,
    AutonomousExecutionPersistenceCoordinator,
    AutonomousModelInventoryError,
    AutonomousModelInventoryPersistenceCoordinator,
    AutonomousModelInventoryReadiness,
    AutonomousModelInventorySnapshot,
    AutonomousModelInventoryStore,
    AutonomousProvisionedRun,
    AutonomousSelectionPromotionLifecycleStore,
    BrainJobRunResult,
    BrainOutcomeEvaluator,
    BrainRunError,
    Callable,
    CredentialHandle,
    CredentialProvisioningResult,
    CredentialSession,
    CredentialSourceSpec,
    MAX_PROVIDER_DISCOVERED_MODELS,
    Mapping,
    ModelCandidate,
    ProviderConfig,
    ProviderModelDescriptor,
    Sequence,
    _authorize_launch_admission_domains,
)

class AutonomousAgentCatalogMixin:
    def register_model(
        self,
        candidate: ModelCandidate | Mapping[str, Any],
        *,
        replace_existing: bool = False,
    ) -> ModelCandidate:
        """Add one non-secret model route to the application-owned inventory."""

        return self.catalogue.register(candidate, replace_existing=replace_existing)

    def discover_provider_models(
        self,
        provider: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        *,
        path: str | None = None,
        limit: int = MAX_PROVIDER_DISCOVERED_MODELS,
    ) -> list[dict[str, Any]]:
        """Discover provider inventory through the active opaque credential session.

        Discovery is intentionally separate from registration. The returned rows are bounded,
        safe projections and remain caller-owned until explicit routing priors are supplied.
        """

        return [
            descriptor.to_dict()
            for descriptor in self.discover_provider_model_descriptors(
                provider,
                credentials,
                path=path,
                limit=limit,
            )
        ]

    def discover_provider_model_descriptors(
        self,
        provider: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        *,
        path: str | None = None,
        limit: int = MAX_PROVIDER_DISCOVERED_MODELS,
    ) -> tuple[ProviderModelDescriptor, ...]:
        """Return typed inventory rows for direct, explicit catalogue registration."""

        resolved_credentials = self._credential_mapping(credentials)
        return self.runtime.discover_models(
            provider,
            credential=resolved_credentials.get(provider),
            path=path,
            limit=limit,
        )

    def register_discovered_models(
        self,
        descriptors: Sequence[ProviderModelDescriptor],
        *,
        priors: Mapping[str, Mapping[str, Any]],
        replace_existing: bool = False,
    ) -> list[ModelCandidate]:
        """Promote discovered rows only after the application supplies explicit routing priors."""

        return self.catalogue.register_discovered(
            descriptors,
            priors=priors,
            replace_existing=replace_existing,
        )

    def reconcile_discovered_models(
        self,
        descriptors: Sequence[ProviderModelDescriptor],
        *,
        priors: Mapping[str, Mapping[str, Any]],
        providers: Sequence[str] | None = None,
    ) -> dict[str, Any]:
        """Atomically reconcile discovered provider arms, including stale-arm retirement."""

        return self.catalogue.reconcile_discovered(
            descriptors,
            priors=priors,
            providers=providers,
        )

    def _model_inventory_persistence_for(
        self,
        snapshot_store: AutonomousModelInventoryStore,
    ) -> AutonomousModelInventoryPersistenceCoordinator:
        coordinator = self.model_inventory_persistence
        if coordinator is None or coordinator.store is not snapshot_store:
            coordinator = AutonomousModelInventoryPersistenceCoordinator(
                self.model_inventory,
                snapshot_store,
            )
            self.model_inventory_persistence = coordinator
        return coordinator

    def refresh_model_inventory(
        self,
        *,
        credentials: Mapping[str, CredentialHandle] | CredentialSession | None = None,
        providers: Sequence[str] | None = None,
        priors: Mapping[str, Mapping[str, Any]] | None = None,
        prior_factory: Callable[[ProviderModelDescriptor], Mapping[str, Any]] | None = None,
        domain_requirements: Mapping[str, Sequence[str]] | None = None,
        limit: int = MAX_PROVIDER_DISCOVERED_MODELS,
        snapshot_store: AutonomousModelInventoryStore | None = None,
        refresh_id: str | None = None,
        raise_on_error: bool = False,
    ) -> dict[str, Any]:
        """Refresh live provider model inventory and expose coverage for every reviewed domain.

        Discovery is provider-authenticated when required, but the returned snapshot is always
        metadata-only.  Explicit ``priors`` remain mandatory for each discovered ``provider/model``
        arm, or ``prior_factory`` may derive those priors from each typed descriptor after
        discovery; the coordinator reconciles one provider at a time so a failed provider cannot
        retire models belonging to another provider. By default the coverage rows are derived
        from all configured domain packs, making model availability visible before automatic
        routing.
        """

        if domain_requirements is None:
            domain_requirements = {
                row["domain"]: tuple(row["model_capabilities"])
                for row in self.orchestrator.pack_registry.catalogue()
                if isinstance(row, Mapping)
                and isinstance(row.get("domain"), str)
                and isinstance(row.get("model_capabilities"), Sequence)
            }
        try:
            if snapshot_store is None:
                snapshot = self.model_inventory.refresh(
                    credentials=credentials,
                    providers=providers,
                    priors=priors,
                    prior_factory=prior_factory,
                    domain_requirements=domain_requirements,
                    limit=limit,
                    refresh_id=refresh_id,
                    raise_on_error=raise_on_error,
                )
            else:
                snapshot = self._model_inventory_persistence_for(snapshot_store).refresh(
                    credentials=credentials,
                    providers=providers,
                    priors=priors,
                    prior_factory=prior_factory,
                    domain_requirements=domain_requirements,
                    limit=limit,
                    refresh_id=refresh_id,
                    raise_on_error=raise_on_error,
                )
        except Exception as error:
            if isinstance(error, BrainRunError):
                raise
            raise BrainRunError("model inventory refresh failed") from error
        if not isinstance(snapshot, AutonomousModelInventorySnapshot):
            raise BrainRunError("model inventory coordinator returned an invalid snapshot")
        return snapshot.to_dict()

    def restore_model_inventory(
        self,
        snapshot_store: AutonomousModelInventoryStore,
    ) -> dict[str, Any] | None:
        """Restore a metadata-only model catalogue and retain its next inventory CAS fence."""

        try:
            snapshot = self._model_inventory_persistence_for(snapshot_store).restore()
        except AutonomousModelInventoryError as error:
            raise BrainRunError("model inventory restore failed") from error
        return None if snapshot is None else snapshot.to_dict()

    def model_inventory_readiness(
        self,
        *,
        domain_requirements: Mapping[str, Sequence[str]] | None = None,
        estimated_input_tokens: int = 4_096,
        requested_output_tokens: int = 1_024,
    ) -> dict[str, Any]:
        """Return a provider-free live eligibility projection for the model catalogue.

        This complements ``refresh_model_inventory``: refresh performs authenticated discovery
        and may mutate the caller-owned catalogue, while this method only joins the current
        catalogue with the reviewed domain requirements and live runtime gates. It is safe to
        call from a readiness screen before a user has supplied any credential.
        """

        if domain_requirements is None:
            domain_requirements = {
                domain: tuple(self.orchestrator.registry.resolve(domain).required_model_capabilities)
                for domain in AUTONOMOUS_DOMAINS
            }
        try:
            report = self.model_inventory.readiness(
                domain_requirements=domain_requirements,
                estimated_input_tokens=estimated_input_tokens,
                requested_output_tokens=requested_output_tokens,
            )
        except Exception as error:
            if isinstance(error, BrainRunError):
                raise
            raise BrainRunError("model inventory readiness projection failed") from error
        if not isinstance(report, AutonomousModelInventoryReadiness):
            raise BrainRunError("model inventory coordinator returned an invalid readiness report")
        return report.to_dict()

    def flush_model_inventory(
        self,
        snapshot_store: AutonomousModelInventoryStore,
    ) -> dict[str, Any] | None:
        """Re-commit the last inventory image without rediscovering or contacting a provider."""

        try:
            snapshot = self._model_inventory_persistence_for(snapshot_store).flush()
        except AutonomousModelInventoryError as error:
            raise BrainRunError("model inventory flush failed") from error
        return None if snapshot is None else snapshot.to_dict()

    def _persistence_lifecycle_for(
        self,
        model_inventory_store: AutonomousModelInventoryStore | None = None,
        *,
        activation_store: AutonomousCapabilityActivationStore | None = None,
        selection_promotion_store: AutonomousSelectionPromotionLifecycleStore | None = None,
        capability_journal_persistence: AutonomousCapabilityJournalPersistenceCoordinator | None = None,
        decision_cycle_persistence: AutonomousDecisionCyclePersistenceCoordinator | None = None,
        execution_persistence: AutonomousExecutionPersistenceCoordinator | None = None,
        require_all: bool = False,
        continue_on_error: bool = False,
    ) -> AutonomousAgentPersistenceLifecycleCoordinator:
        coordinator = self._persistence_lifecycle_coordinator
        if (
            coordinator is None
            or coordinator.model_inventory_store is not model_inventory_store
            or coordinator.activation_store is not activation_store
            or coordinator.selection_promotion_store is not selection_promotion_store
            or coordinator.capability_journal_persistence is not capability_journal_persistence
            or coordinator.decision_cycle_persistence is not decision_cycle_persistence
            or coordinator.execution_persistence is not execution_persistence
            or coordinator.require_all != require_all
            or coordinator.continue_on_error != continue_on_error
        ):
            coordinator = AutonomousAgentPersistenceLifecycleCoordinator(
                self,
                model_inventory_store=model_inventory_store,
                activation_store=activation_store,
                selection_promotion_store=selection_promotion_store,
                capability_journal_persistence=capability_journal_persistence,
                decision_cycle_persistence=decision_cycle_persistence,
                execution_persistence=execution_persistence,
                require_all=require_all,
                continue_on_error=continue_on_error,
            )
            self._persistence_lifecycle_coordinator = coordinator
            self._persistence_lifecycle_activation_store = activation_store
            self._persistence_lifecycle_selection_promotion_store = selection_promotion_store
            self._persistence_lifecycle_capability_journal_persistence = capability_journal_persistence
            self._persistence_lifecycle_decision_cycle_persistence = decision_cycle_persistence
            self._persistence_lifecycle_execution_persistence = execution_persistence
        return coordinator

    def restore_persisted_state(
        self,
        *,
        model_inventory_store: AutonomousModelInventoryStore | None = None,
        activation_store: AutonomousCapabilityActivationStore | None = None,
        selection_promotion_store: AutonomousSelectionPromotionLifecycleStore | None = None,
        capability_journal_persistence: AutonomousCapabilityJournalPersistenceCoordinator | None = None,
        decision_cycle_persistence: AutonomousDecisionCyclePersistenceCoordinator | None = None,
        execution_persistence: AutonomousExecutionPersistenceCoordinator | None = None,
        strict: bool = True,
        require_all: bool = False,
        continue_on_error: bool = False,
    ) -> dict[str, Any]:
        """Restore all configured brain metadata stores in dependency order."""

        report = self._persistence_lifecycle_for(
            model_inventory_store,
            activation_store=activation_store,
            selection_promotion_store=selection_promotion_store,
            capability_journal_persistence=(self.capability_journal_persistence if capability_journal_persistence is None else capability_journal_persistence),
            decision_cycle_persistence=(self.decision_cycle_persistence if decision_cycle_persistence is None else decision_cycle_persistence),
            execution_persistence=(self.execution_persistence if execution_persistence is None else execution_persistence),
            require_all=require_all,
            continue_on_error=continue_on_error,
        ).restore(strict=strict, continue_on_error=continue_on_error)
        return report.to_dict()

    def flush_persisted_state(
        self,
        *,
        model_inventory_store: AutonomousModelInventoryStore | None = None,
        activation_store: AutonomousCapabilityActivationStore | None = None,
        selection_promotion_store: AutonomousSelectionPromotionLifecycleStore | None = None,
        capability_journal_persistence: AutonomousCapabilityJournalPersistenceCoordinator | None = None,
        decision_cycle_persistence: AutonomousDecisionCyclePersistenceCoordinator | None = None,
        execution_persistence: AutonomousExecutionPersistenceCoordinator | None = None,
        strict: bool = True,
        require_all: bool = False,
        continue_on_error: bool = False,
    ) -> dict[str, Any]:
        """Flush all configured brain metadata stores in reverse dependency order."""

        report = self._persistence_lifecycle_for(
            model_inventory_store,
            activation_store=activation_store,
            selection_promotion_store=selection_promotion_store,
            capability_journal_persistence=(self.capability_journal_persistence if capability_journal_persistence is None else capability_journal_persistence),
            decision_cycle_persistence=(self.decision_cycle_persistence if decision_cycle_persistence is None else decision_cycle_persistence),
            execution_persistence=(self.execution_persistence if execution_persistence is None else execution_persistence),
            require_all=require_all,
            continue_on_error=continue_on_error,
        ).flush(strict=strict, continue_on_error=continue_on_error)
        return report.to_dict()

    def register_provider(self, config: ProviderConfig) -> None:
        """Register non-secret provider transport metadata for the key-entry flow."""

        self.onboarding.register_provider(config)

    def register_environment_credential_source(
        self,
        provider: str,
        *,
        variable: str | None = None,
        ttl_seconds: float | None = None,
        required: bool = True,
        source_label: str | None = None,
        replace_existing: bool = False,
    ) -> CredentialSourceSpec:
        """Register deployment-managed environment resolution without accepting a raw key."""

        return self.credential_provisioner.register_environment(
            provider,
            variable=variable,
            ttl_seconds=ttl_seconds,
            required=required,
            source_label=source_label,
            replace_existing=replace_existing,
        )

    def register_secret_manager_credential_source(
        self,
        provider: str,
        reference: str,
        resolver: Callable[[str], str],
        *,
        ttl_seconds: float | None = None,
        required: bool = True,
        source_label: str | None = None,
        replace_existing: bool = False,
    ) -> CredentialSourceSpec:
        """Register a process-local secret-manager resolver; only its digest is projectable."""

        return self.credential_provisioner.register_resolver(
            provider,
            reference,
            resolver,
            ttl_seconds=ttl_seconds,
            required=required,
            source_label=source_label,
            replace_existing=replace_existing,
        )

    def credential_provisioning_plan(
        self,
        providers: Sequence[str] | None = None,
    ) -> dict[str, Any]:
        """Return the non-secret deployment credential bootstrap plan."""

        return self.credential_provisioner.plan(providers)

    def provision_credentials(
        self,
        session: CredentialSession,
        *,
        providers: Sequence[str] | None = None,
        environ: Mapping[str, str] | None = None,
    ) -> CredentialProvisioningResult:
        """Resolve configured deployment sources into a live short-lived credential session."""

        return self.credential_provisioner.provision(
            session,
            providers=providers,
            environ=environ,
        )

    def start_provisioned_credential_session(
        self,
        *,
        providers: Sequence[str] | None = None,
        ttl_seconds: float | None = None,
        session_id: str | None = None,
        environ: Mapping[str, str] | None = None,
        require_ready: bool = True,
    ) -> tuple[CredentialSession, CredentialProvisioningResult]:
        """Start and populate a fresh session from deployment sources without human key entry.

        The returned session is caller-owned and must be closed after the execution scope.  If
        ``require_ready`` is true, a failed bootstrap closes the session before raising, so a
        caller cannot accidentally dispatch with a partial credential set.
        """

        session = self.start_credential_session(ttl_seconds=ttl_seconds, session_id=session_id)
        try:
            result = self.provision_credentials(session, providers=providers, environ=environ)
            if require_ready and not result.ready:
                session.close()
                raise BrainRunError(
                    "credential provisioning is incomplete for providers: "
                    + ", ".join(result.required_failures)
                )
            return session, result
        except Exception:
            session.close()
            raise

    def _run_with_provisioned_credentials(
        self,
        *,
        credential_providers: Sequence[str] | None,
        credential_ttl_seconds: float | None,
        provision_environ: Mapping[str, str] | None,
        require_ready: bool,
        refresh_inventory: bool,
        inventory_priors: Mapping[str, Mapping[str, Any]] | None,
        inventory_prior_factory: Callable[[ProviderModelDescriptor], Mapping[str, Any]] | None,
        inventory_domain_requirements: Mapping[str, Sequence[str]] | None,
        inventory_limit: int,
        inventory_snapshot_store: AutonomousModelInventoryStore | None,
        inventory_refresh_id: str | None,
        runner: Callable[[CredentialSession], Any],
    ) -> AutonomousProvisionedRun:
        """Compose request-scoped provisioning, optional discovery, execution, and revocation."""

        if not isinstance(require_ready, bool):
            raise BrainRunError("require_ready must be a boolean")
        if not isinstance(refresh_inventory, bool):
            raise BrainRunError("refresh_inventory must be a boolean")
        if not callable(runner):
            raise BrainRunError("provisioned execution runner must be callable")
        session, provisioning = self.start_provisioned_credential_session(
            providers=credential_providers,
            ttl_seconds=credential_ttl_seconds,
            environ=provision_environ,
            require_ready=require_ready,
        )
        try:
            inventory: Mapping[str, Any] | None = None
            if refresh_inventory:
                # Discovery is deliberately strict here. A caller that opts into inventory
                # refresh must not execute against a stale catalogue after discovery failed.
                inventory = self.refresh_model_inventory(
                    credentials=session,
                    providers=credential_providers,
                    priors=inventory_priors,
                    prior_factory=inventory_prior_factory,
                    domain_requirements=inventory_domain_requirements,
                    limit=inventory_limit,
                    snapshot_store=inventory_snapshot_store,
                    refresh_id=inventory_refresh_id,
                    raise_on_error=True,
                )
            result = runner(session)
            return AutonomousProvisionedRun(
                result=result,
                provisioning=provisioning,
                inventory=inventory,
            )
        finally:
            # Handles are valid only for this request. This runs for provider refusal, route
            # abstention, inventory failure, and ordinary success alike.
            session.close()

    def run_with_provisioned_credentials(
        self,
        *,
        task: str,
        domain: str,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        credential_providers: Sequence[str] | None = None,
        credential_ttl_seconds: float | None = None,
        provision_environ: Mapping[str, str] | None = None,
        require_ready: bool = True,
        refresh_inventory: bool = False,
        inventory_priors: Mapping[str, Mapping[str, Any]] | None = None,
        inventory_prior_factory: Callable[[ProviderModelDescriptor], Mapping[str, Any]] | None = None,
        inventory_domain_requirements: Mapping[str, Sequence[str]] | None = None,
        inventory_limit: int = MAX_PROVIDER_DISCOVERED_MODELS,
        inventory_snapshot_store: AutonomousModelInventoryStore | None = None,
        inventory_refresh_id: str | None = None,
        **kwargs: Any,
    ) -> AutonomousProvisionedRun:
        """Execute one explicit-domain task from deployment-managed credentials.

        The application registers environment or secret-manager wiring once, then this method
        creates a fresh opaque-handle session, optionally refreshes authenticated model
        inventory, invokes the existing explicit ``run`` path, and revokes every handle in a
        ``finally`` block. ``result`` remains available to the initiating caller; ``to_dict``
        is safe for persistence and never includes that provider result.
        """

        if "credentials" in kwargs:
            raise BrainRunError("provisioned execution owns credentials; do not pass credentials")
        return self._run_with_provisioned_credentials(
            credential_providers=credential_providers,
            credential_ttl_seconds=credential_ttl_seconds,
            provision_environ=provision_environ,
            require_ready=require_ready,
            refresh_inventory=refresh_inventory,
            inventory_priors=inventory_priors,
            inventory_prior_factory=inventory_prior_factory,
            inventory_domain_requirements=inventory_domain_requirements,
            inventory_limit=inventory_limit,
            inventory_snapshot_store=inventory_snapshot_store,
            inventory_refresh_id=inventory_refresh_id,
            runner=lambda session: self.run(
                task=task,
                domain=domain,
                credentials=session,
                model_candidates=model_candidates,
                **kwargs,
            ),
        )

    def run_auto_with_provisioned_credentials(
        self,
        *,
        task: str,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        credential_providers: Sequence[str] | None = None,
        credential_ttl_seconds: float | None = None,
        provision_environ: Mapping[str, str] | None = None,
        require_ready: bool = True,
        refresh_inventory: bool = False,
        inventory_priors: Mapping[str, Mapping[str, Any]] | None = None,
        inventory_prior_factory: Callable[[ProviderModelDescriptor], Mapping[str, Any]] | None = None,
        inventory_domain_requirements: Mapping[str, Sequence[str]] | None = None,
        inventory_limit: int = MAX_PROVIDER_DISCOVERED_MODELS,
        inventory_snapshot_store: AutonomousModelInventoryStore | None = None,
        inventory_refresh_id: str | None = None,
        **kwargs: Any,
    ) -> AutonomousProvisionedRun:
        """Provision, optionally discover, and execute automatic single/cross-domain routing."""

        if "credentials" in kwargs:
            raise BrainRunError("provisioned execution owns credentials; do not pass credentials")
        return self._run_with_provisioned_credentials(
            credential_providers=credential_providers,
            credential_ttl_seconds=credential_ttl_seconds,
            provision_environ=provision_environ,
            require_ready=require_ready,
            refresh_inventory=refresh_inventory,
            inventory_priors=inventory_priors,
            inventory_prior_factory=inventory_prior_factory,
            inventory_domain_requirements=inventory_domain_requirements,
            inventory_limit=inventory_limit,
            inventory_snapshot_store=inventory_snapshot_store,
            inventory_refresh_id=inventory_refresh_id,
            runner=lambda session: self.run_auto(
                task=task,
                credentials=session,
                model_candidates=model_candidates,
                **kwargs,
            ),
        )

    def run_with_provisioned_credentials_with_launch_admission(
        self,
        *,
        task: str,
        domain: str,
        launch_admission: Mapping[str, Any],
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        **kwargs: Any,
    ) -> AutonomousProvisionedRun:
        """Admit an explicit domain before provisioning any deployment-managed credential."""

        _authorize_launch_admission_domains(launch_admission, (domain,))
        return self.run_with_provisioned_credentials(
            task=task,
            domain=domain,
            model_candidates=model_candidates,
            **kwargs,
        )

    def run_auto_with_provisioned_credentials_with_launch_admission(
        self,
        *,
        task: str,
        launch_admission: Mapping[str, Any],
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        **kwargs: Any,
    ) -> AutonomousProvisionedRun:
        """Preview and admit automatic routing before provisioning deployment-managed credentials."""

        self.authorize_auto_launch_admission(
            task=task,
            launch_admission=launch_admission,
            **kwargs,
        )
        return self.run_auto_with_provisioned_credentials(
            task=task,
            model_candidates=model_candidates,
            **kwargs,
        )

    def unregister_credential_source(self, provider: str, source_id: str) -> bool:
        """Remove deployment wiring; active sessions remain caller-owned and independently revocable."""

        return self.credential_provisioner.unregister(provider, source_id)

    def run_resumable_learning_job(
        self,
        store: Any,
        *,
        job_id: str,
        worker_id: str,
        resolver: Callable[[Mapping[str, Any]], Mapping[str, Any]],
        evaluator: BrainOutcomeEvaluator,
        bandit_state: Mapping[str, Any],
        credential_providers: Sequence[str] | None = None,
        credential_ttl_seconds: float | None = None,
        provision_environ: Mapping[str, str] | None = None,
        **kwargs: Any,
    ) -> BrainJobRunResult:
        """Run a mission-learning job while resolving deployment credentials per attempt."""

        return self._run_resumable_with_provisioned_credentials(
            resolver,
            credential_providers=credential_providers,
            credential_ttl_seconds=credential_ttl_seconds,
            provision_environ=provision_environ,
            runner=lambda managed: self.brain.run_resumable_learning_job(
                store,
                job_id=job_id,
                worker_id=worker_id,
                resolver=managed,
                evaluator=evaluator,
                bandit_state=bandit_state,
                **self._job_kwargs(kwargs),
            ),
        )

    def run_resumable_workflow_job(
        self,
        store: Any,
        *,
        job_id: str,
        worker_id: str,
        resolver: Callable[[Mapping[str, Any]], Mapping[str, Any]],
        evaluator: BrainOutcomeEvaluator | None,
        bandit_state: Mapping[str, Any],
        credential_providers: Sequence[str] | None = None,
        credential_ttl_seconds: float | None = None,
        provision_environ: Mapping[str, str] | None = None,
        **kwargs: Any,
    ) -> BrainJobRunResult:
        """Run one bounded workflow continuation with fresh deployment credentials."""

        return self._run_resumable_with_provisioned_credentials(
            resolver,
            credential_providers=credential_providers,
            credential_ttl_seconds=credential_ttl_seconds,
            provision_environ=provision_environ,
            runner=lambda managed: self.brain.run_resumable_workflow_job(
                store,
                job_id=job_id,
                worker_id=worker_id,
                resolver=managed,
                evaluator=evaluator,
                bandit_state=bandit_state,
                **self._job_kwargs(kwargs),
            ),
        )

    def run_resumable_cross_domain_job(
        self,
        store: Any,
        *,
        job_id: str,
        worker_id: str,
        resolver: Callable[[Mapping[str, Any]], Mapping[str, Any]],
        evaluator: BrainOutcomeEvaluator | None,
        bandit_state: Mapping[str, Any],
        credential_providers: Sequence[str] | None = None,
        credential_ttl_seconds: float | None = None,
        provision_environ: Mapping[str, str] | None = None,
        **kwargs: Any,
    ) -> BrainJobRunResult:
        """Run one cross-domain child/synthesis continuation with fresh credentials."""

        return self._run_resumable_with_provisioned_credentials(
            resolver,
            credential_providers=credential_providers,
            credential_ttl_seconds=credential_ttl_seconds,
            provision_environ=provision_environ,
            runner=lambda managed: self.brain.run_resumable_cross_domain_job(
                store,
                job_id=job_id,
                worker_id=worker_id,
                resolver=managed,
                evaluator=evaluator,
                bandit_state=bandit_state,
                **self._job_kwargs(kwargs),
            ),
        )

    def _run_resumable_with_provisioned_credentials(
        self,
        resolver: Callable[[Mapping[str, Any]], Mapping[str, Any]],
        *,
        credential_providers: Sequence[str] | None,
        credential_ttl_seconds: float | None,
        provision_environ: Mapping[str, str] | None,
        runner: Callable[[Callable[[Mapping[str, Any]], Mapping[str, Any]]], BrainJobRunResult],
    ) -> BrainJobRunResult:
        if not callable(resolver):
            raise BrainRunError("resumable job resolver must be callable")
        if credential_providers is None:
            configured = tuple(sorted({spec.provider for spec in self.credential_provisioner.source_specs()}))
        else:
            configured = credential_providers
        should_provision = credential_providers is not None or bool(configured)
        if not should_provision:
            return runner(resolver)
        active_sessions: list[CredentialSession] = []

        def managed(job_metadata: Mapping[str, Any]) -> Mapping[str, Any]:
            resolved = resolver(job_metadata)
            if not isinstance(resolved, Mapping):
                raise BrainRunError("resumable job resolver must return a mapping")
            session, _provisioning = self.start_provisioned_credential_session(
                providers=configured,
                ttl_seconds=credential_ttl_seconds,
                environ=provision_environ,
                require_ready=True,
            )
            active_sessions.append(session)
            # The mapping is transient and consumed only by the brain call. The durable job
            # receives only its public metadata/checkpoint, never this credential snapshot.
            return {**dict(resolved), "credentials": session.handles()}

        try:
            return runner(managed)
        finally:
            for session in active_sessions:
                session.close()

    def _job_kwargs(self, kwargs: Mapping[str, Any]) -> dict[str, Any]:
        resolved = dict(kwargs)
        if "provider_health" not in resolved and self.health_ledger is not None:
            resolved["provider_health"] = self.health_ledger.health_snapshot()
        if "model_health" not in resolved and self.health_ledger is not None:
            resolved["model_health"] = self.health_ledger.model_health_snapshot()
        return resolved

    @staticmethod
    def _merge_selection_overrides(
        historical: Mapping[str, Any],
        supplied: Mapping[str, Any] | None,
    ) -> Mapping[str, Any] | None:
        """Merge durable provider/model health while preserving caller-owned overlay values."""

        if supplied is None:
            return historical or None
        if not isinstance(supplied, Mapping):
            return supplied
        merged = dict(historical)
        merged.update(dict(supplied))
        for field_name in ("provider_health", "model_health"):
            historical_rows = historical.get(field_name)
            supplied_rows = supplied.get(field_name)
            if isinstance(historical_rows, Mapping) and isinstance(supplied_rows, Mapping):
                merged[field_name] = {**dict(historical_rows), **dict(supplied_rows)}
        return merged

    def credential_status(self, provider: str) -> dict[str, Any]:
        """Return one redacted provider onboarding state for a UI or request gate."""

        return self.onboarding.status(provider)

    def credential_statuses(self) -> list[dict[str, Any]]:
        """Return redacted onboarding states without returning keys, handles, or references."""

        return self.onboarding.statuses()

    def credential_instructions(self, provider: str) -> dict[str, Any]:
        """Return the redacted key-collection contract for a protected application UI.

        The result tells an embedding application whether the provider is registered, which
        input paths are supported, and what next action to render.  It never returns a key or
        asks the autonomous brain to collect one; the UI submits its value through the live
        :class:`CredentialSession` instead.
        """

        return self.onboarding.instructions(provider).to_dict()

    def start_credential_session(
        self,
        *,
        ttl_seconds: float | None = None,
        session_id: str | None = None,
    ) -> CredentialSession:
        """Start a short-lived BYOK session for protected UI or request-scoped collection."""

        return self.onboarding.start_session(ttl_seconds=ttl_seconds, session_id=session_id)

    def activation_state(self) -> dict[str, Any]:
        """Return the redacted durable provider/domain activation snapshot."""

        return self.activation.to_dict()

    def selection_promotion_state(self) -> dict[str, Any] | None:
        """Return the digest-only authority state for learned model selection."""

        return None if self.selection_promotion is None else self.selection_promotion.state.to_dict()

    def apply_selection_promotion(self, report: Mapping[str, Any]) -> dict[str, Any]:
        """Apply replay admission and make learned selection eligible only when admitted."""

        if self.selection_promotion is None:
            raise BrainRunError("selection promotion lifecycle is not configured")
        try:
            return self.selection_promotion.apply(report).to_dict()
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("selection promotion could not be applied") from error

    def rollback_selection_promotion(self, *, reason: str = "selection_promotion_rollback") -> dict[str, Any]:
        """Immediately stop promoted learned selection while retaining rollback metadata."""

        if self.selection_promotion is None:
            raise BrainRunError("selection promotion lifecycle is not configured")
        try:
            return self.selection_promotion.rollback(reason=reason).to_dict()
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("selection promotion could not be rolled back") from error

    def save_selection_promotion(
        self,
        store: AutonomousSelectionPromotionLifecycleStore,
    ) -> dict[str, Any]:
        """Persist only digest-bound learned-selection authority metadata."""

        if not isinstance(store, AutonomousSelectionPromotionLifecycleStore):
            raise BrainRunError(
                "save_selection_promotion requires an AutonomousSelectionPromotionLifecycleStore"
            )
        if self.selection_promotion is None:
            raise BrainRunError("selection promotion lifecycle is not configured")
        try:
            store.save(self.selection_promotion.state)
            return store.snapshot()
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("selection promotion state could not be persisted") from error

    def restore_selection_promotion(
        self,
        store: AutonomousSelectionPromotionLifecycleStore,
    ) -> dict[str, Any] | None:
        """Restore learned-selection authority after store digest and revision validation."""

        if not isinstance(store, AutonomousSelectionPromotionLifecycleStore):
            raise BrainRunError(
                "restore_selection_promotion requires an AutonomousSelectionPromotionLifecycleStore"
            )
        if self.selection_promotion is None:
            raise BrainRunError("selection promotion lifecycle is not configured")
        try:
            state = store.load()
            return None if state is None else self.selection_promotion.restore(state).to_dict()
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("selection promotion state could not be restored") from error

    def _assert_selection_promotion_admitted(self) -> None:
        if self.selection_promotion is None:
            return
        if not self.selection_promotion.is_admitted():
            state = self.selection_promotion.state
            raise BrainRunError(
                "learned model selection is not admitted: "
                f"{state.status}; {state.last_reason or 'apply an admitted promotion report'}"
            )

    def save_activation(
        self,
        store: AutonomousCapabilityActivationStore,
    ) -> dict[str, Any]:
        """Persist activation metadata atomically without persisting keys or handles."""

        if not isinstance(store, AutonomousCapabilityActivationStore):
            raise BrainRunError("save_activation requires an AutonomousCapabilityActivationStore")
        try:
            return store.save(self.activation)
        except AutonomousActivationError as error:
            raise BrainRunError("activation state could not be persisted") from error

    def restore_activation(
        self,
        store: AutonomousCapabilityActivationStore,
    ) -> dict[str, Any] | None:
        """Restore redacted activation state while preserving revocation and identity fences."""

        if not isinstance(store, AutonomousCapabilityActivationStore):
            raise BrainRunError("restore_activation requires an AutonomousCapabilityActivationStore")
        try:
            state = store.load()
            return None if state is None else self.activation.restore(state).to_dict()
        except AutonomousActivationError as error:
            raise BrainRunError("activation state could not be restored") from error

    def revoke_activation(self, *, reason: str = "activation_revoked") -> dict[str, Any]:
        """Revoke the activation snapshot without pretending to revoke provider credentials."""

        try:
            return self.activation.revoke(reason=reason).to_dict()
        except AutonomousActivationError as error:
            raise BrainRunError("activation could not be revoked") from error

    def models(self, *, enabled_only: bool = False) -> list[dict[str, Any]]:
        """Return deterministic model metadata suitable for a configuration UI."""

        return self.catalogue.candidates(enabled_only=enabled_only)

    def domains(self) -> list[dict[str, Any]]:
        """Return the redacted domain strategy catalogue used by automatic intake."""

        return self.orchestrator.registry.catalogue()

# Keep method introspection compatible with the public AutonomousAgent class.
for _name, _descriptor in vars(AutonomousAgentCatalogMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, staticmethod) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousAgent.{_name}"
