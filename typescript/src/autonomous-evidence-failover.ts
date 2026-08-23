import { ArgumentError } from "./errors.js";
import type { AutonomousDomainName } from "./autonomous.js";
import {
  AutonomousEvidenceAdapterRegistry,
  type AutonomousEvidenceAdapterManifest,
} from "./autonomous-evidence-adapters.js";
import {
  AutonomousEvidenceAdapterSelectionPlan,
  type AutonomousEvidenceAdapterSelectionRow,
} from "./autonomous-evidence-adapter-selection.js";
import {
  AutonomousEvidenceRetryPolicy,
  classifyAutonomousEvidenceAcquisitionError,
  createAutonomousEvidenceRetryingAcquirer,
  type AutonomousEvidenceRetryAcquirerOptions,
  type AutonomousEvidenceRetryAttempt,
  type AutonomousEvidenceRetryClassifier,
} from "./autonomous-evidence-retry.js";
import {
  createAutonomousEvidenceSourceGuard,
  type AutonomousEvidenceSourceDescriptorContext,
  type AutonomousEvidenceSourceDescriptorInput,
  type AutonomousEvidenceSourceLedger,
  type AutonomousEvidenceSourcePolicy,
} from "./autonomous-evidence-source.js";
import type { AutonomousEvidenceAcquirer, AutonomousEvidenceAcquisitionContext } from "./autonomous-evidence-runtime.js";
import type { AutonomousEvidenceProviderContractRegistry } from "./autonomous-evidence-provider-contract.js";
import type { JsonObject, JsonValue } from "./types.js";

/** Explicit fallback policy for digest-bound evidence adapter candidates. */
export const AUTONOMOUS_EVIDENCE_FAILOVER_POLICY_SCHEMA = "bioprism-typescript-autonomous-evidence-failover-policy/0.1" as const;
export const AUTONOMOUS_EVIDENCE_FAILOVER_EVENT_SCHEMA = "bioprism-typescript-autonomous-evidence-failover-event/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_FAILOVERS = 7;

export interface AutonomousEvidenceFailoverPolicyJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_FAILOVER_POLICY_SCHEMA;
  max_failovers: number;
  retry_policy: JsonObject;
  execution: "caller_controlled_reviewed_candidate_failover;no_fuzzy_selection";
  retention: "metadata_only_candidate_identity_and_failure_class";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceFailoverEvent extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_FAILOVER_EVENT_SCHEMA;
  domain: AutonomousDomainName;
  candidate_id: string;
  candidate_manifest_digest: string;
  candidate_rank: number;
  status: "candidate_failed" | "fallback_started" | "candidate_succeeded" | "failover_exhausted";
  failure_class: string | null;
  retryable: boolean;
  failovers_used: number;
  remaining_candidates: number;
  retention: "metadata_only;candidate_identity_and_failure_class";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceFailoverPolicyOptions {
  maxFailovers?: number;
  retryPolicy?: AutonomousEvidenceRetryPolicy;
}

export interface AutonomousEvidenceFailoverAcquirerOptions extends AutonomousEvidenceFailoverPolicyOptions {
  /** Optional provider/source contract registry enforced for every primary and fallback attempt. */
  providerContracts?: AutonomousEvidenceProviderContractRegistry;
  classify?: AutonomousEvidenceRetryClassifier;
  observeFailover?: (event: AutonomousEvidenceFailoverEvent) => void | Promise<void>;
  observeAttempt?: (attempt: AutonomousEvidenceRetryAttempt) => void | Promise<void>;
  clock?: () => number;
  sleep?: AutonomousEvidenceRetryAcquirerOptions["sleep"];
  /** Optional strict provenance/freshness gate applied inside each reviewed candidate route. */
  sourceBoundary?: {
    policy: AutonomousEvidenceSourcePolicy;
    ledger?: AutonomousEvidenceSourceLedger;
    sourceKind?: string;
    describeSource: (input: AutonomousEvidenceSourceDescriptorContext) => AutonomousEvidenceSourceDescriptorInput | Promise<AutonomousEvidenceSourceDescriptorInput>;
  };
}

export class AutonomousEvidenceFailoverPolicy {
  readonly max_failovers: number;
  readonly retry_policy: AutonomousEvidenceRetryPolicy;

  constructor(options: AutonomousEvidenceFailoverPolicyOptions = {}) {
    if (options.retryPolicy !== undefined && !(options.retryPolicy instanceof AutonomousEvidenceRetryPolicy)) throw new ArgumentError("evidence failover retry policy is malformed");
    this.max_failovers = integer("evidence failover maxFailovers", options.maxFailovers ?? 0, 0, MAX_AUTONOMOUS_EVIDENCE_FAILOVERS);
    this.retry_policy = options.retryPolicy ?? new AutonomousEvidenceRetryPolicy();
  }

  toJSON(): AutonomousEvidenceFailoverPolicyJSON {
    return {
      schema: AUTONOMOUS_EVIDENCE_FAILOVER_POLICY_SCHEMA,
      max_failovers: this.max_failovers,
      retry_policy: this.retry_policy.toJSON(),
      execution: "caller_controlled_reviewed_candidate_failover;no_fuzzy_selection",
      retention: "metadata_only_candidate_identity_and_failure_class",
      secret_material: "never_returned",
    };
  }
}

function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} is outside its bound`);
  return value as number;
}

function candidateOrder(row: AutonomousEvidenceAdapterSelectionRow): string[] {
  return row.candidate_ids
    .map((adapterId, index) => ({ adapterId, eligible: row.candidate_eligible[index] === true, score: row.candidate_scores[index] ?? 0 }))
    .filter((candidate) => candidate.eligible)
    .sort((left, right) => right.score - left.score || left.adapterId.localeCompare(right.adapterId))
    .map((candidate) => candidate.adapterId);
}

function routeFor(registry: AutonomousEvidenceAdapterRegistry, row: AutonomousEvidenceAdapterSelectionRow, adapterId: string): AutonomousEvidenceAdapterManifest {
  return registry.resolve(row.domain, adapterId);
}

function eventFor(
  row: AutonomousEvidenceAdapterSelectionRow,
  manifest: AutonomousEvidenceAdapterManifest,
  candidateRank: number,
  status: AutonomousEvidenceFailoverEvent["status"],
  failureClass: string | null,
  retryable: boolean,
  failoversUsed: number,
  remainingCandidates: number,
): AutonomousEvidenceFailoverEvent {
  return {
    schema: AUTONOMOUS_EVIDENCE_FAILOVER_EVENT_SCHEMA,
    domain: row.domain,
    candidate_id: manifest.adapter_id,
    candidate_manifest_digest: manifest.manifest_digest,
    candidate_rank: candidateRank,
    status,
    failure_class: failureClass,
    retryable,
    failovers_used: failoversUsed,
    remaining_candidates: remainingCandidates,
    retention: "metadata_only;candidate_identity_and_failure_class",
    secret_material: "never_returned",
  };
}

/**
 * Route a reviewed selection plan through bounded retries and, only when explicitly budgeted,
 * score-ordered eligible fallback candidates. The plan is verified before every acquisition.
 */
export function createAutonomousEvidenceAdapterFailoverAcquirer(
  registry: AutonomousEvidenceAdapterRegistry,
  plan: AutonomousEvidenceAdapterSelectionPlan | unknown,
  options: AutonomousEvidenceFailoverAcquirerOptions = {},
): AutonomousEvidenceAcquirer {
  if (!(registry instanceof AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("evidence failover requires a typed adapter registry");
  const typedPlan = plan instanceof AutonomousEvidenceAdapterSelectionPlan ? plan : AutonomousEvidenceAdapterSelectionPlan.fromJSON(plan);
  typedPlan.verify(registry);
  const policy = new AutonomousEvidenceFailoverPolicy(options);
  if (options.classify !== undefined && typeof options.classify !== "function") throw new ArgumentError("evidence failover classifier is malformed");
  if (options.observeFailover !== undefined && typeof options.observeFailover !== "function") throw new ArgumentError("evidence failover observer is malformed");
  if (options.sourceBoundary !== undefined) {
    if (!options.providerContracts) throw new ArgumentError("source-bound failover requires a provider contract registry");
    if (!options.sourceBoundary.policy || typeof options.sourceBoundary.describeSource !== "function") throw new ArgumentError("source-bound failover requires a policy and source descriptor callback");
  }
  const rows = new Map<AutonomousDomainName, AutonomousEvidenceAdapterSelectionRow>(typedPlan.rows.map((row) => [row.domain, row]));
  return {
    acquire: async (context: AutonomousEvidenceAcquisitionContext): Promise<JsonValue> => {
      const row = rows.get(context?.requirement?.domain);
      if (!row) throw new ArgumentError(`evidence failover plan does not cover ${context?.requirement?.domain ?? "the requested domain"}`);
      if (row.status !== "selected") throw new ArgumentError(`evidence failover selection is incomplete for ${row.domain}`);
      const candidates = candidateOrder(row);
      if (candidates.length === 0) throw new ArgumentError(`evidence failover selection has no eligible candidates for ${row.domain}`);
      let failoversUsed = 0;
      let lastError: unknown = null;
      for (let candidateIndex = 0; candidateIndex < candidates.length && candidateIndex <= policy.max_failovers; candidateIndex += 1) {
        const candidateId = candidates[candidateIndex]!;
        const manifest = routeFor(registry, row, candidateId);
        let candidateAcquirer = options.providerContracts
          ? options.providerContracts.createAcquirerForAdapter(candidateId, row.domain)
          : registry.createAcquirer({ adapterIdForDomain: { [row.domain]: candidateId } as Partial<Record<AutonomousDomainName, string>> });
        if (options.sourceBoundary) {
          const contract = options.providerContracts!.contractForAdapter(candidateId, row.domain);
          const sourceKind = options.sourceBoundary.sourceKind ?? (contract.source_kinds.length === 1 ? contract.source_kinds[0]! : (() => { throw new ArgumentError(`source-bound failover requires sourceKind for ${contract.contract_id}`); })());
          candidateAcquirer = createAutonomousEvidenceSourceGuard(candidateAcquirer, {
            contract,
            adapterId: candidateId,
            domain: row.domain,
            sourceKind,
            policy: options.sourceBoundary.policy,
            ...(options.sourceBoundary.ledger === undefined ? {} : { ledger: options.sourceBoundary.ledger }),
            describeSource: options.sourceBoundary.describeSource,
          });
        }
        const resilient = createAutonomousEvidenceRetryingAcquirer(candidateAcquirer, {
          policy: policy.retry_policy,
          classify: options.classify,
          observe: options.observeAttempt,
          clock: options.clock,
          sleep: options.sleep,
        });
        try {
          const value = await resilient.acquire(context);
          await options.observeFailover?.(eventFor(row, manifest, candidateIndex + 1, "candidate_succeeded", null, false, failoversUsed, Math.max(0, candidates.length - candidateIndex - 1)));
          return value;
        } catch (error) {
          lastError = error;
          const classification = options.classify ? options.classify(error) : classifyAutonomousEvidenceAcquisitionError(error);
          const permitted = policy.retry_policy.permits(classification);
          const remaining = Math.max(0, candidates.length - candidateIndex - 1);
          const canFailover = permitted && failoversUsed < policy.max_failovers && remaining > 0;
          const status: AutonomousEvidenceFailoverEvent["status"] = canFailover ? "fallback_started" : permitted ? "failover_exhausted" : "candidate_failed";
          await options.observeFailover?.(eventFor(row, manifest, candidateIndex + 1, status, classification.failure_class, classification.retryable, failoversUsed, remaining));
          if (!canFailover) throw error;
          failoversUsed += 1;
        }
      }
      throw lastError ?? new ArgumentError("evidence failover exhausted unexpectedly");
    },
  };
}
