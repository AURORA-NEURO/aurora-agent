/**
 * Node-only incremental SHA-256. This is the explicit bound of the "Node fallback": it exists
 * because WebCrypto cannot hash incrementally, and manifest-style callers that want a running
 * hash without buffering need one primitive only this runtime exposes.
 *
 * The bound is threefold and deliberate: (1) importing or opening this module outside a Node
 * runtime throws instead of degrading; (2) no other module in this slice may import node:crypto,
 * so the fallback surface cannot quietly widen; (3) output must be byte-identical to the
 * WebCrypto path, which test/digest.test.mjs asserts rather than assumes.
 */

/** Reports that incremental hashing was requested where node:crypto does not exist. */
export class NodeRuntimeRequiredError extends Error {
  constructor() {
    super("incremental SHA-256 requires a Node runtime exposing process.versions.node");
    this.name = "NodeRuntimeRequiredError";
  }
}

export interface IncrementalSha256 {
  /** Absorbs more bytes into the running hash. */
  update(data: Uint8Array | string): void;
  /** Returns the lowercase hex digest and leaves the hasher unusable, mirroring createHash. */
  digestHex(): string;
}

function requireNodeVersions(): string {
  const versions =
    typeof process !== "undefined" && typeof process.versions === "object"
      ? process.versions
      : undefined;
  if (versions === undefined || typeof versions.node !== "string") {
    throw new NodeRuntimeRequiredError();
  }
  return versions.node;
}

/** Opens an incremental hasher; throws {@link NodeRuntimeRequiredError} outside Node. */
export async function openIncrementalSha256(): Promise<IncrementalSha256> {
  requireNodeVersions();
  const nodeCrypto = await import("node:crypto");
  let released = false;
  const hash = nodeCrypto.createHash("sha256");
  return {
    update(data) {
      if (released) throw new Error("incremental SHA-256 already digested");
      hash.update(typeof data === "string" ? new TextEncoder().encode(data) : data);
    },
    digestHex() {
      if (released) throw new Error("incremental SHA-256 already digested");
      released = true;
      return hash.digest("hex");
    },
  };
}
