//! Embedding top-k retrieval.
//!
//! The embedding-retriever baseline `docs/FINDINGS.md` named as missing. Each fact's searchable
//! text — the same id, provided variable, tags and serialised value BM25 reads, via
//! [`crate::lexical::fact_tokens`] — is embedded as hashed character-3-gram counts in a fixed
//! 512-dimensional space (FNV-1a into buckets), and facts are ranked by cosine similarity against
//! the embedded query.
//!
//! # What this is, and what it is not
//!
//! It is a **fixed-basis lexical embedding, not a learned or neural model**, and every report row
//! it produces says so. The basis is decided by a hash function, not by training, and that
//! difference could change both of this baseline's headline behaviours:
//!
//! - a trained encoder places semantically related but lexically distinct terms near each other,
//!   so it could recover decisive evidence that shares *no characters* with the query, which this
//!   basis structurally cannot;
//! - conversely this basis scores camouflaged distractor tags (`identity_summary`) close to the
//!   protected vocabulary (`identity`) *because* they share trigrams — a trained encoder might or
//!   might not, and nothing measured here settles which.
//!
//! Substituting this for a neural retriever and declaring the comparison closed would be the
//! strawman-baseline failure 43.38 exists to prevent. What this baseline does establish is how far
//! character-level similarity alone gets, under exactly equal data access, with the difference
//! from a learned model stated rather than implied.

use crate::lexical::{fact_tokens, query_tokens};
use crate::strategy::{ContextStrategy, Selection};
use bioprism_fiber::Query;
use bioprism_world::World;
use std::collections::BTreeSet;

/// Fixed embedding width. A power of two, so bucketing is a mask rather than a modulo.
const DIMENSION: usize = 512;

/// Character n-gram order. Trigrams are the standard fastText-style subword unit.
const NGRAM: usize = 3;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Embeds a token sequence as bucketed character-trigram counts.
///
/// Each token is wrapped in `#` boundary markers before the trigram slide, the fastText
/// convention: it lets a whole short token (`#id#`) be its own feature and keeps prefixes
/// distinguishable from infixes. `#` cannot collide with token content because
/// [`crate::lexical::tokenize`] emits ASCII alphanumerics only.
fn embed(tokens: impl IntoIterator<Item = String>) -> Vec<f64> {
    let mut vector = vec![0.0_f64; DIMENSION];
    for token in tokens {
        let wrapped = format!("#{token}#");
        let bytes = wrapped.as_bytes();
        for gram in bytes.windows(NGRAM) {
            let bucket = (fnv1a(gram) as usize) & (DIMENSION - 1);
            vector[bucket] += 1.0;
        }
    }
    vector
}

fn cosine(a: &[f64], b: &[f64]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

pub struct EmbeddingTopK {
    pub k: usize,
}

impl ContextStrategy for EmbeddingTopK {
    fn name(&self) -> String {
        format!("embedding-top-{}", self.k)
    }

    fn method(&self) -> String {
        format!(
            "hashed character-{NGRAM}-gram embedding (FNV-1a into {DIMENSION} fixed buckets) over \
             fact id, provided variable, tags and serialised value; cosine similarity against the \
             query's targets and protected tags; top {} by score, ties broken by fact id. A \
             fixed-basis lexical embedding, not a learned or neural model.",
            self.k
        )
    }

    fn select(&self, world: &World, query: &Query) -> Selection {
        let query_vector = embed(query_tokens(query));

        let mut scored: Vec<(usize, f64)> = (0..world.facts.len())
            .map(|position| {
                let fact_vector = embed(fact_tokens(world, position));
                (position, cosine(&fact_vector, &query_vector))
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| world.facts[a.0].id.as_str().cmp(world.facts[b.0].id.as_str()))
        });

        let facts: BTreeSet<String> = scored
            .iter()
            .take(self.k)
            .map(|(position, _)| world.facts[*position].id.as_str().to_string())
            .collect();

        Selection::new(facts).noting(format!(
            "{} facts scored above zero; fixed-basis lexical embedding, not a learned model",
            scored.len()
        ))
    }
}
