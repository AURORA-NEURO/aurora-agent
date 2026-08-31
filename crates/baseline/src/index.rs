//! Work shared by the whole panel, computed at most once per comparison.
//!
//! Four of the strategies [`crate::compare::default_panel`] enters derive the *same* intermediate
//! structure from the world before they diverge. [`crate::incidence::KHopIncidence`] at four depths
//! and [`crate::incidence::ConnectedComponent`] each walked their own copy of the incidence graph;
//! the two [`crate::lexical::LexicalTopK`] budgets and the two [`crate::embedding::EmbeddingTopK`]
//! budgets each tokenised every fact from scratch, and that tokenisation canonically serialises
//! every fact value. On the default panel that was five incidence builds and four full-corpus
//! tokenisations for one comparison, and the 36-cell sweep paid for 144 of each.
//!
//! None of it depends on which strategy asked. The graph is a function of the world, the corpus is
//! a function of the world, and the two retrieval rankings are functions of the world and the one
//! query the comparison is about — so a panel that runs one world against one query can compute
//! each of them once and lend it out.
//!
//! # Why this cannot move a number
//!
//! Every cached value is the output of a pure function of data the index holds by shared reference
//! and therefore cannot mutate. Sharing changes how many times each function runs, never what it
//! returns, so a strategy reading a cached corpus reads exactly what building its own would have
//! produced. The obligation is discharged by measurement rather than by this paragraph:
//! `tests/sweep_grid.rs` reproduces the committed 36-cell table and `tests/equal_engineering.rs`
//! the committed comparison rows.
//!
//! # Why the cells are lazy
//!
//! A panel need not contain any retrieval strategy, and [`crate::compare::compare`] is public API
//! reached with a caller's own panel of one. Populating every cell up front would make the narrow
//! caller pay for a corpus nothing reads — the cost this module exists to remove, moved rather than
//! removed. Each cell is therefore filled on first request and never again.

use crate::incidence::Adjacency;
use bioprism_fiber::Query;
use bioprism_world::World;
use std::cell::OnceCell;

/// One comparison's shared intermediates: one world, one query, every panel member.
///
/// Not [`Sync`], because [`OnceCell`] is not. A panel evaluated across threads would need one index
/// per thread, which costs exactly what this type saves and is why nothing here tries to.
pub struct PanelIndex<'a> {
    world: &'a World,
    query: &'a Query,
    incidence: OnceCell<Adjacency>,
    documents: OnceCell<Vec<Vec<String>>>,
    lexical_ranking: OnceCell<Vec<(usize, f64)>>,
    embeddings: OnceCell<Vec<Vec<f64>>>,
    embedding_ranking: OnceCell<Vec<(usize, f64)>>,
}

impl<'a> PanelIndex<'a> {
    /// An index over one world and one query, with nothing computed yet.
    pub fn new(world: &'a World, query: &'a Query) -> Self {
        PanelIndex {
            world,
            query,
            incidence: OnceCell::new(),
            documents: OnceCell::new(),
            lexical_ranking: OnceCell::new(),
            embeddings: OnceCell::new(),
            embedding_ranking: OnceCell::new(),
        }
    }

    pub fn world(&self) -> &'a World {
        self.world
    }

    pub fn query(&self) -> &'a Query {
        self.query
    }

    /// The undirected factor/variable incidence graph, shared by every depth of the walk.
    pub(crate) fn incidence(&self) -> &Adjacency {
        self.incidence
            .get_or_init(|| crate::incidence::build_incidence(self.world))
    }

    /// Every fact's searchable tokens, in world order, shared by both retrieval families.
    pub(crate) fn documents(&self) -> &[Vec<String>] {
        self.documents
            .get_or_init(|| crate::lexical::documents(self.world))
    }

    /// Facts scoring above zero under BM25, best first — shared by every `k`, which differs only
    /// in how far down this list it reads.
    pub(crate) fn lexical_ranking(&self) -> &[(usize, f64)] {
        self.lexical_ranking
            .get_or_init(|| crate::lexical::rank(self.world, self.query, self.documents()))
    }

    /// Every fact's embedding vector, in world order.
    pub(crate) fn embeddings(&self) -> &[Vec<f64>] {
        self.embeddings
            .get_or_init(|| crate::embedding::embed_documents(self.documents()))
    }

    /// Facts scoring above zero under cosine similarity, best first — shared by every `k`.
    pub(crate) fn embedding_ranking(&self) -> &[(usize, f64)] {
        self.embedding_ranking
            .get_or_init(|| crate::embedding::rank(self.world, self.query, self.embeddings()))
    }
}
