//! The conformance suite runner and the bundles it produces.
//!
//! Implements blueprint 40.32. A suite is a versioned document — fixture manifest plus cases —
//! and running it against an [`Implementation`] yields a machine-readable bundle with a verdict
//! per requirement, an environment manifest, and the identity of what was tested.
//!
//! Three properties of the runner are deliberate.
//!
//! **It does not stop at the first failure.** A vendor needs the full list, and a suite that
//! aborts reports the alphabetically-first defect over and over across a remediation cycle.
//!
//! **It distinguishes four outcomes, not two.** Passed, Failed, Unsupported (the implementation
//! does not publish what the requirement observes — 40.32's "partial unsupported requirement")
//! and Errored (the case itself could not run). Collapsing Unsupported into Failed punishes a
//! partial implementation for being honest about its scope; collapsing it into Passed certifies
//! a requirement nobody checked.
//!
//! **A certificate is all-or-nothing.** [`SuiteReport::certify`] refuses over any failed, errored
//! or unsupported `must` requirement. The bundle is still produced and still discloses
//! everything; what is withheld is the certificate. 40.32 invariant 1 — conformance is not trust
//! or accuracy — cuts both ways: it is also not partial credit.
//!
//! Not implemented: certificate revocation lists, signing, and expiry evaluation. The certificate
//! carries caller-supplied `issued_at`/`expires_at` stamps because this crate has no clock and no
//! key material; nothing here checks that a certificate is still in date.

use crate::case::{ConformanceCase, Expectation, Layer, Requirement};
use crate::error::ConformanceError;
use crate::fixture::{FixtureDrift, FixtureManifest, FixtureStore};
use crate::gate::BaselineReference;
use crate::implementation::{
    CompileArtifacts, CompileFailure, EnvironmentManifest, Implementation, ImplementationIdentity,
};
use crate::pyramid::PyramidShape;
use bioprism_ids::{to_canonical_string, ContentHash};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

pub const SUITE_SCHEMA_VERSION: &str = "bioprism-conformance-suite/0.1";
pub const REPORT_SCHEMA_VERSION: &str = "bioprism-conformance-report/0.1";
pub const CONFORMANCE_CERTIFICATE_SCHEMA_VERSION: &str = "bioprism-conformance-certificate/0.1";

/// A versioned set of certifiable requirements plus the fixtures they run against.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Suite {
    pub schema_version: String,
    pub suite_id: String,
    pub suite_version: String,
    pub description: String,
    pub manifest: FixtureManifest,
    pub cases: Vec<ConformanceCase>,
}

impl Suite {
    pub fn new(
        suite_id: impl Into<String>,
        suite_version: impl Into<String>,
        description: impl Into<String>,
        manifest: FixtureManifest,
        cases: Vec<ConformanceCase>,
    ) -> Self {
        Suite {
            schema_version: SUITE_SCHEMA_VERSION.to_string(),
            suite_id: suite_id.into(),
            suite_version: suite_version.into(),
            description: description.into(),
            manifest,
            cases,
        }
    }

    /// The suite as data, for a third-party runner.
    pub fn to_json(&self) -> Result<Value, ConformanceError> {
        serde_json::to_value(self).map_err(|e| ConformanceError::MalformedSuite(e.to_string()))
    }

    pub fn from_json(document: &Value) -> Result<Self, ConformanceError> {
        let suite: Suite = serde_json::from_value(document.clone())
            .map_err(|e| ConformanceError::MalformedSuite(e.to_string()))?;
        if suite.schema_version != SUITE_SCHEMA_VERSION {
            return Err(ConformanceError::MalformedSuite(format!(
                "unsupported suite schema: expected {SUITE_SCHEMA_VERSION:?}, got {:?}",
                suite.schema_version
            )));
        }
        Ok(suite)
    }

    /// Content digest of the suite document.
    ///
    /// 40.32 invariant 2 requires suites to be versioned; a version string alone is a promise,
    /// while the digest is checkable. A report carries this so that "passed suite 0.1.0" cannot
    /// silently mean two different sets of cases.
    pub fn digest(&self) -> Result<String, ConformanceError> {
        let document = self.to_json()?;
        ContentHash::of_value(&document)
            .map(|hash| hash.as_str().to_string())
            .map_err(|e| ConformanceError::Canonical(e.to_string()))
    }

    pub fn case(&self, id: &str) -> Option<&ConformanceCase> {
        self.cases.iter().find(|case| case.id == id)
    }

    pub fn pyramid(&self) -> PyramidShape {
        PyramidShape::from_layers(self.cases.iter().map(|case| case.layer))
    }

    /// Distinct blueprint module ids the suite claims to enforce.
    pub fn enforced_modules(&self) -> BTreeSet<&str> {
        self.cases
            .iter()
            .flat_map(|case| case.enforces.iter().map(String::as_str))
            .collect()
    }

    pub fn run(
        &self,
        implementation: &dyn Implementation,
        fixtures: &FixtureStore,
    ) -> Result<SuiteReport, ConformanceError> {
        let mut results = Vec::with_capacity(self.cases.len());
        for case in &self.cases {
            results.push(self.run_case(case, implementation, fixtures));
        }

        Ok(SuiteReport {
            schema_version: REPORT_SCHEMA_VERSION.to_string(),
            suite_id: self.suite_id.clone(),
            suite_version: self.suite_version.clone(),
            suite_digest: self.digest()?,
            implementation: implementation.identity(),
            environment: EnvironmentManifest::detect(),
            fixture_manifest_id: fixtures.manifest_id().to_string(),
            fixture_count: fixtures.len(),
            synthetic_fixtures: fixtures
                .synthetic_ids()
                .into_iter()
                .map(str::to_string)
                .collect(),
            results,
            fixture_drift: fixtures.drift().to_vec(),
            baseline: None,
            retried_to_green: Vec::new(),
        })
    }

    fn run_case(
        &self,
        case: &ConformanceCase,
        implementation: &dyn Implementation,
        fixtures: &FixtureStore,
    ) -> CaseResult {
        let result = |outcome: CaseOutcome| CaseResult {
            case_id: case.id.clone(),
            title: case.title.clone(),
            layer: case.layer,
            requirement: case.requirement,
            enforces: case.enforces.clone(),
            invariant: case.invariant.clone(),
            expectations: case.expectation_kinds(),
            outcome,
        };

        let inputs = match prepare_inputs(case, fixtures) {
            Ok(inputs) => inputs,
            Err(error) => {
                return result(CaseOutcome::Errored {
                    reason: error.to_string(),
                })
            }
        };

        let first = implementation.compile(&inputs.0, &inputs.1);
        let needs_second = case
            .expect
            .iter()
            .any(|e| matches!(e, Expectation::RecompilationIsByteIdentical));
        let second = if needs_second {
            Some(implementation.compile(&inputs.0, &inputs.1))
        } else {
            None
        };

        for expectation in &case.expect {
            match evaluate(expectation, &first, second.as_ref(), fixtures) {
                Check::Pass => {}
                Check::Fail(detail) => {
                    return result(CaseOutcome::Failed {
                        expectation: expectation.kind().to_string(),
                        detail,
                    })
                }
                Check::Unsupported(reason) => {
                    return result(CaseOutcome::Unsupported {
                        expectation: expectation.kind().to_string(),
                        reason,
                    })
                }
            }
        }
        result(CaseOutcome::Passed)
    }
}

fn prepare_inputs(
    case: &ConformanceCase,
    fixtures: &FixtureStore,
) -> Result<(Value, Value), ConformanceError> {
    let mut world = fixtures.get(&case.input.world)?.clone();
    let mut query = fixtures.get(&case.input.query)?.clone();
    apply_overrides(&mut world, &case.input.world_overrides, &case.id, "world")?;
    apply_overrides(&mut query, &case.input.query_overrides, &case.id, "query")?;
    Ok((world, query))
}

fn apply_overrides(
    document: &mut Value,
    overrides: &[crate::case::Override],
    case_id: &str,
    which: &str,
) -> Result<(), ConformanceError> {
    for edit in overrides {
        match document.pointer_mut(&edit.pointer) {
            Some(slot) => *slot = edit.value.clone(),
            None => {
                return Err(ConformanceError::PointerNotFound {
                    case: case_id.to_string(),
                    pointer: edit.pointer.clone(),
                    document: which.to_string(),
                })
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum CaseOutcome {
    Passed,
    Failed {
        expectation: String,
        detail: String,
    },
    /// The implementation does not publish what this requirement observes.
    Unsupported {
        expectation: String,
        reason: String,
    },
    /// The case could not be run: a missing fixture, or an override targeting a location the
    /// fixture does not have. A defect in the suite, not in the implementation — but it still
    /// blocks certification, because an unrun requirement is not a satisfied one.
    Errored {
        reason: String,
    },
}

impl CaseOutcome {
    pub fn is_pass(&self) -> bool {
        matches!(self, CaseOutcome::Passed)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            CaseOutcome::Passed => "passed",
            CaseOutcome::Failed { .. } => "failed",
            CaseOutcome::Unsupported { .. } => "unsupported",
            CaseOutcome::Errored { .. } => "errored",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseResult {
    pub case_id: String,
    pub title: String,
    pub layer: Layer,
    pub requirement: Requirement,
    pub enforces: Vec<String>,
    pub invariant: String,
    /// Expectation kinds this case exercised, so the release gates can ask what a run actually
    /// covered instead of inferring it from a green tick.
    pub expectations: Vec<String>,
    pub outcome: CaseOutcome,
}

/// The machine-readable conformance bundle of 40.32.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuiteReport {
    pub schema_version: String,
    pub suite_id: String,
    pub suite_version: String,
    pub suite_digest: String,
    pub implementation: ImplementationIdentity,
    pub environment: EnvironmentManifest,
    pub fixture_manifest_id: String,
    pub fixture_count: usize,
    /// Fixtures declared synthetic, restated here because 40.33 invariant 2 ("synthetic does not
    /// imply realistic") has to travel with the result to do any work.
    pub synthetic_fixtures: Vec<String>,
    pub results: Vec<CaseResult>,
    pub fixture_drift: Vec<FixtureDrift>,
    /// The equal-engineering comparator required by 43.40's release gate. Declared by the caller;
    /// nothing here verifies that the comparison was competently run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<BaselineReference>,
    /// Cases that were re-run after a failure and then reported green. 40.31 invariant 4 forbids
    /// retrying a flaky test to green, so a non-empty list blocks release rather than being
    /// discarded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retried_to_green: Vec<String>,
}

impl SuiteReport {
    pub fn declaring_baseline(mut self, baseline: BaselineReference) -> Self {
        self.baseline = Some(baseline);
        self
    }

    pub fn noting_retry(mut self, case_id: impl Into<String>) -> Self {
        self.retried_to_green.push(case_id.into());
        self
    }

    pub fn passed(&self) -> usize {
        self.count_where(|r| matches!(r.outcome, CaseOutcome::Passed))
    }

    pub fn failed(&self) -> usize {
        self.count_where(|r| matches!(r.outcome, CaseOutcome::Failed { .. }))
    }

    pub fn unsupported(&self) -> usize {
        self.count_where(|r| matches!(r.outcome, CaseOutcome::Unsupported { .. }))
    }

    pub fn errored(&self) -> usize {
        self.count_where(|r| matches!(r.outcome, CaseOutcome::Errored { .. }))
    }

    fn count_where(&self, predicate: impl Fn(&CaseResult) -> bool) -> usize {
        self.results.iter().filter(|r| predicate(r)).count()
    }

    pub fn result(&self, case_id: &str) -> Option<&CaseResult> {
        self.results.iter().find(|r| r.case_id == case_id)
    }

    /// Every `must` requirement that did not pass, for any reason.
    pub fn unmet_requirements(&self) -> Vec<&CaseResult> {
        self.results
            .iter()
            .filter(|r| r.requirement == Requirement::Must && !r.outcome.is_pass())
            .collect()
    }

    pub fn is_fully_conformant(&self) -> bool {
        self.unmet_requirements().is_empty() && self.fixture_drift.is_empty()
    }

    pub fn pyramid(&self) -> PyramidShape {
        PyramidShape::from_layers(self.results.iter().map(|r| r.layer))
    }

    /// Whether some passing case actually exercised an expectation of this kind.
    ///
    /// The release gates use this rather than trusting that a green suite covered what it should:
    /// a suite can be entirely green and never once have recomputed a digest.
    pub fn covers(&self, expectation_kind: &str) -> bool {
        self.results.iter().any(|r| {
            r.outcome.is_pass() && r.expectations.iter().any(|k| k == expectation_kind)
        })
    }

    /// Whether some passing case at this layer exists.
    pub fn covers_layer(&self, layer: Layer) -> bool {
        self.results
            .iter()
            .any(|r| r.layer == layer && r.outcome.is_pass())
    }

    pub fn to_json(&self) -> Result<Value, ConformanceError> {
        serde_json::to_value(self).map_err(|e| ConformanceError::MalformedSuite(e.to_string()))
    }

    /// Issues a certificate, or refuses and says which requirement stopped it.
    ///
    /// `issued_at` and `expires_at` are supplied by the caller: this crate reads no clock, so it
    /// records the stamps it is given and does not evaluate them.
    pub fn certify(
        &self,
        issued_at: impl Into<String>,
        expires_at: impl Into<String>,
    ) -> Result<ConformanceCertificate, ConformanceError> {
        if let Some(drift) = self.fixture_drift.first() {
            return Err(ConformanceError::CertificationRefused {
                reason: format!("fixture drift: {drift}"),
            });
        }
        if let Some(unmet) = self.unmet_requirements().first() {
            return Err(ConformanceError::CertificationRefused {
                reason: format!(
                    "requirement {} ({}) is {}",
                    unmet.case_id,
                    unmet.invariant,
                    unmet.outcome.as_str()
                ),
            });
        }

        Ok(ConformanceCertificate {
            schema_version: CONFORMANCE_CERTIFICATE_SCHEMA_VERSION.to_string(),
            suite_id: self.suite_id.clone(),
            suite_version: self.suite_version.clone(),
            suite_digest: self.suite_digest.clone(),
            implementation: self.implementation.clone(),
            environment: self.environment.clone(),
            requirements: self
                .results
                .iter()
                .map(|r| RequirementStatus {
                    case_id: r.case_id.clone(),
                    requirement: r.requirement,
                    status: r.outcome.as_str().to_string(),
                    enforces: r.enforces.clone(),
                })
                .collect(),
            issued_at: issued_at.into(),
            expires_at: expires_at.into(),
        })
    }

    /// A short human summary. Kept alongside the JSON bundle because a reviewer reading CI output
    /// should not have to parse a report to learn that four requirements failed.
    pub fn summary(&self) -> String {
        let mut lines = vec![format!(
            "{} {} against {} {}: {} passed, {} failed, {} unsupported, {} errored",
            self.suite_id,
            self.suite_version,
            self.implementation.name,
            self.implementation.version,
            self.passed(),
            self.failed(),
            self.unsupported(),
            self.errored()
        )];
        for drift in &self.fixture_drift {
            lines.push(format!("  FIXTURE DRIFT {drift}"));
        }
        for result in self.unmet_requirements() {
            lines.push(match &result.outcome {
                CaseOutcome::Failed { expectation, detail } => {
                    format!("  FAILED {} [{expectation}] {detail}", result.case_id)
                }
                CaseOutcome::Unsupported { expectation, reason } => {
                    format!("  UNSUPPORTED {} [{expectation}] {reason}", result.case_id)
                }
                CaseOutcome::Errored { reason } => {
                    format!("  ERRORED {} {reason}", result.case_id)
                }
                CaseOutcome::Passed => unreachable!("unmet requirements are never passes"),
            });
        }
        lines.join("\n")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementStatus {
    pub case_id: String,
    pub requirement: Requirement,
    pub status: String,
    pub enforces: Vec<String>,
}

/// A certificate binding a suite version, an implementation identity and an environment.
///
/// The digest is computed over the body exactly as a Context Certificate's is (43.26), so the
/// same verification routine shape applies: recompute over everything but `certificate_sha256`
/// and compare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceCertificate {
    pub schema_version: String,
    pub suite_id: String,
    pub suite_version: String,
    pub suite_digest: String,
    pub implementation: ImplementationIdentity,
    pub environment: EnvironmentManifest,
    pub requirements: Vec<RequirementStatus>,
    pub issued_at: String,
    pub expires_at: String,
}

impl ConformanceCertificate {
    fn body(&self) -> Result<Value, ConformanceError> {
        serde_json::to_value(self).map_err(|e| ConformanceError::MalformedSuite(e.to_string()))
    }

    pub fn digest(&self) -> Result<String, ConformanceError> {
        let body = self.body()?;
        ContentHash::of_value(&body)
            .map(|hash| hash.as_str().to_string())
            .map_err(|e| ConformanceError::Canonical(e.to_string()))
    }

    pub fn to_json(&self) -> Result<Value, ConformanceError> {
        let body = self.body()?;
        let digest = self.digest()?;
        let mut map = body
            .as_object()
            .cloned()
            .unwrap_or_else(Map::new);
        map.insert("certificate_sha256".into(), json!(digest));
        Ok(Value::Object(map))
    }

    /// Recomputes the embedded digest, the way a registry ingesting a bundle must.
    pub fn verify(document: &Value) -> Result<(), ConformanceError> {
        let map = document.as_object().ok_or_else(|| {
            ConformanceError::MalformedSuite("certificate is not an object".to_string())
        })?;
        let claimed = map
            .get("certificate_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ConformanceError::MalformedSuite("certificate has no certificate_sha256".to_string())
            })?;
        let mut body = map.clone();
        body.remove("certificate_sha256");
        let recomputed = ContentHash::of_value(&Value::Object(body))
            .map_err(|e| ConformanceError::Canonical(e.to_string()))?;
        if recomputed.as_str() == claimed {
            Ok(())
        } else {
            Err(ConformanceError::CertificateDigestMismatch {
                claimed: claimed.to_string(),
                recomputed: recomputed.as_str().to_string(),
            })
        }
    }
}

enum Check {
    Pass,
    Fail(String),
    Unsupported(String),
}

fn evaluate(
    expectation: &Expectation,
    first: &Result<CompileArtifacts, CompileFailure>,
    second: Option<&Result<CompileArtifacts, CompileFailure>>,
    fixtures: &FixtureStore,
) -> Check {
    match check(expectation, first, second, fixtures) {
        Ok(()) => Check::Pass,
        Err(check) => check,
    }
}

fn check(
    expectation: &Expectation,
    first: &Result<CompileArtifacts, CompileFailure>,
    second: Option<&Result<CompileArtifacts, CompileFailure>>,
    fixtures: &FixtureStore,
) -> Result<(), Check> {
    if let Expectation::FailsWith { error_kind } = expectation {
        return match first {
            Ok(artifacts) => Err(Check::Fail(format!(
                "compilation succeeded and published {:?}; it must refuse with {error_kind:?}",
                artifacts.names()
            ))),
            Err(failure) if &failure.kind == error_kind => Ok(()),
            Err(failure) => Err(Check::Fail(format!(
                "refused with {:?} ({}), expected {error_kind:?}",
                failure.kind, failure.message
            ))),
        };
    }

    let artifacts = match first {
        Ok(artifacts) => artifacts,
        Err(failure) => {
            return Err(Check::Fail(format!(
                "compilation refused with {:?}: {}",
                failure.kind, failure.message
            )))
        }
    };

    match expectation {
        Expectation::FailsWith { .. } => unreachable!("handled above"),

        Expectation::ProducesArtifacts { artifacts: names } => {
            for name in names {
                if artifacts.get(name).is_none() {
                    return Err(Check::Unsupported(format!(
                        "no artifact named {name:?}; published {:?}",
                        artifacts.names()
                    )));
                }
            }
            Ok(())
        }

        Expectation::FieldEquals {
            artifact,
            pointer,
            value,
        } => {
            let found = at(doc(artifacts, artifact)?, artifact, pointer)?;
            if found == value {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{artifact}{pointer} is {found}, expected {value}"
                )))
            }
        }

        Expectation::FieldDiffersFrom {
            artifact,
            pointer,
            value,
        } => {
            let found = at(doc(artifacts, artifact)?, artifact, pointer)?;
            if found == value {
                Err(Check::Fail(format!(
                    "{artifact}{pointer} is still {value}; it had to change"
                )))
            } else {
                Ok(())
            }
        }

        Expectation::ArrayContains {
            artifact,
            pointer,
            values,
        } => {
            let members = strings(array(doc(artifacts, artifact)?, artifact, pointer)?, artifact, pointer)?;
            let missing: Vec<&String> = values.iter().filter(|v| !members.contains(*v)).collect();
            if missing.is_empty() {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{artifact}{pointer} is missing {missing:?}"
                )))
            }
        }

        Expectation::ArrayLenIs {
            artifact,
            pointer,
            len,
        } => {
            let items = array(doc(artifacts, artifact)?, artifact, pointer)?;
            if items.len() == *len {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{artifact}{pointer} has {} members, expected {len}",
                    items.len()
                )))
            }
        }

        Expectation::ArrayIsNonEmpty { artifact, pointer } => {
            let items = array(doc(artifacts, artifact)?, artifact, pointer)?;
            if items.is_empty() {
                Err(Check::Fail(format!("{artifact}{pointer} is empty")))
            } else {
                Ok(())
            }
        }

        Expectation::ArrayIsSubsetOf {
            artifact,
            subset,
            superset,
        } => {
            let document = doc(artifacts, artifact)?;
            let small = strings(array(document, artifact, subset)?, artifact, subset)?;
            let large = strings(array(document, artifact, superset)?, artifact, superset)?;
            let escaped: Vec<&String> = small.iter().filter(|m| !large.contains(*m)).collect();
            if escaped.is_empty() {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{escaped:?} appear in {artifact}{subset} but not in {artifact}{superset}"
                )))
            }
        }

        Expectation::NoArrayMemberStartsWith {
            artifact,
            pointer,
            prefix,
        } => {
            let members = strings(array(doc(artifacts, artifact)?, artifact, pointer)?, artifact, pointer)?;
            let offenders: Vec<&String> =
                members.iter().filter(|m| m.starts_with(prefix)).collect();
            if offenders.is_empty() {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{artifact}{pointer} contains {offenders:?}, which start with {prefix:?}"
                )))
            }
        }

        Expectation::ArrayProjectionContains {
            artifact,
            pointer,
            field,
            values,
        } => {
            let projected = project(doc(artifacts, artifact)?, artifact, pointer, field)?;
            let missing: Vec<&String> = values.iter().filter(|v| !projected.contains(*v)).collect();
            if missing.is_empty() {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{artifact}{pointer}[*].{field} is {projected:?} and omits {missing:?}"
                )))
            }
        }

        Expectation::ArrayProjectionEquals {
            artifact,
            pointer,
            field,
            values,
        } => {
            let projected = project(doc(artifacts, artifact)?, artifact, pointer, field)?;
            if &projected == values {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{artifact}{pointer}[*].{field} is {projected:?}, expected {values:?}"
                )))
            }
        }

        Expectation::ArrayPartitionsTotal {
            artifact,
            pointer,
            field,
            allowed,
            count_field,
            total,
        } => {
            let document = doc(artifacts, artifact)?;
            let expected = integer(document, artifact, total)?;
            let groups = array(document, artifact, pointer)?;
            let mut sum: i64 = 0;
            for (index, group) in groups.iter().enumerate() {
                let class = group.get(field).and_then(Value::as_str).ok_or_else(|| {
                    Check::Fail(format!(
                        "{artifact}{pointer}[{index}] has no string {field:?}"
                    ))
                })?;
                if !allowed.iter().any(|a| a == class) {
                    return Err(Check::Fail(format!(
                        "{artifact}{pointer}[{index}].{field} is {class:?}, outside {allowed:?}"
                    )));
                }
                let count = group.get(count_field).and_then(Value::as_i64).ok_or_else(|| {
                    Check::Fail(format!(
                        "{artifact}{pointer}[{index}] has no integer {count_field:?}"
                    ))
                })?;
                sum += count;
            }
            if sum == expected {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{artifact}{pointer} accounts for {sum} of the {expected} at {total}"
                )))
            }
        }

        Expectation::ArrayLenEqualsField {
            artifact,
            pointer,
            field_pointer,
        } => {
            let document = doc(artifacts, artifact)?;
            let len = array(document, artifact, pointer)?.len() as i64;
            let declared = integer(document, artifact, field_pointer)?;
            if len == declared {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{artifact}{pointer} has {len} members but {artifact}{field_pointer} declares {declared}"
                )))
            }
        }

        Expectation::IntegersReconcile {
            artifact,
            minuend,
            subtrahend,
            difference,
        } => {
            let document = doc(artifacts, artifact)?;
            let a = integer(document, artifact, minuend)?;
            let b = integer(document, artifact, subtrahend)?;
            let c = integer(document, artifact, difference)?;
            if a - b == c {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{a} at {minuend} minus {b} at {subtrahend} is {}, but {difference} says {c}",
                    a - b
                )))
            }
        }

        Expectation::IntegerIsAtMost {
            artifact,
            pointer,
            bound,
        } => {
            let document = doc(artifacts, artifact)?;
            let value = integer(document, artifact, pointer)?;
            let limit = integer(document, artifact, bound)?;
            if value <= limit {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{artifact}{pointer} is {value}, above the {limit} at {bound}"
                )))
            }
        }

        Expectation::EmbeddedDigestVerifies {
            artifact,
            digest_field,
        } => {
            let (claimed, recomputed) = embedded_digest(doc(artifacts, artifact)?, artifact, digest_field)?;
            if claimed == recomputed {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{artifact} claims {claimed} but recomputes to {recomputed}"
                )))
            }
        }

        Expectation::EmbeddedDigestIs {
            artifact,
            digest_field,
            sha256,
        } => {
            let document = doc(artifacts, artifact)?;
            let claimed = document
                .get(digest_field)
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    Check::Fail(format!("{artifact} has no string {digest_field:?}"))
                })?;
            if claimed == sha256 {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{artifact}.{digest_field} is {claimed}, expected {sha256}"
                )))
            }
        }

        Expectation::DocumentDigestIs { artifact, sha256 } => {
            let observed = canonical_digest(doc(artifacts, artifact)?)?;
            if &observed == sha256 {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{artifact} hashes to {observed}, expected {sha256}"
                )))
            }
        }

        Expectation::DigestBinds {
            artifact,
            pointer,
            target_artifact,
        } => {
            let recorded = at(doc(artifacts, artifact)?, artifact, pointer)?
                .as_str()
                .ok_or_else(|| Check::Fail(format!("{artifact}{pointer} is not a string")))?
                .to_string();
            let target = canonical_digest(doc(artifacts, target_artifact)?)?;
            if recorded == target {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{artifact}{pointer} records {recorded} but the delivered {target_artifact} hashes to {target}"
                )))
            }
        }

        Expectation::MatchesGoldenFixture { artifact, fixture } => {
            let golden = fixtures
                .get(fixture)
                .map_err(|e| Check::Fail(e.to_string()))?;
            let expected = canonical(golden)?;
            let observed = canonical(doc(artifacts, artifact)?)?;
            if expected == observed {
                Ok(())
            } else {
                Err(Check::Fail(format!(
                    "{artifact} diverged from golden fixture {fixture}: {}",
                    first_divergence(&expected, &observed)
                )))
            }
        }

        Expectation::RecompilationIsByteIdentical => {
            let repeat = second.ok_or_else(|| {
                Check::Fail("the runner did not perform a second compilation".to_string())
            })?;
            let repeat = match repeat {
                Ok(artifacts) => artifacts,
                Err(failure) => {
                    return Err(Check::Fail(format!(
                        "the second compilation refused with {:?} after the first succeeded",
                        failure.kind
                    )))
                }
            };
            let mut names: BTreeSet<&str> = artifacts.names().into_iter().collect();
            names.extend(repeat.names());
            for name in names {
                let a = artifacts.get(name).map(canonical).transpose()?;
                let b = repeat.get(name).map(canonical).transpose()?;
                if a != b {
                    return Err(Check::Fail(format!(
                        "artifact {name:?} differs between two compilations of the same inputs"
                    )));
                }
            }
            Ok(())
        }
    }
}

fn doc<'a>(artifacts: &'a CompileArtifacts, name: &str) -> Result<&'a Value, Check> {
    artifacts.get(name).ok_or_else(|| {
        Check::Unsupported(format!(
            "no artifact named {name:?}; published {:?}",
            artifacts.names()
        ))
    })
}

fn at<'a>(document: &'a Value, artifact: &str, pointer: &str) -> Result<&'a Value, Check> {
    document
        .pointer(pointer)
        .ok_or_else(|| Check::Fail(format!("{artifact} has nothing at {pointer}")))
}

fn array<'a>(document: &'a Value, artifact: &str, pointer: &str) -> Result<&'a Vec<Value>, Check> {
    at(document, artifact, pointer)?
        .as_array()
        .ok_or_else(|| Check::Fail(format!("{artifact}{pointer} is not an array")))
}

fn strings(items: &[Value], artifact: &str, pointer: &str) -> Result<Vec<String>, Check> {
    items
        .iter()
        .map(|item| {
            item.as_str().map(str::to_string).ok_or_else(|| {
                Check::Fail(format!("{artifact}{pointer} contains a non-string member"))
            })
        })
        .collect()
}

fn project(
    document: &Value,
    artifact: &str,
    pointer: &str,
    field: &str,
) -> Result<Vec<String>, Check> {
    array(document, artifact, pointer)?
        .iter()
        .enumerate()
        .map(|(index, item)| {
            item.get(field)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| {
                    Check::Fail(format!(
                        "{artifact}{pointer}[{index}] has no string {field:?}"
                    ))
                })
        })
        .collect()
}

fn integer(document: &Value, artifact: &str, pointer: &str) -> Result<i64, Check> {
    at(document, artifact, pointer)?
        .as_i64()
        .ok_or_else(|| Check::Fail(format!("{artifact}{pointer} is not an integer")))
}

fn canonical(document: &Value) -> Result<String, Check> {
    to_canonical_string(document).map_err(|e| Check::Fail(format!("canonical encoding: {e}")))
}

fn canonical_digest(document: &Value) -> Result<String, Check> {
    ContentHash::of_value(document)
        .map(|hash| hash.as_str().to_string())
        .map_err(|e| Check::Fail(format!("canonical encoding: {e}")))
}

fn embedded_digest(
    document: &Value,
    artifact: &str,
    digest_field: &str,
) -> Result<(String, String), Check> {
    let map = document
        .as_object()
        .ok_or_else(|| Check::Fail(format!("{artifact} is not an object")))?;
    let claimed = map
        .get(digest_field)
        .and_then(Value::as_str)
        .ok_or_else(|| Check::Fail(format!("{artifact} has no string {digest_field:?}")))?
        .to_string();
    let mut body = map.clone();
    body.remove(digest_field);
    let recomputed = canonical_digest(&Value::Object(body))?;
    Ok((claimed, recomputed))
}

/// Where two canonical encodings first differ, with a little context on each side.
///
/// A golden mismatch on a 4.5 KB section is unreadable as two whole documents; the byte offset
/// and the surrounding text is what a reviewer actually needs to find the changed field.
fn first_divergence(expected: &str, observed: &str) -> String {
    let offset = expected
        .bytes()
        .zip(observed.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| expected.len().min(observed.len()));
    format!(
        "first difference at byte {offset}; expected ...{}... observed ...{}...",
        window(expected, offset),
        window(observed, offset)
    )
}

/// Forty bytes either side of `offset`, snapped outward to character boundaries.
fn window(text: &str, offset: usize) -> &str {
    let start = (0..=offset.saturating_sub(40).min(text.len()))
        .rev()
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(0);
    let end = ((offset + 40).min(text.len())..=text.len())
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(text.len());
    &text[start..end]
}
