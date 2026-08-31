/** Canonical built-in domain vocabulary shared by autonomous subsystems. */

export const AUTONOMOUS_DOMAIN_NAMES = [
  "coding",
  "browser",
  "data",
  "science",
  "biomedical",
  "neuroscience",
  "operations",
  "enterprise",
  "multi_agent",
  "multimodal",
  "cross_domain",
  "evaluation",
] as const;

export type AutonomousDomainName = typeof AUTONOMOUS_DOMAIN_NAMES[number];
