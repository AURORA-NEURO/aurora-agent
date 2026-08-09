//! The physical-storage boundary.
//!
//! Blueprint 43.16: "logical semantics are independent of physical backend." This trait is where
//! that is enforced. The compiler is written against `WorldSource` and never against a concrete
//! representation, so an eager in-memory [`World`] and a lazily-indexed on-disk store must produce
//! byte-identical Decision Sections and Certificates.
//!
//! Every method here is deliberately *narrow*: the compiler may ask for a fact by id, a fact by
//! the variable it provides, the factors producing a variable, and aggregate counts. It may not
//! ask to iterate the corpus. That restriction is what allows a backend to make compile cost
//! proportional to the compiled region rather than to the world — the whole point of 43.34.

use crate::event::CausalEvent;
use crate::fact::Fact;
use crate::factor::Factor;
use crate::world::World;
use bioprism_ids::ContentHash;
use std::collections::BTreeSet;

pub trait WorldSource {
    fn world_id(&self) -> &str;

    /// Hash of the canonical world document.
    ///
    /// Precomputed by indexed backends: recomputing it would require reading the whole corpus,
    /// which is exactly what this trait exists to avoid.
    fn world_digest(&self) -> ContentHash;

    fn total_facts(&self) -> usize;
    fn total_factors(&self) -> usize;

    /// How many facts in the whole world carry `tag`.
    fn count_with_tag(&self, tag: &str) -> usize;

    /// Ids of every fact carrying at least one of `tags`. Backs the protected closure, which must
    /// be computed before any relevance step (43.13) and so cannot be derived from the slice.
    fn fact_ids_with_any_tag(&self, tags: &BTreeSet<String>) -> BTreeSet<String>;

    fn fact(&self, id: &str) -> Option<Fact>;

    /// The fact providing `variable`. Where several do, the last in document order wins, matching
    /// the reference runtime's dict-comprehension semantics.
    fn fact_providing(&self, variable: &str) -> Option<Fact>;

    fn factor(&self, id: &str) -> Option<Factor>;

    /// Ids of factors that output `variable`, in document order.
    fn producer_ids(&self, variable: &str) -> Vec<String>;

    /// The causal event structure. Small by construction — events describe releases, not records.
    fn events(&self) -> Vec<CausalEvent>;
}

impl WorldSource for World {
    fn world_id(&self) -> &str {
        self.world_id.as_str()
    }

    fn world_digest(&self) -> ContentHash {
        self.content_hash()
    }

    fn total_facts(&self) -> usize {
        self.facts.len()
    }

    fn total_factors(&self) -> usize {
        self.factors.len()
    }

    fn count_with_tag(&self, tag: &str) -> usize {
        self.facts.iter().filter(|fact| fact.has_tag(tag)).count()
    }

    fn fact_ids_with_any_tag(&self, tags: &BTreeSet<String>) -> BTreeSet<String> {
        self.facts
            .iter()
            .filter(|fact| fact.has_any_tag(tags))
            .map(|fact| fact.id.as_str().to_string())
            .collect()
    }

    fn fact(&self, id: &str) -> Option<Fact> {
        World::fact(self, id).cloned()
    }

    fn fact_providing(&self, variable: &str) -> Option<Fact> {
        World::fact_providing(self, variable).cloned()
    }

    fn factor(&self, id: &str) -> Option<Factor> {
        World::factor(self, id).cloned()
    }

    fn producer_ids(&self, variable: &str) -> Vec<String> {
        World::producers_of(self, variable)
            .map(|factor| factor.id.as_str().to_string())
            .collect()
    }

    fn events(&self) -> Vec<CausalEvent> {
        self.events.clone()
    }
}
