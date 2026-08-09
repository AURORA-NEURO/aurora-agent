//! BioQL: a typed query language over biological worlds.
//!
//! Implements blueprint 25.21. A query is lexed ([`lexer`]), parsed into an untyped tree ([`ast`],
//! [`parser`]) and checked against a published schema ([`types`], [`check`]). It is never executed:
//! there is no engine, no optimiser and no planner in this crate, and [`check`] documents why.
//!
//! # A query
//!
//! ```
//! use bioprism_biolang::bioql::{compile, BioType, CollectionDecl, QuerySchema};
//! use bioprism_standards::Unit;
//!
//! let schema = QuerySchema::new().with(
//!     CollectionDecl::new("lesions")
//!         .declare("tumor_volume", BioType::quantity(Unit::parse("mm3")?))
//!         .costing(10),
//! );
//!
//! let typed = compile(
//!     r#"select tumor_volume from lesions
//!        where tumor_volume > 12.5 mm3
//!        labels { "phi:deidentified" }
//!        cost limit 100"#,
//!     &schema,
//! )?;
//! assert_eq!(typed.cost_estimate, 20);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # A query that does not compile
//!
//! ```
//! use bioprism_biolang::bioql::{compile, BioType, CollectionDecl, QuerySchema};
//! use bioprism_biolang::error::{QueryError, TypeError};
//! use bioprism_standards::Unit;
//!
//! let schema = QuerySchema::new().with(
//!     CollectionDecl::new("lesions").declare("tumor_volume", BioType::quantity(Unit::parse("mm3")?)),
//! );
//!
//! let refusal = compile(
//!     r#"select tumor_volume from lesions
//!        where tumor_volume > 12.5 cm3
//!        labels {}
//!        cost limit 100"#,
//!     &schema,
//! )
//! .unwrap_err();
//!
//! let QueryError::Type(TypeError::Incomparable { dimension, .. }) = refusal else {
//!     panic!("expected an incomparability");
//! };
//! assert_eq!(dimension, "unit identity");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod ast;
pub mod check;
pub mod lexer;
pub mod parser;
pub mod token;
pub mod types;

pub use ast::{
    AggregateClause, Aggregation, BinaryOp, CollectionRef, CostClause, ExpansionClause,
    ExpansionPolicy, Expr, LabelClause, Literal, Path, Projection, ProvenanceClause, ProvenanceMode,
    Query, ScopeBinding, ScopeClause, ScopeLiteral, TimeClause, UnaryOp,
};
pub use check::{check, compile, estimate_cost, TypedAggregation, TypedQuery, AGGREGATE_FUNCTIONS};
pub use lexer::lex;
pub use parser::parse;
pub use token::{Keyword, Token, TokenKind};
pub use types::{BioType, CollectionDecl, FieldDecl, QuerySchema, SYNTHETIC_CONTIG};
