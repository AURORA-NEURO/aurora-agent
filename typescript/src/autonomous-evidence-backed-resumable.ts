import { ArgumentError, ProviderRuntimeError } from "./errors.js";
import { AutonomousAgent } from "./autonomous.js";
import type {
  AutonomousDomainName,
  AutonomousAutoRunOptions,
  AutonomousAutoRunResult,
  AutonomousEvidenceBackedRunOptions,
  AutonomousEvidenceBackedRunPreflight,
  AutonomousEvidenceBackedRunResult,
  AutonomousEvidenceBackedRunStatus,
  AutonomousCrossDomainRunResult,
  AutonomousPlanAndRunStatus,
  AutonomousProviderPlanningOptions,
  AutonomousPromptChunk,
  AutonomousReviewedEvidencePreparationOptions,
  AutonomousRunResult,
} from "./autonomous.js";
import { AutonomousEffectBoundary } from "./autonomous-effects.js";
import { AutonomousEvidenceAdapterRegistry } from "./autonomous-evidence-adapters.js";
import { AutonomousEvidenceFailoverPolicy } from "./autonomous-evidence-failover.js";
import { AutonomousEvidenceProviderContractRegistry } from "./autonomous-evidence-provider-contract.js";
import { AutonomousEvidenceReadinessPolicy } from "./autonomous-evidence-readiness.js";
import { AutonomousEvidenceRetryPolicy } from "./autonomous-evidence-retry.js";
import { AutonomousEvidenceSourcePolicy } from "./autonomous-evidence-source.js";
import type {
  AutonomousEvidenceExecutionOptions,
  AutonomousEvidenceExecutionPlan,
  AutonomousEvidenceExecutionPlanJSON,
  AutonomousEvidenceExecutionResult,
  AutonomousEvidenceExecutionResultJSON,
} from "./autonomous-evidence-execution.js";
import { AutonomousPromptRegistry } from "./autonomous-prompt-registry.js";
import {
  AutonomousRuntime,
  CredentialStore,
  LLMRuntime,
  internalProviderTransportDispatchBinding,
  type ProviderTransportDispatchContext,
} from "./llm.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Metadata-only restart boundary for one reviewed evidence-to-provider operation. */
export const AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA = "bioprism-typescript-autonomous-evidence-backed-checkpoint/0.3" as const;
export const AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA = "bioprism-typescript-autonomous-evidence-backed-resumable-result/0.1" as const;
export const AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCH_RECEIPT_SCHEMA = "bioprism-typescript-autonomous-evidence-backed-provider-dispatch-receipt/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES = 64_000;
export const MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCHES = 1_024;

export type AutonomousEvidenceBackedCheckpointStatus =
  | "evidence_review_required"
  | "evidence_blocked"
  | "evidence_incomplete"
  | "provider_pending"
  | "provider_in_flight"
  | "provider_reconciliation_required"
  | "completed";

export interface AutonomousEvidenceBackedCheckpointJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA;
  job_id: string;
  generation: number;
  previous_checkpoint_digest: string | null;
  task_digest: string;
  request_digest: string;
  run_policy_digest: string;
  evidence_plan_digest: string;
  execution_plan_digest: string;
  evidence_result_digest: string | null;
  prompt_projection_digest: string | null;
  provider_operation_digest: string | null;
  provider_dispatch_count: number;
  provider_dispatch_head_digest: string | null;
  provider_result_digest: string | null;
  provider_status: AutonomousPlanAndRunStatus | null;
  status: AutonomousEvidenceBackedCheckpointStatus;
  checkpoint_digest: string;
  retention: "metadata_only;task_requests_evidence_and_provider_payloads_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceBackedCheckpointStore {
  read(): Promise<AutonomousEvidenceBackedCheckpointJSON | null> | AutonomousEvidenceBackedCheckpointJSON | null;
  write(checkpoint: AutonomousEvidenceBackedCheckpointJSON): Promise<void> | void;
  /** Optional atomic fence; false means another worker committed after this controller restored. */
  writeIfUnchanged?(expectedCheckpointDigest: string | null, checkpoint: AutonomousEvidenceBackedCheckpointJSON): Promise<boolean> | boolean;
  /** Atomically retain the checkpoint and its privileged raw-key dispatch receipt. */
  writeDispatchIfUnchanged?(
    expectedCheckpointDigest: string | null,
    checkpoint: AutonomousEvidenceBackedCheckpointJSON,
    receipt: AutonomousEvidenceBackedProviderDispatchReceipt,
  ): Promise<boolean> | boolean;
}

/** Public metadata-only projection of one durably fenced provider transport. */
export interface AutonomousEvidenceBackedProviderDispatchReceiptProjection extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCH_RECEIPT_SCHEMA;
  job_id: string;
  provider_operation_digest: string;
  sequence: number;
  previous_receipt_digest: string | null;
  provider: string;
  model: string;
  kind: string;
  transport_attempt: number;
  request_digest: string;
  provider_idempotency_key_digest: string;
  receipt_digest: string;
  retention: "metadata_only;raw_provider_idempotency_key_private_to_dispatch_store";
  secret_material: "never_returned";
}

/**
 * Privileged dispatch-store input. The raw provider key lives in a private class field and is
 * available only through this explicit method; JSON serialization returns the public projection.
 */
export interface AutonomousEvidenceBackedProviderDispatchReceipt {
  readonly projection: AutonomousEvidenceBackedProviderDispatchReceiptProjection;
  providerIdempotencyKey(): string;
  toJSON(): AutonomousEvidenceBackedProviderDispatchReceiptProjection;
}

/** Dispatch persistence was not acknowledged exactly; callers must reload before continuing. */
export class AutonomousEvidenceBackedDispatchTransactionError extends ArgumentError {
  override readonly name = "AutonomousEvidenceBackedDispatchTransactionError";
  readonly reloadRequired = true;
  override readonly cause?: unknown;

  constructor(message: string, cause?: unknown) {
    super(message);
    this.cause = cause;
  }
}

export interface AutonomousEvidenceBackedCheckpointTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousEvidenceBackedTransactionalCheckpointTextStore extends AutonomousEvidenceBackedCheckpointTextStore {
  writeIfUnchanged(expectedCheckpointDigest: string | null, value: string): Promise<boolean> | boolean;
  /** Optional atomic storage for a checkpoint plus a protected private dispatch receipt. */
  writeDispatchIfUnchanged?(
    expectedCheckpointDigest: string | null,
    checkpointValue: string,
    privateReceiptValue: string,
  ): Promise<boolean> | boolean;
}

export interface AutonomousEvidenceBackedProviderRehydrationContext {
  checkpoint: AutonomousEvidenceBackedCheckpointJSON;
  /** Detached, value-only projections: a rehydrator can never mutate the active probe graph. */
  executionPlan: AutonomousEvidenceExecutionPlanJSON;
  evidence: AutonomousEvidenceExecutionResultJSON;
  promptContext: readonly AutonomousPromptChunk[];
  providerDispatchCount: number;
  providerDispatchHeadDigest: string | null;
}

export type AutonomousEvidenceBackedProviderRehydrator = (
  context: AutonomousEvidenceBackedProviderRehydrationContext,
) => AutonomousRunResult | null | Promise<AutonomousRunResult | null>;

export type AutonomousEvidenceBackedAutomaticRehydrator = (
  context: AutonomousEvidenceBackedProviderRehydrationContext,
) => AutonomousAutoRunResult | null | Promise<AutonomousAutoRunResult | null>;

export type AutonomousEvidenceBackedCrossDomainRehydrator = (
  context: AutonomousEvidenceBackedProviderRehydrationContext,
) => AutonomousCrossDomainRunResult | null | Promise<AutonomousCrossDomainRunResult | null>;

/** Stable caller-owned identity for a result-affecting callback used across restarts. */
export interface AutonomousEvidenceBackedResumableRoleIdentity extends JsonObject {
  id: string;
  version: string;
  config_digest?: string | null;
}

export interface AutonomousEvidenceBackedResumableProviderPolicyIdentity extends AutonomousEvidenceBackedResumableRoleIdentity {
  config_digest: string;
}

/**
 * Identities for callbacks whose captured configuration cannot be recovered from JavaScript.
 * A custom prompt builder, projector, or value rehydrator must have an explicit entry. A
 * value-rehydrator identity may be reserved before the callback is available, then repeated
 * after restart. Adapter manifests and evaluator id/version fields are also bound independently.
 */
export interface AutonomousEvidenceBackedResumablePolicyIdentity extends JsonObject {
  acquirer?: AutonomousEvidenceBackedResumableRoleIdentity;
  projector?: AutonomousEvidenceBackedResumableRoleIdentity;
  evaluator?: AutonomousEvidenceBackedResumableRoleIdentity;
  value_rehydrator?: AutonomousEvidenceBackedResumableRoleIdentity;
  prompt_builder?: AutonomousEvidenceBackedResumableRoleIdentity;
  /**
   * Aggregate identity for provider-affecting state that JavaScript cannot project by value:
   * callbacks, stores/controllers, prompt renderers, and agent-owned registries/runtimes. The
   * config digest must identify the complete immutable configuration restored by the caller.
   */
  provider_policy: AutonomousEvidenceBackedResumableProviderPolicyIdentity;
}

export interface AutonomousEvidenceBackedResumableExecutionOptions extends Omit<AutonomousEvidenceBackedRunOptions, "beforeProviderRun" | "beforeProviderDispatch" | "providerRunOverride" | "automaticRunOverride" | "crossDomainRunOverride"> {
  jobId: string;
  checkpoint?: AutonomousEvidenceBackedCheckpointJSON;
  checkpointSink: (checkpoint: AutonomousEvidenceBackedCheckpointJSON) => Promise<void> | void;
  /**
   * Atomic checkpoint commit. A fresh provider-capable dispatch requires this callback; false
   * means the expected head changed and the provider boundary remains closed.
   */
  checkpointCompareAndStore?: (
    expectedCheckpointDigest: string | null,
    checkpoint: AutonomousEvidenceBackedCheckpointJSON,
  ) => Promise<boolean> | boolean;
  /**
   * Atomic dispatch transaction. Literal true is required before every provider transport;
   * false or throw closes the boundary and requires a fresh store reload.
   */
  checkpointDispatchCompareAndStore?: (
    expectedCheckpointDigest: string | null,
    checkpoint: AutonomousEvidenceBackedCheckpointJSON,
    receipt: AutonomousEvidenceBackedProviderDispatchReceipt,
  ) => Promise<boolean> | boolean;
  /** Stable identities/config digests for arbitrary callbacks whose closures are caller-owned. */
  resumablePolicyIdentity: AutonomousEvidenceBackedResumablePolicyIdentity;
  /** Rehydrate a prior provider result by its caller-owned digest; returning null requires reconciliation. */
  rehydrateProviderRun?: AutonomousEvidenceBackedProviderRehydrator;
  /** Rehydrate a completed automatic envelope by its caller-owned checkpoint digest. */
  rehydrateAutomaticRun?: AutonomousEvidenceBackedAutomaticRehydrator;
  /** Rehydrate a completed cross-domain fan-out by its caller-owned checkpoint digest. */
  rehydrateCrossDomainRun?: AutonomousEvidenceBackedCrossDomainRehydrator;
  /** A restored provider_pending checkpoint dispatches only when this and approveProviderCall are true. */
  resumeProvider?: boolean;
}

export interface AutonomousEvidenceBackedResumableRunProjection extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA;
  status: AutonomousEvidenceBackedResumableStatus;
  job_id: string;
  checkpoint_digest: string;
  result_status: AutonomousEvidenceBackedRunStatus;
  provider_rehydrated: boolean;
  retention: "metadata_only;raw_evidence_and_provider_payloads_caller_owned";
  secret_material: "never_returned";
}

export type AutonomousEvidenceBackedResumableStatus =
  | AutonomousEvidenceBackedRunStatus
  | "provider_pending"
  | "provider_in_flight"
  | "provider_reconciliation_required";

export interface AutonomousEvidenceBackedResumableRun {
  schema: typeof AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA;
  status: AutonomousEvidenceBackedResumableStatus;
  job_id: string;
  result: AutonomousEvidenceBackedRunResult;
  checkpoint: AutonomousEvidenceBackedCheckpointJSON;
  provider_rehydrated: boolean;
  toJSON(): AutonomousEvidenceBackedResumableRunProjection;
}

export interface AutonomousEvidenceBackedControllerProjection extends JsonObject {
  schema: "bioprism-typescript-autonomous-evidence-backed-controller/0.1";
  status: "empty" | "restored" | "flushed" | "completed" | "provider_pending" | "provider_in_flight" | "provider_reconciliation_required" | "evidence_incomplete";
  job_id: string;
  checkpoint_digest: string | null;
  persisted: true;
  retention: "metadata_only_task_request_evidence_and_provider_payloads_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceBackedControllerRun {
  controller: AutonomousEvidenceBackedControllerProjection;
  run: AutonomousEvidenceBackedResumableRun;
}

export type AutonomousEvidenceBackedControllerRunOptions = Omit<
  AutonomousEvidenceBackedResumableExecutionOptions,
  "jobId" | "checkpoint" | "checkpointSink" | "checkpointCompareAndStore" | "checkpointDispatchCompareAndStore"
>;

const RETENTION = "metadata_only;task_requests_evidence_and_provider_payloads_caller_owned" as const;
const SECRET_MATERIAL = "never_returned" as const;
const DISPATCH_RECEIPT_RETENTION = "metadata_only;raw_provider_idempotency_key_private_to_dispatch_store" as const;
const nativeStructuredClone = globalThis.structuredClone;
const nativeObjectFreeze = Object.freeze;
const nativeObjectEntries = Object.entries;
const nativeObjectKeys = Object.keys;
const nativeObjectGetPrototypeOf = Object.getPrototypeOf;
const nativeObjectGetOwnPropertyDescriptor = Object.getOwnPropertyDescriptor;
const nativeObjectGetOwnPropertyDescriptors = Object.getOwnPropertyDescriptors;
const nativeObjectIsFrozen = Object.isFrozen;
const nativeObjectHasOwnProperty = Object.prototype.hasOwnProperty;
const nativeObjectPrototype = Object.prototype;
const nativeReflectOwnKeys = Reflect.ownKeys;
const nativeReflectApply = Reflect.apply;
const nativeArrayIsArray = Array.isArray;
const nativeMapPrototype = Map.prototype;
const nativeMapGet = Map.prototype.get;
const nativeMapSet = Map.prototype.set;
const nativeMapForEach = Map.prototype.forEach;
const nativePromiseThen = Promise.prototype.then;
const nativePromiseResolve = Promise.resolve.bind(Promise);

function captureCoreCallableFingerprint(prototype: object): readonly (readonly [string, Function])[] {
  return nativeObjectFreeze(nativeObjectEntries(nativeObjectGetOwnPropertyDescriptors(prototype))
    .filter(([key, descriptor]) => key !== "constructor" && "value" in descriptor && typeof descriptor.value === "function")
    .map(([key, descriptor]) => nativeObjectFreeze([key, descriptor.value as Function] as const)));
}

const CORE_AGENT_CALLABLES = captureCoreCallableFingerprint(AutonomousAgent.prototype);
const CORE_LLM_CALLABLES = captureCoreCallableFingerprint(LLMRuntime.prototype);
const CORE_CREDENTIAL_STORE_CALLABLES = captureCoreCallableFingerprint(CredentialStore.prototype);
const CORE_AUTONOMOUS_RUNTIME_CALLABLES = captureCoreCallableFingerprint(AutonomousRuntime.prototype);
const CORE_EFFECT_BOUNDARY_CALLABLES = captureCoreCallableFingerprint(AutonomousEffectBoundary.prototype);

interface ResumableProviderTransportGraph {
  readonly llm: LLMRuntime;
  readonly fetchImplementation: Function;
  readonly credentials: object;
  readonly credentialEntries: Map<unknown, unknown>;
  readonly credentialClock: Function;
  readonly providerQuota: unknown;
  readonly clock: Function;
  readonly providers: Map<unknown, unknown>;
  readonly providerEntries: readonly (readonly [unknown, unknown])[];
}

function ownDataProperty(instance: object, key: string, name: string): unknown {
  const descriptor = nativeObjectGetOwnPropertyDescriptor(instance, key);
  if (!descriptor || !("value" in descriptor)) throw new ArgumentError(`${name} must be an own data property`);
  return descriptor.value;
}

function exactProviderMapEntries(providers: Map<unknown, unknown>): readonly (readonly [unknown, unknown])[] {
  if (nativeObjectGetPrototypeOf(providers) !== nativeMapPrototype || nativeReflectOwnKeys(providers).length !== 0) {
    throw new ArgumentError("evidence-backed resumable execution requires an unshadowed built-in provider registry");
  }
  const entries: (readonly [unknown, unknown])[] = [];
  nativeReflectApply(nativeMapForEach, providers, [(config: unknown, provider: unknown) => {
    entries[entries.length] = [provider, config] as const;
  }]);
  return entries;
}

function captureResumableProviderTransportGraph(agent: AutonomousAgent): ResumableProviderTransportGraph {
  const llm = ownDataProperty(agent, "llm", "AutonomousAgent.llm") as LLMRuntime;
  const fetchImplementation = ownDataProperty(llm, "fetchImplementation", "LLMRuntime.fetchImplementation") as Function;
  const credentials = ownDataProperty(llm, "credentials", "LLMRuntime.credentials") as CredentialStore;
  const providerQuota = ownDataProperty(llm, "providerQuota", "LLMRuntime.providerQuota");
  const clock = ownDataProperty(llm, "clock", "LLMRuntime.clock") as Function;
  const providers = ownDataProperty(llm, "providers", "LLMRuntime.providers") as Map<unknown, unknown>;
  if (nativeObjectGetPrototypeOf(credentials) !== CredentialStore.prototype) {
    throw new ArgumentError("evidence-backed resumable execution requires an exact built-in CredentialStore");
  }
  assertUnshadowedCoreMethods(credentials, CredentialStore.prototype, CORE_CREDENTIAL_STORE_CALLABLES, "CredentialStore");
  const credentialEntries = ownDataProperty(credentials, "entries", "CredentialStore.entries") as Map<unknown, unknown>;
  const credentialClock = ownDataProperty(credentials, "clock", "CredentialStore.clock") as Function;
  if (typeof fetchImplementation !== "function" || typeof clock !== "function"
      || nativeObjectGetPrototypeOf(credentialEntries as object) !== nativeMapPrototype || nativeReflectOwnKeys(credentialEntries as object).length !== 0
      || typeof credentialClock !== "function" || nativeObjectGetPrototypeOf(providers as object) !== nativeMapPrototype) {
    throw new ArgumentError("evidence-backed resumable execution requires a concrete built-in provider transport graph");
  }
  const providerEntries = nativeObjectFreeze(exactProviderMapEntries(providers).map(([provider, config]) => {
    if (typeof provider !== "string" || !isObject(config) || nativeObjectGetPrototypeOf(config) !== nativeObjectPrototype || !nativeObjectIsFrozen(config)) {
      throw new ArgumentError("evidence-backed resumable execution requires immutable normalized provider registrations");
    }
    const transport = nativeObjectGetOwnPropertyDescriptor(config, "transport")?.value;
    if (transport !== undefined && (!isObject(transport) || nativeObjectGetPrototypeOf(transport) !== nativeObjectPrototype || !nativeObjectIsFrozen(transport))) {
      throw new ArgumentError("evidence-backed resumable execution requires an immutable local provider transport");
    }
    return nativeObjectFreeze([provider, config] as const);
  }));
  return nativeObjectFreeze({
    llm,
    fetchImplementation,
    credentials,
    credentialEntries,
    credentialClock,
    providerQuota,
    clock,
    providers,
    providerEntries,
  });
}

function assertResumableProviderTransportGraph(
  agent: AutonomousAgent,
  expected: ResumableProviderTransportGraph,
): void {
  assertExactResumableCore(agent);
  const llm = ownDataProperty(agent, "llm", "AutonomousAgent.llm");
  if (nativeObjectGetPrototypeOf(expected.credentials) !== CredentialStore.prototype) {
    throw new ArgumentError("evidence-backed resumable credential store changed after its policy snapshot");
  }
  assertUnshadowedCoreMethods(expected.credentials, CredentialStore.prototype, CORE_CREDENTIAL_STORE_CALLABLES, "CredentialStore");
  if (llm !== expected.llm
      || ownDataProperty(expected.llm, "fetchImplementation", "LLMRuntime.fetchImplementation") !== expected.fetchImplementation
      || ownDataProperty(expected.llm, "credentials", "LLMRuntime.credentials") !== expected.credentials
      || ownDataProperty(expected.credentials, "entries", "CredentialStore.entries") !== expected.credentialEntries
      || ownDataProperty(expected.credentials, "clock", "CredentialStore.clock") !== expected.credentialClock
      || ownDataProperty(expected.llm, "providerQuota", "LLMRuntime.providerQuota") !== expected.providerQuota
      || ownDataProperty(expected.llm, "clock", "LLMRuntime.clock") !== expected.clock
      || ownDataProperty(expected.llm, "providers", "LLMRuntime.providers") !== expected.providers) {
    throw new ArgumentError("evidence-backed resumable provider transport graph changed after its policy snapshot");
  }
  const currentEntries = exactProviderMapEntries(expected.providers);
  if (currentEntries.length !== expected.providerEntries.length) {
    throw new ArgumentError("evidence-backed resumable provider registry changed after its policy snapshot");
  }
  for (let index = 0; index < currentEntries.length; index += 1) {
    if (currentEntries[index]?.[0] !== expected.providerEntries[index]?.[0]
        || currentEntries[index]?.[1] !== expected.providerEntries[index]?.[1]) {
      throw new ArgumentError("evidence-backed resumable provider registry changed after its policy snapshot");
    }
  }
}

function assertResumableProviderDispatchBinding(
  dispatch: ProviderTransportDispatchContext,
  expected: ResumableProviderTransportGraph,
): void {
  const binding = internalProviderTransportDispatchBinding(dispatch);
  let expectedEntry: readonly [unknown, unknown] | undefined;
  for (const entry of expected.providerEntries) {
    if (entry[0] === dispatch.provider) {
      expectedEntry = entry;
      break;
    }
  }
  const expectedConfig = expectedEntry?.[1];
  const expectedLocalTransport = isObject(expectedConfig)
    ? nativeObjectGetOwnPropertyDescriptor(expectedConfig, "transport")?.value ?? null
    : null;
  if (binding === null
      || expectedConfig === undefined
      || binding.providerConfig !== expectedConfig
      || binding.fetchImplementation !== expected.fetchImplementation
      || binding.localTransport !== expectedLocalTransport
      || binding.credentialStore !== expected.credentials
      || typeof binding.credentialBindingProbe !== "function"
      || binding.credentialBindingProbe() !== true) {
    throw new ArgumentError("selected provider transport binding does not match the snapshotted registry and fetch implementation");
  }
}

function assertUnshadowedCoreMethods(
  instance: object,
  prototype: object,
  methods: readonly (readonly [string, Function])[],
  name: string,
): void {
  const currentCallableKeys: string[] = [];
  for (const [key, descriptor] of nativeObjectEntries(nativeObjectGetOwnPropertyDescriptors(prototype))) {
    if (key !== "constructor" && "value" in descriptor && typeof descriptor.value === "function") {
      currentCallableKeys[currentCallableKeys.length] = key;
    }
  }
  if (currentCallableKeys.length !== methods.length) {
    throw new ArgumentError(`${name} core callable provider surface was replaced`);
  }
  for (let index = 0; index < currentCallableKeys.length; index += 1) {
    if (currentCallableKeys[index] !== methods[index]?.[0]) {
      throw new ArgumentError(`${name} core callable provider surface was replaced`);
    }
  }
  for (const [key, expected] of methods) {
    if (nativeReflectApply(nativeObjectHasOwnProperty, instance, [key])) throw new ArgumentError(`${name} has an instance-shadowed ${key} provider path`);
    const descriptor = nativeObjectGetOwnPropertyDescriptor(prototype, key);
    if (!descriptor || !("value" in descriptor) || descriptor.value !== expected) throw new ArgumentError(`${name} core ${key} provider path was replaced`);
  }
}

function assertExactEffectBoundary(value: unknown, name: string): void {
  if (value === undefined) return;
  if (typeof value !== "object" || value === null || nativeObjectGetPrototypeOf(value) !== AutonomousEffectBoundary.prototype) {
    throw new ArgumentError(`${name} must be an exact built-in AutonomousEffectBoundary without overrides`);
  }
  assertUnshadowedCoreMethods(value, AutonomousEffectBoundary.prototype, CORE_EFFECT_BOUNDARY_CALLABLES, name);
}

function assertExactResumableCore(agent: unknown): asserts agent is AutonomousAgent {
  if (typeof agent !== "object" || agent === null || nativeObjectGetPrototypeOf(agent) !== AutonomousAgent.prototype) {
    throw new ArgumentError("evidence-backed resumable execution requires an exact built-in AutonomousAgent");
  }
  assertUnshadowedCoreMethods(agent, AutonomousAgent.prototype, CORE_AGENT_CALLABLES, "AutonomousAgent");
  const llmDescriptor = nativeObjectGetOwnPropertyDescriptor(agent, "llm");
  const runtimeDescriptor = nativeObjectGetOwnPropertyDescriptor(agent, "runtime");
  if (!llmDescriptor || !("value" in llmDescriptor) || !runtimeDescriptor || !("value" in runtimeDescriptor)) {
    throw new ArgumentError("evidence-backed resumable execution requires data-bound core runtimes");
  }
  const llm = llmDescriptor.value;
  const runtime = runtimeDescriptor.value;
  if (typeof llm !== "object" || llm === null || nativeObjectGetPrototypeOf(llm) !== LLMRuntime.prototype) {
    throw new ArgumentError("evidence-backed resumable execution requires an exact built-in LLMRuntime without provider-path overrides");
  }
  assertUnshadowedCoreMethods(llm, LLMRuntime.prototype, CORE_LLM_CALLABLES, "LLMRuntime");
  if (typeof runtime !== "object" || runtime === null || nativeObjectGetPrototypeOf(runtime) !== AutonomousRuntime.prototype) {
    throw new ArgumentError("evidence-backed resumable execution requires an exact built-in AutonomousRuntime without provider-path overrides");
  }
  const runtimeLlmDescriptor = nativeObjectGetOwnPropertyDescriptor(runtime, "llm");
  if (!runtimeLlmDescriptor || !("value" in runtimeLlmDescriptor) || runtimeLlmDescriptor.value !== llm) {
    throw new ArgumentError("evidence-backed resumable execution requires AutonomousRuntime to share the exact validated LLMRuntime");
  }
  assertUnshadowedCoreMethods(runtime, AutonomousRuntime.prototype, CORE_AUTONOMOUS_RUNTIME_CALLABLES, "AutonomousRuntime");
  if (nativeReflectApply(nativeObjectHasOwnProperty, llm, ["effectBoundary"])) throw new ArgumentError("evidence-backed resumable execution refuses a shadowed LLM effectBoundary accessor");
  const llmEffectDescriptor = nativeObjectGetOwnPropertyDescriptor(llm, "effectBoundaryValue");
  const agentEffectDescriptor = nativeObjectGetOwnPropertyDescriptor(agent, "effectBoundary");
  if (!llmEffectDescriptor || !("value" in llmEffectDescriptor) || !agentEffectDescriptor || !("value" in agentEffectDescriptor)) {
    throw new ArgumentError("evidence-backed resumable execution requires data-bound core effect boundaries");
  }
  const llmEffectBoundary = llmEffectDescriptor.value;
  const agentEffectBoundary = agentEffectDescriptor.value;
  if (agentEffectBoundary !== undefined && llmEffectBoundary !== agentEffectBoundary) {
    throw new ArgumentError("evidence-backed resumable execution requires one identical agent/runtime effect boundary");
  }
  assertExactEffectBoundary(llmEffectBoundary, "runtime-bound resumable effectBoundary");
  assertExactEffectBoundary(agentEffectBoundary, "agent-bound resumable effectBoundary");
}
const RESULT_RETENTION = "metadata_only;raw_evidence_and_provider_payloads_caller_owned" as const;
const CONTROLLER_RETENTION = "metadata_only_task_request_evidence_and_provider_payloads_caller_owned" as const;

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !nativeArrayIsArray(value);
}

function boundedIdentifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || value.includes("\u0000") || !/^[A-Za-z0-9_.:+-]+$/.test(value)) throw new ArgumentError(`${name} is outside its bounded identifier contract`);
  return value;
}

function boundedDispatchText(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string" || !value.trim() || value.length > maximum || value.includes("\u0000")) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function optionalDigest(name: string, value: unknown): string | null {
  if (value === null || value === undefined) return null;
  return digest(name, value);
}

function allowedKeys(value: Record<string, unknown>, allowed: readonly string[], name: string): void {
  void snapshotControlEnvelope(value, allowed, name);
}

function snapshotControlEnvelope(
  value: Record<string, unknown>,
  allowed: readonly string[],
  name: string,
): Record<string, unknown> {
  const prototype = nativeObjectGetPrototypeOf(value);
  if (prototype !== nativeObjectPrototype && prototype !== null) throw new ArgumentError(`${name} must be a plain data object`);
  const descriptors = nativeObjectGetOwnPropertyDescriptors(value);
  const set = new Set(allowed);
  const snapshot: Record<string, unknown> = {};
  for (const key of nativeReflectOwnKeys(descriptors)) {
    if (typeof key !== "string" || !set.has(key)) throw new ArgumentError(`${name} contains unsupported fields`);
    const descriptor = descriptors[key]!;
    if (!descriptor.enumerable || !("value" in descriptor)) throw new ArgumentError(`${name} must contain enumerable data properties only`);
    snapshot[key] = descriptor.value;
  }
  return snapshot;
}

function exactKeys(value: Record<string, unknown>, expected: readonly string[], name: string): void {
  const actual = nativeObjectKeys(value);
  const set = new Set(expected);
  if (actual.length !== expected.length || actual.some((key) => !set.has(key))) throw new ArgumentError(`${name} fields are invalid`);
}

function generation(value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 1) throw new ArgumentError("evidence-backed checkpoint generation is outside its bounded contract");
  return value as number;
}

function providerDispatchCount(value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCHES) throw new ArgumentError("evidence-backed checkpoint provider_dispatch_count is outside its bounded contract");
  return value as number;
}

async function requestDigest(requests: AutonomousEvidenceBackedResumableExecutionOptions["requests"]): Promise<string> {
  return digestJson({ schema: AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA, requests });
}

function roleIdentity(
  name: "acquirer" | "projector" | "evaluator" | "value_rehydrator" | "prompt_builder" | "provider_policy",
  value: AutonomousEvidenceBackedResumableRoleIdentity | undefined,
): { id: string; version: string; config_digest: string | null } | null {
  if (value === undefined) return null;
  if (!isObject(value)) throw new ArgumentError(`resumable policy ${name} identity is malformed`);
  const snapshot = snapshotControlEnvelope(value, ["id", "version", "config_digest"], `resumable policy ${name} identity`);
  return {
    id: boundedIdentifier(`resumable policy ${name} id`, snapshot.id),
    version: boundedIdentifier(`resumable policy ${name} version`, snapshot.version),
    config_digest: optionalDigest(`resumable policy ${name} config_digest`, snapshot.config_digest),
  };
}

function resumablePolicyIdentities(options: AutonomousEvidenceBackedResumableExecutionOptions): {
  acquirer: { id: string; version: string; config_digest: string | null } | null;
  projector: { id: string; version: string; config_digest: string | null } | null;
  evaluator: { id: string; version: string; config_digest: string | null } | null;
  value_rehydrator: { id: string; version: string; config_digest: string | null } | null;
  prompt_builder: { id: string; version: string; config_digest: string | null } | null;
  provider_policy: { id: string; version: string; config_digest: string };
} {
  const raw = options.resumablePolicyIdentity;
  let identitySnapshot: AutonomousEvidenceBackedResumablePolicyIdentity | undefined;
  if (raw !== undefined) {
    if (!isObject(raw)) throw new ArgumentError("resumablePolicyIdentity is malformed");
    identitySnapshot = snapshotControlEnvelope(raw, ["acquirer", "projector", "evaluator", "value_rehydrator", "prompt_builder", "provider_policy"], "resumablePolicyIdentity") as unknown as AutonomousEvidenceBackedResumablePolicyIdentity;
  }
  const acquirer = roleIdentity("acquirer", identitySnapshot?.acquirer);
  const projector = roleIdentity("projector", identitySnapshot?.projector);
  const explicitEvaluator = roleIdentity("evaluator", identitySnapshot?.evaluator);
  const valueRehydrator = roleIdentity("value_rehydrator", identitySnapshot?.value_rehydrator);
  const promptBuilder = roleIdentity("prompt_builder", identitySnapshot?.prompt_builder);
  const providerPolicy = roleIdentity("provider_policy", identitySnapshot?.provider_policy);
  if (providerPolicy === null || providerPolicy.config_digest === null) {
    throw new ArgumentError("resumable evidence-backed execution requires provider_policy identity with config_digest for caller-owned and agent-owned provider state");
  }
  if (options.promptBuilder !== undefined && promptBuilder === null) {
    throw new ArgumentError("resumable evidence-backed execution requires prompt_builder identity for a custom promptBuilder");
  }
  if (options.promptBuilder === undefined && promptBuilder !== null) {
    throw new ArgumentError("resumable prompt_builder identity requires a custom promptBuilder");
  }
  if (options.execute?.projector !== undefined && projector === null) {
    throw new ArgumentError("resumable evidence-backed execution requires projector identity for a custom projector");
  }
  if (options.execute?.projector === undefined && projector !== null) {
    throw new ArgumentError("resumable projector identity requires a custom projector");
  }
  if (options.execute?.rehydrateValue !== undefined && valueRehydrator === null) {
    throw new ArgumentError("resumable evidence-backed execution requires value_rehydrator identity for a custom rehydrateValue callback");
  }
  const configuredEvaluator = options.execute?.evaluator;
  if (configuredEvaluator === undefined && explicitEvaluator !== null) {
    throw new ArgumentError("resumable evaluator identity requires a configured evaluator");
  }
  if (configuredEvaluator !== undefined && explicitEvaluator !== null &&
      (explicitEvaluator.id !== configuredEvaluator.evaluator_id || explicitEvaluator.version !== configuredEvaluator.evaluator_version)) {
    throw new ArgumentError("resumable evaluator identity does not match the configured evaluator");
  }
  const evaluator = configuredEvaluator === undefined ? null : {
    id: boundedIdentifier("resumable evaluator id", configuredEvaluator.evaluator_id),
    version: boundedIdentifier("resumable evaluator version", configuredEvaluator.evaluator_version),
    config_digest: explicitEvaluator?.config_digest ?? null,
  };
  return {
    acquirer,
    projector,
    evaluator,
    value_rehydrator: valueRehydrator,
    prompt_builder: promptBuilder,
    provider_policy: { ...providerPolicy, config_digest: providerPolicy.config_digest },
  };
}

const AUTONOMOUS_EVIDENCE_BACKED_RUN_OPTION_KEYS = [
  "registry",
  "domains",
  "requests",
  "availableEvidence",
  "completedStages",
  "prepare",
  "execute",
  "runMode",
  "run",
  "crossDomain",
  "promptBuilder",
  "beforeProviderRun",
  "beforeProviderDispatch",
  "providerRunOverride",
  "automaticRunOverride",
  "crossDomainRunOverride",
  "allowIncompleteEvidence",
  "evidenceCheckpointStore",
  "evidenceJobId",
  "evidenceReconciliationAuthority",
  "evidenceExecutionPolicyIdentity",
  "evidenceReconciliationReceipt",
  "evidenceResumeAfterReconciliation",
] as const satisfies readonly (keyof AutonomousEvidenceBackedRunOptions)[];

type MissingEvidenceBackedRunOptionKey = Exclude<
  keyof AutonomousEvidenceBackedRunOptions,
  typeof AUTONOMOUS_EVIDENCE_BACKED_RUN_OPTION_KEYS[number]
>;
const AUTONOMOUS_EVIDENCE_BACKED_RUN_OPTION_KEYS_ARE_EXHAUSTIVE: MissingEvidenceBackedRunOptionKey extends never ? true : never = true;

const RESUMABLE_EXECUTION_OPTION_KEYS = [
  "registry",
  "domains",
  "requests",
  "availableEvidence",
  "completedStages",
  "prepare",
  "execute",
  "runMode",
  "run",
  "crossDomain",
  "promptBuilder",
  "allowIncompleteEvidence",
  "evidenceCheckpointStore",
  "evidenceJobId",
  "evidenceReconciliationAuthority",
  "evidenceExecutionPolicyIdentity",
  "evidenceReconciliationReceipt",
  "evidenceResumeAfterReconciliation",
  "jobId",
  "checkpoint",
  "checkpointSink",
  "checkpointCompareAndStore",
  "checkpointDispatchCompareAndStore",
  "resumablePolicyIdentity",
  "rehydrateProviderRun",
  "rehydrateAutomaticRun",
  "rehydrateCrossDomainRun",
  "resumeProvider",
] as const satisfies readonly (keyof AutonomousEvidenceBackedResumableExecutionOptions)[];

type MissingResumableExecutionOptionKey = Exclude<
  keyof AutonomousEvidenceBackedResumableExecutionOptions,
  typeof RESUMABLE_EXECUTION_OPTION_KEYS[number]
>;
const RESUMABLE_EXECUTION_OPTION_KEYS_ARE_EXHAUSTIVE: MissingResumableExecutionOptionKey extends never ? true : never = true;
const CONTROLLER_MANAGED_RESUMABLE_OPTION_KEYS = new Set(["jobId", "checkpoint", "checkpointSink", "checkpointCompareAndStore", "checkpointDispatchCompareAndStore"]);
const CONTROLLER_RUN_OPTION_KEYS = RESUMABLE_EXECUTION_OPTION_KEYS.filter((key) => !CONTROLLER_MANAGED_RESUMABLE_OPTION_KEYS.has(key));

const RESUMABLE_RUN_POLICY_KEYS = [
  "domain",
  "workflowContext",
  "routeOverride",
  "semanticRouting",
  "capability",
  "candidates",
  "credential",
  "credentialFor",
  "authorizationContext",
  "context",
  "promptTemplate",
  "promptRegistry",
  "promptSelection",
  "promptLearningState",
  "promptLearningExploration",
  "promptStage",
  "contentParts",
  "memoryStore",
  "memoryQuery",
  "memoryRecall",
  "memoryLimit",
  "memoryTags",
  "memoryRunId",
  "recordMemory",
  "retrieveMemory",
  "memoryLesson",
  "memoryConsolidator",
  "memoryLessonResolver",
  "memoryLessonContextResolver",
  "consolidatedMemoryLimit",
  "retrieveConsolidatedMemory",
  "consolidatedMemoryRequired",
  "learning",
  "learningEpisodeId",
  "hints",
  "minConfidence",
  "minMargin",
  "maxDomains",
  "allowCrossDomain",
  "maxInputTokens",
  "contextBudget",
  "maxOutputTokens",
  "maxCostPerMillionTokens",
  "maxLatencyMs",
  "minQuality",
  "minSelectionConfidence",
  "selectionWeights",
  "selectionObservations",
  "maxTotalCostUnits",
  "costBudget",
  "requireJson",
  "responseSchema",
  "structuredDomainResponse",
  "requireStructuredResponseReview",
  "temperature",
  "tools",
  "authorizeAndExecute",
  "toolReadOnly",
  "approveProviderCall",
  "approveEffects",
  "execution",
  "effectBoundary",
  "acceptedCrossDomainPlanRefinement",
  "acceptedSingleDomainPlanRefinement",
  "executionAttempt",
  "providerIdempotencyKey",
  "maxProviderFailovers",
  "executionLifecycle",
  "signal",
  "observer",
  "providerDispatchFence",
  "selectionEventCallback",
  "toolSelectionState",
  "toolSelectionExploration",
  "maxToolRiskClass",
  "domainPolicyMode",
  "domainPolicyEvidenceReady",
  "domainPolicyEvaluatorConfigured",
  "domainPolicyPlanAccepted",
  "domainPolicyEffectsRequested",
  "domainPolicyEffectsApproved",
  "maxToolTurns",
  "planning",
  "subtasks",
  "planningPromptStage",
  "planningPromptLearningState",
  "planningPromptLearningExploration",
  "acceptPlan",
  "planningMode",
] as const satisfies readonly (keyof AutonomousAutoRunOptions)[];

type MissingResumableRunPolicyKey = Exclude<keyof AutonomousAutoRunOptions, typeof RESUMABLE_RUN_POLICY_KEYS[number]>;
const RESUMABLE_RUN_POLICY_KEYS_ARE_EXHAUSTIVE: MissingResumableRunPolicyKey extends never ? true : never = true;

const RESUMABLE_PLANNING_POLICY_KEYS = [
  "candidates",
  "credential",
  "credentialFor",
  "context",
  "promptTemplate",
  "promptRegistry",
  "promptSelection",
  "promptLearningState",
  "promptLearningExploration",
  "promptStage",
  "maxInputTokens",
  "maxOutputTokens",
  "maxCostPerMillionTokens",
  "maxLatencyMs",
  "minQuality",
  "minSelectionConfidence",
  "selectionWeights",
  "selectionObservations",
  "maxTotalCostUnits",
  "costBudget",
  "approveProviderCall",
  "runId",
  "temperature",
  "execution",
  "executionAttempt",
  "maxProviderFailovers",
  "signal",
  "observer",
  "providerDispatchFence",
  "selectionEventCallback",
  "domainPolicyMode",
  "domainPolicyEvidenceReady",
  "domainPolicyEvaluatorConfigured",
  "domainPolicyEffectsRequested",
  "domainPolicyEffectsApproved",
  "authorizationContext",
] as const satisfies readonly (keyof AutonomousProviderPlanningOptions)[];

type MissingResumablePlanningPolicyKey = Exclude<keyof AutonomousProviderPlanningOptions, typeof RESUMABLE_PLANNING_POLICY_KEYS[number]>;
const RESUMABLE_PLANNING_POLICY_KEYS_ARE_EXHAUSTIVE: MissingResumablePlanningPolicyKey extends never ? true : never = true;

type ResumableCrossDomainPolicy = NonNullable<AutonomousEvidenceBackedRunOptions["crossDomain"]>;
const RESUMABLE_CROSS_DOMAIN_POLICY_KEYS = [
  "subtasks",
  "allowPartial",
  "synthesize",
  "maxParallelChildren",
  "responseAlignments",
  "requireResponseAlignment",
  "minimumResponseReward",
  "minimumResponseAlignmentConfidence",
  "responseContradictionConfidenceThreshold",
] as const satisfies readonly (keyof ResumableCrossDomainPolicy)[];

type MissingResumableCrossDomainPolicyKey = Exclude<keyof ResumableCrossDomainPolicy, typeof RESUMABLE_CROSS_DOMAIN_POLICY_KEYS[number]>;
const RESUMABLE_CROSS_DOMAIN_POLICY_KEYS_ARE_EXHAUSTIVE: MissingResumableCrossDomainPolicyKey extends never ? true : never = true;

const RESUMABLE_PREPARE_POLICY_KEYS = [
  "selectionPlan",
  "selectionOptions",
  "adaptiveSelection",
  "healthSelectionOptions",
  "readinessPolicy",
  "retryPolicy",
  "failoverPolicy",
  "providerContracts",
  "sourceBoundary",
  "allowDegradedDispatch",
  "healthStore",
] as const satisfies readonly (keyof AutonomousReviewedEvidencePreparationOptions)[];

type MissingResumablePreparePolicyKey = Exclude<keyof AutonomousReviewedEvidencePreparationOptions, typeof RESUMABLE_PREPARE_POLICY_KEYS[number]>;
const RESUMABLE_PREPARE_POLICY_KEYS_ARE_EXHAUSTIVE: MissingResumablePreparePolicyKey extends never ? true : never = true;

const RESUMABLE_EXECUTE_POLICY_KEYS = [
  "approveSourceDispatch",
  "providerContracts",
  "projector",
  "evaluator",
  "journal",
  "rehydrateValue",
  "parentEvidenceDigests",
  "stopOnFailure",
  "reevaluatePending",
  "classify",
  "observeFailover",
  "observeAttempt",
  "clock",
  "sleep",
  "sourceBoundary",
  "authorizationContext",
  "authorizationDomain",
  "authorizationCapability",
  "authorizationRiskClass",
] as const satisfies readonly (keyof AutonomousEvidenceExecutionOptions)[];

type MissingResumableExecutePolicyKey = Exclude<keyof AutonomousEvidenceExecutionOptions, typeof RESUMABLE_EXECUTE_POLICY_KEYS[number]>;
const RESUMABLE_EXECUTE_POLICY_KEYS_ARE_EXHAUSTIVE: MissingResumableExecutePolicyKey extends never ? true : never = true;

function copyDefinedPolicyFields(value: object, keys: readonly string[]): Record<string, unknown> {
  const source = value as Record<string, unknown>;
  return Object.fromEntries(keys.flatMap((key) => source[key] === undefined ? [] : [[key, source[key]]]));
}

function policyValueProjection(name: string, value: unknown): unknown {
  if (value === undefined) return null;
  const candidate = value as { toJSON?: () => unknown };
  const projected = isObject(value) && typeof candidate.toJSON === "function" ? candidate.toJSON() : value;
  canonicalJson(projected);
  return projected;
}

function promptTemplatePolicyProjection(name: string, value: unknown): unknown {
  if (value === undefined) return null;
  if (!isObject(value) || !("manifest" in value)) throw new ArgumentError(`${name} is not a typed prompt template`);
  return policyValueProjection(`${name} manifest`, value.manifest);
}

function credentialPolicyProjection(name: string, value: unknown): JsonObject | null {
  if (value === undefined) return null;
  if (!isObject(value)) throw new ArgumentError(`${name} is malformed`);
  return {
    present: true,
    provider: boundedIdentifier(`${name} provider`, value.provider),
    identity: "bound_by_resumable_provider_policy_config_digest",
  };
}

function presencePolicyProjection(value: unknown): JsonObject | null {
  return value === undefined ? null : { present: true, identity: "bound_by_resumable_provider_policy_config_digest" };
}

function clonePolicyValue(name: string, value: unknown): unknown {
  let snapshot: unknown;
  try {
    snapshot = nativeStructuredClone(value);
  } catch {
    throw new ArgumentError(`${name} cannot be snapshotted as canonical JSON`);
  }
  canonicalJson(snapshot);
  return snapshot;
}

function snapshotProviderResultGraph(name: string, value: unknown): unknown {
  const seen = new Set<object>();
  let nodes = 0;
  const visit = (candidate: unknown, depth: number): unknown => {
    if (depth > 64 || ++nodes > 100_000) throw new ArgumentError(`${name} exceeds its bounded JSON graph`);
    if (candidate === null || typeof candidate === "string" || typeof candidate === "boolean") return candidate;
    if (typeof candidate === "number") {
      if (!Number.isFinite(candidate)) throw new ArgumentError(`${name} contains a non-finite number`);
      return candidate;
    }
    if (typeof candidate !== "object") throw new ArgumentError(`${name} must contain JSON data only`);
    if (seen.has(candidate)) throw new ArgumentError(`${name} contains a cycle`);
    seen.add(candidate);
    try {
      const descriptors = nativeObjectGetOwnPropertyDescriptors(candidate);
      const keys = nativeReflectOwnKeys(descriptors);
      if (nativeArrayIsArray(candidate)) {
        if (nativeObjectGetPrototypeOf(candidate) !== Array.prototype) throw new ArgumentError(`${name} contains a non-core array`);
        const array = candidate as unknown[];
        const expected = new Set<string>(["length"]);
        for (let index = 0; index < array.length; index += 1) expected.add(String(index));
        if (keys.some((key) => typeof key !== "string" || !expected.has(key)) || keys.length !== expected.size) throw new ArgumentError(`${name} contains a sparse, accessor-backed, or extended array`);
        const snapshot: unknown[] = [];
        for (let index = 0; index < array.length; index += 1) {
          const descriptor = descriptors[String(index)];
          if (!descriptor || !descriptor.enumerable || !("value" in descriptor)) throw new ArgumentError(`${name} contains a sparse or accessor-backed array`);
          snapshot.push(visit(descriptor.value, depth + 1));
        }
        return snapshot;
      }
      const prototype = nativeObjectGetPrototypeOf(candidate);
      if (prototype !== nativeObjectPrototype && prototype !== null) throw new ArgumentError(`${name} contains inherited or branded provider data`);
      const snapshot: Record<string, unknown> = {};
      for (const key of keys) {
        if (typeof key !== "string") throw new ArgumentError(`${name} contains symbol-keyed provider data`);
        const descriptor = descriptors[key]!;
        if (!descriptor.enumerable || !("value" in descriptor)) throw new ArgumentError(`${name} contains hidden or accessor-backed provider data`);
        Object.defineProperty(snapshot, key, { value: visit(descriptor.value, depth + 1), enumerable: true, configurable: true, writable: true });
      }
      return snapshot;
    } finally {
      seen.delete(candidate);
    }
  };
  const snapshot = visit(value, 0);
  const encoded = canonicalJson(snapshot);
  if (bytes(encoded) > 2_000_000) throw new ArgumentError(`${name} exceeds its bounded serialized size`);
  return snapshot;
}

function cloneProjectedPolicyValue(name: string, value: unknown): unknown {
  const projected = policyValueProjection(name, value);
  return clonePolicyValue(name, projected);
}

function snapshotPromptRegistry(name: string, value: unknown): AutonomousPromptRegistry | undefined {
  if (value === undefined) return undefined;
  if (!(value instanceof AutonomousPromptRegistry)) throw new ArgumentError(`${name} is not a typed prompt registry`);
  const templates = value.manifests.map((manifest) => value.templateFor(manifest.prompt_id));
  const snapshot = new AutonomousPromptRegistry(templates);
  canonicalJson(snapshot.toJSON());
  return snapshot;
}

function snapshotEvidenceAdapterRegistry(value: unknown): {
  registry: AutonomousEvidenceAdapterRegistry;
  projection: ReturnType<AutonomousEvidenceAdapterRegistry["toJSON"]>;
} {
  if (!(value instanceof AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("resumable evidence-backed execution requires a typed adapter registry");
  const projection = nativeStructuredClone(value.toJSON());
  const expectedDigest = projection.registry_digest;
  const assertUnchanged = (): void => {
    if (value.toJSON().registry_digest !== expectedDigest) {
      throw new ArgumentError("resumable evidence adapter registry changed after its policy snapshot");
    }
  };
  const snapshot = new AutonomousEvidenceAdapterRegistry();
  for (const manifest of projection.adapters) {
    const adapterIdForDomain = Object.fromEntries(manifest.domains.map((domain) => [domain, manifest.adapter_id])) as Partial<Record<AutonomousDomainName, string>>;
    const acquirer = value.createAcquirer({ adapterIdForDomain });
    const projector = value.createProjector({ adapterIdForDomain });
    snapshot.register({
      adapterId: manifest.adapter_id,
      version: manifest.version,
      domains: manifest.domains,
      capabilities: manifest.capabilities,
      sourceKinds: manifest.source_kinds,
      acquire: async (context) => {
        assertUnchanged();
        return acquirer.acquire(context);
      },
      project: async (evidenceValue, context) => {
        assertUnchanged();
        return projector.project(evidenceValue, context);
      },
    });
  }
  if (snapshot.toJSON().registry_digest !== expectedDigest) throw new ProviderRuntimeError("resumable evidence adapter registry snapshot does not match its manifest digest");
  return { registry: snapshot, projection };
}

function snapshotReadinessPolicy(value: unknown): AutonomousEvidenceReadinessPolicy {
  if (!(value instanceof AutonomousEvidenceReadinessPolicy)) throw new ArgumentError("resumable evidence readinessPolicy is not typed");
  const policy = nativeStructuredClone(value.toJSON());
  return new AutonomousEvidenceReadinessPolicy({
    requireHealth: policy.require_health,
    minAttempts: policy.min_attempts,
    failureThreshold: policy.failure_threshold,
    minSuccessRate: policy.min_success_rate,
  });
}

function snapshotRetryPolicy(value: unknown): AutonomousEvidenceRetryPolicy {
  if (!(value instanceof AutonomousEvidenceRetryPolicy)) throw new ArgumentError("resumable evidence retryPolicy is not typed");
  const policy = nativeStructuredClone(value.toJSON());
  return new AutonomousEvidenceRetryPolicy({
    maxAttempts: policy.max_attempts,
    baseDelayMs: policy.base_delay_ms,
    maxDelayMs: policy.max_delay_ms,
    retryableFailureClasses: policy.retryable_failure_classes,
  });
}

function snapshotFailoverPolicy(value: unknown): AutonomousEvidenceFailoverPolicy {
  if (!(value instanceof AutonomousEvidenceFailoverPolicy)) throw new ArgumentError("resumable evidence failoverPolicy is not typed");
  const policy = nativeStructuredClone(value.toJSON());
  const retry = policy.retry_policy as ReturnType<AutonomousEvidenceRetryPolicy["toJSON"]>;
  return new AutonomousEvidenceFailoverPolicy({
    maxFailovers: policy.max_failovers,
    retryPolicy: new AutonomousEvidenceRetryPolicy({
      maxAttempts: retry.max_attempts,
      baseDelayMs: retry.base_delay_ms,
      maxDelayMs: retry.max_delay_ms,
      retryableFailureClasses: retry.retryable_failure_classes,
    }),
  });
}

function snapshotProviderContracts(
  value: unknown,
  adapterRegistry: AutonomousEvidenceAdapterRegistry,
): AutonomousEvidenceProviderContractRegistry {
  if (!(value instanceof AutonomousEvidenceProviderContractRegistry)) throw new ArgumentError("resumable evidence providerContracts is not typed");
  const projection = nativeStructuredClone(value.toJSON());
  if (projection.adapter_registry_digest !== adapterRegistry.toJSON().registry_digest) {
    throw new ArgumentError("resumable evidence providerContracts do not match the snapshotted adapter registry");
  }
  const snapshot = new AutonomousEvidenceProviderContractRegistry(adapterRegistry);
  for (const contract of projection.contracts) {
    snapshot.register({
      contractId: contract.contract_id,
      version: contract.version,
      provider: contract.provider,
      protocol: contract.protocol,
      operations: contract.operations,
      domains: contract.domains,
      capabilities: contract.capabilities,
      sourceKinds: contract.source_kinds,
      authMode: contract.auth_mode,
      freshness: contract.freshness,
      pagination: contract.pagination,
      requiredMetadata: contract.required_metadata,
      operationMetadataKey: contract.operation_metadata_key,
      adapterId: contract.adapter_id,
    });
  }
  if (snapshot.toJSON().registry_digest !== projection.registry_digest) {
    throw new ProviderRuntimeError("resumable evidence providerContracts snapshot does not match its registry digest");
  }
  return snapshot;
}

function snapshotSourceBoundary(value: unknown): unknown {
  if (!isObject(value)) throw new ArgumentError("resumable evidence sourceBoundary is malformed");
  const source = snapshotControlEnvelope(value, ["policy", "ledger", "sourceKind", "describeSource"], "resumable evidence sourceBoundary");
  if (!(source.policy instanceof AutonomousEvidenceSourcePolicy)) throw new ArgumentError("resumable evidence sourceBoundary policy is not typed");
  const policy = nativeStructuredClone(source.policy.toJSON());
  return {
    policy: new AutonomousEvidenceSourcePolicy({
      maxAgeMs: policy.max_age_ms,
      maxFutureSkewMs: policy.max_future_skew_ms,
      allowPartial: policy.allow_partial,
      allowUnverified: policy.allow_unverified,
      requireSourceDigest: policy.require_source_digest,
      now: source.policy.now.bind(source.policy),
    }),
    ...(source.ledger === undefined ? {} : { ledger: source.ledger }),
    ...(source.sourceKind === undefined ? {} : { sourceKind: source.sourceKind }),
    ...(source.describeSource === undefined ? {} : { describeSource: source.describeSource }),
  };
}

function snapshotPreparePolicy(
  prepare: AutonomousReviewedEvidencePreparationOptions | undefined,
  adapterRegistry: AutonomousEvidenceAdapterRegistry,
): AutonomousReviewedEvidencePreparationOptions | undefined {
  if (prepare === undefined) return undefined;
  if (!isObject(prepare)) throw new ArgumentError("resumable evidence preparation options are malformed");
  const source = snapshotControlEnvelope(prepare, RESUMABLE_PREPARE_POLICY_KEYS, "resumable evidence preparation options") as unknown as AutonomousReviewedEvidencePreparationOptions;
  const snapshot: Record<string, unknown> = {};
  for (const key of RESUMABLE_PREPARE_POLICY_KEYS) {
    const value = source[key];
    if (value === undefined) continue;
    if (key === "readinessPolicy") snapshot[key] = snapshotReadinessPolicy(value);
    else if (key === "retryPolicy") snapshot[key] = snapshotRetryPolicy(value);
    else if (key === "failoverPolicy") snapshot[key] = snapshotFailoverPolicy(value);
    else if (key === "providerContracts") snapshot[key] = snapshotProviderContracts(value, adapterRegistry);
    else if (key === "healthStore") snapshot[key] = value;
    else if (key === "sourceBoundary") snapshot[key] = snapshotSourceBoundary(value);
    else if (key === "selectionPlan") snapshot[key] = cloneProjectedPolicyValue("resumable evidence selectionPlan", value);
    else snapshot[key] = clonePolicyValue(`resumable evidence prepare ${key}`, value);
  }
  return snapshot as unknown as AutonomousReviewedEvidencePreparationOptions;
}

const OPAQUE_EXECUTE_POLICY_KEYS = new Set<keyof AutonomousEvidenceExecutionOptions>([
  "providerContracts",
  "projector",
  "evaluator",
  "journal",
  "rehydrateValue",
  "classify",
  "observeFailover",
  "observeAttempt",
  "clock",
  "sleep",
  "sourceBoundary",
  "authorizationContext",
]);

function snapshotExecutePolicy(
  execute: AutonomousEvidenceExecutionOptions | undefined,
  adapterRegistry: AutonomousEvidenceAdapterRegistry,
): AutonomousEvidenceExecutionOptions | undefined {
  if (execute === undefined) return undefined;
  if (!isObject(execute)) throw new ArgumentError("resumable evidence execution options are malformed");
  const source = snapshotControlEnvelope(execute, RESUMABLE_EXECUTE_POLICY_KEYS, "resumable evidence execution options") as unknown as AutonomousEvidenceExecutionOptions;
  const snapshot: Record<string, unknown> = {};
  for (const key of RESUMABLE_EXECUTE_POLICY_KEYS) {
    const value = source[key];
    if (value === undefined) continue;
    if (key === "providerContracts") snapshot[key] = snapshotProviderContracts(value, adapterRegistry);
    else if (key === "sourceBoundary") snapshot[key] = snapshotSourceBoundary(value);
    else snapshot[key] = OPAQUE_EXECUTE_POLICY_KEYS.has(key)
      ? value
      : clonePolicyValue(`resumable evidence execute ${key}`, value);
  }
  return snapshot as unknown as AutonomousEvidenceExecutionOptions;
}

function preparePolicyProjection(prepare: AutonomousReviewedEvidencePreparationOptions | undefined): Record<string, unknown> | null {
  if (prepare === undefined) return null;
  const projected = copyDefinedPolicyFields(prepare, RESUMABLE_PREPARE_POLICY_KEYS);
  projected.selectionPlan = policyValueProjection("resumable evidence selectionPlan", prepare.selectionPlan);
  projected.readinessPolicy = policyValueProjection("resumable evidence readinessPolicy", prepare.readinessPolicy);
  projected.retryPolicy = policyValueProjection("resumable evidence retryPolicy", prepare.retryPolicy);
  projected.failoverPolicy = policyValueProjection("resumable evidence failoverPolicy", prepare.failoverPolicy);
  projected.providerContracts = policyValueProjection("resumable evidence providerContracts", prepare.providerContracts);
  projected.healthStore = presencePolicyProjection(prepare.healthStore);
  projected.sourceBoundary = prepare.sourceBoundary === undefined ? null : {
    policy: policyValueProjection("resumable evidence source policy", prepare.sourceBoundary.policy),
    source_kind: prepare.sourceBoundary.sourceKind ?? null,
  };
  canonicalJson(projected);
  return projected;
}

function executePolicyProjection(execute: AutonomousEvidenceExecutionOptions | undefined): Record<string, unknown> | null {
  if (execute === undefined) return null;
  const projected = copyDefinedPolicyFields(execute, RESUMABLE_EXECUTE_POLICY_KEYS);
  projected.approveSourceDispatch = "managed_by_resumable_evidence_transition";
  projected.providerContracts = policyValueProjection("resumable evidence execute providerContracts", execute.providerContracts);
  for (const key of OPAQUE_EXECUTE_POLICY_KEYS) {
    if (key === "rehydrateValue") projected[key] = "reserved_by_value_rehydrator_identity";
    else if (key !== "providerContracts") projected[key] = presencePolicyProjection(execute[key]);
  }
  canonicalJson(projected);
  return projected;
}

const OPAQUE_RUN_POLICY_KEYS = new Set<keyof AutonomousAutoRunOptions>([
  "credential",
  "credentialFor",
  "authorizationContext",
  "promptTemplate",
  "promptRegistry",
  "memoryStore",
  "memoryConsolidator",
  "memoryLessonResolver",
  "memoryLessonContextResolver",
  "learning",
  "costBudget",
  "authorizeAndExecute",
  "toolReadOnly",
  "execution",
  "effectBoundary",
  "signal",
  "observer",
  "selectionEventCallback",
]);

const OPAQUE_PLANNING_POLICY_KEYS = new Set<keyof AutonomousProviderPlanningOptions>([
  "credential",
  "credentialFor",
  "promptTemplate",
  "promptRegistry",
  "costBudget",
  "execution",
  "signal",
  "observer",
  "selectionEventCallback",
  "authorizationContext",
]);

function snapshotPlanningPolicy(
  planning: AutonomousProviderPlanningOptions | undefined,
  requireExplicitCandidates = false,
): AutonomousProviderPlanningOptions | undefined {
  if (requireExplicitCandidates && planning === undefined) {
    throw new ArgumentError("resumable provider planning requires explicit non-empty planning.candidates");
  }
  if (planning === undefined) return undefined;
  if (!isObject(planning)) throw new ArgumentError("resumable provider planning options are malformed");
  const source = snapshotControlEnvelope(planning, RESUMABLE_PLANNING_POLICY_KEYS, "resumable provider planning options") as unknown as AutonomousProviderPlanningOptions;
  if (source.providerDispatchFence !== undefined) throw new ArgumentError("resumable evidence-backed execution owns the provider planning dispatch fence");
  if (source.runId !== undefined) throw new ArgumentError("resumable evidence-backed execution derives provider planning runId internally");
  if ((requireExplicitCandidates && !nativeArrayIsArray(source.candidates))
      || (source.candidates !== undefined && (!nativeArrayIsArray(source.candidates) || source.candidates.length < 1))) {
    throw new ArgumentError("resumable provider planning requires explicit non-empty planning.candidates");
  }
  const snapshot: Record<string, unknown> = {};
  for (const key of RESUMABLE_PLANNING_POLICY_KEYS) {
    const value = source[key];
    if (value === undefined || key === "runId") continue;
    if (key === "promptRegistry") snapshot[key] = snapshotPromptRegistry("resumable provider planning promptRegistry", value);
    else if (key === "promptSelection" || key === "promptLearningState") snapshot[key] = cloneProjectedPolicyValue(`resumable provider planning ${key}`, value);
    else if (OPAQUE_PLANNING_POLICY_KEYS.has(key)) snapshot[key] = value;
    else snapshot[key] = clonePolicyValue(`resumable provider planning ${key}`, value);
  }
  return snapshot as unknown as AutonomousProviderPlanningOptions;
}

function snapshotProviderRunPolicy(run: AutonomousAutoRunOptions | undefined): AutonomousAutoRunOptions {
  if (!isObject(run)) throw new ArgumentError("resumable run options are malformed");
  const source = snapshotControlEnvelope(run, RESUMABLE_RUN_POLICY_KEYS, "resumable run options") as unknown as AutonomousAutoRunOptions;
  if (source.providerDispatchFence !== undefined) throw new ArgumentError("resumable evidence-backed execution owns the provider dispatch fence");
  if (source.providerIdempotencyKey !== undefined) throw new ArgumentError("resumable evidence-backed execution derives providerIdempotencyKey internally");
  assertExactEffectBoundary(source.effectBoundary, "resumable run effectBoundary");
  if (source.semanticRouting !== undefined && source.semanticRouting !== false) throw new ArgumentError("resumable evidence-backed execution requires provider-free semantic routing");
  if (!nativeArrayIsArray(source.candidates) || source.candidates.length < 1) {
    throw new ArgumentError("resumable evidence-backed execution requires explicit non-empty run.candidates");
  }
  const snapshot: Record<string, unknown> = {};
  for (const key of RESUMABLE_RUN_POLICY_KEYS) {
    const value = source[key];
    if (value === undefined || key === "providerIdempotencyKey") continue;
    if (key === "planning") snapshot[key] = snapshotPlanningPolicy(source.planning, source.planningMode === "provider");
    else if (key === "promptRegistry") snapshot[key] = snapshotPromptRegistry("resumable provider promptRegistry", value);
    else if (key === "promptSelection" || key === "promptLearningState" || key === "planningPromptLearningState") snapshot[key] = cloneProjectedPolicyValue(`resumable provider ${key}`, value);
    else if (OPAQUE_RUN_POLICY_KEYS.has(key)) snapshot[key] = value;
    else snapshot[key] = clonePolicyValue(`resumable provider ${key}`, value);
  }
  return snapshot as unknown as AutonomousAutoRunOptions;
}

function snapshotCrossDomainPolicy(crossDomain: ResumableCrossDomainPolicy | undefined): ResumableCrossDomainPolicy | undefined {
  if (crossDomain === undefined) return undefined;
  if (!isObject(crossDomain)) throw new ArgumentError("resumable crossDomain options are malformed");
  const snapshot = snapshotControlEnvelope(crossDomain, RESUMABLE_CROSS_DOMAIN_POLICY_KEYS, "resumable crossDomain options");
  return clonePolicyValue("resumable crossDomain options", snapshot) as ResumableCrossDomainPolicy;
}

interface ResumableProviderPolicySnapshot {
  run: AutonomousAutoRunOptions;
  crossDomain: ResumableCrossDomainPolicy | undefined;
}

function snapshotResumableProviderPolicy(options: AutonomousEvidenceBackedResumableExecutionOptions): ResumableProviderPolicySnapshot {
  return {
    run: snapshotProviderRunPolicy(options.run),
    crossDomain: snapshotCrossDomainPolicy(options.crossDomain),
  };
}

interface ResumableExecutionInputSnapshot {
  baseOptions: AutonomousEvidenceBackedRunOptions;
  registryProjection: ReturnType<AutonomousEvidenceAdapterRegistry["toJSON"]>;
  identities: ReturnType<typeof resumablePolicyIdentities>;
  controlOptions: AutonomousEvidenceBackedResumableExecutionOptions;
}

function snapshotResumableExecutionInputs(options: AutonomousEvidenceBackedResumableExecutionOptions): ResumableExecutionInputSnapshot {
  const controlOptions = snapshotControlEnvelope(
    options as unknown as Record<string, unknown>,
    RESUMABLE_EXECUTION_OPTION_KEYS,
    "evidence-backed resumable options",
  ) as unknown as AutonomousEvidenceBackedResumableExecutionOptions;
  const providerPolicy = snapshotResumableProviderPolicy(controlOptions);
  const adapterRegistry = snapshotEvidenceAdapterRegistry(controlOptions.registry);
  const baseOptions: AutonomousEvidenceBackedRunOptions = {
    registry: adapterRegistry.registry,
    requests: clonePolicyValue("resumable evidence requests", controlOptions.requests) as AutonomousEvidenceBackedRunOptions["requests"],
    domains: controlOptions.domains === undefined
      ? undefined
      : clonePolicyValue("resumable evidence domains", controlOptions.domains) as AutonomousEvidenceBackedRunOptions["domains"],
    availableEvidence: controlOptions.availableEvidence === undefined
      ? undefined
      : clonePolicyValue("resumable availableEvidence", controlOptions.availableEvidence) as AutonomousEvidenceBackedRunOptions["availableEvidence"],
    completedStages: controlOptions.completedStages === undefined
      ? undefined
      : clonePolicyValue("resumable completedStages", controlOptions.completedStages) as AutonomousEvidenceBackedRunOptions["completedStages"],
    prepare: snapshotPreparePolicy(controlOptions.prepare, adapterRegistry.registry),
    execute: snapshotExecutePolicy(controlOptions.execute, adapterRegistry.registry),
    runMode: controlOptions.runMode,
    run: providerPolicy.run,
    crossDomain: providerPolicy.crossDomain,
    promptBuilder: controlOptions.promptBuilder,
    allowIncompleteEvidence: controlOptions.allowIncompleteEvidence,
    evidenceCheckpointStore: controlOptions.evidenceCheckpointStore,
    evidenceJobId: controlOptions.evidenceJobId,
    evidenceReconciliationAuthority: controlOptions.evidenceReconciliationAuthority === undefined
      ? undefined
      : clonePolicyValue("resumable evidence reconciliation authority", controlOptions.evidenceReconciliationAuthority) as AutonomousEvidenceBackedRunOptions["evidenceReconciliationAuthority"],
    evidenceExecutionPolicyIdentity: controlOptions.evidenceExecutionPolicyIdentity === undefined
      ? undefined
      : clonePolicyValue("resumable evidence execution policy identity", controlOptions.evidenceExecutionPolicyIdentity) as AutonomousEvidenceBackedRunOptions["evidenceExecutionPolicyIdentity"],
    evidenceReconciliationReceipt: controlOptions.evidenceReconciliationReceipt === undefined
      ? undefined
      : clonePolicyValue("resumable evidence reconciliation receipt", controlOptions.evidenceReconciliationReceipt) as AutonomousEvidenceBackedRunOptions["evidenceReconciliationReceipt"],
    evidenceResumeAfterReconciliation: controlOptions.evidenceResumeAfterReconciliation,
  };
  return {
    baseOptions,
    registryProjection: adapterRegistry.projection,
    identities: resumablePolicyIdentities({
      ...controlOptions,
      ...baseOptions,
      resumablePolicyIdentity: controlOptions.resumablePolicyIdentity,
    }),
    controlOptions,
  };
}

function planningPolicyProjection(planning: AutonomousProviderPlanningOptions | undefined): Record<string, unknown> | null {
  if (planning === undefined) return null;
  if (!isObject(planning)) throw new ArgumentError("resumable provider planning options are malformed");
  allowedKeys(planning, RESUMABLE_PLANNING_POLICY_KEYS, "resumable provider planning options");
  if (planning.runId !== undefined) throw new ArgumentError("resumable evidence-backed execution derives provider planning runId internally");
  if (planning.candidates !== undefined && (!nativeArrayIsArray(planning.candidates) || planning.candidates.length < 1)) {
    throw new ArgumentError("resumable provider planning candidates must be a non-empty explicit list");
  }
  const projected = copyDefinedPolicyFields(planning, RESUMABLE_PLANNING_POLICY_KEYS);
  projected.approveProviderCall = "managed_by_resumable_pending_transition";
  projected.credential = credentialPolicyProjection("resumable provider planning credential", planning.credential);
  projected.credentialFor = presencePolicyProjection(planning.credentialFor);
  projected.promptTemplate = promptTemplatePolicyProjection("resumable provider planning promptTemplate", planning.promptTemplate);
  projected.promptRegistry = policyValueProjection("resumable provider planning promptRegistry", planning.promptRegistry);
  projected.promptSelection = policyValueProjection("resumable provider planning promptSelection", planning.promptSelection);
  projected.promptLearningState = policyValueProjection("resumable provider planning promptLearningState", planning.promptLearningState);
  projected.costBudget = presencePolicyProjection(planning.costBudget);
  projected.execution = presencePolicyProjection(planning.execution);
  projected.signal = presencePolicyProjection(planning.signal);
  projected.observer = presencePolicyProjection(planning.observer);
  projected.providerDispatchFence = "managed_by_resumable_dispatch_transaction";
  projected.selectionEventCallback = presencePolicyProjection(planning.selectionEventCallback);
  projected.authorizationContext = presencePolicyProjection(planning.authorizationContext);
  delete projected.runId;
  canonicalJson(projected);
  return projected;
}

function crossDomainPolicyProjection(crossDomain: ResumableCrossDomainPolicy | undefined): Record<string, unknown> | null {
  if (crossDomain === undefined) return null;
  if (!isObject(crossDomain)) throw new ArgumentError("resumable crossDomain options are malformed");
  allowedKeys(crossDomain, RESUMABLE_CROSS_DOMAIN_POLICY_KEYS, "resumable crossDomain options");
  const projected = copyDefinedPolicyFields(crossDomain, RESUMABLE_CROSS_DOMAIN_POLICY_KEYS);
  canonicalJson(projected);
  return projected;
}

function providerRunPolicyProjection(run: AutonomousAutoRunOptions): Record<string, unknown> {
  if (!isObject(run)) throw new ArgumentError("resumable run options are malformed");
  allowedKeys(run, RESUMABLE_RUN_POLICY_KEYS, "resumable run options");
  if (run.providerIdempotencyKey !== undefined) throw new ArgumentError("resumable evidence-backed execution derives providerIdempotencyKey internally");
  if (run.semanticRouting !== undefined && run.semanticRouting !== false) throw new ArgumentError("resumable evidence-backed execution requires provider-free semantic routing");
  if (!nativeArrayIsArray(run.candidates) || run.candidates.length < 1) {
    throw new ArgumentError("resumable evidence-backed execution requires explicit non-empty run.candidates");
  }
  const projected = copyDefinedPolicyFields(run, RESUMABLE_RUN_POLICY_KEYS);
  projected.approveProviderCall = "managed_by_resumable_pending_transition";
  projected.credential = credentialPolicyProjection("resumable provider credential", run.credential);
  projected.credentialFor = presencePolicyProjection(run.credentialFor);
  projected.authorizationContext = presencePolicyProjection(run.authorizationContext);
  projected.promptTemplate = promptTemplatePolicyProjection("resumable provider promptTemplate", run.promptTemplate);
  projected.promptRegistry = policyValueProjection("resumable provider promptRegistry", run.promptRegistry);
  projected.promptSelection = policyValueProjection("resumable provider promptSelection", run.promptSelection);
  projected.promptLearningState = policyValueProjection("resumable provider promptLearningState", run.promptLearningState);
  projected.memoryStore = presencePolicyProjection(run.memoryStore);
  projected.memoryConsolidator = presencePolicyProjection(run.memoryConsolidator);
  projected.memoryLessonResolver = presencePolicyProjection(run.memoryLessonResolver);
  projected.memoryLessonContextResolver = presencePolicyProjection(run.memoryLessonContextResolver);
  projected.learning = presencePolicyProjection(run.learning);
  projected.costBudget = presencePolicyProjection(run.costBudget);
  projected.authorizeAndExecute = presencePolicyProjection(run.authorizeAndExecute);
  projected.toolReadOnly = presencePolicyProjection(run.toolReadOnly);
  projected.execution = presencePolicyProjection(run.execution);
  projected.effectBoundary = presencePolicyProjection(run.effectBoundary);
  projected.signal = presencePolicyProjection(run.signal);
  projected.observer = presencePolicyProjection(run.observer);
  projected.providerDispatchFence = "managed_by_resumable_dispatch_transaction";
  projected.selectionEventCallback = presencePolicyProjection(run.selectionEventCallback);
  projected.planning = planningPolicyProjection((run as AutonomousAutoRunOptions).planning);
  projected.planningPromptLearningState = policyValueProjection("resumable planningPromptLearningState", run.planningPromptLearningState);
  delete projected.providerIdempotencyKey;
  canonicalJson(projected);
  return projected;
}

async function runPolicyDigest(
  options: AutonomousEvidenceBackedRunOptions,
  identities: ReturnType<typeof resumablePolicyIdentities>,
  registry: ReturnType<AutonomousEvidenceAdapterRegistry["toJSON"]>,
): Promise<string> {
  const run = options.run!;
  void RESUMABLE_RUN_POLICY_KEYS_ARE_EXHAUSTIVE;
  void RESUMABLE_PLANNING_POLICY_KEYS_ARE_EXHAUSTIVE;
  void RESUMABLE_CROSS_DOMAIN_POLICY_KEYS_ARE_EXHAUSTIVE;
  void RESUMABLE_PREPARE_POLICY_KEYS_ARE_EXHAUSTIVE;
  void RESUMABLE_EXECUTE_POLICY_KEYS_ARE_EXHAUSTIVE;
  void AUTONOMOUS_EVIDENCE_BACKED_RUN_OPTION_KEYS_ARE_EXHAUSTIVE;
  void RESUMABLE_EXECUTION_OPTION_KEYS_ARE_EXHAUSTIVE;
  return digestJson({
    schema: AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA,
    run_mode: options.runMode ?? "domain",
    domains: options.domains === undefined ? null : [...options.domains].sort(),
    available_evidence: options.availableEvidence === undefined ? null : [...options.availableEvidence].sort(),
    completed_stages_digest: options.completedStages === undefined ? null : await digestJson(options.completedStages),
    allow_incomplete_evidence: options.allowIncompleteEvidence ?? false,
    adapter_registry_digest: registry.registry_digest,
    implementation_identities: identities,
    provider_run_policy: providerRunPolicyProjection(run),
    cross_domain_policy: crossDomainPolicyProjection(options.crossDomain),
    evidence_prepare_policy: preparePolicyProjection(options.prepare),
    evidence_execute_policy: executePolicyProjection(options.execute),
    evidence_checkpointed: options.evidenceCheckpointStore !== undefined,
    evidence_job_id: options.evidenceJobId ?? null,
    evidence_reconciliation_authority: options.evidenceReconciliationAuthority ?? null,
    evidence_execution_policy_identity: options.evidenceExecutionPolicyIdentity ?? null,
  });
}

function providerResultCompleted(status: string | null): boolean {
  return status === "completed" || status === "children_completed";
}

function checkpointStatusForResult(
  result: AutonomousEvidenceBackedRunResult,
  providerOperationDigest: string | null,
): AutonomousEvidenceBackedCheckpointStatus {
  if (result.status === "evidence_review_required") return "evidence_review_required";
  if (result.status === "evidence_blocked") return "evidence_blocked";
  if (result.evidence && result.evidence.status !== "completed") return providerOperationDigest === null ? "evidence_incomplete" : "provider_reconciliation_required";
  const providerStatus = result.automatic?.status ?? result.cross_domain_run?.status ?? result.run?.status ?? null;
  if (providerResultCompleted(providerStatus) && providerOperationDigest !== null) return "completed";
  return providerOperationDigest === null ? "provider_pending" : "provider_reconciliation_required";
}

function nextLineage(previous: AutonomousEvidenceBackedCheckpointJSON | null): {
  generation: number;
  previous_checkpoint_digest: string | null;
} {
  if (previous?.generation === Number.MAX_SAFE_INTEGER) throw new ProviderRuntimeError("evidence-backed checkpoint generation is exhausted");
  return {
    generation: previous === null ? 1 : previous.generation + 1,
    previous_checkpoint_digest: previous?.checkpoint_digest ?? null,
  };
}

const CHECKPOINT_SUCCESSORS: Readonly<Record<AutonomousEvidenceBackedCheckpointStatus, readonly AutonomousEvidenceBackedCheckpointStatus[]>> = {
  evidence_review_required: ["evidence_review_required", "evidence_blocked", "evidence_incomplete", "provider_pending"],
  evidence_blocked: ["evidence_review_required", "evidence_blocked", "evidence_incomplete", "provider_pending"],
  evidence_incomplete: ["evidence_review_required", "evidence_blocked", "evidence_incomplete", "provider_pending"],
  provider_pending: ["provider_in_flight"],
  provider_in_flight: ["provider_in_flight", "provider_reconciliation_required", "completed"],
  provider_reconciliation_required: ["provider_reconciliation_required", "completed"],
  completed: [],
};

function assertCheckpointSuccessor(
  previous: AutonomousEvidenceBackedCheckpointJSON | null,
  next: AutonomousEvidenceBackedCheckpointJSON,
  dispatchReceipt: AutonomousEvidenceBackedProviderDispatchReceiptProjection | null = null,
): void {
  const expectedDigest = previous?.checkpoint_digest ?? null;
  if (next.previous_checkpoint_digest !== expectedDigest || next.generation !== (previous?.generation ?? 0) + 1) {
    throw new ProviderRuntimeError("evidence-backed checkpoint successor does not extend the exact expected head");
  }
  if (previous !== null && !CHECKPOINT_SUCCESSORS[previous.status].includes(next.status)) {
    throw new ProviderRuntimeError(`evidence-backed checkpoint transition ${previous.status} -> ${next.status} is not permitted`);
  }
  if (previous !== null && (
    next.job_id !== previous.job_id
    || next.task_digest !== previous.task_digest
    || next.request_digest !== previous.request_digest
    || next.run_policy_digest !== previous.run_policy_digest
    || next.evidence_plan_digest !== previous.evidence_plan_digest
    || next.execution_plan_digest !== previous.execution_plan_digest
  )) throw new ProviderRuntimeError("evidence-backed checkpoint transition changed immutable operation binding");
  if (previous !== null && ["provider_pending", "provider_in_flight", "provider_reconciliation_required", "completed"].includes(previous.status) && (
    next.evidence_result_digest !== previous.evidence_result_digest
    || next.prompt_projection_digest !== previous.prompt_projection_digest
  )) throw new ProviderRuntimeError("provider-bound checkpoint transition changed settled evidence or prompt binding");
  if (previous?.provider_result_digest !== null && previous?.provider_result_digest !== undefined && (
    next.provider_result_digest !== previous.provider_result_digest
    || next.provider_status !== previous.provider_status
  )) throw new ProviderRuntimeError("provider settlement transition changed an observed result binding");
  if (dispatchReceipt === null) {
    if (next.provider_operation_digest !== (previous?.provider_operation_digest ?? null)
        || next.provider_dispatch_count !== (previous?.provider_dispatch_count ?? 0)
        || next.provider_dispatch_head_digest !== (previous?.provider_dispatch_head_digest ?? null)) {
      throw new ProviderRuntimeError("non-dispatch checkpoint transition cannot alter provider operation or dispatch-chain identity");
    }
    return;
  }
  if (next.status !== "provider_in_flight"
      || next.provider_operation_digest === null
      || next.provider_dispatch_count !== (previous?.provider_dispatch_count ?? 0) + 1
      || next.provider_dispatch_head_digest !== dispatchReceipt.receipt_digest
      || dispatchReceipt.sequence !== next.provider_dispatch_count
      || dispatchReceipt.previous_receipt_digest !== (previous?.provider_dispatch_head_digest ?? null)
      || dispatchReceipt.provider_operation_digest !== next.provider_operation_digest) {
    throw new ProviderRuntimeError("provider dispatch checkpoint does not extend the exact dispatch receipt chain");
  }
  if (previous?.provider_operation_digest !== null
      && previous?.provider_operation_digest !== undefined
      && previous.provider_operation_digest !== next.provider_operation_digest) {
    throw new ProviderRuntimeError("provider dispatch checkpoint changed its bound provider operation");
  }
}

async function providerOperationDigest(input: {
  jobId: string;
  taskDigest: string;
  requestDigest: string;
  runPolicyDigest: string;
  runMode: string;
  preflight: AutonomousEvidenceBackedRunPreflight;
}): Promise<string> {
  return digestJson({
    schema: "bioprism-typescript-autonomous-evidence-backed-provider-operation/0.1",
    job_id: input.jobId,
    task_digest: input.taskDigest,
    request_digest: input.requestDigest,
    run_policy_digest: input.runPolicyDigest,
    run_mode: input.runMode,
    evidence_plan_digest: input.preflight.executionPlan.evidence_plan_digest,
    execution_plan_digest: input.preflight.executionPlan.plan_digest,
    evidence_result_digest: input.preflight.evidence.result_digest,
    prompt_projection_digest: input.preflight.promptContext.length ? await digestJson(input.preflight.promptContext) : null,
  });
}

async function providerIdempotencyKey(operationDigest: string): Promise<string> {
  return digestJson({
    schema: "bioprism-typescript-autonomous-evidence-backed-provider-idempotency/0.1",
    provider_operation_digest: operationDigest,
  });
}

class PrivateAutonomousEvidenceBackedProviderDispatchReceipt implements AutonomousEvidenceBackedProviderDispatchReceipt {
  readonly projection: AutonomousEvidenceBackedProviderDispatchReceiptProjection;
  readonly #providerIdempotencyKey: string;

  constructor(projection: AutonomousEvidenceBackedProviderDispatchReceiptProjection, providerIdempotencyKeyValue: string) {
    this.projection = nativeObjectFreeze(nativeStructuredClone(projection));
    this.#providerIdempotencyKey = providerIdempotencyKeyValue;
    nativeObjectFreeze(this);
  }

  providerIdempotencyKey(): string {
    return this.#providerIdempotencyKey;
  }

  toJSON(): AutonomousEvidenceBackedProviderDispatchReceiptProjection {
    return nativeStructuredClone(this.projection);
  }
}

// The private receipt crosses a caller-owned storage callback. Freeze its method surface so that
// callback cannot poison later key lookup or public projection by rewriting the shared prototype.
nativeObjectFreeze(PrivateAutonomousEvidenceBackedProviderDispatchReceipt.prototype);

async function makeProviderDispatchReceipt(input: {
  jobId: string;
  operationDigest: string;
  sequence: number;
  previousReceiptDigest: string | null;
  dispatch: ProviderTransportDispatchContext;
}): Promise<AutonomousEvidenceBackedProviderDispatchReceipt> {
  const providerIdempotencyKeyValue = boundedIdentifier(
    "resumable provider dispatch idempotency key",
    input.dispatch.providerIdempotencyKey,
  );
  const payload = {
    schema: AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCH_RECEIPT_SCHEMA,
    job_id: boundedIdentifier("resumable provider dispatch job_id", input.jobId),
    provider_operation_digest: digest("resumable provider dispatch operation digest", input.operationDigest),
    sequence: providerDispatchCount(input.sequence),
    previous_receipt_digest: optionalDigest("resumable provider dispatch previous receipt digest", input.previousReceiptDigest),
    provider: boundedDispatchText("resumable provider dispatch provider", input.dispatch.provider, 128),
    model: boundedDispatchText("resumable provider dispatch model", input.dispatch.model, 512),
    kind: boundedDispatchText("resumable provider dispatch kind", input.dispatch.kind, 128),
    transport_attempt: generation(input.dispatch.transportAttempt),
    request_digest: digest("resumable provider dispatch request digest", input.dispatch.requestDigest),
    provider_idempotency_key_digest: await digestJson({ provider_idempotency_key: providerIdempotencyKeyValue }),
  };
  const projection: AutonomousEvidenceBackedProviderDispatchReceiptProjection = {
    ...payload,
    receipt_digest: await digestJson(payload),
    retention: DISPATCH_RECEIPT_RETENTION,
    secret_material: SECRET_MATERIAL,
  };
  return new PrivateAutonomousEvidenceBackedProviderDispatchReceipt(projection, providerIdempotencyKeyValue);
}

async function validateProviderDispatchReceiptProjection(
  value: unknown,
): Promise<AutonomousEvidenceBackedProviderDispatchReceiptProjection> {
  if (!isObject(value)) throw new ArgumentError("provider dispatch receipt projection is malformed");
  const projection = nativeStructuredClone(value) as unknown as AutonomousEvidenceBackedProviderDispatchReceiptProjection;
  exactKeys(projection, ["schema", "job_id", "provider_operation_digest", "sequence", "previous_receipt_digest", "provider", "model", "kind", "transport_attempt", "request_digest", "provider_idempotency_key_digest", "receipt_digest", "retention", "secret_material"], "provider dispatch receipt projection");
  if (projection.schema !== AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCH_RECEIPT_SCHEMA
      || projection.retention !== DISPATCH_RECEIPT_RETENTION
      || projection.secret_material !== SECRET_MATERIAL) {
    throw new ArgumentError("provider dispatch receipt contract is invalid");
  }
  boundedIdentifier("provider dispatch receipt job_id", projection.job_id);
  digest("provider dispatch receipt operation digest", projection.provider_operation_digest);
  if (providerDispatchCount(projection.sequence) < 1) throw new ArgumentError("provider dispatch receipt sequence is outside its bounded contract");
  optionalDigest("provider dispatch receipt previous digest", projection.previous_receipt_digest);
  boundedDispatchText("provider dispatch receipt provider", projection.provider, 128);
  boundedDispatchText("provider dispatch receipt model", projection.model, 512);
  boundedDispatchText("provider dispatch receipt kind", projection.kind, 128);
  generation(projection.transport_attempt);
  digest("provider dispatch receipt request digest", projection.request_digest);
  digest("provider dispatch receipt idempotency digest", projection.provider_idempotency_key_digest);
  const receiptDigest = digest("provider dispatch receipt digest", projection.receipt_digest);
  const { receipt_digest: _receiptDigest, retention: _retention, secret_material: _secretMaterial, ...payload } = projection;
  if (receiptDigest !== await digestJson(payload)) throw new ArgumentError("provider dispatch receipt digest is invalid");
  return projection;
}

async function validateProviderDispatchReceipt(
  receipt: AutonomousEvidenceBackedProviderDispatchReceipt,
  checkpoint: AutonomousEvidenceBackedCheckpointJSON,
): Promise<{ projection: AutonomousEvidenceBackedProviderDispatchReceiptProjection; providerIdempotencyKey: string }> {
  if (!(receipt instanceof PrivateAutonomousEvidenceBackedProviderDispatchReceipt)) throw new ArgumentError("provider dispatch receipt must be created by the resumable runtime");
  const projection = await validateProviderDispatchReceiptProjection(receipt.toJSON());
  const providerIdempotencyKeyValue = boundedDispatchText("provider dispatch receipt idempotency key", receipt.providerIdempotencyKey(), 512);
  if (projection.job_id !== checkpoint.job_id
      || projection.provider_operation_digest !== checkpoint.provider_operation_digest
      || projection.sequence !== checkpoint.provider_dispatch_count
      || projection.receipt_digest !== checkpoint.provider_dispatch_head_digest) {
    throw new ArgumentError("provider dispatch receipt does not match its checkpoint");
  }
  if (projection.provider_idempotency_key_digest !== await digestJson({ provider_idempotency_key: providerIdempotencyKeyValue })) {
    throw new ArgumentError("provider dispatch receipt idempotency key digest is invalid");
  }
  return { projection: nativeStructuredClone(projection), providerIdempotencyKey: providerIdempotencyKeyValue };
}

async function checkpointForResult(input: {
  jobId: string;
  requestDigest: string;
  runPolicyDigest: string;
  result: AutonomousEvidenceBackedRunResult;
  previous: AutonomousEvidenceBackedCheckpointJSON | null;
  providerOperationDigest: string | null;
  status?: AutonomousEvidenceBackedCheckpointStatus;
  providerMetadataFrom?: AutonomousEvidenceBackedCheckpointJSON;
  unknownProviderOutcome?: boolean;
}): Promise<AutonomousEvidenceBackedCheckpointJSON> {
  const status = input.status ?? checkpointStatusForResult(input.result, input.providerOperationDigest);
  const observedProviderStatus = input.result.automatic?.status ?? input.result.cross_domain_run?.status ?? input.result.run?.status ?? null;
  const observedProviderResult = input.result.automatic ?? input.result.cross_domain_run ?? input.result.run ?? null;
  // An exploratory provider answer produced from incomplete evidence is deliberately quarantined:
  // it cannot become a resumable completion or authorize a provider-result rehydration path.
  const retainProviderMetadata = ["completed", "provider_reconciliation_required"].includes(status) && input.unknownProviderOutcome !== true;
  let providerStatus: AutonomousPlanAndRunStatus | null = null;
  let providerResultDigest: string | null = null;
  if (retainProviderMetadata) {
    if (input.providerMetadataFrom !== undefined) {
      providerStatus = input.providerMetadataFrom.provider_status;
      providerResultDigest = input.providerMetadataFrom.provider_result_digest;
    } else if (input.providerOperationDigest !== null && observedProviderResult && observedProviderStatus !== null) {
      providerStatus = providerResultCompleted(observedProviderStatus) ? "completed" : observedProviderStatus;
      providerResultDigest = await digestJson(observedProviderResult);
    }
  }
  const retainedOperationDigest = ["completed", "provider_reconciliation_required"].includes(status)
    ? input.providerOperationDigest
    : null;
  const providerDispatchCount = retainedOperationDigest === null
    ? 0
    : input.providerMetadataFrom?.provider_dispatch_count ?? input.previous?.provider_dispatch_count ?? 0;
  const providerDispatchHeadDigest = retainedOperationDigest === null
    ? null
    : input.providerMetadataFrom?.provider_dispatch_head_digest ?? input.previous?.provider_dispatch_head_digest ?? null;
  const payload = {
    schema: AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA,
    job_id: input.jobId,
    ...nextLineage(input.previous),
    task_digest: input.result.task_digest,
    request_digest: input.requestDigest,
    run_policy_digest: input.runPolicyDigest,
    evidence_plan_digest: input.result.execution_plan.evidence_plan_digest,
    execution_plan_digest: input.result.execution_plan.plan_digest,
    evidence_result_digest: input.result.evidence?.result_digest ?? null,
    prompt_projection_digest: input.result.prompt_context.length ? await digestJson(input.result.prompt_context) : null,
    provider_operation_digest: retainedOperationDigest,
    provider_dispatch_count: providerDispatchCount,
    provider_dispatch_head_digest: providerDispatchHeadDigest,
    provider_result_digest: providerResultDigest,
    provider_status: providerStatus,
    status,
  };
  const encoded = JSON.stringify(payload);
  if (bytes(encoded) > MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES) throw new ProviderRuntimeError("evidence-backed checkpoint exceeds its bounded size");
  return { ...payload, checkpoint_digest: await digestJson(payload), retention: RETENTION, secret_material: SECRET_MATERIAL };
}

async function checkpointForProviderDispatch(input: {
  jobId: string;
  taskDigest: string;
  requestDigest: string;
  runPolicyDigest: string;
  runMode: string;
  preflight: AutonomousEvidenceBackedRunPreflight;
  previous: AutonomousEvidenceBackedCheckpointJSON | null;
  operationDigest: string;
  providerDispatchCount: number;
  providerDispatchHeadDigest: string;
}): Promise<AutonomousEvidenceBackedCheckpointJSON> {
  const payload = {
    schema: AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA,
    job_id: input.jobId,
    ...nextLineage(input.previous),
    task_digest: input.taskDigest,
    request_digest: input.requestDigest,
    run_policy_digest: input.runPolicyDigest,
    evidence_plan_digest: input.preflight.executionPlan.evidence_plan_digest,
    execution_plan_digest: input.preflight.executionPlan.plan_digest,
    evidence_result_digest: input.preflight.evidence.result_digest,
    prompt_projection_digest: input.preflight.promptContext.length ? await digestJson(input.preflight.promptContext) : null,
    provider_operation_digest: input.operationDigest,
    provider_dispatch_count: input.providerDispatchCount,
    provider_dispatch_head_digest: input.providerDispatchHeadDigest,
    provider_result_digest: null,
    provider_status: null,
    status: "provider_in_flight" as const,
  };
  const encoded = JSON.stringify(payload);
  if (bytes(encoded) > MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES) throw new ProviderRuntimeError("evidence-backed preflight checkpoint exceeds its bounded size");
  return { ...payload, checkpoint_digest: await digestJson(payload), retention: RETENTION, secret_material: SECRET_MATERIAL };
}

/** Validate checkpoint structure, retention, and the content digest before any dispatch. */
export async function validateAutonomousEvidenceBackedCheckpoint(value: unknown): Promise<AutonomousEvidenceBackedCheckpointJSON> {
  if (!isObject(value) || value.schema !== AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA) throw new ArgumentError("evidence-backed checkpoint schema is invalid");
  exactKeys(value, ["schema", "job_id", "generation", "previous_checkpoint_digest", "task_digest", "request_digest", "run_policy_digest", "evidence_plan_digest", "execution_plan_digest", "evidence_result_digest", "prompt_projection_digest", "provider_operation_digest", "provider_dispatch_count", "provider_dispatch_head_digest", "provider_result_digest", "provider_status", "status", "checkpoint_digest", "retention", "secret_material"], "evidence-backed checkpoint");
  const jobId = boundedIdentifier("evidence-backed checkpoint job_id", value.job_id);
  const checkpointGeneration = generation(value.generation);
  const previousCheckpointDigest = optionalDigest("evidence-backed checkpoint previous_checkpoint_digest", value.previous_checkpoint_digest);
  if ((checkpointGeneration === 1) !== (previousCheckpointDigest === null)) throw new ArgumentError("evidence-backed checkpoint lineage is inconsistent");
  const taskDigest = digest("evidence-backed checkpoint task_digest", value.task_digest);
  const requestDigestValue = digest("evidence-backed checkpoint request_digest", value.request_digest);
  const runPolicyDigest = digest("evidence-backed checkpoint run_policy_digest", value.run_policy_digest);
  const evidencePlanDigest = digest("evidence-backed checkpoint evidence_plan_digest", value.evidence_plan_digest);
  const executionPlanDigest = digest("evidence-backed checkpoint execution_plan_digest", value.execution_plan_digest);
  const evidenceResultDigest = optionalDigest("evidence-backed checkpoint evidence_result_digest", value.evidence_result_digest);
  const promptProjectionDigest = optionalDigest("evidence-backed checkpoint prompt_projection_digest", value.prompt_projection_digest);
  const operationDigest = optionalDigest("evidence-backed checkpoint provider_operation_digest", value.provider_operation_digest);
  const dispatchCount = providerDispatchCount(value.provider_dispatch_count);
  const dispatchHeadDigest = optionalDigest("evidence-backed checkpoint provider_dispatch_head_digest", value.provider_dispatch_head_digest);
  const providerResultDigest = optionalDigest("evidence-backed checkpoint provider_result_digest", value.provider_result_digest);
  const providerStatus = value.provider_status === null ? null : value.provider_status as AutonomousPlanAndRunStatus;
  if (providerStatus !== null && !["completed", "children_completed", "children_partial", "approval_required", "policy_review_required", "policy_blocked", "reconciliation_required", "turn_limit_reached", "child_failed", "route_review_required", "response_review_required", "cross_domain_partial", "plan_review_required", "provider_invalid", "provider_failed", "provider_disagreement", "abstained"].includes(providerStatus)) throw new ArgumentError("evidence-backed checkpoint provider_status is invalid");
  const status = value.status as AutonomousEvidenceBackedCheckpointStatus;
  if (!["evidence_review_required", "evidence_blocked", "evidence_incomplete", "provider_pending", "provider_in_flight", "provider_reconciliation_required", "completed"].includes(status)) throw new ArgumentError("evidence-backed checkpoint status is invalid");
  if (["provider_reconciliation_required", "completed"].includes(status) && checkpointGeneration < 2) throw new ArgumentError("terminal provider checkpoint must succeed an in-flight checkpoint");
  if ((dispatchCount === 0) !== (dispatchHeadDigest === null)) throw new ArgumentError("evidence-backed checkpoint provider dispatch chain is inconsistent");
  if ((operationDigest === null) !== (dispatchCount === 0)) throw new ArgumentError("evidence-backed checkpoint provider operation and dispatch chain are inconsistent");
  if (status === "completed" && (operationDigest === null || providerResultDigest === null || providerStatus !== "completed")) throw new ArgumentError("completed evidence-backed checkpoint requires a bound operation and completed provider digest");
  if (status === "provider_pending" && (operationDigest !== null || providerResultDigest !== null || providerStatus !== null)) throw new ArgumentError("provider-pending checkpoint cannot contain provider metadata");
  if (status === "provider_in_flight" && (operationDigest === null || dispatchCount < 1 || providerResultDigest !== null || providerStatus !== null)) throw new ArgumentError("provider-in-flight checkpoint metadata is inconsistent");
  if (status === "provider_reconciliation_required") {
    if (operationDigest === null) throw new ArgumentError("provider reconciliation checkpoint requires a provider operation digest");
    if ((providerResultDigest === null) !== (providerStatus === null)) throw new ArgumentError("provider reconciliation checkpoint result metadata is inconsistent");
  }
  if (["provider_pending", "provider_in_flight", "provider_reconciliation_required", "completed"].includes(status) && evidenceResultDigest === null) throw new ArgumentError("provider checkpoint requires a settled evidence result digest");
  if (["evidence_review_required", "evidence_blocked", "evidence_incomplete"].includes(status) && operationDigest !== null) throw new ArgumentError("evidence-only checkpoint cannot contain a provider operation digest");
  if (["evidence_review_required", "evidence_blocked", "evidence_incomplete"].includes(status) && (providerResultDigest !== null || providerStatus !== null)) throw new ArgumentError("evidence-only checkpoint cannot contain provider result metadata");
  if (value.retention !== RETENTION || value.secret_material !== SECRET_MATERIAL) throw new ArgumentError("evidence-backed checkpoint retention contract is invalid");
  const checkpointDigest = digest("evidence-backed checkpoint checkpoint_digest", value.checkpoint_digest);
  const payload = { schema: AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA, job_id: jobId, generation: checkpointGeneration, previous_checkpoint_digest: previousCheckpointDigest, task_digest: taskDigest, request_digest: requestDigestValue, run_policy_digest: runPolicyDigest, evidence_plan_digest: evidencePlanDigest, execution_plan_digest: executionPlanDigest, evidence_result_digest: evidenceResultDigest, prompt_projection_digest: promptProjectionDigest, provider_operation_digest: operationDigest, provider_dispatch_count: dispatchCount, provider_dispatch_head_digest: dispatchHeadDigest, provider_result_digest: providerResultDigest, provider_status: providerStatus, status };
  if (await digestJson(payload) !== checkpointDigest) throw new ArgumentError("evidence-backed checkpoint digest is invalid");
  const encoded = JSON.stringify(value);
  if (bytes(encoded) > MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES) throw new ArgumentError("evidence-backed checkpoint exceeds its bounded size");
  return nativeStructuredClone({ ...payload, checkpoint_digest: checkpointDigest, retention: RETENTION, secret_material: SECRET_MATERIAL });
}

function assertCheckpointBinding(
  checkpoint: AutonomousEvidenceBackedCheckpointJSON,
  jobId: string,
  taskDigest: string,
  requestDigestValue: string,
  runPolicyDigestValue: string,
): void {
  if (checkpoint.job_id !== jobId || checkpoint.task_digest !== taskDigest || checkpoint.request_digest !== requestDigestValue || checkpoint.run_policy_digest !== runPolicyDigestValue) throw new ArgumentError("evidence-backed checkpoint does not match the current task, requests, run policy, or job");
}

async function assertCheckpointProjectionBinding(
  checkpoint: AutonomousEvidenceBackedCheckpointJSON,
  projection: {
    executionPlan: AutonomousEvidenceExecutionPlan;
    evidence: AutonomousEvidenceExecutionResult | null;
    promptContext: readonly AutonomousPromptChunk[];
  },
): Promise<void> {
  const evidenceResultDigest = projection.evidence?.result_digest ?? null;
  const promptProjectionDigest = projection.promptContext.length ? await digestJson(projection.promptContext) : null;
  const drift: string[] = [];
  if (checkpoint.evidence_plan_digest !== projection.executionPlan.evidence_plan_digest) drift.push("evidence plan");
  if (checkpoint.execution_plan_digest !== projection.executionPlan.plan_digest) drift.push("execution plan");
  if (checkpoint.evidence_result_digest !== evidenceResultDigest) drift.push("evidence result");
  if (checkpoint.prompt_projection_digest !== promptProjectionDigest) drift.push("prompt projection");
  if (drift.length) throw new ArgumentError(`evidence-backed checkpoint does not match the current ${drift.join(", ")}`);
}

async function assertProviderOperationBinding(
  checkpoint: AutonomousEvidenceBackedCheckpointJSON,
  input: {
    jobId: string;
    taskDigest: string;
    requestDigest: string;
    runPolicyDigest: string;
    runMode: string;
    executionPlan: AutonomousEvidenceExecutionPlan;
    evidence: AutonomousEvidenceExecutionResult | null;
    promptContext: readonly AutonomousPromptChunk[];
  },
): Promise<void> {
  if (checkpoint.provider_operation_digest === null) return;
  if (input.evidence === null) throw new ArgumentError("provider checkpoint cannot be rebound without settled evidence");
  const expected = await providerOperationDigest({
    jobId: input.jobId,
    taskDigest: input.taskDigest,
    requestDigest: input.requestDigest,
    runPolicyDigest: input.runPolicyDigest,
    runMode: input.runMode,
    preflight: {
      executionPlan: input.executionPlan,
      evidence: input.evidence,
      promptContext: input.promptContext,
    },
  });
  if (checkpoint.provider_operation_digest !== expected) throw new ArgumentError("evidence-backed checkpoint provider operation does not match the reconstructed preflight");
}

async function makeResumableResult(input: {
  jobId: string;
  status: AutonomousEvidenceBackedResumableStatus;
  result: AutonomousEvidenceBackedRunResult;
  checkpoint: AutonomousEvidenceBackedCheckpointJSON;
  providerRehydrated: boolean;
}): Promise<AutonomousEvidenceBackedResumableRun> {
  const projection = {
    schema: AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA,
    status: input.status,
    job_id: input.jobId,
    checkpoint_digest: input.checkpoint.checkpoint_digest,
    result_status: input.result.status,
    provider_rehydrated: input.providerRehydrated,
    retention: RESULT_RETENTION,
    secret_material: SECRET_MATERIAL,
  } satisfies AutonomousEvidenceBackedResumableRunProjection;
  return {
    schema: AUTONOMOUS_EVIDENCE_BACKED_RESUMABLE_RESULT_SCHEMA,
    status: input.status,
    job_id: input.jobId,
    result: input.result,
    checkpoint: nativeStructuredClone(input.checkpoint),
    provider_rehydrated: input.providerRehydrated,
    toJSON: () => nativeStructuredClone(projection),
  };
}

async function persist(
  sink: (checkpoint: AutonomousEvidenceBackedCheckpointJSON) => Promise<void> | void,
  checkpoint: AutonomousEvidenceBackedCheckpointJSON,
): Promise<AutonomousEvidenceBackedCheckpointJSON> {
  await sink(checkpoint);
  return checkpoint;
}

/**
 * Execute or resume one evidence-backed run. Evidence journals replay completed source work;
 * provider results are never replayed implicitly. A provider_pending checkpoint is known to be
 * pre-attempt and requires both a fresh provider approval and resumeProvider=true. Attempted
 * states can only consume an operation-bound, caller-rehydrated result.
 */
export async function runAutonomousEvidenceBackedResumable(
  agent: AutonomousAgent,
  task: string,
  options: AutonomousEvidenceBackedResumableExecutionOptions,
): Promise<AutonomousEvidenceBackedResumableRun> {
  assertExactResumableCore(agent);
  const initialProviderTransportGraph = captureResumableProviderTransportGraph(agent);
  const initialCoreEffectBoundary = agent.llm.effectBoundary;
  if (!options || typeof options !== "object") throw new ArgumentError("evidence-backed resumable options are malformed");
  const executionInputSnapshot = snapshotResumableExecutionInputs(options);
  const controlOptions = executionInputSnapshot.controlOptions;
  const jobId = boundedIdentifier("evidence-backed resumable jobId", controlOptions.jobId);
  const checkpointSink = controlOptions.checkpointSink;
  const checkpointCompareAndStore = controlOptions.checkpointCompareAndStore;
  const checkpointDispatchCompareAndStore = controlOptions.checkpointDispatchCompareAndStore;
  if (typeof checkpointSink !== "function") throw new ArgumentError("evidence-backed resumable execution requires checkpointSink");
  const checkpointInput = controlOptions.checkpoint === undefined
    ? null
    : clonePolicyValue("resumable checkpoint", controlOptions.checkpoint) as AutonomousEvidenceBackedCheckpointJSON;
  const rehydrateProviderRun = controlOptions.rehydrateProviderRun;
  const rehydrateAutomaticRun = controlOptions.rehydrateAutomaticRun;
  const rehydrateCrossDomainRun = controlOptions.rehydrateCrossDomainRun;
  const resumeProvider = controlOptions.resumeProvider;
  const baseOptions = executionInputSnapshot.baseOptions;
  const taskDigest = await digestJson({ task });
  const requestDigestValue = await requestDigest(baseOptions.requests);
  const runPolicyDigestValue = await runPolicyDigest(baseOptions, executionInputSnapshot.identities, executionInputSnapshot.registryProjection);
  const restored = checkpointInput === null ? null : await validateAutonomousEvidenceBackedCheckpoint(checkpointInput);
  if (restored !== null) {
    assertCheckpointBinding(restored, jobId, taskDigest, requestDigestValue, runPolicyDigestValue);
    if (!baseOptions.execute?.journal) throw new ArgumentError("evidence-backed resume requires the caller-owned evidence journal");
  }
  const runMode = baseOptions.runMode ?? "domain";
  const providerApproved = baseOptions.run?.approveProviderCall === true
    && !(runMode === "auto"
      && baseOptions.run?.planningMode === "provider"
      && baseOptions.run.planning?.approveProviderCall !== true);
  const dispatchFromPending = restored?.status === "provider_pending" && resumeProvider === true && providerApproved;
  const freshProviderDispatch = restored === null && providerApproved;
  const evidenceRecoveryTransition = restored !== null
    && ["evidence_review_required", "evidence_blocked", "evidence_incomplete"].includes(restored.status)
    && baseOptions.execute?.approveSourceDispatch === true;
  if ((freshProviderDispatch || dispatchFromPending) && typeof checkpointCompareAndStore !== "function") {
    throw new ArgumentError("provider dispatch requires atomic checkpointCompareAndStore persistence");
  }
  if ((freshProviderDispatch || dispatchFromPending) && typeof checkpointDispatchCompareAndStore !== "function") {
    throw new ArgumentError("provider dispatch requires atomic checkpointDispatchCompareAndStore persistence");
  }
  if (restored?.status === "provider_in_flight" && typeof checkpointCompareAndStore !== "function") {
    throw new ArgumentError("provider in-flight reconciliation requires atomic checkpointCompareAndStore persistence");
  }
  if (evidenceRecoveryTransition && typeof checkpointCompareAndStore !== "function") {
    throw new ArgumentError("resuming approved evidence acquisition requires atomic checkpointCompareAndStore persistence");
  }
  const restoredProviderRehydrator = runMode === "auto"
    ? rehydrateAutomaticRun
    : runMode === "cross_domain"
      ? rehydrateCrossDomainRun
      : rehydrateProviderRun;
  if (restored !== null
      && ["completed", "provider_reconciliation_required"].includes(restored.status)
      && typeof restoredProviderRehydrator === "function"
      && typeof checkpointCompareAndStore !== "function") {
    throw new ArgumentError("restored provider settlement requires atomic checkpointCompareAndStore persistence before rehydration");
  }

  let head = restored;
  const persistOrdinary = async (checkpoint: AutonomousEvidenceBackedCheckpointJSON): Promise<void> => {
    assertCheckpointSuccessor(head, checkpoint);
    const callbackCheckpoint = nativeStructuredClone(checkpoint);
    const expectedCallbackValue = canonicalJson(callbackCheckpoint);
    await checkpointSink(callbackCheckpoint);
    const validatedCallbackValue = await validateAutonomousEvidenceBackedCheckpoint(callbackCheckpoint);
    if (canonicalJson(validatedCallbackValue) !== expectedCallbackValue) {
      throw new ProviderRuntimeError("checkpoint sink mutated its commit value; reload before continuing");
    }
    head = checkpoint;
  };
  const persistAtomic = async (
    checkpoint: AutonomousEvidenceBackedCheckpointJSON,
    expected: AutonomousEvidenceBackedCheckpointJSON | null,
  ): Promise<void> => {
    if (typeof checkpointCompareAndStore !== "function") {
      throw new ArgumentError("checkpoint transition requires atomic checkpointCompareAndStore persistence");
    }
    const expectedDigest = expected?.checkpoint_digest ?? null;
    assertCheckpointSuccessor(expected, checkpoint);
    const callbackCheckpoint = nativeStructuredClone(checkpoint);
    const expectedCallbackValue = canonicalJson(callbackCheckpoint);
    const committed = await checkpointCompareAndStore(expectedDigest, callbackCheckpoint);
    if (typeof committed !== "boolean") throw new ArgumentError("checkpointCompareAndStore returned a non-boolean result");
    if (!committed) throw new ProviderRuntimeError("evidence-backed checkpoint compare-and-swap conflict; reload before continuing");
    const validatedCallbackValue = await validateAutonomousEvidenceBackedCheckpoint(callbackCheckpoint);
    if (canonicalJson(validatedCallbackValue) !== expectedCallbackValue) {
      throw new ProviderRuntimeError("checkpointCompareAndStore mutated its commit value; reload before continuing");
    }
    head = checkpoint;
  };

  const probe = async (): Promise<AutonomousEvidenceBackedRunResult> => agent.runWithReviewedEvidence(task, {
    ...baseOptions,
    run: {
      ...(baseOptions.run ?? {}),
      approveProviderCall: false,
      ...(baseOptions.run?.planning === undefined
        ? {}
        : { planning: { ...baseOptions.run.planning, approveProviderCall: false, runId: undefined } }),
      providerIdempotencyKey: undefined,
    },
  });

  const bindProbe = async (
    checkpoint: AutonomousEvidenceBackedCheckpointJSON,
    probeResult: AutonomousEvidenceBackedRunResult,
  ): Promise<void> => {
    await assertCheckpointProjectionBinding(checkpoint, {
      executionPlan: probeResult.execution_plan,
      evidence: probeResult.evidence,
      promptContext: probeResult.prompt_context,
    });
    await assertProviderOperationBinding(checkpoint, {
      jobId,
      taskDigest,
      requestDigest: requestDigestValue,
      runPolicyDigest: runPolicyDigestValue,
      runMode,
      executionPlan: probeResult.execution_plan,
      evidence: probeResult.evidence,
      promptContext: probeResult.prompt_context,
    });
  };

  const finishRestoredProvider = async (
    probeResult: AutonomousEvidenceBackedRunResult,
    checkpoint: AutonomousEvidenceBackedCheckpointJSON,
  ): Promise<AutonomousEvidenceBackedResumableRun> => {
    // Rehydrators are caller-owned and awaited. Keep both the durable checkpoint and the replayed
    // evidence graph private, and use a fresh validated snapshot for every comparison after the
    // callback so mutation of its detached context cannot rewrite persisted truth in memory.
    const stableCheckpoint = await validateAutonomousEvidenceBackedCheckpoint(checkpoint);
    await bindProbe(stableCheckpoint, probeResult);
    if (!probeResult.evidence || probeResult.evidence.status !== "completed") {
      return makeResumableResult({ jobId, status: "provider_reconciliation_required", result: probeResult, checkpoint: stableCheckpoint, providerRehydrated: false });
    }

    const context = nativeObjectFreeze({
      checkpoint: nativeStructuredClone(stableCheckpoint),
      executionPlan: nativeStructuredClone(probeResult.execution_plan.toJSON()),
      evidence: nativeStructuredClone(probeResult.evidence.toJSON()),
      promptContext: nativeStructuredClone(probeResult.prompt_context),
      providerDispatchCount: stableCheckpoint.provider_dispatch_count,
      providerDispatchHeadDigest: stableCheckpoint.provider_dispatch_head_digest,
    });
    const recoveredRaw = runMode === "auto"
      ? await rehydrateAutomaticRun?.(context) ?? null
      : runMode === "cross_domain"
        ? await rehydrateCrossDomainRun?.(context) ?? null
        : await rehydrateProviderRun?.(context) ?? null;
    const recovered = recoveredRaw === null
      ? null
      : snapshotProviderResultGraph("rehydrated provider result", recoveredRaw) as AutonomousRunResult | AutonomousAutoRunResult | AutonomousCrossDomainRunResult;
    await bindProbe(stableCheckpoint, probeResult);
    assertResumableProviderTransportGraph(agent, initialProviderTransportGraph);

    if (recovered === null) {
      return makeResumableResult({
        jobId,
        status: stableCheckpoint.status === "completed" ? "completed" : "provider_reconciliation_required",
        result: probeResult,
        checkpoint: stableCheckpoint,
        providerRehydrated: false,
      });
    }

    let finalResult: AutonomousEvidenceBackedRunResult;
    if (runMode === "auto") {
      if (!isObject(recovered) || recovered.schema !== "bioprism-typescript-autonomous-auto-run/0.1") throw new ArgumentError("rehydrated automatic run is malformed");
      if (stableCheckpoint.provider_result_digest !== null && await digestJson(recovered) !== stableCheckpoint.provider_result_digest) throw new ProviderRuntimeError("rehydrated automatic run does not match its checkpoint digest");
      finalResult = await agent.runWithReviewedEvidence(task, {
        ...baseOptions,
        run: { ...(baseOptions.run ?? {}), approveProviderCall: true },
        automaticRunOverride: recovered,
      });
    } else if (runMode === "cross_domain") {
      if (!isObject(recovered) || recovered.schema !== "bioprism-typescript-autonomous-cross-domain-result/0.1") throw new ArgumentError("rehydrated cross-domain run is malformed");
      if (stableCheckpoint.provider_result_digest !== null && await digestJson(recovered) !== stableCheckpoint.provider_result_digest) throw new ProviderRuntimeError("rehydrated cross-domain run does not match its checkpoint digest");
      finalResult = await agent.runWithReviewedEvidence(task, {
        ...baseOptions,
        run: { ...(baseOptions.run ?? {}), approveProviderCall: true },
        crossDomainRunOverride: recovered,
      });
    } else {
      if (!isObject(recovered) || recovered.schema !== "bioprism-typescript-autonomous-run/0.1") throw new ArgumentError("rehydrated provider run is malformed");
      if (stableCheckpoint.provider_result_digest !== null && await digestJson(recovered) !== stableCheckpoint.provider_result_digest) throw new ProviderRuntimeError("rehydrated provider run does not match its checkpoint digest");
      finalResult = await agent.runWithReviewedEvidence(task, {
        ...baseOptions,
        run: { ...(baseOptions.run ?? {}), approveProviderCall: true },
        providerRunOverride: recovered,
      });
    }
    await assertCheckpointProjectionBinding(stableCheckpoint, {
      executionPlan: finalResult.execution_plan,
      evidence: finalResult.evidence,
      promptContext: finalResult.prompt_context,
    });
    await assertProviderOperationBinding(stableCheckpoint, {
      jobId,
      taskDigest,
      requestDigest: requestDigestValue,
      runPolicyDigest: runPolicyDigestValue,
      runMode,
      executionPlan: finalResult.execution_plan,
      evidence: finalResult.evidence,
      promptContext: finalResult.prompt_context,
    });
    if (stableCheckpoint.status === "completed") {
      if (!providerResultCompleted(finalResult.status)) throw new ProviderRuntimeError("rehydrated completed provider result is no longer terminal");
      return makeResumableResult({ jobId, status: "completed", result: finalResult, checkpoint: stableCheckpoint, providerRehydrated: true });
    }
    const nextStatus = providerResultCompleted(finalResult.status) ? "completed" : "provider_reconciliation_required";
    const next = await checkpointForResult({
      jobId,
      requestDigest: requestDigestValue,
      runPolicyDigest: runPolicyDigestValue,
      result: finalResult,
      previous: stableCheckpoint,
      providerOperationDigest: stableCheckpoint.provider_operation_digest,
      status: nextStatus,
    });
    if ((stableCheckpoint.provider_result_digest !== null && next.provider_result_digest !== stableCheckpoint.provider_result_digest)
        || (stableCheckpoint.provider_status !== null && next.provider_status !== stableCheckpoint.provider_status)) {
      throw new ProviderRuntimeError("rehydrated provider settlement does not match checkpoint metadata");
    }
    await persistAtomic(next, stableCheckpoint);
    return makeResumableResult({ jobId, status: nextStatus, result: finalResult, checkpoint: next, providerRehydrated: true });
  };

  if (restored?.status === "provider_in_flight") {
    const probeResult = await probe();
    await bindProbe(restored, probeResult);
    const next = await checkpointForResult({
      jobId,
      requestDigest: requestDigestValue,
      runPolicyDigest: runPolicyDigestValue,
      result: probeResult,
      previous: restored,
      providerOperationDigest: restored.provider_operation_digest,
      status: "provider_reconciliation_required",
      unknownProviderOutcome: true,
    });
    await persistAtomic(next, restored);
    return makeResumableResult({ jobId, status: "provider_reconciliation_required", result: probeResult, checkpoint: next, providerRehydrated: false });
  }

  if (restored && ["completed", "provider_reconciliation_required"].includes(restored.status)) {
    return finishRestoredProvider(await probe(), restored);
  }

  if (restored && restored.status === "provider_pending" && (!resumeProvider || !providerApproved)) {
    const probeResult = await probe();
    await bindProbe(restored, probeResult);
    return makeResumableResult({ jobId, status: "provider_pending", result: probeResult, checkpoint: restored, providerRehydrated: false });
  }

  if (restored && ["evidence_review_required", "evidence_blocked", "evidence_incomplete"].includes(restored.status)) {
    const probeResult = await probe();
    if (evidenceRecoveryTransition) {
      if (probeResult.execution_plan.evidence_plan_digest !== restored.evidence_plan_digest
          || probeResult.execution_plan.plan_digest !== restored.execution_plan_digest) {
        throw new ArgumentError("resumed evidence approval changed the bound evidence or execution plan");
      }
      const next = await checkpointForResult({
        jobId,
        requestDigest: requestDigestValue,
        runPolicyDigest: runPolicyDigestValue,
        result: probeResult,
        previous: restored,
        providerOperationDigest: null,
      });
      const changed = next.status !== restored.status
        || next.evidence_result_digest !== restored.evidence_result_digest
        || next.prompt_projection_digest !== restored.prompt_projection_digest;
      if (!changed) {
        return makeResumableResult({ jobId, status: probeResult.status, result: probeResult, checkpoint: restored, providerRehydrated: false });
      }
      await persistAtomic(next, restored);
      const status = next.status === "provider_pending" ? "provider_pending" : probeResult.status;
      return makeResumableResult({ jobId, status, result: probeResult, checkpoint: next, providerRehydrated: false });
    }
    await bindProbe(restored, probeResult);
    const status = restored.status === "evidence_incomplete" ? "evidence_incomplete" : probeResult.status;
    return makeResumableResult({ jobId, status, result: probeResult, checkpoint: restored, providerRehydrated: false });
  }

  let preparedProviderPreflight: AutonomousEvidenceBackedRunPreflight | null = null;
  let inFlightCheckpoint: AutonomousEvidenceBackedCheckpointJSON | null = null;
  let providerDispatchTail: Promise<void> = nativePromiseResolve();
  let providerDispatchFailure: unknown = null;
  let latestProviderDispatch: ProviderTransportDispatchContext | null = null;
  let operationDigest: string | null = null;
  const dispatchRunOptions = { ...(baseOptions.run ?? {}) };
  const beforeProviderRun = async (preflight: AutonomousEvidenceBackedRunPreflight): Promise<void> => {
    assertResumableProviderTransportGraph(agent, initialProviderTransportGraph);
    if (agent.llm.effectBoundary !== initialCoreEffectBoundary) throw new ArgumentError("agent-bound effectBoundary changed during resumable preparation");
    assertExactEffectBoundary(dispatchRunOptions.effectBoundary, "resumable run effectBoundary");
    if (preparedProviderPreflight !== null) throw new ProviderRuntimeError("provider preflight was invoked more than once for one resumable operation");
    if (restored?.status === "provider_pending") {
      await assertCheckpointProjectionBinding(restored, preflight);
    }
    operationDigest = await providerOperationDigest({
      jobId,
      taskDigest,
      requestDigest: requestDigestValue,
      runPolicyDigest: runPolicyDigestValue,
      runMode,
      preflight,
    });
    dispatchRunOptions.providerIdempotencyKey = await providerIdempotencyKey(operationDigest);
    preparedProviderPreflight = preflight;
  };
  const beforeProviderDispatch = async (
    preflight: AutonomousEvidenceBackedRunPreflight,
    dispatch: ProviderTransportDispatchContext,
  ): Promise<void> => {
    let fenceCompleted = false;
    const queued = nativeReflectApply(nativePromiseThen, providerDispatchTail, [async () => {
      if (providerDispatchFailure !== null) throw providerDispatchFailure;
      assertResumableProviderTransportGraph(agent, initialProviderTransportGraph);
      assertResumableProviderDispatchBinding(dispatch, initialProviderTransportGraph);
      if (agent.llm.effectBoundary !== initialCoreEffectBoundary) throw new ArgumentError("agent-bound effectBoundary changed before provider dispatch");
      assertExactEffectBoundary(dispatchRunOptions.effectBoundary, "resumable run effectBoundary");
      const prepared = preparedProviderPreflight;
      const boundOperationDigest = operationDigest;
      if (prepared === null || boundOperationDigest === null) throw new ProviderRuntimeError("provider transport reached dispatch without a prepared resumable operation");
      if (prepared !== preflight) throw new ProviderRuntimeError("provider transport preflight identity changed before dispatch");
      const recomputedOperationDigest = await providerOperationDigest({
        jobId,
        taskDigest,
        requestDigest: requestDigestValue,
        runPolicyDigest: runPolicyDigestValue,
        runMode,
        preflight,
      });
      if (recomputedOperationDigest !== boundOperationDigest) throw new ArgumentError("provider transport preflight changed after its operation was bound");
      const expected = head;
      const sequence = (expected?.provider_dispatch_count ?? 0) + 1;
      const receipt = await makeProviderDispatchReceipt({
        jobId,
        operationDigest: boundOperationDigest,
        sequence,
        previousReceiptDigest: expected?.provider_dispatch_head_digest ?? null,
        dispatch,
      });
      const next = await checkpointForProviderDispatch({
        jobId,
        taskDigest,
        requestDigest: requestDigestValue,
        runPolicyDigest: runPolicyDigestValue,
        runMode,
        preflight,
        previous: expected,
        operationDigest: boundOperationDigest,
        providerDispatchCount: sequence,
        providerDispatchHeadDigest: receipt.projection.receipt_digest,
      });
      assertCheckpointSuccessor(expected, next, receipt.projection);
      if (typeof checkpointDispatchCompareAndStore !== "function") {
        throw new AutonomousEvidenceBackedDispatchTransactionError("provider dispatch transaction authority is unavailable; reload required");
      }
      let committed: unknown;
      const callbackCheckpoint = nativeStructuredClone(next);
      const expectedCallbackValue = canonicalJson(callbackCheckpoint);
      try {
        committed = await checkpointDispatchCompareAndStore(expected?.checkpoint_digest ?? null, callbackCheckpoint, receipt);
      } catch (error) {
        throw new AutonomousEvidenceBackedDispatchTransactionError("provider dispatch transaction acknowledgement failed; reload required", error);
      }
      if (committed !== true) {
        throw new AutonomousEvidenceBackedDispatchTransactionError("provider dispatch transaction was not acknowledged exactly; reload required");
      }
      let validatedCallbackValue: AutonomousEvidenceBackedCheckpointJSON;
      try {
        validatedCallbackValue = await validateAutonomousEvidenceBackedCheckpoint(callbackCheckpoint);
      } catch (error) {
        throw new AutonomousEvidenceBackedDispatchTransactionError("provider dispatch transaction mutated its commit value; reload required", error);
      }
      if (canonicalJson(validatedCallbackValue) !== expectedCallbackValue) {
        throw new AutonomousEvidenceBackedDispatchTransactionError("provider dispatch transaction mutated its commit value; reload required");
      }
      assertCheckpointSuccessor(expected, validatedCallbackValue, receipt.projection);
      try {
        await validateProviderDispatchReceipt(receipt, validatedCallbackValue);
      } catch (error) {
        throw new AutonomousEvidenceBackedDispatchTransactionError("provider dispatch transaction mutated its private receipt; reload required", error);
      }
      // The atomic store is caller-owned code and may itself yield. Recheck after its exact
      // acknowledgement so it cannot swap the endpoint/fetch/local handler behind the receipt.
      assertResumableProviderTransportGraph(agent, initialProviderTransportGraph);
      assertResumableProviderDispatchBinding(dispatch, initialProviderTransportGraph);
      latestProviderDispatch = dispatch;
      head = next;
      inFlightCheckpoint = next;
      fenceCompleted = true;
    }]) as Promise<void>;
    providerDispatchTail = nativeReflectApply(
      nativePromiseThen,
      queued,
      [() => undefined, (error: unknown) => {
        providerDispatchFailure = error;
      }],
    ) as Promise<void>;
    await queued;
    if (!fenceCompleted) {
      throw new AutonomousEvidenceBackedDispatchTransactionError(
        "provider dispatch transaction did not execute its private fence; reload required",
      );
    }
  };

  const shouldDispatch = providerApproved && (restored === null || (restored.status === "provider_pending" && resumeProvider === true));
  const result = shouldDispatch
    ? await agent.runWithReviewedEvidence(task, {
      ...baseOptions,
      run: dispatchRunOptions,
      beforeProviderRun,
      beforeProviderDispatch,
    })
    : await probe();
  // Outcome observers are caller-owned and run after transport. Refuse to settle a completed
  // checkpoint if they changed the provider graph or invalidated the selected credential binding.
  assertResumableProviderTransportGraph(agent, initialProviderTransportGraph);
  if (latestProviderDispatch !== null) {
    assertResumableProviderDispatchBinding(latestProviderDispatch, initialProviderTransportGraph);
  }
  if (inFlightCheckpoint !== null) {
    await assertCheckpointProjectionBinding(inFlightCheckpoint, {
      executionPlan: result.execution_plan,
      evidence: result.evidence,
      promptContext: result.prompt_context,
    });
    await assertProviderOperationBinding(inFlightCheckpoint, {
      jobId,
      taskDigest,
      requestDigest: requestDigestValue,
      runPolicyDigest: runPolicyDigestValue,
      runMode,
      executionPlan: result.execution_plan,
      evidence: result.evidence,
      promptContext: result.prompt_context,
    });
  } else if (restored?.status === "provider_pending") {
    await assertCheckpointProjectionBinding(restored, {
      executionPlan: result.execution_plan,
      evidence: result.evidence,
      promptContext: result.prompt_context,
    });
    return makeResumableResult({ jobId, status: "provider_pending", result, checkpoint: restored, providerRehydrated: false });
  }
  const predecessor = inFlightCheckpoint ?? head;
  const finalCheckpoint = await checkpointForResult({
    jobId,
    requestDigest: requestDigestValue,
    runPolicyDigest: runPolicyDigestValue,
    result,
    previous: predecessor,
    providerOperationDigest: inFlightCheckpoint === null ? null : operationDigest,
  });
  if (inFlightCheckpoint !== null) await persistAtomic(finalCheckpoint, inFlightCheckpoint);
  else await persistOrdinary(finalCheckpoint);
  const status: AutonomousEvidenceBackedResumableStatus = finalCheckpoint.status === "provider_pending"
    ? "provider_pending"
    : finalCheckpoint.status === "provider_in_flight"
      ? "provider_in_flight"
      : finalCheckpoint.status === "provider_reconciliation_required"
        ? "provider_reconciliation_required"
        : result.status;
  return makeResumableResult({ jobId, status, result, checkpoint: finalCheckpoint, providerRehydrated: false });
}

export async function runAutonomousEvidenceBackedResumableWithCheckpoint(
  agent: AutonomousAgent,
  task: string,
  options: Omit<AutonomousEvidenceBackedResumableExecutionOptions, "checkpoint"> & { checkpoint: AutonomousEvidenceBackedCheckpointJSON },
): Promise<AutonomousEvidenceBackedResumableRun> {
  return runAutonomousEvidenceBackedResumable(agent, task, options);
}

/** In-memory checkpoint adapter for local workers and tests. */
export class InMemoryAutonomousEvidenceBackedCheckpointStore implements AutonomousEvidenceBackedCheckpointStore {
  private checkpoint: AutonomousEvidenceBackedCheckpointJSON | null;
  readonly #dispatchReceipts = new Map<string, AutonomousEvidenceBackedProviderDispatchReceipt>();

  constructor(initial?: AutonomousEvidenceBackedCheckpointJSON | null) {
    this.checkpoint = initial === undefined || initial === null ? null : nativeStructuredClone(initial);
  }

  async read(): Promise<AutonomousEvidenceBackedCheckpointJSON | null> {
    return this.checkpoint === null ? null : validateAutonomousEvidenceBackedCheckpoint(this.checkpoint);
  }

  async write(checkpoint: AutonomousEvidenceBackedCheckpointJSON): Promise<void> {
    this.checkpoint = nativeStructuredClone(await validateAutonomousEvidenceBackedCheckpoint(checkpoint));
  }

  async writeIfUnchanged(expectedCheckpointDigest: string | null, checkpoint: AutonomousEvidenceBackedCheckpointJSON): Promise<boolean> {
    const validated = nativeStructuredClone(await validateAutonomousEvidenceBackedCheckpoint(checkpoint));
    const previous = this.checkpoint === null ? null : await validateAutonomousEvidenceBackedCheckpoint(this.checkpoint);
    if ((this.checkpoint?.checkpoint_digest ?? null) !== expectedCheckpointDigest
        || (previous?.checkpoint_digest ?? null) !== expectedCheckpointDigest) return false;
    assertCheckpointSuccessor(previous, validated);
    // No await may appear between checking the head and replacing it: JavaScript's run-to-
    // completion semantics make this compare-and-swap atomic for one in-memory event loop.
    this.checkpoint = validated;
    return true;
  }

  async writeDispatchIfUnchanged(
    expectedCheckpointDigest: string | null,
    checkpoint: AutonomousEvidenceBackedCheckpointJSON,
    receipt: AutonomousEvidenceBackedProviderDispatchReceipt,
  ): Promise<boolean> {
    const validated = nativeStructuredClone(await validateAutonomousEvidenceBackedCheckpoint(checkpoint));
    const validatedReceipt = await validateProviderDispatchReceipt(receipt, validated);
    const previous = this.checkpoint === null ? null : await validateAutonomousEvidenceBackedCheckpoint(this.checkpoint);
    if ((this.checkpoint?.checkpoint_digest ?? null) !== expectedCheckpointDigest
        || (previous?.checkpoint_digest ?? null) !== expectedCheckpointDigest) return false;
    assertCheckpointSuccessor(previous, validated, validatedReceipt.projection);
    const retainedReceipt = new PrivateAutonomousEvidenceBackedProviderDispatchReceipt(
      validatedReceipt.projection,
      validatedReceipt.providerIdempotencyKey,
    );
    const existing = nativeReflectApply(
      nativeMapGet,
      this.#dispatchReceipts,
      [validatedReceipt.projection.receipt_digest],
    ) as AutonomousEvidenceBackedProviderDispatchReceipt | undefined;
    if (existing !== undefined && (
      canonicalJson(existing.toJSON()) !== canonicalJson(retainedReceipt.toJSON())
      || existing.providerIdempotencyKey() !== retainedReceipt.providerIdempotencyKey()
    )) {
      throw new ProviderRuntimeError("provider dispatch receipt digest collides with different private data");
    }
    // Store the receipt before publishing the checkpoint head. The final checkpoint assignment is
    // deliberately the only operation after the receipt write, so an exception cannot expose an
    // in-flight head whose exact private receipt was not retained atomically with it.
    nativeReflectApply(nativeMapSet, this.#dispatchReceipts, [validatedReceipt.projection.receipt_digest, retainedReceipt]);
    this.checkpoint = validated;
    return true;
  }

  /** Privileged reconciliation lookup; callers must not serialize the returned raw-key receipt. */
  providerDispatchReceipt(receiptDigest: string): AutonomousEvidenceBackedProviderDispatchReceipt | null {
    digest("provider dispatch receipt lookup digest", receiptDigest);
    const receipt = nativeReflectApply(nativeMapGet, this.#dispatchReceipts, [receiptDigest]) as AutonomousEvidenceBackedProviderDispatchReceipt | undefined;
    return receipt === undefined
      ? null
      : new PrivateAutonomousEvidenceBackedProviderDispatchReceipt(receipt.toJSON(), receipt.providerIdempotencyKey());
  }

  /** Ordered public projections for one selected receipt chain; raw provider keys stay private. */
  providerDispatchReceiptProjections(headDigest: string | null = null): AutonomousEvidenceBackedProviderDispatchReceiptProjection[] {
    const selectedHead = headDigest ?? this.checkpoint?.provider_dispatch_head_digest ?? null;
    if (selectedHead === null) return [];
    digest("provider dispatch receipt head", selectedHead);
    const reversed: AutonomousEvidenceBackedProviderDispatchReceiptProjection[] = [];
    let cursor: string | null = selectedHead;
    while (cursor !== null) {
      const receipt = nativeReflectApply(nativeMapGet, this.#dispatchReceipts, [cursor]) as AutonomousEvidenceBackedProviderDispatchReceipt | undefined;
      if (receipt === undefined) throw new ProviderRuntimeError("provider dispatch receipt chain is incomplete");
      const projection = receipt.toJSON();
      if (projection.receipt_digest !== cursor) throw new ProviderRuntimeError("provider dispatch receipt chain identity is invalid");
      reversed.push(projection);
      if (reversed.length > MAX_AUTONOMOUS_EVIDENCE_BACKED_PROVIDER_DISPATCHES) {
        throw new ProviderRuntimeError("provider dispatch receipt chain exceeds its bound");
      }
      cursor = projection.previous_receipt_digest;
    }
    const rows = reversed.reverse();
    for (let index = 0; index < rows.length; index += 1) {
      const row = rows[index]!;
      if (row.sequence !== index + 1
          || row.previous_receipt_digest !== (index === 0 ? null : rows[index - 1]!.receipt_digest)
          || row.job_id !== rows[0]!.job_id
          || row.provider_operation_digest !== rows[0]!.provider_operation_digest) {
        throw new ProviderRuntimeError("provider dispatch receipt chain is inconsistent");
      }
    }
    if (this.checkpoint?.provider_dispatch_head_digest === selectedHead
        && rows.length !== this.checkpoint.provider_dispatch_count) {
      throw new ProviderRuntimeError("provider dispatch receipt chain count does not match checkpoint");
    }
    return rows.map((row) => nativeStructuredClone(row));
  }
}

/** Browser/Node text adapter with strict JSON and byte bounds. */
export class JsonAutonomousEvidenceBackedCheckpointStore implements AutonomousEvidenceBackedCheckpointStore {
  constructor(protected readonly store: AutonomousEvidenceBackedCheckpointTextStore) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("evidence-backed JSON checkpoint store is malformed");
  }

  async read(): Promise<AutonomousEvidenceBackedCheckpointJSON | null> {
    const encoded = await this.store.read();
    if (encoded === null) return null;
    if (typeof encoded !== "string" || bytes(encoded) > MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES) throw new ArgumentError("evidence-backed checkpoint text exceeds its bound");
    let parsed: unknown;
    try {
      parsed = JSON.parse(encoded);
    } catch {
      throw new ArgumentError("evidence-backed checkpoint text is invalid JSON");
    }
    if (canonicalJson(parsed) !== encoded) throw new ArgumentError("evidence-backed checkpoint text is not canonical");
    return validateAutonomousEvidenceBackedCheckpoint(parsed);
  }

  async write(checkpoint: AutonomousEvidenceBackedCheckpointJSON): Promise<void> {
    const validated = await validateAutonomousEvidenceBackedCheckpoint(checkpoint);
    const encoded = canonicalJson(validated);
    if (bytes(encoded) > MAX_AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_BYTES) throw new ArgumentError("evidence-backed checkpoint text exceeds its bound");
    await this.store.write(encoded);
  }
}

/** Text adapter that exposes compare-and-swap rather than pretending ordinary writes are atomic. */
export class TransactionalJsonAutonomousEvidenceBackedCheckpointStore extends JsonAutonomousEvidenceBackedCheckpointStore {
  readonly writeDispatchIfUnchanged?: (
    expectedCheckpointDigest: string | null,
    checkpoint: AutonomousEvidenceBackedCheckpointJSON,
    receipt: AutonomousEvidenceBackedProviderDispatchReceipt,
  ) => Promise<boolean>;

  constructor(private readonly transactionalStore: AutonomousEvidenceBackedTransactionalCheckpointTextStore) {
    super(transactionalStore);
    if (typeof transactionalStore.writeIfUnchanged !== "function") throw new ArgumentError("transactional evidence-backed checkpoint store requires writeIfUnchanged");
    if (typeof transactionalStore.writeDispatchIfUnchanged === "function") {
      this.writeDispatchIfUnchanged = async (expectedCheckpointDigest, checkpoint, receipt) => {
        const validated = await validateAutonomousEvidenceBackedCheckpoint(checkpoint);
        const validatedReceipt = await validateProviderDispatchReceipt(receipt, validated);
        const previous = await this.read();
        if ((previous?.checkpoint_digest ?? null) !== expectedCheckpointDigest) return false;
        assertCheckpointSuccessor(previous, validated, validatedReceipt.projection);
        const checkpointValue = canonicalJson(validated);
        const privateReceiptValue = canonicalJson({
          schema: "bioprism-typescript-autonomous-evidence-backed-provider-dispatch-private/0.1",
          projection: validatedReceipt.projection,
          provider_idempotency_key: validatedReceipt.providerIdempotencyKey,
        });
        const committed = await transactionalStore.writeDispatchIfUnchanged!(expectedCheckpointDigest, checkpointValue, privateReceiptValue);
        if (typeof committed !== "boolean") throw new ArgumentError("transactional evidence-backed dispatch store returned a non-boolean result");
        return committed;
      };
    }
  }

  async writeIfUnchanged(expectedCheckpointDigest: string | null, checkpoint: AutonomousEvidenceBackedCheckpointJSON): Promise<boolean> {
    const validated = await validateAutonomousEvidenceBackedCheckpoint(checkpoint);
    const previous = await this.read();
    if ((previous?.checkpoint_digest ?? null) !== expectedCheckpointDigest) return false;
    assertCheckpointSuccessor(previous, validated);
    const encoded = canonicalJson(validated);
    const committed = await this.transactionalStore.writeIfUnchanged(expectedCheckpointDigest, encoded);
    if (typeof committed !== "boolean") throw new ArgumentError("transactional evidence-backed checkpoint store returned a non-boolean result");
    return committed;
  }
}

/** Restart-aware controller with serialized local operations and optional CAS fencing. */
export class AutonomousEvidenceBackedController {
  private checkpoint: AutonomousEvidenceBackedCheckpointJSON | null = null;
  private expectedCheckpointDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();
  private reloadRequired = false;
  private controllerStatus: AutonomousEvidenceBackedControllerProjection["status"] = "empty";

  constructor(readonly agent: AutonomousAgent, readonly jobId: string, readonly persistence: AutonomousEvidenceBackedCheckpointStore) {
    assertExactResumableCore(agent);
    boundedIdentifier("evidence-backed controller jobId", jobId);
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("evidence-backed controller persistence is malformed");
  }

  async restore(): Promise<AutonomousEvidenceBackedControllerProjection> {
    return this.enqueue(async () => {
      const stored = await this.persistence.read();
      this.checkpoint = stored === null ? null : await validateAutonomousEvidenceBackedCheckpoint(stored);
      this.expectedCheckpointDigest = this.checkpoint?.checkpoint_digest ?? null;
      this.reloadRequired = false;
      this.controllerStatus = this.checkpoint === null ? "empty" : "restored";
      return this.projection();
    });
  }

  async run(task: string, options: AutonomousEvidenceBackedControllerRunOptions): Promise<AutonomousEvidenceBackedControllerRun> {
    return this.enqueue(async () => {
      if (!options || typeof options !== "object") throw new ArgumentError("evidence-backed controller run options are malformed");
      const controllerOptions = snapshotControlEnvelope(
        options as unknown as Record<string, unknown>,
        CONTROLLER_RUN_OPTION_KEYS,
        "evidence-backed controller run options",
      ) as unknown as AutonomousEvidenceBackedControllerRunOptions;
      const stored = this.reloadRequired ? await this.persistence.read() : this.checkpoint ?? await this.persistence.read();
      this.checkpoint = stored === null ? null : await validateAutonomousEvidenceBackedCheckpoint(stored);
      this.expectedCheckpointDigest = this.checkpoint?.checkpoint_digest ?? null;
      this.reloadRequired = false;
      const invalidateCachedHead = (): void => {
        this.reloadRequired = true;
        this.checkpoint = null;
        this.expectedCheckpointDigest = null;
      };
      const commit = async (
        expectedCheckpointDigest: string | null,
        checkpoint: AutonomousEvidenceBackedCheckpointJSON,
      ): Promise<boolean> => {
        if (this.expectedCheckpointDigest !== expectedCheckpointDigest) {
          invalidateCachedHead();
          return false;
        }
        invalidateCachedHead();
        const committed = await this.persistence.writeIfUnchanged!(expectedCheckpointDigest, checkpoint);
        if (committed !== true) return false;
        this.checkpoint = checkpoint;
        this.expectedCheckpointDigest = checkpoint.checkpoint_digest;
        this.reloadRequired = false;
        this.controllerStatus = checkpoint.status === "completed"
          ? "completed"
          : checkpoint.status === "provider_pending"
            ? "provider_pending"
            : checkpoint.status === "provider_in_flight"
              ? "provider_in_flight"
              : checkpoint.status === "provider_reconciliation_required"
                ? "provider_reconciliation_required"
                : checkpoint.status === "evidence_incomplete"
                  ? "evidence_incomplete"
                  : "flushed";
        return true;
      };
      const commitDispatch = async (
        expectedCheckpointDigest: string | null,
        checkpoint: AutonomousEvidenceBackedCheckpointJSON,
        receipt: AutonomousEvidenceBackedProviderDispatchReceipt,
      ): Promise<boolean> => {
        if (this.expectedCheckpointDigest !== expectedCheckpointDigest) {
          invalidateCachedHead();
          return false;
        }
        invalidateCachedHead();
        const committed = await this.persistence.writeDispatchIfUnchanged!(expectedCheckpointDigest, checkpoint, receipt);
        if (committed !== true) return false;
        this.checkpoint = checkpoint;
        this.expectedCheckpointDigest = checkpoint.checkpoint_digest;
        this.reloadRequired = false;
        this.controllerStatus = "provider_in_flight";
        return true;
      };
      const result = await runAutonomousEvidenceBackedResumable(this.agent, task, {
        ...controllerOptions,
        jobId: this.jobId,
        ...(this.checkpoint === null ? {} : { checkpoint: this.checkpoint }),
        checkpointSink: async (checkpoint) => {
          if (typeof this.persistence.writeIfUnchanged === "function") {
            const committed = await commit(this.expectedCheckpointDigest, checkpoint);
            if (!committed) throw new ArgumentError("evidence-backed checkpoint compare-and-swap conflict; reload before continuing");
          } else {
            await this.persistence.write(checkpoint);
            this.checkpoint = checkpoint;
            this.expectedCheckpointDigest = checkpoint.checkpoint_digest;
            this.controllerStatus = checkpoint.status === "provider_pending"
              ? "provider_pending"
              : checkpoint.status === "evidence_incomplete"
                ? "evidence_incomplete"
                : "flushed";
          }
        },
        ...(typeof this.persistence.writeIfUnchanged === "function" ? { checkpointCompareAndStore: commit } : {}),
        ...(typeof this.persistence.writeDispatchIfUnchanged === "function" ? { checkpointDispatchCompareAndStore: commitDispatch } : {}),
      });
      this.checkpoint = result.checkpoint;
      this.expectedCheckpointDigest = result.checkpoint.checkpoint_digest;
      this.controllerStatus = result.status === "completed" ? "completed" : result.status === "provider_pending" ? "provider_pending" : result.status === "provider_in_flight" ? "provider_in_flight" : result.status === "provider_reconciliation_required" ? "provider_reconciliation_required" : result.status === "evidence_incomplete" ? "evidence_incomplete" : "flushed";
      return { controller: this.projection(), run: result };
    });
  }

  projection(): AutonomousEvidenceBackedControllerProjection {
    return {
      schema: "bioprism-typescript-autonomous-evidence-backed-controller/0.1",
      status: this.controllerStatus,
      job_id: this.jobId,
      checkpoint_digest: this.checkpoint?.checkpoint_digest ?? null,
      persisted: true,
      retention: CONTROLLER_RETENTION,
      secret_material: SECRET_MATERIAL,
    };
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}
