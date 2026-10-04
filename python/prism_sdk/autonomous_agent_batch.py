"""Focused autonomous-agent batch methods."""

from __future__ import annotations

from .autonomy import (
    AUTONOMOUS_BATCH_CHECKPOINT_SCHEMA,
    AUTONOMOUS_BATCH_MODES,
    AUTONOMOUS_DOMAINS,
    AUTONOMOUS_TRACED_AUTO_BATCH_SCHEMA,
    Any,
    AutonomousBatchCheckpoint,
    AutonomousBatchItem,
    AutonomousBatchRehydrationContext,
    AutonomousBatchResult,
    AutonomousCrossDomainResult,
    AutonomousCrossDomainTrajectoryLearningResult,
    AutonomousRunTraceSession,
    AutonomousRunTraceStore,
    AutonomousTracedRunResult,
    BrainEpisodicMemory,
    BrainLearningLedger,
    BrainOutcomeEvaluator,
    BrainRunError,
    Callable,
    CredentialHandle,
    CredentialSession,
    Lock,
    MAX_AUTONOMOUS_AGENT_BATCH,
    MAX_AUTONOMOUS_AGENT_PARALLELISM,
    MAX_AUTONOMOUS_CROSS_DOMAIN_CHILDREN,
    MAX_AUTONOMY_TEXT_BYTES,
    Mapping,
    ModelCandidate,
    Sequence,
    ThreadPoolExecutor,
    _batch_automatic_execution_policy_digest,
    _batch_digest,
    _batch_error_projection,
    _batch_item_digest,
    _batch_request_digest,
    _batch_result_classification,
    _batch_semantic_routing_policy_digest,
    _identifier,
    _text,
    autonomous_run_trace_status,
    content_digest,
    uuid,
)

class AutonomousAgentBatchMixin:
    @staticmethod
    def _batch_controls(max_parallelism: int, stop_on_error: bool) -> tuple[int, bool]:
        if (
            not isinstance(max_parallelism, int)
            or isinstance(max_parallelism, bool)
            or not 1 <= max_parallelism <= MAX_AUTONOMOUS_AGENT_PARALLELISM
        ):
            raise BrainRunError(
                "autonomous batch max_parallelism must be between 1 and "
                f"{MAX_AUTONOMOUS_AGENT_PARALLELISM}"
            )
        if not isinstance(stop_on_error, bool):
            raise BrainRunError("autonomous batch stop_on_error must be a boolean")
        return max_parallelism, stop_on_error

    def _authorize_batch_launch_admission(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        mode: str,
        launch_admission: Mapping[str, Any],
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None,
    ) -> Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None:
        """Authorize every route in a batch before credentials or dispatch are touched.

        Batch preparation normally resolves the shared credential mapping before it builds item
        descriptors.  A launch admission must sit outside that path: callers should be able to
        reject a held or under-scoped batch even when the credential argument is deliberately
        absent or malformed.  The options-factory replay cache also makes the provider-free route
        preview and the later executor consume exactly one caller-produced option mapping per item.
        """

        from .autonomous_launch_admission import authorize_autonomous_launch_domains

        if mode not in AUTONOMOUS_BATCH_MODES:
            raise BrainRunError("autonomous batch mode must be one of: domain, auto, cross_domain")
        if not isinstance(requests, Sequence) or isinstance(requests, (str, bytes)):
            raise BrainRunError("autonomous batch requests must be a sequence")
        if not 1 <= len(requests) <= MAX_AUTONOMOUS_AGENT_BATCH:
            raise BrainRunError(
                "autonomous batch requests must contain between 1 and "
                f"{MAX_AUTONOMOUS_AGENT_BATCH} entries"
            )
        if options_factory is not None and not callable(options_factory):
            raise BrainRunError("autonomous batch options_factory must be callable or None")

        cached_factory_options: dict[int, Mapping[str, Any]] = {}

        def merged_options(raw: Mapping[str, Any], index: int) -> dict[str, Any]:
            raw_options = raw.get("options", {})
            if raw_options is None:
                raw_options = {}
            if not isinstance(raw_options, Mapping):
                raise BrainRunError(f"autonomous batch request {index} options must be a mapping")
            options = dict(raw_options)
            if options_factory is not None:
                generated = cached_factory_options.get(index)
                if generated is None:
                    try:
                        generated_value = options_factory(raw, index)
                    except Exception as error:
                        raise BrainRunError(
                            f"autonomous batch options_factory failed for request {index}"
                        ) from error
                    if not isinstance(generated_value, Mapping):
                        raise BrainRunError(
                            f"autonomous batch options_factory result {index} must be a mapping"
                        )
                    generated = dict(generated_value)
                    cached_factory_options[index] = generated
                options.update(generated)
            reserved = {
                "credentials",
                "task",
                "domain",
                "subtasks",
                "model_candidates",
                "execution_id",
            }
            overridden = sorted(reserved.intersection(options))
            if overridden:
                raise BrainRunError(
                    f"autonomous batch request {index} options cannot override: {', '.join(overridden)}"
                )
            return options

        requested_domains: list[str] = []
        for index, raw in enumerate(requests):
            if not isinstance(raw, Mapping):
                raise BrainRunError(f"autonomous batch request {index} must be a mapping")
            if "credentials" in raw:
                raise BrainRunError(
                    "autonomous batch requests cannot carry credentials; pass one shared opaque "
                    "credential mapping or session"
                )
            task = _text(
                f"autonomous batch request {index} task",
                raw.get("task"),
                maximum=MAX_AUTONOMY_TEXT_BYTES,
            )
            options = merged_options(raw, index)
            if mode == "domain":
                domain = raw.get("domain")
                _identifier(f"autonomous batch request {index} domain", domain)
                if domain not in AUTONOMOUS_DOMAINS:
                    raise BrainRunError(
                        f"autonomous batch request {index} domain is unsupported: {domain!r}"
                    )
                requested_domains.append(domain)
            elif mode == "cross_domain":
                subtasks = raw.get("subtasks")
                if not isinstance(subtasks, Sequence) or isinstance(subtasks, (str, bytes)):
                    raise BrainRunError(
                        f"autonomous cross-domain batch request {index} subtasks must be a sequence"
                    )
                if not 1 <= len(subtasks) <= MAX_AUTONOMOUS_CROSS_DOMAIN_CHILDREN:
                    raise BrainRunError(
                        f"autonomous cross-domain batch request {index} subtasks are outside their bound"
                    )
                for subtask_index, subtask in enumerate(subtasks):
                    if not isinstance(subtask, Mapping) or not isinstance(subtask.get("domain"), str):
                        raise BrainRunError(
                            f"autonomous cross-domain batch request {index} subtask {subtask_index} has no valid domain"
                        )
                    domain = subtask["domain"]
                    _identifier(
                        f"autonomous cross-domain batch request {index} subtask {subtask_index} domain",
                        domain,
                    )
                    if domain not in AUTONOMOUS_DOMAINS:
                        raise BrainRunError(
                            f"autonomous cross-domain batch request {index} domain is unsupported: {domain!r}"
                        )
                    requested_domains.append(domain)
            else:
                semantic_routing = options.get("semantic_routing", False)
                if not isinstance(semantic_routing, bool):
                    raise BrainRunError("autonomous batch semantic_routing must be boolean")
                if semantic_routing:
                    raise BrainRunError(
                        "launch-admitted automatic batch execution requires provider-free routing; "
                        "admit semantic routing separately before enabling it"
                    )
                route_options = {
                    key: options[key]
                    for key in (
                        "hints",
                        "min_confidence",
                        "min_margin",
                        "max_domains",
                        "allow_cross_domain",
                        "context",
                        "constraints",
                        "desired_outputs",
                        "capability",
                        "risk_class",
                        "max_steps",
                        "require_json",
                        "structured_domain_response",
                        "response_schema",
                        "execution_mode",
                        "max_input_tokens",
                        "required_model_capabilities",
                        "memory_episodes",
                    )
                    if key in options
                }
                blueprint = self.prepare_auto(task=task, **route_options)
                requested_domains.extend(blueprint.route.selected_domains)

        authorize_autonomous_launch_domains(
            launch_admission,
            tuple(dict.fromkeys(requested_domains)),
        )
        if options_factory is None:
            return None

        def replay_factory(_raw: Mapping[str, Any], index: int) -> Mapping[str, Any]:
            # The outer batch preparer merges this generated mapping with the raw request options.
            # Returning the cached copy prevents a non-deterministic factory from changing the
            # route after the admission was reviewed.
            try:
                return cached_factory_options[index]
            except KeyError as error:  # pragma: no cover - the preparer always visits each item
                raise BrainRunError(f"autonomous batch options cache is missing request {index}") from error

        return replay_factory

    def _prepare_batch_invocations(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None,
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None,
        cross_domain: bool,
    ) -> tuple[tuple[dict[str, Any], ...], dict[str, CredentialHandle]]:
        if not isinstance(requests, Sequence) or isinstance(requests, (str, bytes)):
            raise BrainRunError("autonomous batch requests must be a sequence")
        if not 1 <= len(requests) <= MAX_AUTONOMOUS_AGENT_BATCH:
            raise BrainRunError(
                "autonomous batch requests must contain between 1 and "
                f"{MAX_AUTONOMOUS_AGENT_BATCH} entries"
            )
        if options_factory is not None and not callable(options_factory):
            raise BrainRunError("autonomous batch options_factory must be callable or None")
        resolved_credentials = self._credential_mapping(credentials)
        common_candidates = self._resolve_candidates(model_candidates)
        prepared: list[dict[str, Any]] = []
        reserved_options = {
            "credentials",
            "task",
            "domain",
            "subtasks",
            "model_candidates",
            "execution_id",
        }
        for index, raw in enumerate(requests):
            if not isinstance(raw, Mapping):
                raise BrainRunError(f"autonomous batch request {index} must be a mapping")
            if "credentials" in raw:
                raise BrainRunError(
                    "autonomous batch requests cannot carry credentials; pass one shared opaque "
                    "credential mapping or session"
                )
            task = _text(
                f"autonomous batch request {index} task",
                raw.get("task"),
                maximum=MAX_AUTONOMY_TEXT_BYTES,
            )
            if cross_domain:
                subtasks = raw.get("subtasks")
                if not isinstance(subtasks, Sequence) or isinstance(subtasks, (str, bytes)):
                    raise BrainRunError(
                        f"autonomous cross-domain batch request {index} subtasks must be a sequence"
                    )
                if not 1 <= len(subtasks) <= MAX_AUTONOMOUS_CROSS_DOMAIN_CHILDREN:
                    raise BrainRunError(
                        f"autonomous cross-domain batch request {index} subtasks are outside their bound"
                    )
            else:
                domain = raw.get("domain")
                _identifier(f"autonomous batch request {index} domain", domain)
                if domain not in AUTONOMOUS_DOMAINS:
                    raise BrainRunError(
                        f"autonomous batch request {index} domain is unsupported: {domain!r}"
                    )
            raw_options = raw.get("options", {})
            if raw_options is None:
                raw_options = {}
            if not isinstance(raw_options, Mapping):
                raise BrainRunError(f"autonomous batch request {index} options must be a mapping")
            options = dict(raw_options)
            if options_factory is not None:
                try:
                    generated = options_factory(raw, index)
                except Exception as error:
                    raise BrainRunError(
                        f"autonomous batch options_factory failed for request {index}"
                    ) from error
                if not isinstance(generated, Mapping):
                    raise BrainRunError(
                        f"autonomous batch options_factory result {index} must be a mapping"
                    )
                options.update(generated)
            reserved = sorted(reserved_options.intersection(options))
            if reserved:
                raise BrainRunError(
                    f"autonomous batch request {index} options cannot override: {', '.join(reserved)}"
                )
            item_candidates = raw.get("model_candidates", common_candidates)
            if item_candidates is None:
                item_candidates = common_candidates
            normalized_candidates = self._resolve_candidates(item_candidates)
            execution_id = raw.get("execution_id")
            prepared.append(
                {
                    "index": index,
                    "task": task,
                    "task_digest": content_digest({"task": task}),
                    "domain": raw.get("domain"),
                    "subtasks": tuple(subtasks) if cross_domain else None,
                    "model_candidates": normalized_candidates,
                    "execution_id": execution_id,
                    "options": options,
                }
            )
        return tuple(prepared), resolved_credentials

    def _invoke_domain_batch_descriptor(
        self,
        descriptor: Mapping[str, Any],
        *,
        credentials: Mapping[str, CredentialHandle],
    ) -> Any:
        """Dispatch one explicit-domain item through focused capability execution when requested."""

        options = dict(descriptor["options"])
        capability = options.pop("capability", None)
        approve_capability = options.pop("approve_capability", False)
        if capability is None:
            if approve_capability:
                raise BrainRunError("approve_capability requires a batch capability")
            return self.run(
                task=descriptor["task"],
                domain=descriptor["domain"],
                credentials=credentials,
                model_candidates=descriptor["model_candidates"],
                execution_id=descriptor["execution_id"],
                **options,
            )
        return self.run_capability(
            task=descriptor["task"],
            domain=descriptor["domain"],
            capability=capability,
            credentials=credentials,
            model_candidates=descriptor["model_candidates"],
            execution_id=descriptor["execution_id"],
            approve_capability=approve_capability,
            **options,
        )

    @staticmethod
    def _execute_prepared_batch(
        prepared: Sequence[Mapping[str, Any]],
        *,
        invoke: Callable[[Mapping[str, Any]], Any],
        max_parallelism: int,
        stop_on_error: bool,
        initial_items: Sequence[AutonomousBatchItem | None] | None = None,
        on_progress: Callable[[Sequence[AutonomousBatchItem | None]], Any] | None = None,
    ) -> AutonomousBatchResult:
        workers = min(max_parallelism, len(prepared))
        if initial_items is not None and len(initial_items) != len(prepared):
            raise BrainRunError("autonomous batch initial item state must align with requests")
        items: list[AutonomousBatchItem | None] = list(initial_items) if initial_items is not None else [None] * len(prepared)
        if any(item is not None and item.index != index for index, item in enumerate(items)):
            raise BrainRunError("autonomous batch initial item state has an invalid index")
        lock = Lock()
        next_index = 0
        halted = False

        def worker() -> None:
            nonlocal next_index, halted
            while True:
                with lock:
                    if next_index >= len(prepared):
                        return
                    index = next_index
                    next_index += 1
                    if items[index] is not None:
                        continue
                    if halted:
                        items[index] = AutonomousBatchItem(
                            index=index,
                            status="omitted",
                            task_digest=None,
                        )
                        continue
                descriptor = prepared[index]
                try:
                    result = invoke(descriptor)
                    status, _result_status = _batch_result_classification(result)
                    item = AutonomousBatchItem(
                        index=index,
                        status=status,
                        task_digest=descriptor["task_digest"],
                        result=result,
                    )
                except Exception as error:
                    error_class, failure_code = _batch_error_projection(error)
                    item = AutonomousBatchItem(
                        index=index,
                        status="failed",
                        task_digest=descriptor["task_digest"],
                        error_class=error_class,
                        failure_code=failure_code,
                    )
                with lock:
                    items[index] = item
                    if on_progress is not None and item.status == "succeeded":
                        on_progress(tuple(items))
                    if stop_on_error and item.status != "succeeded":
                        halted = True

        with ThreadPoolExecutor(max_workers=workers, thread_name_prefix="aurora-autonomous-batch") as pool:
            futures = [pool.submit(worker) for _ in range(workers)]
            for future in futures:
                future.result()
        normalized = tuple(
            item
            if item is not None
            else AutonomousBatchItem(
                index=index,
                status="failed",
                task_digest=None,
                error_class="AutonomousBatchError",
                failure_code="missing_batch_result",
            )
            for index, item in enumerate(items)
        )
        completed = sum(item.status == "succeeded" for item in normalized)
        failed = sum(item.status in {"failed", "refused"} for item in normalized)
        omitted = sum(item.status == "omitted" for item in normalized)
        status = "completed" if failed == 0 and omitted == 0 else "partial" if completed else "failed"
        return AutonomousBatchResult(
            status=status,
            items=normalized,
            completed_count=completed,
            failed_count=failed,
            omitted_count=omitted,
            max_parallelism=max_parallelism,
            stop_on_error=stop_on_error,
            batch_digest=_batch_digest(normalized),
        )

    def run_resumable_batch(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        job_id: str,
        mode: str = "domain",
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None = None,
        max_parallelism: int = 4,
        stop_on_error: bool = False,
        checkpoint: AutonomousBatchCheckpoint | Mapping[str, Any] | None = None,
        checkpoint_sink: Callable[[AutonomousBatchCheckpoint], Any] | None = None,
        rehydrate_result: Callable[[AutonomousBatchRehydrationContext], Any] | None = None,
    ) -> AutonomousBatchResult:
        """Run a restart-safe batch with caller-owned metadata checkpointing.

        ``mode`` selects ``run_batch``, ``run_auto_batch``, or ``run_cross_domain_batch`` semantics.
        A checkpoint skips only items previously proven successful; every skipped item is restored
        through ``rehydrate_result`` and its redacted item digest is verified before any new
        provider call. The task list, options, prompts, credentials, provider values, and raw
        results remain transient. The caller should persist each checkpoint atomically and pass
        the same options factory and credential session after a restart.
        """

        job_id = _identifier("autonomous batch job_id", job_id)
        if mode not in AUTONOMOUS_BATCH_MODES:
            raise BrainRunError("autonomous batch mode must be one of: domain, auto, cross_domain")
        max_parallelism, stop_on_error = self._batch_controls(max_parallelism, stop_on_error)
        if checkpoint_sink is not None and not callable(checkpoint_sink):
            raise BrainRunError("autonomous batch checkpoint_sink must be callable or None")
        if rehydrate_result is not None and not callable(rehydrate_result):
            raise BrainRunError("autonomous batch rehydrate_result must be callable or None")

        if mode == "domain":
            prepared, resolved_credentials = self._prepare_batch_invocations(
                requests,
                credentials=credentials,
                model_candidates=model_candidates,
                options_factory=options_factory,
                cross_domain=False,
            )

            def invoke(descriptor: Mapping[str, Any]) -> Any:
                return self._invoke_domain_batch_descriptor(
                    descriptor,
                    credentials=resolved_credentials,
                )
        elif mode == "auto":
            prepared, resolved_credentials = self._prepare_auto_batch_invocations(
                requests,
                credentials=credentials,
                model_candidates=model_candidates,
                options_factory=options_factory,
            )

            def invoke(descriptor: Mapping[str, Any]) -> Any:
                return self.run_auto(
                    task=descriptor["task"],
                    credentials=resolved_credentials,
                    model_candidates=descriptor["model_candidates"],
                    execution_id=descriptor["execution_id"],
                    **descriptor["options"],
                )
        else:
            prepared, resolved_credentials = self._prepare_batch_invocations(
                requests,
                credentials=credentials,
                model_candidates=model_candidates,
                options_factory=options_factory,
                cross_domain=True,
            )

            def invoke(descriptor: Mapping[str, Any]) -> Any:
                return self.run_cross_domain(
                    task=descriptor["task"],
                    subtasks=descriptor["subtasks"],
                    credentials=resolved_credentials,
                    model_candidates=descriptor["model_candidates"],
                    execution_id=descriptor["execution_id"],
                    **descriptor["options"],
                )

        request_digests = tuple(_batch_request_digest(descriptor, mode) for descriptor in prepared)
        semantic_routing_policy_digest = _batch_semantic_routing_policy_digest(prepared, mode)
        automatic_execution_policy_digest = _batch_automatic_execution_policy_digest(prepared, mode)
        batch_input_digest = content_digest({
            "schema": AUTONOMOUS_BATCH_CHECKPOINT_SCHEMA,
            "mode": mode,
            "request_digests": list(request_digests),
            **(
                {"semantic_routing_policy_digest": semantic_routing_policy_digest}
                if semantic_routing_policy_digest is not None
                else {}
            ),
            **(
                {"automatic_execution_policy_digest": automatic_execution_policy_digest}
                if automatic_execution_policy_digest is not None
                else {}
            ),
        })
        current_checkpoint: AutonomousBatchCheckpoint | None
        if checkpoint is None:
            current_checkpoint = None
        elif isinstance(checkpoint, AutonomousBatchCheckpoint):
            current_checkpoint = checkpoint
        elif isinstance(checkpoint, Mapping):
            current_checkpoint = AutonomousBatchCheckpoint.from_dict(checkpoint)
        else:
            raise BrainRunError("autonomous batch checkpoint must be a checkpoint, mapping, or None")
        if current_checkpoint is not None:
            if current_checkpoint.job_id != job_id:
                raise BrainRunError("autonomous batch checkpoint job_id does not match")
            if current_checkpoint.mode != mode:
                raise BrainRunError("autonomous batch checkpoint mode does not match")
            if current_checkpoint.request_digests != request_digests:
                raise BrainRunError("autonomous batch checkpoint requests do not match the current batch")
            if semantic_routing_policy_digest is not None and current_checkpoint.semantic_routing_policy_digest is None:
                raise BrainRunError("legacy autonomous batch checkpoint requires explicit semantic-routing policy rebinding")
            if current_checkpoint.semantic_routing_policy_digest != semantic_routing_policy_digest:
                raise BrainRunError("autonomous batch checkpoint semantic-routing policy does not match")
            if current_checkpoint.automatic_execution_policy_digest != automatic_execution_policy_digest:
                raise BrainRunError("autonomous batch checkpoint automatic execution policy does not match")
            if current_checkpoint.batch_input_digest != batch_input_digest:
                raise BrainRunError("autonomous batch checkpoint does not match the current execution policy")
            if current_checkpoint.max_parallelism != max_parallelism or current_checkpoint.stop_on_error != stop_on_error:
                raise BrainRunError("autonomous batch checkpoint execution controls do not match")
            if current_checkpoint.completed_indices and rehydrate_result is None:
                raise BrainRunError("resuming a batch requires rehydrate_result for completed items")

        items: list[AutonomousBatchItem | None] = [None] * len(prepared)
        if current_checkpoint is not None:
            for index, expected_result_digest in zip(
                current_checkpoint.completed_indices,
                current_checkpoint.completed_result_digests,
            ):
                descriptor = prepared[index]
                context = AutonomousBatchRehydrationContext(
                    job_id=job_id,
                    index=index,
                    mode=mode,
                    request_digest=request_digests[index],
                    task_digest=descriptor["task_digest"],
                    expected_result_digest=expected_result_digest,
                )
                try:
                    result = rehydrate_result(context) if rehydrate_result is not None else None
                except Exception as error:
                    raise BrainRunError(f"autonomous batch result rehydration failed for item {index}") from error
                status, _result_status = _batch_result_classification(result)
                if status != "succeeded":
                    raise BrainRunError(f"rehydrated autonomous batch item {index} is not successful")
                item = AutonomousBatchItem(
                    index=index,
                    status="succeeded",
                    task_digest=descriptor["task_digest"],
                    result=result,
                )
                if _batch_item_digest(item) != expected_result_digest:
                    raise BrainRunError(f"rehydrated autonomous batch item {index} does not match its checkpoint digest")
                items[index] = item

        def persist(status: str) -> None:
            if checkpoint_sink is None:
                return
            completed_items = [
                (index, item)
                for index, item in enumerate(items)
                if item is not None and item.status == "succeeded"
            ]
            value = AutonomousBatchCheckpoint(
                job_id=job_id,
                mode=mode,
                batch_input_digest=batch_input_digest,
                request_digests=request_digests,
                semantic_routing_policy_digest=semantic_routing_policy_digest,
                automatic_execution_policy_digest=automatic_execution_policy_digest,
                completed_indices=tuple(index for index, _item in completed_items),
                completed_result_digests=tuple(_batch_item_digest(item) for _index, item in completed_items),
                max_parallelism=max_parallelism,
                stop_on_error=stop_on_error,
                status=status,
            )
            checkpoint_sink(value)

        def persist_progress(snapshot: Sequence[AutonomousBatchItem | None]) -> None:
            items[:] = snapshot
            persist("running")

        persist("running")
        result = self._execute_prepared_batch(
            prepared,
            invoke=invoke,
            max_parallelism=max_parallelism,
            stop_on_error=stop_on_error,
            initial_items=items,
            on_progress=persist_progress,
        )
        persist("completed" if result.status == "completed" else "partial")
        return result

    def run_resumable_auto_batch_with_trace(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        job_id: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        trace_store: AutonomousRunTraceStore,
        run_id: str | None = None,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None = None,
        max_parallelism: int = 4,
        stop_on_error: bool = False,
        checkpoint: AutonomousBatchCheckpoint | Mapping[str, Any] | None = None,
        checkpoint_sink: Callable[[AutonomousBatchCheckpoint], Any] | None = None,
        rehydrate_result: Callable[[AutonomousBatchRehydrationContext], Any] | None = None,
    ) -> AutonomousTracedRunResult:
        """Resume an automatic batch with one trace spanning recovery and fresh execution.

        The generic resumable runner remains the authority for checkpoint digest validation and
        rehydration. This adapter only composes value-only trace callbacks around it, records
        settled item metadata after the runner classifies each item, and never serializes the
        restored result. Trace persistence therefore cannot bypass a checkpoint, provider
        approval, credential scope, or effect reconciliation boundary.
        """

        self._trace_store(trace_store)
        resolved_job_id = _identifier("autonomous traced batch job_id", job_id)
        if not isinstance(requests, Sequence) or isinstance(requests, (str, bytes)):
            raise BrainRunError("autonomous traced automatic batch requests must be a sequence")
        task_digests = [
            content_digest({"task": request.get("task") if isinstance(request, Mapping) else None})
            for request in requests
        ]
        resolved_run_id = run_id or f"trace-{uuid.uuid4().hex}"
        session = AutonomousRunTraceSession(
            trace_store,
            run_id=resolved_run_id,
            task_digest=content_digest({
                "schema": AUTONOMOUS_TRACED_AUTO_BATCH_SCHEMA,
                "mode": "auto",
                "task_digests": task_digests,
            }),
            domains=AUTONOMOUS_DOMAINS,
        )
        session.started(detail_digest=content_digest({
            "mode": "auto",
            "job_id": resolved_job_id,
            "item_count": len(task_digests),
            "task_digests": task_digests,
        }))
        rehydrated_indices: set[int] = set()

        def traced_factory(request: Mapping[str, Any], index: int) -> Mapping[str, Any]:
            raw_options = request.get("options", {}) if isinstance(request, Mapping) else {}
            if isinstance(raw_options, Mapping) and any(
                key in raw_options for key in ("invocation_observer", "trace_event_callback")
            ):
                raise BrainRunError(
                    "automatic traced batch options cannot override invocation_observer or trace_event_callback"
                )
            options = {} if options_factory is None else options_factory(request, index)
            if not isinstance(options, Mapping):
                raise BrainRunError("autonomous traced automatic batch options_factory must return a mapping")
            if any(key in options for key in ("invocation_observer", "trace_event_callback")):
                raise BrainRunError(
                    "automatic traced batch options cannot override invocation_observer or trace_event_callback"
                )
            return {
                **dict(options),
                "invocation_observer": session.provider_observer(),
                "trace_event_callback": session.record,
            }

        def traced_rehydrate(context: AutonomousBatchRehydrationContext) -> Any:
            if rehydrate_result is None:
                raise BrainRunError("resuming a traced automatic batch requires rehydrate_result")
            result = rehydrate_result(context)
            # The generic runner verifies the returned value and its result digest after this
            # callback returns. Record the rehydration only after that verification succeeds.
            rehydrated_indices.add(context.index)
            return result

        try:
            result = self.run_resumable_batch(
                requests,
                job_id=resolved_job_id,
                mode="auto",
                credentials=credentials,
                model_candidates=model_candidates,
                options_factory=traced_factory,
                max_parallelism=max_parallelism,
                stop_on_error=stop_on_error,
                checkpoint=checkpoint,
                checkpoint_sink=checkpoint_sink,
                rehydrate_result=traced_rehydrate if rehydrate_result is not None else None,
            )
            for item in result.items:
                metadata = self._trace_execution_metadata(item.result) if item.result is not None else {
                    "route_digest": None,
                    "plan_digest": None,
                    "selection_digest": None,
                }
                if item.result is not None:
                    trace_status = autonomous_run_trace_status(getattr(item.result, "status", "unknown"))
                    terminal_phase = (
                        "completed"
                        if trace_status in {"completed", "partial"}
                        else trace_status
                        if trace_status in {"paused", "refused", "failed"}
                        else "failed"
                    )
                else:
                    trace_status = "failed"
                    terminal_phase = "failed"
                session.record(
                    phase="plan_compiled",
                    status="running",
                    route_digest=metadata["route_digest"],
                    plan_digest=metadata["plan_digest"],
                    selection_digest=metadata["selection_digest"],
                    detail_digest=content_digest({
                        "index": item.index,
                        "task_digest": task_digests[item.index],
                        "state": "rehydrated" if item.index in rehydrated_indices else "settled",
                    }),
                )
                session.record(
                    phase=terminal_phase,
                    status=trace_status,
                    route_digest=metadata["route_digest"],
                    plan_digest=metadata["plan_digest"],
                    selection_digest=metadata["selection_digest"],
                    failure_class=item.error_class,
                    failure_code=item.failure_code,
                    detail_digest=content_digest({
                        "index": item.index,
                        "status": item.status,
                        "result_status": item.result_status,
                    }),
                )
            session.complete(
                status=autonomous_run_trace_status(result.status),
                detail_digest=content_digest({
                    "batch_digest": result.batch_digest,
                    "completed_count": result.completed_count,
                    "failed_count": result.failed_count,
                    "omitted_count": result.omitted_count,
                }),
            )
        except Exception as error:
            session.fail(
                failure_class=type(error).__name__,
                failure_code="execution_error",
                detail_digest=content_digest({"error_class": type(error).__name__}),
            )
            raise
        return AutonomousTracedRunResult(result=result, trace=session.summary())

    def run_resumable_auto_batch_with_launch_admission_and_trace(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        job_id: str,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        trace_store: AutonomousRunTraceStore,
        run_id: str | None = None,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None = None,
        max_parallelism: int = 4,
        stop_on_error: bool = False,
        checkpoint: AutonomousBatchCheckpoint | Mapping[str, Any] | None = None,
        checkpoint_sink: Callable[[AutonomousBatchCheckpoint], Any] | None = None,
        rehydrate_result: Callable[[AutonomousBatchRehydrationContext], Any] | None = None,
    ) -> AutonomousTracedRunResult:
        """Re-admit every current automatic route before traced checkpoint recovery or dispatch."""

        replay_factory = self._authorize_batch_launch_admission(
            requests,
            mode="auto",
            launch_admission=launch_admission,
            options_factory=options_factory,
        )
        return self.run_resumable_auto_batch_with_trace(
            requests,
            job_id=job_id,
            credentials=credentials,
            trace_store=trace_store,
            run_id=run_id,
            model_candidates=model_candidates,
            options_factory=replay_factory,
            max_parallelism=max_parallelism,
            stop_on_error=stop_on_error,
            checkpoint=checkpoint,
            checkpoint_sink=checkpoint_sink,
            rehydrate_result=rehydrate_result,
        )

    def _prepare_auto_batch_invocations(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None,
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None,
    ) -> tuple[tuple[dict[str, Any], ...], dict[str, CredentialHandle]]:
        if not isinstance(requests, Sequence) or isinstance(requests, (str, bytes)):
            raise BrainRunError("autonomous auto batch requests must be a sequence")
        if not 1 <= len(requests) <= MAX_AUTONOMOUS_AGENT_BATCH:
            raise BrainRunError(
                "autonomous auto batch requests must contain between 1 and "
                f"{MAX_AUTONOMOUS_AGENT_BATCH} entries"
            )
        if options_factory is not None and not callable(options_factory):
            raise BrainRunError("autonomous auto batch options_factory must be callable or None")
        resolved_credentials = self._credential_mapping(credentials)
        common_candidates = self._resolve_candidates(model_candidates)
        prepared: list[dict[str, Any]] = []
        reserved_options = {"credentials", "task", "model_candidates", "execution_id"}
        for index, raw in enumerate(requests):
            if not isinstance(raw, Mapping):
                raise BrainRunError(f"autonomous auto batch request {index} must be a mapping")
            if "credentials" in raw:
                raise BrainRunError(
                    "autonomous auto batch requests cannot carry credentials; pass one shared "
                    "opaque credential mapping or session"
                )
            task = _text(
                f"autonomous auto batch request {index} task",
                raw.get("task"),
                maximum=MAX_AUTONOMY_TEXT_BYTES,
            )
            raw_options = raw.get("options", {})
            if raw_options is None:
                raw_options = {}
            if not isinstance(raw_options, Mapping):
                raise BrainRunError(f"autonomous auto batch request {index} options must be a mapping")
            options = dict(raw_options)
            if options_factory is not None:
                try:
                    generated = options_factory(raw, index)
                except Exception as error:
                    raise BrainRunError(
                        f"autonomous auto batch options_factory failed for request {index}"
                    ) from error
                if not isinstance(generated, Mapping):
                    raise BrainRunError(
                        f"autonomous auto batch options_factory result {index} must be a mapping"
                    )
                options.update(generated)
            reserved = sorted(reserved_options.intersection(options))
            if reserved:
                raise BrainRunError(
                    f"autonomous auto batch request {index} options cannot override: {', '.join(reserved)}"
                )
            item_candidates = raw.get("model_candidates", common_candidates)
            if item_candidates is None:
                item_candidates = common_candidates
            prepared.append(
                {
                    "index": index,
                    "task": task,
                    "task_digest": content_digest({"task": task}),
                    "model_candidates": self._resolve_candidates(item_candidates),
                    "execution_id": raw.get("execution_id"),
                    "options": options,
                }
            )
        return tuple(prepared), resolved_credentials

    def run_batch(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None = None,
        max_parallelism: int = 4,
        stop_on_error: bool = False,
    ) -> AutonomousBatchResult:
        """Run bounded single-domain tasks across the shared provider/learning envelope.

        Each request contains ``task``, ``domain``, and optional ``options``,
        ``model_candidates``, and ``execution_id``. Credentials are deliberately shared at the
        method boundary and must be opaque handles or a live session; a request cannot smuggle a
        raw key or replace the credential mapping. All request shape and model-catalogue checks
        finish before the first provider call, while results remain caller-owned and transient.
        """

        max_parallelism, stop_on_error = self._batch_controls(max_parallelism, stop_on_error)
        prepared, resolved_credentials = self._prepare_batch_invocations(
            requests,
            credentials=credentials,
            model_candidates=model_candidates,
            options_factory=options_factory,
            cross_domain=False,
        )

        def invoke(descriptor: Mapping[str, Any]) -> Any:
            return self._invoke_domain_batch_descriptor(
                descriptor,
                credentials=resolved_credentials,
            )

        return self._execute_prepared_batch(
            prepared,
            invoke=invoke,
            max_parallelism=max_parallelism,
            stop_on_error=stop_on_error,
        )

    def run_auto_batch(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None = None,
        max_parallelism: int = 4,
        stop_on_error: bool = False,
    ) -> AutonomousBatchResult:
        """Route and execute bounded tasks without requiring callers to preselect a domain."""

        max_parallelism, stop_on_error = self._batch_controls(max_parallelism, stop_on_error)
        prepared, resolved_credentials = self._prepare_auto_batch_invocations(
            requests,
            credentials=credentials,
            model_candidates=model_candidates,
            options_factory=options_factory,
        )

        def invoke(descriptor: Mapping[str, Any]) -> Any:
            return self.run_auto(
                task=descriptor["task"],
                credentials=resolved_credentials,
                model_candidates=descriptor["model_candidates"],
                execution_id=descriptor["execution_id"],
                **descriptor["options"],
            )

        return self._execute_prepared_batch(
            prepared,
            invoke=invoke,
            max_parallelism=max_parallelism,
            stop_on_error=stop_on_error,
        )

    def run_auto_batch_with_trace(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        trace_store: AutonomousRunTraceStore,
        run_id: str | None = None,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None = None,
        max_parallelism: int = 4,
        stop_on_error: bool = False,
    ) -> AutonomousTracedRunResult:
        """Route and execute an automatic batch with one metadata-only lifecycle trace.

        The trace is deliberately shared by every item so operators can correlate routing,
        provider selection, failover, learning callbacks, refusals, omissions, and the aggregate
        terminal state.  The underlying batch still owns deterministic item ordering and bounded
        concurrency.  A trace request owns the invocation observer and callback slots; callers
        that need custom observers should compose them outside this high-level helper rather than
        silently replacing the audit stream.
        """

        self._trace_store(trace_store)
        max_parallelism, stop_on_error = self._batch_controls(max_parallelism, stop_on_error)
        prepared, resolved_credentials = self._prepare_auto_batch_invocations(
            requests,
            credentials=credentials,
            model_candidates=model_candidates,
            options_factory=options_factory,
        )
        resolved_run_id = run_id or f"trace-{uuid.uuid4().hex}"
        task_digest = content_digest({
            "schema": AUTONOMOUS_TRACED_AUTO_BATCH_SCHEMA,
            "mode": "auto",
            "task_digests": [descriptor["task_digest"] for descriptor in prepared],
        })
        session = AutonomousRunTraceSession(
            trace_store,
            run_id=resolved_run_id,
            task_digest=task_digest,
            domains=AUTONOMOUS_DOMAINS,
        )
        session.started(detail_digest=content_digest({
            "mode": "auto",
            "item_count": len(prepared),
            "task_digests": [descriptor["task_digest"] for descriptor in prepared],
        }))

        def invoke(descriptor: Mapping[str, Any]) -> Any:
            index = descriptor["index"]
            options = dict(descriptor["options"])
            if "invocation_observer" in options or "trace_event_callback" in options:
                raise BrainRunError(
                    "automatic traced batch options cannot override invocation_observer or trace_event_callback"
                )
            options["invocation_observer"] = session.provider_observer()
            options["trace_event_callback"] = session.record
            before_provider_events = len(
                trace_store.events({"run_id": resolved_run_id, "phase": "provider_invocation_finished"})
            )
            try:
                result = self.run_auto(
                    task=descriptor["task"],
                    credentials=resolved_credentials,
                    model_candidates=descriptor["model_candidates"],
                    execution_id=descriptor["execution_id"],
                    **options,
                )
                metadata = self._trace_execution_metadata(result)
                session.record(
                    phase="plan_compiled",
                    status="running",
                    route_digest=metadata["route_digest"],
                    plan_digest=metadata["plan_digest"],
                    selection_digest=metadata["selection_digest"],
                    detail_digest=content_digest({
                        "index": index,
                        "task_digest": descriptor["task_digest"],
                        "state": "prepared_and_executed",
                    }),
                )
                after_provider_events = len(
                    trace_store.events({"run_id": resolved_run_id, "phase": "provider_invocation_finished"})
                )
                if after_provider_events == before_provider_events:
                    session.record_provider_receipts(metadata["receipts"])
                trace_status = autonomous_run_trace_status(getattr(result, "status", "unknown"))
                terminal_phase = (
                    "completed"
                    if trace_status in {"completed", "partial"}
                    else trace_status
                    if trace_status in {"paused", "refused", "failed"}
                    else "failed"
                )
                session.record(
                    phase=terminal_phase,
                    status=trace_status,
                    route_digest=metadata["route_digest"],
                    plan_digest=metadata["plan_digest"],
                    selection_digest=metadata["selection_digest"],
                    detail_digest=content_digest({
                        "index": index,
                        "result_status": getattr(result, "status", "unknown"),
                    }),
                )
                return result
            except Exception as error:
                session.record(
                    phase="failed",
                    status="failed",
                    detail_digest=content_digest({
                        "index": index,
                        "error_class": type(error).__name__,
                        "failure_code": "execution_error",
                    }),
                )
                raise

        try:
            result = self._execute_prepared_batch(
                prepared,
                invoke=invoke,
                max_parallelism=max_parallelism,
                stop_on_error=stop_on_error,
            )
            session.complete(
                status=autonomous_run_trace_status(result.status),
                detail_digest=content_digest({
                    "batch_digest": result.batch_digest,
                    "completed_count": result.completed_count,
                    "failed_count": result.failed_count,
                    "omitted_count": result.omitted_count,
                }),
            )
        except Exception as error:
            session.fail(
                failure_class=type(error).__name__,
                failure_code="execution_error",
                detail_digest=content_digest({"error_class": type(error).__name__}),
            )
            raise
        return AutonomousTracedRunResult(result=result, trace=session.summary())

    def run_auto_batch_with_launch_admission_and_trace(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        trace_store: AutonomousRunTraceStore,
        run_id: str | None = None,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None = None,
        max_parallelism: int = 4,
        stop_on_error: bool = False,
    ) -> AutonomousTracedRunResult:
        """Run the traced automatic batch only after its complete route union is admitted."""

        replay_factory = self._authorize_batch_launch_admission(
            requests,
            mode="auto",
            launch_admission=launch_admission,
            options_factory=options_factory,
        )
        return self.run_auto_batch_with_trace(
            requests,
            credentials=credentials,
            trace_store=trace_store,
            run_id=run_id,
            model_candidates=model_candidates,
            options_factory=replay_factory,
            max_parallelism=max_parallelism,
            stop_on_error=stop_on_error,
        )

    def run_cross_domain_batch(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None = None,
        max_parallelism: int = 4,
        stop_on_error: bool = False,
    ) -> AutonomousBatchResult:
        """Run bounded fan-out/fan-in tasks with the same deterministic batch accounting."""

        max_parallelism, stop_on_error = self._batch_controls(max_parallelism, stop_on_error)
        prepared, resolved_credentials = self._prepare_batch_invocations(
            requests,
            credentials=credentials,
            model_candidates=model_candidates,
            options_factory=options_factory,
            cross_domain=True,
        )

        def invoke(descriptor: Mapping[str, Any]) -> Any:
            return self.run_cross_domain(
                task=descriptor["task"],
                subtasks=descriptor["subtasks"],
                credentials=resolved_credentials,
                model_candidates=descriptor["model_candidates"],
                execution_id=descriptor["execution_id"],
                **descriptor["options"],
            )

        return self._execute_prepared_batch(
            prepared,
            invoke=invoke,
            max_parallelism=max_parallelism,
            stop_on_error=stop_on_error,
        )

    def run_batch_with_launch_admission(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None = None,
        max_parallelism: int = 4,
        stop_on_error: bool = False,
    ) -> AutonomousBatchResult:
        """Run an explicit-domain batch only after one admission covers every item domain."""

        replay_factory = self._authorize_batch_launch_admission(
            requests,
            mode="domain",
            launch_admission=launch_admission,
            options_factory=options_factory,
        )
        return self.run_batch(
            requests,
            credentials=credentials,
            model_candidates=model_candidates,
            options_factory=replay_factory,
            max_parallelism=max_parallelism,
            stop_on_error=stop_on_error,
        )

    def run_auto_batch_with_launch_admission(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None = None,
        max_parallelism: int = 4,
        stop_on_error: bool = False,
    ) -> AutonomousBatchResult:
        """Route a batch provider-free, then require admission for the union of selected domains."""

        replay_factory = self._authorize_batch_launch_admission(
            requests,
            mode="auto",
            launch_admission=launch_admission,
            options_factory=options_factory,
        )
        return self.run_auto_batch(
            requests,
            credentials=credentials,
            model_candidates=model_candidates,
            options_factory=replay_factory,
            max_parallelism=max_parallelism,
            stop_on_error=stop_on_error,
        )

    def run_cross_domain_batch_with_launch_admission(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None = None,
        max_parallelism: int = 4,
        stop_on_error: bool = False,
    ) -> AutonomousBatchResult:
        """Run fan-out/fan-in batch work only when every specialist domain is admitted."""

        replay_factory = self._authorize_batch_launch_admission(
            requests,
            mode="cross_domain",
            launch_admission=launch_admission,
            options_factory=options_factory,
        )
        return self.run_cross_domain_batch(
            requests,
            credentials=credentials,
            model_candidates=model_candidates,
            options_factory=replay_factory,
            max_parallelism=max_parallelism,
            stop_on_error=stop_on_error,
        )

    def run_resumable_batch_with_launch_admission(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        job_id: str,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        mode: str = "domain",
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        options_factory: Callable[[Mapping[str, Any], int], Mapping[str, Any]] | None = None,
        max_parallelism: int = 4,
        stop_on_error: bool = False,
        checkpoint: AutonomousBatchCheckpoint | Mapping[str, Any] | None = None,
        checkpoint_sink: Callable[[AutonomousBatchCheckpoint], Any] | None = None,
        rehydrate_result: Callable[[AutonomousBatchRehydrationContext], Any] | None = None,
    ) -> AutonomousBatchResult:
        """Resume a batch only after re-reviewing its complete current route set.

        Admission is checked before checkpoint rehydration and credential resolution.  A restored
        successful item therefore cannot be used to skip a changed or newly under-scoped route.
        """

        replay_factory = self._authorize_batch_launch_admission(
            requests,
            mode=mode,
            launch_admission=launch_admission,
            options_factory=options_factory,
        )
        return self.run_resumable_batch(
            requests,
            job_id=job_id,
            mode=mode,
            credentials=credentials,
            model_candidates=model_candidates,
            options_factory=replay_factory,
            max_parallelism=max_parallelism,
            stop_on_error=stop_on_error,
            checkpoint=checkpoint,
            checkpoint_sink=checkpoint_sink,
            rehydrate_result=rehydrate_result,
        )

    def settle_cross_domain_trajectory_learning(
        self,
        *,
        cross_domain: AutonomousCrossDomainResult,
        bandit_state: Mapping[str, Any],
        evaluator: BrainOutcomeEvaluator,
        evidence: Mapping[str, Mapping[str, Any]] | None = None,
        memory: BrainEpisodicMemory | None = None,
        memory_tags: Sequence[str] = (),
        trajectory_id: str | None = None,
        trajectory_discount: float = 0.90,
        trajectory_terminal_reward: float | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> AutonomousCrossDomainTrajectoryLearningResult:
        """Apply delayed credit to already-completed durable cross-domain results."""

        return self.orchestrator.settle_cross_domain_trajectory_learning(
            cross_domain=cross_domain,
            bandit_state=bandit_state,
            evaluator=evaluator,
            evidence=evidence,
            memory=memory if memory is not None else self.memory,
            memory_tags=memory_tags,
            trajectory_id=trajectory_id,
            trajectory_discount=trajectory_discount,
            trajectory_terminal_reward=trajectory_terminal_reward,
            ledger=ledger,
        )

# Keep method introspection compatible with the public AutonomousAgent class.
for _name, _descriptor in vars(AutonomousAgentBatchMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, (staticmethod, classmethod)) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousAgent.{_name}"
