//! Policy as an access-control input (43.33), and its collision with mandatory closure (43.13).
//!
//! Blueprint 43.33 requires policy, consent, purpose, residency and export constraints to be
//! "enforced during compilation and execution—not as a final response filter". Until this pass
//! existed the compiler parsed `policy` off the query into [`Query::policy`] and no stage read it:
//! 40.25's `policy conflict` failure had no firing condition, its second invariant — that ranking
//! cannot bypass policy or closure — had nothing to enforce, and
//! [`bioprism_section::InfluenceClass::InaccessibleByPolicy`] had no constructor anywhere in the
//! workspace.
//!
//! # The model, and why it is this small
//!
//! `fiber-world/0.1` carries no policy rules, no labels and no lattice; `fiber-query/0.1` carries
//! a role string and a flat list of clause names. Exactly two places in those formats can carry
//! access control without inventing fields:
//!
//! * the world's `data_policy` variable — the clause set the corpus was released under, the
//!   *governing* policy. The reference world already ships it, tagged `policy` and `protected`;
//! * a fact's `policy` scope dimension — already a registered
//!   [`bioprism_scope::ScopeClass::Policy`] dimension — naming the clauses a reader must have
//!   accepted before that fact may be released.
//!
//! A clause is an *obligation*, and accepting the obligation is what unlocks the evidence. The
//! query's `policy` list is therefore read as the set of obligations the caller accepts, not as a
//! description of the data. Three consequences follow, and each is the point of a rule below: a
//! caller that accepts fewer obligations sees less; a caller that accepts an obligation the world
//! never offered is refused, because a grant the corpus did not issue cannot be honoured and
//! ignoring it would leave the caller believing it holds access it does not; and a fact the caller
//! cannot unlock is *withheld and named*, never dropped into the structural-irrelevance bucket
//! where "provably cannot matter" already lives.
//!
//! # The stricter reading, where the syntax is ambiguous
//!
//! `ScopeValue::OneOf` means a disjunction in the scope algebra: a claim valid across any of
//! several regions. On the `policy` dimension this pass reads the same syntax as a *conjunction*
//! of clauses the reader must hold, and so deliberately does not use
//! [`bioprism_scope::ScopeKey::refines`] to decide access. Two reasons. Refinement runs the wrong
//! way for authorisation — a caller that accepts *more* obligations must see more evidence, while
//! a narrower scope key refines *fewer* coarse keys — and where a syntax admits two readings,
//! 43.33's invariant that "policy requirements can only become stricter" settles which one to
//! take. A fact released under two restrictions requires both to be honoured, not either.
//!
//! # What is deliberately not implemented
//!
//! `bioprism-policy` models 43.33 properly: a seven-axis label lattice fibred over scopes, purpose
//! binding, consent withdrawal, residency and transport, versioned declassification with leakage
//! analysis, and a policy trace for the certificate. None of it is reachable from here. A
//! `bioprism_policy::Request` binds a principal, a purpose, a channel and a destination, and
//! `fiber-query/0.1` carries none of the four; a `PolicyLattice` needs rules attached at scopes and
//! `fiber-world/0.1` declares none. Wiring the crate in would mean fabricating those fields, which
//! is the "quietly substitute a default" failure [`Query::missing_contract_fields`] exists to
//! prevent. This pass implements the fragment the wire formats can state and nothing beyond it.
//!
//! Also absent, and each is a real gap rather than a simplification:
//!
//! * **Role is still unread.** `fiber-query/0.1` carries `role`, `bioprism-scope` registers `role`
//!   and `visibility` as policy-class dimensions, and no pass consults either. The audit named
//!   `policy`; `role` is the same hole one field over.
//! * **No consent, purpose or residency check.** All three are registered scope dimensions and the
//!   query can express none of them, so a fact scoped by consent is read as unconstrained here.
//! * **No redaction and no declassification.** A withheld fact is withheld whole. There is no
//!   partial release, no small-cell suppression and no approved operator that could lower a
//!   clause requirement.
//! * **No information-flow order over outputs.** This pass decides what may be *read*. Whether the
//!   compiled section may then move to a given recipient is 43.33's `⊑_P` and is not modelled.
//! * **No obligation discharge.** Accepting a clause is a claim the caller makes about itself.
//!   Nothing here can tell an honoured obligation from an ignored one.

use crate::qir::Query;
use bioprism_world::{Fact, WorldSource};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// The world variable carrying the governing data policy.
pub const DATA_POLICY_VARIABLE: &str = "data_policy";

/// The scope dimension on which a fact names the clauses a reader must hold.
pub const POLICY_SCOPE_DIMENSION: &str = "policy";

/// The refinement a caller can act on when policy withheld evidence.
///
/// Unlike the temporal frontier this is not a move the compiler can make on the caller's behalf:
/// advancing a time cut is a parameter change, obtaining a grant is a change to what the caller
/// is entitled to. The action string says so rather than implying a retry would help.
pub const POLICY_REFINEMENT_ACTION: &str = "declare_the_required_policy_clauses_or_obtain_a_grant";

/// Typed policy failures.
///
/// Carried by `FiberError::Policy` exactly as `WorldError` is carried by `FiberError::World`: the
/// taxonomy of a distinct concern stays in its own enum, and the compiler's error type gains one
/// variant rather than one per policy rule.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum PolicyViolation {
    /// 40.25's named `policy conflict` failure.
    ///
    /// The query accepts an obligation the corpus was not released under. This is a refusal and
    /// not a warning because the alternative readings are both worse: honouring the clause would
    /// grant access on an authority no source issued, and dropping it silently would leave a
    /// caller believing a clause it declared was in force when nothing checked it.
    #[error("policy conflict: the query declares clause(s) {clauses:?} that the world's data policy does not grant; the corpus was released under {governing:?}")]
    Conflict {
        clauses: Vec<String>,
        governing: Vec<String>,
    },

    /// The governing policy exists but does not parse as a clause list.
    ///
    /// Treating it as absent would turn a malformed authority into an unchecked one, so the
    /// compile refuses instead.
    #[error("fact {fact_id:?} provides {variable:?} but its value is not a list of non-empty clause names: {found}")]
    MalformedDataPolicy {
        fact_id: String,
        variable: &'static str,
        found: String,
    },

    /// A fact binds the policy dimension to something that names no clause.
    ///
    /// An empty or non-nominal requirement is not an absent one. Reading it as "requires nothing"
    /// would release evidence on the strength of a typo.
    #[error("fact {fact_id:?} binds the {dimension:?} scope dimension to {found}, which names no policy clause")]
    UninterpretableRequirement {
        fact_id: String,
        dimension: &'static str,
        found: String,
    },

    /// Policy withheld a member of the mandatory closure.
    ///
    /// 43.13 computes the closure *before* any relevance step so that a strategy cannot be
    /// credited for guessing correctly from evidence it never had. A compile that proceeds without
    /// a protected fact has produced exactly that state, and the v0.1 certificate has no field in
    /// which to confess it, so the compile refuses. See [`screen`] for the full argument.
    #[error("policy withholds protected fact(s) {fact_ids:?} for want of clause(s) {clauses:?}; the mandatory closure of 43.13 cannot be delivered, so no section is compiled")]
    ProtectedClosureWithheld {
        fact_ids: Vec<String>,
        clauses: Vec<String>,
    },
}

/// The clauses governing a compile: what the corpus offers and what the caller accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyEnvelope {
    governing: Option<BTreeSet<String>>,
    in_force: BTreeSet<String>,
}

impl PolicyEnvelope {
    /// Reads the governing policy and checks the caller's declared clauses against it.
    ///
    /// Runs before the protected closure, and is the only part of this module that does. It needs
    /// no evidence beyond the single `data_policy` lookup, so gating here means a conflicting
    /// query is refused before any closure, slice or materialisation happens — 43.33's
    /// "enforced during compilation" taken at its word. The *screen* cannot move up with it, for
    /// the reason given at [`screen`].
    pub fn resolve<S: WorldSource + ?Sized>(
        source: &S,
        query: &Query,
    ) -> Result<Self, PolicyViolation> {
        let governing = match source.fact_providing(DATA_POLICY_VARIABLE) {
            Some(fact) => Some(governing_clauses(&fact)?),
            None => None,
        };
        let in_force: BTreeSet<String> = query.policy.iter().cloned().collect();

        if let Some(declared) = &governing {
            let ungranted: Vec<String> = in_force.difference(declared).cloned().collect();
            if !ungranted.is_empty() {
                return Err(PolicyViolation::Conflict {
                    clauses: ungranted,
                    governing: declared.iter().cloned().collect(),
                });
            }
        }

        Ok(PolicyEnvelope {
            governing,
            in_force,
        })
    }

    /// The clause set the corpus declares it was released under, when it declares one.
    pub fn governing(&self) -> Option<&BTreeSet<String>> {
        self.governing.as_ref()
    }

    /// The obligations the caller accepted, and therefore the grants this compile holds.
    pub fn in_force(&self) -> &BTreeSet<String> {
        &self.in_force
    }

    /// Whether a declared data policy corroborated the caller's clauses.
    ///
    /// False means the world states no release regime. The clauses still take effect — they are
    /// the caller's own undertaking — but nothing checked them, and "nobody checked" is not
    /// "checked and fine". [`PolicyOutcome::unverified`] reports them.
    pub fn is_corroborated(&self) -> bool {
        self.governing.is_some()
    }

    /// Governing clauses the caller did not accept, and so cannot unlock evidence with.
    pub fn unaccepted(&self) -> Vec<String> {
        match &self.governing {
            Some(declared) => declared.difference(&self.in_force).cloned().collect(),
            None => Vec::new(),
        }
    }

    /// Clauses in force that no declared data policy corroborates.
    pub fn unverified(&self) -> Vec<String> {
        match self.governing {
            Some(_) => Vec::new(),
            None => self.in_force.iter().cloned().collect(),
        }
    }

    /// Clauses `fact` requires that this compile does not hold.
    pub fn missing_for(&self, fact: &Fact) -> Result<BTreeSet<String>, PolicyViolation> {
        Ok(requirement(fact)?
            .difference(&self.in_force)
            .cloned()
            .collect())
    }
}

/// Which candidates policy withheld, and for want of which clauses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyScreen {
    withheld: BTreeMap<String, Vec<String>>,
    requirements_seen: usize,
}

impl PolicyScreen {
    /// Withheld fact ids, in certificate order.
    pub fn withheld_ids(&self) -> Vec<String> {
        self.withheld.keys().cloned().collect()
    }

    /// Every clause that withheld something, deduplicated.
    pub fn missing_clauses(&self) -> Vec<String> {
        self.withheld
            .values()
            .flatten()
            .cloned()
            .collect::<BTreeSet<String>>()
            .into_iter()
            .collect()
    }

    /// The clauses `fact_id` was withheld for.
    pub fn missing_for(&self, fact_id: &str) -> Option<&[String]> {
        self.withheld.get(fact_id).map(Vec::as_slice)
    }

    /// Candidates that declared a policy requirement at all, whether or not it was satisfied.
    ///
    /// Zero here means the pass ran and found nothing to enforce, which is a different statement
    /// from the pass not running. Both appear on the receipt.
    pub fn requirements_seen(&self) -> usize {
        self.requirements_seen
    }

    pub fn is_empty(&self) -> bool {
        self.withheld.is_empty()
    }

    pub fn len(&self) -> usize {
        self.withheld.len()
    }

    /// The obligation text for a withheld fact, identical in both implementations.
    ///
    /// The Decision Section is hashed into the certificate, so this string is wire format: the
    /// CPython reference builds the same sentence from the same sorted clause list.
    pub fn obligation_detail(fact_id: &str, missing: &[String]) -> String {
        format!(
            "{fact_id} requires undeclared policy clauses: {}",
            missing.join(", ")
        )
    }
}

/// Screens the compiled candidate region against the clauses in force.
///
/// # Why this runs after the closure and not before it
///
/// 43.33 would rather this ran first: evidence a principal may not read should never enter
/// scoring range, and `bioprism-policy` is built so a selector can consult the lattice per
/// candidate for exactly that reason. Running first is nonetheless wrong *here*, and the reason is
/// the whole point of the pass.
///
/// Screening the corpus before [`crate::closure::protected_closure`] would compute the mandatory
/// closure over an already-filtered world. A protected fact the caller cannot unlock would simply
/// never appear, the closure would report itself complete, `dropped_protected` would be empty, and
/// the compile would deliver a section that looks whole while missing evidence 39.05 classes as
/// non-compressible. That is the silent failure AGENTS.md forbids — a right answer reached from a
/// basis the strategy never had. Running after the closure makes the collision *observable*, and
/// an observable collision can be refused.
///
/// The cost of that ordering is stated rather than hidden: to read a candidate's policy scope the
/// compiler must materialise the candidate, so a withheld fact's value is loaded into the
/// compiler's memory before it is refused. It never reaches the section, the oracle's value map or
/// the certificate. Avoiding even that would need a `WorldSource` accessor returning a fact's
/// scope without its value, which is a change to a trait this crate does not own.
///
/// # Why a withheld protected fact is a refusal and not a receipt
///
/// The temporal cut may drop a protected fact and the compile still proceeds, so the asymmetry
/// needs a reason. Three, in fact. The cut's exclusions are named on the reference certificate in
/// `omissions.inaccessible_selected_before_cut`, so that compile is honest on the wire, while
/// `fiber-context-certificate/0.1` has no field for a policy exclusion. The cut has a refinement
/// the compiler can offer — advance it, or ask retrospectively — while a policy exclusion is
/// remedied only outside the compile, by changing what the caller is entitled to or by changing
/// which tags the query calls protected. And 43.13 forbids trimming protected evidence to fit,
/// which is why [`crate::error::FiberError::BudgetExceeded`] is already a hard failure; policy
/// trimming the same set for a different reason is the same violation.
///
/// A caller that wants the smaller section can ask for it honestly, by not declaring the tag
/// protected. That is a change to the decision contract, which is what it should look like.
pub fn screen(
    envelope: &PolicyEnvelope,
    candidates: &BTreeMap<String, Fact>,
    protected: &BTreeSet<String>,
) -> Result<PolicyScreen, PolicyViolation> {
    let mut withheld: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut requirements_seen = 0usize;

    for (id, fact) in candidates {
        let required = requirement(fact)?;
        if required.is_empty() {
            continue;
        }
        requirements_seen += 1;
        let missing: Vec<String> = required.difference(envelope.in_force()).cloned().collect();
        if !missing.is_empty() {
            withheld.insert(id.clone(), missing);
        }
    }

    let blocked: Vec<String> = withheld
        .keys()
        .filter(|id| protected.contains(*id))
        .cloned()
        .collect();
    if !blocked.is_empty() {
        let clauses: BTreeSet<String> = blocked
            .iter()
            .flat_map(|id| withheld[id].iter().cloned())
            .collect();
        return Err(PolicyViolation::ProtectedClosureWithheld {
            fact_ids: blocked,
            clauses: clauses.into_iter().collect(),
        });
    }

    Ok(PolicyScreen {
        withheld,
        requirements_seen,
    })
}

/// What the policy pass decided, for the compile trace.
///
/// `fiber-context-certificate/0.1` has no policy block, so this is where a reader learns that the
/// pass ran, what it held, and what it could not verify. Recording it on the trace rather than
/// inventing a certificate field keeps the wire format — and every shipped digest — unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyOutcome {
    /// Clauses the corpus declares it was released under; `None` when it declares no data policy.
    pub governing: Option<Vec<String>>,
    /// Obligations the caller accepted.
    pub in_force: Vec<String>,
    /// Governing clauses the caller declined, and so cannot unlock evidence with.
    pub unaccepted: Vec<String>,
    /// Clauses in force that no declared data policy corroborates.
    pub unverified: Vec<String>,
    /// Candidate facts withheld by policy, in certificate order.
    pub withheld: Vec<String>,
    /// Candidates that declared a policy requirement at all.
    pub requirements_seen: usize,
}

impl PolicyOutcome {
    pub fn new(envelope: &PolicyEnvelope, screen: &PolicyScreen) -> Self {
        PolicyOutcome {
            governing: envelope
                .governing()
                .map(|clauses| clauses.iter().cloned().collect()),
            in_force: envelope.in_force().iter().cloned().collect(),
            unaccepted: envelope.unaccepted(),
            unverified: envelope.unverified(),
            withheld: screen.withheld_ids(),
            requirements_seen: screen.requirements_seen(),
        }
    }

    /// Whether policy removed nothing from the compiled region.
    pub fn released_everything_requested(&self) -> bool {
        self.withheld.is_empty()
    }
}

/// The clauses a fact requires a reader to hold.
///
/// An absent binding is an absent requirement: `fiber-world/0.1` has no way to say "unclassified",
/// so a world that declares nothing about a fact says nothing about it. That is a weaker default
/// than 43.33's "unknown policy defaults to no export", and it is the wire format's limit rather
/// than a choice — defaulting to refusal here would withhold every fact in every world that ships,
/// including the reference one.
///
/// Read from the fact's original document rather than from its parsed [`bioprism_scope::ScopeKey`],
/// for a parity reason and a semantic one. [`bioprism_scope::ScopeValue`] canonicalises `3` and
/// `true` into the exact strings `"3"` and `"true"`, so a scope key cannot tell a clause named
/// `"3"` from a number that was never a clause name at all, and the CPython reference — which has
/// no such coercion — would refuse where this accepted. And [`ScopeValue::OneOf`] is a disjunction
/// in the scope algebra, which is not what a list of required clauses means here.
fn requirement(fact: &Fact) -> Result<BTreeSet<String>, PolicyViolation> {
    let Some(bound) = fact
        .raw()
        .get("scope")
        .and_then(|scope| scope.get(POLICY_SCOPE_DIMENSION))
    else {
        return Ok(BTreeSet::new());
    };
    let uninterpretable = || PolicyViolation::UninterpretableRequirement {
        fact_id: fact.id.as_str().to_string(),
        dimension: POLICY_SCOPE_DIMENSION,
        found: bound.to_string(),
    };
    let clauses = clause_set(bound).map_err(|_| uninterpretable())?;
    if clauses.is_empty() {
        return Err(uninterpretable());
    }
    Ok(clauses)
}

/// The governing clause set carried by the world's `data_policy` fact.
///
/// An empty list is legal and means the corpus grants nothing, which is a different statement
/// from declaring no data policy at all: the first refuses every clause a query declares, the
/// second has no authority to refuse with. Only a value that is not a clause list is an error.
fn governing_clauses(fact: &Fact) -> Result<BTreeSet<String>, PolicyViolation> {
    clause_set(&fact.value).map_err(|found| PolicyViolation::MalformedDataPolicy {
        fact_id: fact.id.as_str().to_string(),
        variable: DATA_POLICY_VARIABLE,
        found,
    })
}

/// A clause name, or a list of them. Anything else is rejected with the offending JSON.
fn clause_set(value: &Value) -> Result<BTreeSet<String>, String> {
    match value {
        Value::String(clause) if !clause.is_empty() => {
            Ok(std::iter::once(clause.clone()).collect())
        }
        Value::Array(items) => {
            let mut clauses = BTreeSet::new();
            for item in items {
                match item.as_str() {
                    Some(clause) if !clause.is_empty() => {
                        clauses.insert(clause.to_string());
                    }
                    _ => return Err(item.to_string()),
                }
            }
            Ok(clauses)
        }
        other => Err(other.to_string()),
    }
}
