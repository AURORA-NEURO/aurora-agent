# Bringing your own domain

How to carry the FIBER pipeline to a decision question it was not born knowing, using a
**domain pack** — a versioned document that declares the question, its scope vocabulary, and the
rule oracle that judges it. The three worked packs under `fixtures/domains/` (trade surveillance,
legal privilege review, software supply-chain release review) compile end to end in
`crates/domain/tests/end_to_end.rs`; every statement below is checked there or in
`crates/domain/tests/{pack_documents,rule_language}.rs`.

Project modeling is the worked large-scale example of a domain pack: `bioprism-project` scans an
entire software project into a fiber-world under the sealed adapter contract and judges it with
the `project-release-readiness` pack — see [PROJECT_MODELING](PROJECT_MODELING.md).

## Where the biology actually lived

The pipeline itself is domain-neutral. Facts, typed factors, scopes, protected closure, slicing,
the temporal cut and the certificate never mention biology — `crates/fiber/src/compile.rs` runs
the same passes whatever the world is about. The domain content lived in exactly three places:

1. **The oracle.** `crates/fiber/src/oracle.rs` ships one wired-in judge,
   `deterministic_split_integrity_v1`, which recognises `subject_aliases`, `split_assignment`,
   `site_assignment`, `label_source_time`, `training_decision_time`, `preprocess_fit_scope` and
   nothing else. A world with a genuinely different decision compiled to `valid` with an empty
   witness list and read as clean rather than as unjudged — the defect
   `crates/worldgen/src/spec.rs` records on `Skeleton` as the reason it carries no `Custom`
   variant.
2. **The scope defaults.** `crates/scope/src/class.rs` classifies dimension names into the seven
   canonical classes (identity, region, specimen, time, coordinate, ontology, policy), and its
   default table covers the neuro-oncology vocabulary the OncoWorld packs use. A fact scoped by
   `venue` or `custodian` was `unclassified` and could not be proven closed over.
3. **Worldgen content.** `crates/worldgen/src/spec.rs` generates worlds whose decisive structure
   is the split-integrity question, by construction.

Two of the three are now data. `bioprism_fiber::compile_with_oracle` accepts any
`DecisionOracle`; `bioprism_scope::DimensionRegistry::from_json` extends the dimension table from
a document; and `crates/domain` (`bioprism-domain`) binds both into a pack. Worldgen's content
remains biological — a non-reference domain writes its worlds directly, as the fixtures do.

## The domain pack: `bioprism-domain/0.1`

One pack declares one decision question. Parsing is strict — an undeclared key is refused, and
nothing is consulted lazily, so a malformed pack fails at the boundary rather than mid-compile
(`crates/domain/src/pack.rs`).

| field | required | meaning |
|---|---|---|
| `schema_version` | yes | exactly `"bioprism-domain/0.1"` |
| `name` | yes | a non-empty lowercase-ascii slug (`a-z`, `0-9`, `-`) |
| `description` | yes | what the pack decides, one sentence |
| `goal` | no | the goal a query in this domain should declare |
| `protected_tags` | no | the tags a query in this domain should place in protected closure |
| `scope_dimensions` | no | a `bioprism-scope-dimensions/0.1` document, validated on load |
| `oracle` | yes | the rule oracle (next section) |

`goal` and `protected_tags` are **advisory**: the pack cannot inject them into a query, because
the certificate binds the query's bytes by hash, and mutating a query would change what it
certifies. The query stays the sole author of its own contract; the pack records what a
well-formed query in the domain should say.

A pack deliberately carries no worlds and no queries. Worlds are evidence, queries are decisions,
and both bind to the pack only at compile time, through `compile_with_oracle`. The certificate
then names the pack's oracle kind, which is the only coupling a verifier needs.

There is also no oracle selection on the query wire: no `fiber-query` version carries an oracle
field, so choosing a pack is a caller decision at the API or CLI boundary, recorded in the
verdict's `oracle_kind`.

## The rule language

The oracle document is `{"kind": "rule/...", "require": [...], "checks": [...]}`
(`crates/domain/src/rules.rs`). The kind must begin with `"rule/"` so any consumer of a
certificate can tell a declared rule oracle from a native one by its verdict alone.

Checks are **violation detectors**: a fired check is evidence against the world, exactly as a
leakage witness is. Each check is `{"name", "description", "when"}`, where `when` is a predicate
over the compiled value map. Check names must be unique within an oracle, and an oracle with no
checks is refused — it would return `valid` for every world.

### Predicate kinds and their wire forms

Every predicate is an object with a `kind` and exactly the fields that kind declares; an
undeclared field is refused before a missing one is reported, so a misspelled field does not send
the author after the wrong problem.

| kind | fields | true when |
|---|---|---|
| `exists` | `variable` | the variable was delivered, whatever its value (total) |
| `missing` | `variable` | the variable was not delivered (total) |
| `equals` | `variable`, `value` | the value equals the given JSON value |
| `not_equals` | `variable`, `value` | the value differs from the given JSON value |
| `number_at_least` | `variable`, `minimum` | numeric value ≥ minimum |
| `number_below` | `variable`, `maximum` | numeric value < maximum |
| `string_before` | `variable`, `than` | string value < `than`, lexicographic |
| `string_after` | `variable`, `than` | string value > `than`, lexicographic |
| `contains` | `variable`, `value` | the array-valued variable contains the element |
| `has_key` | `variable`, `key` | the object-valued variable carries the key |
| `nonempty` | `variable` | the array- or object-valued variable has ≥ 1 element |
| `count_at_least` | `variable`, `minimum` | the array- or object-valued variable has ≥ minimum elements |
| `all_of` | `predicates` | every limb is true (non-empty list) |
| `any_of` | `predicates` | at least one limb is true (non-empty list) |
| `not` | `predicate` | the inner predicate is false |

String comparison is lexicographic on the raw strings, matching the reference oracle's flagged
temporal behaviour, and is refused for non-strings rather than coerced. For zero-offset RFC 3339
`...Z` timestamps of equal precision this agrees with instant ordering; for mixed offsets it does
not, and that limitation carries over deliberately.

### Three-valued evaluation

A predicate over an absent or wrongly-typed variable is **unevaluable**, never false: "the check
did not run" and "the check passed" must not share a representation. Only `exists` and `missing`
are total, because absence is the very thing they ask about. Composition follows strong
three-valued logic: an `all_of` with one false limb is determinately false whatever the other
limbs would have said, an `any_of` with one true limb is determinately true, and otherwise one
unevaluable limb makes the whole predicate unevaluable.

### Verdict semantics

- any fired check → **`invalid`**, one `domain_check` witness per fired check;
- no fired check but a check that could not run → **`underdetermined`**, one witness per unrun
  check naming the variable and reason that stopped it;
- everything ran and nothing fired → **`valid`**.

`invalid` outranks `underdetermined`: a proven violation stands even when another check is blind.
An `invalid` verdict still reports its unrun checks, after the violations, so the gap is never
hidden behind the finding.

A fired-check witness carries the rule's name, the bindings it read (canonically rendered, so a
human can re-run the check by hand), and the declared description — a checkable object, never a
score.

### `require`

`require` lists variables the compiled region must deliver before any check runs. A required
variable the compiler could not deliver abstains the whole verdict up front, with one
`domain_check` witness per missing variable whose check is named `required_evidence`. Use it for the variables whose absence makes
the whole question unanswerable, rather than making every check individually unevaluable.

## The dimension document: `bioprism-scope-dimensions/0.1`

`{"schema_version": "bioprism-scope-dimensions/0.1", "dimensions": {name: class}}`, where every
class is one of the seven canonical names. Loaded through
`bioprism_scope::DimensionRegistry::{from_json, extend_from_json}`, it extends the default table
so a domain's own dimensions (`venue`, `custodian`, `pipeline`, …) classify instead of drawing an
`unclassified_scope_dimension` diagnostic on every fact.

Two refusals are deliberate:

- **A canonical dimension cannot be reclassified.** Protected closure rules are written against
  the class, so silently moving `time` or `subject` to another class would move evidence out of
  closure. The registry refuses the reclassification rather than accepting it.
- **`"unclassified"` is not a parseable class.** It is the absence of a classification; a
  document declaring a dimension unclassified would assert the very state the registry already
  reports for every name it has never seen.

## Worked walkthrough: trade surveillance

`fixtures/domains/trade-surveillance/` decides one wash-trade review. The pack
(`domain.json`) declares four scope dimensions (`account`→identity, `venue`→region,
`session`→time, `rulebook`→ontology) and a `rule/trade-surveillance-v1` oracle with three checks:

```json
{ "name": "self_cross",
  "description": "a buy and a sell in the window resolve to the same beneficial account",
  "when": { "kind": "nonempty", "variable": "self_match_conflicts" } }
```

plus `late_reporting` (`equals` on `reporting_window_closed` = `false`) and
`cancel_ratio_excessive` (`number_at_least` on `cancel_ratio`, minimum 0.95), with
`self_match_conflicts` and `reporting_window_closed` required.

The world (`world.json`) is one trading session on venue X-ALPHA: four decisive facts feeding a
`factor.wash_trade_review` whose output is `wash_trade_status`, and six exploratory
market-colour facts feeding an unrelated aggregation. The decisive fact is precomputed:

```json
{ "provides": "self_match_conflicts",
  "value": [ { "account": "ACC-9", "buy_order": "ORD-1201", "sell_order": "ORD-1288" } ] }
```

The query (`query.json`) targets `wash_trade_status` with protected tags
`["identity", "time", "protected"]` and a decision time after the session close.

The actual compile outcome, pinned in `end_to_end.rs`: the oracle status is **`invalid`** with
exactly one witness —

- `check`: `self_cross`
- `observed`: `self_match_conflicts` =
  `[{"account":"ACC-9","buy_order":"ORD-1201","sell_order":"ORD-1288"}]`, canonically rendered
- `detail`: "a buy and a sell in the window resolve to the same beneficial account"

— and the compiled region is the decision's region, not the corpus: 4 facts selected, the 6
market-colour facts omitted, protected closure fully delivered (`dropped_protected` empty).

### What the other two fixtures demonstrate

**`privilege-review` — temporal withholding becomes abstention.** The disclosure log exists in
the world, but its collection event's availability time postdates the query's decision time, so
the temporal cut withholds `third_party_disclosures` — a variable the oracle requires. The
verdict is **`underdetermined`**, with a `domain_check` witness (check `required_evidence`) naming the variable, and the
certificate's `inaccessible_selected_before_cut` names the withheld fact, so the gap is traceable
to the cut rather than to the corpus. Before `compile_with_oracle`, this world compiled to
`valid` with an empty witness list — a wrong answer, not a missing one.

**`supply-chain` — valid, with full closure.** Every artifact signed, provenance complete, SBOM
inside the freeze window: every check runs, nothing fires, the verdict is **`valid`**, and the
three protected facts are all in protected closure. `valid` here means "every declared check ran
and found nothing", not "nobody checked".

## What the rule language deliberately cannot express

**No relational predicates.** The reference identity check joins two maps key-by-key
(`subject_aliases` against `split_assignment`); the rule language cannot express that join. A
domain that needs one has two honest options:

1. **Precompute the conflict as its own variable.** The trade-surveillance fixture does this:
   `self_match_conflicts` is the *output* of the join, produced upstream and shipped as a fact,
   and the rule merely asks whether it is non-empty.
2. **Implement `bioprism_fiber::DecisionOracle` natively.** The trait is two methods; a Rust
   oracle can compute whatever it likes, provided it stays a pure function of the value map and
   returns witnesses rather than scores.

Also inherited from the reference oracle's contract: **checks see the selected value map only**.
A rule cannot consult scopes, events, tags or omitted facts — the compiler's passes have already
turned those into the value map and the certificate.

## How parity is preserved

`compile()` is untouched: it is now a one-line delegation to `compile_with_oracle` with the
split-integrity oracle, and `crates/fiber/tests/oracle_injection.rs` asserts that the default
compile and an explicitly injected reference oracle produce byte-identical certificates and
sections on the golden world. The CPython parity contract therefore still pins the same bytes.

A pack changes bytes only through its verdict. Every pass before and after the oracle is
identical; only the verdict — and the certificate bytes that carry it — depends on the oracle.
`crates/domain/tests/end_to_end.rs` additionally pins that a domain compile is deterministic byte
for byte across repeated runs, on both certificate profiles.
