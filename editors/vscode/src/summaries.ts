type JsonObject = Record<string, unknown>;

function asObject(value: unknown): JsonObject | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as JsonObject) : undefined;
}

function mono(value: unknown): string {
  return `\`${String(value)}\``;
}

function escapeText(value: string): string {
  return value.replace(/\|/g, "\\|").replace(/\r?\n/g, " ");
}

function cell(value: unknown): string {
  if (value === null || value === undefined) {
    return "—";
  }
  if (Array.isArray(value)) {
    if (value.length === 0) {
      return "0 items";
    }
    const rendered = value.map((item) => (typeof item === "object" ? JSON.stringify(item) : String(item)));
    return `${value.length} item${value.length === 1 ? "" : "s"}: ${rendered.join(", ")}`;
  }
  if (typeof value === "object") {
    return JSON.stringify(value);
  }
  return escapeText(String(value));
}

function table(rows: Array<[string, unknown]>, headers: [string, string]): string {
  const lines = [`| ${headers[0]} | ${headers[1]} |`, "| --- | --- |"];
  for (const [key, value] of rows) {
    lines.push(`| ${escapeText(key)} | ${cell(value)} |`);
  }
  return lines.join("\n");
}

const DIGEST_KEY = /(digest|sha256|hash)/i;

function collectDigests(value: unknown, prefix: string, out: Array<[string, string]>, depth: number): void {
  if (depth > 3) {
    return;
  }
  const object = asObject(value);
  if (!object) {
    return;
  }
  for (const [key, entry] of Object.entries(object)) {
    const label = prefix === "" ? key : `${prefix}.${key}`;
    if (typeof entry === "string" && DIGEST_KEY.test(key) && /^[0-9a-f]{16,}$/.test(entry)) {
      out.push([label, entry]);
    } else if (entry && typeof entry === "object" && !Array.isArray(entry) && (key === "source_hashes" || depth === 0)) {
      collectDigests(entry, label, out, depth + 1);
    }
  }
}

export function limitationsSection(document: unknown): string {
  const object = asObject(document);
  const raw = object ? object["limitations"] : undefined;
  const lines = ["## Limitations (verbatim)", ""];
  if (Array.isArray(raw) && raw.length > 0) {
    for (const item of raw) {
      lines.push(`- ${typeof item === "string" ? item : JSON.stringify(item)}`);
    }
  } else {
    lines.push("_This document contains no `limitations` field._");
  }
  return lines.join("\n");
}

function digestsSection(document: unknown): string {
  const digests: Array<[string, string]> = [];
  collectDigests(document, "", digests, 0);
  if (digests.length === 0) {
    return "## Digests\n\n_No digest fields found in this document._";
  }
  const lines = ["## Digests", ""];
  for (const [label, digest] of digests) {
    lines.push(`- ${label}: ${mono(digest)}`);
  }
  return lines.join("\n");
}

function rawLinkSection(rawFileUri: string): string {
  return `[Open raw JSON](${rawFileUri})`;
}

function str(object: JsonObject | undefined, key: string): string | undefined {
  const value = object ? object[key] : undefined;
  return typeof value === "string" ? value : undefined;
}

function arrayLen(object: JsonObject | undefined, key: string): number | undefined {
  const value = object ? object[key] : undefined;
  return Array.isArray(value) ? value.length : undefined;
}

export function renderCertificateSummary(document: unknown, rawFileUri: string): string {
  const object = asObject(document) ?? {};
  const oracle = asObject(object["oracle"]);
  const plan = asObject(object["plan"]);
  const omissions = object["omissions"];

  const parts: string[] = [];
  parts.push(`# Context Certificate — ${str(object, "query_id") ?? "(no query_id)"}`);

  const status = str(oracle, "status");
  const oracleKind = str(oracle, "oracle_kind");
  if (status !== undefined) {
    parts.push(`**Verdict (oracle status): ${status}**${oracleKind ? ` — oracle ${mono(oracleKind)}` : ""}`);
  } else {
    parts.push("_This document carries no `oracle.status` field._");
  }
  const witnesses = arrayLen(oracle, "witnesses");
  if (witnesses !== undefined && witnesses > 0) {
    parts.push(`Witnesses: ${witnesses}`);
  }

  const headline: Array<[string, unknown]> = [];
  const worldId = str(object, "world_id");
  if (worldId !== undefined) {
    headline.push(["world_id", worldId]);
  }
  const schema = str(object, "schema_version");
  if (schema !== undefined) {
    headline.push(["schema_version", schema]);
  }
  if (headline.length > 0) {
    parts.push(table(headline, ["field", "value"]));
  }

  const selection: Array<[string, unknown]> = [];
  const selectedFacts = arrayLen(object, "selected_facts");
  if (selectedFacts !== undefined) {
    selection.push(["selected facts", selectedFacts]);
  }
  const selectedFactors = arrayLen(object, "selected_factors");
  if (selectedFactors !== undefined) {
    selection.push(["selected factors", selectedFactors]);
  }
  const closure = arrayLen(object, "protected_closure");
  if (closure !== undefined) {
    selection.push(["protected closure", closure]);
  }
  if (plan) {
    for (const key of [
      "backend",
      "fallback",
      "compiled_fact_count",
      "compiled_factor_count",
      "total_fact_count",
      "total_factor_count",
      "max_selected_factor_arity",
    ]) {
      if (key in plan) {
        selection.push([`plan.${key}`, plan[key]]);
      }
    }
  }
  parts.push("## Selection");
  parts.push(selection.length > 0 ? table(selection, ["measure", "value"]) : "_No selection fields found._");

  parts.push("## Omission accounting");
  const omissionObject = asObject(omissions);
  if (omissionObject) {
    parts.push(table(Object.entries(omissionObject), ["field", "value"]));
  } else if (Array.isArray(omissions)) {
    parts.push(
      table(
        omissions.map((entry, index) => [String(index), entry] as [string, unknown]),
        ["entry", "value"]
      )
    );
  } else {
    parts.push("_This document contains no `omissions` field._");
  }

  parts.push(digestsSection(object));
  parts.push(limitationsSection(object));
  parts.push(rawLinkSection(rawFileUri));
  return parts.join("\n\n") + "\n";
}

export function renderReportSummary(document: unknown, rawFileUri: string): string {
  const object = asObject(document) ?? {};
  const totals = asObject(object["totals"]);
  const grant = asObject(object["grant"]);
  const attempts = object["attempts"];

  const parts: string[] = [];
  parts.push(`# Autopilot Report — ${str(object, "base_mission_id") ?? "(no base_mission_id)"}`);

  const finalStatus = str(object, "final_status");
  parts.push(
    finalStatus !== undefined
      ? `**Final status: ${finalStatus}**`
      : "_This document carries no `final_status` field._"
  );

  if (totals) {
    parts.push("## Totals");
    parts.push(table(Object.entries(totals), ["field", "value"]));
  }

  if (grant) {
    parts.push("## Grant (as recorded in the report)");
    const rows: Array<[string, unknown]> = [];
    for (const key of ["allowed_tools", "allow_side_effects", "max_attempts", "retry", "schedule", "require_reconciliation_complete", "stop_on_first_success"]) {
      if (key in grant) {
        rows.push([key, grant[key]]);
      }
    }
    parts.push(rows.length > 0 ? table(rows, ["field", "value"]) : "_Grant object present but carries none of the expected fields._");
  }

  parts.push("## Attempts");
  if (Array.isArray(attempts) && attempts.length > 0) {
    const rendered: string[] = [];
    attempts.forEach((attempt, index) => {
      const record = asObject(attempt);
      if (record) {
        const fields = Object.entries(record)
          .map(([key, value]) => `${escapeText(key)}: ${cell(value)}`)
          .join("; ");
        rendered.push(`${index + 1}. ${fields}`);
      } else {
        rendered.push(`${index + 1}. ${cell(attempt)}`);
      }
    });
    parts.push(rendered.join("\n"));
  } else {
    parts.push("_No attempts recorded._");
  }

  parts.push(digestsSection(object));
  parts.push(limitationsSection(object));
  parts.push(rawLinkSection(rawFileUri));
  return parts.join("\n\n") + "\n";
}

export function renderGenericSummary(title: string, document: unknown, rawFileUri: string): string {
  const object = asObject(document);
  const parts: string[] = [`# ${title}`];
  if (object) {
    const scalars: Array<[string, unknown]> = [];
    for (const [key, value] of Object.entries(object)) {
      if (key === "limitations") {
        continue;
      }
      if (value === null || typeof value !== "object") {
        scalars.push([key, value]);
      } else if (Array.isArray(value)) {
        scalars.push([key, value]);
      } else {
        scalars.push([key, JSON.stringify(value)]);
      }
    }
    parts.push(scalars.length > 0 ? table(scalars, ["field", "value"]) : "_Empty document._");
  } else {
    parts.push("_The document is not a JSON object._");
    parts.push("```json\n" + JSON.stringify(document, null, 2) + "\n```");
  }
  parts.push(digestsSection(object ?? {}));
  parts.push(limitationsSection(object ?? {}));
  parts.push(rawLinkSection(rawFileUri));
  return parts.join("\n\n") + "\n";
}
