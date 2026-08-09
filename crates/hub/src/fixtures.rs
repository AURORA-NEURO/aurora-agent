//! Shared test fixtures.
//!
//! Compiled only under `cfg(test)`. Kept in one place so that a change to the submission contract
//! breaks every module's tests at once rather than being papered over module by module.

use crate::attribution::Licence;
use crate::id::{Epoch, SubmissionId, SubmitterId};
use crate::submission::{
    accept, BuildProvenance, DeclaredScope, EvidenceScale, NonClaim, Provenance, Submission,
    SubmissionDraft, Submitter,
};
use bioprism_ids::ContentHash;

pub(crate) fn submitter() -> Submitter {
    Submitter::unverified(SubmitterId::parse("lab-a").unwrap()).declaring_no_conflicts()
}

pub(crate) fn complete_draft() -> SubmissionDraft {
    SubmissionDraft {
        id: Some(SubmissionId::parse("sub-1").unwrap()),
        submitter: Some(SubmitterId::parse("lab-a").unwrap()),
        content: Some(ContentHash::of_bytes(b"artifact")),
        scope: Some(DeclaredScope {
            disease: vec!["glioma".into()],
            modality: vec!["mri".into()],
            decision_family: vec!["evidence-acquisition".into()],
            intended_use: "compare context compilers on a synthetic glioma worldline".into(),
            out_of_scope: vec!["paediatric cohorts".into()],
        }),
        licence: Some(Licence::permissive("CC0-1.0")),
        provenance: Some(Provenance {
            ancestors: Vec::new(),
            build: BuildProvenance {
                toolchain: "rustc 1.85".into(),
                source_digest: ContentHash::of_bytes(b"source"),
                reproducible: true,
            },
            attestations: vec!["local-signature".into()],
        }),
        does_not_establish: vec![NonClaim::clinical_validity()],
        attributions: Vec::new(),
        evidence_scale: Some(EvidenceScale::new(120, 12)),
        claimed_verification: None,
        submitted_at: Epoch(1),
    }
}

pub(crate) fn accepted_submission() -> Submission {
    accept(complete_draft(), &submitter()).expect("fixture satisfies the submission contract")
}

pub(crate) fn submission_with_id(id: &str) -> Submission {
    let mut draft = complete_draft();
    draft.id = Some(SubmissionId::parse(id).unwrap());
    draft.content = Some(ContentHash::of_bytes(id.as_bytes()));
    accept(draft, &submitter()).expect("fixture satisfies the submission contract")
}
