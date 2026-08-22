import type { CredentialHandle } from "./llm.js";

/** Stable contract for a worker-local credential scope; no scope value is durable job state. */
export const AUTONOMOUS_CREDENTIAL_SCOPE_SCHEMA = "bioprism-typescript-autonomous-credential-scope/0.1" as const;

export interface AutonomousCredentialScopeContext {
  jobId: string;
  attempt: number;
  approvalReleased: true;
}

/** One approved dispatch's opaque resolver and synchronous revocation boundary. */
export interface AutonomousCredentialBinding {
  credentialFor(provider: string): CredentialHandle | undefined;
  close(): void;
}

/** Deployment-owned factory invoked only after a durable worker's approval gate is released. */
export interface AutonomousCredentialScope {
  open(context: AutonomousCredentialScopeContext): Promise<AutonomousCredentialBinding> | AutonomousCredentialBinding;
}
