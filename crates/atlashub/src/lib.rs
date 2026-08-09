//! BioAtlas surfaces: the questions a public hub must be able to answer.
//!
//! Implements the code-bearing part of blueprint section 34 (BioAtlas Public Hub and Ecosystem)
//! that the hub and distribution crates do not: 34.03 (BioWorld Registry and World Cards), 34.10
//! (Value-of-Experiment and Active-Evaluation Laboratory), 34.12 (Data Connector and Research
//! Object Registry), 34.17 (Private, Federated and Bring-Your-Own-Data Evaluation) and 34.20
//! (GitHub Actions and Research CI).
//!
//! # Section 34 is mostly a product document, and this crate says which parts
//!
//! Seventeen of section 34's modules were unclaimed when this crate was written. They are not the
//! same kind of thing. The test applied to each was: *is the detailed design a set of predicates
//! over an artifact, or a description of what people see and do?* Five passed. Twelve did not, and
//! their blueprint ids are deliberately **not written anywhere in this crate**, because
//! `tools/coverage.sh` counts any `NN.MM` token found under `crates/` as a position taken, and
//! citing a module in order to move a percentage is the one thing this workspace must not do. They
//! are named here in prose instead:
//!
//! | Module (by title) | Why it is not code here |
//! |---|---|
//! | *Users, Personas and Jobs to be Done* | Its capabilities are seven user intentions and its metrics are adoption rates. Nothing is a predicate over an artifact. |
//! | *Information Architecture and Navigation* | Its capabilities are a list of twelve object names. The one checkable residue — that a card's outbound links all resolve — is enforced by [`card::CardLinks`] under 34.03 rather than claimed separately. |
//! | *Worldline Timeline and State Explorer* | A visibility-cutoff slider and layered tracks. The non-visual half (valid time versus record time, the future-evidence firewall) belongs to `bioprism-world` and to `bioprism-lens`'s leakage module; what is left is a viewport. |
//! | *BioDecision Cell Inference Microscope* | The object it inspects is `bioprism_prism::DecisionCell`, which already exists and already carries the state, the candidate actions and the acceptance criteria. This module is the act of looking at one. |
//! | *Fork Compare and Counterfactual Lab* | `bioprism_prism::matched_fork` is the mechanism, including seed control and paired execution. Factor pickers and interaction plots are the surface over it. |
//! | *Oracle Evidence and Disagreement Explorer* | Oracle planes, reader distributions, adjudication and circularity are `bioprism-oracle`'s. An appeal queue with a turnaround metric is a workflow. |
//! | *BioCapability Atlas* | `bioprism-atlas` is that atlas, and its rule that a bare score is unserializable is the thing worth having. Section 34's version is the rendering, explicitly out of scope. |
//! | *Failure Atlas and First-Divergence Browser* | The taxonomy is `bioprism_atlas::failure` and minimization is `bioprism_prism::minimize`. Aggregating them by mechanism for browsing is presentation. |
//! | *Architecture and Agent Molecule Registry* | The closest near-miss. `bioprism_prism::Architecture` is already the declarative immutable composition, and the one predicate section 34 adds — substitution safety — is `bioprism-sdk`'s rule that a plugin without conformance evidence is not selectable for load-bearing work. Implementing it here would be a second capability declaration. |
//! | *Notebook, IDE, MCP and Agent Integrations* | Seven integration targets. `bioprism-mcp` owns the MCP resource shape and `bioprism-sdk` the client contract; round-trip fidelity is a property of those objects' serde, tested where they live. |
//! | *No-Key Demos and Onboarding* | An MVP acceptance test written as a first-run experience. Genuinely valuable and genuinely not a type. |
//! | *Open Source Community and Star Flywheel* | Scaffold commands, working groups, bounties, a stars-to-active-use ratio. A programme. |
//!
//! # What the five that passed have in common
//!
//! Each names something an artifact must carry, such that an artifact missing it is wrong rather
//! than merely thin:
//!
//! - [`card`] — a world card must say what the world is **not** suitable for, and that set is
//!   derived from the provenance rung rather than written by the author.
//! - [`voe`] — an experiment whose value cannot be computed is refused, not scored zero.
//! - [`connector`] — a connector that declares a lossy mapping with an empty loss ledger is
//!   refused; one with no conformance evidence is not selectable for load-bearing work.
//! - [`federated`] — a federated result carries *less* evidence than a centralised one, must name
//!   what it could not check, and pooling never buys the missing evidence back.
//! - [`ci`] — the checks a result must pass before publication, as functions that return a
//!   three-valued outcome in which *undetermined* is not *pass*.
//!
//! # Boundary: four things this crate deliberately does not build
//!
//! | Not built here | Owned by |
//! |---|---|
//! | a second leaderboard, result card, challenge or review queue | `bioprism-hub` |
//! | a second trust tier, or trust that transits a federation | `bioprism-hubapi` ([`federated`] consumes its [`TrustStanding`](bioprism_hubapi::federation::TrustStanding), it does not mint one) |
//! | a second capability atlas | `bioprism-atlas` |
//! | a second lens or view grammar | `bioprism-lens` |
//!
//! [`hub::BioAtlasCard`](bioprism_hub::BioAtlasCard) and [`card::WorldCard`] are different objects
//! and the collision of names is worth stating plainly: the hub's card describes **a submitted
//! result** and answers *may this score be shown*; this crate's card describes **a world** and
//! answers *what may be asked of it*. A world card carries no score at all.
//!
//! # What is not implemented, and will not be
//!
//! - **No server, no network, no event stream, no query language.** Section 34's API surface is
//!   its own module and is not this crate's; every type here is a value somebody already has.
//! - **No UI, no rendering, no layout, no navigation.** The recurring shape of this crate is that
//!   it models *what a surface must be able to answer* and stops. There is no renderer, and the
//!   absence is why the process/code split above had to be made explicitly rather than by feel.
//! - **No clock.** Staleness is judged against a caller-supplied [`bioprism_hub::Epoch`], the same
//!   way `bioprism-hubapi` judges mirror freshness, and an absent reference epoch produces an
//!   undetermined answer rather than an optimistic one.
//! - **No estimators.** Not for expected information gain, not for meta-analytic pooling weights,
//!   not for figure-reproduction tolerances. Where section 34 names a number without defining how
//!   to get it, the number is taken from the caller and labelled uncalibrated.
//! - **No cryptography.** Site attestations are values whose signature somebody else checked.
//!
//! # Where section 34 assumes a surface it never specifies
//!
//! Recorded here because each was a decision this crate had to make rather than inherit, and a
//! reader should be able to find them without diffing against the blueprint:
//!
//! 1. Every one of the twenty-three modules carries the identical sentence *"Every rendered score
//!    resolves to immutable result objects"* and none says what a resolution failure renders as.
//!    [`card::CardLinks`] makes an unresolvable link a construction error.
//! 2. The shared *failure states* paragraph requires eight states — unavailable, controlled,
//!    stale, under-review, disputed, withdrawn, non-reproducible, not-comparable — and never says
//!    which object carries them. `bioprism_hub::PublicationState` carries them for results;
//!    [`card::WorldHealth`] carries the three the world registry actually distinguishes and refuses
//!    to invent the rest.
//! 3. The shared API object has a `provenance: []` field with no element type. 34.03 is the only
//!    module whose prose says what belongs in it (parent provenance and licence), and even there
//!    the construction rung — the thing `bioprism-worldfactory` found to govern which claims a
//!    world can support at all — is absent. [`card::Ancestry`] puts it back and makes it mandatory.
//! 4. 34.10 asks for "expected information gain" and lists four budget axes with no exchange rate
//!    between them. This is the same gap `bioprism-lab` recorded at 09.02. [`voe`] answers with a
//!    Pareto front, which needs no exchange rate, and a scalar ranking only when the caller
//!    supplies and signs for one.
//! 5. 34.12 wants "connector conformance" as a product metric and never defines a conformance
//!    suite. [`connector::Conformance`] is therefore a record of somebody else's suite, with a
//!    denominator that cannot be zero.
//! 6. 34.17 wants "small-cell protection" with no threshold and "cross-site meta-analysis" with no
//!    pooling method. [`federated::SmallCellPolicy`] takes the threshold from the caller;
//!    [`federated::pool`] returns the union of what was unchecked and explicitly does not compute
//!    a pooled effect estimate.
//! 7. 34.20 wants "figure reproduction" with no notion of tolerance for a floating-point plot.
//!    [`ci::Check::FigureReproduces`] compares digests, so a figure that differs in the last bit
//!    fails, and the check documents that as the deliberate strict reading.
//!
//! # Section 34's boilerplate, measured
//!
//! Method: over the twenty-three module files, count non-blank lines; a line is *unique* if it
//! occurs in exactly one file across the section. Boilerplate is the complement. Two choices move
//! the number and both are reported:
//!
//! | Corpus | Front matter | Unique / total | Boilerplate |
//! |---|---|---|---|
//! | the 23 modules | counted | 348 / 1339 | **74.0%** |
//! | the 23 modules | stripped | 325 / 1178 | **72.4%** |
//! | plus the section index | counted | 359 / 1357 | 73.5% |
//! | plus the section index | stripped | 335 / 1189 | 71.8% |
//!
//! The front-matter sensitivity is 1.6 points, smaller than the 4.3 points the same choice moved
//! section 23, because section 34's shared *body* is proportionally larger — the seven identical
//! front-matter lines are a smaller share of a file that is already 74% shared. Including the
//! section index moves it another 0.5 points and is the likelier source of small disagreements
//! between measurements, since the index is not a module and counting it inflates the unique share.
//!
//! Every file shares the same eight-step canonical user flow, the same seven trust requirements,
//! the same API object and the same MVP acceptance test. What varies is a title, a one-sentence
//! purpose, six to twelve capability bullets and five product metrics. 34.20 is the thinnest at 13
//! unique lines of 57 counting front matter; the richest, *Information Architecture and
//! Navigation*, is rich only because its capability list is twelve proper nouns.

pub mod card;
pub mod ci;
pub mod connector;
pub mod error;
pub mod federated;
pub mod voe;

pub use card::{
    Ancestry, AncestryStep, CardLinks, ClaimKind, Currency, HealthCheck, LatentState,
    ProvenanceRung, ResourceModel, Unsuitability, WorldCard, WorldCardDraft, WorldHealth,
    RESOURCE_TYPE,
};
pub use ci::{
    Check, CheckOutcome, CiReport, Observation, Publishability, ResultUnderReview,
};
pub use connector::{
    select, AuthMode, Conformance, Connector, ConnectorId, Egress, Fetch, Health, Selection, Use,
};
pub use error::{CardError, CiError, ConnectorError, FederatedError, ValueError};
pub use federated::{
    pool, Aggregate, DataOrigin, Evaluation, FederatedResult, Reported, SiteParticipation,
    SiteResult, SmallCellPolicy, Unchecked,
};
pub use voe::{
    pareto_front, rank_with, Budget, Calibration, DeclaredValue, ExchangeRate, ExclusionReason,
    Excluded, Experiment, ExperimentId, Hypothesis, HypothesisId, HypothesisState, ParetoFront,
    Privacy, Ranked, Ranking,
};
