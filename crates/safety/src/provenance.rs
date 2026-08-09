//! Content provenance and the authority a segment is allowed to carry.
//!
//! Implements blueprint 13.12 (prompt injection, memory and context attacks) and 13.13 (prompt
//! injection and untrusted content).
//!
//! # There is no detector here, and that is the design
//!
//! 13.13's "Detection" paragraph proposes heuristics and classifiers that "annotate suspected
//! injection", and then says the thing that matters: "Detection alone is not a security boundary."
//! A function in this crate that scanned text for `ignore previous instructions` would be worse
//! than useless — it would produce a number that looks like a safety property, be trivially evaded
//! by rephrasing, and give a caller a reason to relax the one control that does work. So this
//! module contains no pattern list, no classifier, no score, and no `is_injection` predicate.
//!
//! What it models instead is *where content came from and what position it ended up in*. An
//! injection succeeds when content whose provenance is untrusted reaches a position where it is
//! followed as an instruction. That is a structural fact about the assembly, it is decidable, and
//! [`ContextAssembly::injection_paths`] returns it as a **path** — the derivation chain from the
//! untrusted origin to the sink — never as a probability.
//!
//! # Two rules, both in the type system's reach
//!
//! 1. **A segment's authority is capped by its provenance.** [`Authority::ceiling_for`] is total
//!    over [`Provenance`], retrieved content caps at [`Authority::Data`] (13.13: "Retrieved content
//!    defaults to data, not policy"), and [`ContextAssembly::add`] refuses a segment that asks for
//!    more with [`SafetyError::AuthorityElevation`].
//! 2. **Derivation cannot raise authority.** A summary of a web page is a web page. A segment that
//!    names a parent inherits `min(parent, own ceiling)`, and asking for more is
//!    [`SafetyError::AuthorityLaundering`]. This closes the laundering route where untrusted text
//!    is paraphrased into a system preamble and arrives clean.
//!
//! # What is deliberately not implemented
//!
//! * **No tokenizer, no renderer, no parser.** 13.13's "Isolation" section asks for untrusted
//!   content to be rendered in a sandbox with active content stripped. Nothing here parses HTML,
//!   executes nothing, and strips nothing.
//! * **No effect authorisation.** 13.12's tool gating — "high-risk effects require structured
//!   intent and policy authorization" — is `crates/runtime`'s 05.08 effect and permission system.
//!   [`Sink::ToolArgument`] models only the provenance question: *which segment did this argument
//!   come from*. Whether the tool may run at all is not decided here.
//! * **No memory store.** [`MemoryWrite`] records that a write declared a source and a scope
//!   (13.12: "Writes require source and scope"). It stores nothing and quarantines nothing.
//! * **No Unicode or encoding analysis.** 13.13 lists "Unicode/encoding" and "visual injection" as
//!   benchmark categories. Those are content inspection, which this module does not do.
//!
//! # Where the blueprint is silent
//!
//! 13.13 gives a four-level precedence order and never says what happens to a segment with no
//! declared provenance. This module has no answer either, so it has no representation for one:
//! [`Segment`] requires a [`Provenance`], and the caller has to decide before it gets here.

use crate::error::SafetyError;
use bioprism_scope::ScopeKey;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Where a context segment came from. 13.12's authority classes, as a closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// The platform's own system prompt.
    System,
    /// The operator deploying the architecture.
    Developer,
    /// The human whose task this is.
    User,
    /// The benchmark task statement.
    BenchmarkTask,
    /// A policy the architecture declared and an operator approved.
    ArchitecturePolicy,
    /// Output of a tool call.
    ToolResult,
    /// Retrieved documents, web pages, search results.
    RetrievedContent,
    /// Something an earlier turn wrote to memory.
    Memory,
    /// A message from another agent in a multi-agent run.
    PeerAgentMessage,
    /// Metadata fields on an artifact: titles, descriptions, EXIF, pack cards.
    DocumentMetadata,
    /// Execution logs and traces replayed into context.
    TraceReplay,
}

impl Provenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Provenance::System => "system",
            Provenance::Developer => "developer",
            Provenance::User => "user",
            Provenance::BenchmarkTask => "benchmark_task",
            Provenance::ArchitecturePolicy => "architecture_policy",
            Provenance::ToolResult => "tool_result",
            Provenance::RetrievedContent => "retrieved_content",
            Provenance::Memory => "memory",
            Provenance::PeerAgentMessage => "peer_agent_message",
            Provenance::DocumentMetadata => "document_metadata",
            Provenance::TraceReplay => "trace_replay",
        }
    }

    /// True when this provenance caps at [`Authority::Data`].
    ///
    /// "Untrusted" here is a statement about *authority*, not about whether the content is
    /// malicious. A tool result from a perfectly honest tool is untrusted in this sense, because
    /// the platform did not author it.
    pub fn is_untrusted(self) -> bool {
        Authority::ceiling_for(self) == Authority::Data
    }
}

impl fmt::Display for Provenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 13.13's instruction precedence, lowest first so that `Ord` is the precedence order.
///
/// "Models may reason about lower-level instructions but cannot elevate them" — reasoning about is
/// modelled by a segment being present at [`Position::Data`]; elevation is what
/// [`ContextAssembly::add`] refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Authority {
    /// Content. The default for everything the platform did not author.
    Data,
    /// An approved architecture policy.
    ArchitecturePolicy,
    /// The explicit user or benchmark task.
    TaskInstruction,
    /// System and platform policy. Nothing overrides it.
    PlatformPolicy,
}

impl Authority {
    /// The highest authority a segment of this provenance may ever be assembled at.
    pub fn ceiling_for(provenance: Provenance) -> Authority {
        match provenance {
            Provenance::System | Provenance::Developer => Authority::PlatformPolicy,
            Provenance::User | Provenance::BenchmarkTask => Authority::TaskInstruction,
            Provenance::ArchitecturePolicy => Authority::ArchitecturePolicy,
            Provenance::ToolResult
            | Provenance::RetrievedContent
            | Provenance::Memory
            | Provenance::PeerAgentMessage
            | Provenance::DocumentMetadata
            | Provenance::TraceReplay => Authority::Data,
        }
    }

    /// Whether an instruction at `self` may override one at `other`.
    pub fn may_override(self, other: Authority) -> bool {
        self > other
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Authority::Data => "data",
            Authority::ArchitecturePolicy => "architecture_policy",
            Authority::TaskInstruction => "task_instruction",
            Authority::PlatformPolicy => "platform_policy",
        }
    }
}

impl fmt::Display for Authority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether a segment sits somewhere its text will be followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Position {
    /// Quoted, delimited, presented as material to reason about.
    Data,
    /// The system preamble, the task statement, the tool-selection prompt: anywhere the assembly
    /// intends the text to be obeyed.
    Instruction,
}

impl Position {
    pub fn as_str(self) -> &'static str {
        match self {
            Position::Data => "data",
            Position::Instruction => "instruction",
        }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One piece of assembled context.
///
/// `may_contain_instructions` is a *declaration by the assembler*, not an inspection of the text.
/// It defaults to `true` for untrusted provenance, because assuming otherwise is exactly the
/// assumption injection exploits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub id: String,
    pub provenance: Provenance,
    pub authority: Authority,
    pub position: Position,
    /// The segment this one was summarised, quoted, extracted or translated from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from: Option<String>,
    pub may_contain_instructions: bool,
}

impl Segment {
    /// A segment at its provenance's ceiling, in data position.
    pub fn new(id: impl Into<String>, provenance: Provenance) -> Self {
        Segment {
            id: id.into(),
            provenance,
            authority: Authority::ceiling_for(provenance),
            position: Position::Data,
            derived_from: None,
            may_contain_instructions: provenance.is_untrusted(),
        }
    }

    pub fn at(mut self, authority: Authority) -> Self {
        self.authority = authority;
        self
    }

    pub fn positioned(mut self, position: Position) -> Self {
        self.position = position;
        self
    }

    pub fn derived_from(mut self, parent: impl Into<String>) -> Self {
        self.derived_from = Some(parent.into());
        self
    }
}

/// Where untrusted content landed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "sink", rename_all = "snake_case")]
pub enum Sink {
    /// It is in an instruction-following position in the assembled context.
    InstructionPosition,
    /// It supplied an argument to a tool call.
    ToolArgument { tool: String, argument: String },
    /// It was written to memory, where a later turn will retrieve it.
    MemoryWrite { key: String },
}

impl Sink {
    pub fn as_str(&self) -> &'static str {
        match self {
            Sink::InstructionPosition => "instruction_position",
            Sink::ToolArgument { .. } => "tool_argument",
            Sink::MemoryWrite { .. } => "memory_write",
        }
    }
}

/// A concrete route by which untrusted content acquired influence.
///
/// The finding is the path. `hops` starts at the untrusted origin and ends at the segment that
/// reached the sink, so a reviewer reads the laundering chain in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InjectionPath {
    pub origin: String,
    pub origin_provenance: Provenance,
    pub hops: Vec<String>,
    pub granted: Authority,
    pub sink: Sink,
}

impl InjectionPath {
    /// The sentence a reviewer needs, with the chain in it.
    pub fn describe(&self) -> String {
        format!(
            "{} content {} reached {} at authority {} via {}",
            self.origin_provenance,
            self.origin,
            self.sink.as_str(),
            self.granted,
            self.hops.join(" -> ")
        )
    }
}

/// A memory write, with the source and scope 13.12 requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryWrite {
    pub key: String,
    /// The segment this memory came from. Required: a memory with no source cannot have its
    /// provenance preserved on retrieval, which is the property 13.12 asks for.
    pub source_segment: String,
    pub scope: ScopeKey,
}

impl MemoryWrite {
    pub fn new(key: impl Into<String>, source_segment: impl Into<String>, scope: ScopeKey) -> Self {
        MemoryWrite {
            key: key.into(),
            source_segment: source_segment.into(),
            scope,
        }
    }
}

/// A tool call, with the segment each argument came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    /// `(argument name, originating segment id)`. An argument the architecture composed itself has
    /// no entry here.
    pub argument_origins: Vec<(String, String)>,
}

impl ToolCall {
    pub fn new(tool: impl Into<String>) -> Self {
        ToolCall {
            tool: tool.into(),
            argument_origins: Vec::new(),
        }
    }

    pub fn argument_from(
        mut self,
        argument: impl Into<String>,
        segment: impl Into<String>,
    ) -> Self {
        self.argument_origins
            .push((argument.into(), segment.into()));
        self
    }
}

/// The assembled context, plus the tool calls and memory writes it produced.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAssembly {
    segments: Vec<Segment>,
    tool_calls: Vec<ToolCall>,
    memory_writes: Vec<MemoryWrite>,
}

impl ContextAssembly {
    pub fn new() -> Self {
        ContextAssembly::default()
    }

    /// Adds a segment, enforcing the two authority rules.
    pub fn add(&mut self, segment: Segment) -> Result<(), SafetyError> {
        let ceiling = Authority::ceiling_for(segment.provenance);
        if segment.authority > ceiling {
            return Err(SafetyError::AuthorityElevation {
                segment: segment.id.clone(),
                provenance: segment.provenance.to_string(),
                granted: segment.authority.to_string(),
                permitted: ceiling.to_string(),
            });
        }
        if let Some(parent_id) = &segment.derived_from {
            let parent = self
                .segments
                .iter()
                .find(|s| &s.id == parent_id)
                .ok_or_else(|| SafetyError::DanglingDerivation {
                    segment: segment.id.clone(),
                    parent: parent_id.clone(),
                })?;
            if segment.authority > parent.authority {
                return Err(SafetyError::AuthorityLaundering {
                    segment: segment.id.clone(),
                    parent: parent.id.clone(),
                    granted: segment.authority.to_string(),
                    parent_authority: parent.authority.to_string(),
                });
            }
        }
        self.segments.push(segment);
        Ok(())
    }

    pub fn record_tool_call(&mut self, call: ToolCall) {
        self.tool_calls.push(call);
    }

    pub fn record_memory_write(&mut self, write: MemoryWrite) {
        self.memory_writes.push(write);
    }

    pub fn segment(&self, id: &str) -> Option<&Segment> {
        self.segments.iter().find(|s| s.id == id)
    }

    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// The derivation chain from a segment's untrusted root down to the segment itself.
    ///
    /// Returns `None` when the root's provenance is trusted: a summary of the system prompt is not
    /// an injection path however many hops it took.
    fn untrusted_chain(&self, segment: &Segment) -> Option<(Provenance, Vec<String>)> {
        let mut chain = vec![segment.id.clone()];
        let mut current = segment;
        let mut guard = 0usize;
        while let Some(parent_id) = &current.derived_from {
            guard += 1;
            if guard > self.segments.len() {
                break;
            }
            match self.segments.iter().find(|s| &s.id == parent_id) {
                Some(parent) => {
                    chain.push(parent.id.clone());
                    current = parent;
                }
                None => break,
            }
        }
        if current.provenance.is_untrusted() {
            chain.reverse();
            Some((current.provenance, chain))
        } else {
            None
        }
    }

    /// Every route by which untrusted content acquired influence.
    ///
    /// Three sinks, all structural: an instruction-following position, a tool argument, and a
    /// memory write that a later turn will read back.
    pub fn injection_paths(&self) -> Vec<InjectionPath> {
        let mut paths = Vec::new();
        for segment in &self.segments {
            let Some((origin_provenance, hops)) = self.untrusted_chain(segment) else {
                continue;
            };
            let origin = hops[0].clone();
            if segment.position == Position::Instruction {
                paths.push(InjectionPath {
                    origin: origin.clone(),
                    origin_provenance,
                    hops: hops.clone(),
                    granted: segment.authority,
                    sink: Sink::InstructionPosition,
                });
            }
            for call in &self.tool_calls {
                for (argument, source) in &call.argument_origins {
                    if source == &segment.id {
                        paths.push(InjectionPath {
                            origin: origin.clone(),
                            origin_provenance,
                            hops: hops.clone(),
                            granted: segment.authority,
                            sink: Sink::ToolArgument {
                                tool: call.tool.clone(),
                                argument: argument.clone(),
                            },
                        });
                    }
                }
            }
            for write in &self.memory_writes {
                if write.source_segment == segment.id {
                    paths.push(InjectionPath {
                        origin: origin.clone(),
                        origin_provenance,
                        hops: hops.clone(),
                        granted: segment.authority,
                        sink: Sink::MemoryWrite {
                            key: write.key.clone(),
                        },
                    });
                }
            }
        }
        paths
    }

    /// Segments that declare they may contain instructions and sit in an instruction position.
    ///
    /// A narrower query than [`ContextAssembly::injection_paths`], for callers that want the
    /// assembler's own declaration rather than the structural fact.
    pub fn declared_instruction_carriers(&self) -> Vec<&Segment> {
        self.segments
            .iter()
            .filter(|s| s.may_contain_instructions && s.position == Position::Instruction)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retrieved_content_cannot_be_assembled_above_data_authority() {
        let mut assembly = ContextAssembly::new();
        let error = assembly
            .add(
                Segment::new("doc-1", Provenance::RetrievedContent)
                    .at(Authority::TaskInstruction),
            )
            .expect_err("retrieved content defaults to data, not policy");
        assert!(matches!(error, SafetyError::AuthorityElevation { .. }));
        assert!(error.to_string().contains("permits at most data"), "{error}");
    }

    #[test]
    fn a_summary_of_a_web_page_cannot_be_promoted_above_the_web_page() {
        let mut assembly = ContextAssembly::new();
        assembly
            .add(Segment::new("page", Provenance::RetrievedContent))
            .expect("data authority is the ceiling and the default");
        let error = assembly
            .add(
                Segment::new("summary", Provenance::System)
                    .derived_from("page")
                    .at(Authority::PlatformPolicy),
            )
            .expect_err("laundering through a system-authored summary is still laundering");
        assert!(matches!(error, SafetyError::AuthorityLaundering { .. }));
    }

    #[test]
    fn a_segment_deriving_from_an_absent_parent_is_refused_rather_than_treated_as_a_root() {
        let mut assembly = ContextAssembly::new();
        let error = assembly
            .add(Segment::new("summary", Provenance::System).derived_from("nowhere"))
            .expect_err("an unbounded parent is an unbounded authority");
        assert!(matches!(error, SafetyError::DanglingDerivation { .. }));
    }

    #[test]
    fn untrusted_content_in_an_instruction_position_is_reported_as_a_path_not_a_score() {
        let mut assembly = ContextAssembly::new();
        assembly
            .add(
                Segment::new("tool-out", Provenance::ToolResult)
                    .positioned(Position::Instruction),
            )
            .expect("data authority in an instruction position is legal and is the finding");
        let paths = assembly.injection_paths();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].sink, Sink::InstructionPosition);
        assert_eq!(paths[0].hops, vec!["tool-out".to_string()]);
        assert!(paths[0].describe().contains("tool_result"));
    }

    #[test]
    fn the_reported_path_walks_the_whole_derivation_chain_back_to_the_untrusted_root() {
        let mut assembly = ContextAssembly::new();
        assembly
            .add(Segment::new("page", Provenance::RetrievedContent))
            .expect("root");
        assembly
            .add(
                Segment::new("extract", Provenance::System)
                    .derived_from("page")
                    .at(Authority::Data),
            )
            .expect("a system-authored extract of a web page is still a web page");
        assembly
            .add(
                Segment::new("preamble", Provenance::System)
                    .derived_from("extract")
                    .at(Authority::Data)
                    .positioned(Position::Instruction),
            )
            .expect("still capped at the root's data authority");
        let paths = assembly.injection_paths();
        assert_eq!(paths.len(), 1);
        assert_eq!(
            paths[0].hops,
            vec![
                "page".to_string(),
                "extract".to_string(),
                "preamble".to_string()
            ]
        );
        assert_eq!(paths[0].origin_provenance, Provenance::RetrievedContent);
    }

    #[test]
    fn a_derived_segment_must_declare_its_authority_rather_than_inheriting_the_default() {
        let mut assembly = ContextAssembly::new();
        assembly
            .add(Segment::new("page", Provenance::RetrievedContent))
            .expect("root");
        let error = assembly
            .add(Segment::new("extract", Provenance::System).derived_from("page"))
            .expect_err("the default is this segment's own ceiling, which is above the parent's");
        assert!(matches!(error, SafetyError::AuthorityLaundering { .. }));
    }

    #[test]
    fn a_derivation_chain_rooted_in_the_system_prompt_is_not_an_injection_path() {
        let mut assembly = ContextAssembly::new();
        assembly
            .add(Segment::new("sys", Provenance::System).positioned(Position::Instruction))
            .expect("root");
        assembly
            .add(
                Segment::new("restated", Provenance::System)
                    .derived_from("sys")
                    .positioned(Position::Instruction),
            )
            .expect("derived");
        assert!(assembly.injection_paths().is_empty());
    }

    #[test]
    fn a_tool_argument_sourced_from_a_web_page_is_its_own_finding() {
        let mut assembly = ContextAssembly::new();
        assembly
            .add(Segment::new("page", Provenance::RetrievedContent))
            .expect("root");
        assembly.record_tool_call(ToolCall::new("http_get").argument_from("url", "page"));
        let paths = assembly.injection_paths();
        assert_eq!(paths.len(), 1);
        assert_eq!(
            paths[0].sink,
            Sink::ToolArgument {
                tool: "http_get".into(),
                argument: "url".into()
            }
        );
    }

    #[test]
    fn a_memory_write_sourced_from_a_peer_agent_message_is_reported() {
        let mut assembly = ContextAssembly::new();
        assembly
            .add(Segment::new("peer-msg", Provenance::PeerAgentMessage))
            .expect("root");
        assembly.record_memory_write(MemoryWrite::new(
            "preferred_tool",
            "peer-msg",
            ScopeKey::default().exact("run", "r-1"),
        ));
        let paths = assembly.injection_paths();
        assert_eq!(paths.len(), 1);
        assert_eq!(
            paths[0].sink,
            Sink::MemoryWrite {
                key: "preferred_tool".into()
            }
        );
    }

    #[test]
    fn precedence_runs_platform_over_task_over_architecture_over_data() {
        assert!(Authority::PlatformPolicy.may_override(Authority::TaskInstruction));
        assert!(Authority::TaskInstruction.may_override(Authority::ArchitecturePolicy));
        assert!(Authority::ArchitecturePolicy.may_override(Authority::Data));
        assert!(!Authority::Data.may_override(Authority::Data));
        assert!(!Authority::Data.may_override(Authority::PlatformPolicy));
    }

    #[test]
    fn every_provenance_the_platform_did_not_author_caps_at_data() {
        for provenance in [
            Provenance::ToolResult,
            Provenance::RetrievedContent,
            Provenance::Memory,
            Provenance::PeerAgentMessage,
            Provenance::DocumentMetadata,
            Provenance::TraceReplay,
        ] {
            assert!(provenance.is_untrusted(), "{provenance}");
            assert_eq!(Authority::ceiling_for(provenance), Authority::Data);
        }
        for provenance in [Provenance::System, Provenance::Developer, Provenance::User] {
            assert!(!provenance.is_untrusted(), "{provenance}");
        }
    }

    #[test]
    fn a_new_untrusted_segment_declares_that_it_may_carry_instructions() {
        assert!(Segment::new("doc", Provenance::RetrievedContent).may_contain_instructions);
        assert!(!Segment::new("sys", Provenance::System).may_contain_instructions);
    }

    #[test]
    fn declared_carriers_are_a_narrower_query_than_structural_paths() {
        let mut assembly = ContextAssembly::new();
        let mut quiet = Segment::new("doc", Provenance::RetrievedContent)
            .positioned(Position::Instruction);
        quiet.may_contain_instructions = false;
        assembly.add(quiet).expect("the assembler may declare this");
        assert!(assembly.declared_instruction_carriers().is_empty());
        assert_eq!(
            assembly.injection_paths().len(),
            1,
            "the assembler's declaration does not change where the content sits"
        );
    }
}
