/** Authorized live execution for exact reviewed domain-tool bindings. */

import { isObject, ProviderRuntimeError } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES } from "./autonomous-domains.js";
import type { AutonomousDomainName } from "./autonomous-domains.js";
import { AutonomousAuthorizationError, type AutonomousAuthorizationContext } from "./autonomous-authorization.js";
import { AutonomousEffectBoundary, AutonomousEffectReconciliationRequiredError } from "./autonomous-effects.js";
import type { AutonomousExecutionController } from "./autonomous-execution.js";
import { boundedDigest, boundedIdentifier, bytes } from "./autonomous-validation.js";
import { autonomousWorkflowStageContractDigest } from "./autonomous-tool-selection.js";
import { AutonomousDomainToolRegistryBase } from "./autonomous-domain-tool-registry.js";
import { canonicalJson, digestJson } from "./tooling.js";
import type { ProviderToolCall, ProviderToolResult } from "./llm.js";
import type { JsonObject, JsonValue } from "./types.js";
import type {
  AutonomousDomainToolBinding,
  AutonomousDomainToolExecutionReceipt,
  AutonomousWorkflow,
  AutonomousWorkflowStage,
  AutonomousWorkflowToolContext,
  DomainToolApprover,
  DomainToolExecutor,
} from "./autonomous-agent-contracts.js";
import type { AutonomousDomainToolRegistry } from "./autonomous.js";

export const AUTONOMOUS_DOMAIN_TOOL_REGISTRY_SCHEMA = "bioprism-typescript-autonomous-domain-tool-registry/0.1" as const;

function assertSafeToolArguments(value: unknown, depth = 0): void {
  if (depth > 32) throw new ProviderRuntimeError("autonomous tool arguments are too deeply nested");
  if (Array.isArray(value)) { for (const child of value) assertSafeToolArguments(child, depth + 1); return; }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (["apikey", "authorization", "bearer", "credential", "password", "secret", "token", "privatekey", "refreshtoken"].includes(normalized)) throw new ProviderRuntimeError("autonomous tool arguments cannot contain credential-shaped fields");
      assertSafeToolArguments(child, depth + 1);
    }
  }
}

function normalizeWorkflowToolContext(value: unknown): AutonomousWorkflowToolContext {
  if (!isObject(value)) throw new ProviderRuntimeError("autonomous workflow tool context is malformed");
  if (Object.keys(value).some((key) => !["domain", "workflow_id", "workflow_digest", "stage_id", "stage_plan_digest", "stage_contract_digest", "selected_tool_names"].includes(key))) throw new ProviderRuntimeError("autonomous workflow tool context contains unsupported fields");
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(value.domain as AutonomousDomainName)) throw new ProviderRuntimeError("autonomous workflow tool context domain is unsupported");
  const workflowId = boundedIdentifier("autonomous workflow tool context workflow_id", value.workflow_id);
  const workflowDigest = value.workflow_digest;
  if (typeof workflowDigest !== "string" || !/^[0-9a-f]{64}$/.test(workflowDigest)) throw new ProviderRuntimeError("autonomous workflow tool context workflow_digest is malformed");
  const stageId = boundedIdentifier("autonomous workflow tool context stage_id", value.stage_id);
  const stagePlanDigest = value.stage_plan_digest === undefined ? undefined : boundedDigest("autonomous workflow tool context stage_plan_digest", value.stage_plan_digest);
  const stageContractDigest = value.stage_contract_digest === undefined ? undefined : boundedDigest("autonomous workflow tool context stage_contract_digest", value.stage_contract_digest);
  const selectedToolNames = value.selected_tool_names === undefined
    ? undefined
    : (() => {
      if (!Array.isArray(value.selected_tool_names) || value.selected_tool_names.length > 512) throw new ProviderRuntimeError("autonomous workflow tool context selected_tool_names exceed their bound");
      const names = value.selected_tool_names.map((name, index) => boundedIdentifier(`autonomous workflow tool context selected_tool_names[${index}]`, name));
      if (new Set(names).size !== names.length) throw new ProviderRuntimeError("autonomous workflow tool context selected_tool_names contain duplicates");
      return names;
    })();
  return {
    domain: value.domain as AutonomousDomainName,
    workflow_id: workflowId,
    workflow_digest: workflowDigest,
    stage_id: stageId,
    ...(stagePlanDigest === undefined ? {} : { stage_plan_digest: stagePlanDigest }),
    ...(stageContractDigest === undefined ? {} : { stage_contract_digest: stageContractDigest }),
    ...(selectedToolNames === undefined ? {} : { selected_tool_names: selectedToolNames }),
  };
}

/** Execute only exact live tools, with schema preflight and approval for every effectful row. */
export class AutonomousDomainToolRuntime {
  readonly registry: AutonomousDomainToolRegistry;
  readonly executor: DomainToolExecutor;
  readonly approver?: DomainToolApprover;
  readonly effectBoundary?: AutonomousEffectBoundary;
  private readonly receipts: AutonomousDomainToolExecutionReceipt[] = [];

  constructor(registry: AutonomousDomainToolRegistry, executor: DomainToolExecutor, options: { approver?: DomainToolApprover; effectBoundary?: AutonomousEffectBoundary } = {}) {
    if (!(registry instanceof AutonomousDomainToolRegistryBase)) throw new ProviderRuntimeError("autonomous domain tool runtime requires a registry");
    if (typeof executor !== "function") throw new ProviderRuntimeError("autonomous domain tool executor must be callable");
    if (options.effectBoundary !== undefined && !(options.effectBoundary instanceof AutonomousEffectBoundary)) throw new ProviderRuntimeError("autonomous domain tool effectBoundary is malformed");
    this.registry = registry;
    this.executor = executor;
    this.approver = options.approver;
    this.effectBoundary = options.effectBoundary;
  }

  async authorizeAndExecute(calls: readonly ProviderToolCall[], options: { domains: readonly string[]; approveEffects?: boolean; execution?: AutonomousExecutionController; effectBoundary?: AutonomousEffectBoundary; workflowContext?: AutonomousWorkflowToolContext; authorizationContext?: AutonomousAuthorizationContext } ): Promise<ProviderToolResult[]> {
    if (!Array.isArray(calls) || calls.length > 128) throw new ProviderRuntimeError("autonomous tool call count is outside its bounds");
    const workflowContext = options.workflowContext === undefined ? null : normalizeWorkflowToolContext(options.workflowContext);
    if (workflowContext && !options.domains.includes(workflowContext.domain)) throw new ProviderRuntimeError("autonomous workflow tool context domain is outside the selected domains");
    const results: ProviderToolResult[] = [];
    for (const call of calls) {
      const started = Date.now();
      let planned: ReturnType<AutonomousDomainToolRegistry["stagePlan"]> | ReturnType<AutonomousDomainToolRegistry["callPlan"]> | undefined;
      let stageContractDigest: string | null = null;
      let requiredEvidenceOutputs: string[] = [];
      let stageApprovalRequired = false;
      const makeReceipt = (extra: JsonObject = {}): AutonomousDomainToolExecutionReceipt => ({
        schema: AUTONOMOUS_DOMAIN_TOOL_REGISTRY_SCHEMA,
        receipt_kind: "tool_execution_receipt",
        call_id: call.id,
        execution_id: options.execution?.state.execution_id ?? null,
        domain: workflowContext?.domain ?? (planned && "binding" in planned ? planned.binding.domains[0] ?? null : null),
        workflow_id: workflowContext?.workflow_id ?? null,
        workflow_digest: workflowContext?.workflow_digest ?? null,
        stage_id: workflowContext?.stage_id ?? null,
        stage_contract_digest: stageContractDigest,
        ...(workflowContext?.stage_plan_digest === undefined ? {} : { stage_plan_digest: workflowContext.stage_plan_digest }),
        required_evidence_outputs: [...requiredEvidenceOutputs],
        evidence_status: "tool_execution_only",
        does_not_claim: ["tool dispatch is not proof that the domain task succeeded", "a result digest is not a claim about external-world truth", "stage evidence outputs still require evaluator review"],
        tool: call.name,
        capability: planned?.binding.capability ?? null,
        duration_ms: Math.max(0, Date.now() - started),
        secret_material: "never_returned",
        ...extra,
      } as AutonomousDomainToolExecutionReceipt);
      try {
        assertSafeToolArguments(call.arguments);
        if (workflowContext) {
          const stagePlanned = this.registry.stagePlan(call.name, call.arguments, workflowContext);
          planned = stagePlanned;
          requiredEvidenceOutputs = [...stagePlanned.stage.evidence_outputs];
          stageApprovalRequired = stagePlanned.stage.approval_required;
          stageContractDigest = await autonomousWorkflowStageContractDigest(stagePlanned.workflow, stagePlanned.stage.id);
          if (workflowContext.stage_contract_digest !== undefined && workflowContext.stage_contract_digest !== stageContractDigest) throw new ProviderRuntimeError("autonomous workflow stage contract is stale or does not match the reviewed workflow");
        } else {
          planned = this.registry.callPlan(call.name, call.arguments, options.domains);
        }
        if (!planned) throw new ProviderRuntimeError("autonomous tool call was not planned");
        const executable = planned;
        let approved = executable.binding.read_only && !executable.binding.approval_required && !stageApprovalRequired;
        if (!approved && options.approveEffects === true) approved = this.approver ? await this.approver(executable.binding, call) : true;
        if (!approved) {
          const receipt = makeReceipt({ status: "approval_required", schema_digest: executable.schemaDigest, arguments_digest: await digestJson(executable.arguments), effect: executable.binding.risk_class });
          this.receipts.push(receipt);
          results.push({ callId: call.id, approved: false, isError: true, content: { status: "approval_required", tool: call.name, receipt_digest: await digestJson(receipt) } });
          continue;
        }
        const effectBoundary = options.effectBoundary ?? this.effectBoundary;
        if (options.authorizationContext && executable.binding.read_only) {
          options.authorizationContext.authorizeOperation({
            operation: "tool_execution",
            domains: options.domains,
            capability: executable.binding.capability,
            riskClass: executable.binding.risk_class,
            resourceDigest: await digestJson({ tool: executable.binding.name, call_id: call.id, arguments_digest: await digestJson(executable.arguments) }),
          });
        }
        let value: JsonValue;
        if (effectBoundary && !executable.binding.read_only) {
          value = await effectBoundary.execute({ execution_id: options.execution?.state.execution_id ?? null, tool: call.name, call_id: call.id, risk_class: executable.binding.risk_class, arguments: executable.arguments }, async (effectContext) => this.executor(executable.binding, executable.arguments, effectContext), { execution: options.execution, authorizationContext: options.authorizationContext, authorizationDomains: options.domains, authorizationCapability: executable.binding.capability });
        } else {
          if (options.authorizationContext && !executable.binding.read_only) {
            options.authorizationContext.authorizeOperation({ operation: "effect_dispatch", domains: options.domains, capability: executable.binding.capability, riskClass: executable.binding.risk_class, resourceDigest: await digestJson({ tool: executable.binding.name, call_id: call.id, arguments_digest: await digestJson(executable.arguments) }) });
          }
          value = await this.executor(executable.binding, executable.arguments);
        }
        assertSafeToolArguments(value);
        const encoded = canonicalJson(value);
        if (bytes(encoded) > 1_000_000) throw new ProviderRuntimeError("autonomous tool result exceeds its bounded size");
        const receipt = makeReceipt({ status: "executed", schema_digest: executable.schemaDigest, arguments_digest: await digestJson(executable.arguments), result_digest: await digestJson(value), effect: executable.binding.risk_class });
        this.receipts.push(receipt);
        results.push({ callId: call.id, approved: true, content: value });
      } catch (unknownError) {
        const error = unknownError instanceof Error ? unknownError : new Error("tool execution failed");
        if (unknownError instanceof AutonomousEffectReconciliationRequiredError) {
          const receipt = makeReceipt({ status: "reconciliation_required", effect_id: unknownError.effectId, idempotency_key: unknownError.idempotencyKey });
          this.receipts.push(receipt);
          results.push({ callId: call.id, approved: false, isError: true, content: { status: "reconciliation_required", tool: call.name, effect_id: unknownError.effectId, idempotency_key: unknownError.idempotencyKey, receipt_digest: await digestJson(receipt), secret_material: "never_returned" } });
          continue;
        }
        if (unknownError instanceof AutonomousAuthorizationError) {
          const receipt = makeReceipt({ status: "authorization_required" });
          this.receipts.push(receipt);
          results.push({ callId: call.id, approved: false, isError: true, content: { status: "authorization_required", tool: call.name, secret_material: "never_returned", receipt_digest: await digestJson(receipt) } });
          continue;
        }
        const receipt = makeReceipt({ status: "execution_failed", error_class: error.constructor.name });
        this.receipts.push(receipt);
        results.push({ callId: call.id, approved: false, isError: true, content: { status: "execution_failed", tool: call.name, error_class: error.constructor.name, receipt_digest: await digestJson(receipt) } });
      }
    }
    return results;
  }

  receiptsSnapshot(): AutonomousDomainToolExecutionReceipt[] {
    return this.receipts.map((receipt) => ({ ...receipt }));
  }
}
