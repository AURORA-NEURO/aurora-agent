//! The vertical slice: a named, runnable, self-checking end-to-end scenario.
//!
//! Blueprint 38 opens by calling its reference worlds "implementation-grade specifications", and
//! 38.01 closes with a ten-point acceptance list whose first item is that a clean machine loads
//! the world. A [`VerticalSlice`] is that specification made executable: a world spec, the query
//! asked of it, the property it is supposed to demonstrate, and — the part that makes it evidence
//! rather than documentation — the outcome it is supposed to produce.
//!
//! Putting the expectation *inside* the example is the whole design. A reference example whose
//! expected verdict lives in a README drifts the first time a pass changes, and nothing fails. A
//! reference example that carries its own expectation fails a test the moment the claim stops
//! holding, which is the only difference between a demonstration and an assertion.
//!
//! What a slice deliberately does not do is decide whether a failure is a bug. [`run`] returns
//! `Ok` with a failing report when a claim stops holding, and `Err` only when the example itself
//! is broken. Those are different findings and collapsing them would hide the interesting one.

use crate::error::ExampleError;
use crate::expectation::{Expectation, GraphWalkProbe};
use crate::property::Property;
use crate::report::{
    CompiledObservation, DeferredPass, Observations, PassObservation, RefusalCode,
    RefusalObservation, SliceReport,
};
use crate::scenario::SliceWorld;
use crate::walk;
use bioprism_fiber::{compile, CompileOutput, FiberError};
use bioprism_section::CertificateProfile;
use bioprism_world::WorldSource;
use serde::{Deserialize, Serialize};

/// One end-to-end scenario the platform claims to handle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerticalSlice {
    /// Stable identifier. Reports are attributed by it, so it must not change once published.
    pub id: String,
    pub title: String,
    /// Why this slice exists and what a reader should conclude from it, in prose.
    pub narrative: String,
    pub blueprint_modules: Vec<String>,
    /// The single property this slice is registered to demonstrate.
    pub demonstrates: Property,
    /// Further properties this run happens to exercise. Secondary coverage is still coverage,
    /// and pretending otherwise would understate what the suite tests.
    #[serde(default)]
    pub also_exercises: Vec<Property>,
    pub world: SliceWorld,
    pub expectation: Expectation,
    /// An equal-engineering neighbourhood-walk sweep run alongside the compile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_walk: Option<GraphWalkProbe>,
}

impl VerticalSlice {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        demonstrates: Property,
        world: SliceWorld,
        expectation: Expectation,
    ) -> Self {
        VerticalSlice {
            id: id.into(),
            title: title.into(),
            narrative: String::new(),
            blueprint_modules: demonstrates
                .blueprint_modules()
                .iter()
                .map(|m| (*m).to_string())
                .collect(),
            demonstrates,
            also_exercises: Vec::new(),
            world,
            expectation,
            graph_walk: None,
        }
    }

    pub fn narrating(mut self, narrative: impl Into<String>) -> Self {
        self.narrative = narrative.into();
        self
    }

    pub fn citing<I, S>(mut self, modules: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.blueprint_modules = modules.into_iter().map(Into::into).collect();
        self
    }

    pub fn also_exercising<I: IntoIterator<Item = Property>>(mut self, properties: I) -> Self {
        self.also_exercises = properties.into_iter().collect();
        self
    }

    pub fn with_graph_walk(mut self, probe: GraphWalkProbe) -> Self {
        self.graph_walk = Some(probe);
        self
    }

    /// Every property this slice claims to touch, primary first.
    pub fn exercised(&self) -> Vec<Property> {
        let mut all = vec![self.demonstrates];
        all.extend(self.also_exercises.iter().copied());
        all
    }

    /// Builds the world, compiles, probes, and checks every declared expectation.
    pub fn run(&self) -> Result<SliceReport, ExampleError> {
        let (world, query) = self.world.build(&self.id)?;

        let mut observations = Observations {
            world_id: WorldSource::world_id(&world).to_string(),
            query_id: query.query_id.as_str().to_string(),
            world_digest: world.world_digest().as_str().to_string(),
            total_facts: world.facts.len(),
            total_factors: world.factors.len(),
            compiled: None,
            refused: None,
            graph_walk: None,
        };

        match compile(&world, &query) {
            Ok(output) => {
                observations.compiled = Some(observe_compile(&self.id, &output)?);
            }
            Err(error) => observations.refused = Some(observe_refusal(&error)),
        }

        let mut failures = self.expectation.check(&observations);

        if let Some(probe) = &self.graph_walk {
            let observed = walk::sweep(&world, &query, probe.max_depth);
            failures.extend(probe.check(&observed));
            observations.graph_walk = Some(observed);
        }

        let mut report = SliceReport {
            slice_id: self.id.clone(),
            title: self.title.clone(),
            demonstrates: self.demonstrates,
            also_exercises: self.also_exercises.clone(),
            blueprint_modules: self.blueprint_modules.clone(),
            observations,
            failures,
            digest: String::new(),
        };
        report.digest = report
            .recompute_digest()
            .map_err(|source| ExampleError::Digest {
                slice: self.id.clone(),
                source,
            })?;
        Ok(report)
    }
}

/// Projects a compile into the recorded observation.
///
/// Both certificate profiles are digested and both are verified. The reference profile is the one
/// that must stay byte-compatible across implementations; the extended profile is the one that
/// actually carries the influence-classified omission manifest 43.26 requires. A slice that
/// recorded only the first would be silently blind to the manifest.
fn observe_compile(
    slice: &str,
    output: &CompileOutput,
) -> Result<CompiledObservation, ExampleError> {
    let digest_error = |source| ExampleError::Digest {
        slice: slice.to_string(),
        source,
    };

    let certificate = &output.certificate;
    let section = &output.section;

    let reference_document = certificate
        .to_json(CertificateProfile::Reference)
        .map_err(digest_error)?;
    let extended_document = certificate
        .to_json(CertificateProfile::Extended)
        .map_err(digest_error)?;
    let certificate_verifies = bioprism_section::ContextCertificate::verify(&reference_document)
        .map_err(digest_error)?
        .is_valid()
        && bioprism_section::ContextCertificate::verify(&extended_document)
            .map_err(digest_error)?
            .is_valid();

    Ok(CompiledObservation {
        status: certificate.oracle.status,
        witness_kinds: certificate
            .oracle
            .witness_kinds()
            .into_iter()
            .map(str::to_string)
            .collect(),
        witnesses: certificate.oracle.witnesses.clone(),
        selected_facts: certificate.selected_facts.clone(),
        selected_factors: certificate.selected_factors.clone(),
        protected_closure: certificate.protected_closure.clone(),
        protected_closure_satisfied: output.protected_closure_satisfied(),
        dropped_protected: output.trace.dropped_protected.clone(),
        unmatched_protected_tags: output.trace.unmatched_protected_tags.clone(),
        unresolved_obligations: section.unresolved_obligations.clone(),
        refinement_frontier_actions: section
            .refinement_frontier
            .iter()
            .map(|option| option.action.clone())
            .collect(),
        omission_influence_classes: certificate
            .manifest
            .groups
            .iter()
            .map(|group| group.influence.as_str().to_string())
            .collect(),
        omitted_fact_count: certificate.omissions.total_facts,
        supports_sufficiency_claim: certificate.manifest.supports_sufficiency_claim(),
        deferred_passes: output
            .trace
            .deferred_passes
            .iter()
            .map(|(pass, reason)| DeferredPass {
                pass: (*pass).to_string(),
                reason: (*reason).to_string(),
            })
            .collect(),
        passes: output
            .trace
            .passes
            .iter()
            .map(|receipt| PassObservation {
                name: receipt.name.to_string(),
                retained: receipt.retained,
                note: receipt.note.clone(),
            })
            .collect(),
        backend: certificate.plan.backend.as_str().to_string(),
        compiled_factor_count: certificate.plan.compiled_factor_count,
        section_digest: section
            .content_hash()
            .map_err(digest_error)?
            .as_str()
            .to_string(),
        certificate_digest_reference: certificate
            .digest(CertificateProfile::Reference)
            .map_err(digest_error)?
            .as_str()
            .to_string(),
        certificate_digest_extended: certificate
            .digest(CertificateProfile::Extended)
            .map_err(digest_error)?
            .as_str()
            .to_string(),
        certificate_verifies,
    })
}

/// Classifies a compiler refusal.
///
/// Every `FiberError` variant is matched explicitly rather than through a wildcard, so a new
/// failure mode in the compiler forces a decision here instead of arriving silently as
/// [`RefusalCode::Other`].
fn observe_refusal(error: &FiberError) -> RefusalObservation {
    let (code, selected, max_facts) = match error {
        FiberError::BudgetExceeded {
            selected,
            max_facts,
        } => (
            RefusalCode::BudgetExceeded,
            Some(*selected),
            Some(*max_facts),
        ),
        FiberError::UnorderableSplitGroups { .. } => {
            (RefusalCode::UnorderableSplitGroups, None, None)
        }
        FiberError::UnsupportedQuerySchema { .. }
        | FiberError::QueryNotAnObject
        | FiberError::UnknownQueryFields { .. }
        | FiberError::MissingQueryField(_)
        | FiberError::WrongQueryFieldType { .. }
        | FiberError::InvalidIdentifier(_)
        | FiberError::InvalidDecisionTime(_) => (RefusalCode::MalformedQuery, None, None),
        FiberError::World(_) => (RefusalCode::MalformedWorld, None, None),
        FiberError::Policy(_) => (RefusalCode::PolicyRefused, None, None),
    };

    RefusalObservation {
        code,
        message: error.to_string(),
        selected,
        max_facts,
    }
}
