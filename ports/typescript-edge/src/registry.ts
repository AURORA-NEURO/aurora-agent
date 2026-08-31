/**
 * Deterministic adapter descriptor registry, transcribed from
 * integrations/scale-5m/aurora_scale/registry.py.
 *
 * A descriptor records what a (platform, protocol) pair speaks; it never claims a live
 * connection. The state vocabulary is the honesty mechanism: `descriptor-only` and `refused`
 * are explicit unsupported states that `requireLive` turns into typed refusals carrying their
 * notes, so a shape-only entry can never be presented as a working connector by accident.
 */

/** The four states the reference layer defines; none is inferred, each is declared. */
export type AdapterState = "supported" | "partial" | "descriptor-only" | "refused";

/**
 * True exactly for the states that must never be presented as live support. Keeping this
 * predicate in one place means a new state cannot quietly opt itself into liveness.
 */
export function isUnsupportedState(state: AdapterState): boolean {
  return state === "descriptor-only" || state === "refused";
}

/** Negation with a name, so call sites read as intent rather than double negation. */
export function isLiveUsable(state: AdapterState): boolean {
  return !isUnsupportedState(state);
}

export interface AdapterDescriptor {
  readonly platform: string;
  readonly protocol: string;
  readonly state: AdapterState;
  /** Sorted and unique; enforced at registration because declaration order is contract. */
  readonly capabilities: readonly string[];
  readonly notes: string;
}

/** Base class for registry failures; every refusal names the pair and the reason. */
export class RegistryError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "RegistryError";
  }
}

export class InvalidDescriptorError extends RegistryError {
  constructor(platform: string, protocol: string, reason: string) {
    super(`invalid descriptor ${platform}/${protocol}: ${reason}`);
    this.name = "InvalidDescriptorError";
  }
}

export class UnknownAdapterError extends RegistryError {
  constructor(platform: string, protocol: string) {
    super(`no descriptor for ${platform}/${protocol}`);
    this.name = "UnknownAdapterError";
  }
}

/** The requireLive refusal: carries state and notes so callers see why, not just that. */
export class UnsupportedAdapterError extends RegistryError {
  readonly platform: string;
  readonly protocol: string;
  readonly state: AdapterState;
  readonly notes: string;

  constructor(platform: string, protocol: string, state: AdapterState, notes: string) {
    super(`${platform}/${protocol} is ${state}: ${notes}`);
    this.name = "UnsupportedAdapterError";
    this.platform = platform;
    this.protocol = protocol;
    this.state = state;
    this.notes = notes;
  }
}

function capabilitiesSortedUnique(capabilities: readonly string[]): boolean {
  for (let i = 1; i < capabilities.length; i += 1) {
    if (capabilities[i - 1]! >= capabilities[i]!) return false;
  }
  return true;
}

/**
 * Exact-keyed registry: lookup has no prefix or wildcard path, because a wildcard hit that
 * resolves to a platform which merely resembles the request is worse than an honest miss.
 */
export class AdapterRegistry {
  private readonly byPlatform = new Map<string, Map<string, AdapterDescriptor>>();

  register(descriptor: AdapterDescriptor): void {
    const { platform, protocol } = descriptor;
    if (platform === "" || protocol === "") {
      throw new InvalidDescriptorError(platform, protocol, "platform and protocol are required");
    }
    if (!capabilitiesSortedUnique(descriptor.capabilities)) {
      throw new InvalidDescriptorError(
        platform,
        protocol,
        "capabilities must be sorted and unique",
      );
    }
    let byProtocol = this.byPlatform.get(platform);
    if (byProtocol === undefined) {
      byProtocol = new Map();
      this.byPlatform.set(platform, byProtocol);
    } else if (byProtocol.has(protocol)) {
      throw new InvalidDescriptorError(platform, protocol, "duplicate adapter descriptor");
    }
    byProtocol.set(protocol, descriptor);
  }

  get(platform: string, protocol: string): AdapterDescriptor {
    const descriptor = this.byPlatform.get(platform)?.get(protocol);
    if (descriptor === undefined) throw new UnknownAdapterError(platform, protocol);
    return descriptor;
  }

  /**
   * Returns the descriptor only when its state can honestly be presented as usable support;
   * descriptor-only and refused entries produce typed refusals carrying their notes.
   */
  requireLive(platform: string, protocol: string): AdapterDescriptor {
    const descriptor = this.get(platform, protocol);
    if (isUnsupportedState(descriptor.state)) {
      throw new UnsupportedAdapterError(
        platform,
        protocol,
        descriptor.state,
        descriptor.notes,
      );
    }
    return descriptor;
  }

  /**
   * Every descriptor ordered by (platform, protocol), so observers see the same sequence
   * regardless of insertion order - map iteration order would otherwise leak registration
   * randomness into outputs that parity treats as data.
   */
  snapshot(): AdapterDescriptor[] {
    const out: AdapterDescriptor[] = [];
    for (const [platform, byProtocol] of this.byPlatform) {
      for (const [protocol, descriptor] of byProtocol) out.push(descriptor);
    }
    out.sort((a, b) => {
      if (a.platform !== b.platform) return compareStrings(a.platform, b.platform);
      return compareStrings(a.protocol, b.protocol);
    });
    return out;
  }

  get size(): number {
    let n = 0;
    for (const byProtocol of this.byPlatform.values()) n += byProtocol.size;
    return n;
  }

  countByState(state: AdapterState): number {
    let n = 0;
    for (const byProtocol of this.byPlatform.values()) {
      for (const descriptor of byProtocol.values()) {
        if (descriptor.state === state) n += 1;
      }
    }
    return n;
  }
}

function compareStrings(a: string, b: string): number {
  // Python tuple-sorts these keys by code point; ASCII inventories make code-unit comparison
  // identical, and the canonical comparator in canonical.ts covers the general case.
  return a < b ? -1 : a > b ? 1 : 0;
}

interface NamedEntry {
  readonly platform: string;
  readonly protocol: string;
  readonly state: AdapterState;
  readonly capabilities: readonly string[];
  readonly notes: string;
}

const NAMED_ENTRIES: readonly NamedEntry[] = [
  { platform: "aurora", protocol: "mcp-stdio", state: "supported", capabilities: ["resources", "tools"], notes: "local stdio contract" },
  { platform: "aurora", protocol: "mcp-http", state: "partial", capabilities: ["tools"], notes: "HTTP/1.1 Content-Length adapter; connectivity is external" },
  { platform: "generic", protocol: "rest", state: "descriptor-only", capabilities: ["request"], notes: "descriptor only; no live connector" },
  { platform: "generic", protocol: "graphql", state: "descriptor-only", capabilities: ["query"], notes: "descriptor only; no live connector" },
  { platform: "generic", protocol: "webhook", state: "descriptor-only", capabilities: ["event"], notes: "descriptor only; no live connector" },
  { platform: "generic", protocol: "cli", state: "descriptor-only", capabilities: ["argv"], notes: "argv shape only; no process launch" },
  { platform: "generic", protocol: "archive", state: "descriptor-only", capabilities: ["import"], notes: "archive shape only; no remote fetch" },
  { platform: "generic", protocol: "a2a", state: "refused", capabilities: [], notes: "wire-shape compatibility is not a live A2A adapter" },
  { platform: "generic", protocol: "acp", state: "refused", capabilities: [], notes: "ACP is intentionally not implemented" },
];

/**
 * The reference inventory: nine named entries plus `generatedPlatforms` compact descriptor-only
 * platforms. The generated entries prove registry scale and deterministic lookup, not platform
 * support - each carries a shape and a note, never a connector.
 */
export function defaultRegistry(generatedPlatforms: number): AdapterRegistry {
  if (!Number.isSafeInteger(generatedPlatforms) || generatedPlatforms < 0) {
    throw new RangeError("generatedPlatforms must be a nonnegative safe integer");
  }
  const registry = new AdapterRegistry();
  for (const entry of NAMED_ENTRIES) {
    registry.register({ ...entry });
  }
  for (let i = 0; i < generatedPlatforms; i += 1) {
    const id = String(i).padStart(4, "0");
    registry.register({
      platform: `platform-${id}`,
      protocol: "rest",
      state: "descriptor-only",
      capabilities: ["request"],
      notes: "generated compact descriptor; integration not claimed",
    });
  }
  return registry;
}
