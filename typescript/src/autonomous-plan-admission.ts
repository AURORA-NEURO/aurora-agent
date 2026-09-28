//! Digest-bound admission for provider-assisted plan refinements.
//!
//! Model-generated orderings remain proposals until this module validates identity, exact
//! membership, focus bounds, and dependency order against the caller-owned base plan.

import { ProviderRuntimeError, isObject } from "./errors.js";
import { digestJson } from "./tooling.js";
import type {
  AutonomousAcceptedCrossDomainPlan,
  AutonomousAcceptedPlan,
  AutonomousCrossDomainBlueprint,
  AutonomousTaskBlueprint,
  AutonomousWorkflowStage,
} from "./autonomous-agent-contracts.js";
import type {
  AutonomousCrossDomainPlanRefinementResult,
  AutonomousPlanRefinementResult,
} from "./types.js";

export function validatePlanningWorkflow(stages: readonly AutonomousWorkflowStage[]): string[] {
  if (!Array.isArray(stages) || stages.length === 0 || stages.length > 64) throw new ProviderRuntimeError("provider planning workflow stages are outside their bounds");
  const stageIds = stages.map((stage) => {
    if (!isObject(stage) || typeof stage.id !== "string" || !stage.id.trim()) throw new ProviderRuntimeError("provider planning workflow stage is malformed");
    return stage.id;
  });
  if (new Set(stageIds).size !== stageIds.length) throw new ProviderRuntimeError("provider planning workflow stages are duplicated");
  const known = new Set(stageIds);
  const indegree = new Map(stageIds.map((id) => [id, 0]));
  const dependents = new Map(stageIds.map((id) => [id, [] as string[]]));
  for (const stage of stages) {
    if (!Array.isArray(stage.depends_on) || stage.depends_on.some((dependency: string) => typeof dependency !== "string" || !known.has(dependency) || dependency === stage.id)) {
      throw new ProviderRuntimeError("provider planning workflow dependencies are not closed");
    }
    if (new Set(stage.depends_on).size !== stage.depends_on.length) throw new ProviderRuntimeError("provider planning workflow dependencies are duplicated");
    indegree.set(stage.id, stage.depends_on.length);
    for (const dependency of stage.depends_on) dependents.get(dependency)!.push(stage.id);
  }
  const ready = stageIds.filter((id) => indegree.get(id) === 0);
  let visited = 0;
  for (let cursor = 0; cursor < ready.length; cursor++) {
    const id = ready[cursor]!;
    visited++;
    for (const dependent of dependents.get(id)!) {
      const remaining = indegree.get(dependent)! - 1;
      indegree.set(dependent, remaining);
      if (remaining === 0) ready.push(dependent);
    }
  }
  if (visited !== stageIds.length) throw new ProviderRuntimeError("provider planning workflow dependencies contain a cycle");
  return stageIds;
}

/** Validate an accepted single-domain proposal before it can shape direct provider invocation. */
export async function acceptedAutonomousPlan(
  blueprint: AutonomousTaskBlueprint,
  refinement: AutonomousPlanRefinementResult | undefined,
): Promise<AutonomousAcceptedPlan | null> {
  if (refinement === undefined) return null;
  if (!isObject(refinement) || refinement.status !== "completed" || refinement.review_required !== false) throw new ProviderRuntimeError("only a completed, non-review plan refinement may be accepted");
  if (refinement.task_digest !== blueprint.task_digest) throw new ProviderRuntimeError("accepted plan task does not match the blueprint");
  if (refinement.base_plan_digest !== await digestJson(blueprint.plan)) throw new ProviderRuntimeError("accepted plan base does not match the blueprint");
  if (refinement.workflow_digest !== blueprint.workflow.workflow_digest) throw new ProviderRuntimeError("accepted plan workflow does not match the blueprint");
  const stages = blueprint.workflow.stages;
  const stageIds = validatePlanningWorkflow(stages);
  if (!Array.isArray(refinement.priority_stage_ids) || !Array.isArray(refinement.focus_stage_ids)) throw new ProviderRuntimeError("accepted plan stage identifiers are malformed");
  const priority = refinement.priority_stage_ids.filter((stageId): stageId is string => typeof stageId === "string");
  const focus = refinement.focus_stage_ids.filter((stageId): stageId is string => typeof stageId === "string");
  if (priority.length !== refinement.priority_stage_ids.length || focus.length !== refinement.focus_stage_ids.length || priority.length !== stageIds.length || new Set(priority).size !== priority.length || new Set(focus).size !== focus.length || priority.some((stageId) => !stageIds.includes(stageId)) || focus.some((stageId) => !stageIds.includes(stageId))) throw new ProviderRuntimeError("accepted plan must contain an exact stage permutation and valid focus subset");
  const positions = new Map(priority.map((stageId, index) => [stageId, index]));
  if (stages.some((stage) => stage.depends_on.some((dependency) => (positions.get(dependency) ?? -1) > (positions.get(stage.id) ?? -1)))) throw new ProviderRuntimeError("accepted plan violates workflow dependencies");
  return { priority_stage_ids: [...priority], focus_stage_ids: [...focus], refinement_digest: await digestJson(refinement) };
}

/** Validate an accepted cross-domain proposal before it can alter fan-out scheduling. */
export async function acceptedCrossDomainPlan(
  blueprint: AutonomousCrossDomainBlueprint,
  refinement: AutonomousCrossDomainPlanRefinementResult | undefined,
): Promise<AutonomousAcceptedCrossDomainPlan | null> {
  if (refinement === undefined) return null;
  if (!isObject(refinement) || refinement.status !== "completed" || refinement.review_required !== false) throw new ProviderRuntimeError("only a completed, non-review cross-domain plan refinement may be accepted");
  if (refinement.task_digest !== blueprint.task_digest) throw new ProviderRuntimeError("accepted cross-domain plan task does not match the blueprint");
  if (refinement.base_plan_digest !== blueprint.plan_digest) throw new ProviderRuntimeError("accepted cross-domain plan base does not match the blueprint");
  const childIds = [...blueprint.child_ids];
  if (!Array.isArray(refinement.priority_child_ids) || !Array.isArray(refinement.focus_child_ids)) throw new ProviderRuntimeError("accepted cross-domain plan child identifiers are malformed");
  const priority = refinement.priority_child_ids.filter((childId): childId is string => typeof childId === "string");
  const focus = refinement.focus_child_ids.filter((childId): childId is string => typeof childId === "string");
  if (priority.length !== refinement.priority_child_ids.length || focus.length !== refinement.focus_child_ids.length || priority.length !== childIds.length || new Set(priority).size !== priority.length || new Set(focus).size !== focus.length || priority.some((childId) => !childIds.includes(childId)) || focus.some((childId) => !childIds.includes(childId))) throw new ProviderRuntimeError("accepted cross-domain plan must contain an exact child permutation and valid focus subset");
  return { priority_child_ids: [...priority], focus_child_ids: [...focus], refinement_digest: await digestJson(refinement) };
}
