//! Argument parsing.
//!
//! Hand-rolled rather than derived. The 40.13 contract wants golden help output, a stable
//! exit-code matrix and a `--json` mode with no stray bytes on stdout; owning the parser keeps
//! all three exactly specified and keeps the binary dependency-free.

use crate::exit::{CliError, CliResult, ExitCode};
use std::path::PathBuf;

pub const HELP: &str = "\
bioprism — query-compiled inference for executable biology

Compiles a typed decision query against a FIBER world into the smallest decision-sufficient
Decision Section, together with a Context Certificate stating exactly what was omitted and
whether the omission could have changed the decision.

USAGE
  bioprism [--json] <command> <subcommand> [options]

COMMANDS
  world validate    --world <path>
                    Report structural diagnostics. Exit 1 if any are errors.
  world show        --world <path>
                    Summarise facts, factors, events, tags and factor kinds.
  world index       --world <path> --store <dir> [--dry-run]
                    Build a content-addressed index so compile cost tracks the compiled region
                    rather than the corpus. Afterwards --world accepts the store directory.
  world generate    --family reference-like|discriminating [--distractors <n>]
                    [--world-out <path>] [--query-out <path>] [--dry-run]
                    Generate a synthetic structural family (43.39) and its matching query.

  context explain   --world <path> --query <path>
                    Show the compile plan: passes run, passes deferred, selection ratios and
                    omissions grouped by influence class. Writes nothing.
  context compile   --world <path> --query <path>
                    [--certificate-out <path>] [--section-out <path>]
                    [--profile reference|extended] [--dry-run] [--fail-on-invalid]
                    Compile and optionally write artifacts.
  context verify    --certificate <path>
                    Recompute the certificate digest. Exit 1 if it does not verify.
  context compare   --world <path> --query <path> [--markdown]
                    Run the equal-engineering baseline panel (full-context, graph k-hop,
                    hypergraph component, query-graph, lexical top-k, FIBER) and report which
                    strategies preserve the reference verdict and at what cost.

  prism fork        --world <path> --query <path> [--bundle-out <path>] [--minimize]
                    Run every architecture from one frozen Decision Cell and report which
                    context policy explains the difference. Exit 1 if any architecture fails.
  prism minimize    --world <path>
                    Reduce the world to a 1-minimal set of facts that preserves the oracle
                    signature, then re-verify the reduction.

  mutate family     --world <path> [--out-dir <dir>]
                    Apply the standard metamorphic suite, validate every postcondition against
                    the oracle, deduplicate by content, and report effective diversity.

GLOBAL OPTIONS
  --json            Emit exactly one JSON document on stdout and nothing else.
  -h, --help        Show this help.
  -V, --version     Show the version.

EXIT CODES
  0  ok                the command completed and its assertion held
  1  assertion_failed  completed, but the checked property does not hold
  2  usage             bad invocation (never retryable)
  3  invalid_input     input failed its schema (never retryable)
  4  compile_failed    no sound result within the declared contract (never retryable)
  5  io                a file could not be read or written (may be retryable)

Research and developer infrastructure. Not a medical device: it does not diagnose, recommend
treatment, or triage care.
";

#[derive(Debug, PartialEq, Eq)]
pub enum Parsed {
    Help,
    Version,
    Run(Invocation),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Invocation {
    pub json: bool,
    pub command: Command,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    WorldValidate { world: PathBuf },
    WorldShow { world: PathBuf },
    ContextExplain { world: PathBuf, query: PathBuf },
    ContextCompile(CompileOptions),
    ContextVerify { certificate: PathBuf },
    ContextCompare { world: PathBuf, query: PathBuf, markdown: bool },
    WorldGenerate(GenerateOptions),
    WorldIndex { world: PathBuf, store: PathBuf, dry_run: bool },
    PrismFork { world: PathBuf, query: PathBuf, bundle_out: Option<PathBuf>, minimize: bool },
    PrismMinimize { world: PathBuf },
    MutateFamily { world: PathBuf, out_dir: Option<PathBuf> },
}

#[derive(Debug, PartialEq, Eq)]
pub struct GenerateOptions {
    pub family: Family,
    pub distractors: usize,
    pub world_out: Option<PathBuf>,
    pub query_out: Option<PathBuf>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    ReferenceLike,
    Discriminating,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CompileOptions {
    pub world: PathBuf,
    pub query: PathBuf,
    pub certificate_out: Option<PathBuf>,
    pub section_out: Option<PathBuf>,
    pub profile: Profile,
    pub dry_run: bool,
    pub fail_on_invalid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Reference,
    Extended,
}

pub fn parse<I: IntoIterator<Item = String>>(arguments: I) -> CliResult<Parsed> {
    let mut tokens: Vec<String> = arguments.into_iter().collect();

    if tokens.iter().any(|t| t == "-h" || t == "--help") {
        return Ok(Parsed::Help);
    }
    if tokens.iter().any(|t| t == "-V" || t == "--version") {
        return Ok(Parsed::Version);
    }

    let json = extract_flag(&mut tokens, "--json");
    let mut cursor = tokens.into_iter();

    let group = cursor.next().ok_or_else(|| usage("no command given"))?;
    let subcommand = cursor
        .next()
        .ok_or_else(|| usage(format!("{group:?} needs a subcommand")))?;
    let mut options = Options::collect(cursor)?;

    let command = match (group.as_str(), subcommand.as_str()) {
        ("world", "validate") => Command::WorldValidate {
            world: options.take_path("--world")?,
        },
        ("world", "show") => Command::WorldShow {
            world: options.take_path("--world")?,
        },
        ("world", "index") => Command::WorldIndex {
            world: options.take_path("--world")?,
            store: options.take_path("--store")?,
            dry_run: options.take_switch("--dry-run"),
        },
        ("world", "generate") => Command::WorldGenerate(GenerateOptions {
            family: match options.take_optional("--family").as_deref() {
                None | Some("discriminating") => Family::Discriminating,
                Some("reference-like") => Family::ReferenceLike,
                Some(other) => {
                    return Err(usage(format!(
                        "--family must be reference-like or discriminating, got {other:?}"
                    )))
                }
            },
            distractors: match options.take_optional("--distractors") {
                None => 750,
                Some(text) => text.parse().map_err(|_| {
                    usage(format!("--distractors must be a number, got {text:?}"))
                })?,
            },
            world_out: options.take_optional_path("--world-out"),
            query_out: options.take_optional_path("--query-out"),
            dry_run: options.take_switch("--dry-run"),
        }),
        ("context", "explain") => Command::ContextExplain {
            world: options.take_path("--world")?,
            query: options.take_path("--query")?,
        },
        ("context", "compile") => Command::ContextCompile(CompileOptions {
            world: options.take_path("--world")?,
            query: options.take_path("--query")?,
            certificate_out: options.take_optional_path("--certificate-out"),
            section_out: options.take_optional_path("--section-out"),
            profile: match options.take_optional("--profile").as_deref() {
                None | Some("reference") => Profile::Reference,
                Some("extended") => Profile::Extended,
                Some(other) => {
                    return Err(usage(format!(
                        "--profile must be reference or extended, got {other:?}"
                    )))
                }
            },
            dry_run: options.take_switch("--dry-run"),
            fail_on_invalid: options.take_switch("--fail-on-invalid"),
        }),
        ("context", "verify") => Command::ContextVerify {
            certificate: options.take_path("--certificate")?,
        },
        ("prism", "fork") => Command::PrismFork {
            world: options.take_path("--world")?,
            query: options.take_path("--query")?,
            bundle_out: options.take_optional_path("--bundle-out"),
            minimize: options.take_switch("--minimize"),
        },
        ("mutate", "family") => Command::MutateFamily {
            world: options.take_path("--world")?,
            out_dir: options.take_optional_path("--out-dir"),
        },
        ("prism", "minimize") => Command::PrismMinimize {
            world: options.take_path("--world")?,
        },
        ("context", "compare") => Command::ContextCompare {
            world: options.take_path("--world")?,
            query: options.take_path("--query")?,
            markdown: options.take_switch("--markdown"),
        },
        _ => return Err(usage(format!("unknown command {group:?} {subcommand:?}"))),
    };

    options.reject_leftovers()?;
    Ok(Parsed::Run(Invocation { json, command }))
}

fn extract_flag(tokens: &mut Vec<String>, flag: &str) -> bool {
    let present = tokens.iter().any(|t| t == flag);
    tokens.retain(|t| t != flag);
    present
}

fn usage(message: impl Into<String>) -> CliError {
    CliError::new(ExitCode::Usage, message)
}

struct Options {
    values: Vec<(String, Option<String>)>,
}

impl Options {
    fn collect<I: Iterator<Item = String>>(mut cursor: I) -> CliResult<Self> {
        let mut values = Vec::new();
        while let Some(token) = cursor.next() {
            if !token.starts_with("--") {
                return Err(usage(format!("unexpected positional argument {token:?}")));
            }
            if let Some((name, inline)) = token.split_once('=') {
                values.push((name.to_string(), Some(inline.to_string())));
                continue;
            }
            match cursor.next() {
                Some(next) if !next.starts_with("--") => values.push((token, Some(next))),
                Some(next) => {
                    values.push((token, None));
                    values.push((next, None));
                }
                None => values.push((token, None)),
            }
        }
        Ok(Options { values })
    }

    fn take_optional(&mut self, name: &str) -> Option<String> {
        let position = self.values.iter().position(|(key, _)| key == name)?;
        let (_, value) = self.values.remove(position);
        value
    }

    fn take_optional_path(&mut self, name: &str) -> Option<PathBuf> {
        self.take_optional(name).map(PathBuf::from)
    }

    fn take_path(&mut self, name: &str) -> CliResult<PathBuf> {
        self.take_optional_path(name)
            .ok_or_else(|| usage(format!("{name} is required and takes a path")))
    }

    fn take_switch(&mut self, name: &str) -> bool {
        match self.values.iter().position(|(key, _)| key == name) {
            Some(position) => {
                self.values.remove(position);
                true
            }
            None => false,
        }
    }

    fn reject_leftovers(&self) -> CliResult<()> {
        match self.values.first() {
            None => Ok(()),
            Some((name, _)) => Err(usage(format!("unrecognised option {name:?}"))),
        }
    }
}
