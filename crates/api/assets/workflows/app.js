import { parse as parseJson, stringify } from '/workflows/json.js';
/* The fixed Python worker is authoritative. Browser state never grants dispatch by itself. */
(() => {
  'use strict';
  const $ = id => document.getElementById(id);
  const state = { token: '', connectionRevision: 0, catalogue: [], capabilities: [], plan: null, proposal: null, dirty: true, editRevision: 0, pendingRun: false, planningSettingsDirty: false, run: null, timer: null,
    intentRows: [], intentRevision: 0, intentContext: null, review: null, acceptedReview: null, reviewRequest: 0, planRequest: 0 };
  const endpoint = '/v1/executable-workflows';
  const templateOperations = {
    'glioma-mutations': ['gdc.gene_mutation_counts', 'glioma.mutation.analyze', 'glioma.mutation.review_handoff'],
    'gene-references': ['ensembl.gene_references', 'ncbi_gene.summary', 'glioma.gene_references.analyze', 'glioma.gene_references.review_handoff'],
    'protein-pathways': ['uniprot.protein_annotation', 'reactome.pathway_annotations', 'glioma.protein_pathways.analyze', 'glioma.protein_pathways.review_handoff'],
    'denominator-coverage': ['gdc.gene_mutation_counts', 'gdc.ssm_project_coverage', 'glioma.denominator.assess', 'glioma.denominator.review_handoff']
  };
  function error(message) { $('error').textContent = String(message); $('error').hidden = false; }
  function clearError() { $('error').hidden = true; $('error').textContent = ''; $('diagnostics-panel').hidden = true; }
  function diagnostics(value, expand = false) { if (value) { $('diagnostics-panel').hidden = false; $('diagnostics-panel').open = expand; $('diagnostics').textContent = stringify(value); } }
  async function request(path, body) {
    if (!state.token) throw new Error('Connect this page with the operator-provided token.');
    const response = await fetch(endpoint + path, { method: body === undefined ? 'GET' : 'POST', headers: { Authorization: 'Bearer ' + state.token, ...(body === undefined ? {} : { 'Content-Type': 'application/json' }) }, ...(body === undefined ? {} : { body: stringify(body) }), cache: 'no-store', credentials: 'omit', redirect: 'error' });
    const envelope = parseJson(await response.text());
    if (!response.ok || envelope.ok !== true) { const failure = new Error(envelope.error?.message || 'The API refused this request.'); failure.diagnostics = envelope.error?.diagnostics; throw failure; }
    return envelope.data;
  }
  function action(fn) { return async event => { event?.preventDefault(); clearError(); try { await fn(); } catch (failure) { invalidateIntent('The request failed. Prepare or load a review again.'); $('source-grant').checked = false; $('run').disabled = true; error(failure.message || failure); diagnostics(failure.diagnostics, true); } }; }
  function dirty() { state.editRevision += 1; state.dirty = true; $('source-grant').checked = false; $('source-grant').disabled = true; $('run').disabled = true; $('validation-status').textContent = 'Unsaved edits — validate before approval.'; }
  function element(tag, text, className) { const node = document.createElement(tag); if (text !== undefined) node.textContent = text; if (className) node.className = className; return node; }
  function updatePlannerAvailability() {
    const installed = new Set(state.token ? state.catalogue.map(operation => operation.operation_id) : []);
    for (const [planner, required] of Object.entries(templateOperations)) $('planner').querySelector('option[value="' + planner + '"]').disabled = !required.every(operation => installed.has(operation));
    const selected = $('planner').value;
    const group = $('capability-templates'); group.replaceChildren();
    const available = new Set();
    for (const profile of state.capabilities.slice(0, 32)) {
      if (!profile || typeof profile.profile_id !== 'string' || !/^[a-z][a-z0-9-]{0,63}$/.test(profile.profile_id) || typeof profile.label !== 'string' || !Array.isArray(profile.required_operations)) continue;
      const option = element('option', profile.label.slice(0, 200)); option.value = 'capability:' + profile.profile_id;
      option.disabled = !state.token || profile.operator_enabled !== true || profile.descriptor_available !== true || !profile.required_operations.length || !profile.required_operations.every(id => typeof id === 'string' && installed.has(id));
      if (!option.disabled) available.add(option.value);
      group.append(option);
    }
    if (selected.startsWith('capability:') && !available.has(selected)) {
      $('planner').value = 'glioma-intent';
      if (state.plan) { state.planningSettingsDirty = true; dirty(); }
    } else { $('planner').value = selected; }
    if ($('planner').selectedOptions[0]?.disabled) {
      $('planner').value = 'glioma-intent';
      if (state.plan) { state.planningSettingsDirty = true; dirty(); }
    }
    updatePlanningControls();
  }
  function updatePlanningControls() {
    const planner = $('planner').value;
    const incremental = planner === 'local-incremental';
    const modeled = incremental || planner === 'local-model';
    $('mandatory-enabled').disabled = !modeled;
    if (!modeled && $('mandatory-enabled').checked) { $('mandatory-enabled').checked = false; invalidateIntent('Select a local model planner to review mandatory work.'); }
    $('mandatory-controls').hidden = !modeled || !$('mandatory-enabled').checked;
    $('mandatory-availability').textContent = modeled ? 'Optional: prepared declarations remain unreviewed until you explicitly accept them.' : 'Available for the two local model planners.';
    $('incremental-controls').hidden = !incremental;
    for (const id of ['max-model-calls', 'decision-attempts', 'operation-ids']) $(id).disabled = !incremental;
    $('provider-approval').hidden = !modeled; $('provider-grant').disabled = !modeled;
    if (!modeled) $('provider-grant').checked = false;
    $('provider-label').textContent = incremental ? 'Approve these bounded local model planning calls' : 'Approve this local model planning call';
    const fixedScopes = {
      'glioma-mutations': 'Fixed TP53 mutation counts for TCGA-GBM/LGG; no prevalence or assay coverage.',
      'gene-references': 'Fixed Ensembl/NCBI TP53 references; source agreement is not scientific identity.',
      'protein-pathways': 'Fixed TP53 protein/pathway annotations; no glioma activity or enrichment.',
      'denominator-coverage': 'Fixed TP53 and GDC SSM-data coverage for TCGA-GBM/LGG; no prevalence.'
    };
    let scope;
    if (planner.startsWith('capability:')) {
      const profile = state.capabilities.find(item => item?.profile_id === planner.slice('capability:'.length));
      scope = typeof profile?.scope === 'string' ? profile.scope.slice(0, 400) : 'Fixed reviewed source metadata; scientific validity is unassessed.';
    } else { scope = fixedScopes[planner]; }
    $('fixed-template-scope').textContent = scope ? scope + ' Task text does not change source queries.'
      : modeled ? 'Use the local model to propose a plan from installed operations. Model planning and source execution need separate approvals.'
      : 'Bounded glioma interpretation plans supported sources or asks for clarification. Review the plan before approving source dispatch.';
  }
  function planningSnapshot() {
    return stringify({ connection: state.connectionRevision, controls: ['task', 'planner', 'max-steps', 'max-model-calls', 'decision-attempts', 'operation-ids'].map(id => $(id).value),
      mandatory: $('mandatory-enabled').checked, intentRevision: state.intentRevision, retainedContext: state.intentContext });
  }
  function planRequest() {
    const body = { task: $('task').value, planner: $('planner').value, max_steps: Number($('max-steps').value), approve_provider_call: $('provider-grant').checked };
    if (body.planner === 'local-incremental') {
      body.max_model_calls = Number($('max-model-calls').value);
      body.decision_attempts = Number($('decision-attempts').value);
      if ($('operation-ids').value.trim()) body.operation_ids = parseJson($('operation-ids').value);
    }
    if ($('mandatory-enabled').checked) {
      if (!state.acceptedReview || state.acceptedReview.snapshot !== planningSnapshot()) throw new Error('Prepare and explicitly accept the current mandatory declarations before planning.');
      body.intent_review_id = state.acceptedReview.review_id;
      body.expected_acceptance_digest = state.acceptedReview.acceptance_digest;
      Object.assign(body, retainedReviewControls());
    }
    return body;
  }
  function invalidateIntent(message = 'Declarations changed. Prepare a new review.') {
    state.intentRevision += 1; state.reviewRequest += 1; state.review = null; state.acceptedReview = null;
    $('mandatory-accept-grant').checked = false; $('mandatory-accept-grant').disabled = true; $('mandatory-accept').disabled = true;
    $('mandatory-suggest-grant').checked = false; $('mandatory-findings').textContent = ''; $('mandatory-status').textContent = message;
    $('provider-grant').checked = false;
  }
  function intentChanged() {
    invalidateIntent();
    if (state.plan) { state.planningSettingsDirty = true; dirty(); }
  }
  function selectedTaskSpan() {
    const task = $('task').value; const start = $('task').selectionStart; const end = $('task').selectionEnd;
    if (!Number.isInteger(start) || !Number.isInteger(end) || start >= end) throw new Error('Select the exact mandatory excerpt in the research task first.');
    // A browser selection uses UTF-16 units; Python spans count Unicode code points.
    const splitPair = offset => offset > 0 && offset < task.length && /[\uD800-\uDBFF]/.test(task[offset - 1]) && /[\uDC00-\uDFFF]/.test(task[offset]);
    if (splitPair(start) || splitPair(end)) throw new Error('Select complete Unicode characters in the task.');
    return { start: String([...task.slice(0, start)].length), text: task.slice(start, end) };
  }
  function intentRows() {
    if (!state.intentRows.length || state.intentRows.length > 128) throw new Error('Declare between one and 128 mandatory items.');
    const task = [...$('task').value];
    return state.intentRows.map(row => {
      if (!/^(0|[1-9]\d*)$/.test(row.start)) throw new Error('Task span starts require nonnegative Unicode code point offsets.');
      const start = Number(row.start); const end = start + [...row.text].length;
      if (!Number.isSafeInteger(start) || !row.text.trim() || end > task.length || task.slice(start, end).join('') !== row.text) throw new Error('Every declaration must select exact current task text. Select it again after task edits.');
      const result = { id: row.id, kind: row.kind, span: { start, end, text: row.text } };
      if (row.kind === 'required_operation') Object.assign(result, { operation: row.operation, output: row.output });
      else if (row.kind === 'argument_equals') Object.assign(result, { operation: row.operation, path: row.path, value: parseJson(row.literal) });
      else if (row.kind === 'unresolved_action') result.reason = row.reason;
      else throw new Error('Unsupported declaration kind.');
      return result;
    });
  }
  function declarationField(card, label, value, apply, options) {
    const wrapper = element('div'); const multiline = ['Exact task excerpt', 'Literal JSON value', 'Unresolved reason'].includes(label);
    const input = element(options ? 'select' : multiline ? 'textarea' : 'input'); input.value = value; if (multiline) input.rows = 2;
    input.id = 'declaration-' + crypto.randomUUID(); input.name = label; input.spellcheck = false;
    if (options) { for (const [key, text] of options) { const option = element('option', text); option.value = key; input.append(option); } input.value = value; }
    const caption = element('label', label); caption.htmlFor = input.id;
    input.addEventListener(options ? 'change' : 'input', () => { apply(input.value); intentChanged(); if (options) renderIntentRows(); });
    wrapper.append(caption, input); card.append(wrapper); return input;
  }
  function renderIntentRows() {
    const list = $('mandatory-rows'); list.replaceChildren();
    state.intentRows.forEach((row, index) => {
      const card = element('article', undefined, 'step'); const heading = element('div', undefined, 'step-heading');
      const remove = element('button', 'Remove', 'quiet'); remove.type = 'button'; remove.name = 'Remove declaration';
      remove.addEventListener('click', () => { state.intentRows.splice(index, 1); intentChanged(); renderIntentRows(); });
      heading.append(element('h3', 'MANDATORY ITEM ' + (index + 1)), remove); card.append(heading);
      declarationField(card, 'Kind', row.kind, value => { row.kind = value; }, [['required_operation', 'Required operation'], ['argument_equals', 'Literal argument equals'], ['unresolved_action', 'Unresolved action (blocks planning)']]);
      const selection = element('button', 'Use selected task text', 'quiet'); selection.type = 'button'; selection.name = 'Use task selection';
      selection.addEventListener('click', action(async () => { Object.assign(row, selectedTaskSpan()); intentChanged(); renderIntentRows(); })); card.append(selection);
      declarationField(card, 'Exact task excerpt', row.text, value => { row.text = value; });
      declarationField(card, 'Start (Unicode code points)', row.start, value => { row.start = value; });
      if (row.kind !== 'unresolved_action') declarationField(card, 'Operation ID', row.operation, value => { row.operation = value; });
      if (row.kind === 'required_operation') {
        const label = element('label', undefined, 'check'); const output = element('input'); output.type = 'checkbox'; output.name = 'Required output'; output.checked = row.output;
        output.addEventListener('change', () => { row.output = output.checked; intentChanged(); }); label.append(output, element('span', 'Require an output from this operation')); card.append(label);
      } else if (row.kind === 'argument_equals') {
        declarationField(card, 'Argument JSON pointer', row.path, value => { row.path = value; });
        declarationField(card, 'Literal JSON value', row.literal, value => { row.literal = value; });
      } else declarationField(card, 'Unresolved reason', row.reason, value => { row.reason = value; });
      list.append(card);
    });
  }
  function reviewContext() {
    if (!['local-model', 'local-incremental'].includes($('planner').value) || !$('mandatory-enabled').checked) throw new Error('Enable mandatory declarations for a local model planner first.');
    const body = { task: $('task').value, planner: $('planner').value, max_steps: Number($('max-steps').value) };
    if (body.planner === 'local-incremental') {
      body.max_model_calls = Number($('max-model-calls').value); body.decision_attempts = Number($('decision-attempts').value);
      if ($('operation-ids').value.trim()) body.operation_ids = parseJson($('operation-ids').value);
    }
    return Object.assign(body, retainedReviewControls());
  }
  function retainedReviewControls() {
    if (!state.intentContext) return {};
    const context = { requirements: state.intentContext.pre_obligation_requirements, planning_attempts: state.intentContext.planning_attempts };
    if ($('planner').value === 'local-model' && state.intentContext.operation_ids !== null) context.operation_ids = state.intentContext.operation_ids;
    return context;
  }
  function showReview(data) {
    if (!/^[a-f0-9]{32}$/.test(data.review_id || '') || !/^[a-f0-9]{64}$/.test(data.review_digest || '') || data.task !== $('task').value || data.planner !== $('planner').value || !Array.isArray(data.candidate?.obligations)) throw new Error('The review does not match this task and planner.');
    state.review = { data, snapshot: planningSnapshot() }; state.acceptedReview = null;
    $('mandatory-review-id').value = data.review_id; $('mandatory-findings').textContent = stringify(data);
    $('mandatory-status').textContent = 'Review retained. Inspect every declaration and finding before accepting.';
    $('mandatory-accept-grant').checked = false; $('mandatory-accept-grant').disabled = false; $('mandatory-accept').disabled = true;
  }
  function rowsFromCandidate(candidate) {
    return candidate.obligations.map(row => ({ id: row.id, kind: row.kind, start: String(row.span.start), text: row.span.text,
      operation: row.operation ?? '', output: row.output === true, path: row.path ?? '', literal: 'value' in row ? stringify(row.value) : 'null', reason: row.reason ?? '' }));
  }
  async function prepareReview(suggest = false) {
    const body = reviewContext();
    if (suggest) {
      if (!$('mandatory-suggest-grant').checked) throw new Error('Explicitly approve one local model suggestion call first.');
      body.approve_provider_call = true;
    } else body.obligations = intentRows();
    invalidateIntent('Preparing an unreviewed declaration candidate…'); clearPlan();
    const snapshot = planningSnapshot(); const ticket = ++state.reviewRequest;
    try {
      const data = await request('/intent-reviews/' + (suggest ? 'suggest' : 'prepare'), body);
      if (ticket !== state.reviewRequest || snapshot !== planningSnapshot()) throw new Error('The task, declarations or controls changed while preparing this review. Prepare again.');
      if (!data.review_id) {
        $('mandatory-findings').textContent = stringify(data); $('mandatory-status').textContent = 'Suggestions stopped: ' + String(data.status) + '. No review or plan was accepted.'; return;
      }
      if (suggest) { state.intentRows = rowsFromCandidate(data.candidate); renderIntentRows(); }
      showReview(data);
    } finally { $('mandatory-suggest-grant').checked = false; }
  }
  async function loadReview() {
    const id = $('mandatory-review-id').value;
    if (!/^[a-f0-9]{32}$/.test(id)) throw new Error('Enter a 32-character lowercase hexadecimal review ID.');
    invalidateIntent('Loading retained declarations…'); clearPlan();
    const snapshot = planningSnapshot(); const ticket = ++state.reviewRequest;
    const data = await request('/intent-reviews/' + id);
    if (ticket !== state.reviewRequest || snapshot !== planningSnapshot()) throw new Error('The task or controls changed while loading. Load the review again.');
    if (!['local-model', 'local-incremental'].includes(data.planner) || !data.context || !Array.isArray(data.candidate?.obligations)) throw new Error('The retained review has an unsupported planning context.');
    $('task').value = data.task; $('planner').value = data.planner; $('max-steps').value = String(data.context.max_steps);
    if (data.planner === 'local-incremental') {
      $('max-model-calls').value = String(data.context.max_model_calls); $('decision-attempts').value = String(data.context.decision_attempts);
      $('operation-ids').value = data.context.operation_ids === null ? '' : stringify(data.context.operation_ids);
    }
    state.intentContext = data.context; state.intentRows = rowsFromCandidate(data.candidate); $('mandatory-enabled').checked = true;
    updatePlanningControls(); renderIntentRows(); showReview(data);
  }
  async function acceptReview() {
    const review = state.review;
    if (!$('mandatory-accept-grant').checked || !review || review.snapshot !== planningSnapshot()) throw new Error('Review the unchanged current declarations and explicitly accept them first.');
    $('mandatory-accept-grant').checked = false; $('mandatory-accept').disabled = true;
    const snapshot = planningSnapshot(); const ticket = ++state.reviewRequest;
    const data = await request('/intent-reviews/accept', { review_id: review.data.review_id, expected_review_digest: review.data.review_digest, accept_caller_declarations: true });
    if (ticket !== state.reviewRequest || snapshot !== planningSnapshot()) throw new Error('The task, declarations or controls changed while accepting. Prepare a new review.');
    if (data.review_id !== review.data.review_id || data.review_digest !== review.data.review_digest) throw new Error('The acceptance response does not identify the exact reviewed declarations.');
    showReview(data);
    const acceptance = data.acceptance;
    if (data.status !== 'accepted' || acceptance?.admission?.status !== 'ready_for_constrained_authoring') {
      $('mandatory-status').textContent = 'Declarations retained; admission still blocks planning. Inspect the findings.'; return;
    }
    if (!/^[a-f0-9]{64}$/.test(acceptance.acceptance_digest || '')) throw new Error('The accepted review omitted its exact acceptance digest.');
    state.acceptedReview = { review_id: data.review_id, acceptance_digest: acceptance.acceptance_digest, snapshot: planningSnapshot() };
    $('mandatory-status').textContent = 'Caller declarations accepted for these settings. Model planning and source dispatch still need separate approvals.';
    $('mandatory-accept-grant').disabled = true;
  }
  function clearPlan() {
    state.plan = null; state.proposal = null; state.dirty = true; state.planningSettingsDirty = false;
    $('interpretation-panel').hidden = true; $('interpretation-panel').open = false; $('interpretation').textContent = '';
    $('plan-workspace').hidden = true; $('source-grant').checked = false; $('source-grant').disabled = true; $('run').disabled = true;
    $('approval-identity').textContent = 'Build and validate a plan to enable execution.';
    $('mandatory-export-plan').disabled = true;
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
    $('mandatory-export-plan').disabled = !data.intent_review;
    if (data.interpretation) showInterpretation(data.interpretation); diagnostics(data.validation);
    if (data.planner === 'local-incremental' && data.interpretation?.limits && !data.intent_review) {
      $('max-model-calls').value = String(data.interpretation.limits.max_model_calls);
      $('decision-attempts').value = String(data.interpretation.limits.max_decision_attempts);
      $('operation-ids').value = stringify(data.interpretation.visible_operations);
    }
    updatePlanningControls();
  }
  function showInterpretation(data) { $('interpretation-panel').hidden = false; $('interpretation-panel').open = true; $('interpretation').textContent = stringify(data); }
  async function pollRun() {
    if (!state.run) return;
    const selectedRun = state.run.run_id; const connection = state.connectionRevision; const data = await request('/runs/' + selectedRun); if (connection !== state.connectionRevision || state.run?.run_id !== selectedRun) return; state.run = data;
    $('run-status').textContent = data.status + (data.active ? ' · worker active' : '') + (data.worker_process?.response_ok === false ? ' · admission refused' : '');
    $('review-status').textContent = 'Scientific review: ' + (Object.keys(data.output_review_statuses || {}).length ? stringify(data.output_review_statuses) : 'not assessed');
    $('progress').textContent = stringify(data);
    for (const id of ['cancel', 'refresh', 'resume', 'export-outputs', 'export-checkpoint']) $(id).disabled = false;
    $('mandatory-export-run').disabled = false;
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
  async function exportIntentEvidence(owner, id) {
    if (!state.token || !/^[a-f0-9]{32}$/.test(id || '')) throw new Error('Select a saved plan or run before exporting its declaration evidence.');
    const response = await fetch(endpoint + '/' + owner + '/' + id + '/intent-evidence', { headers: { Authorization: 'Bearer ' + state.token }, cache: 'no-store', credentials: 'omit', redirect: 'error' });
    const text = await response.text();
    if (!response.ok) { let message = 'Declaration evidence export was refused.'; try { message = parseJson(text).error?.message || message; } catch {} throw new Error(message); }
    const url = URL.createObjectURL(new Blob([text], { type: 'application/json' })); const link = element('a'); link.href = url; link.download = 'aurora-' + id + '-intent-evidence.json'; link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  $('connect-form').addEventListener('submit', action(async () => { state.connectionRevision += 1; const connection = state.connectionRevision; invalidateIntent('Connection changed. Prepare or load a review again.'); clearPlan(); state.token = $('token').value; $('token').value = ''; $('connection-status').textContent = 'Connecting…'; try { const catalogue = await request('/catalogue'); if (connection !== state.connectionRevision) return; state.catalogue = catalogue.operations; state.capabilities = Array.isArray(catalogue.capabilities?.profiles) ? catalogue.capabilities.profiles : []; $('provider-coverage').textContent = '';  updatePlannerAvailability(); $('catalog-count').textContent = catalogue.operation_count + ' operations · ' + catalogue.integration_count + ' integration IDs'; $('connection-status').textContent = 'Connected to this local API'; renderCatalogue(); } catch (failure) { if (connection !== state.connectionRevision) return; state.token = ''; state.catalogue = []; state.capabilities = []; $('provider-coverage').textContent = '';  updatePlannerAvailability(); $('connection-status').textContent = 'Disconnected'; throw failure; } }));
  $('disconnect').addEventListener('click', () => { state.connectionRevision += 1; invalidateIntent('Disconnected. Prepare or load a review after connecting.'); clearPlan(); state.token = ''; state.catalogue = []; state.capabilities = []; $('provider-coverage').textContent = '';  updatePlannerAvailability(); clearTimeout(state.timer); $('token').value = ''; $('connection-status').textContent = 'Disconnected'; $('source-grant').checked = false; $('run').disabled = true; $('mandatory-export-run').disabled = true; });
  $('catalog-filter').addEventListener('input', renderCatalogue);
  $('inspect-providers').addEventListener('click', action(async () => {
    if (!state.token) throw new Error('Connect this page before inspecting provider coverage.');
    const token = state.token; const catalogue = state.catalogue;
    const data = await request('/providers');
    if (state.token !== token || state.catalogue !== catalogue) return;
    $('provider-coverage').textContent = stringify(data);
  }));
  $('plan-form').addEventListener('submit', action(async () => { $('build').disabled = true; const ticket = ++state.planRequest; try { const snapshot = planningSnapshot(); clearPlan(); const body = planRequest(); const data = await request('/plans', body); if (ticket !== state.planRequest || planningSnapshot() !== snapshot) throw new Error('The task or planning controls changed while planning. Build a new plan for the current settings.'); acceptPlan(data); } finally { if (ticket === state.planRequest) $('build').disabled = false; $('provider-grant').checked = false; } }));
  $('interpret').addEventListener('click', action(async () => showInterpretation(await request('/interpret', { task: $('task').value }))));
  for (const id of ['planner', 'max-steps', 'max-model-calls', 'decision-attempts', 'operation-ids']) $(id).addEventListener('change', () => { intentChanged(); updatePlanningControls(); });
  for (const id of ['max-steps', 'max-model-calls', 'decision-attempts', 'operation-ids']) $(id).addEventListener('input', intentChanged);
  $('task').addEventListener('input', () => { intentChanged(); if (state.plan) $('validation-status').textContent = 'Task changed — build a new plan to retain a new task contract.'; });
  $('mandatory-enabled').addEventListener('change', () => { intentChanged(); updatePlanningControls(); });
  $('mandatory-add').addEventListener('click', action(async () => {
    if (state.intentRows.length >= 128) throw new Error('A review supports at most 128 mandatory items.');
    const span = selectedTaskSpan();
    state.intentRows.push({ id: 'item-' + crypto.randomUUID(), kind: 'required_operation', ...span, operation: '', output: false, path: '', literal: 'null', reason: '' }); intentChanged(); renderIntentRows();
  }));
  $('mandatory-new').addEventListener('click', () => { state.intentContext = null; state.intentRows = []; $('mandatory-review-id').value = ''; intentChanged(); clearPlan(); renderIntentRows(); });
  $('mandatory-prepare').addEventListener('click', action(() => prepareReview()));
  $('mandatory-suggest').addEventListener('click', action(() => prepareReview(true)));
  $('mandatory-load').addEventListener('click', action(loadReview));
  $('mandatory-review-id').addEventListener('input', () => invalidateIntent('Review selection changed. Load it before accepting.'));
  $('mandatory-accept-grant').addEventListener('change', () => { $('mandatory-accept').disabled = !$('mandatory-accept-grant').checked || !state.review || state.review.snapshot !== planningSnapshot(); });
  $('mandatory-accept').addEventListener('click', action(acceptReview));
  $('mandatory-export-plan').addEventListener('click', action(() => exportIntentEvidence('plans', state.plan?.plan_id)));
  $('mandatory-export-run').addEventListener('click', action(() => exportIntentEvidence('runs', state.run?.run_id)));
  $('plan-name').addEventListener('input', () => { state.proposal.name = $('plan-name').value; dirty(); updateProposalJson(); });
  $('outputs').addEventListener('input', () => { dirty(); try { const outputs = parseJson($('outputs').value); if (!Array.isArray(outputs)) throw new Error(); state.proposal.outputs = outputs; $('outputs').setCustomValidity(''); updateProposalJson(); } catch { $('outputs').setCustomValidity('Outputs require a JSON array.'); $('validation-status').textContent = 'Outputs require a JSON array.'; } });
  $('proposal').addEventListener('input', dirty);
  $('apply-json').addEventListener('click', action(async () => { const proposal = parseJson($('proposal').value); if (!proposal || !Array.isArray(proposal.steps) || !Array.isArray(proposal.outputs) || typeof proposal.name !== 'string') throw new Error('A proposal requires name, steps, and outputs.'); state.proposal = proposal; $('plan-name').value = proposal.name; $('outputs').value = stringify(proposal.outputs); $('outputs').setCustomValidity(''); dirty(); renderSteps(); }));
  $('add-step').addEventListener('click', () => { state.proposal.steps.push({ id: 'step_' + (state.proposal.steps.length + 1), operation: '', arguments: {}, depends_on: [] }); dirty(); renderSteps(); updateProposalJson(); });
  $('validate').addEventListener('click', action(async () => { if (state.planningSettingsDirty) throw new Error('Build a new plan for the changed planning settings.'); if ($('task').value !== state.plan.task) throw new Error('Build a new plan for the changed task before editing its steps.'); for (const input of [...$('steps').querySelectorAll('input,textarea'), $('outputs')]) { if (!input.checkValidity()) { input.reportValidity(); throw new Error('Correct the invalid step fields before saving.'); } } const proposal = parseJson($('proposal').value); const revision = state.editRevision; $('validate').disabled = true; try { const saved = await request('/plans/validate', { plan_id: state.plan.plan_id, expected_plan_digest: state.plan.plan_digest, proposal }); if (state.editRevision !== revision) { state.plan = saved; $('plan-identity').textContent = 'Revision ' + saved.plan_id + '\nDigest ' + saved.plan_digest; dirty(); throw new Error('An earlier snapshot was saved while you edited. Validate the current edits before approval.'); } acceptPlan(saved); } finally { $('validate').disabled = false; } }));
  $('source-grant').addEventListener('change', () => { $('run').disabled = state.pendingRun || !state.token || state.dirty || !$('source-grant').checked; });
  $('run').addEventListener('click', action(async () => { if (state.dirty || !$('source-grant').checked) throw new Error('Validate edits and explicitly approve the exact saved plan first.'); if (state.pendingRun) return; state.pendingRun = true; $('run').disabled = true; $('source-grant').disabled = true; let data; try { data = await request('/runs', { plan_id: state.plan.plan_id, expected_plan_digest: state.plan.plan_digest, approve_source_dispatch: true, max_step_calls: Number($('call-budget').value) }); } finally { state.pendingRun = false; $('source-grant').checked = false; $('source-grant').disabled = state.dirty; } state.run = data; $('run-id').value = data.run_id; $('source-grant').checked = false; $('run').disabled = true; await pollRun(); }));
  $('load-run').addEventListener('click', action(async () => { const id = $('run-id').value; if (!/^[a-f0-9]{32}$/.test(id)) throw new Error('Enter a 32-character lowercase hexadecimal run ID.'); invalidateIntent('Run selection changed. Load its review before starting another model plan.'); clearPlan(); const connection = state.connectionRevision; state.run = { run_id: id }; await pollRun(); if (connection !== state.connectionRevision) return; if (state.run.plan_id) { const saved = await request('/plans/' + state.run.plan_id); if (connection !== state.connectionRevision || state.run?.run_id !== id) return; $('task').value = saved.task; $('planner').value = saved.planner; acceptPlan(saved); } }));
  $('refresh').addEventListener('click', action(pollRun));
  $('cancel').addEventListener('click', action(async () => { await request('/runs/' + state.run.run_id + '/cancel', {}); await pollRun(); }));
  $('resume').addEventListener('click', action(async () => { if (!$('source-grant').checked || state.dirty || !state.plan || state.plan.plan_digest !== state.run.plan_digest) throw new Error('Load the unchanged saved plan and approve its exact digest before resuming.'); await request('/runs/' + state.run.run_id + '/resume', { expected_plan_digest: state.run.plan_digest, approve_source_dispatch: true, retry_failed: $('retry-failed').checked, resume_cancelled: $('release-cancel').checked, max_step_calls: Number($('call-budget').value) }); $('source-grant').checked = false; $('retry-failed').checked = false; $('release-cancel').checked = false; $('run').disabled = true; await pollRun(); }));
  $('export-outputs').addEventListener('click', action(() => exportRun('outputs'))); $('export-checkpoint').addEventListener('click', action(() => exportRun('checkpoint')));
  updatePlanningControls();
})();
