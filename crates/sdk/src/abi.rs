//! Portability grades, and refusing an ask the plugin cannot hold.
//!
//! Blueprint 23.49 defines ABI-P0 through ABI-P4 and says capability cards state the supported
//! grade. Its governing sentence is the last one of the version-negotiation section: *"A plain-text
//! agent can participate at a lower grade, but it cannot falsely claim continuation or evidence
//! semantics."*
//!
//! # The grade is a ceiling on what may be asked, not a score
//!
//! A P0 plugin is not a worse plugin. It is a plugin that cannot hold state between turns, and the
//! failure mode 23.49 is guarding against is a host that hands one a continuation handle and
//! discovers the loss when the resumed thread quietly starts from nothing. [`AbiGrade::can`] turns
//! that into a check that happens before the handle is passed.
//!
//! # Why the asks are an enum and not a bitmask
//!
//! 23.49's grades are cumulative — P3 includes P2's capsule awareness — so a set of independent
//! flags would be able to represent "resumes continuations but cannot read a capsule", which the
//! specification does not define and no host could sensibly use. Making the grade a total order
//! and each ask a floor on it keeps the unrepresentable states unrepresentable.

use crate::error::SelectionError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// 23.49's portability grades, cumulative and totally ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbiGrade {
    /// Message adapter: receives and emits typed acts, cannot resume state.
    P0,
    /// Artifact and effect adapter: typed artifacts and kernel-mediated effects.
    P1,
    /// Capsule-aware: consumes recipient projections, reports structured uncertainty.
    P2,
    /// Continuation-aware: can suspend, transfer and resume typed execution state.
    P3,
    /// Molecule-native: dynamic role binding, nested interfaces, full conformance and replay.
    P4,
}

impl AbiGrade {
    pub const ALL: [AbiGrade; 5] = [
        AbiGrade::P0,
        AbiGrade::P1,
        AbiGrade::P2,
        AbiGrade::P3,
        AbiGrade::P4,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            AbiGrade::P0 => "ABI-P0",
            AbiGrade::P1 => "ABI-P1",
            AbiGrade::P2 => "ABI-P2",
            AbiGrade::P3 => "ABI-P3",
            AbiGrade::P4 => "ABI-P4",
        }
    }

    /// What 23.49 says this grade adds over the one below it.
    pub fn summary(self) -> &'static str {
        match self {
            AbiGrade::P0 => "receives and emits typed acts; cannot resume state",
            AbiGrade::P1 => "uses typed artifacts and kernel-mediated effects",
            AbiGrade::P2 => "consumes recipient projections and reports structured uncertainty",
            AbiGrade::P3 => "suspends, transfers and resumes typed execution state",
            AbiGrade::P4 => "binds roles dynamically and exposes nested interfaces",
        }
    }

    pub fn can(self, ask: HostAsk) -> bool {
        self >= ask.requires()
    }
}

impl fmt::Display for AbiGrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Something a host wants a participant plugin to do, with the grade it needs.
///
/// One variant per rung, because each rung of 23.49 is defined by exactly the thing the rung below
/// cannot do. A host that wants to ask something not on this list is asking for a rung that does
/// not exist yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAsk {
    /// Exchange typed communicative acts.
    ExchangeActs,
    /// Accept artifact references and route effects through the host.
    UseArtifactsAndEffects,
    /// Read a recipient-projected context capsule and report structured uncertainty.
    ConsumeContextCapsule,
    /// Resume a continuation handle: the ask 23.49 singles out as the one a low grade must not be
    /// allowed to falsely claim.
    ResumeContinuation,
    /// Bind a role dynamically and expose nested interfaces.
    BindRoleDynamically,
}

impl HostAsk {
    pub fn requires(self) -> AbiGrade {
        match self {
            HostAsk::ExchangeActs => AbiGrade::P0,
            HostAsk::UseArtifactsAndEffects => AbiGrade::P1,
            HostAsk::ConsumeContextCapsule => AbiGrade::P2,
            HostAsk::ResumeContinuation => AbiGrade::P3,
            HostAsk::BindRoleDynamically => AbiGrade::P4,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            HostAsk::ExchangeActs => "exchange typed acts",
            HostAsk::UseArtifactsAndEffects => "use artifacts and mediated effects",
            HostAsk::ConsumeContextCapsule => "consume a context capsule",
            HostAsk::ResumeContinuation => "resume a continuation",
            HostAsk::BindRoleDynamically => "bind a role dynamically",
        }
    }
}

impl fmt::Display for HostAsk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Checks one ask against one declared grade, producing the refusal 23.49 requires.
///
/// Free function rather than a method on [`AbiGrade`] because the error needs the plugin's name,
/// which the grade does not know. The refusal names the plugin, the ask, what was declared and
/// what was needed — enough to fix without reading either side's source.
pub fn check_ask(plugin: &str, declared: AbiGrade, ask: HostAsk) -> Result<(), SelectionError> {
    if declared.can(ask) {
        Ok(())
    } else {
        Err(SelectionError::AbiGradeTooLow {
            plugin: plugin.to_string(),
            ask: ask.to_string(),
            declared: declared.to_string(),
            required: ask.requires().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_p0_plugin_may_not_be_asked_to_resume_a_continuation() {
        let error = check_ask("plaintext-agent", AbiGrade::P0, HostAsk::ResumeContinuation)
            .expect_err("P0 cannot hold state");
        let rendered = error.to_string();
        assert!(rendered.contains("ABI-P0"), "{rendered}");
        assert!(rendered.contains("ABI-P3"), "{rendered}");
    }

    #[test]
    fn a_p0_plugin_may_still_exchange_typed_acts() {
        assert!(check_ask("plaintext-agent", AbiGrade::P0, HostAsk::ExchangeActs).is_ok());
    }

    #[test]
    fn grades_are_cumulative_so_a_higher_grade_answers_every_lower_ask() {
        for ask in [
            HostAsk::ExchangeActs,
            HostAsk::UseArtifactsAndEffects,
            HostAsk::ConsumeContextCapsule,
            HostAsk::ResumeContinuation,
            HostAsk::BindRoleDynamically,
        ] {
            assert!(AbiGrade::P4.can(ask), "P4 should answer {ask}");
        }
    }

    #[test]
    fn every_grade_is_the_floor_for_exactly_one_ask() {
        let mut floors: Vec<AbiGrade> = [
            HostAsk::ExchangeActs,
            HostAsk::UseArtifactsAndEffects,
            HostAsk::ConsumeContextCapsule,
            HostAsk::ResumeContinuation,
            HostAsk::BindRoleDynamically,
        ]
        .iter()
        .map(|ask| ask.requires())
        .collect();
        floors.sort();
        floors.dedup();
        assert_eq!(floors, AbiGrade::ALL.to_vec());
    }
}
