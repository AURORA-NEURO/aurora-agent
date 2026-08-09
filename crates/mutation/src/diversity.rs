//! Effective diversity.
//!
//! The executive summary is blunt about this: *"PRISM must never imply that one million
//! automatically paraphrased questions equal one million meaningful benchmarks."* Gate 3 makes it
//! operational — "report effective diversity rather than just count."
//!
//! So the headline number here is not the instance count. It is the number of **independent
//! equivalence classes**: distinct combinations of parent, mutation family and oracle signature.
//! Two instances that share all three test the same thing twice, however different their bytes.
//!
//! This is a *counted* quantity, not a modelled one. It makes no assumption about correlation
//! structure and cannot be tuned; a generator that inflates instance count without adding classes
//! shows an inflation ratio that rises and an effective count that does not.

use crate::lineage::Family;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diversity {
    /// Validated instances. The number a naive report would headline.
    pub instances: usize,
    /// Audited parents they descend from.
    pub parents: usize,
    /// Distinct mutation families applied.
    pub families: usize,
    /// Distinct oracle signatures — what is actually being tested.
    pub signatures: usize,
    /// Distinct (parent, family, signature) classes. The honest denominator.
    pub equivalence_classes: usize,
    /// instances / equivalence_classes. 1.0 means every instance is independent.
    pub inflation_ratio: f64,
    pub caveat: String,
}

impl Diversity {
    /// The sentence a report should lead with.
    pub fn headline(&self) -> String {
        format!(
            "{} validated instances from {} audited parent(s) across {} mutation famil(ies), \
             providing {} independent equivalence classes (inflation ×{:.2}). Instance count is \
             not benchmark count.",
            self.instances,
            self.parents,
            self.families,
            self.equivalence_classes,
            self.inflation_ratio
        )
    }

    /// Whether the family adds enough independent information to be worth publishing.
    ///
    /// A deliberately conservative gate: a family whose instances collapse into two or fewer
    /// classes is a robustness check, not a benchmark, and should be labelled as one.
    pub fn is_publishable(&self) -> bool {
        self.equivalence_classes >= 3
    }
}

pub fn measure(families: &[Family]) -> Diversity {
    let mut parents = BTreeSet::new();
    let mut mutation_families = BTreeSet::new();
    let mut signatures = BTreeSet::new();
    let mut classes = BTreeSet::new();
    let mut instances = 0usize;

    for family in families {
        parents.insert(family.parent_sha256.clone());
        for instance in &family.accepted {
            instances += 1;
            mutation_families.insert(instance.family.clone());
            signatures.insert(instance.signature());
            classes.insert(format!(
                "{}|{}|{}",
                family.parent_sha256,
                instance.family,
                instance.signature()
            ));
        }
    }

    let equivalence_classes = classes.len();
    Diversity {
        instances,
        parents: parents.len(),
        families: mutation_families.len(),
        signatures: signatures.len(),
        equivalence_classes,
        inflation_ratio: if equivalence_classes == 0 {
            0.0
        } else {
            instances as f64 / equivalence_classes as f64
        },
        caveat: "Equivalence classes are counted as distinct (parent, mutation family, oracle \
                 signature) triples. This measures independent diagnostic information, not \
                 difficulty or realism, and makes no claim about correlation beyond those three \
                 axes."
            .into(),
    }
}
