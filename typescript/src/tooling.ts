import { ArgumentError, isObject } from "./errors.js";
import type {
  JsonObject,
  ToolArguments,
  ToolCallPlan,
  ToolDefinition,
  ToolValidationIssue,
  ToolValidationReport,
} from "./types.js";

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
    issues.push({ path, code: "type", message: `expected JSON type ${JSON.stringify(expected)}, got ${jsonType(value)}` });
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
    const childPath = /^[A-Za-z_$][\w$]*$/.test(name) ? `${path}.${name}` : `${path}[${JSON.stringify(name)}]`;
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
  if (typeof value === "string" || typeof value === "boolean") return JSON.stringify(value);
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new ArgumentError("JSON contains a non-finite number");
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) return `[${value.map((item) => canonicalJson(item, depth + 1)).join(",")}]`;
  if (isObject(value)) {
    const entries = Object.keys(value).sort().map((key) => {
      const child = value[key];
      if (child === undefined) throw new ArgumentError(`JSON property ${key} is undefined`);
      return `${JSON.stringify(key)}:${canonicalJson(child, depth + 1)}`;
    });
    return `{${entries.join(",")}}`;
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

async function sha256Hex(value: string): Promise<string> {
  const subtle = globalThis.crypto?.subtle;
  if (!subtle) throw new ArgumentError("Web Crypto SHA-256 is required for tool catalogue digests");
  const bytes = await subtle.digest("SHA-256", new TextEncoder().encode(value));
  return [...new Uint8Array(bytes)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}
