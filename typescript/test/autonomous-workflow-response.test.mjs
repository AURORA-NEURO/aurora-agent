import assert from "node:assert/strict";
import { test } from "node:test";

import {
  builtinAutonomousDomainProfiles,
  evaluateAutonomousWorkflowStageResponse,
  replayAutonomousWorkflowStageResponseEvaluation,
  validateAutonomousWorkflowStageResponseEvaluation,
} from "../dist/index.js";

function response(stageId, suffix = "") {
  return {
    stage_id: stageId,
    status: "completed",
    evidence: [`verified-${stageId}${suffix}`],
    uncertainty: [`bounded-uncertainty-${stageId}${suffix}`],
    notes: `The ${stageId} stage produced a reviewable result${suffix}.`,
    next_actions: [`review-${stageId}${suffix}`],
  };
}

test("workflow-stage composition evaluation is executable and replayable for every built-in domain", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  assert.ok(profiles.length >= 8);
  for (const profile of profiles) {
    const stage = profile.workflow.stages[0];
    const evaluation = evaluateAutonomousWorkflowStageResponse(response(stage.id), {
      domain: profile.domain,
      workflowId: profile.workflow.workflow_id,
      workflowDigest: profile.workflow.workflow_digest,
      stageId: stage.id,
    });
    assert.equal(evaluation.domain, profile.domain);
    assert.equal(evaluation.workflow_digest, profile.workflow.workflow_digest);
    assert.equal(evaluation.stage_id, stage.id);
    assert.equal(evaluation.evidence_digest, evaluation.response_digest);
    assert.deepEqual(validateAutonomousWorkflowStageResponseEvaluation(evaluation), evaluation);
    assert.deepEqual(replayAutonomousWorkflowStageResponseEvaluation(response(stage.id), evaluation), evaluation);
  }
});

test("workflow-stage composition evaluation rejects credential-shaped values and replay drift", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((candidate) => candidate.domain === "coding");
  const stage = profile.workflow.stages[0];
  const options = {
    domain: profile.domain,
    workflowId: profile.workflow.workflow_id,
    workflowDigest: profile.workflow.workflow_digest,
    stageId: stage.id,
  };
  const evaluation = evaluateAutonomousWorkflowStageResponse(response(stage.id), options);
  await assert.rejects(
    async () => evaluateAutonomousWorkflowStageResponse({ ...response(stage.id), notes: "gsk_fixture_redacted" }, options),
    /credential-shaped/,
  );
  assert.throws(
    () => replayAutonomousWorkflowStageResponseEvaluation(response(stage.id, "-drift"), evaluation),
    /replay drifted/,
  );
  assert.throws(
    () => validateAutonomousWorkflowStageResponseEvaluation({ ...evaluation, domain: "unsupported" }),
    /domain is not supported/,
  );
});
