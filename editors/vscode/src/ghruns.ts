export interface GhRun {
  id: string;
  workflow: string;
  title: string;
  status: string;
  conclusion: string;
  url: string;
  createdAt: string;
}

export function mapGhRuns(jsonText: string): GhRun[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(jsonText);
  } catch {
    return [];
  }
  if (!Array.isArray(parsed)) {
    return [];
  }
  const runs: GhRun[] = [];
  for (const entry of parsed) {
    if (!entry || typeof entry !== "object") {
      continue;
    }
    const record = entry as Record<string, unknown>;
    const str = (key: string): string => (typeof record[key] === "string" ? (record[key] as string) : "");
    const id =
      typeof record["databaseId"] === "number"
        ? String(record["databaseId"])
        : str("databaseId");
    runs.push({
      id,
      workflow: str("name") || str("workflowName"),
      title: str("displayTitle"),
      status: str("status"),
      conclusion: str("conclusion"),
      url: str("url"),
      createdAt: str("createdAt"),
    });
  }
  return runs;
}

export function parseRemoteToNwo(remoteUrl: string): string | undefined {
  const trimmed = remoteUrl.trim();
  if (trimmed === "") {
    return undefined;
  }
  const patterns = [
    /^https?:\/\/(?:[^@/]+@)?github\.com\/([^/]+)\/([^/]+?)(?:\.git)?\/?$/,
    /^git@github\.com:([^/]+)\/([^/]+?)(?:\.git)?$/,
    /^ssh:\/\/git@github\.com\/([^/]+)\/([^/]+?)(?:\.git)?\/?$/,
  ];
  for (const pattern of patterns) {
    const match = pattern.exec(trimmed);
    if (match) {
      return `${match[1]}/${match[2]}`;
    }
  }
  return undefined;
}
