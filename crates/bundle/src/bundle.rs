//! The bundle itself: a manifest plus the content it claims, and a verifier that believes neither.
//!
//! Blueprint 34.14 (Result Cards, Signed Bundles and Reproduction) requires that "every public claim
//! resolves to configuration, worlds, runs, oracles, and statistical code", that "every rendered
//! score resolves to immutable result objects", and that a new user can "verify the signed result
//! bundle without creating an account". 13.15's first invariant requires every reported result to be
//! linked to an immutable run, pack, architecture, model, evaluator and environment version.
//!
//! # More than a tarball
//!
//! A tarball plus a hash file proves that a file list has not changed since someone wrote the hash
//! file. A bundle carries the 43.26 Context Certificate, the 43.25 Decision Section, the world and
//! query digests, the toolchain and crate versions, the declared environment, and the recorded
//! provenance of every input — and hashes all of it into one manifest whose bytes a single tag
//! covers.
//!
//! # The invariant
//!
//! **Verification recomputes every digest the bundle contains. It never reads a digest out of the
//! manifest and calls that a check.** A manifest is a *claim* about content; a verifier that trusts
//! it is checking the claim against itself. [`ResultBundle::verify`] hashes the carried content and
//! compares, and a disagreement is [`crate::BundleError::EntryDigestMismatch`], which names the
//! entry, the manifest's digest and the recomputed one.
//!
//! The order in [`AttestedBundle::verify`] is deliberate and is the other half of the same point:
//! recompute first, authenticate second. Authenticating first and then trusting the manifest would
//! prove that a key holder committed to a list of digests, and prove nothing about the bytes in the
//! bundle.
//!
//! Three things verification checks that a digest comparison alone would miss:
//!
//! - Content carried under a name the manifest does not list is [`crate::BundleError::UnlistedContent`].
//!   An unlisted payload is outside the authenticated closure, so a tag says nothing about it, and
//!   silently ignoring it would let a bundle smuggle bytes past its own attestation.
//! - A manifest entry marked inline whose content is missing is
//!   [`crate::BundleError::MissingInlineContent`], not a skipped check.
//! - A carried Context Certificate must still self-verify under 43.26's own rule, because its
//!   `certificate_sha256` is a digest the bundle contains.
//!
//! # Deliberately not implemented
//!
//! No serialization to an archive format, no filesystem or network access, no fetching of referenced
//! entries, no compression, no encryption or confidentiality of any kind (a bundle is plaintext, and
//! 34.14's disclosure-protection requirement for small cohorts is not addressed here), no access
//! tiers, and no `health.status` lifecycle from 34.14's API object — this crate has no clock with
//! which to call anything stale.

use crate::attestation::{
    Attestation, AttestationPurpose, ClaimedProducer, KeyHolderAuthenticated,
};
use crate::environment::{EnvironmentFacts, ToolchainFacts};
use crate::error::BundleError;
use crate::mac::SecretKey;
use crate::manifest::{BundleManifest, EntryBody, EntryRole, ManifestEntry, BUNDLE_SCHEMA_VERSION};
use crate::provenance::{ProvenanceState, SupplyChainPosture};
use crate::signature::{PublicKeyAttestation, SigningKey, VerificationKey};
use crate::trust::{KeyRegistry, TrustPolicy, TrustReport};
use bioprism_ids::ContentHash;
use bioprism_section::{CertificateProfile, CertificateVerification, ContextCertificate};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// A manifest and the content it lists.
///
/// Contents are a sorted map so that a bundle's own serialization is deterministic; the manifest is
/// what a tag covers, and the contents are what verification hashes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultBundle {
    pub manifest: BundleManifest,
    pub contents: BTreeMap<String, Value>,
}

impl ResultBundle {
    pub fn builder(bundle_id: impl Into<String>) -> BundleBuilder {
        BundleBuilder {
            bundle_id: bundle_id.into(),
            entries: Vec::new(),
            contents: BTreeMap::new(),
            environment: EnvironmentFacts::undeclared(),
            toolchain: ToolchainFacts::declared(),
        }
    }

    /// Recomputes every digest in the bundle and reports what could and could not be recomputed.
    ///
    /// See the module docs. A mismatch is an error naming the entry; an entry whose content did not
    /// travel is [`EntryCheck::NotCarried`], never a pass.
    pub fn verify(&self) -> Result<VerifiedBundle, BundleError> {
        for name in self.contents.keys() {
            if self.manifest.entry(name).is_none() {
                return Err(BundleError::UnlistedContent {
                    entry: name.clone(),
                });
            }
        }

        let mut checks = BTreeMap::new();
        for entry in &self.manifest.entries {
            let check = match &entry.body {
                EntryBody::Inline => {
                    let Some(content) = self.contents.get(&entry.name) else {
                        return Err(BundleError::MissingInlineContent {
                            entry: entry.name.clone(),
                        });
                    };
                    let recomputed = ContentHash::of_value(content)?;
                    if recomputed != entry.digest {
                        return Err(BundleError::EntryDigestMismatch {
                            entry: entry.name.clone(),
                            claimed: entry.digest.as_str().to_string(),
                            recomputed: recomputed.as_str().to_string(),
                        });
                    }
                    EntryCheck::Recomputed { digest: recomputed }
                }
                EntryBody::Reference { .. } => EntryCheck::NotCarried {
                    digest: entry.digest.clone(),
                },
            };
            checks.insert(entry.name.clone(), check);
        }

        let certificate = self.verify_embedded_certificate()?;

        Ok(VerifiedBundle {
            manifest_digest: self.manifest.digest()?,
            entry_checks: checks,
            certificate,
            posture: self.manifest.supply_chain_posture(),
        })
    }

    fn verify_embedded_certificate(&self) -> Result<EmbeddedCertificate, BundleError> {
        let certificates: Vec<&ManifestEntry> = self
            .manifest
            .entries
            .iter()
            .filter(|entry| entry.role == EntryRole::ContextCertificate)
            .collect();
        let entry = match certificates.as_slice() {
            [] => return Ok(EmbeddedCertificate::Absent),
            [only] => *only,
            other => {
                return Err(BundleError::RoleCardinality {
                    role: EntryRole::ContextCertificate.to_string(),
                    found: other.len(),
                })
            }
        };
        if !entry.body.is_inline() {
            return Ok(EmbeddedCertificate::NotCarried);
        }
        let document = self
            .contents
            .get(&entry.name)
            .expect("inline entries were checked for presence above");
        match ContextCertificate::verify(document)? {
            CertificateVerification::Valid => Ok(EmbeddedCertificate::SelfVerified),
            CertificateVerification::DigestMismatch {
                claimed,
                recomputed,
            } => Err(BundleError::EmbeddedCertificateInvalid {
                entry: entry.name.clone(),
                detail: format!(
                    "certificate_sha256 says {claimed} but its body hashes to {recomputed}"
                ),
            }),
            CertificateVerification::Malformed(reason) => {
                Err(BundleError::EmbeddedCertificateInvalid {
                    entry: entry.name.clone(),
                    detail: reason,
                })
            }
        }
    }
}

/// Assembles a bundle, computing each entry's digest from the content rather than accepting one.
pub struct BundleBuilder {
    bundle_id: String,
    entries: Vec<ManifestEntry>,
    contents: BTreeMap<String, Value>,
    environment: EnvironmentFacts,
    toolchain: ToolchainFacts,
}

impl BundleBuilder {
    /// Adds content that travels with the bundle. The digest is computed here, so a builder cannot
    /// produce a manifest that disagrees with its own contents.
    pub fn carrying(
        mut self,
        name: impl Into<String>,
        role: EntryRole,
        content: Value,
    ) -> Result<Self, BundleError> {
        let name = name.into();
        let digest = ContentHash::of_value(&content)?;
        self.entries
            .push(ManifestEntry::new(name.clone(), role, digest));
        self.contents.insert(name, content);
        Ok(self)
    }

    /// [`BundleBuilder::carrying`] with 13.15 provenance attached to the entry.
    pub fn carrying_with_provenance(
        self,
        name: impl Into<String>,
        role: EntryRole,
        content: Value,
        provenance: ProvenanceState,
    ) -> Result<Self, BundleError> {
        let name = name.into();
        let mut builder = self.carrying(name.clone(), role, content)?;
        if let Some(entry) = builder.entries.iter_mut().find(|entry| entry.name == name) {
            entry.provenance = provenance;
        }
        Ok(builder)
    }

    /// Adds a 43.26 Context Certificate, rendered through `bioprism-section`'s own serializer so the
    /// bytes in the bundle are the bytes the certificate defines.
    pub fn carrying_certificate(
        self,
        name: impl Into<String>,
        certificate: &ContextCertificate,
        profile: CertificateProfile,
    ) -> Result<Self, BundleError> {
        let document = certificate.to_json(profile)?;
        self.carrying(name, EntryRole::ContextCertificate, document)
    }

    /// Records an entry by digest without carrying its content. See [`EntryBody::Reference`].
    pub fn referencing(
        mut self,
        name: impl Into<String>,
        role: EntryRole,
        digest: ContentHash,
        locator: Option<String>,
        provenance: ProvenanceState,
    ) -> Self {
        self.entries.push(
            ManifestEntry::new(name, role, digest)
                .referenced(locator)
                .with_provenance(provenance),
        );
        self
    }

    pub fn in_environment(mut self, environment: EnvironmentFacts) -> Self {
        self.environment = environment;
        self
    }

    pub fn with_toolchain(mut self, toolchain: ToolchainFacts) -> Self {
        self.toolchain = toolchain;
        self
    }

    pub fn build(self) -> Result<ResultBundle, BundleError> {
        let manifest = BundleManifest::new(
            self.bundle_id,
            self.entries,
            self.environment,
            self.toolchain,
        )?;
        Ok(ResultBundle {
            manifest,
            contents: self.contents,
        })
    }
}

/// One entry's verification outcome. A mismatch is not here; it is an error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "check", rename_all = "snake_case")]
pub enum EntryCheck {
    /// Content travelled with the bundle; its digest was recomputed and matched.
    Recomputed { digest: ContentHash },
    /// Only a digest travelled. Nothing was recomputed, and this is not a pass.
    NotCarried { digest: ContentHash },
}

impl EntryCheck {
    /// True only when the digest was recomputed from content this bundle actually carries.
    pub fn was_recomputed(&self) -> bool {
        matches!(self, EntryCheck::Recomputed { .. })
    }
}

/// Whether the bundle's Context Certificate satisfied 43.26's own self-verification.
///
/// A certificate that fails is not a state here; it is [`BundleError::EmbeddedCertificateInvalid`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddedCertificate {
    /// A certificate travelled inline and its `certificate_sha256` matched its body.
    SelfVerified,
    /// A certificate entry exists but only by digest, so nothing was recomputed.
    NotCarried,
    /// The bundle has no certificate entry at all.
    Absent,
}

/// The result of recomputing a bundle. Private fields, minted only by [`ResultBundle::verify`].
///
/// `Serialize` but not `Deserialize`: a verification result must be produced by verifying, never
/// parsed out of a document that asserts it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifiedBundle {
    manifest_digest: ContentHash,
    entry_checks: BTreeMap<String, EntryCheck>,
    certificate: EmbeddedCertificate,
    posture: SupplyChainPosture,
}

impl VerifiedBundle {
    pub fn manifest_digest(&self) -> &ContentHash {
        &self.manifest_digest
    }

    pub fn entry_checks(&self) -> &BTreeMap<String, EntryCheck> {
        &self.entry_checks
    }

    pub fn certificate(&self) -> EmbeddedCertificate {
        self.certificate
    }

    /// 13.15's three-state provenance picture, never collapsed to a single verdict.
    pub fn supply_chain_posture(&self) -> &SupplyChainPosture {
        &self.posture
    }

    /// Entries whose digests could not be recomputed because their content did not travel.
    pub fn not_recomputed(&self) -> Vec<&str> {
        self.entry_checks
            .iter()
            .filter(|(_, check)| !check.was_recomputed())
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// The sentence a surface must print. Never claims more than was recomputed.
    pub fn honest_label(&self) -> String {
        let recomputed = self.entry_checks.len() - self.not_recomputed().len();
        format!(
            "{recomputed} of {} entries recomputed from carried content; {} recorded by digest only; {}",
            self.entry_checks.len(),
            self.not_recomputed().len(),
            self.posture.honest_label()
        )
    }
}

/// A bundle with an HMAC attestation over its manifest digest.
///
/// This compatibility wrapper is intentionally distinct from [`PubliclyAttestedBundle`], whose
/// Ed25519 signature can be verified with public material only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestedBundle {
    pub bundle: ResultBundle,
    pub attestation: Attestation,
}

/// A bundle with an Ed25519 attestation that a verifier can check using public material only.
///
/// This is deliberately a separate type from [`AttestedBundle`]. Existing HMAC bundles remain
/// valid and retain their weaker semantics; a caller must opt into the stronger public-key wire
/// contract rather than having a deserializer reinterpret an old MAC as a signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PubliclyAttestedBundle {
    pub bundle: ResultBundle,
    pub attestation: PublicKeyAttestation,
}

impl PubliclyAttestedBundle {
    pub fn produce(
        bundle: ResultBundle,
        key: &SigningKey,
        purpose: AttestationPurpose,
        claimed_producer: ClaimedProducer,
    ) -> Result<Self, BundleError> {
        Self::produce_with(bundle, key, purpose, claimed_producer, None, None, None)
    }

    pub fn produce_with(
        bundle: ResultBundle,
        key: &SigningKey,
        purpose: AttestationPurpose,
        claimed_producer: ClaimedProducer,
        nonce: Option<String>,
        recorded_at: Option<String>,
        signed_at: Option<u64>,
    ) -> Result<Self, BundleError> {
        let verified = bundle.verify()?;
        let attestation = PublicKeyAttestation::produce_with(
            purpose,
            verified.manifest_digest().clone(),
            key,
            claimed_producer,
            nonce,
            recorded_at,
            signed_at,
        )?;
        Ok(Self {
            bundle,
            attestation,
        })
    }

    /// Recomputes the bundle before checking the public-key signature.
    pub fn verify(
        &self,
        key: &VerificationKey,
    ) -> Result<(VerifiedBundle, KeyHolderAuthenticated), BundleError> {
        let verified = self.bundle.verify()?;
        if &self.attestation.subject_digest != verified.manifest_digest() {
            return Err(BundleError::AttestationCoversDifferentManifest {
                attested: self.attestation.subject_digest.as_str().to_string(),
                actual: verified.manifest_digest().as_str().to_string(),
            });
        }
        let authenticated = self
            .attestation
            .verify_for_or_error(key, self.attestation.purpose)?;
        Ok((verified, authenticated))
    }

    /// Recomputes the bundle, verifies its Ed25519 attestation, and then applies a caller-supplied
    /// registry-backed lifecycle policy. The direct [`Self::verify`] method remains available for
    /// callers that intentionally only have one verification key and do not want registry policy.
    pub fn verify_with_registry(
        &self,
        registry: &KeyRegistry,
        policy: &TrustPolicy,
    ) -> Result<(VerifiedBundle, KeyHolderAuthenticated, TrustReport), BundleError> {
        let verified = self.bundle.verify()?;
        if &self.attestation.subject_digest != verified.manifest_digest() {
            return Err(BundleError::AttestationCoversDifferentManifest {
                attested: self.attestation.subject_digest.as_str().to_string(),
                actual: verified.manifest_digest().as_str().to_string(),
            });
        }
        let (authenticated, report) = registry
            .verify_attestation(&self.attestation, policy)
            .map_err(|error| BundleError::TrustPolicyRejected {
                detail: error.to_string(),
            })?;
        Ok((verified, authenticated, report))
    }

    pub fn schema_version(&self) -> &str {
        BUNDLE_SCHEMA_VERSION
    }
}

impl AttestedBundle {
    /// Attests a bundle under `key`, after recomputing it — a producer that attests a bundle it has
    /// not checked is committing a key to bytes it has not read.
    pub fn produce(
        bundle: ResultBundle,
        key: &SecretKey,
        purpose: AttestationPurpose,
        claimed_producer: ClaimedProducer,
    ) -> Result<Self, BundleError> {
        let verified = bundle.verify()?;
        let attestation = Attestation::produce(
            purpose,
            verified.manifest_digest().clone(),
            key,
            claimed_producer,
        )?;
        Ok(AttestedBundle {
            bundle,
            attestation,
        })
    }

    /// Recomputes the bundle, then checks the tag. In that order; see the module docs.
    pub fn verify(
        &self,
        key: &SecretKey,
    ) -> Result<(VerifiedBundle, KeyHolderAuthenticated), BundleError> {
        let verified = self.bundle.verify()?;
        if &self.attestation.subject_digest != verified.manifest_digest() {
            return Err(BundleError::AttestationCoversDifferentManifest {
                attested: self.attestation.subject_digest.as_str().to_string(),
                actual: verified.manifest_digest().as_str().to_string(),
            });
        }
        let authenticated = self
            .attestation
            .verify_for_or_error(key, self.attestation.purpose)?;
        Ok((verified, authenticated))
    }

    pub fn schema_version(&self) -> &str {
        BUNDLE_SCHEMA_VERSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mac::{AuthenticationScheme, KeyIdentity, Repudiability};
    use crate::provenance::RecordedProvenance;
    use crate::signature::KeyValidity;
    use crate::trust::{KeyRole, RegisteredKey, TrustVerdict};
    use serde_json::json;

    fn key() -> SecretKey {
        SecretKey::new(KeyIdentity::new("hub-2026"), vec![0x44; 32])
    }

    fn signing_key() -> SigningKey {
        SigningKey::new(KeyIdentity::new("hub-ed25519"), [0x24; 32])
    }

    fn certificate() -> ContextCertificate {
        use bioprism_section::{
            Backend, InfluenceClass, OmissionGroup, OmissionManifest, OracleStatus, OracleVerdict,
            PlanDescriptor, ReferenceOmissions, SourceHashes,
        };
        ContextCertificate {
            world_id: "world-1".into(),
            query_id: "query-1".into(),
            selected_facts: vec!["f1".into()],
            selected_factors: vec!["x1".into()],
            protected_closure: vec!["f1".into()],
            omissions: ReferenceOmissions {
                total_facts: 3,
                exploratory_facts: 2,
                classification: "bounded".into(),
                inaccessible_selected_before_cut: vec![],
            },
            plan: PlanDescriptor {
                backend: Backend::BackwardFactorSliceReference,
                compiled_factor_count: 1,
                compiled_fact_count: 1,
                total_factor_count: 2,
                total_fact_count: 3,
                max_selected_factor_arity: 2,
                fallback: None,
            },
            oracle: OracleVerdict {
                status: OracleStatus::Valid,
                witnesses: vec![],
                oracle_kind: "split_integrity".into(),
            },
            source_hashes: SourceHashes {
                world_sha256: "0".repeat(64),
                query_sha256: "1".repeat(64),
                decision_section_sha256: "2".repeat(64),
            },
            limitations: vec!["reference world".into()],
            manifest: OmissionManifest {
                groups: vec![OmissionGroup {
                    reason: "unreachable from the target".into(),
                    influence: InfluenceClass::Zero,
                    count: 2,
                    bound: None,
                    examples: vec!["f2".into()],
                }],
            },
        }
    }

    fn bundle() -> ResultBundle {
        ResultBundle::builder("run-1")
            .carrying_certificate("certificate", &certificate(), CertificateProfile::Reference)
            .expect("carries the certificate")
            .carrying(
                "section",
                EntryRole::DecisionSection,
                json!({"layers": ["l0", "l1"]}),
            )
            .expect("carries the section")
            .referencing(
                "world",
                EntryRole::World,
                ContentHash::of_bytes(b"world bytes"),
                Some("registry://worlds/world-1".into()),
                ProvenanceState::Recorded(
                    RecordedProvenance::new(
                        "registry://worlds",
                        ContentHash::of_bytes(b"world bytes"),
                    )
                    .pinned_at("rev-7"),
                ),
            )
            .carrying("query", EntryRole::Query, json!({"target": "t"}))
            .expect("carries the query")
            .in_environment(
                EnvironmentFacts::undeclared()
                    .with_os("linux")
                    .with_arch("x86_64"),
            )
            .with_toolchain(ToolchainFacts::declared().with_rustc_version("1.85.0"))
            .build()
            .expect("builds")
    }

    #[test]
    fn verification_recomputes_carried_digests_rather_than_reading_the_manifest() {
        let verified = bundle().verify().expect("verifies");
        assert!(verified.entry_checks()["certificate"].was_recomputed());
        assert!(verified.entry_checks()["section"].was_recomputed());
        assert_eq!(
            verified.entry_checks()["section"],
            EntryCheck::Recomputed {
                digest: ContentHash::of_value(&json!({"layers": ["l0", "l1"]})).expect("hashes")
            }
        );
    }

    #[test]
    fn a_manifest_that_disagrees_with_its_contents_names_the_offending_entry() {
        let mut tampered = bundle();
        tampered
            .contents
            .insert("section".into(), json!({"layers": ["l0"]}));
        let error = tampered.verify().expect_err("must not verify");
        match error {
            BundleError::EntryDigestMismatch {
                entry,
                claimed,
                recomputed,
            } => {
                assert_eq!(entry, "section");
                assert_ne!(claimed, recomputed);
            }
            other => panic!("expected an entry mismatch, got {other:?}"),
        }
    }

    #[test]
    fn a_referenced_entry_is_reported_as_not_recomputed_rather_than_as_a_pass() {
        let verified = bundle().verify().expect("verifies");
        assert!(!verified.entry_checks()["world"].was_recomputed());
        assert_eq!(verified.not_recomputed(), vec!["world"]);
        assert!(verified.honest_label().contains("recorded by digest only"));
    }

    #[test]
    fn content_the_manifest_does_not_list_is_refused_rather_than_ignored() {
        let mut smuggled = bundle();
        smuggled
            .contents
            .insert("extra".into(), json!({"payload": "unlisted"}));
        assert_eq!(
            smuggled.verify().expect_err("must not verify"),
            BundleError::UnlistedContent {
                entry: "extra".into()
            }
        );
    }

    #[test]
    fn an_inline_entry_whose_content_is_missing_is_an_error_not_a_skipped_check() {
        let mut stripped = bundle();
        stripped.contents.remove("query");
        assert_eq!(
            stripped.verify().expect_err("must not verify"),
            BundleError::MissingInlineContent {
                entry: "query".into()
            }
        );
    }

    #[test]
    fn a_carried_certificate_must_still_satisfy_its_own_self_verification() {
        let verified = bundle().verify().expect("verifies");
        assert_eq!(verified.certificate(), EmbeddedCertificate::SelfVerified);

        let mut broken = bundle();
        let document = broken.contents.get_mut("certificate").expect("present");
        document
            .as_object_mut()
            .expect("object")
            .insert("certificate_sha256".into(), json!("f".repeat(64)));
        let digest = ContentHash::of_value(document).expect("hashes");
        broken
            .manifest
            .entries
            .iter_mut()
            .find(|entry| entry.name == "certificate")
            .expect("present")
            .digest = digest;

        match broken.verify().expect_err("must not verify") {
            BundleError::EmbeddedCertificateInvalid { entry, .. } => {
                assert_eq!(entry, "certificate")
            }
            other => panic!("expected a certificate failure, got {other:?}"),
        }
    }

    #[test]
    fn a_bundle_without_a_certificate_reports_absent_rather_than_verified() {
        let bare = ResultBundle::builder("run-2")
            .carrying("section", EntryRole::DecisionSection, json!({"layers": []}))
            .expect("carries")
            .build()
            .expect("builds");
        assert_eq!(
            bare.verify().expect("verifies").certificate(),
            EmbeddedCertificate::Absent
        );
    }

    #[test]
    fn an_attested_bundle_verifies_and_names_the_key_that_produced_it() {
        let attested = AttestedBundle::produce(
            bundle(),
            &key(),
            AttestationPurpose::PublisherManifest,
            ClaimedProducer::new("AURORA Working Group"),
        )
        .expect("attests");
        let (verified, authenticated) = attested.verify(&key()).expect("verifies");
        assert_eq!(authenticated.subject_digest(), verified.manifest_digest());
        assert_eq!(authenticated.key_identity().as_str(), "hub-2026");
    }

    #[test]
    fn a_publicly_attested_bundle_verifies_after_transport_with_public_material_only() {
        let signing = signing_key();
        let public = signing.verification_key(KeyValidity::unbounded());
        let attested = PubliclyAttestedBundle::produce_with(
            bundle(),
            &signing,
            AttestationPurpose::PublisherManifest,
            ClaimedProducer::new("AURORA Working Group"),
            Some("bundle-nonce".into()),
            Some("2026-08-18T20:00:00Z".into()),
            Some(1_755_552_000),
        )
        .expect("attests");
        let wire = serde_json::to_string(&attested).expect("serialises");
        let received: PubliclyAttestedBundle = serde_json::from_str(&wire).expect("deserialises");
        let (verified, authenticated) = received.verify(&public).expect("verifies");
        assert_eq!(
            authenticated.scheme(),
            AuthenticationScheme::Ed25519PublicKey
        );
        assert_eq!(
            authenticated.repudiability(),
            Repudiability::NotForgeableByVerifier
        );
        assert_eq!(authenticated.subject_digest(), verified.manifest_digest());
        assert!(authenticated
            .honest_label()
            .contains("third parties can verify"));
    }

    #[test]
    fn a_publicly_attested_bundle_can_apply_registry_backed_policy() {
        let signing = signing_key();
        let public = signing.verification_key(KeyValidity::unbounded());
        let attested = PubliclyAttestedBundle::produce_with(
            bundle(),
            &signing,
            AttestationPurpose::PublisherManifest,
            ClaimedProducer::new("AURORA Working Group"),
            None,
            Some("2026-08-18T20:00:00Z".into()),
            Some(1_755_552_000),
        )
        .expect("attests");
        let mut registry = KeyRegistry::new();
        registry
            .register_root(
                RegisteredKey::root(
                    public.identity().clone(),
                    public.public_key(),
                    KeyValidity::unbounded(),
                    "AURORA Working Group",
                    [KeyRole::Publisher].into_iter().collect(),
                    std::collections::BTreeSet::new(),
                )
                .expect("registered key"),
            )
            .expect("root registration");
        let (verified, authenticated, report) = attested
            .verify_with_registry(
                &registry,
                &TrustPolicy::for_purpose(AttestationPurpose::PublisherManifest),
            )
            .expect("registry policy verifies");
        assert_eq!(authenticated.key_identity().as_str(), "hub-ed25519");
        assert_eq!(verified.manifest_digest(), authenticated.subject_digest());
        assert_eq!(report.verdict, TrustVerdict::Trusted);
        assert_eq!(report.producer, "AURORA Working Group");
    }

    #[test]
    fn a_public_signature_does_not_survive_content_or_manifest_rewriting() {
        let signing = signing_key();
        let public = signing.verification_key(KeyValidity::unbounded());
        let attested = PubliclyAttestedBundle::produce(
            bundle(),
            &signing,
            AttestationPurpose::PublisherManifest,
            ClaimedProducer::new("AURORA Working Group"),
        )
        .expect("attests");
        let mut rewritten = attested.clone();
        rewritten
            .bundle
            .contents
            .insert("query".into(), json!({"target": "rewritten"}));
        assert!(matches!(
            rewritten.verify(&public),
            Err(BundleError::EntryDigestMismatch { entry, .. }) if entry == "query"
        ));
        let mut relabelled = attested;
        relabelled.bundle.manifest.environment.os = Some("darwin".into());
        assert!(matches!(
            relabelled.verify(&public),
            Err(BundleError::AttestationCoversDifferentManifest { .. })
        ));
    }

    #[test]
    fn tampering_with_content_is_caught_before_the_tag_is_ever_checked() {
        let mut attested = AttestedBundle::produce(
            bundle(),
            &key(),
            AttestationPurpose::PublisherManifest,
            ClaimedProducer::new("AURORA Working Group"),
        )
        .expect("attests");
        attested
            .bundle
            .contents
            .insert("query".into(), json!({"target": "other"}));
        match attested.verify(&key()).expect_err("must not verify") {
            BundleError::EntryDigestMismatch { entry, .. } => assert_eq!(entry, "query"),
            other => panic!("recomputation must precede authentication, got {other:?}"),
        }
    }

    #[test]
    fn rewriting_a_manifest_entry_and_its_content_together_still_breaks_the_attestation() {
        let mut attested = AttestedBundle::produce(
            bundle(),
            &key(),
            AttestationPurpose::PublisherManifest,
            ClaimedProducer::new("AURORA Working Group"),
        )
        .expect("attests");
        let replacement = json!({"target": "other"});
        let digest = ContentHash::of_value(&replacement).expect("hashes");
        attested.bundle.contents.insert("query".into(), replacement);
        attested
            .bundle
            .manifest
            .entries
            .iter_mut()
            .find(|entry| entry.name == "query")
            .expect("present")
            .digest = digest;
        match attested.verify(&key()).expect_err("must not verify") {
            BundleError::AttestationCoversDifferentManifest { .. } => {}
            other => panic!("expected a manifest coverage failure, got {other:?}"),
        }
    }

    #[test]
    fn a_bundle_verifies_after_a_json_round_trip_through_a_third_party() {
        let attested = AttestedBundle::produce(
            bundle(),
            &key(),
            AttestationPurpose::PublisherManifest,
            ClaimedProducer::new("AURORA Working Group"),
        )
        .expect("attests");
        let json = serde_json::to_string(&attested).expect("serialises");
        let back: AttestedBundle = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(back, attested);
        back.verify(&key()).expect("verifies after transport");
    }

    #[test]
    fn the_bundle_reports_supply_chain_state_per_entry_without_collapsing_it() {
        let verified = bundle().verify().expect("verifies");
        let posture = verified.supply_chain_posture();
        assert_eq!(posture.recorded, vec!["world".to_string()]);
        assert_eq!(
            posture.unrecorded,
            vec![
                "certificate".to_string(),
                "query".to_string(),
                "section".to_string()
            ]
        );
        assert!(posture.rejected.is_empty());
        assert!(!posture.is_fully_recorded());
    }

    #[test]
    fn a_certificate_recorded_by_digest_only_is_not_reported_as_self_verified() {
        let referenced = ResultBundle::builder("run-3")
            .referencing(
                "certificate",
                EntryRole::ContextCertificate,
                ContentHash::of_bytes(b"certificate bytes"),
                None,
                ProvenanceState::Unrecorded,
            )
            .build()
            .expect("builds");
        assert_eq!(
            referenced.verify().expect("verifies").certificate(),
            EmbeddedCertificate::NotCarried
        );
    }

    #[test]
    fn changing_a_declared_environment_fact_invalidates_the_attestation() {
        let attested = AttestedBundle::produce(
            bundle(),
            &key(),
            AttestationPurpose::PublisherManifest,
            ClaimedProducer::new("AURORA Working Group"),
        )
        .expect("attests");
        let mut relabelled = attested.clone();
        relabelled.bundle.manifest.environment.os = Some("darwin".into());
        assert!(matches!(
            relabelled.verify(&key()),
            Err(BundleError::AttestationCoversDifferentManifest { .. })
        ));
        assert!(attested.verify(&key()).is_ok());
    }

    #[test]
    fn an_attestation_lifted_from_one_bundle_does_not_cover_another() {
        let first = AttestedBundle::produce(
            bundle(),
            &key(),
            AttestationPurpose::PublisherManifest,
            ClaimedProducer::new("AURORA Working Group"),
        )
        .expect("attests");
        let other_bundle = ResultBundle::builder("run-other")
            .carrying("section", EntryRole::DecisionSection, json!({"layers": []}))
            .expect("carries")
            .build()
            .expect("builds");
        let transplanted = AttestedBundle {
            bundle: other_bundle,
            attestation: first.attestation.clone(),
        };
        assert!(matches!(
            transplanted.verify(&key()),
            Err(BundleError::AttestationCoversDifferentManifest { .. })
        ));
    }

    #[test]
    fn the_honest_label_states_how_much_was_recomputed_and_how_much_was_not() {
        let label = bundle().verify().expect("verifies").honest_label();
        assert!(label.contains("3 of 4 entries recomputed"), "{label}");
        assert!(label.contains("1 recorded by digest only"), "{label}");
        assert!(label.contains("nobody checked"), "{label}");
    }

    #[test]
    fn the_schema_version_is_inside_the_hashed_bytes() {
        let attested = AttestedBundle::produce(
            bundle(),
            &key(),
            AttestationPurpose::PublisherManifest,
            ClaimedProducer::new("AURORA Working Group"),
        )
        .expect("attests");
        assert_eq!(attested.schema_version(), BUNDLE_SCHEMA_VERSION);
        let bytes = attested
            .bundle
            .manifest
            .canonical_bytes()
            .expect("canonical");
        assert!(String::from_utf8(bytes)
            .expect("utf8")
            .contains(BUNDLE_SCHEMA_VERSION));
    }
}
