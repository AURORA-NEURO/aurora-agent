import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import vm from 'node:vm';

const app = await readFile(new URL('./app.js', import.meta.url), 'utf8');
const html = await readFile(new URL('./index.html', import.meta.url), 'utf8');
const jsonSource = await readFile(new URL('./json.js', import.meta.url), 'utf8');
const codec = await import('data:text/javascript;base64,' + Buffer.from(jsonSource).toString('base64'));
const task = 'Inspect fixed public research annotations';
const proteinOperations = ['uniprot.protein_annotation', 'reactome.pathway_annotations', 'glioma.protein_pathways.analyze', 'glioma.protein_pathways.review_handoff'];

class Element {
  constructor() { this.value = ''; this.checked = false; this.disabled = false; this.hidden = false; this.textContent = ''; this.listeners = new Map(); this.children = []; }
  addEventListener(name, callback) { const handlers = this.listeners.get(name) || []; handlers.push(callback); this.listeners.set(name, handlers); }
  async fire(name) { for (const handler of this.listeners.get(name) || []) await handler({ preventDefault() {} }); }
  append(...children) { this.children.push(...children); }
  replaceChildren(...children) { this.children = children; }
  setCustomValidity(message) { this.validity = message; }
  checkValidity() { return !this.validity; }
  reportValidity() { return this.checkValidity(); }
  querySelectorAll() { return []; }
  set innerHTML(_value) { throw new Error('HTML interpretation is forbidden'); }
}

function page(replies = []) {
  const nodes = new Map([...html.matchAll(/\bid="([^"]+)"/g)].map(match => [match[1], new Element()]));
  const options = new Map([...html.matchAll(/<option value="([^"]+)"([^>]*)>/g)].map(match => [match[1], { value: match[1], disabled: match[2].includes('disabled') }]));
  const $ = id => { assert.ok(nodes.has(id), id); return nodes.get(id); };
  Object.assign($('task'), { value: task });
  Object.assign($('planner'), { value: 'glioma-intent', querySelector: selector => options.get(selector.match(/value="([^"]+)"/)[1]) });
  Object.defineProperty($('planner'), 'selectedOptions', { get() { return [options.get(this.value) || $('capability-templates').children.find(option => option.value === this.value)]; } });
  for (const [id, value] of Object.entries({ 'max-steps': '16', 'max-model-calls': '64', 'decision-attempts': '2', 'call-budget': '32' })) $(id).value = value;
  $('source-grant').disabled = $('run').disabled = true;
  $('plan-workspace').hidden = true;
  const calls = [];
  const context = vm.createContext({
    document: { getElementById: $, createElement: () => new Element() }, crypto: { randomUUID },
    setTimeout, clearTimeout, URL, Blob, console,
    fetch: async (url, options) => {
      calls.push({ url, options, body: options.body === undefined ? undefined : JSON.parse(options.body) });
      const reply = replies.shift();
      assert.notEqual(reply, undefined, 'Every UI request needs an explicit offline reply');
      const data = await (typeof reply === 'function' ? reply(calls.at(-1)) : reply);
      if (data?.httpResponse) return data.httpResponse;
      return { ok: true, text: async () => typeof data === 'string' ? data : codec.stringify({ ok: true, data }) };
    } });
  // Both production modules share one browser realm. Keep the codec's ordinary-object
  // admission meaningful rather than passing host-realm functions into the VM.
  vm.runInContext(jsonSource.replaceAll('export function ', 'function ') + '\nconst parseJson = parse;', context, { filename: 'json.js' });
  vm.runInContext(app.replace(/^import[^\n]+\r?\n/, ''), context, { filename: 'app.js' });
  return { $, calls, options, replies, async connect(operations = [], profiles = []) {
    replies.unshift({ operations: operations.map(operation_id => ({ operation_id, description: 'Offline fixture', input_schema: {}, integration_id: 'fixture', implementation: 'local' })), operation_count: operations.length, integration_count: 1, capabilities: { profiles } });
    $('token').value = 'a'.repeat(32); await $('connect-form').fire('submit');
  } };
}

function planned() {
  return { status: 'planned', planner: 'local-incremental', task, plan_id: 'a'.repeat(32), plan_digest: 'b'.repeat(64),
    plan: { name: 'Synthetic plan', steps: [{ id: 's1', operation: 'pubmed.search', arguments: { query: 'glioma' }, depends_on: [] }], outputs: ['s1'], requirements: {} },
    interpretation: { status: 'planned', model_calls: 3, source_dispatches: 0, visible_operations: ['pubmed.search'], limits: { max_model_calls: 3, max_decision_attempts: 1 } } };
}

test('reviewed capability options require operator selection and every native operation', async () => {
  const profile = { profile_id: 'glioma-ontology', label: '<img src=x> source terminology',
    required_operations: ['nci_thesaurus.glioma_ontology'], operator_enabled: true, descriptor_available: true };
  const result = planned(); result.planner = 'capability:glioma-ontology';
  const p = page([result]);
  await p.connect([], [profile]);
  assert.equal(p.$('capability-templates').children[0].disabled, true);
  await p.connect(profile.required_operations, [{ ...profile, operator_enabled: false }]);
  assert.equal(p.$('capability-templates').children[0].disabled, true);
  await p.connect(profile.required_operations, [profile]);
  const option = p.$('capability-templates').children[0];
  assert.equal(option.disabled, false);
  assert.equal(option.textContent, profile.label);
  p.$('planner').value = option.value;
  await p.$('plan-form').fire('submit');
  assert.deepEqual(p.calls.at(-1).body, { task, planner: 'capability:glioma-ontology', max_steps: 16, approve_provider_call: false });
  assert.equal(p.$('source-grant').checked, false);
  assert.equal(p.$('run').disabled, true);
  await p.$('disconnect').fire('click');
  assert.equal(p.$('capability-templates').children.length, 0);
  assert.equal(p.$('planner').value, 'glioma-intent');
  assert.equal(p.$('source-grant').disabled, true);
});

test('capability catalogue labels remain inert and malformed or changed profiles cannot stay selected', async () => {
  const p = page();
  const profile = { profile_id: 'glioma-ontology', label: 'Terminology', required_operations: ['nci_thesaurus.glioma_ontology'], operator_enabled: true, descriptor_available: true };
  await p.connect(profile.required_operations, [profile, { ...profile, profile_id: 'bad\"]script' }, null]);
  assert.equal(p.$('capability-templates').children.length, 1);
  p.$('planner').value = 'capability:glioma-ontology';
  await p.connect(profile.required_operations, [{ ...profile, descriptor_available: false }]);
  assert.equal(p.$('planner').value, 'glioma-intent');
  assert.equal(p.$('capability-templates').children[0].disabled, true);
});

test('profile selection shows its inert server scope and clears a previous model approval', async () => {
  const profile = { profile_id: 'glioma-ontology', label: 'Terminology',
    scope: 'Fixed NCIt Glioma relationships; <img src=x onerror=alert(1)> is source text.',
    required_operations: ['nci_thesaurus.glioma_ontology'], operator_enabled: true, descriptor_available: true };
  const p = page();
  assert.equal(p.$('provider-approval').hidden, true);
  assert.equal(p.$('provider-grant').disabled, true);
  assert.match(p.$('fixed-template-scope').textContent, /Bounded glioma interpretation/);
  await p.connect(profile.required_operations, [profile]);
  p.$('planner').value = 'local-model'; await p.$('planner').fire('change');
  assert.equal(p.$('provider-approval').hidden, false);
  assert.equal(p.$('provider-grant').disabled, false);
  assert.match(p.$('fixed-template-scope').textContent, /separate approvals/);
  p.$('provider-grant').checked = true;
  p.$('planner').value = 'capability:glioma-ontology'; await p.$('planner').fire('change');
  assert.equal(p.$('fixed-template-scope').textContent, profile.scope + ' Task text does not change source queries.');
  assert.equal(p.$('provider-approval').hidden, true);
  assert.equal(p.$('provider-grant').disabled, true);
  assert.equal(p.$('provider-grant').checked, false);
  assert.equal(p.calls.length, 1);
  p.$('planner').value = 'local-incremental'; await p.$('planner').fire('change');
  assert.equal(p.$('provider-approval').hidden, false);
  assert.equal(p.$('provider-grant').disabled, false);
  assert.equal(p.$('provider-grant').checked, false);
  assert.match(p.$('provider-label').textContent, /bounded.*calls/);
  await p.$('disconnect').fire('click');
  p.$('planner').value = 'glioma-intent'; await p.$('planner').fire('change');
  assert.equal(p.$('provider-approval').hidden, true);
  assert.equal(p.$('provider-grant').checked, false);
  assert.match(p.$('fixed-template-scope').textContent, /Bounded glioma interpretation/);
});

test('each legacy template presents only its relevant fixed source scope', async () => {
  const p = page();
  const scopes = new Map([
    ['glioma-mutations', /TP53 mutation counts for TCGA-GBM\/LGG/],
    ['gene-references', /Ensembl\/NCBI TP53 references/],
    ['protein-pathways', /TP53 protein\/pathway annotations/],
    ['denominator-coverage', /TP53 and GDC SSM-data coverage for TCGA-GBM\/LGG/]
  ]);
  await p.connect(['gdc.gene_mutation_counts', 'glioma.mutation.analyze', 'glioma.mutation.review_handoff',
    'ensembl.gene_references', 'ncbi_gene.summary', 'glioma.gene_references.analyze', 'glioma.gene_references.review_handoff',
    ...proteinOperations, 'gdc.ssm_project_coverage', 'glioma.denominator.assess', 'glioma.denominator.review_handoff']);
  for (const [planner, expected] of scopes) {
    assert.equal(p.options.get(planner).disabled, false);
    p.$('planner').value = planner; await p.$('planner').fire('change');
    assert.match(p.$('fixed-template-scope').textContent, expected);
    assert.match(p.$('fixed-template-scope').textContent, /Task text does not change source queries/);
    assert.ok(p.$('fixed-template-scope').textContent.length < 160);
    assert.equal(p.$('provider-approval').hidden, true);
    assert.equal(p.$('provider-grant').checked, false);
  }
  assert.equal(p.calls.length, 1);
});

test('incremental cost controls are explicit and only incremental requests transmit them', async () => {
  const p = page([{ status: 'budget_exhausted', interpretation: { status: 'budget_exhausted', model_calls: 3, source_dispatches: 0 } }]);
  await p.connect(['pubmed.search']);
  assert.equal(p.$('incremental-controls').hidden, true);
  assert.equal(p.$('max-model-calls').disabled, true);
  p.$('planner').value = 'local-incremental'; await p.$('planner').fire('change');
  assert.equal(p.$('incremental-controls').hidden, false);
  assert.equal(p.$('max-model-calls').disabled, false);
  assert.match(p.$('provider-label').textContent, /bounded.*calls/);
  p.$('max-model-calls').value = '3'; p.$('decision-attempts').value = '1';
  p.$('operation-ids').value = '["pubmed.search"]'; p.$('provider-grant').checked = true;
  await p.$('plan-form').fire('submit');
  assert.deepEqual(p.calls.at(-1).body, { task, planner: 'local-incremental', max_steps: 16,
    approve_provider_call: true, max_model_calls: 3, decision_attempts: 1, operation_ids: ['pubmed.search'] });
  assert.match(p.$('interpretation').textContent, /budget_exhausted/);
  assert.equal(p.$('plan-workspace').hidden, true);
  assert.equal(p.$('source-grant').disabled, true);
  assert.equal(p.$('run').disabled, true);
  assert.equal(p.$('provider-grant').checked, false);
});

test('protein template requires all four installed operations and rechecks on disconnect', async () => {
  const p = page();
  assert.equal(p.options.get('protein-pathways').disabled, true);
  await p.connect(proteinOperations.slice(0, -1));
  assert.equal(p.options.get('protein-pathways').disabled, true);
  await p.connect(proteinOperations);
  assert.equal(p.options.get('protein-pathways').disabled, false);
  p.$('planner').value = 'protein-pathways';
  await p.$('disconnect').fire('click');
  assert.equal(p.options.get('protein-pathways').disabled, true);
  assert.equal(p.$('planner').value, 'glioma-intent');
});

test('denominator template requires its exact source and analysis operations', async () => {
  const p = page();
  const operations = ['gdc.gene_mutation_counts', 'gdc.ssm_project_coverage', 'glioma.denominator.assess', 'glioma.denominator.review_handoff'];
  await p.connect(operations.slice(0, -1));
  assert.equal(p.options.get('denominator-coverage').disabled, true);
  await p.connect(operations);
  assert.equal(p.options.get('denominator-coverage').disabled, false);
  p.$('planner').value = 'denominator-coverage';
  await p.$('disconnect').fire('click');
  assert.equal(p.options.get('denominator-coverage').disabled, true);
  assert.equal(p.$('planner').value, 'glioma-intent');
});

test('provider inspection is explicit inert text and clears on disconnect', async () => {
  const p = page();
  await p.$('inspect-providers').fire('click');
  assert.equal(p.calls.length, 0);
  await p.connect([]);
  p.replies.push({ counts: { implemented_reader_paths: 16, independently_live_verified_providers: { value: null } }, note: '<img src=x onerror=alert(1)>' });
  await p.$('inspect-providers').fire('click');
  assert.equal(p.calls.at(-1).url, '/v1/executable-workflows/providers');
  assert.equal(p.calls.at(-1).body, undefined);
  assert.match(p.$('provider-coverage').textContent, /independently_live_verified_providers/);
  assert.match(p.$('provider-coverage').textContent, /<img/);
  await p.$('disconnect').fire('click');
  assert.equal(p.$('provider-coverage').textContent, '');
});

test('successful validation keeps diagnostics available without obscuring the workflow', async () => {
  const value = planned();
  value.validation = { reference_paths: { blocking_issue_count: 0, runtime_resolution_required: true } };
  const p = page([value]);
  await p.connect(['pubmed.search']);
  p.$('planner').value = 'local-incremental'; p.$('provider-grant').checked = true;
  await p.$('plan-form').fire('submit');
  assert.equal(p.$('plan-workspace').hidden, false);
  assert.equal(p.$('diagnostics-panel').hidden, false);
  assert.equal(p.$('diagnostics-panel').open, false);
  assert.match(p.$('diagnostics').textContent, /runtime_resolution_required/);
  p.replies.push(JSON.stringify({ ok: false, error: { message: 'The proposal violates its retained task.',
    diagnostics: { requirements_audit: { contract_satisfied: false, blocking_issues: [{ code: 'required_output_missing' }] } } } }));
  p.$('provider-grant').checked = true;
  await p.$('plan-form').fire('submit');
  assert.equal(p.$('plan-workspace').hidden, true);
  assert.equal(p.$('diagnostics-panel').open, true);
  assert.match(p.$('diagnostics').textContent, /required_output_missing/);
  assert.equal(p.$('source-grant').disabled, true);
});

test('another planner never inherits hidden incremental controls or operation allowlists', async () => {
  const p = page([{ status: 'needs_clarification', interpretation: { status: 'needs_clarification' } }]);
  await p.connect([]);
  p.$('max-model-calls').value = '128'; p.$('decision-attempts').value = '4'; p.$('operation-ids').value = '["unregistered.operation"]';
  await p.$('plan-form').fire('submit');
  assert.deepEqual(p.calls.at(-1).body, { task, planner: 'glioma-intent', max_steps: 16, approve_provider_call: false });
  assert.equal(p.$('source-grant').disabled, true);
});

test('a nonplanned result clears an earlier saved plan and source approval before the response arrives', async () => {
  let release;
  const pending = new Promise(resolve => { release = resolve; });
  const p = page([planned(), () => pending]);
  await p.connect(['pubmed.search']);
  p.$('planner').value = 'local-incremental'; p.$('provider-grant').checked = true;
  await p.$('plan-form').fire('submit');
  assert.equal(p.$('plan-workspace').hidden, false);
  p.$('source-grant').checked = true; await p.$('source-grant').fire('change');
  assert.equal(p.$('run').disabled, false);
  p.$('provider-grant').checked = true;
  const building = p.$('plan-form').fire('submit');
  assert.equal(p.$('source-grant').checked, false);
  assert.equal(p.$('source-grant').disabled, true);
  assert.equal(p.$('run').disabled, true);
  assert.equal(p.$('plan-workspace').hidden, true);
  release({ status: 'refused', interpretation: { status: 'refused', message: '<img src=x onerror=alert(1)>' } });
  await building;
  assert.equal(p.$('plan-workspace').hidden, true);
  assert.match(p.$('interpretation').textContent, /<img/);
  assert.match(p.$('error').textContent, /No executable plan was saved/);
  assert.equal(p.calls.filter(call => call.url.endsWith('/runs')).length, 0);
});

for (const failure of [
  { name: 'HTTP 500 with a success-shaped body', reply: { httpResponse: {
    status: 500, ok: false, text: async () => codec.stringify({ ok: true, data: planned() }) } },
    message: /API refused/ },
  { name: 'a native validation failure', reply: JSON.stringify({ ok: false,
    error: { code: 'validation_failed', message: 'The new profile exceeds its fixed step bound.',
      diagnostics: { requirements_audit: { contract_satisfied: false } } } }),
    message: /fixed step bound/ }
]) {
  test(`a new profile clears prior interpretation while pending and after ${failure.name}`, async () => {
    let release;
    const pending = new Promise(resolve => { release = resolve; });
    const previous = planned(); previous.planner = 'capability:glioma-ontology';
    previous.interpretation = { profile_id: 'glioma-ontology', provider_calls: 0, scientific_validity: 'unassessed' };
    const profiles = [
      { profile_id: 'glioma-ontology', label: 'NCI terminology', required_operations: ['nci_thesaurus.glioma_ontology'],
        operator_enabled: true, descriptor_available: true },
      { profile_id: 'gene-identity', label: 'Gene identifier review', required_operations: ['hgnc.gene_references',
        'ensembl.gene_references', 'ncbi_gene.summary', 'glioma.gene_identity.analyze', 'glioma.gene_identity.review_handoff'],
        operator_enabled: true, descriptor_available: true }
    ];
    const p = page([previous, () => pending]);
    await p.connect(profiles.flatMap(profile => profile.required_operations), profiles);
    p.$('planner').value = previous.planner; await p.$('planner').fire('change');
    await p.$('plan-form').fire('submit');
    assert.equal(p.$('interpretation-panel').hidden, false);
    assert.equal(p.$('interpretation-panel').open, true);
    assert.match(p.$('interpretation').textContent, /glioma-ontology/);
    p.$('source-grant').checked = true; await p.$('source-grant').fire('change');
    assert.equal(p.$('run').disabled, false);
    p.$('planner').value = 'capability:gene-identity'; await p.$('planner').fire('change');
    const building = p.$('plan-form').fire('submit');
    assert.equal(p.$('interpretation-panel').hidden, true);
    assert.equal(p.$('interpretation-panel').open, false);
    assert.equal(p.$('interpretation').textContent, '');
    assert.equal(p.$('plan-workspace').hidden, true);
    assert.equal(p.$('source-grant').checked, false);
    assert.equal(p.$('source-grant').disabled, true);
    assert.equal(p.$('run').disabled, true);
    release(failure.reply); await building;
    assert.match(p.$('error').textContent, failure.message);
    assert.equal(p.$('interpretation-panel').hidden, true);
    assert.equal(p.$('interpretation-panel').open, false);
    assert.equal(p.$('interpretation').textContent, '');
    assert.equal(p.$('plan-workspace').hidden, true);
    assert.equal(p.$('source-grant').checked, false);
    assert.equal(p.$('run').disabled, true);
    assert.equal(p.$('build').disabled, false);
    assert.equal(p.calls.at(-1).body.planner, 'capability:gene-identity');
    assert.equal(p.calls.at(-1).body.approve_provider_call, false);
    assert.equal(p.calls.filter(call => call.url.endsWith('/runs')).length, 0);
  });
}

test('changing a cost bound while planning prevents stale plan acceptance or source approval', async () => {
  let release;
  const pending = new Promise(resolve => { release = resolve; });
  const p = page([() => pending]);
  await p.connect(['pubmed.search']);
  p.$('planner').value = 'local-incremental'; p.$('provider-grant').checked = true;
  const building = p.$('plan-form').fire('submit');
  p.$('max-model-calls').value = '4'; await p.$('max-model-calls').fire('input');
  release(planned()); await building;
  assert.match(p.$('error').textContent, /controls changed/);
  assert.equal(p.$('source-grant').disabled, true);
  assert.equal(p.$('plan-workspace').hidden, true);
});

test('UI proposal editing keeps exact evidence number lexemes through incremental plan inspection', async () => {
  const data = planned();
  const envelope = JSON.stringify({ ok: true, data }).replace('"query":"glioma"', '"large":1208925819614629174706176,"zero":0.0,"negative_zero":-0.0');
  const p = page([envelope]); await p.connect(['pubmed.search']);
  p.$('planner').value = 'local-incremental'; p.$('provider-grant').checked = true;
  await p.$('plan-form').fire('submit');
  p.$('plan-name').value = 'Edited label'; await p.$('plan-name').fire('input');
  for (const number of ['1208925819614629174706176', '0.0', '-0.0']) assert.ok(p.$('proposal').value.includes(number), number);
  assert.equal(p.$('source-grant').checked, false);
  assert.equal(p.$('run').disabled, true);
});
