export interface CatalogueGroup {
  id: string;
  title: string;
  status: string;
  domains: string[];
  toolsDeclared: string[];
  toolsAvailable: string[];
  toolsMissing: string[];
}

function stringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item): item is string => typeof item === "string");
}

export function parseCatalogue(document: unknown): CatalogueGroup[] {
  if (!document || typeof document !== "object") {
    return [];
  }
  const workflows = (document as Record<string, unknown>)["workflows"];
  if (!Array.isArray(workflows)) {
    return [];
  }
  const groups: CatalogueGroup[] = [];
  for (const entry of workflows) {
    if (!entry || typeof entry !== "object") {
      continue;
    }
    const record = entry as Record<string, unknown>;
    const id = typeof record["workflow_id"] === "string" ? record["workflow_id"] : "";
    if (id === "") {
      continue;
    }
    const tools =
      record["tools"] && typeof record["tools"] === "object"
        ? (record["tools"] as Record<string, unknown>)
        : {};
    groups.push({
      id,
      title: typeof record["title"] === "string" ? record["title"] : id,
      status: typeof record["status"] === "string" ? record["status"] : "unknown",
      domains: stringArray(record["domains"]),
      toolsDeclared: stringArray(tools["declared"]),
      toolsAvailable: stringArray(tools["available"]),
      toolsMissing: stringArray(tools["missing"]),
    });
  }
  return groups;
}
