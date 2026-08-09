//! BioPRISM foundation objects.
//!
//! Blueprint section 24 (BioPRISM Foundation) is seventeen modules of definitions and
//! positioning. A crate that mirrored all seventeen would be a crate of marker types restating
//! prose, and section 24's own warning in 24.02 — that the project "must not simply announce
//! that these mechanisms are better" — applies to its own implementation first.
//!
//! So this crate implements the subset of section 24 that carries a **checkable obligation**:
//! a rule that some input can violate, where the violation is decidable without data this crate
//! does not have. Everything else is named below and left out.
//!
//! # What is implemented
//!
//! | Module | Blueprint | The obligation it enforces |
//! |---|---|---|
//! | [`mod@unit`] | 24.01 | Family, parent world, generated instance, execution trial and scored result are five types; conflation is a compile error. |
//! | [`system`] | 24.01 | A system that cannot report identity, version and declared capabilities is unevaluable, not badly performing. |
//! | [`worldclass`] | 24.03 | No world class licenses a real-world treatment counterfactual on its own; withheld information needs a reveal policy. |
//! | [`stack`] | 24.04 | An object may only be reported at its own level; the eleven lineage edges are legal only between certain levels. |
//! | [`lattice`] | 24.05 | A capability profile has no overall number, and an unmeasured cell is untested rather than low-scoring. |
//! | [`spine`] | 24.06 | Cross-scale transport requires declared, graded edges and yields a certificate naming every one. |
//! | [`contract`] | 24.07 | A claim with no falsifier is not a claim; a refinement may not widen scope or weaken uncertainty. |
//! | [`lens`] | 24.08 | Measurements are comparable only across matching protocol, instrument, batch, build and preprocessing. |
//! | [`worldline`] | 24.09 | Evidence recorded after the decision leaks, even when it describes earlier biology. |
//! | [`ledger`] | 24.10 | Material is affine: a second material fork does not compile, and unrewarded measurement scores below abstention. |
//! | [`mod@reference`] | 24.11 | An expert distribution may not be collapsed to one label; a minority defensible reading is admitted. |
//! | [`maturity`] | 24.12 | A claim states its applicability envelope, and exploratory work is not "established". |
//! | [`causality`] | 24.13 | Prediction is not mechanism, simulation is not real-world effect, association is not treatment benefit. |
//! | [`boundary`] | 24.15 | Research-use-only is a wrapper type, and simulated authority cannot become institutional authority. |
//!
//! # What is deliberately not implemented, and why
//!
//! **24.02, Novelty Boundary and Prior Art** — positioning. Its content is a list of things the
//! project must not claim novelty for and a list of claims requiring empirical proof. Both are
//! obligations on the authors, discharged by running studies, not by a type. A `NoveltyClaim`
//! struct with a `proven: bool` would be decoration: nothing downstream could be prevented from
//! doing anything by it.
//!
//! **24.14, Users, Workflows and Product Surfaces** — a persona table and a list of CLI verbs.
//! The verbs belong to the CLI crate; the personas constrain nothing a program can check.
//!
//! **24.16, Open-Source Category and Adoption Strategy** — licensing posture, contributor units
//! and demonstration ideas. Real obligations, all of them organizational.
//!
//! **24.17, Glossary** — vocabulary. It is honoured by using its terms as the type names in
//! this crate, which is the only enforcement a glossary can have. A `Glossary` map from term to
//! definition would be a copy of the markdown that could silently drift from it.
//!
//! Three implemented modules also stop short of what their blueprint module describes, and say
//! so in their own docs: [`spine`] enumerates the ten translation mismatches but computes no
//! penalty magnitudes, [`mod@reference`] holds the disagreement distribution but implements none of
//! the seven scoring rules, and [`causality`] checks that identification conditions were
//! declared without deciding whether they hold.
//!
//! # Two obligations that are compile errors rather than runtime checks
//!
//! Most of this crate refuses at runtime with a typed error. Two refuse earlier, because the
//! invariant is about identity rather than content:
//!
//! - passing a [`unit::InstanceId`] where a parent [`unit::WorldId`] belongs does not typecheck;
//! - [`ledger::TissueLedger::fork_material`] consumes its receiver, so the real-world allocation
//!   cannot be forked twice.
//!
//! Both are covered by `compile_fail` doctests, which is how a compile error gets a test.

pub mod boundary;
pub mod causality;
pub mod contract;
pub mod error;
pub mod lattice;
pub mod ledger;
pub mod lens;
pub mod maturity;
pub mod reference;
pub mod spine;
pub mod stack;
pub mod system;
pub mod unit;
pub mod worldclass;
pub mod worldline;
