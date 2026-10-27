import { parse as parseJson, stringify } from '/workflows/json.js';
/* The fixed Python worker is authoritative. Browser state never grants dispatch by itself. */
(() => {
  'use strict';
  const $ = id => document.getElementById(id);
  const state = { token: '', catalogue: [], plan: null, proposal: null, dirty: true, editRevision: 0, pendingRun: false, planningSettingsDirty: false, run: null, timer: null };
  const endpoint = '/v1/executable-workflows';
  const templateOperations = {
    'glioma-mutations': ['gdc.gene_mutation_counts', 'glioma.mutation.analyze', 'glioma.mutation.review_handoff'],
    'gene-references': ['ensembl.gene_references', 'ncbi_gene.summary', 'glioma.gene_references.analyze', 'glioma.gene_references.review_handoff'],
    'protein-pathways': ['uniprot.protein_annotation', 'reactome.pathway_annotations', 'glioma.protein_pathways.analyze', 'glioma.protein_pathways.review_handoff']
  };
  function error(message) { $('error').textContent = String(message); $('error').hidden = false; }
  function clearError() { $('error').hidden = true; $('error').textContent = ''; $('diagnostics-panel').hidden = true; }
  function diagnostics(value) { if (value) { $('diagnostics-panel').hidden = false; $('diagnostics-panel').open = true; $('diagnostics').textContent = stringify(value); } }
  async function request(path, body) {
    if (!state.token) throw new Error('Connect this page with the operator-provided token.');
    const response = await fetch(endpoint + path, { method: body === undefined ? 'GET' : 'POST', headers: { Authorization: 'Bearer ' + state.token, ...(body === undefined ? {} : { 'Content-Type': 'application/json' }) }, ...(body === undefined ? {} : { body: stringify(body) }), cache: 'no-store', credentials: 'omit', redirect: 'error' });
    const envelope = parseJson(await response.text());
    if (!response.ok || envelope.ok !== true) { const failure = new Error(envelope.error?.message || 'The API refused this request.'); failure.diagnostics = envelope.error?.diagnostics; throw failure; }
    return envelope.data;
  }
  function action(fn) { return async event => { event?.preventDefault(); clearError(); try { await fn(); } catch (failure) { error(failure.message || failure); diagnostics(failure.diagnostics); } }; }
  function dirty() { state.editRevision += 1; state.dirty = true; $('source-grant').checked = false; $('source-grant').disabled = true; $('run').disabled = true; $('validation-status').textContent = 'Unsaved edits — validate before approval.'; }
  function element(tag, text, className) { const node = document.createElement(tag); if (text !== undefined) node.textContent = text; if (className) node.className = className; return node; }
  function updatePlannerAvailability() {
    const installed = new Set(state.token ? state.catalogue.map(operation => operation.operation_id) : []);
    for (const [planner, required] of Object.entries(templateOperations)) $('planner').querySelector('option[value="' + planner + '"]').disabled = !required.every(operation => installed.has(operation));
    if ($('planner').selectedOptions[0]?.disabled) {
      $('planner').value = 'glioma-intent';
      if (state.plan) { state.planningSettingsDirty = true; dirty(); }
    }
    updatePlanningControls();
  }
  function updatePlanningControls() {
    const incremental = $('planner').value === 'local-incremental';
    $('incremental-controls').hidden = !incremental;
    for (const id of ['max-model-calls', 'decision-attempts', 'operation-ids']) $(id).disabled = !incremental;
    $('provider-label').textContent = incremental ? 'Approve these bounded local model planning calls' : 'Approve this local model planning call';
  }
  function planningSnapshot() {
    return ['task', 'planner', 'max-steps', 'max-model-calls', 'decision-attempts', 'operation-ids'].map(id => $(id).value).join('\u0000');
  }
  function planRequest() {
    const body = { task: $('task').value, planner: $('planner').value, max_steps: Number($('max-steps').value), approve_provider_call: $('provider-grant').checked };
    if (body.planner === 'local-incremental') {
      body.max_model_calls = Number($('max-model-calls').value);
      body.decision_attempts = Number($('decision-attempts').value);
      if ($('operation-ids').value.trim()) body.operation_ids = parseJson($('operation-ids').value);
    }
    return body;
  }
  function clearPlan() {
    state.plan = null; state.proposal = null; state.dirty = true; state.planningSettingsDirty = false;
    $('plan-workspace').hidden = true; $('source-grant').checked = false; $('source-grant').disabled = true; $('run').disabled = true;
    $('approval-identity').textContent = 'Build and validate a plan to enable execution.';
  }
  function renderCatalogue() {
    const filter = $('catalog-filter').value.toLowerCase(); const list = $('catalog-list'); list.replaceChildren();
    for (const operation of state.catalogue.filter(item => (item.operation_id + ' ' + item.description).toLowerCase().includes(filter))) {
      const card = element('article', undefined, 'operation'); card.append(element('h3', operation.operation_id), element('p', operation.description));
      card.append(element('span', operation.integration_id, 'pill'), element('span', operation.implementation, 'pill'));
      const details = element('details'); details.append(element('summary', 'Input schema'), element('pre', stringify(operation.input_schema))); card.append(details); list.append(card);
    }
  }
  function proposalFromPlan(plan) { return { name: plan.name, steps: parseJson(stringify(plan.steps)), outputs: parseJson(stringify(plan.outputs)) }; }
  function updateProposalJson() { $('proposal').value = stringify(state.proposal); }
  function editorInput(label, value, onInput, multiline = false) {
    const wrapper = element('div'); const id = 'field-' + crypto.randomUUID(); const input = element(multiline ? 'textarea' : 'input'); input.id = id; input.value = value; input.spellcheck = false; if (multiline) input.rows = 5;
    const lab = element('label', label); lab.htmlFor = id; input.addEventListener('input', () => { dirty(); try { onInput(input.value); input.setCustomValidity(''); } catch (failure) { input.setCustomValidity('Enter valid JSON of the required type.'); $('validation-status').textContent = failure.message || 'Enter valid JSON of the required type.'; } }); wrapper.append(lab, input); return wrapper;
  }
  function renderSteps() {
    const list = $('steps'); list.replaceChildren();
    state.proposal.steps.forEach((step, index) => {
      const card = element('article', undefined, 'step'); const heading = element('div', undefined, 'step-heading'); const remove = element('button', 'Remove', 'quiet'); remove.type = 'button'; remove.addEventListener('click', () => { state.proposal.steps.splice(index, 1); dirty(); renderSteps(); updateProposalJson(); }); heading.append(element('h3', 'STEP ' + String(index + 1).padStart(2, '0')), remove); card.append(heading);
      card.append(editorInput('Step ID', step.id, value => { step.id = value; updateProposalJson(); }));
      card.append(editorInput('Operation ID', step.operation, value => { step.operation = value; updateProposalJson(); }));
      card.append(editorInput('Dependencies (JSON array)', stringify(step.depends_on), value => { const parsed = parseJson(value); if (!Array.isArray(parsed)) throw new Error('Dependencies require a JSON array.'); step.depends_on = parsed; updateProposalJson(); }));
      card.append(editorInput('Arguments (JSON; references use {"$ref":"step#/pointer"})', stringify(step.arguments), value => { step.arguments = parseJson(value); updateProposalJson(); }, true));
      list.append(card);
    });
  }
  function acceptPlan(data) {
    if (data.status !== 'planned') { clearPlan(); showInterpretation(data.interpretation); error('Planning stopped: ' + String(data.status) + '. No executable plan was saved. Review the planning result before another attempt.'); return; }
    state.planningSettingsDirty = false; state.plan = data; state.proposal = proposalFromPlan(data.plan); state.dirty = false; $('plan-workspace').hidden = false;
    $('plan-identity').textContent = 'Revision ' + data.plan_id + '\nDigest ' + data.plan_digest;
    $('retained-task').textContent = data.task; $('contract').textContent = stringify(data.plan.requirements ?? { state: 'No additional fixed contract was supplied.' });
    $('plan-name').value = state.proposal.name; $('outputs').value = stringify(state.proposal.outputs); $('outputs').setCustomValidity(''); updateProposalJson(); renderSteps();
    $('validation-status').textContent = 'Saved and validated by the Python engine.'; $('approval-identity').textContent = 'Approve exact digest: ' + data.plan_digest;
    $('source-grant').checked = false; $('source-grant').disabled = false; $('run').disabled = true;
    if (data.interpretation) showInterpretation(data.interpretation); diagnostics(data.validation);
    if (data.planner === 'local-incremental' && data.interpretation?.limits) {
      $('max-model-calls').value = String(data.interpretation.limits.max_model_calls);
      $('decision-attempts').value = String(data.interpretation.limits.max_decision_attempts);
      $('operation-ids').value = stringify(data.interpretation.visible_operations);
    }
    updatePlanningControls();
  }
  function showInterpretation(data) { $('interpretation-panel').hidden = false; $('interpretation-panel').open = true; $('interpretation').textContent = stringify(data); }
  async function pollRun() {
    if (!state.run) return;
    const selectedRun = state.run.run_id; const data = await request('/runs/' + selectedRun); if (state.run?.run_id !== selectedRun) return; state.run = data;
    $('run-status').textContent = data.status + (data.active ? ' · worker active' : '') + (data.worker_process?.response_ok === false ? ' · admission refused' : '');
    $('review-status').textContent = 'Scientific review: ' + (Object.keys(data.output_review_statuses || {}).length ? stringify(data.output_review_statuses) : 'not assessed');
    $('progress').textContent = stringify(data);
    for (const id of ['cancel', 'refresh', 'resume', 'export-outputs', 'export-checkpoint']) $(id).disabled = false;
    $('resume').disabled = data.active === true;
    clearTimeout(state.timer); const processStopped = ['finished', 'interrupted', 'not_started'].includes(data.worker_process?.state); if (data.active || (!processStopped && (data.worker_process?.state === 'running' || ['queued', 'running', 'cancellation_requested', 'resume_requested'].includes(data.status)))) state.timer = setTimeout(() => action(pollRun)(), 1800);
  }
  async function exportRun(kind) {
    const id = state.run.run_id;
    if (!state.token) throw new Error('Connect this page before exporting.');
    const response = await fetch(endpoint + '/runs/' + id + '/' + kind, { headers: { Authorization: 'Bearer ' + state.token }, cache: 'no-store', credentials: 'omit', redirect: 'error' });
    const text = await response.text();
    if (!response.ok) { let message = 'The export was refused.'; try { message = parseJson(text).error?.message || message; } catch {} throw new Error(message); }
    const blob = new Blob([text], { type: 'application/json' }); const url = URL.createObjectURL(blob); const link = element('a'); link.href = url; link.download = 'aurora-' + id + '-' + kind + '.json'; link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  $('connect-form').addEventListener('submit', action(async () => { state.token = $('token').value; $('token').value = ''; $('connection-status').textContent = 'Connecting…'; try { const catalogue = await request('/catalogue'); state.catalogue = catalogue.operations; updatePlannerAvailability(); $('catalog-count').textContent = catalogue.operation_count + ' operations · ' + catalogue.integration_count + ' integration IDs'; $('connection-status').textContent = 'Connected to this local API'; renderCatalogue(); } catch (failure) { state.token = ''; state.catalogue = []; updatePlannerAvailability(); $('connection-status').textContent = 'Disconnected'; throw failure; } }));
  $('disconnect').addEventListener('click', () => { state.token = ''; state.catalogue = []; updatePlannerAvailability(); clearTimeout(state.timer); $('token').value = ''; $('connection-status').textContent = 'Disconnected'; $('source-grant').checked = false; $('run').disabled = true; });
  $('catalog-filter').addEventListener('input', renderCatalogue);
  $('plan-form').addEventListener('submit', action(async () => { $('build').disabled = true; try { const snapshot = planningSnapshot(); const body = planRequest(); clearPlan(); const data = await request('/plans', body); if (planningSnapshot() !== snapshot) throw new Error('The task or planning controls changed while planning. Build a new plan for the current settings.'); acceptPlan(data); } finally { $('build').disabled = false; $('provider-grant').checked = false; } }));
  $('interpret').addEventListener('click', action(async () => showInterpretation(await request('/interpret', { task: $('task').value }))));
  for (const id of ['planner', 'max-steps', 'max-model-calls', 'decision-attempts', 'operation-ids']) $(id).addEventListener('change', () => { updatePlanningControls(); if (state.plan) { state.planningSettingsDirty = true; dirty(); } });
  for (const id of ['max-model-calls', 'decision-attempts', 'operation-ids']) $(id).addEventListener('input', () => { if (state.plan) { state.planningSettingsDirty = true; dirty(); } });
  $('task').addEventListener('input', () => { if (state.plan) { dirty(); $('validation-status').textContent = 'Task changed — build a new plan to retain a new task contract.'; } });
  $('plan-name').addEventListener('input', () => { state.proposal.name = $('plan-name').value; dirty(); updateProposalJson(); });
  $('outputs').addEventListener('input', () => { dirty(); try { const outputs = parseJson($('outputs').value); if (!Array.isArray(outputs)) throw new Error(); state.proposal.outputs = outputs; $('outputs').setCustomValidity(''); updateProposalJson(); } catch { $('outputs').setCustomValidity('Outputs require a JSON array.'); $('validation-status').textContent = 'Outputs require a JSON array.'; } });
  $('proposal').addEventListener('input', dirty);
  $('apply-json').addEventListener('click', action(async () => { const proposal = parseJson($('proposal').value); if (!proposal || !Array.isArray(proposal.steps) || !Array.isArray(proposal.outputs) || typeof proposal.name !== 'string') throw new Error('A proposal requires name, steps, and outputs.'); state.proposal = proposal; $('plan-name').value = proposal.name; $('outputs').value = stringify(proposal.outputs); $('outputs').setCustomValidity(''); dirty(); renderSteps(); }));
  $('add-step').addEventListener('click', () => { state.proposal.steps.push({ id: 'step_' + (state.proposal.steps.length + 1), operation: '', arguments: {}, depends_on: [] }); dirty(); renderSteps(); updateProposalJson(); });
  $('validate').addEventListener('click', action(async () => { if (state.planningSettingsDirty) throw new Error('Build a new plan for the changed planning settings.'); if ($('task').value !== state.plan.task) throw new Error('Build a new plan for the changed task before editing its steps.'); for (const input of [...$('steps').querySelectorAll('input,textarea'), $('outputs')]) { if (!input.checkValidity()) { input.reportValidity(); throw new Error('Correct the invalid step fields before saving.'); } } const proposal = parseJson($('proposal').value); const revision = state.editRevision; $('validate').disabled = true; try { const saved = await request('/plans/validate', { plan_id: state.plan.plan_id, expected_plan_digest: state.plan.plan_digest, proposal }); if (state.editRevision !== revision) { state.plan = saved; $('plan-identity').textContent = 'Revision ' + saved.plan_id + '\nDigest ' + saved.plan_digest; dirty(); throw new Error('An earlier snapshot was saved while you edited. Validate the current edits before approval.'); } acceptPlan(saved); } finally { $('validate').disabled = false; } }));
  $('source-grant').addEventListener('change', () => { $('run').disabled = state.pendingRun || !state.token || state.dirty || !$('source-grant').checked; });
  $('run').addEventListener('click', action(async () => { if (state.dirty || !$('source-grant').checked) throw new Error('Validate edits and explicitly approve the exact saved plan first.'); if (state.pendingRun) return; state.pendingRun = true; $('run').disabled = true; $('source-grant').disabled = true; let data; try { data = await request('/runs', { plan_id: state.plan.plan_id, expected_plan_digest: state.plan.plan_digest, approve_source_dispatch: true, max_step_calls: Number($('call-budget').value) }); } finally { state.pendingRun = false; $('source-grant').checked = false; $('source-grant').disabled = state.dirty; } state.run = data; $('run-id').value = data.run_id; $('source-grant').checked = false; $('run').disabled = true; await pollRun(); }));
  $('load-run').addEventListener('click', action(async () => { const id = $('run-id').value; if (!/^[a-f0-9]{32}$/.test(id)) throw new Error('Enter a 32-character lowercase hexadecimal run ID.'); state.run = { run_id: id }; await pollRun(); if (state.run.plan_id) { const saved = await request('/plans/' + state.run.plan_id); if (state.run?.run_id !== id) return; $('task').value = saved.task; $('planner').value = saved.planner; acceptPlan(saved); } }));
  $('refresh').addEventListener('click', action(pollRun));
  $('cancel').addEventListener('click', action(async () => { await request('/runs/' + state.run.run_id + '/cancel', {}); await pollRun(); }));
  $('resume').addEventListener('click', action(async () => { if (!$('source-grant').checked || state.dirty || !state.plan || state.plan.plan_digest !== state.run.plan_digest) throw new Error('Load the unchanged saved plan and approve its exact digest before resuming.'); await request('/runs/' + state.run.run_id + '/resume', { expected_plan_digest: state.run.plan_digest, approve_source_dispatch: true, retry_failed: $('retry-failed').checked, resume_cancelled: $('release-cancel').checked, max_step_calls: Number($('call-budget').value) }); $('source-grant').checked = false; $('retry-failed').checked = false; $('release-cancel').checked = false; $('run').disabled = true; await pollRun(); }));
  $('export-outputs').addEventListener('click', action(() => exportRun('outputs'))); $('export-checkpoint').addEventListener('click', action(() => exportRun('checkpoint')));
  updatePlanningControls();
})();
