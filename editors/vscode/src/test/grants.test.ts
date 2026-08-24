import { test } from "node:test";
import * as assert from "node:assert/strict";
import { summarizeGrant, grantConfirmationText } from "../grants";

test("summarizeGrant extracts the confirm-modal fields from a full grant", () => {
  const grant = {
    allowed_tools: ["fiber_compile", "workbench_verify"],
    allow_side_effects: true,
    max_attempts: 3,
    retry: { retry_retryable_as_is: true, retry_retryable_after_change: false, retry_unknown: false },
    schedule: {},
    require_reconciliation_complete: true,
    stop_on_first_success: true,
  };
  const summary = summarizeGrant(JSON.stringify(grant));
  assert.deepEqual(summary.allowedTools, ["fiber_compile", "workbench_verify"]);
  assert.equal(summary.allowSideEffects, true);
  assert.equal(summary.maxAttempts, 3);
  assert.deepEqual(summary.problems, []);
  const text = grantConfirmationText(summary);
  assert.match(text, /Allowed tools \(2\): fiber_compile, workbench_verify/);
  assert.match(text, /Side effects permitted: YES/);
  assert.match(text, /Max attempts: 3/);
});

test("absent allow_side_effects stays undefined and the modal names the platform default", () => {
  const summary = summarizeGrant(JSON.stringify({ allowed_tools: ["a"], max_attempts: 1 }));
  assert.equal(summary.allowSideEffects, undefined);
  assert.deepEqual(summary.problems, []);
  const text = grantConfirmationText(summary);
  assert.match(text, /Side effects permitted: not set in grant \(platform default: no\)/);
});

test("explicit allow_side_effects: false renders as no, not as the platform-default wording", () => {
  const summary = summarizeGrant(JSON.stringify({ allowed_tools: ["a"], allow_side_effects: false, max_attempts: 1 }));
  assert.equal(summary.allowSideEffects, false);
  const text = grantConfirmationText(summary);
  assert.match(text, /Side effects permitted: no/);
  assert.doesNotMatch(text, /not set in grant/);
});

test("missing allowed_tools is reported as authorising nothing", () => {
  const summary = summarizeGrant(JSON.stringify({ max_attempts: 1 }));
  assert.equal(summary.allowedTools.length, 0);
  assert.ok(summary.problems.some((p) => p.includes("authorises nothing")));
});

test("empty allowed_tools is reported as authorising nothing", () => {
  const summary = summarizeGrant(JSON.stringify({ allowed_tools: [], max_attempts: 1 }));
  assert.ok(summary.problems.some((p) => p.includes("authorises nothing")));
});

test("bad max_attempts is a problem, not a fabricated number", () => {
  const summary = summarizeGrant(JSON.stringify({ allowed_tools: ["a"], max_attempts: "three" }));
  assert.equal(summary.maxAttempts, undefined);
  assert.ok(summary.problems.some((p) => p.includes("max_attempts")));
});

test("max_attempts accepts exactly the integer range 1..=16", () => {
  for (const ok of [1, 16]) {
    const summary = summarizeGrant(JSON.stringify({ allowed_tools: ["a"], max_attempts: ok }));
    assert.equal(summary.maxAttempts, ok);
    assert.deepEqual(summary.problems, []);
  }
  for (const bad of [0, 17, -1]) {
    const summary = summarizeGrant(JSON.stringify({ allowed_tools: ["a"], max_attempts: bad }));
    assert.equal(summary.maxAttempts, undefined);
    assert.ok(summary.problems.some((p) => p.includes(`max_attempts is ${bad}`)));
  }
  const fractional = summarizeGrant(JSON.stringify({ allowed_tools: ["a"], max_attempts: 2.5 }));
  assert.equal(fractional.maxAttempts, undefined);
  assert.ok(fractional.problems.some((p) => p.includes("not an integer between 1 and 16")));
});

test("invalid JSON and non-object documents are problems", () => {
  assert.ok(summarizeGrant("{oops").problems.length > 0);
  assert.ok(summarizeGrant("[1,2]").problems.some((p) => p.includes("JSON object")));
});
