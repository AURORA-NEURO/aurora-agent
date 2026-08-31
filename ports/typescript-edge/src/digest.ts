/**
 * Deterministic SHA-256 abstraction.
 *
 * Primary backend is WebCrypto (`globalThis.crypto.subtle`) so the slice stays runtime-neutral
 * across browsers, workers and modern Node. The only fallback is Node's own crypto, and that
 * fallback is explicitly bounded: it is attempted exclusively when WebCrypto is absent AND the
 * runtime advertises `process.versions.node`, it lives behind one detection point here, and
 * both backends must produce identical bytes (asserted in test/digest.test.mjs). Nothing in
 * this module ever substitutes a different algorithm, a truncated digest, or a zero hash when
 * no backend exists - that is a typed error instead.
 *
 * Incremental hashing is deliberately NOT faked on top of WebCrypto, which cannot stream:
 * src/node-digest.ts exposes the Node-only incremental form, and the manifest layer keeps its
 * memory bound by buffering one chunk at a time rather than pretending subtle.digest updates.
 */

export type DigestBackend = "webcrypto" | "node-crypto";

/** Reports that no SHA-256 backend exists in this runtime. Never downgraded to empty output. */
export class DigestUnavailableError extends Error {
  constructor() {
    super("no SHA-256 backend: runtime exposes neither WebCrypto subtle.digest nor node:crypto");
    this.name = "DigestUnavailableError";
  }
}

const encoder = new TextEncoder();
const HEX_DIGITS = "0123456789abcdef";

interface NodeCryptoLike {
  createHash(algorithm: "sha256"): {
    update(data: Uint8Array): { digest(): Uint8Array };
    digest(): Uint8Array;
  };
}

let nodeCryptoPromise: Promise<NodeCryptoLike | null> | undefined;

async function loadNodeCrypto(): Promise<NodeCryptoLike | null> {
  if (nodeCryptoPromise === undefined) {
    const isNode =
      typeof process !== "undefined" &&
      typeof process.versions === "object" &&
      typeof process.versions.node === "string";
    nodeCryptoPromise = isNode
      ? (import("node:crypto") as Promise<unknown>).then((m) => m as NodeCryptoLike)
      : Promise.resolve(null);
  }
  return nodeCryptoPromise;
}

function hasWebCrypto(): boolean {
  return (
    typeof globalThis.crypto !== "undefined" &&
    typeof globalThis.crypto.subtle !== "undefined" &&
    typeof globalThis.crypto.subtle.digest === "function"
  );
}

/**
 * Names the backend a digest call on this runtime will actually take. The Node fallback fires
 * only when WebCrypto is missing; where both exist WebCrypto wins, so callers can cite which
 * implementation produced their bytes.
 */
export async function activeDigestBackend(): Promise<DigestBackend> {
  if (hasWebCrypto()) return "webcrypto";
  if ((await loadNodeCrypto()) !== null) return "node-crypto";
  throw new DigestUnavailableError();
}

function toBytes(data: Uint8Array | string): Uint8Array {
  return typeof data === "string" ? encoder.encode(data) : data;
}

async function sha256Raw(data: Uint8Array): Promise<Uint8Array> {
  if (hasWebCrypto()) {
    const buffer = await globalThis.crypto.subtle.digest("SHA-256", data);
    return new Uint8Array(buffer);
  }
  const nodeCrypto = await loadNodeCrypto();
  if (nodeCrypto === null) throw new DigestUnavailableError();
  return new Uint8Array(nodeCrypto.createHash("sha256").update(data).digest());
}

/** Lowercase hex SHA-256 over UTF-8 `data`, identical bytes on every supported backend. */
export async function sha256Hex(data: Uint8Array | string): Promise<string> {
  const raw = await sha256Raw(toBytes(data));
  let hex = "";
  for (const byte of raw) hex += HEX_DIGITS[(byte >> 4) & 0xf]! + HEX_DIGITS[byte & 0xf]!;
  return hex;
}

/** Raw 32-byte digest, for callers folding digests into parent digests without re-parsing hex. */
export async function sha256Bytes(data: Uint8Array | string): Promise<Uint8Array> {
  return sha256Raw(toBytes(data));
}
