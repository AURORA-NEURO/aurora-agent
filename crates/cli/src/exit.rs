//! Exit codes.
//!
//! Blueprint 40.13 requires errors to map to documented exit codes and 40.36 requires a typed
//! error taxonomy with a retry classification. The distinction that matters to a CI gate is
//! between *this input is wrong* (never retry) and *this run could not complete* (may retry), so
//! the codes are grouped that way rather than by which module raised them.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// The command completed and its assertion held.
    Ok = 0,
    /// The command completed but the thing it checked did not hold: an invalid split, a
    /// certificate that does not verify. Distinct from an error, and scriptable.
    AssertionFailed = 1,
    /// Bad invocation. Never retryable.
    Usage = 2,
    /// Input did not satisfy its schema or could not be parsed. Never retryable.
    InvalidInput = 3,
    /// Compilation could not produce a sound result within the declared contract, for example a
    /// budget smaller than the protected closure. Never retryable without changing the query.
    CompileFailed = 4,
    /// A file could not be read or written. Possibly retryable.
    Io = 5,
}

impl ExitCode {
    pub fn as_i32(self) -> i32 {
        self as i32
    }

    pub fn is_retryable(self) -> bool {
        matches!(self, ExitCode::Io)
    }

    pub fn slug(self) -> &'static str {
        match self {
            ExitCode::Ok => "ok",
            ExitCode::AssertionFailed => "assertion_failed",
            ExitCode::Usage => "usage",
            ExitCode::InvalidInput => "invalid_input",
            ExitCode::CompileFailed => "compile_failed",
            ExitCode::Io => "io",
        }
    }
}

impl fmt::Display for ExitCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.as_i32(), self.slug())
    }
}

/// A failure carrying the exit code it should produce.
#[derive(Debug)]
pub struct CliError {
    pub code: ExitCode,
    pub message: String,
    pub subject: Option<String>,
}

impl CliError {
    pub fn new(code: ExitCode, message: impl Into<String>) -> Self {
        CliError {
            code,
            message: message.into(),
            subject: None,
        }
    }

    pub fn about(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    pub fn io(path: &std::path::Path, source: std::io::Error) -> Self {
        CliError::new(ExitCode::Io, source.to_string()).about(path.display().to_string())
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        CliError::new(ExitCode::InvalidInput, message)
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "ok": false,
            "error": {
                "code": self.code.as_i32(),
                "kind": self.code.slug(),
                "retryable": self.code.is_retryable(),
                "message": self.message,
                "subject": self.subject,
            }
        })
    }
}

pub type CliResult<T> = Result<T, CliError>;
