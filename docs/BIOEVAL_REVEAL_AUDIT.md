# Bioevaluation prospective seal and reveal audit

bioeval_reveal_audit exposes the bioprism-bioevalx prospective evaluation state machine. It
records what was committed, freezes the commitment set under a content digest of the rubric,
reveals outcomes only after sealing, and preserves the difference between an admissible pair and
a correct prediction.

The route is intentionally not a scoring engine. A commitment contains an opaque prediction and
an analysis plan, but this kernel does not invent a loss function or a biological truth criterion.
It certifies that a later scoring operation is using the same sealed rubric and that every scored
outcome names a target that was committed before reveal.

## Request

~~~json
{
  "study": "prospective-2026",
  "commitments": [
    {
      "target": "case-a",
      "prediction": { "class": "stable" },
      "analysis_plan": "plan-v1"
    },
    {
      "target": "case-b",
      "prediction": { "class": "progression" },
      "analysis_plan": "plan-v1"
    }
  ],
  "rubric": {
    "version": 1,
    "rules": ["predeclared"]
  },
  "sealed_at": "2026-08-16T12:00:00Z",
  "outcomes": [
    {
      "target": "case-a",
      "observed": { "class": "stable" }
    }
  ],
  "score_rubric": {
    "version": 1,
    "rules": ["predeclared"]
  },
  "require_scoring": true,
  "require_rubric_match": true,
  "require_complete": false
}
~~~

study and target identifiers are bounded strings. prediction and observed are arbitrary
JSON-compatible values: the route carries them without interpreting them. analysis_plan is
required because a result can be paired with the right outcome while still being evaluated under
the wrong declared plan.

sealed_at is an RFC-3339 value parsed by the shared timestamp contract. It is recorded in the
projection, but it is a caller assertion, not a timestamp authority or signature. A prospective
claim should bind this projection to an external attestation if timestamp provenance matters.

## State machine

The underlying transitions are:

~~~text
Registration(open)
    -- commit(target, prediction, analysis_plan) -->
Registration(open)
    -- seal(rubric, sealed_at) -->
Sealed(frozen commitments, rubric digest)
    -- reveal(outcomes) -->
Revealed(outcomes)
    -- score_under(score_rubric) -->
Scoring(admissible pairs, unrevealed commitments)
~~~

The open registration is consumed by seal. A caller cannot use the returned Sealed value to add a
new commitment. The sealed state is consumed by reveal. A caller cannot use the returned Revealed
value to reveal a second outcome set. The route probes both locks and reports their real refusal
messages in seal_lock and reveal_lock.

The probes are evidence about the state machine, not extra user commitments or outcomes. They
use an internal sentinel target and do not change the sealed or revealed state.

## Commitment digest

At seal, the kernel computes:

- a rubric digest over the canonical JSON rubric value; and
- a commitment digest over the canonical serialized commitment map.

The projection returns both digests. The commitment map is keyed by target, so duplicate targets
are refused before sealing. The route also refuses duplicate outcome targets to avoid turning one
target into multiple rows with ambiguous completion semantics.

JSON formatting does not matter to the digest. A semantically identical rubric represented with
different whitespace has the same canonical content. A changed version, rule, threshold, or
other JSON value has a different digest.

The digest is an integrity witness, not a provenance witness. It proves that the scoring rubric
matches what this request sealed; it does not prove that the rubric was sealed before a public
leak, that an external registry received it, or that the caller controlled the timestamp.

## Reveal and scoring

An outcome is admissible for scoring only when its target is present in the frozen commitment map.
An outcome for a new target returns a nested scoring refusal with the uncommitted target in the
kernel error. The route does not discard the outcome or reinterpret it as a negative.

When score_rubric is omitted, scoring status is not_requested. When it is supplied, the real
score_under operation runs:

- accepted means the presented rubric digest matches the sealed digest;
- refused means the digest changed or an outcome was not committed; and
- complete is true only when every commitment has an outcome.

The value field contains Scoring for an accepted projection. Scoring retains study, rubric_digest,
commitment_digest, every admitted predicted/observed pair, and unrevealed target identifiers.
The route returns no accuracy, loss, rank, or biological conclusion.

The optional policies are:

- require_scoring refuses when no score rubric was supplied or scoring was refused;
- require_rubric_match refuses unless the score rubric matches the sealed digest; and
- require_complete refuses unless admitted scoring covers every commitment.

Policies are fail-closed. A request without require_complete can successfully return an incomplete
scoring projection, with selective_publication true and the unrevealed targets visible. Incomplete
does not become complete merely because the revealed subset scored successfully.

## Selective publication

Selective publication is represented as a first-class finding. If case-a is committed and revealed
but case-b is committed and omitted, the projection reports:

~~~json
{
  "scoring": {
    "status": "accepted",
    "complete": false,
    "value": {
      "scored": [{ "target": "case-a" }],
      "unrevealed": ["case-b"]
    }
  },
  "findings": {
    "unrevealed_commitments": {
      "ids": ["case-b"],
      "total": 1,
      "omitted": 0
    },
    "selective_publication": true
  }
}
~~~

This is not a claim that case-b was intentionally hidden. It is a structural fact about what the
request revealed. An external release policy can choose to block incomplete publication through
require_complete.

## Successful projection

Schema is bioprism-mcp/bioeval-reveal-audit/0.1.

~~~json
{
  "ok": true,
  "schema": "bioprism-mcp/bioeval-reveal-audit/0.1",
  "workflow": "bioeval_reveal_audit",
  "study": "prospective-2026",
  "sealed_at": "2026-08-16T12:00:00Z",
  "digests": {
    "rubric": "content-hash",
    "commitments": "content-hash"
  },
  "commitments": {
    "rows": [],
    "returned": 0,
    "total": 2,
    "omitted": 2
  },
  "outcomes": {
    "rows": [],
    "returned": 0,
    "total": 1,
    "omitted": 1
  },
  "seal_lock": {
    "status": "refused",
    "refusal": "this registration is already sealed"
  },
  "reveal_lock": {
    "status": "refused",
    "refusal": "the outcome has already been revealed"
  },
  "scoring": {
    "status": "accepted",
    "value": {
      "study": "prospective-2026",
      "rubric_digest": "content-hash",
      "commitment_digest": "content-hash",
      "scored": [],
      "unrevealed": ["case-b"]
    },
    "refusal": null,
    "complete": false
  },
  "findings": {
    "unrevealed_commitments": {
      "ids": ["case-b"],
      "total": 1,
      "omitted": 0
    },
    "selective_publication": true,
    "rubric_match_refused": false,
    "uncommitted_outcome_refused": false,
    "seal_lock_refused": true,
    "reveal_lock_refused": true
  },
  "guarantees": [
    "the rubric and commitment set are content-addressed at seal time",
    "outcomes are revealed only after the commitment set is frozen",
    "a changed rubric cannot produce an admitted scoring projection",
    "unrevealed commitments remain visible instead of disappearing from the denominator",
    "second reveal and post-seal commitment probes are refused by the state machine"
  ],
  "limitations": [
    "sealed_at is a caller assertion and not an external timestamp attestation",
    "the route does not sign commitments or search public artifacts for prior leakage",
    "prediction-versus-outcome correctness is left to a separate scoring contract"
  ]
}
~~~

Every commitment, outcome, and finding identifier projection is bounded. Each bounded collection
has returned, total, and omitted counts. Truncating rows does not remove unrevealed totals.

## Refusal stages

The route uses structured fail-closed stages:

- timestamp_validation protects the shared RFC-3339 parser;
- commitment_deserialization and commitment_validation protect frozen target and plan identity;
- commitment_admission preserves duplicate-target refusal;
- seal protects the nonempty-registration transition;
- outcome_deserialization and outcome_validation protect the reveal set;
- scoring_policy protects a required scoring operation;
- rubric_integrity_policy protects a required unchanged rubric; and
- completeness_policy protects a required complete publication set.

A nested scoring refusal is retained in an otherwise admissible audit when no strict scoring policy
was requested. This keeps the difference between “the prospective state machine admitted the
registration and reveal” and “the optional scoring request could not be paired” visible.

## SDK surfaces

Python exposes BioevalRevealCommitmentArgs, BioevalRevealOutcomeArgs, BioevalRevealAuditArgs,
BioevalRevealAuditReport, and bioeval_reveal_audit_report through Workspace, AsyncWorkspace,
ApiClient, and AsyncApiClient. The argument layer validates unique targets, JSON-compatible
prediction and observation values, bounded identifiers, and the input ceiling.

TypeScript exposes typed commitment, outcome, audit-argument, and audit-result interfaces plus
bioevalRevealAudit. Both SDK families preserve digest, lock, selective-publication, and nested
scoring refusal fields rather than flattening them to a pass/fail score.

## Composition and boundaries

This route composes with bioeval_design_audit when a design needs a prospective seal around its
arms, and with bioeval_mesh_audit when evaluator verdicts should be revealed only after a panel
was frozen. It can feed a release-gate waiver audit as evidence about completeness or rubric
integrity, but it does not waive its own refusal.

It does not replace evaluation_worldline_audit. A sealed rubric can still have leaked through a
public channel before the recorded sealed_at value. Worldline accessibility and external
attestation remain separate evidence obligations.

The central boundary is between admissibility and correctness. A sealed, complete, digest-matched
pair is eligible for an external scoring rule. It is not automatically a correct prediction,
causal finding, clinical conclusion, or biological truth.
