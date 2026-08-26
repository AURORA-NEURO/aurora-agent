# The receipts audit: a depth battery for digest-sealed documents

Every receipt this workspace emits makes the same claim — *recompute the digest and you will catch
any later edit*. Until now each verifier backed that claim with a handful of happy-path tests and
one hand-written tamper case, which establishes something much weaker: that the verifier catches
the one edit whoever wrote the test thought of.

`bioprism-receipts-audit` replaces that with a measurement. It enumerates every position in a
well-formed document, generates a structure-aware mutation at each one, states in advance whether
that mutation is formatting-only or semantic and — where the document says what the right answer is
— *which* refusal it must produce, then checks the verifier against the statement. What comes out is
a number: how many positions, how many cases, and how many the verifier got wrong.

The crate is a library plus two test suites. The library is reusable — a new digest-sealed
document type gets a battery by writing one closure — and the suites are the workspace's current
application of it: `receipt_battery.rs` over the six receipt documents the crate started with, and
`verifier_battery.rs` over thirteen more verifiers reached since. The two run the same generators
against the same expectations; where a number below belongs to one of them, it says which.

## The two expectations

Every generated case carries exactly one of two claims, and there is deliberately no third:

| expectation | means | example |
|---|---|---|
| `Expect::VerdictUnchanged` | the canonical bytes are identical and so is the verdict | an object's keys written in a different order |
| `Expect::Rejected(_)` | the canonical bytes differ and the document must be refused | an array's elements written in a different order |

The pair is the point. JSON objects are unordered and JSON arrays are ordered, so exactly one of
those two examples may move a digest. If key ordering changed a verdict, the digests in this
workspace would be artefacts of one serializer rather than names for content, and the
cross-language replay the certificates depend on would be a coincidence. If array ordering did
*not* change a verdict, the digest would not be naming the document.

A generator that could not decide which expectation it was producing would be testing nothing,
which is why no `Expect::Unknown` exists. Cases whose canonical bytes turn out to equal the
original's are dropped before execution rather than asserted on — a digest cannot distinguish a
document from itself, and claiming such a case was "rejected" would be claiming something untrue.

## Refused is not enough

A battery that asked only *was this refused?* would pass a verifier that answers every question
with the same wrong word. The rejection classes are a shipped distinction, and the two that matter
most say opposite things about who is at fault:

- `digest_mismatch` — the claimed digest is a digest, and the body no longer hashes to it. *The
  document moved after it was sealed.*
- `digest_malformed` — the claimed digest is present and was never a digest. *The producer wrote a
  typo.*
- `digest_absent` — there is no digest to check. *Nothing was claimed.*

Reporting the second as the first accuses the holder of a receipt of tampering on the strength of a
typo. That is not hypothetical: it is one of the two holes this battery found and closed.

So `Expect::Rejected` carries a `Refusal`, and where the correct class is determined the battery
demands it. `BatteryConfig::sealed_by` names the field that seals a document, and `refine` turns
each generator's bare *refused* into the specific answer that case has to produce:

| what the edit did to the sealing digest | the answer it must get |
|---|---|
| replaced it with a well-formed digest, and touched nothing else | `digest_mismatch`, exactly |
| broke its shape, and touched nothing else | `digest_malformed`, exactly |
| removed it, and touched nothing else | `digest_absent`, exactly |
| left it holding an empty string, or something that is not a string | `digest_absent` or `digest_malformed` |
| left it exactly as issued and moved the body instead | `digest_mismatch`, `malformed`, or `structural_failure` — **never** `digest_absent` or `digest_malformed` |

The last row is the one that needs an exhaustive sweep to check, and it is the rule that would have
caught the certificate hole on its own: an edit anywhere in the body leaves the claimed digest
exactly as the producer issued it, so no answer that blames the digest can be right. In the receipt
battery, 17,310 body edits across its five sealed documents are held to it, and 375 cases are
pinned to a single class.

Where two readings are genuinely defensible the library permits both rather than manufacturing a
failure out of a design choice — a `bundle_digest` of `""` is a present-and-defective digest to
four verifiers and no digest at all to the fifth, and both are arguable. The *measurement* of which
one each verifier picked is pinned in the battery instead, so a change in either direction is
visible. See the finding below.

## The generators

Fourteen families, all pure functions of the document and a seeded SplitMix64. Where a family has
more candidates than it emits — which sibling pair to swap, which hex digit to substitute, how far
to rotate an array — the choice comes from that generator, so the seed printed with every reported
hole reproduces the exact case.

| family | what it does | expectation |
|---|---|---|
| `digest_byte_flip` | one different hex character at **every** offset of **every** digest | rejected |
| `digest_length_change` | a digest one character short, one long, two long, headless, or empty | rejected |
| `digest_case_change` | a digest fully uppercased, and one character uppercased | rejected |
| `sibling_swap` | two same-typed siblings exchanged inside one container | rejected |
| `required_key_deletion` | each key of each visited object removed in turn | rejected |
| `array_element_deletion` | each element of each visited array removed in turn | rejected |
| `unexpected_key` | a key the schema does not know, sorting before and after every existing key | rejected |
| `numeric_near_equal` | an integer as its equal-valued float, a float one ULP away, a zero with its sign flipped | rejected |
| `numeric_boundary` | `i64::MAX`, `i64::MIN`, `0`, `-1`, `u64::MAX`, the first integer no `f64` holds exactly, a float where an integer stood, and the three non-finite spellings | rejected |
| `object_key_reordering` | the same entries reversed and rotated | **unchanged** |
| `array_reordering` | the same elements reversed and rotated | rejected |
| `unicode_confusable_string` | Cyrillic homoglyphs, a precomposed/decomposed accent pair, an appended zero-width space | rejected |
| `empty_or_null_substitution` | each visited value replaced by `""` and by `null` | rejected |
| `wire_duplicate_key` | one object written with a key twice, parsed back with the last occurrence winning | rejected |

`sibling_swap` is the family a field-by-field validator is most likely to miss: no key is added,
removed, or retyped, and every value in the container is one the producer really emitted — only the
binding between name and value moved.

`numeric_near_equal` exists because `1` and `1.0` print the same to a careless reader and encode
differently. A verifier that accepted both interchangeably would make its digest depend on how a
caller's JSON parser happened to type a literal.

`numeric_boundary` asks the opposite question: not whether a verifier notices a change too small to
see, but whether it notices one too large to be plausible. A count at `i64::MAX`, a byte length of
`-1`, an identifier past the exact-integer range of the `f64` a JavaScript reader would parse it
into. `9007199254740993` is in the list on purpose — it is the first integer an `f64` cannot hold,
so a verifier that round-trips a number through a float cannot tell it from its predecessor.

NaN and infinity appear only as the strings `"NaN"`, `"Infinity"`, and `"-Infinity"`, because the
format permits them in no other form: `serde_json::Number` refuses to hold a non-finite value,
`{"x":NaN}` is not parseable JSON, and `to_canonical_string` refuses to write one. A string that
spells one is what a coercing parser on the far side would revive it from, and it is the closest
this format gets. The library asserts all four of those facts rather than skipping the case
quietly.

## The properties asserted

Each is a claim-named test in `crates/receipts-audit/tests/receipt_battery.rs`. The
verifier battery asserts the same properties over its own thirteen subjects, in
`crates/receipts-audit/tests/verifier_battery.rs`; what it adds is listed after this section.

- **Every single-byte digest mutation is caught at every offset of every digest field.** Digest
  coverage is never bounded. A digest that catches tampering at 63 of its 64 offsets is not a
  digest, and a battery that sampled offsets could not tell the difference. At the 320 offsets of
  the five *sealing* digests the answer must be `digest_mismatch` and nothing else.
- **Object key reordering never changes a verdict at any position**, and the canonical bytes are
  asserted identical as well — if they were not, the defect would be in canonicalisation, not in
  the verifier, and the battery reports those separately.
- **Array reordering always changes a verdict at any position.**
- **A document whose sealing digest is absent is rejected distinctly from one whose digest is
  wrong.** Absent is `digest_absent`; wrong is `digest_mismatch`.
- **A shape break in the sealing digest is reported as malformed and never as tampering**, at every
  one of the seven shape mutations of all five sealing digests.
- **The five verifiers agree on every unusable sealing digest except the empty string.** Twelve
  shapes — deleted, empty, null, a number, an array, an object, a boolean, 63 hex, 65 hex,
  uppercase, non-hex, whitespace-padded — against five verifiers, sixty pinned answers, none of
  them `digest_mismatch`.
- **Every body edit forbids the two answers that would blame the digest.**
- **Deleting any field at any visited position is rejected and never silently accepted.**
- **A numeric near-equal substitution lands on one stable verdict and never between them** — each
  case is verified twice and both answers must agree.
- **A numeric boundary substitution is refused at every numeric position**, and the class breakdown
  is pinned (see the finding below).
- **A non-finite number can only reach a verifier as a string, and is refused as one.**
- **A string replaced by a confusable form is rejected at every visited position.**
- **A swapped pair of same-typed siblings is rejected at every visited container.**
- **An unexpected key at any level is rejected**, except where a recorded gap says otherwise (see
  below).
- **An object written with a duplicate key resolves to a document that is rejected.**
- **A document fed to a verifier that does not own it is always rejected** — twenty cross-feeds,
  five verifiers against four foreign documents each.
- **Verifying the same document twice yields byte-identical projections**, for accepted documents
  and for rejected ones. A verifier that memoised its first answer would pass the accepted half
  alone.
- **No subject carries a position bound.**

### Differential checks

Three cross-implementation agreements, in the spirit of the repository's parity discipline:

- The certificate digest agrees across three paths: the value the producer embedded, a
  recomputation through `ContentHash::of_value`, and a direct `Sha256` over the canonical bytes
  with no wrapper in between. A pre-processing step inside the wrapper would show up here.
- Every inlined dossier artifact hashes to the digest its own record claims, and its recorded
  `canonical_bytes` is the length its content actually encodes to.
- The figure renderer and the dossier agree on every artifact digest: each figure's
  `source sha256:` footer, computed inside `bioprism-figures`, names an artifact the dossier
  records — and the digest the dossier verifier recomputes is the one the rendered report prints.

## What was measured

### The receipt battery

Six subjects, one seed (`0x9E3779B97F4A7C15`), 18,320 generated cases, every position of every
document.

| subject | positions | digest fields | digest offsets | cases |
|---|---|---|---|---|
| context certificate | 39 of 39 | 4 | 256 | 568 |
| autopilot report | 93 of 93 | 11 | 704 | 1,451 |
| research dossier | **1,981 of 1,981** | 27 | 1,728 | 14,777 |
| mission evidence bundle | 34 of 34 | 2 | 128 | 374 |
| delivery receipt | 89 of 89 | 7 | 448 | 996 |
| delivery audit behind a fixed receipt | 39 of 39 | 0 | 0 | 154 |
| **total** | **2,275 of 2,275** | **51** | **3,264** | **18,320** |

Cases by family: 4,520 empty-or-null substitutions, 3,667 confusable strings, 3,264 digest byte
flips, 2,269 deletions, 1,940 numeric boundary substitutions, 754 unexpected keys, 472 key
reorderings, 376 duplicate keys, 357 digest shape changes, 315 sibling swaps, 223 numeric near-equal
substitutions, 163 array reorderings.

### The verifier battery

Thirteen more subjects, the same seed, **5,221 positions and 47,976 generated cases**, again every
position of every document — `the_whole_battery_finds_no_hole_outside_the_gaps_this_repository_has_named`
pins both totals and asserts `Coverage::is_exhaustive()` per subject, so no bound can creep back in
unannounced. Across them are 164 digest fields, each checked at all 64 offsets, for 10,496
single-character digest mutations.

The subjects: the prism result bundle, the registry benchmark pack, the conformance certificate,
the cookbook report, the bioworlds catalogue report, the examples registry report, the repair plan,
the repair acceptance report, the two domain workflow verifications, the workbench verification,
and the two provider replay requests.

Two of them are not sealed documents in the sense the rest are. The workflow verification is a
*replay* comparison — it re-derives the retained document from the caller's request and compares
field by field — and the repair acceptance report carries no integrity claim at all. The second is
measured for the size of its silence rather than for a broken promise: `unsealed_accepted_cases`
pins that its verifier accepts 195 of the battery's mutations, and every per-family test pins how
many of those it saw, so a new acceptance changes a number rather than passing unremarked.

Cases by family where a per-family test pins them: 10,579 confusable strings, 10,496 digest byte
flips, 10,400 empty-or-null substitutions, 5,208 deletions, 1,576 unexpected keys, 1,271 key
reorderings, 1,148 digest shape changes, 1,018 sibling swaps, 780 duplicate keys, 541 array
reorderings. The remaining 4,959 are the two numeric families, which the headline total covers.

### Which verifiers are covered, and which are not

Nineteen subjects across the two batteries is not every verifier in the workspace, and
`every_document_verifier_in_the_workspace_is_covered_or_recorded` is what stops this document from
implying otherwise. It scans `crates/*/src` for both shapes an entry point takes here — a `pub fn`
whose name begins with `verify`, and `pub fn digest_is_intact` — and fails unless every site is in
one of four lists: driven here, driven by the receipt battery, not a document verifier at all, or a
document verifier no battery reaches. Fifty-seven entries, and **ten of them are in the last list**:
the per-slice self-seals in `bioworlds` and `examples` that their catalogue reports never recurse
into, `ResultBundle::verify` in `bundle`, the factory's two snapshot verifiers, the ledger's
projection checkpoint, the escrow reveal, the stewardship pre-registration, the registry index, and
`Credit::verify`.

The scan finds functions by name, which is a real bound and is stated in the test: a verifying
*constructor* is invisible to it. `RepairPlan::from_json` and `AcceptanceReport::from_json` are two
the verifier battery already drives, and `WorldTape` verifies its chain from a `#[serde(try_from)]`
reader that no name pattern would catch.

### The bound is gone

The research dossier used to be the one document the sweep did not cover: 1,981 positions, visited
at every eighth JSON pointer. That bound has been removed and `no_subject_carries_a_position_bound`
asserts it stays removed.

Removing it cost less than it looks, because the mutation loop was rewritten first. A case is now a
`walk::Patch` — the pointer to the smallest subtree that differs and the value to put there —
instead of a whole mutated document. Three costs went with the change:

- **No copy per case.** `run_cases` keeps one working document and swaps each patch into it and
  back out again with `std::mem::swap`. Measured head to head over the dossier's 14,777 cases: the
  old shape, a fresh document per case, costs **4.52 ms/case**; the swap costs **2.98 ms/case**, of
  which ~2.0 ms is the verifier itself. Cloning the dossier alone was 0.34 ms/case.
- **No re-serialisation of the document per case.** Asking whether a case moves the canonical bytes
  is now a question about the patched subtree, not about 42 kB of dossier. The two answers are
  identical — canonical JSON writes a value the same way wherever it sits — and
  `judging_degeneracy_by_the_patched_subtree_agrees_with_judging_it_by_the_whole_document` holds
  them to it over a seeded corpus of 64 documents rather than one example, running every family
  over each and asserting the two judgements agree on all 5,000-plus cases. Dropping degenerate
  cases across the whole dossier now costs 116 ms.
- **One traversal, not four.** `walk::pointers` ran once per digest family and again in
  `run_battery`; the walk and the digest-field scan are now hoisted to one call each, 8 ms total.

End to end for the dossier, in a debug build: **74.6 s for 13,139 cases before, 55.4 s for 14,777
cases after** — and the 14,777 includes the 1,638 numeric-boundary cases the old battery did not
have. That is comfortably inside a minute for the one document that needed the bound, so no bound
remains. The receipt battery's 27 tests now run in 50-90 s. The verifier battery is the long pole
and is not inside a minute: its 20 tests take 150-225 s in a debug build depending on machine load,
because it runs two and a half times the cases over documents whose verifiers do more work per
call. The whole crate is 92 tests in four to six minutes.

## What the batteries found

From the receipt battery: two holes, both fixed; one gap, recorded rather than closed; two
measurements that are not holes but belong in the record; and one behaviour noted and left alone,
because fixing it meant changing a function this work could not validate end to end. Those are the
six sections that follow.

From the verifier battery, which is newer and swept a wider surface, the shape of the result is
different and the honest summary is that it found a great deal more. One hole was closed outright —
an unrecognised key was dropped by the reader before hashing, so a document carrying content nobody
hashed still verified — by making every sealed report type refuse a field it does not declare. What
remains is recorded in three lists rather than two, and they are described in
"What the verifier battery left open" below: **34 known gaps**, **10 open holes**, and **2
flattened envelopes**. Every entry in all three carries a must-still-fire assertion, so a gap that
stops reproducing fails the battery instead of rotting into a stale excuse.

### Fixed — the certificate verifier reported a typo as tampering

`ContextCertificate::verify` did not check the shape of `certificate_sha256` before comparing it.
A digest that was uppercase, truncated, or not hex at all recomputed to something different and was
reported as `DigestMismatch` — the answer that means *the body moved after the digest was taken*.
`verify_autopilot_report`, `verify_dossier`, and `verify_mission_evidence_bundle` all ship this
distinction and document it; the certificate, the oldest of the four, did not.

Fixed in `crates/section/src/certificate.rs`: a `certificate_sha256` that is not a 64-character
lowercase hex digest is now `Malformed` with a reason that names the field. The variant already
existed, so no consumer's match arm changed.

### Fixed — the delivery receipt verifier checked six fields out of twenty

`verify_delivery_receipt` recomputed the receipt from the delivery audit and then compared six
projections: the three digests, the targets, the evidence, and `release_candidate`. Nothing else.
`receipt_digest` is taken over the receipt's identity, digests, targets, evidence, and readiness
flag, so it does not cover the derived counts, `structurally_valid`, `verification`, `findings`, or
the guarantee and limitation text either. Those fourteen fields were sealed by nothing and compared
against nothing.

The battery generated 106 mutations of a stored receipt that verified as `valid: true`, including
emptying the receipt's own `limitations` array and setting `ready_target_count` to a number the
targets do not support. On a document whose purpose is to be a checkable handoff, unprotected
honesty text is the worst of the fourteen.

Fixed in `crates/devplat/src/delivery_receipt.rs`: every field the recomputation produces is now
compared against the stored receipt, under a `receipt_projection_mismatch` finding code that leaves
the six pre-existing codes separately identifiable. The same change gives the receipt the
malformed-versus-mismatch distinction the other verifiers have, as
`receipt_digest_malformed`.

### Recorded, not closed — an unrecognised key on a receipt is not checked

The projection comparison is one-directional: a stored receipt may carry fields the recomputation
does not, and those are ignored. This is not an oversight to fix later. The shipped MCP surface
returns the receipt with `ok`, `workflow`, `valid`, `receipt_ready`, and `delivery` written onto the
same object, so treating an unrecognised key as tampering would reject every receipt the server
hands out.

The battery records this as a `KnownGap` with that reason, and asserts it twice over: matching
cases are excused from the hole count, *and* the gap must still fire. Closing the underlying
behaviour without deleting the entry fails the battery, so the list cannot rot into a set of stale
excuses.

### Measured — one verifier reads an empty digest as a missing one

`verify_mission_evidence_bundle` requires `bundle_digest` to be "a non-empty string", so a
`bundle_digest` of `""` comes back as `digest_absent`. The other four verifiers call it
`digest_malformed`, which is the reading that keeps the two answers apart: the field is there, and
what is in it was never a digest.

This is much milder than the certificate hole — it misdescribes a defect rather than blaming the
wrong party — and both readings are arguable, so the library permits either. What the battery does
is pin the measurement: `the_five_verifiers_agree_on_every_unusable_sealing_digest_except_the_empty_string`
asserts all sixty answers and names the one divergence, so fixing it and drifting further both fail
loudly. Every other unusable shape is unanimous: a field holding a non-string is `digest_absent`
everywhere, and every wrong-shaped string is `digest_malformed` everywhere.

### Measured — the digest is the only thing checking any number

Of the 1,940 numeric boundary substitutions, **1,891 were caught by the digest alone** and 49 by a
check of the verifier's own. All 49 are on the delivery receipt, whose verifier recomputes the whole
projection from the delivery audit and compares it field by field.

The four self-sealing documents apply no range or plausibility check to any number they carry.
`i64::MAX` in a count, `-1` in a byte length, `u64::MAX` in an identifier, `"NaN"` where a number
belongs — every one of them is stopped, and every one of them is stopped only because the canonical
bytes moved. Nothing was accepted that should not have been, which is the result the battery went
looking for; but a consumer that reads one of these receipts without recomputing its digest is not
protected from any of those values, and the class breakdown is pinned so that stays visible.

### Noted, not fixed — the certificate verifier checks a digest, not a schema

`ContextCertificate::verify` recomputes the embedded digest and nothing else. It does not check
that the document is a certificate. All twenty cross-document feeds in the battery are rejected,
because no other document type carries a `certificate_sha256` — but the rejection comes from the
absent field, not from a schema check, and a foreign document resealed with a self-consistent
`certificate_sha256` would verify.

This is reported rather than fixed. Adding a schema check changes the behaviour of a function
consumed by `crates/mcp/src/server.rs`, which this work could not modify, so the change could not be
validated end to end here.

## What the verifier battery left open

Three lists, each with a reason per entry and each asserted twice over: matching cases are excused
from the hole count, *and* the entry must still fire. Closing an underlying behaviour without
deleting its entry fails the battery.

- **`KNOWN_GAPS` (34)** — places where a refusal is not available to the reader rather than
  withheld by it. The largest groups: three documents whose only integrity check is a `bool`, so
  none of them can name the *class* of a defect in its own digest; positions carrying
  `#[serde(default)]`, where deleting a field and writing its default are the same document to the
  reader; an internally tagged enum, where `serde` cannot enforce `deny_unknown_fields` at all
  because the tagged representation buffers content before it knows which variant it is reading;
  the caller's replay request, which is an input to be re-instantiated rather than a document under
  check; and the caller-supplied halves of a workbench verification request — `session`,
  `ci_replay` and `policy` — which no digest covers and which stay open on purpose, because
  refusing a forward-compatible field on an input is a breaking wire change with nothing to protect.
- **`OPEN_HOLES` (10)** — real defects, reproducing, not fixed here. Two are the same
  `digest_is_intact()` problem: the digest is recomputed by re-serialising the *parsed* struct, so
  anything the reader normalises away is outside the seal, and an `f64` field written as an integer
  literal verifies against a digest taken over the other spelling. The other eight are one hole
  reached from six mutator families at two pointers: `claim_posture` and `parent_digests` on the
  provider replay are covered by none of the five digests it compares. Closing it means changing
  what `intake_digest` hashes, and that digest is a published wire value recorded across the
  workspace.
- **`FLATTENED_ENVELOPES` (2)** — positions where an unrecognised key cannot be refused at all,
  because `serde` will not combine `deny_unknown_fields` with `flatten` and these requests flatten
  their observation into the root. A limit of the reader, not a decision this repository made.

### Closed — an unknown key was dropped before it could be hashed

Every one of these reports recomputes its digest by re-serialising the parsed struct. A key the
reader did not declare was therefore discarded before the recomputation ever saw it: the claimed
digest still agreed, and a document carrying content nobody hashed read as intact. Twenty-eight
sealed report types across `bioworlds`, `cookbook`, `examples` and `devplat` now refuse a field
they do not declare, and each crate's docs say so.

The tightening stops at the seal. Eleven request and input types that the same sweep had also
closed were reopened: a `WorkbenchRequest`'s session, CI request and policy, and a
`PostTreatmentSpec`, are things a caller sends rather than documents under a digest, and rejecting
a field a newer schema added would be a breaking change protecting nothing. `DashboardQuery` is the
one type that is genuinely both — a caller's filter that is echoed verbatim into the sealed report
— and it was left open. The residual gap is real and this paragraph is its only record: an unknown
key at `/report/dashboard/query` is still dropped before hashing, and the battery cannot see it
because the workbench fixture carries no dashboard. The durable fix is a separate wire type for the
echoed query, or retaining it in the report as a raw value, rather than the attribute.

## The self-test

A battery that cannot fail measures nothing, so the library keeps three deliberately wrong
verifiers and asserts that each is caught:

- **`shallow`** compares a projection of two keys and one floating-point total. It really does
  refuse a document whose projection moves — it is not a verifier that says yes to everything — but
  an edit the projection does not reach gets through, and so does an edit to a number an `f64`
  rounds back onto the value it replaced. The battery must report both, including a
  `numeric_boundary` hole at exactly the integer past the fifty-third bit. The demand is on
  *every* family that claims a refusal, not a sample of them, and the one exemption is derived
  rather than named: `object_key_reordering` claims the verdict is unchanged, so a hole there would
  mean the verifier moved rather than that the family detected anything. The three digest families
  are held out too, and for the opposite reason — `shallow` is not shallow about the seal, so it
  catches them, and no hole there is the family working.
- **`blames_the_body`** is the honest verifier with one word changed: every refusal comes back as
  `digest_mismatch`. Nothing gets past it, so a battery that asked only *was this refused?* would
  call it perfect. It is caught by the class assertions and by nothing else — and the paired test
  `a_battery_told_nothing_about_the_sealing_digest_demands_only_that_a_case_be_refused` shows the
  same verifier passing when no sealing digest is declared, which is what makes the class
  assertions' contribution measurable rather than assumed.
- **`honest`** must survive the whole battery clean, or the expectations are wrong rather than the
  verifiers.

## What this does not prove

The battery is a statement about verifiers. It is not any of the following, and a citation of the
numbers above should carry this paragraph with it.

- **Not a proof of collision resistance.** Nothing here tests SHA-256. Every case works by changing
  the canonical bytes and expecting the digest to notice; a battery cannot distinguish a strong hash
  from a weak one that happens to have no collisions among the 66,296 mutations the two batteries
  generate.
- **Not a proof that the producing code is correct.** The battery starts from a document its
  verifier accepts and asks whether edits to it are caught. Whether the producer put the right
  values in that document in the first place is a different question, checked by each crate's own
  tests. The differential checks touch the edge of this and no more.
- **Not a security audit.** Digest verification says a document's canonical bytes are the ones its
  digest names. It says nothing about who produced it, whether they were entitled to, whether the
  document reached the verifier over a channel anyone controls, or whether a valid receipt means the
  thing it describes actually happened.
- **Not a proof about the wire form.** Verifiers receive a parsed `serde_json::Value`. The
  duplicate-key family checks that the *resolved* document — last occurrence winning — is rejected;
  it cannot check that duplication was detectable, because by the time a verifier runs, the
  information is gone.
- **Not a Unicode normalisation test.** The canonical encoder applies no normalisation, deliberately,
  because the CPython reference it must agree with byte for byte applies none either. The confusable
  family asserts that no verifier has quietly introduced normalisation or trimming that would erase a
  difference; it does not assert anything about normalisation forms themselves.
- **Not a test of non-finite numbers as numbers.** JSON has no spelling for one and this workspace's
  parser and canonical encoder both refuse to carry one. The boundary family uses the string
  spellings, which is a different thing and is labelled as one.
- **Not a claim that a pinned class is the only defensible one everywhere.** Where two readings of an
  unusable digest are both arguable the library permits both, and the battery pins what these five
  verifiers actually answer. A different verifier making the other choice is a divergence to record,
  not a hole.
- **Not a claim about documents other than the nineteen built here.** A dossier from a different
  research request, or a receipt over a different delivery, has different positions. The library
  exists so that adding a subject is cheap; the numbers belong to the subjects that were run.
- **Not a claim to cover every verifier in the workspace.** Ten document verifiers are reached by
  neither battery, named in `UNCOVERED_DOCUMENT_VERIFIERS` and listed above. The enumeration test
  keeps that list honest as the workspace changes; it does not shrink it.
- **Not a claim that the recorded gaps are acceptable.** A `KnownGap` entry says a refusal was not
  available to the reader and why; an `OPEN_HOLES` entry says a defect reproduces and was not
  fixed. Both are measurements of what is true today, not arguments that it should stay true.

## Running it

```
cargo test -p bioprism-receipts-audit --offline
```

92 tests: 45 for the library's own generators, patch machinery, refinement rules, and self-test
verifiers, 27 for the receipt battery, and 20 for the verifier battery. Both batteries' coverage
numbers are pinned equalities rather than lower bounds — a coverage number that can silently shrink
is not a coverage number, and a generator that stopped producing cases would otherwise turn the
file green by doing nothing. Expect the verifier battery to dominate the wall clock; see the
timings above.
