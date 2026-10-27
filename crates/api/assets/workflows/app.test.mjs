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

function declarationReview(overrides = {}) {
  return { status: 'review_required', review_id: 'c'.repeat(32), review_digest: 'd'.repeat(64), task, planner: 'local-model',
    context: { task, planner: 'local-model', max_steps: 16, planning_attempts: 1, max_model_calls: null, decision_attempts: null, operation_ids: null,
      pre_obligation_requirements: { schema: 'offline-contract', retained: 'caller contract' } },
    candidate: { interpretation_basis: 'model_candidate', obligations: [{ id: 'required-read', kind: 'required_operation', span: { start: 0, end: 7, text: 'Inspect' }, operation: 'pubmed.search', output: false }] },
    candidate_admission: { status: 'blocked', blocking_issues: [{ code: 'unreviewed_model_candidate' }] }, acceptance: null, source_dispatch: false, ...overrides };
}
function acceptedReview(review = declarationReview(), status = 'ready_for_constrained_authoring') {
  return { ...review, status: 'accepted', acceptance: { acceptance_digest: 'e'.repeat(64), admission: { status } } };
}
function children(node) { return [node, ...node.children.flatMap(children)]; }
function declarationField(p, name) { const node = children(p.$('mandatory-rows')).find(child => child.name === name); assert.ok(node, name); return node; }
async function manualDeclaration(p) {
  p.$('planner').value = 'local-model'; await p.$('planner').fire('change');
  p.$('mandatory-enabled').checked = true; await p.$('mandatory-enabled').fire('change');
  p.$('task').selectionStart = 0; p.$('task').selectionEnd = 7; await p.$('mandatory-add').fire('click');
  const operation = declarationField(p, 'Operation ID'); operation.value = 'pubmed.search'; await operation.fire('input');
}
async function prepareAndAccept(p, review = declarationReview()) {
  p.replies.push(review); await p.$('mandatory-prepare').fire('click');
  p.$('mandatory-accept-grant').checked = true; await p.$('mandatory-accept-grant').fire('change');
  p.replies.push(acceptedReview(review)); await p.$('mandatory-accept').fire('click');
}

test('mandatory declarations are opt-in and require separate review acceptance before model planning', async () => {
  const p = page(); await p.connect(['pubmed.search']); await manualDeclaration(p);
  await p.$('plan-form').fire('submit');
  assert.equal(p.calls.length, 1); assert.match(p.$('error').textContent, /explicitly accept/);
  await prepareAndAccept(p);
  const preparation = p.calls.find(call => call.url.endsWith('/intent-reviews/prepare'));
  assert.deepEqual(preparation.body.obligations[0], { id: preparation.body.obligations[0].id, kind: 'required_operation', span: { start: 0, end: 7, text: 'Inspect' }, operation: 'pubmed.search', output: false });
  assert.equal(preparation.body.interpretation_basis, undefined);
  assert.match(p.$('mandatory-findings').textContent, /model_candidate/);
  assert.equal(p.$('source-grant').disabled, true);
  p.replies.push({ ...planned(), planner: 'local-model', intent_review: { review_id: 'c'.repeat(32) } });
  p.$('provider-grant').checked = true; await p.$('plan-form').fire('submit');
  assert.deepEqual(p.calls.at(-1).body, { task, planner: 'local-model', max_steps: 16, approve_provider_call: true, intent_review_id: 'c'.repeat(32), expected_acceptance_digest: 'e'.repeat(64) });
  assert.equal(p.$('source-grant').checked, false); assert.equal(p.$('run').disabled, true);
  assert.equal(p.calls.filter(call => call.url.endsWith('/runs')).length, 0);
});

test('task selection converts browser UTF16 units to Python code point offsets without repairing split characters', async () => {
  const p = page(); await p.connect(['pubmed.search']);
  p.$('planner').value = 'local-model'; await p.$('planner').fire('change');
  p.$('mandatory-enabled').checked = true; await p.$('mandatory-enabled').fire('change');
  p.$('task').value = '🧠 Read 🧬 then Read'; await p.$('task').fire('input');
  p.$('task').selectionStart = 3; p.$('task').selectionEnd = 7; await p.$('mandatory-add').fire('click');
  assert.equal(declarationField(p, 'Start (Unicode code points)').value, '2');
  assert.equal(declarationField(p, 'Exact task excerpt').value, 'Read');
  declarationField(p, 'Operation ID').value = 'pubmed.search'; await declarationField(p, 'Operation ID').fire('input');
  p.replies.push(declarationReview({ task: p.$('task').value })); await p.$('mandatory-prepare').fire('click');
  assert.deepEqual(p.calls.at(-1).body.obligations[0].span, { start: 2, end: 6, text: 'Read' });
  p.$('task').selectionStart = 1; p.$('task').selectionEnd = 2; await p.$('mandatory-add').fire('click');
  assert.match(p.$('error').textContent, /complete Unicode/);
  assert.equal(p.$('mandatory-rows').children.length, 1);
});

test('mandatory literal JSON keeps integer float boolean and negative zero number tokens exact', async () => {
  const p = page(); await p.connect(['pubmed.search']); await manualDeclaration(p);
  declarationField(p, 'Kind').value = 'argument_equals'; await declarationField(p, 'Kind').fire('change');
  declarationField(p, 'Argument JSON pointer').value = '/panel'; await declarationField(p, 'Argument JSON pointer').fire('input');
  declarationField(p, 'Literal JSON value').value = '[9007199254740993,1,1.0,true,-0.0]'; await declarationField(p, 'Literal JSON value').fire('input');
  p.replies.push(declarationReview()); await p.$('mandatory-prepare').fire('click');
  assert.ok(p.calls.at(-1).options.body.includes('9007199254740993'));
  assert.ok(p.calls.at(-1).options.body.includes('1,\n      1.0,\n      true,\n      -0.0') || /1,\s+1\.0,\s+true,\s+-0\.0/.test(p.calls.at(-1).options.body));
  declarationField(p, 'Literal JSON value').value = '{"a":1,"a":2}'; await declarationField(p, 'Literal JSON value').fire('input');
  const count = p.calls.length; await p.$('mandatory-prepare').fire('click'); assert.equal(p.calls.length, count);
  assert.match(p.$('error').textContent, /unique object fields/);
});

test('accepted declarations with blocking admission cannot authorize planning', async () => {
  const p = page(); await p.connect(['pubmed.search']); await manualDeclaration(p);
  p.replies.push(declarationReview()); await p.$('mandatory-prepare').fire('click');
  p.$('mandatory-accept-grant').checked = true; await p.$('mandatory-accept-grant').fire('change');
  p.replies.push(acceptedReview(declarationReview(), 'blocked')); await p.$('mandatory-accept').fire('click');
  assert.match(p.$('mandatory-status').textContent, /still blocks/);
  const count = p.calls.length; await p.$('plan-form').fire('submit'); assert.equal(p.calls.length, count);
  assert.equal(p.$('source-grant').disabled, true);
});

for (const edited of ['task', 'max-steps', 'max-model-calls', 'decision-attempts', 'operation-ids']) {
  test(`editing ${edited} invalidates caller acceptance before a subsequent plan`, async () => {
    const p = page(); await p.connect(['pubmed.search']); await manualDeclaration(p); await prepareAndAccept(p);
    p.$(edited).value += ' '; await p.$(edited).fire('input');
    assert.equal(p.$('mandatory-accept-grant').checked, false); assert.equal(p.$('mandatory-accept-grant').disabled, true);
    const count = p.calls.length; await p.$('plan-form').fire('submit'); assert.equal(p.calls.length, count);
  });
}

test('editing declaration rows or a failed request clears acceptance and cannot retain an earlier review', async () => {
  const p = page(); await p.connect(['pubmed.search']); await manualDeclaration(p); await prepareAndAccept(p);
  const operation = declarationField(p, 'Operation ID'); operation.value = 'unavailable.write'; await operation.fire('input');
  assert.equal(p.$('mandatory-findings').textContent, ''); assert.equal(p.$('mandatory-accept-grant').disabled, true);
  operation.value = 'pubmed.search'; await operation.fire('input'); await prepareAndAccept(p);
  p.replies.push(JSON.stringify({ ok: false, error: { message: 'Inspection failed.' } })); await p.$('inspect-providers').fire('click');
  const count = p.calls.length; await p.$('plan-form').fire('submit'); assert.equal(p.calls.length, count);
});

test('a racing prepare response after row edits or disconnect cannot enable acceptance', async () => {
  for (const change of ['row', 'disconnect']) {
    let release; const pending = new Promise(resolve => { release = resolve; });
    const p = page(); await p.connect(['pubmed.search']); await manualDeclaration(p);
    p.replies.push(() => pending); const preparing = p.$('mandatory-prepare').fire('click');
    if (change === 'row') { declarationField(p, 'Operation ID').value = 'other.operation'; await declarationField(p, 'Operation ID').fire('input'); }
    else await p.$('disconnect').fire('click');
    release(declarationReview()); await preparing;
    assert.equal(p.$('mandatory-accept-grant').disabled, true); assert.equal(p.$('mandatory-findings').textContent, '');
    assert.equal(p.$('source-grant').disabled, true); assert.match(p.$('error').textContent, /changed while preparing/);
  }
});

test('suggestions require a separate provider grant and stay unreviewed until caller acceptance', async () => {
  const p = page(); await p.connect(['pubmed.search']); await manualDeclaration(p);
  await p.$('mandatory-suggest').fire('click'); assert.equal(p.calls.length, 1);
  p.$('mandatory-suggest-grant').checked = true; p.replies.push(declarationReview()); await p.$('mandatory-suggest').fire('click');
  assert.equal(p.calls.at(-1).url, '/v1/executable-workflows/intent-reviews/suggest');
  assert.equal(p.calls.at(-1).body.approve_provider_call, true); assert.equal(p.calls.at(-1).body.obligations, undefined);
  assert.equal(p.$('mandatory-suggest-grant').checked, false); assert.equal(p.$('mandatory-accept-grant').checked, false);
  assert.equal(p.$('source-grant').disabled, true); assert.equal(p.$('provider-grant').checked, false);
  p.replies.push({ status: 'rejected', extraction: { raw_message: '<img src=x>' }, source_dispatch: false });
  p.$('mandatory-suggest-grant').checked = true; await p.$('mandatory-suggest').fire('click');
  assert.match(p.$('mandatory-findings').textContent, /<img/); assert.equal(p.$('mandatory-accept-grant').disabled, true);
});

test('durable review load preserves prior caller requirements and still needs explicit current-session acceptance', async () => {
  const p = page(); await p.connect(['pubmed.search']);
  p.$('mandatory-review-id').value = 'c'.repeat(32); p.replies.push(acceptedReview()); await p.$('mandatory-load').fire('click');
  assert.equal(p.$('mandatory-enabled').checked, true); assert.equal(p.$('planner').value, 'local-model');
  assert.equal(p.$('mandatory-accept-grant').checked, false);
  const count = p.calls.length; await p.$('plan-form').fire('submit'); assert.equal(p.calls.length, count);
  // A refused planning attempt invalidates the page's review, so explicitly load it again.
  p.replies.push(acceptedReview()); await p.$('mandatory-load').fire('click');
  p.$('mandatory-accept-grant').checked = true; await p.$('mandatory-accept-grant').fire('change');
  p.replies.push(acceptedReview()); await p.$('mandatory-accept').fire('click');
  p.replies.push({ ...planned(), planner: 'local-model', intent_review: {} }); await p.$('plan-form').fire('submit');
  assert.deepEqual(p.calls.at(-1).body.requirements, declarationReview().context.pre_obligation_requirements);
  assert.equal(p.calls.at(-1).body.planning_attempts, 1);
});

test('racing acceptance responses cannot survive changed controls or a different review identity', async () => {
  for (const variant of ['edited', 'wrong identity']) {
    let release; const pending = new Promise(resolve => { release = resolve; });
    const p = page(); await p.connect(['pubmed.search']); await manualDeclaration(p);
    p.replies.push(declarationReview()); await p.$('mandatory-prepare').fire('click');
    p.$('mandatory-accept-grant').checked = true; await p.$('mandatory-accept-grant').fire('change');
    p.replies.push(() => pending); const accepting = p.$('mandatory-accept').fire('click');
    if (variant === 'edited') { p.$('max-steps').value = '8'; await p.$('max-steps').fire('input'); }
    const result = acceptedReview(); if (variant === 'wrong identity') result.review_id = 'f'.repeat(32);
    release(result); await accepting;
    assert.equal(p.$('mandatory-accept-grant').disabled, true); assert.equal(p.$('source-grant').disabled, true);
    const count = p.calls.length; await p.$('plan-form').fire('submit'); assert.equal(p.calls.length, count);
  }
});

test('disconnect and reconnect prevent an old successful plan response from enabling source approval', async () => {
  let release; const pending = new Promise(resolve => { release = resolve; });
  const p = page(); await p.connect(['pubmed.search']); await manualDeclaration(p); await prepareAndAccept(p);
  p.replies.push(() => pending); p.$('provider-grant').checked = true;
  const building = p.$('plan-form').fire('submit'); await p.$('disconnect').fire('click'); await p.connect(['pubmed.search']);
  release({ ...planned(), planner: 'local-model', intent_review: {} }); await building;
  assert.equal(p.$('plan-workspace').hidden, true); assert.equal(p.$('source-grant').disabled, true);
  assert.equal(p.$('mandatory-accept-grant').disabled, true);
  assert.match(p.$('error').textContent, /controls changed/);
});

test('disconnect during durable run inspection cannot load an old plan into the disconnected editor', async () => {
  let release; const pending = new Promise(resolve => { release = resolve; });
  const p = page(); await p.connect(['pubmed.search']);
  p.$('run-id').value = 'f'.repeat(32); p.replies.push(() => pending);
  const loading = p.$('load-run').fire('click'); await p.$('disconnect').fire('click');
  release({ run_id: 'f'.repeat(32), plan_id: 'a'.repeat(32), status: 'completed' }); await loading;
  assert.equal(p.calls.length, 2); assert.equal(p.$('plan-workspace').hidden, true);
  assert.equal(p.$('source-grant').disabled, true); assert.equal(p.$('mandatory-export-run').disabled, true);
});

test('manual excerpt mismatches are refused locally and a new declaration set explicitly clears inherited requirements', async () => {
  const p = page(); await p.connect(['pubmed.search']); await manualDeclaration(p);
  declarationField(p, 'Exact task excerpt').value = 'Different wording'; await declarationField(p, 'Exact task excerpt').fire('input');
  await p.$('mandatory-prepare').fire('click'); assert.equal(p.calls.length, 1);
  assert.match(p.$('error').textContent, /exact current task text/);
  p.$('mandatory-review-id').value = 'c'.repeat(32); p.replies.push(declarationReview()); await p.$('mandatory-load').fire('click');
  await p.$('mandatory-new').fire('click'); assert.equal(p.$('mandatory-rows').children.length, 0);
  p.$('task').selectionStart = 0; p.$('task').selectionEnd = 7; await p.$('mandatory-add').fire('click');
  declarationField(p, 'Operation ID').value = 'pubmed.search'; await declarationField(p, 'Operation ID').fire('input');
  p.replies.push(declarationReview()); await p.$('mandatory-prepare').fire('click');
  assert.equal(p.calls.at(-1).body.requirements, undefined); assert.equal(p.calls.at(-1).body.planning_attempts, undefined);
});

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

test('a current literal policy conflict shows its exact report and clears prior plan and source authority', async () => {
  const report = { schema: 'aurora-workflow-literal-policy/0.1', status: 'current_policy_conflict',
    checks: [{ obligation_id: 'size', state: 'definite_scalar_conflict',
      diagnostics: [{ code: 'maximum', message: 'value violates maximum=100' }] }],
    whole_call_validity: 'not_assessed', provider_calls: 0, source_dispatches: 0 };
  const p = page([planned(), { status: 'current_policy_conflict', interpretation: report,
    intent_review: { status: 'accepted' }, source_dispatch: false }]);
  await p.connect(['pubmed.search']);
  p.$('planner').value = 'local-incremental'; p.$('provider-grant').checked = true;
  await p.$('plan-form').fire('submit');
  p.$('source-grant').checked = true; await p.$('source-grant').fire('change');
  assert.equal(p.$('run').disabled, false);
  p.$('provider-grant').checked = true;
  await p.$('plan-form').fire('submit');
  assert.equal(p.$('plan-workspace').hidden, true);
  assert.equal(p.$('source-grant').checked, false);
  assert.equal(p.$('source-grant').disabled, true);
  assert.equal(p.$('run').disabled, true);
  assert.match(p.$('interpretation').textContent, /definite_scalar_conflict/);
  assert.match(p.$('interpretation').textContent, /maximum=100/);
  assert.match(p.$('error').textContent, /current_policy_conflict.*No executable plan was saved/);
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
