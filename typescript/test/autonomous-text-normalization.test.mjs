import assert from "node:assert/strict";
import test from "node:test";
import { normalizeRouteText, termMatches } from "../dist/autonomous-text-normalization.js";

test("route normalization uses the cross-SDK ASCII token contract", () => {
  for (const [value, expected] of [
    ["AURORA-Agent 2.0", "aurora agent 2 0"],
    ["München", "m nchen"],
    ["İstanbul", "i stanbul"],
    ["東京", ""],
  ]) {
    assert.equal(normalizeRouteText(value), expected);
  }
});

test("route term matching preserves normalized token boundaries", () => {
  const normalized = normalizeRouteText("AURORA-Agent supports reviewed routing");

  assert.equal(termMatches(normalized, "agent"), true);
  assert.equal(termMatches(normalized, "gent"), false);
  assert.equal(termMatches(normalized, "東京"), false);
});
