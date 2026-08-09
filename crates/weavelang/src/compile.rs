//! The compiler driver: source text in, WeaveIR out.
//!
//! Blueprint 23.03's pipeline, restricted to the phases this crate implements. The phases it does
//! not implement are named in the crate docs and are not silently skipped here; a caller asking for
//! `compile` gets phases 1 to 4 and nothing that pretends to be 5 to 11.

use crate::diagnostic::{Diagnostic, Span};
use crate::ir::WeaveIr;
use crate::lower::{lower_program, LowerError};
use crate::parser::{parse, ParseError};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompileError {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    Lower(#[from] LowerError),
}

impl Diagnostic for CompileError {
    fn code(&self) -> &'static str {
        match self {
            CompileError::Parse(error) => error.code(),
            CompileError::Lower(error) => error.code(),
        }
    }

    fn span(&self) -> Option<Span> {
        match self {
            CompileError::Parse(error) => error.span(),
            CompileError::Lower(error) => error.span(),
        }
    }
}

/// Parses and lowers one WeaveLang source file.
///
/// The returned IR has its `program_id` assigned, so it is ready to be hashed, stored or compared.
pub fn compile(source: &str) -> Result<WeaveIr, CompileError> {
    let program = parse(source)?;
    Ok(lower_program(&program, source)?)
}
