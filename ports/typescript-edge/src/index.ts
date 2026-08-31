/**
 * Public surface of the TypeScript edge parity slice.
 *
 * Included and verified here: typed nonzero ids (ids.ts), SplitMix64/rendezvous placement
 * (mix64.ts), canonical JSON matching the CPython reference byte for byte (canonical.ts),
 * the WebCrypto-first SHA-256 abstraction with its explicitly bounded Node fallback
 * (digest.ts, node-digest.ts), the descriptor-only adapter registry (registry.ts), the bounded
 * queue and telemetry counters (queue.ts, telemetry.ts), streaming manifest metadata
 * (manifest.ts), and boundary type shapes only (boundaries.ts).
 *
 * Not implemented in this port, stated plainly rather than implied: network transports or any
 * live MCP/REST/webhook client; authentication, OAuth, or credential handling; provider SDKs
 * or Hermes/Codex/Claude behavior; a scheduler, lease table, persistence layer, or replay
 * host; OS process spawning. Logical descriptors are not live workers. The authoritative
 * implementations remain integrations/agent-fabric (Rust) and integrations/scale-5m (Python).
 */

export * from "./ids.js";
export * from "./mix64.js";
export * from "./canonical.js";
export * from "./digest.js";
export * from "./node-digest.js";
export * from "./registry.js";
export * from "./queue.js";
export * from "./telemetry.js";
export * from "./manifest.js";
export type * from "./boundaries.js";
