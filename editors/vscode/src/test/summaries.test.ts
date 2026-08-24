import { test } from "node:test";
import * as assert from "node:assert/strict";
import { renderCertificateSummary, renderReportSummary, renderGenericSummary, limitationsSection } from "../summaries";
import { parseCatalogue } from "../catalogue";

test("certificate summary shows verdict, counts, omissions, full digests, and verbatim limitations", () => {
  const certificate = {
    schema_version: "fiber-context-certificate/0.1",
    query_id: "audit-split-integrity-v1",
    world_id: "radiogenomic-integrity-demo-v1",
    certificate_sha256: "c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4",
    selected_facts: ["fact.cohort", "fact.other"],
    selected_factors: ["factor.claim_support"],
    protected_closure: ["fact.cohort"],
    plan: { backend: "backward_factor_slice_reference", compiled_fact_count: 11, total_fact_count: 761 },
    omissions: { classification: "no_backward_dependency_path", exploratory_facts: 750, total_facts: 750 },
    oracle: { oracle_kind: "deterministic_split_integrity_v1", status: "invalid", witnesses: [{}, {}] },
    source_hashes: {
      world_sha256: "b3809731cf93040fcd8aef43deb2a552492064b49154e07ea58caa724c10cbb5",
    },
    limitations: ["Reference slicer uses dependency reachability and protected tags; it does not weigh relevance."],
  };
  const markdown = renderCertificateSummary(certificate, "file:///tmp/cert.json");
  assert.match(markdown, /Verdict \(oracle status\): invalid/);
  assert.match(markdown, /selected facts \| 2/);
  assert.match(markdown, /Omission accounting/);
  assert.match(markdown, /exploratory_facts \| 750/);
  assert.match(markdown, /`c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4`/);
  assert.match(markdown, /`b3809731cf93040fcd8aef43deb2a552492064b49154e07ea58caa724c10cbb5`/);
  assert.match(markdown, /Reference slicer uses dependency reachability and protected tags; it does not weigh relevance\./);
  assert.match(markdown, /\[Open raw JSON\]\(file:\/\/\/tmp\/cert\.json\)/);
});

test("report summary shows final_status, totals, grant fields, and verbatim limitations", () => {
  const report = {
    base_mission_id: "demo",
    final_status: "exhausted",
    totals: { attempts_used: 3, max_attempts: 3, steps_in_plan: 4 },
    grant: { allowed_tools: ["a"], allow_side_effects: false, max_attempts: 3 },
    attempts: [{ attempt: 1, status: "failed" }],
    report_sha256: "d".repeat(64),
    limitations: ["no scheduling or recurrence", "no cross-process resume"],
  };
  const markdown = renderReportSummary(report, "file:///tmp/report.json");
  assert.match(markdown, /Final status: exhausted/);
  assert.match(markdown, /attempts_used \| 3/);
  assert.match(markdown, /allowed_tools \| 1 item: a/);
  assert.match(markdown, /`dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd`/);
  assert.match(markdown, /- no scheduling or recurrence/);
  assert.match(markdown, /- no cross-process resume/);
});

test("table keys and attempt keys are escaped the same way as cell values", () => {
  const report = {
    base_mission_id: "demo",
    final_status: "succeeded",
    totals: { "attempts|used": 1 },
    attempts: [{ "weird|key": "a|b" }],
  };
  const markdown = renderReportSummary(report, "file:///tmp/report.json");
  assert.match(markdown, /\| attempts\\\|used \| 1 \|/);
  assert.match(markdown, /weird\\\|key: a\\\|b/);
});

test("generic summary always links the raw document, including virtual-document URIs", () => {
  const raw = "aurora-agent:/summary/3/World%20validation-raw.json";
  const markdown = renderGenericSummary("World validation", { ok: true }, raw);
  assert.match(markdown, /\[Open raw JSON\]\(aurora-agent:\/summary\/3\/World%20validation-raw\.json\)/);
});

test("a document without limitations says so instead of hiding the section", () => {
  const section = limitationsSection({ ok: true });
  assert.match(section, /## Limitations \(verbatim\)/);
  assert.match(section, /contains no `limitations` field/);
});

test("parseCatalogue extracts capability groups and their tools", () => {
  const catalogue = {
    ok: true,
    workflow_count: 2,
    workflows: [
      {
        workflow_id: "decision_context",
        title: "decision context workflow",
        status: "available",
        domains: ["fiber"],
        tools: { declared: ["fiber_compile"], available: ["fiber_compile"], missing: [] },
      },
      {
        workflow_id: "evidence",
        title: "evidence workflow",
        status: "available",
        domains: [],
        tools: { declared: ["a", "b"], available: ["a"], missing: ["b"] },
      },
    ],
  };
  const groups = parseCatalogue(catalogue);
  assert.equal(groups.length, 2);
  assert.equal(groups[0].id, "decision_context");
  assert.deepEqual(groups[1].toolsMissing, ["b"]);
  assert.deepEqual(parseCatalogue({ ok: true }), []);
});
