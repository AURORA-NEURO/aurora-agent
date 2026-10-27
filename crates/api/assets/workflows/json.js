// Keep original JSON number tokens while editing Python-owned plans and contracts.
class JsonNumber {
  constructor(raw) { this.raw = raw; Object.freeze(this); }
  [Symbol.toPrimitive](hint) { return hint === 'number' ? Number(this.raw) : this.raw; }
}

const MAX_BYTES = 16_000_000;
const MAX_NODES = 250_000;

export function parse(text) {
  if (typeof text !== 'string') throw new TypeError('JSON input must be text.');
  if (text.length > MAX_BYTES) throw new SyntaxError('JSON exceeds the 16000000-byte editor bound.');
  let inputBytes = 0;
  for (const character of text) { const point = character.codePointAt(0); inputBytes += point <= 0x7f ? 1 : point <= 0x7ff ? 2 : point <= 0xffff ? 3 : 4; if (inputBytes > MAX_BYTES) throw new SyntaxError('JSON exceeds the 16000000-byte editor bound.'); }
  let cursor = 0; let nodes = 0;
  const number = /-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/y;
  function invalid() { throw new SyntaxError('Enter valid JSON with unique object fields.'); }
  function space() { while (cursor < text.length && /[ \t\r\n]/.test(text[cursor])) cursor += 1; }
  function string() {
    if (text[cursor] !== '"') return invalid();
    const start = cursor++;
    while (cursor < text.length) {
      if (text[cursor] === '\\') { cursor += 2; continue; }
      if (text[cursor++] === '"') return JSON.parse(text.slice(start, cursor));
    }
    return invalid();
  }
  function value(depth) {
    if (++nodes > MAX_NODES) throw new SyntaxError('JSON exceeds the 250000-node editor bound.');
    if (depth > 128) throw new SyntaxError('JSON nesting exceeds the editor bound.');
    space(); const first = text[cursor];
    if (first === '"') return string();
    if (first === '{') {
      cursor += 1; space(); const object = Object.create(null);
      if (text[cursor] === '}') { cursor += 1; return object; }
      while (cursor < text.length) {
        space(); const key = string();
        if (Object.hasOwn(object, key)) return invalid();
        space(); if (text[cursor++] !== ':') return invalid();
        object[key] = value(depth + 1); space();
        const separator = text[cursor++];
        if (separator === '}') return object;
        if (separator !== ',') return invalid();
      }
      return invalid();
    }
    if (first === '[') {
      cursor += 1; space(); const array = [];
      if (text[cursor] === ']') { cursor += 1; return array; }
      while (cursor < text.length) {
        array.push(value(depth + 1)); space(); const separator = text[cursor++];
        if (separator === ']') return array;
        if (separator !== ',') return invalid();
      }
      return invalid();
    }
    for (const [literal, parsed] of [['true', true], ['false', false], ['null', null]]) {
      if (text.startsWith(literal, cursor)) { cursor += literal.length; return parsed; }
    }
    number.lastIndex = cursor; const matched = number.exec(text);
    if (matched) { cursor = number.lastIndex; return new JsonNumber(matched[0]); }
    return invalid();
  }
  const result = value(0); space(); if (cursor !== text.length) return invalid(); return result;
}

export function stringify(value) {
  const active = new Set(); let emittedBytes = 0; let nodes = 0;
  function consume(count) { emittedBytes += count; if (emittedBytes > MAX_BYTES) throw new TypeError('JSON exceeds the 16000000-byte editor output bound.'); }
  function quoted(text) {
    consume(2);
    for (let index = 0; index < text.length; index += 1) {
      const code = text.charCodeAt(index);
      if (code === 34 || code === 92) consume(2);
      else if (code < 32) consume([8,9,10,12,13].includes(code) ? 2 : 6);
      else if (code >= 0xd800 && code <= 0xdbff) {
        const next = text.charCodeAt(index + 1);
        if (next >= 0xdc00 && next <= 0xdfff) { consume(4); index += 1; } else consume(6);
      } else if (code >= 0xdc00 && code <= 0xdfff) consume(6);
      else consume(code <= 0x7f ? 1 : code <= 0x7ff ? 2 : 3);
    }
    return JSON.stringify(text);
  }
  function encode(current, depth) {
    if (++nodes > MAX_NODES) throw new TypeError('JSON exceeds the 250000-node editor bound.');
    if (depth > 128) throw new TypeError('JSON nesting exceeds the editor bound.');
    if (current instanceof JsonNumber) { consume(current.raw.length); return current.raw; }
    if (current === null) { consume(4); return 'null'; }
    if (typeof current === 'string') return quoted(current);
    if (typeof current === 'boolean') { const encoded = JSON.stringify(current); consume(encoded.length); return encoded; }
    if (typeof current === 'number') {
      if (!Number.isFinite(current)) throw new TypeError('Enter a finite number.');
      const encoded = JSON.stringify(current); consume(encoded.length); return encoded;
    }
    if (typeof current !== 'object' || active.has(current)) throw new TypeError('The editor accepts acyclic JSON values.');
    if (!Array.isArray(current) && ![null,Object.prototype].includes(Object.getPrototypeOf(current))) throw new TypeError('The editor accepts ordinary JSON objects.');
    active.add(current);
    const isArray = Array.isArray(current); const keys = isArray ? current.keys() : Object.keys(current);
    const rows = []; const indentation = '  '.repeat(depth + 1); const closing = '  '.repeat(depth);
    const length = isArray ? current.length : keys.length;
    if (!length) { consume(2); active.delete(current); return isArray ? '[]' : '{}'; }
    consume(2); let index = 0;
    for (const key of keys) {
      consume(indentation.length); let row = indentation;
      if (!isArray) { row += quoted(key); consume(2); row += ': '; }
      row += encode(current[key], depth + 1); rows.push(row);
      consume(++index < length ? 2 : 1);
    }
    consume(closing.length + 1);
    const encoded = (isArray ? '[\n' : '{\n') + rows.join(',\n') + '\n' + closing + (isArray ? ']' : '}');
    active.delete(current); return encoded;
  }
  return encode(value, 0);
}
