//! A deterministic schema oracle over a FIBER world document (31.02).
//!
//! Where [`crate::SchemaOracle`](super::SchemaOracle) checks fields the caller declares, this one
//! delegates to `bioprism_world::World::from_json` — the acceptance check the runtime itself
//! applies. That makes it a genuine artifact oracle in 31.02's sense rather than a configurable
//! approximation of one: whatever the world parser rejects, this oracle contradicts, and the two
//! cannot drift apart because there is only one implementation.
//!
//! It reads the world from a single evidence field, defaulting to `world`, and treats a parse
//! failure as [`Finding::Malformed`] carrying the parser's own message.
//!
//! Not implemented: the structural diagnostics of `bioprism_world::validate`, which would let this
//! oracle surface shadowed variables and leakage canaries as well as parse failures. Those need a
//! `DimensionRegistry`, which lives in a crate this one deliberately does not depend on.

use crate::error::OracleError;
use crate::evidence::Evidence;
use crate::judgement::{Confidence, Finding, Judgement, Position};
use crate::ladder::EvidenceTier;
use crate::manifest::{OracleId, OracleManifest, OracleRef, OracleVersion};
use crate::oracle::Oracle;
use crate::plane::Plane;
use crate::time::ValidityWindow;
use bioprism_world::World;

/// Parses an embedded world document and contradicts anything the runtime would reject.
pub struct WorldDocumentOracle {
    manifest: OracleManifest,
    pointer: String,
}

impl WorldDocumentOracle {
    /// Builds a world-document oracle at [`EvidenceTier::Deterministic`], establishing only
    /// [`Plane::Artifact`].
    pub fn new(
        id: impl Into<String>,
        version: OracleVersion,
        validity: ValidityWindow,
    ) -> Result<Self, OracleError> {
        let manifest = OracleManifest::new(
            OracleRef::new(OracleId::parse(id)?, version),
            EvidenceTier::Deterministic,
            [Plane::Artifact],
            [],
            validity,
        )?
        .disclaiming_the_rest()
        .with_failure_mode(
            "a world that parses is well formed, not correct; nothing here checks that its facts \
             describe anything real",
        );

        Ok(WorldDocumentOracle {
            manifest,
            pointer: "world".to_string(),
        })
    }

    /// Reads the world from a different evidence field.
    pub fn at_pointer(mut self, pointer: impl Into<String>) -> Self {
        self.pointer = pointer.into();
        self
    }

    pub fn manifest_mut(&mut self) -> &mut OracleManifest {
        &mut self.manifest
    }
}

impl Oracle for WorldDocumentOracle {
    fn manifest(&self) -> &OracleManifest {
        &self.manifest
    }

    fn evaluate(&self, evidence: &Evidence) -> Result<Judgement, OracleError> {
        let Some(document) = evidence.field(&self.pointer) else {
            return Ok(Judgement::from_manifest(
                &self.manifest,
                &evidence.at,
                Position::NotEvaluable,
                Confidence::CERTAIN,
            )
            .with_rationale(format!(
                "no world document at {:?}; this evidence is not a world",
                self.pointer
            )));
        };

        match World::from_json(document.clone()) {
            Ok(world) => Ok(Judgement::from_manifest(
                &self.manifest,
                &evidence.at,
                Position::Supported,
                Confidence::CERTAIN,
            )
            .with_rationale(format!(
                "world {} parses against the runtime's own acceptance checks",
                world.world_id
            ))),
            Err(error) => Ok(Judgement::from_manifest(
                &self.manifest,
                &evidence.at,
                Position::Contradicted,
                Confidence::CERTAIN,
            )
            .with_findings([Finding::Malformed {
                pointer: self.pointer.clone(),
                detail: error.to_string(),
            }])
            .with_rationale("the world document was rejected by the runtime parser")),
        }
    }
}
