/** Shared text normalization for deterministic routing and capability ranking. */

export function normalizeRouteText(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, " ").trim().replace(/\s+/g, " ");
}

export function termMatches(normalized: string, term: string): boolean {
  const normalizedTerm = normalizeRouteText(term);
  return normalizedTerm.length > 0 && ` ${normalized} `.includes(` ${normalizedTerm} `);
}
