//! Lexical top-k retrieval.
//!
//! Stands in for the embedding-similarity baseline of 43.41. It is a BM25-style scorer over
//! tokens drawn from each fact's id, provided variable, tags and serialised value, scored against
//! tokens from the query's targets and protected tags.
//!
//! It is **not** a neural retriever, and the comparison report says so. Substituting a weak
//! stand-in and declaring victory is the strawman-baseline failure 43.38 exists to prevent, so
//! this is made as strong as a lexical method can be — inverse document frequency, term-frequency
//! saturation, length normalisation — and its nature is reported alongside its score. On a world
//! whose decisive facts are literally tagged `identity`, `split` and `site`, a lexical retriever
//! that reads tags is a genuinely hard baseline to beat on recall.

use crate::strategy::{ContextStrategy, Selection};
use bioprism_fiber::Query;
use bioprism_ids::to_canonical_string;
use bioprism_world::World;
use std::collections::{BTreeMap, BTreeSet};

const K1: f64 = 1.2;
const B: f64 = 0.75;

pub struct LexicalTopK {
    pub k: usize,
}

/// Shared with [`crate::embedding`], so the two retrieval baselines provably score the same
/// searchable text and differ only in how they score it.
pub(crate) fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

/// The searchable text of one fact: id, provided variable, tags and serialised value.
///
/// Also shared with [`crate::embedding`] for the reason above — an embedding baseline that read
/// different fields would make any divergence in results unattributable to the scoring method.
pub(crate) fn fact_tokens(world: &World, position: usize) -> Vec<String> {
    let fact = &world.facts[position];
    let mut text = String::new();
    text.push_str(fact.id.as_str());
    text.push(' ');
    text.push_str(fact.provides.as_str());
    for tag in &fact.tags {
        text.push(' ');
        text.push_str(tag);
    }
    text.push(' ');
    if let Ok(value) = to_canonical_string(&fact.value) {
        text.push_str(&value);
    }
    tokenize(&text)
}

/// The query-side tokens both retrieval baselines score against: targets and protected tags.
pub(crate) fn query_tokens(query: &Query) -> BTreeSet<String> {
    let mut tokens: Vec<String> = Vec::new();
    for target in &query.targets {
        tokens.extend(tokenize(target.as_str()));
    }
    for tag in &query.protected_tags {
        tokens.extend(tokenize(tag));
    }
    tokens.into_iter().collect()
}

impl ContextStrategy for LexicalTopK {
    fn name(&self) -> String {
        format!("lexical-top-{}", self.k)
    }

    fn method(&self) -> String {
        format!(
            "BM25 (k1={K1}, b={B}) over fact id, provided variable, tags and serialised value; \
             top {} by score, ties broken by fact id. A lexical proxy for embedding retrieval, \
             not a neural model.",
            self.k
        )
    }

    fn select(&self, world: &World, query: &Query) -> Selection {
        let documents: Vec<Vec<String>> = (0..world.facts.len())
            .map(|position| fact_tokens(world, position))
            .collect();

        let total = documents.len() as f64;
        let average_length = if documents.is_empty() {
            1.0
        } else {
            documents.iter().map(|d| d.len()).sum::<usize>() as f64 / total
        };

        let mut document_frequency: BTreeMap<&str, usize> = BTreeMap::new();
        for document in &documents {
            for token in document.iter().collect::<BTreeSet<_>>() {
                *document_frequency.entry(token.as_str()).or_default() += 1;
            }
        }

        let query_tokens = query_tokens(query);

        let mut scored: Vec<(usize, f64)> = documents
            .iter()
            .enumerate()
            .map(|(position, document)| {
                let length = document.len() as f64;
                let mut score = 0.0;
                for token in &query_tokens {
                    let frequency =
                        document.iter().filter(|t| t.as_str() == token.as_str()).count() as f64;
                    if frequency == 0.0 {
                        continue;
                    }
                    let df = *document_frequency.get(token.as_str()).unwrap_or(&0) as f64;
                    let idf = ((total - df + 0.5) / (df + 0.5) + 1.0).ln();
                    let saturated = (frequency * (K1 + 1.0))
                        / (frequency + K1 * (1.0 - B + B * length / average_length));
                    score += idf * saturated;
                }
                (position, score)
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
            "{} facts scored above zero; lexical proxy, not an embedding model",
            scored.len()
        ))
    }
}
