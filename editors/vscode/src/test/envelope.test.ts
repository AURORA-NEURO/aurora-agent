import { test } from "node:test";
import * as assert from "node:assert/strict";
import { parseEnvelope, describeOutcome, retryabilityForExit } from "../envelope";

test("exit 0 with a JSON document is ok", () => {
  const outcome = parseEnvelope(0, JSON.stringify({ ok: true, workflow_count: 30 }), "");
  assert.equal(outcome.kind, "ok");
  assert.equal(outcome.exitCode, 0);
  assert.deepEqual(outcome.document, { ok: true, workflow_count: 30 });
});

test("exit 1 is a verdict, never a crash, and carries the report document", () => {
  const report = { final_status: "refused", totals: { attempts_used: 0 } };
  const outcome = parseEnvelope(1, JSON.stringify(report), "");
  assert.equal(outcome.kind, "verdict");
  assert.equal(outcome.exitCode, 1);
  assert.deepEqual(outcome.document, report);
  assert.equal(outcome.retryability, undefined);
  assert.match(describeOutcome(outcome), /verdict \(exit 1\)/);
});

test("failure envelope surfaces kind, message, subject, and retryability", () => {
  const envelope = {
    ok: false,
    error: {
      code: 4,
      kind: "compile_failed",
      retryable: false,
      retryability: "retryable_after_change",
      message: "no result satisfies the declared contract",
      subject: "world.json",
    },
  };
  const outcome = parseEnvelope(4, JSON.stringify(envelope), "");
  assert.equal(outcome.kind, "failure");
  assert.equal(outcome.errorKind, "compile_failed");
  assert.equal(outcome.retryability, "retryable_after_change");
  assert.match(outcome.message, /world\.json: no result satisfies/);
  assert.match(describeOutcome(outcome), /retryability: retryable_after_change/);
});

test("exit 7 without an envelope falls back to the documented retryability table", () => {
  const outcome = parseEnvelope(7, "", "grant does not authorise tool x");
  assert.equal(outcome.kind, "failure");
  assert.equal(outcome.retryability, "retryable_after_change");
  assert.equal(outcome.message, "grant does not authorise tool x");
});

test("retryability table matches the CLI registry", () => {
  assert.equal(retryabilityForExit(0), undefined);
  assert.equal(retryabilityForExit(1), undefined);
  assert.equal(retryabilityForExit(2), "terminal");
  assert.equal(retryabilityForExit(3), "terminal");
  assert.equal(retryabilityForExit(4), "retryable_after_change");
  assert.equal(retryabilityForExit(5), "retryable_as_is");
  assert.equal(retryabilityForExit(6), "terminal");
  assert.equal(retryabilityForExit(7), "retryable_after_change");
  assert.equal(retryabilityForExit(8), "retryable_after_change");
  assert.equal(retryabilityForExit(9), "retryable_as_is");
});

test("exit 0 with non-JSON stdout is reported as a crash, not silently ok", () => {
  const outcome = parseEnvelope(0, "not json", "");
  assert.equal(outcome.kind, "crash");
});

test("null exit code (signal) is a crash", () => {
  const outcome = parseEnvelope(null, "", "killed");
  assert.equal(outcome.kind, "crash");
  assert.equal(outcome.message, "killed");
});
