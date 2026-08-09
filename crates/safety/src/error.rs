//! Typed failures for the adversarial half of blueprint section 13.
//!
//! Every variant here names a *modelling* failure, not a runtime interception. `bioprism-safety`
//! never stops an attack; it refuses to let a caller record that an attack was stopped. Read each
//! message as "the model says you may not claim this", never as "the platform prevented this".
//!
//! The two families worth distinguishing when you match on these:
//!
//! * **Overclaim** — [`SafetyError::UnenforcedReliance`], [`SafetyError::ContainmentOverclaimed`],
//!   [`SafetyError::UnratedDimension`], [`SafetyError::UnwitnessedAttestation`]. Something was
//!   about to be reported as safe on the strength of a declaration. 13.23's "do not overclaim
//!   containment" is the whole family in one sentence.
//! * **Model violation** — [`SafetyError::BoundaryViolation`], [`SafetyError::AuthorityElevation`],
//!   [`SafetyError::EvaluatorOverreach`], [`SafetyError::ArtifactSubstitution`]. A caller asked
//!   this crate to move something across a boundary the model forbids, and the *model* said no.
//!   The artifact did not move because this crate was the one moving it. Anything moving by
//!   another route is invisible here.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Every failure this crate can produce.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SafetyError {
    /// A caller treated a mitigation as effective when nothing applies it. 13.01's control
    /// philosophy lists a dozen controls; in this process every one of them that is not a Rust
    /// type is a sentence in a document.
    #[error(
        "threat {threat:?} is mitigated only by declaration {mitigation:?}; a declared mitigation \
         is not a control and must not be relied on"
    )]
    UnenforcedReliance { threat: String, mitigation: String },

    /// A threat with no analysed mitigation was asked for a status other than unmitigated.
    #[error("threat {threat:?} has no mitigation recorded; absent is not the same as accepted")]
    UnmitigatedThreat { threat: String },

    /// Residual risk was accepted without an accepting party. 13.01 requires residual risk to be
    /// *documented*; an anonymous acceptance documents nothing.
    #[error("residual risk for threat {threat:?} was accepted with no accepting party named")]
    AnonymousAcceptance { threat: String },

    /// An artifact was asked to cross a trust boundary in a direction the model forbids. 13.05's
    /// data diode is the case that matters: nothing flows from the evaluator sandbox back to the
    /// agent sandbox, in any channel, ever.
    #[error(
        "{artifact:?} may not cross from {from} to {to}{channel}: {reason}",
        channel = .channel.as_ref().map(|c| format!(" via {c}")).unwrap_or_default()
    )]
    BoundaryViolation {
        artifact: String,
        from: String,
        to: String,
        channel: Option<String>,
        reason: String,
    },

    /// A boundary was consulted that the model does not describe. Deny by default (13.01): an
    /// unmodelled edge is refused rather than allowed, because the alternative is that forgetting
    /// to model an edge opens it.
    #[error("no trust-boundary edge is modelled from {from} to {to}; unmodelled edges are closed")]
    UnmodelledEdge { from: String, to: String },

    /// The evaluator tried to do something 13.05 reserves for the control plane: publish a result,
    /// edit pack metadata, or choose the comparison population.
    #[error("evaluator {evaluator:?} may issue claims and evidence but may not {action}")]
    EvaluatorOverreach { evaluator: String, action: String },

    /// A context segment reached an instruction-following position with authority higher than its
    /// provenance allows. 13.13: "Retrieved content defaults to data, not policy."
    #[error(
        "segment {segment:?} from {provenance} was assembled at authority {granted} but its \
         provenance permits at most {permitted}"
    )]
    AuthorityElevation {
        segment: String,
        provenance: String,
        granted: String,
        permitted: String,
    },

    /// A derived segment claims more authority than the segment it came from. Derivation may lose
    /// authority and may never gain it.
    #[error(
        "segment {segment:?} derives from {parent:?} at authority {granted}, above the parent's \
         {parent_authority}; derivation cannot raise authority"
    )]
    AuthorityLaundering {
        segment: String,
        parent: String,
        granted: String,
        parent_authority: String,
    },

    /// A segment names a parent that is not in the assembly, so its authority cannot be bounded.
    #[error("segment {segment:?} derives from {parent:?}, which is not in this assembly")]
    DanglingDerivation { segment: String, parent: String },

    /// A declared digest and an observed digest disagree. This is a string comparison over two
    /// values the caller supplied; this crate fetched nothing and hashed no remote artifact.
    #[error("component {component:?} was pinned to {declared} but the caller observed {observed}")]
    ArtifactSubstitution {
        component: String,
        declared: String,
        observed: String,
    },

    /// A published run contains a component with no immutable pin. 13.15: "no floating `latest` in
    /// published runs".
    #[error("component {component:?} is pinned to the floating reference {reference:?}")]
    FloatingPin {
        component: String,
        reference: String,
    },

    /// An audit chain does not rehash to its recorded links.
    #[error(
        "audit record {index} links to {recorded}, but the preceding record hashes to {actual}"
    )]
    AuditChainBroken {
        index: usize,
        recorded: String,
        actual: String,
    },

    /// An attestation asserted something this process never observed. 13.20 asks for narrow claims
    /// like "built from manifest X"; this crate can only mint the ones it computed itself.
    #[error(
        "attestation {claim:?} is asserted by {asserter:?}; nothing in this process observed it"
    )]
    UnwitnessedAttestation { claim: String, asserter: String },

    /// A vulnerability moved through its lifecycle out of order, or backwards in epoch.
    #[error("vulnerability {id:?} cannot move from {from} to {to}: {reason}")]
    LifecycleViolation {
        id: String,
        from: String,
        to: String,
        reason: String,
    },

    /// An operator epoch went backwards. There is no clock here; epochs are the only ordering.
    #[error("{subject:?} recorded epoch {epoch}, not after epoch {previous}")]
    EpochNotAdvancing {
        subject: String,
        previous: u64,
        epoch: u64,
    },

    /// A red-team finding that nobody reproduced was promoted to a regression cell. 13.21 converts
    /// *confirmed* classes into sentinels; an unconfirmed one would install a test for a bug that
    /// may not exist.
    #[error("finding {finding:?} is {status} and cannot become a regression cell until confirmed")]
    UnconfirmedFinding { finding: String, status: String },

    /// Containment was declared complete while the blast radius is incompletely known. 13.23:
    /// "State known facts and uncertainty; do not overclaim containment."
    #[error(
        "incident {incident:?} cannot be reported contained: blast radius is {completeness} and \
         {unresolved} dependent result(s) are unresolved"
    )]
    ContainmentOverclaimed {
        incident: String,
        completeness: String,
        unresolved: usize,
    },

    /// A dual-use release was cleared with a risk dimension left unrated. Unrated is not low.
    #[error("release gate for {subject:?} cannot clear: dimension {dimension} is unrated")]
    UnratedDimension { subject: String, dimension: String },

    /// Someone tried to use the dual-use gate to hide that a weakness exists, rather than to hide
    /// how to exploit it. 13.26 separates these explicitly and this crate refuses to conflate them.
    #[error(
        "withholding the existence of finding {finding:?} is result suppression, not dual-use \
         control; exploit detail may be withheld, existence may not"
    )]
    SuppressionDisguisedAsSafety { finding: String },

    /// An output crossed 13.25's research boundary into clinical territory.
    #[error(
        "{output:?} is a {category} output; this platform is research-only and does not produce it"
    )]
    ClinicalBoundary { output: String, category: String },

    /// A safety-relevant check was asked for a verdict with no evidence to give one.
    #[error("{subject:?} is underdetermined: {reason}; unmeasured is not clean")]
    Underdetermined { subject: String, reason: String },
}

/// Whether a [`SafetyError`] reports an overclaim or a refused model transition.
///
/// Useful to a caller that wants to fail closed on overclaims while merely logging refusals it
/// expected. The distinction is editorial, not structural, so it lives here rather than splitting
/// the enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorFamily {
    /// Something was about to be reported safer than the evidence supports.
    Overclaim,
    /// A movement or transition the model forbids.
    ModelViolation,
}

impl fmt::Display for ErrorFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ErrorFamily::Overclaim => "overclaim",
            ErrorFamily::ModelViolation => "model-violation",
        })
    }
}

impl SafetyError {
    /// Which family this failure belongs to.
    pub fn family(&self) -> ErrorFamily {
        match self {
            SafetyError::UnenforcedReliance { .. }
            | SafetyError::UnmitigatedThreat { .. }
            | SafetyError::AnonymousAcceptance { .. }
            | SafetyError::UnwitnessedAttestation { .. }
            | SafetyError::UnconfirmedFinding { .. }
            | SafetyError::ContainmentOverclaimed { .. }
            | SafetyError::UnratedDimension { .. }
            | SafetyError::FloatingPin { .. }
            | SafetyError::Underdetermined { .. } => ErrorFamily::Overclaim,
            SafetyError::BoundaryViolation { .. }
            | SafetyError::UnmodelledEdge { .. }
            | SafetyError::EvaluatorOverreach { .. }
            | SafetyError::AuthorityElevation { .. }
            | SafetyError::AuthorityLaundering { .. }
            | SafetyError::DanglingDerivation { .. }
            | SafetyError::ArtifactSubstitution { .. }
            | SafetyError::AuditChainBroken { .. }
            | SafetyError::LifecycleViolation { .. }
            | SafetyError::EpochNotAdvancing { .. }
            | SafetyError::SuppressionDisguisedAsSafety { .. }
            | SafetyError::ClinicalBoundary { .. } => ErrorFamily::ModelViolation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relying_on_a_declared_mitigation_is_reported_as_an_overclaim_not_a_violation() {
        let error = SafetyError::UnenforcedReliance {
            threat: "sandbox-escape".into(),
            mitigation: "hardened container".into(),
        };
        assert_eq!(error.family(), ErrorFamily::Overclaim);
        assert!(error.to_string().contains("is not a control"));
    }

    #[test]
    fn a_refused_boundary_crossing_names_both_zones_and_the_channel() {
        let error = SafetyError::BoundaryViolation {
            artifact: "oracle-key".into(),
            from: "evaluator-sandbox".into(),
            to: "agent-sandbox".into(),
            channel: Some("shared-mount".into()),
            reason: "the evaluator/agent diode is one-way".into(),
        };
        let text = error.to_string();
        assert!(text.contains("evaluator-sandbox"), "{text}");
        assert!(text.contains("agent-sandbox"), "{text}");
        assert!(text.contains("via shared-mount"), "{text}");
        assert_eq!(error.family(), ErrorFamily::ModelViolation);
    }

    #[test]
    fn a_crossing_with_no_named_channel_omits_the_channel_clause_entirely() {
        let error = SafetyError::BoundaryViolation {
            artifact: "holdout".into(),
            from: "catalog".into(),
            to: "agent-sandbox".into(),
            channel: None,
            reason: "hidden assets are evaluator-only".into(),
        };
        assert!(!error.to_string().contains("via"), "{error}");
    }
}
