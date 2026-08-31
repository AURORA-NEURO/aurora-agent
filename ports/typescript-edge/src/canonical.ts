/**
 * Canonical JSON: byte-for-byte parity with the reference layers' serializer,
 * CPython `json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)`.
 *
 * JavaScript's built-in JSON.stringify is not used because it diverges from the reference in
 * ways that would silently break byte equality: it drops object entries whose value is
 * `undefined` instead of erroring, renders 1e16 as "10000000000000000" where CPython switches
 * to exponent form, and its key sort is UTF-16 code-unit order rather than code-point order.
 * Every divergence here is either a thrown typed error or an exact match - never a best-effort
 * rendering that could differ between runtimes.
 *
 * Value domain: null, boolean, safe-integer number, string, array, plain object. Floats are
 * excluded on purpose: matching CPython's repr-based float formatting across engines is a
 * separate tested problem, and assuming Number-to-string agrees with repr above 1e16 is how a
 * parity claim quietly becomes false.
 */

export type CanonicalValue =
  | null
  | boolean
  | number
  | string
  | CanonicalValue[]
  | { readonly [key: string]: CanonicalValue };

/** Reports input a canonical serialization refused, always naming what and why. */
export class CanonicalJSONError extends TypeError {
  constructor(reason: string) {
    super(`canonical JSON refuses this value: ${reason}`);
    this.name = "CanonicalJSONError";
  }
}

interface EscapePair {
  readonly from: string;
  readonly to: string;
}

const STRING_ESCAPES: readonly EscapePair[] = [
  { from: String.fromCharCode(0x22), to: String.fromCharCode(0x5c, 0x22) },
  { from: String.fromCharCode(0x5c), to: String.fromCharCode(0x5c, 0x5c) },
  { from: String.fromCharCode(0x08), to: String.fromCharCode(0x5c, 0x62) },
  { from: String.fromCharCode(0x0c), to: String.fromCharCode(0x5c, 0x66) },
  { from: String.fromCharCode(0x0a), to: String.fromCharCode(0x5c, 0x6e) },
  { from: String.fromCharCode(0x0d), to: String.fromCharCode(0x5c, 0x72) },
  { from: String.fromCharCode(0x09), to: String.fromCharCode(0x5c, 0x74) },
];

const HEX_DIGITS = "0123456789abcdef";

/**
 * Python sorts keys by code point; Array#sort's default compares UTF-16 code units, which
 * orders surrogate pairs before U+E000..U+FFFF characters. Walking code points keeps the two
 * orders identical everywhere, not just on ASCII inventories.
 */
export function compareCodePoints(a: string, b: string): number {
  const ia = Array.from(a);
  const ib = Array.from(b);
  const n = Math.min(ia.length, ib.length);
  for (let i = 0; i < n; i += 1) {
    const ca = ia[i]!.codePointAt(0)!;
    const cb = ib[i]!.codePointAt(0)!;
    if (ca !== cb) return ca < cb ? -1 : 1;
  }
  if (ia.length === ib.length) return 0;
  return ia.length < ib.length ? -1 : 1;
}

function isLoneSurrogateChar(ch: string): boolean {
  const cp = ch.codePointAt(0)!;
  const alone = cp >= 0xd800 && cp <= 0xdfff;
  return alone && ch.length === 1 && ch.codePointAt(0)! === cp;
}

function writeEscapedString(out: string[], s: string): void {
  out.push(String.fromCharCode(0x22));
  for (const ch of s) {
    const pair = STRING_ESCAPES.find((e) => e.from === ch);
    if (pair !== undefined) {
      out.push(pair.to);
      continue;
    }
    const cp = ch.codePointAt(0)!;
    if (cp < 0x20) {
      out.push(
        String.fromCharCode(
          0x5c, 0x75, 0x30, 0x30,
          HEX_DIGITS.charCodeAt((cp >> 4) & 0xf),
          HEX_DIGITS.charCodeAt(cp & 0xf),
        ),
      );
      continue;
    }
    // A lone surrogate has no Unicode character. CPython emits one raw and then fails at
    // UTF-8 encode time; failing here at serialize time names the defect instead of deferring
    // it to whichever transport next touches the bytes.
    if (isLoneSurrogateChar(ch)) {
      throw new CanonicalJSONError(
        `lone surrogate U+${cp.toString(16).padStart(4, "0")} cannot be encoded as UTF-8`,
      );
    }
    out.push(ch);
  }
  out.push(String.fromCharCode(0x22));
}

function writeValue(out: string[], value: CanonicalValue, ancestors: object[]): void {
  if (value === null) {
    out.push("null");
    return;
  }
  switch (typeof value) {
    case "boolean":
      out.push(value ? "true" : "false");
      return;
    case "number":
      if (!Number.isSafeInteger(value)) {
        throw new CanonicalJSONError(
          `${value} is not a safe integer; floats and oversized counters are outside this serializer's domain`,
        );
      }
      out.push(String(value));
      return;
    case "string":
      writeEscapedString(out, value);
      return;
    default:
      break;
  }
  if (Array.isArray(value)) {
    if (ancestors.includes(value)) {
      throw new CanonicalJSONError("cyclic structure");
    }
    ancestors.push(value);
    out.push("[");
    for (let i = 0; i < value.length; i += 1) {
      if (i > 0) out.push(",");
      writeValue(out, value[i]!, ancestors);
    }
    out.push("]");
    ancestors.pop();
    return;
  }
  if (typeof value === "object") {
    if (ancestors.includes(value)) {
      throw new CanonicalJSONError("cyclic structure");
    }
    ancestors.push(value);
    out.push("{");
    const keys = Object.keys(value).sort(compareCodePoints);
    let first = true;
    for (const key of keys) {
      if (!first) out.push(",");
      first = false;
      writeEscapedString(out, key);
      out.push(":");
      writeValue(out, value[key]!, ancestors);
    }
    out.push("}");
    ancestors.pop();
    return;
  }
  throw new CanonicalJSONError(`unsupported value type ${typeof value}`);
}

/** Serializes `value` to canonical bytes-as-string matching the CPython reference exactly. */
export function canonicalJsonString(value: CanonicalValue): string {
  const out: string[] = [];
  writeValue(out, value, []);
  return out.join("");
}
