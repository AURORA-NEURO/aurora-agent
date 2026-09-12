import { ArgumentError, isObject } from "./errors.js";
import type {
  JsonObject,
  ToolArguments,
  ToolCallPlan,
  ToolDefinition,
  ToolValidationIssue,
  ToolValidationReport,
} from "./types.js";

// Canonical digests are an authorization primitive throughout the SDK. Capture JSON string
// escaping once so an awaited caller hook cannot temporarily replace the global implementation
// and make a receipt describe bytes other than the value being authorized.
const nativeJsonStringify = JSON.stringify.bind(JSON);
const nativeObjectKeys = Object.keys.bind(Object);
const nativeArrayIsArray = Array.isArray.bind(Array);
const NativeTextEncoder = globalThis.TextEncoder;
const nativeTextEncoderEncode = NativeTextEncoder.prototype.encode;
const NativeUint8Array = globalThis.Uint8Array;
const NativeUint32Array = globalThis.Uint32Array;
const nativeSubtleDigest = globalThis.crypto?.subtle?.digest.bind(globalThis.crypto.subtle) ?? null;

export const TOOL_CATALOGUE_SCHEMA = "bioprism-typescript-tool-catalogue/0.1";
export const MAX_TOOL_DEFINITIONS = 512;
export const MAX_TOOL_SCHEMA_BYTES = 1_000_000;
export const MAX_TOOL_CATALOGUE_BYTES = 20_000_000;
export const MAX_TOOL_ARGUMENT_DEPTH = 100;
export const MAX_TOOL_NAME_BYTES = 256;

const IGNORED_SCHEMA_KEYWORDS = new Set(["$comment", "$id", "$schema", "default", "description", "examples", "title"]);
const SUPPORTED_SCHEMA_KEYWORDS = new Set([
  "additionalProperties", "allOf", "anyOf", "const", "enum", "exclusiveMaximum", "exclusiveMinimum",
  "format", "items", "maxItems", "maxLength", "maxProperties", "maximum", "minItems", "minLength",
  "minProperties", "minimum", "not", "oneOf", "pattern", "properties", "required", "type", "uniqueItems",
]);

export class ToolSchemaError extends ArgumentError {
  override readonly name: string = "ToolSchemaError";
  readonly report?: ToolValidationReport;

  constructor(message: string, report?: ToolValidationReport) {
    super(message);
    this.report = report;
  }
}

/**
 * A bounded, digest-addressed snapshot of the authoritative live tools catalogue.
 *
 * Validation here is deliberately transport-only. It cannot approve a scientific claim,
 * policy decision, side effect, or remote result; those remain MCP server responsibilities.
 */
export class ToolCatalogue {
  readonly definitions: readonly ToolDefinition[];
  readonly digest: string;

  private readonly byName: ReadonlyMap<string, ToolDefinition>;
  private readonly schemaDigests: ReadonlyMap<string, string>;

  private constructor(definitions: readonly ToolDefinition[], digest: string, schemaDigests: ReadonlyMap<string, string>) {
    this.definitions = definitions;
    this.digest = digest;
    this.byName = new Map(definitions.map((definition) => [definition.name, definition]));
    this.schemaDigests = schemaDigests;
  }

  static async fromDefinitions(values: readonly ToolDefinition[]): Promise<ToolCatalogue> {
    if (!Array.isArray(values) || values.length > MAX_TOOL_DEFINITIONS) {
      throw new ArgumentError(`tool definitions must be an array of at most ${MAX_TOOL_DEFINITIONS} items`);
    }
    const definitions = values.map((value, index) => normaliseDefinition(value, index));
    const names = new Set<string>();
    for (const definition of definitions) {
      if (names.has(definition.name)) throw new ArgumentError(`duplicate tool definition name: ${definition.name}`);
      names.add(definition.name);
    }
    const payload = [...definitions]
      .sort((left, right) => left.name.localeCompare(right.name))
      .map((definition) => ({ name: definition.name, description: definition.description, inputSchema: definition.inputSchema }));
    const serialized = canonicalJson(payload);
    const bytes = new TextEncoder().encode(serialized).byteLength;
    if (bytes > MAX_TOOL_CATALOGUE_BYTES) throw new ArgumentError(`tool catalogue exceeds ${MAX_TOOL_CATALOGUE_BYTES} bytes`);
    const schemaDigests = new Map<string, string>();
    for (const definition of definitions) schemaDigests.set(definition.name, await sha256Hex(canonicalJson(definition.inputSchema)));
    return new ToolCatalogue(definitions, await sha256Hex(serialized), schemaDigests);
  }

  get(name: string): ToolDefinition {
    if (typeof name !== "string" || !name.trim()) throw new ArgumentError("tool name must be a non-empty string");
    const definition = this.byName.get(name);
    if (!definition) throw new ToolSchemaError(`tool ${name} is absent from the live tools catalogue`);
    return definition;
  }

  validate(name: string, arguments_: ToolArguments = {}): ToolValidationReport {
    const definition = this.get(name);
    if (!isObject(arguments_)) {
      return {
        tool: name,
        schemaDigest: this.schemaDigests.get(name) as string,
        issues: [{ path: "$", code: "object_required", message: "tool arguments must be a JSON object" }],
        warnings: [],
        ok: false,
        fullyChecked: false,
      };
    }
    const issues: ToolValidationIssue[] = [];
    const warnings: ToolValidationIssue[] = [];
    checkSchemaValue(arguments_, definition.inputSchema, "$", issues, warnings, 0);
    return {
      tool: name,
      schemaDigest: this.schemaDigests.get(name) as string,
      issues,
      warnings,
      ok: issues.length === 0,
      fullyChecked: issues.length === 0 && warnings.length === 0,
    };
  }

  plan(name: string, arguments_: ToolArguments = {}): ToolCallPlan {
    const report = this.validate(name, arguments_);
    if (!report.ok) {
      const detail = report.issues.map((issue) => `${issue.path}: ${issue.message}`).join("; ");
      throw new ToolSchemaError(`tool ${name} arguments failed schema preflight: ${detail}`, report);
    }
    canonicalJson(arguments_);
    const definition = this.get(name);
    return {
      tool: name,
      definition,
      arguments: { ...arguments_ },
      report,
      schemaDigest: report.schemaDigest,
    };
  }
}

function normaliseDefinition(value: ToolDefinition, index: number): ToolDefinition {
  if (!isObject(value) || typeof value.name !== "string" || !value.name.trim()) {
    throw new ArgumentError(`tool definition ${index} requires a non-empty string name`);
  }
  if (new TextEncoder().encode(value.name).byteLength > MAX_TOOL_NAME_BYTES || /[\r\n\u0000-\u001f]/.test(value.name)) {
    throw new ArgumentError(`tool definition ${value.name} has an unsafe name`);
  }
  if (!isObject(value.inputSchema)) throw new ArgumentError(`tool definition ${value.name} requires an object inputSchema`);
  const schemaBytes = new TextEncoder().encode(canonicalJson(value.inputSchema)).byteLength;
  if (schemaBytes > MAX_TOOL_SCHEMA_BYTES) throw new ArgumentError(`tool definition ${value.name} exceeds ${MAX_TOOL_SCHEMA_BYTES} schema bytes`);
  return {
    name: value.name,
    description: typeof value.description === "string" ? value.description : "",
    inputSchema: value.inputSchema as JsonObject,
  };
}

function checkSchemaValue(
  value: unknown,
  schema: unknown,
  path: string,
  issues: ToolValidationIssue[],
  warnings: ToolValidationIssue[],
  depth: number,
): void {
  if (depth > MAX_TOOL_ARGUMENT_DEPTH) {
    issues.push({ path, code: "nesting_limit", message: `JSON nesting exceeds ${MAX_TOOL_ARGUMENT_DEPTH} levels` });
    return;
  }
  if (schema === true) return;
  if (schema === false) {
    issues.push({ path, code: "schema_false", message: "the authoritative schema rejects this value" });
    return;
  }
  if (!isObject(schema)) {
    issues.push({ path, code: "invalid_schema", message: "schema branch is not a JSON object or boolean" });
    return;
  }
  for (const keyword of Object.keys(schema)) {
    if (!SUPPORTED_SCHEMA_KEYWORDS.has(keyword) && !IGNORED_SCHEMA_KEYWORDS.has(keyword)) {
      warnings.push({ path, code: "unsupported_schema_keyword", message: `schema keyword ${keyword} was not evaluated` });
    }
  }

  const allOf = schema.allOf;
  if (allOf !== undefined) {
    if (!Array.isArray(allOf)) issues.push({ path, code: "invalid_allOf", message: "allOf must be an array" });
    else for (const branch of allOf) checkSchemaValue(value, branch, path, issues, warnings, depth + 1);
  }
  for (const combinator of ["anyOf", "oneOf"] as const) {
    const branches = schema[combinator];
    if (branches === undefined) continue;
    if (!Array.isArray(branches) || branches.length === 0) {
      issues.push({ path, code: `invalid_${combinator}`, message: `${combinator} must be a non-empty array` });
      continue;
    }
    let matches = 0;
    const branchWarnings: ToolValidationIssue[] = [];
    for (const branch of branches) {
      const branchIssues: ToolValidationIssue[] = [];
      const localWarnings: ToolValidationIssue[] = [];
      checkSchemaValue(value, branch, path, branchIssues, localWarnings, depth + 1);
      if (branchIssues.length === 0) {
        matches += 1;
        branchWarnings.push(...localWarnings);
      }
    }
    if (combinator === "anyOf" && matches === 0) issues.push({ path, code: "anyOf_no_match", message: "value matched none of the schema alternatives" });
    if (combinator === "oneOf" && matches !== 1) issues.push({ path, code: "oneOf_cardinality", message: `value matched ${matches} schema alternatives, expected exactly one` });
    warnings.push(...branchWarnings);
  }

  const forbidden = schema.not;
  if (forbidden !== undefined) {
    const forbiddenIssues: ToolValidationIssue[] = [];
    checkSchemaValue(value, forbidden, path, forbiddenIssues, [], depth + 1);
    if (forbiddenIssues.length === 0) issues.push({ path, code: "not_rejected", message: "value matches a forbidden schema" });
  }

  const expected = schema.type;
  if (expected !== undefined && !matchesType(value, expected)) {
    issues.push({ path, code: "type", message: `expected JSON type ${nativeJsonStringify(expected)}, got ${jsonType(value)}` });
    return;
  }
  const enumeration = schema.enum;
  if (enumeration !== undefined) {
    if (!Array.isArray(enumeration)) issues.push({ path, code: "invalid_enum", message: "enum must be an array" });
    else if (!enumeration.some((choice) => sameJsonValue(value, choice))) issues.push({ path, code: "enum", message: "value is not an allowed enum member" });
  }
  if (schema.const !== undefined && !sameJsonValue(value, schema.const)) {
    issues.push({ path, code: "const", message: "value does not equal the required constant" });
  }
  if (isObject(value)) checkObject(value, schema, path, issues, warnings, depth);
  else if (Array.isArray(value)) checkArray(value, schema, path, issues, warnings, depth);
  else if (typeof value === "string") checkString(value, schema, path, issues);
  else if (typeof value === "number" && Number.isFinite(value)) checkNumber(value, schema, path, issues);
}

function checkObject(value: Record<string, unknown>, schema: Record<string, unknown>, path: string, issues: ToolValidationIssue[], warnings: ToolValidationIssue[], depth: number): void {
  const required = schema.required;
  if (required !== undefined) {
    if (!Array.isArray(required)) issues.push({ path, code: "invalid_required", message: "required must be an array" });
    else for (const name of required) {
      if (typeof name !== "string") issues.push({ path, code: "invalid_required_member", message: "required members must be strings" });
      else if (!(name in value)) issues.push({ path: `${path}.${name}`, code: "required", message: "required property is missing" });
    }
  }
  const properties = schema.properties;
  if (properties !== undefined && !isObject(properties)) issues.push({ path, code: "invalid_properties", message: "properties must be an object" });
  const known = isObject(properties) ? new Set(Object.keys(properties)) : new Set<string>();
  const additional = schema.additionalProperties;
  for (const [name, child] of Object.entries(value)) {
    const childPath = /^[A-Za-z_$][\w$]*$/.test(name) ? `${path}.${name}` : `${path}[${nativeJsonStringify(name)}]`;
    if (known.has(name)) checkSchemaValue(child, (properties as Record<string, unknown>)[name], childPath, issues, warnings, depth + 1);
    else if (additional === false) issues.push({ path: childPath, code: "additional_property", message: "property is not allowed by the schema" });
    else if (isObject(additional) || additional === true) checkSchemaValue(child, additional, childPath, issues, warnings, depth + 1);
  }
  checkCount(Object.keys(value).length, schema, path, "minProperties", "maxProperties", issues);
}

function checkArray(value: unknown[], schema: Record<string, unknown>, path: string, issues: ToolValidationIssue[], warnings: ToolValidationIssue[], depth: number): void {
  checkCount(value.length, schema, path, "minItems", "maxItems", issues);
  if (schema.uniqueItems === true) {
    const seen = new Set<string>();
    for (const item of value) {
      const encoded = canonicalJson(item);
      if (seen.has(encoded)) {
        issues.push({ path, code: "uniqueItems", message: "array items must be unique" });
        break;
      }
      seen.add(encoded);
    }
  }
  const items = schema.items;
  if (Array.isArray(items)) {
    items.forEach((itemSchema, index) => { if (index < value.length) checkSchemaValue(value[index], itemSchema, `${path}[${index}]`, issues, warnings, depth + 1); });
  } else if (items !== undefined) {
    value.forEach((item, index) => checkSchemaValue(item, items, `${path}[${index}]`, issues, warnings, depth + 1));
  }
}

function checkString(value: string, schema: Record<string, unknown>, path: string, issues: ToolValidationIssue[]): void {
  checkCount(value.length, schema, path, "minLength", "maxLength", issues);
  if (schema.pattern !== undefined) {
    if (typeof schema.pattern !== "string") return;
    try {
      if (new RegExp(schema.pattern).test(value) === false) issues.push({ path, code: "pattern", message: "value does not match the schema pattern" });
    } catch {
      // Unsupported regex dialects remain an explicit unchecked boundary through the keyword warning.
    }
  }
}

function checkNumber(value: number, schema: Record<string, unknown>, path: string, issues: ToolValidationIssue[]): void {
  const comparisons: readonly [string, boolean][] = [
    ["minimum", typeof schema.minimum === "number" && value < schema.minimum],
    ["exclusiveMinimum", typeof schema.exclusiveMinimum === "number" && value <= schema.exclusiveMinimum],
    ["maximum", typeof schema.maximum === "number" && value > schema.maximum],
    ["exclusiveMaximum", typeof schema.exclusiveMaximum === "number" && value >= schema.exclusiveMaximum],
  ];
  for (const [keyword, failed] of comparisons) if (failed) issues.push({ path, code: keyword, message: `value violates ${keyword}` });
}

function checkCount(value: number, schema: Record<string, unknown>, path: string, minimumKey: string, maximumKey: string, issues: ToolValidationIssue[]): void {
  if (typeof schema[minimumKey] === "number" && value < schema[minimumKey]) issues.push({ path, code: minimumKey, message: `count ${value} is below ${schema[minimumKey]}` });
  if (typeof schema[maximumKey] === "number" && value > schema[maximumKey]) issues.push({ path, code: maximumKey, message: `count ${value} exceeds ${schema[maximumKey]}` });
}

function matchesType(value: unknown, expected: unknown): boolean {
  const types = Array.isArray(expected) ? expected : [expected];
  return types.some((kind) =>
    (kind === "null" && value === null)
    || (kind === "object" && isObject(value))
    || (kind === "array" && Array.isArray(value))
    || (kind === "string" && typeof value === "string")
    || (kind === "boolean" && typeof value === "boolean")
    || (kind === "integer" && typeof value === "number" && Number.isSafeInteger(value))
    || (kind === "number" && typeof value === "number" && Number.isFinite(value)),
  );
}

function sameJsonType(left: unknown, right: unknown): boolean {
  if (typeof left === "number" && typeof right === "number") return true;
  return typeof left === typeof right;
}

function sameJsonValue(left: unknown, right: unknown): boolean {
  if (!sameJsonType(left, right)) return false;
  try {
    return canonicalJson(left) === canonicalJson(right);
  } catch {
    return false;
  }
}

function jsonType(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  if (isObject(value)) return "object";
  if (typeof value === "number" && Number.isInteger(value)) return "integer";
  return typeof value;
}

export function canonicalJson(value: unknown, depth = 0): string {
  if (depth > MAX_TOOL_ARGUMENT_DEPTH) throw new ArgumentError(`JSON nesting exceeds ${MAX_TOOL_ARGUMENT_DEPTH} levels`);
  if (value === null) return "null";
  if (typeof value === "string" || typeof value === "boolean") return nativeJsonStringify(value);
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new ArgumentError("JSON contains a non-finite number");
    return nativeJsonStringify(value);
  }
  if (nativeArrayIsArray(value)) {
    let encoded = "[";
    for (let index = 0; index < value.length; index += 1) {
      if (index > 0) encoded += ",";
      encoded += canonicalJson(value[index], depth + 1);
    }
    return `${encoded}]`;
  }
  if (typeof value === "object" && value !== null) {
    const objectValue = value as Record<string, unknown>;
    const keys = nativeObjectKeys(objectValue);
    // Keep canonical ordering self-contained instead of consulting mutable Array.prototype.sort.
    for (let index = 1; index < keys.length; index += 1) {
      const selected = keys[index]!;
      let cursor = index - 1;
      while (cursor >= 0 && keys[cursor]! > selected) {
        keys[cursor + 1] = keys[cursor]!;
        cursor -= 1;
      }
      keys[cursor + 1] = selected;
    }
    let encoded = "{";
    for (let index = 0; index < keys.length; index += 1) {
      const key = keys[index]!;
      const child = objectValue[key];
      if (child === undefined) throw new ArgumentError(`JSON property ${key} is undefined`);
      if (index > 0) encoded += ",";
      encoded += `${nativeJsonStringify(key)}:${canonicalJson(child, depth + 1)}`;
    }
    return `${encoded}}`;
  }
  throw new ArgumentError("JSON contains an unsupported value");
}

/** Compute the catalogue's canonical SHA-256 identity for an arbitrary JSON value. */
export async function digestJson(value: unknown): Promise<string> {
  return sha256Hex(canonicalJson(value));
}

/** Hash caller-produced canonical JSON when a cross-language number policy is required. */
export async function digestCanonicalJsonText(value: string): Promise<string> {
  if (typeof value !== "string" || value.length === 0) throw new ArgumentError("canonical JSON text must be non-empty");
  return sha256Hex(value);
}

/**
 * Compute the same SHA-256 identity synchronously for small control-plane values.
 *
 * The async Web Crypto implementation remains the preferred path for general
 * catalogue/payload digests. Contextual learner selection is intentionally
 * synchronous for backwards compatibility, however, so it needs a portable
 * implementation that does not depend on Node's `crypto` module or an async
 * runtime. This is the standard SHA-256 compression function over UTF-8 bytes;
 * it is used only for bounded identity material, never provider payloads.
 */
export function digestCanonicalJsonTextSync(value: string): string {
  if (typeof value !== "string" || value.length === 0) throw new ArgumentError("canonical JSON text must be non-empty");
  return sha256HexSync(value);
}

/** Compute a canonical JSON SHA-256 identity without requiring Web Crypto. */
export function digestJsonSync(value: unknown): string {
  return sha256HexSync(canonicalJson(value));
}

/** Hash bounded caller-owned bytes without requiring Node crypto. */
export function digestBytesSync(value: Uint8Array): string {
  if (!(value instanceof NativeUint8Array)) throw new ArgumentError("SHA-256 input must be a byte array");
  return sha256BytesHexSync(value);
}

async function sha256Hex(value: string): Promise<string> {
  if (!nativeSubtleDigest) throw new ArgumentError("Web Crypto SHA-256 is required for tool catalogue digests");
  const bytes = await nativeSubtleDigest("SHA-256", nativeTextEncoderEncode.call(new NativeTextEncoder(), value));
  const view = new NativeUint8Array(bytes);
  const hex = "0123456789abcdef";
  let digest = "";
  for (let index = 0; index < view.length; index += 1) {
    const byte = view[index]!;
    digest += `${hex[byte >>> 4]}${hex[byte & 0x0f]}`;
  }
  return digest;
}

const SHA256_INITIAL_STATE = [
  0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
  0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
] as const;

const SHA256_ROUND_CONSTANTS = [
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
  0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
  0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
  0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
  0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
  0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
  0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
  0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
  0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
] as const;

function rotateRight(value: number, amount: number): number {
  return (value >>> amount) | (value << (32 - amount));
}

function sha256HexSync(value: string): string {
  return sha256BytesHexSync(nativeTextEncoderEncode.call(new NativeTextEncoder(), value));
}

function sha256BytesHexSync(source: Uint8Array): string {
  const paddedLength = Math.ceil((source.length + 9) / 64) * 64;
  const padded = new NativeUint8Array(paddedLength);
  for (let index = 0; index < source.length; index += 1) padded[index] = source[index]!;
  padded[source.length] = 0x80;
  const bitLength = source.length * 8;
  const highLength = Math.floor(bitLength / 0x1_0000_0000);
  const lowLength = bitLength >>> 0;
  const lengthOffset = padded.length - 8;
  padded[lengthOffset] = (highLength >>> 24) & 0xff;
  padded[lengthOffset + 1] = (highLength >>> 16) & 0xff;
  padded[lengthOffset + 2] = (highLength >>> 8) & 0xff;
  padded[lengthOffset + 3] = highLength & 0xff;
  padded[lengthOffset + 4] = (lowLength >>> 24) & 0xff;
  padded[lengthOffset + 5] = (lowLength >>> 16) & 0xff;
  padded[lengthOffset + 6] = (lowLength >>> 8) & 0xff;
  padded[lengthOffset + 7] = lowLength & 0xff;

  const state: number[] = [
    SHA256_INITIAL_STATE[0], SHA256_INITIAL_STATE[1], SHA256_INITIAL_STATE[2], SHA256_INITIAL_STATE[3],
    SHA256_INITIAL_STATE[4], SHA256_INITIAL_STATE[5], SHA256_INITIAL_STATE[6], SHA256_INITIAL_STATE[7],
  ];
  const schedule = new NativeUint32Array(64);
  for (let offset = 0; offset < padded.length; offset += 64) {
    for (let index = 0; index < 16; index += 1) {
      const position = offset + index * 4;
      schedule[index] = ((padded[position]! << 24) | (padded[position + 1]! << 16) | (padded[position + 2]! << 8) | padded[position + 3]!) >>> 0;
    }
    for (let index = 16; index < 64; index += 1) {
      const prior15 = schedule[index - 15]!;
      const prior2 = schedule[index - 2]!;
      const sigma0 = rotateRight(prior15, 7) ^ rotateRight(prior15, 18) ^ (prior15 >>> 3);
      const sigma1 = rotateRight(prior2, 17) ^ rotateRight(prior2, 19) ^ (prior2 >>> 10);
      schedule[index] = (schedule[index - 16]! + sigma0 + schedule[index - 7]! + sigma1) >>> 0;
    }

    let a = state[0]!;
    let b = state[1]!;
    let c = state[2]!;
    let d = state[3]!;
    let e = state[4]!;
    let f = state[5]!;
    let g = state[6]!;
    let h = state[7]!;
    for (let index = 0; index < 64; index += 1) {
      const sigma1 = rotateRight(e, 6) ^ rotateRight(e, 11) ^ rotateRight(e, 25);
      const choice = (e & f) ^ (~e & g);
      const temp1 = (h + sigma1 + choice + SHA256_ROUND_CONSTANTS[index]! + schedule[index]!) >>> 0;
      const sigma0 = rotateRight(a, 2) ^ rotateRight(a, 13) ^ rotateRight(a, 22);
      const majority = (a & b) ^ (a & c) ^ (b & c);
      const temp2 = (sigma0 + majority) >>> 0;
      h = g;
      g = f;
      f = e;
      e = (d + temp1) >>> 0;
      d = c;
      c = b;
      b = a;
      a = (temp1 + temp2) >>> 0;
    }
    state[0] = (state[0]! + a) >>> 0;
    state[1] = (state[1]! + b) >>> 0;
    state[2] = (state[2]! + c) >>> 0;
    state[3] = (state[3]! + d) >>> 0;
    state[4] = (state[4]! + e) >>> 0;
    state[5] = (state[5]! + f) >>> 0;
    state[6] = (state[6]! + g) >>> 0;
    state[7] = (state[7]! + h) >>> 0;
  }
  const hex = "0123456789abcdef";
  let digest = "";
  for (let index = 0; index < state.length; index += 1) {
    const word = state[index]!;
    for (let shift = 28; shift >= 0; shift -= 4) digest += hex[(word >>> shift) & 0x0f];
  }
  return digest;
}
