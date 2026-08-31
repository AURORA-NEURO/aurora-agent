import { test } from "node:test";
import * as assert from "node:assert/strict";
import { mapGhRuns, parseRemoteToNwo } from "../ghruns";

const SAMPLE = JSON.stringify([
  {
    databaseId: 123456,
    name: "ci",
    displayTitle: "fix parser",
    status: "completed",
    conclusion: "success",
    url: "https://github.com/aurora-neuro/aurora-agent/actions/runs/123456",
    createdAt: "2026-08-20T10:00:00Z",
  },
  {
    databaseId: 123457,
    name: "release",
    displayTitle: "v0.1.3",
    status: "in_progress",
    conclusion: "",
    url: "https://github.com/aurora-neuro/aurora-agent/actions/runs/123457",
    createdAt: "2026-08-21T10:00:00Z",
  },
  "not-an-object",
]);

test("mapGhRuns maps gh run list --json output", () => {
  const runs = mapGhRuns(SAMPLE);
  assert.equal(runs.length, 2);
  assert.equal(runs[0].id, "123456");
  assert.equal(runs[0].workflow, "ci");
  assert.equal(runs[0].title, "fix parser");
  assert.equal(runs[0].status, "completed");
  assert.equal(runs[0].conclusion, "success");
  assert.equal(runs[1].conclusion, "");
  assert.equal(runs[1].status, "in_progress");
});

test("mapGhRuns returns empty on malformed input", () => {
  assert.deepEqual(mapGhRuns("not json"), []);
  assert.deepEqual(mapGhRuns(JSON.stringify({ runs: [] })), []);
});

test("parseRemoteToNwo handles the common GitHub remote forms", () => {
  assert.equal(parseRemoteToNwo("https://github.com/AURORA-NEURO/aurora-agent.git"), "AURORA-NEURO/aurora-agent");
  assert.equal(parseRemoteToNwo("https://github.com/owner/repo"), "owner/repo");
  assert.equal(parseRemoteToNwo("git@github.com:owner/repo.git"), "owner/repo");
  assert.equal(parseRemoteToNwo("ssh://git@github.com/owner/repo.git"), "owner/repo");
  assert.equal(parseRemoteToNwo("https://user@github.com/owner/repo.git"), "owner/repo");
});

test("parseRemoteToNwo refuses non-GitHub remotes", () => {
  assert.equal(parseRemoteToNwo("https://gitlab.com/owner/repo.git"), undefined);
  assert.equal(parseRemoteToNwo(""), undefined);
  assert.equal(parseRemoteToNwo("file:///local/repo"), undefined);
});
