import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
const source = await readFile(new URL('./json.js', import.meta.url), 'utf8');
const { parse, stringify } = await import('data:text/javascript;base64,' + Buffer.from(source).toString('base64'));

test('editing a name retains large integers and Python float lexemes exactly', () => {
  const original = '{"name":"before","arguments":{"large":1208925819614629174706176,"zero":0.0,"negative_zero":-0.0,"small":1e-09,"scientific":1E+20}}';
  const proposal = parse(original); proposal.name = 'after'; const encoded = stringify(proposal);
  for (const literal of ['1208925819614629174706176', '0.0', '-0.0', '1e-09', '1E+20']) assert.ok(encoded.includes(literal), literal);
  assert.equal(String(parse(encoded).arguments.large), '1208925819614629174706176');
  assert.equal(parse(encoded).name, 'after');
});

test('strings, escapes, marker-like text and metadata-shaped objects remain ordinary data', () => {
  const examples = [null, true, false, '123', '__aurora_json_number_0__', 'quote " and slash \\ and \n newline', 'glioma α 🧬', { raw: '123', nested: ['x', false, null] }];
  for (const value of examples) assert.deepEqual(JSON.parse(stringify(parse(JSON.stringify(value)))), value);
  const hostile = parse('{"__proto__":{"polluted":true},"constructor":"text","prototype":{"constructor":{"prototype":{"polluted":true}}}}');
  assert.equal(Object.getPrototypeOf(hostile), null); assert.equal(Object.hasOwn(hostile, '__proto__'), true);
  assert.equal({}.polluted, undefined); assert.equal(JSON.parse(stringify(hostile)).constructor, 'text');
  assert.equal(Object.getPrototypeOf(hostile.prototype.constructor.prototype),null);
  assert.equal(JSON.parse(stringify(hostile)).prototype.constructor.prototype.polluted,true);
});

test('all number forms survive a parse and edit serialization without coercion', () => {
  const literals='[0,-0,0.0,-0.0,1.00e+02,9007199254740993,1208925819614629174706176,0.12345678901234567890123456789,1E-4000]';
  assert.equal(stringify(parse(literals)).replace(/\s/g,''),literals);
});

test('ambiguous duplicate keys including escaped aliases are refused before request generation', () => {
  for (const text of ['{"approve":false,"approve":true}', '{"args":{"limit":1,"limit":2}}', '{"a":1,"\\u0061":2}']) assert.throws(() => parse(text), /unique object fields/);
});

test('invalid JSON cannot be normalized into an accepted proposal', () => {
  for (const text of ['', '01', '+1', '1.', '1e', 'NaN', 'Infinity', '[1,]', '{"a":1,}', '{a:1}', 'true false', '"bad\\x20escape"', '"raw\nnewline"', '\v1']) assert.throws(() => parse(text), undefined, text);
});

test('nested structures and unsupported generated values have explicit failure bounds', () => {
  assert.throws(() => parse('['.repeat(140) + '0' + ']'.repeat(140)), /nesting/);
  const cycle = {}; cycle.self = cycle; assert.throws(() => stringify(cycle), /acyclic/);
  assert.throws(() => stringify(NaN), /finite/); assert.throws(() => stringify(Infinity), /finite/);
  assert.throws(() => stringify(undefined), /acyclic/);
});

test('UTF-8 input bytes and wide node counts are bounded before parsing can grow', () => {
  assert.throws(() => parse('"' + 'é'.repeat(8_000_000) + '"'), /byte editor bound/);
  assert.throws(() => parse('[' + 'null,'.repeat(250_000) + 'null]'), /node editor bound/);
});

test('encoding checks node count and pretty JSON bytes before wide joins allocate output', () => {
  assert.throws(() => stringify(Array(250_001).fill(null)), /node editor bound/);
  const compact = Array(100_000).fill('x'.repeat(155));
  assert.ok(JSON.stringify(compact).length < 16_000_000);
  assert.throws(() => stringify(compact), /byte editor output bound/);
  assert.throws(() => stringify('é'.repeat(8_000_000)), /byte editor output bound/);
});
