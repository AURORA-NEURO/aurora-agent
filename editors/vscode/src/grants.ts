export interface GrantSummary {
  allowedTools: string[];
  allowSideEffects: boolean | undefined;
  maxAttempts: number | undefined;
  problems: string[];
}

export function summarizeGrant(jsonText: string): GrantSummary {
  const summary: GrantSummary = {
    allowedTools: [],
    allowSideEffects: undefined,
    maxAttempts: undefined,
    problems: [],
  };
  let parsed: unknown;
  try {
    parsed = JSON.parse(jsonText);
  } catch (error) {
    summary.problems.push(`grant file is not valid JSON: ${error instanceof Error ? error.message : String(error)}`);
    return summary;
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    summary.problems.push("grant document must be a JSON object");
    return summary;
  }
  const doc = parsed as Record<string, unknown>;

  const tools = doc["allowed_tools"];
  if (Array.isArray(tools)) {
    summary.allowedTools = tools.filter((t): t is string => typeof t === "string");
    if (summary.allowedTools.length === 0) {
      summary.problems.push("allowed_tools is empty — the grant authorises nothing");
    }
  } else {
    summary.problems.push("allowed_tools is missing — the grant authorises nothing");
  }

  const sideEffects = doc["allow_side_effects"];
  if (typeof sideEffects === "boolean") {
    summary.allowSideEffects = sideEffects;
  } else if (sideEffects !== undefined) {
    summary.problems.push("allow_side_effects is present but not a boolean");
  }

  const attempts = doc["max_attempts"];
  if (typeof attempts === "number" && Number.isInteger(attempts) && attempts >= 1 && attempts <= 16) {
    summary.maxAttempts = attempts;
  } else if (typeof attempts === "number" && Number.isInteger(attempts)) {
    summary.problems.push(`max_attempts is ${attempts} — it must be an integer between 1 and 16`);
  } else {
    summary.problems.push("max_attempts is missing or not an integer between 1 and 16");
  }

  return summary;
}

export function grantConfirmationText(summary: GrantSummary): string {
  const lines: string[] = [];
  lines.push(`Allowed tools (${summary.allowedTools.length}): ${summary.allowedTools.join(", ") || "(none)"}`);
  lines.push(
    `Side effects permitted: ${
      summary.allowSideEffects === undefined
        ? "not set in grant (platform default: no)"
        : summary.allowSideEffects
          ? "YES"
          : "no"
    }`
  );
  lines.push(`Max attempts: ${summary.maxAttempts === undefined ? "(not set)" : summary.maxAttempts}`);
  if (summary.problems.length > 0) {
    lines.push(`Problems: ${summary.problems.join("; ")}`);
  }
  return lines.join("\n");
}
