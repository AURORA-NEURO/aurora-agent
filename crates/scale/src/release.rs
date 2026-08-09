//! Benchmark release, versioning and deprecation.
//!
//! Blueprint 35.17: "release packs like scientific software with immutable versions, migrations,
//! corrections, and retirement."
//!
//! ## Immutability is enforced, not requested
//!
//! [`ReleaseLedger::publish`] refuses to republish a version with different content and returns
//! [`ReleaseError::ImmutableReleaseModified`] carrying both digests. A benchmark whose version
//! number can be republished silently invalidates every result anyone ever reported against it, and
//! nobody finds out, because the failure is that nothing happens.
//!
//! ## Nothing is deleted
//!
//! 35's failure containment: "existing result bundles remain available with a clear invalidation or
//! supersession notice; they are not silently deleted." A withdrawal here sets state and records a
//! reason; it never removes the entry. [`ReleaseLedger::status_of`] keeps answering for withdrawn
//! versions, so a stale result can still be traced to what it was measured against.
//!
//! ## Not implemented
//!
//! No migration execution and no dependency resolution. 35.17 requires "data and oracle dependency
//! pins" and "result migration"; pins are recorded here as opaque digests and migration is a
//! transformation over result bundles that this crate does not own.

use crate::error::ReleaseError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Semantic version of a benchmark pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReleaseVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl ReleaseVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        ReleaseVersion {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for ReleaseVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Lifecycle of a published pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseState {
    Published,
    /// Superseded by a later version, still usable, still citable.
    Superseded { by: ReleaseVersion },
    /// Scheduled for retirement after a stated support window, in logical epochs.
    Deprecated { support_epochs: u32, reason: String },
    /// Invalidated. Results against it remain retrievable and are marked, not deleted.
    Withdrawn { reason: String },
}

/// One published release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    pub version: ReleaseVersion,
    /// Digest over the pack contents. The immutability check compares this.
    pub content_digest: String,
    /// Opaque digests of the data and oracle versions this pack was built against.
    pub pins: BTreeMap<String, String>,
    pub state: ReleaseState,
}

/// A supersession notice, as attached to results measured against the older version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Supersession {
    pub old: ReleaseVersion,
    pub new: ReleaseVersion,
    pub notice: String,
}

/// The append-only record of what was released.
#[derive(Debug, Default)]
pub struct ReleaseLedger {
    releases: BTreeMap<ReleaseVersion, Release>,
}

impl ReleaseLedger {
    pub fn new() -> Self {
        ReleaseLedger::default()
    }

    /// Publishes a version. Republishing identical content is a no-op; republishing different
    /// content is refused.
    pub fn publish(
        &mut self,
        version: ReleaseVersion,
        content_digest: impl Into<String>,
        pins: BTreeMap<String, String>,
    ) -> Result<&Release, ReleaseError> {
        let content_digest = content_digest.into();
        if let Some(existing) = self.releases.get(&version) {
            if existing.content_digest != content_digest {
                return Err(ReleaseError::ImmutableReleaseModified {
                    version: version.to_string(),
                    published: existing.content_digest.clone(),
                    attempted: content_digest,
                });
            }
            return Ok(self.releases.get(&version).expect("just checked"));
        }
        self.releases.insert(
            version,
            Release {
                version,
                content_digest,
                pins,
                state: ReleaseState::Published,
            },
        );
        self.releases
            .get(&version)
            .ok_or_else(|| ReleaseError::UnknownRelease {
                version: version.to_string(),
                action: "published",
            })
    }

    /// Marks `old` superseded by `new`, which must be a later version.
    pub fn supersede(
        &mut self,
        old: ReleaseVersion,
        new: ReleaseVersion,
    ) -> Result<Supersession, ReleaseError> {
        if new <= old {
            return Err(ReleaseError::SupersessionGoesBackwards {
                new: new.to_string(),
                old: old.to_string(),
            });
        }
        if !self.releases.contains_key(&new) {
            return Err(ReleaseError::UnknownRelease {
                version: new.to_string(),
                action: "used as a supersession target",
            });
        }
        let entry = self
            .releases
            .get_mut(&old)
            .ok_or_else(|| ReleaseError::UnknownRelease {
                version: old.to_string(),
                action: "superseded",
            })?;
        entry.state = ReleaseState::Superseded { by: new };
        Ok(Supersession {
            old,
            new,
            notice: format!(
                "results measured against {old} remain valid for {old} and are superseded by {new}; \
                 they are not comparable to {new} results without a stated migration"
            ),
        })
    }

    pub fn deprecate(
        &mut self,
        version: ReleaseVersion,
        support_epochs: u32,
        reason: impl Into<String>,
    ) -> Result<(), ReleaseError> {
        let entry = self
            .releases
            .get_mut(&version)
            .ok_or_else(|| ReleaseError::UnknownRelease {
                version: version.to_string(),
                action: "deprecated",
            })?;
        entry.state = ReleaseState::Deprecated {
            support_epochs,
            reason: reason.into(),
        };
        Ok(())
    }

    /// Withdraws a release. The entry stays; only its state changes.
    pub fn withdraw(
        &mut self,
        version: ReleaseVersion,
        reason: impl Into<String>,
    ) -> Result<(), ReleaseError> {
        let entry = self
            .releases
            .get_mut(&version)
            .ok_or_else(|| ReleaseError::UnknownRelease {
                version: version.to_string(),
                action: "withdrawn",
            })?;
        if matches!(entry.state, ReleaseState::Withdrawn { .. }) {
            return Err(ReleaseError::AlreadyWithdrawn(version.to_string()));
        }
        entry.state = ReleaseState::Withdrawn {
            reason: reason.into(),
        };
        Ok(())
    }

    pub fn status_of(&self, version: ReleaseVersion) -> Option<&Release> {
        self.releases.get(&version)
    }

    pub fn all(&self) -> Vec<&Release> {
        self.releases.values().collect()
    }

    pub fn latest(&self) -> Option<&Release> {
        self.releases.values().next_back()
    }
}
