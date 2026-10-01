/** Exact domain-tool catalogue binding and capability portfolio planning. */

import { ArgumentError, ProviderRuntimeError } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES } from "./autonomous-domains.js";
import type { AutonomousDomainName } from "./autonomous-domains.js";
import { boundedIdentifier, boundedText } from "./autonomous-validation.js";
import { ToolCatalogue, digestJson } from "./tooling.js";
import type { ProviderTool } from "./llm.js";
import type { JsonObject, ToolDefinition } from "./types.js";
import {
  AUTONOMOUS_TOOL_RISK_ORDER,
  AUTONOMOUS_TOOL_SELECTION_POLICY,
  AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA,
  autonomousToolRiskAllowed,
  boundedToolSelectionNumber,
  capabilityCandidateRanking,
  capabilityCandidateScore,
  compareCapabilityScores,
  normalizeAutonomousToolSelectionState,
  taskRelevanceTokens,
  toolSelectionArmFor,
  toolSelectionUtility,
  autonomousDomainToolBindingSupportsStage as bindingSupportsStage,
} from "./autonomous-tool-selection.js";
import type {
  AutonomousCapabilityPlan,
  AutonomousCapabilityPlanCoverage,
  AutonomousCapabilityPlanOmission,
  AutonomousCapabilitySelectionStatus,
  AutonomousDomainProfile,
  AutonomousDomainToolBinding,
  AutonomousDomainToolPlan,
  AutonomousDomainToolProfile,
  AutonomousToolRiskClass,
  AutonomousToolSelectionState,
  AutonomousWorkflow,
  AutonomousWorkflowStage,
  AutonomousWorkflowToolContext,
} from "./autonomous-agent-contracts.js";

export const AUTONOMOUS_DOMAIN_TOOL_PLAN_SCHEMA = "bioprism-typescript-autonomous-domain-tool-plan/0.1" as const;
export const AUTONOMOUS_CAPABILITY_PLAN_SCHEMA = "bioprism-typescript-autonomous-capability-plan/0.1" as const;

/** Bind exact reviewed domain tools to a live catalogue without turning metadata into authority. */
export class AutonomousDomainToolRegistryBase {
  readonly catalogue: ToolCatalogue;
  readonly profiles: readonly AutonomousDomainToolProfile[];
  readonly digest: string;
  private readonly bindingsByDomain = new Map<AutonomousDomainName, Map<string, AutonomousDomainToolBinding>>();
  private readonly workflowsByDomain = new Map<AutonomousDomainName, AutonomousDomainProfile>();

  protected constructor(catalogue: ToolCatalogue, profiles: readonly AutonomousDomainToolProfile[], workflowProfiles: readonly AutonomousDomainProfile[], digest: string) {
    this.catalogue = catalogue;
    this.profiles = profiles;
    this.digest = digest;
    for (const profile of profiles) this.bindingsByDomain.set(profile.domain, new Map(profile.bindings.map((binding) => [binding.name, binding])));
    for (const profile of workflowProfiles) this.workflowsByDomain.set(profile.domain, profile);
  }

  profile(domain: string): AutonomousDomainToolProfile {
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError(`unsupported autonomous domain: ${domain}`);
    const profile = this.profiles.find((candidate) => candidate.domain === domain);
    if (!profile) throw new ArgumentError(`domain tool profile is unavailable: ${domain}`);
    return profile;
  }

  binding(name: string, domains: readonly string[] = AUTONOMOUS_DOMAIN_NAMES): AutonomousDomainToolBinding | null {
    boundedIdentifier("domain tool name", name);
    for (const domain of domains) {
      if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError(`unsupported autonomous domain: ${domain}`);
      const binding = this.bindingsByDomain.get(domain as AutonomousDomainName)?.get(name);
      if (binding) return binding;
    }
    return null;
  }

  toolsFor(domains: readonly string[] = AUTONOMOUS_DOMAIN_NAMES): ProviderTool[] {
    const names = new Set<string>();
    for (const domain of domains) for (const binding of this.profile(domain).bindings) if (binding.read_only || binding.approval_required) names.add(binding.name);
    return [...names].sort().flatMap((name) => {
      try {
        const definition = this.catalogue.get(name);
        return [{ name: definition.name, description: definition.description, parameters: definition.inputSchema }];
      } catch { return []; }
    });
  }

  async plan(domains: readonly string[] = AUTONOMOUS_DOMAIN_NAMES): Promise<AutonomousDomainToolPlan> {
    const selectedProfiles = domains.map((domain) => this.profile(domain));
    const curated = new Map<string, AutonomousDomainToolBinding>();
    for (const profile of selectedProfiles) for (const binding of profile.bindings) if (!curated.has(`${profile.domain}/${binding.name}`)) curated.set(`${profile.domain}/${binding.name}`, binding);
    const available: string[] = [];
    const missing: string[] = [];
    const review: AutonomousDomainToolBinding[] = [];
    const proposed: AutonomousDomainToolBinding[] = [];
    for (const binding of curated.values()) {
      if (binding.approval_required) review.push(binding);
      try { this.catalogue.get(binding.name); if (binding.read_only) proposed.push(binding); else review.push(binding); if (binding.read_only) available.push(binding.name); } catch { missing.push(binding.name); }
    }
    const unique = (values: readonly string[]) => [...new Set(values)].sort();
    const availableNames = unique(available);
    const missingNames = unique(missing);
    const reviewRows = review.filter((binding, index, rows) => rows.findIndex((row) => row.name === binding.name && row.domains[0] === binding.domains[0]) === index);
    const coverage = selectedProfiles.map((profile) => {
      const required = profile.bindings.filter((binding) => binding.read_only);
      const availableCount = required.filter((binding) => this.has(binding.name)).length;
      return { domain: profile.domain, required_tool_count: required.length, available_tool_count: availableCount, missing_tools: required.filter((binding) => !this.has(binding.name)).map((binding) => binding.name), review_required_tools: profile.bindings.filter((binding) => binding.approval_required && this.has(binding.name)).map((binding) => binding.name), coverage_ratio: required.length === 0 ? 1 : Number((availableCount / required.length).toFixed(12)) };
    });
    const knownNames = new Set([...curated.values()].map((binding) => binding.name));
    const unclassified = this.catalogue.definitions.map((definition) => definition.name).filter((name) => !knownNames.has(name)).sort();
    const descriptor = { schema: AUTONOMOUS_DOMAIN_TOOL_PLAN_SCHEMA, catalogue_digest: this.catalogue.digest, profile_digest: this.digest, domains: selectedProfiles.map((profile) => profile.domain), available_curated_tools: availableNames, missing_curated_tools: missingNames, review_required_tools: unique(reviewRows.map((binding) => binding.name)), unclassified_tools: unclassified, coverage, proposed_bindings: proposed, review_bindings: reviewRows, execution: "metadata_only; registration_is_not_authorization" as const, secret_material: "never_returned" as const };
    return { ...descriptor, plan_digest: await digestJson(descriptor) };
  }

  /**
   * Select the smallest deterministic live tool portfolio that can cover the reviewed workflow
   * stages. Task text is used only locally for ranking and is retained as a digest; activation
   * allow-lists narrow candidates but never widen them. Missing stages remain provider-only or
   * explicitly unavailable instead of being hidden behind an optimistic coverage claim.
   */
  async planForTask(
    task: string,
    options: { domains?: readonly string[]; capability?: string; allowedTools?: readonly string[]; maxTools?: number; readOnlyOnly?: boolean; maxRiskClass?: AutonomousToolRiskClass; toolSelectionState?: AutonomousToolSelectionState | null; exploration?: number } = {},
  ): Promise<AutonomousCapabilityPlan> {
    const taskText = boundedText("capability plan task", task, 32_000);
    const domains = options.domains === undefined ? this.profiles.map((profile) => profile.domain) : [...options.domains].map((domain) => boundedIdentifier("capability plan domain", domain) as AutonomousDomainName);
    if (!domains.length || domains.length > AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError(`capability plan domains must contain between 1 and ${AUTONOMOUS_DOMAIN_NAMES.length} entries`);
    if (new Set(domains).size !== domains.length) throw new ArgumentError("capability plan domains contain duplicates");
    const maxTools = options.maxTools ?? 32;
    if (!Number.isSafeInteger(maxTools) || maxTools < 1 || maxTools > 128) throw new ArgumentError("capability plan maxTools must be between 1 and 128");
    const requestedCapabilities = options.capability === undefined ? [] : [boundedText("capability plan capability", options.capability, 128)];
    const allowedTools = options.allowedTools === undefined ? null : new Set([...options.allowedTools].map((name) => boundedIdentifier("capability plan allowed tool", name)));
    if (allowedTools && allowedTools.size > 512) throw new ArgumentError("capability plan allowed tools exceed their bound");
    const maxRiskClass = options.maxRiskClass ?? "high_impact_effect";
    if (!AUTONOMOUS_TOOL_RISK_ORDER.includes(maxRiskClass)) throw new ArgumentError("capability plan maxRiskClass is unsupported");
    const toolSelectionState = normalizeAutonomousToolSelectionState(options.toolSelectionState);
    const exploration = options.exploration ?? 0.15;
    boundedToolSelectionNumber("capability plan exploration", exploration, 0, 1);
    const totalPulls = toolSelectionState.arms.reduce((sum, arm) => sum + arm.pulls, 0);
    const selectedProfiles = domains.map((domain) => {
      const profile = this.workflowsByDomain.get(domain);
      if (!profile) throw new ArgumentError(`domain tool profile is unavailable: ${domain}`);
      return profile;
    });
    const tokens = taskRelevanceTokens(taskText);
    const stageRows: Array<{
      domain: AutonomousDomainName;
      stage: AutonomousWorkflowStage;
      bindings: AutonomousDomainToolBinding[];
      liveBindings: AutonomousDomainToolBinding[];
      eligible: AutonomousDomainToolBinding[];
      ranked: AutonomousDomainToolBinding[];
    }> = [];
    for (const profile of selectedProfiles) {
      const toolProfile = this.profile(profile.domain);
      for (const stage of profile.workflow.stages) {
        const bindings = toolProfile.bindings.filter((binding) => bindingSupportsStage(profile, stage, binding));
        const liveBindings = bindings.filter((binding) => this.has(binding.name));
        // Planning must not propose an adapter that the reviewed stage will reject at
        // execution time. Read-only stages admit only read-only bindings, and stages without
        // an approval gate cannot carry an approval-gated binding. This keeps capability
        // selection and stage admission aligned instead of generating guaranteed refusals.
         const eligible = liveBindings.filter((binding) => (
           (options.readOnlyOnly !== true || binding.read_only)
           && (!stage.read_only || binding.read_only)
           && (stage.approval_required || !binding.approval_required)
           && autonomousToolRiskAllowed(binding.risk_class, maxRiskClass)
           && (allowedTools === null || allowedTools.has(binding.name))
           && !toolSelectionArmFor(toolSelectionState, profile.domain, stage, binding)?.disabled
         ));
         const ranked = [...eligible].sort((left, right) => compareCapabilityScores(
           capabilityCandidateScore(tokens, requestedCapabilities, stage, left, profile.domain, toolSelectionState, totalPulls, exploration),
           capabilityCandidateScore(tokens, requestedCapabilities, stage, right, profile.domain, toolSelectionState, totalPulls, exploration),
         ) || left.name.localeCompare(right.name));
        stageRows.push({ domain: profile.domain, stage, bindings, liveBindings, eligible, ranked });
      }
    }
    const preferred = new Map<string, { binding: AutonomousDomainToolBinding; score: readonly [number, number, number, number, number]; domain: AutonomousDomainName }>();
    for (const row of stageRows) {
      const candidate = row.ranked[0];
      if (!candidate) continue;
       const score = capabilityCandidateScore(tokens, requestedCapabilities, row.stage, candidate, row.domain, toolSelectionState, totalPulls, exploration);
      const prior = preferred.get(candidate.name);
      if (!prior || compareCapabilityScores(prior.score, score) > 0 || (compareCapabilityScores(prior.score, score) === 0 && row.domain.localeCompare(prior.domain) < 0)) preferred.set(candidate.name, { binding: candidate, score, domain: row.domain });
    }
    const rankedNames = [...preferred.entries()].sort((left, right) => compareCapabilityScores(left[1].score, right[1].score) || left[0].localeCompare(right[0])).map(([name]) => name);
    const selectedNames = new Set<string>();
    const selectedToolOrder: string[] = [];
    for (const row of stageRows) {
      const candidate = row.ranked.find((binding) => !selectedNames.has(binding.name)) ?? row.ranked[0];
      if (candidate && selectedNames.size < maxTools) {
        selectedNames.add(candidate.name);
        if (!selectedToolOrder.includes(candidate.name)) selectedToolOrder.push(candidate.name);
      }
    }
    for (const name of rankedNames) {
      if (selectedNames.size >= maxTools) break;
      selectedNames.add(name);
      if (!selectedToolOrder.includes(name)) selectedToolOrder.push(name);
    }
    const selectedToolNames = [...selectedNames].sort();
    const selectedBindings = selectedToolNames.flatMap((name) => {
      const row = preferred.get(name);
      return row ? [row.binding] : [];
    });
    const coverage: AutonomousCapabilityPlanCoverage[] = stageRows.map((row) => {
      const selected = row.ranked.find((binding) => selectedNames.has(binding.name));
      const status: AutonomousCapabilitySelectionStatus = selected
        ? "selected"
        : row.bindings.length === 0
          ? "provider_only"
            : row.liveBindings.length === 0
              ? "catalogue_missing"
              : allowedTools !== null && row.eligible.length === 0 && row.liveBindings.every((binding) => !allowedTools.has(binding.name))
                ? "activation_required"
                : row.liveBindings.every((binding) => !autonomousToolRiskAllowed(binding.risk_class, maxRiskClass))
                  ? "risk_budget_blocked"
                : row.liveBindings.some((binding) => toolSelectionArmFor(toolSelectionState, row.domain, row.stage, binding)?.disabled)
                  ? "learning_disabled"
                : "capacity_limited";
      const selectedArm = selected ? toolSelectionArmFor(toolSelectionState, row.domain, row.stage, selected) : null;
      const candidateRanking = capabilityCandidateRanking(tokens, requestedCapabilities, row.stage, row.liveBindings, row.domain, toolSelectionState, totalPulls, exploration, allowedTools, options.readOnlyOnly === true, maxRiskClass);
      const selectedRank = selected === undefined ? null : candidateRanking.find((candidate) => candidate.tool === selected.name)?.rank ?? null;
      const selectionRationale = selected
        ? selectedRank === 1 ? "highest_ranked_eligible_candidate" as const : "portfolio_reuse_lower_rank_candidate" as const
        : status === "provider_only" ? "no_reviewed_binding_for_stage" as const
          : status === "catalogue_missing" ? "no_live_catalogue_binding" as const
            : status === "activation_required" ? "activation_or_allowlist_required" as const
              : status === "learning_disabled" ? "all_candidates_learning_disabled" as const
                : status === "risk_budget_blocked" ? "risk_budget_excluded_all_candidates" as const
                  : "portfolio_capacity_limit" as const;
      return { domain: row.domain, stage_id: row.stage.id, required_capabilities: [...row.stage.required_capabilities], candidate_tool_names: row.liveBindings.map((binding) => binding.name).sort(), selected_tool: selected?.name ?? null, selected_capability: selected?.capability ?? null, approval_required: selected?.approval_required ?? false, selected_arm_id: selectedArm?.arm_id ?? null, selection_utility: selected ? toolSelectionUtility(selectedArm, totalPulls, exploration) : null, candidate_ranking: candidateRanking, selection_rationale: selectionRationale, status };
    });
    const allLiveBindings = selectedProfiles.flatMap((profile) => this.profile(profile.domain).bindings.filter((binding) => this.has(binding.name)));
    const bindingDomains = new Map<string, Set<AutonomousDomainName>>();
    const bindingByName = new Map<string, AutonomousDomainToolBinding>();
    for (const binding of allLiveBindings) {
      bindingDomains.set(binding.name, (bindingDomains.get(binding.name) ?? new Set()).add(binding.domains[0] ?? "cross_domain"));
      bindingByName.set(binding.name, bindingByName.get(binding.name) ?? binding);
    }
    const omissions: AutonomousCapabilityPlanOmission[] = [...bindingByName.keys()].filter((name) => !selectedNames.has(name)).sort().slice(0, 512).map((name) => {
      const binding = bindingByName.get(name)!;
      const disabled = selectedProfiles.some((profile) => profile.workflow.stages.some((stage) => bindingSupportsStage(profile, stage, binding) && toolSelectionArmFor(toolSelectionState, profile.domain, stage, binding)?.disabled));
      return { name, domains: [...(bindingDomains.get(name) ?? new Set())].sort(), capability: binding.capability, reason: allowedTools !== null && !allowedTools.has(name) ? "activation_required" : !autonomousToolRiskAllowed(binding.risk_class, maxRiskClass) ? "risk_budget_limited" : disabled ? "learning_disabled" : preferred.has(name) ? "capacity_limited" : "not_required_for_reviewed_workflow" };
    });
    const missingTools = [...new Set(selectedProfiles.flatMap((profile) => this.profile(profile.domain).bindings.filter((binding) => !this.has(binding.name)).map((binding) => binding.name)))].sort();
    const selectionLearning = { schema: AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA, generation: toolSelectionState.generation, state_digest: await digestJson(toolSelectionState), exploration, total_pulls: totalPulls, known_arm_count: toolSelectionState.arms.length, disabled_arm_count: toolSelectionState.arms.filter((arm) => arm.disabled).length, retention: "value_only;tool_arguments_outputs_prompts_and_credentials_never_returned" as const };
    const selectionConstraints = { max_risk_class: maxRiskClass, read_only_only: options.readOnlyOnly === true, allowed_tools_digest: allowedTools === null ? null : await digestJson([...allowedTools].sort()), policy: AUTONOMOUS_TOOL_SELECTION_POLICY };
    const descriptor = { schema: AUTONOMOUS_CAPABILITY_PLAN_SCHEMA, task_digest: await digestJson({ task: taskText }), catalogue_digest: this.catalogue.digest, profile_digest: this.digest, domains: [...domains], requested_capabilities: requestedCapabilities, max_tools: maxTools, selected_tool_names: selectedToolNames, selected_tool_order: selectedToolOrder, selected_bindings: selectedBindings, approval_required_tools: selectedBindings.filter((binding) => binding.approval_required).map((binding) => binding.name).sort(), missing_tools: missingTools, omissions, coverage, selection_learning: selectionLearning, selection_constraints: selectionConstraints, selection_policy: AUTONOMOUS_TOOL_SELECTION_POLICY, execution: "metadata_only; no_provider_or_tool_calls" as const, authorization: "selection_does_not_authorize_tools_or_effects" as const, secret_material: "never_returned" as const };
    return { ...descriptor, plan_digest: await digestJson(descriptor) };
  }

  has(name: string): boolean {
    try { this.catalogue.get(name); return true; } catch { return false; }
  }

  callPlan(name: string, arguments_: JsonObject, domains: readonly string[]): { binding: AutonomousDomainToolBinding; definition: ToolDefinition; arguments: JsonObject; schemaDigest: string } {
    const binding = this.binding(name, domains);
    if (!binding) throw new ProviderRuntimeError(`tool ${name} is not approved for the selected autonomous domain`);
    const plan = this.catalogue.plan(name, arguments_);
    return { binding, definition: plan.definition, arguments: plan.arguments, schemaDigest: plan.schemaDigest };
  }

  /**
   * Re-authorize a call against the exact reviewed workflow stage. This is deliberately
   * stricter than domain lookup: a registered tool is not admitted merely because it belongs
   * to the same broad domain.
   */
  stagePlan(
    name: string,
    arguments_: JsonObject,
    context: AutonomousWorkflowToolContext,
  ): { binding: AutonomousDomainToolBinding; definition: ToolDefinition; arguments: JsonObject; schemaDigest: string; workflow: AutonomousWorkflow; stage: AutonomousWorkflowStage; profile: AutonomousDomainProfile } {
    const workflowProfile = this.workflowsByDomain.get(context.domain);
    if (!workflowProfile) throw new ProviderRuntimeError(`workflow execution is unavailable for domain ${context.domain}`);
    const workflow = workflowProfile.workflow;
    if (context.workflow_id !== workflow.workflow_id || context.workflow_digest !== workflow.workflow_digest) throw new ProviderRuntimeError("autonomous tool workflow identity does not match the reviewed workflow");
    const stage = workflow.stages.find((candidate) => candidate.id === context.stage_id);
    if (!stage) throw new ProviderRuntimeError(`autonomous tool stage ${context.stage_id} is not in the reviewed workflow`);
    const binding = this.binding(name, [context.domain]);
    if (!binding) throw new ProviderRuntimeError(`tool ${name} is not approved for the selected autonomous domain`);
    if (context.selected_tool_names !== undefined && !context.selected_tool_names.includes(name)) throw new ProviderRuntimeError(`tool ${name} is outside the selected stage execution portfolio`);
    if (!bindingSupportsStage(workflowProfile, stage, binding)) throw new ProviderRuntimeError(`tool ${name} does not satisfy workflow stage ${stage.id}`);
    if (stage.read_only && !binding.read_only) throw new ProviderRuntimeError(`effectful tool ${name} is not permitted by read-only workflow stage ${stage.id}`);
    if (!stage.approval_required && binding.approval_required) throw new ProviderRuntimeError(`tool ${name} requires approval not declared by workflow stage ${stage.id}`);
    const plan = this.catalogue.plan(name, arguments_);
    return { binding, definition: plan.definition, arguments: plan.arguments, schemaDigest: plan.schemaDigest, workflow, stage, profile: workflowProfile };
  }
}
