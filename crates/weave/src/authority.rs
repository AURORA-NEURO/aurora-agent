//! Authority grants.
//!
//! Blueprint 23.13: delegation must **attenuate** and revocation must be **transitive**. A
//! participant cannot pass on more than it holds, and revoking a grant revokes everything
//! delegated from it — otherwise authority leaks downstream and a revocation is cosmetic.
//!
//! Grants form a forest. Each records its parent, so revocation is a subtree operation rather
//! than a flag on one node.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ReadWorld,
    ReadEvidence,
    CompileContext,
    BranchWrite,
    ExecuteTests,
    PublishResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant {
    pub id: String,
    pub holder: String,
    pub capabilities: BTreeSet<Capability>,
    /// The grant this was delegated from, if any. Roots are issued by the kernel operator.
    pub parent: Option<String>,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthorityError {
    #[error("no such grant: {0}")]
    UnknownGrant(String),

    #[error("grant {0} has been revoked")]
    Revoked(String),

    #[error("{holder} does not hold {missing:?} and cannot delegate it")]
    Amplification {
        holder: String,
        missing: Vec<Capability>,
    },

    #[error("{holder} lacks {required:?}")]
    Insufficient {
        holder: String,
        required: Capability,
    },
}

#[derive(Debug, Clone, Default)]
pub struct AuthorityTable {
    grants: BTreeMap<String, Grant>,
    next: usize,
}

impl AuthorityTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Issues a root grant. Only the operator does this; participants may only delegate.
    pub fn issue(
        &mut self,
        holder: impl Into<String>,
        capabilities: impl IntoIterator<Item = Capability>,
    ) -> String {
        self.next += 1;
        let id = format!("grant-{:04}", self.next);
        self.grants.insert(
            id.clone(),
            Grant {
                id: id.clone(),
                holder: holder.into(),
                capabilities: capabilities.into_iter().collect(),
                parent: None,
                revoked: false,
            },
        );
        id
    }

    /// Delegates a subset of a grant.
    ///
    /// Refuses to issue any capability the parent does not hold, which is the attenuation
    /// property: authority can only narrow as it travels.
    pub fn delegate(
        &mut self,
        parent_id: &str,
        holder: impl Into<String>,
        capabilities: impl IntoIterator<Item = Capability>,
    ) -> Result<String, AuthorityError> {
        let parent = self.effective(parent_id)?.clone();
        let requested: BTreeSet<Capability> = capabilities.into_iter().collect();

        let missing: Vec<Capability> = requested
            .difference(&parent.capabilities)
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(AuthorityError::Amplification {
                holder: parent.holder.clone(),
                missing,
            });
        }

        self.next += 1;
        let id = format!("grant-{:04}", self.next);
        self.grants.insert(
            id.clone(),
            Grant {
                id: id.clone(),
                holder: holder.into(),
                capabilities: requested,
                parent: Some(parent_id.to_string()),
                revoked: false,
            },
        );
        Ok(id)
    }

    /// Revokes a grant and everything delegated from it, however deep.
    pub fn revoke(&mut self, id: &str) -> Result<Vec<String>, AuthorityError> {
        if !self.grants.contains_key(id) {
            return Err(AuthorityError::UnknownGrant(id.to_string()));
        }

        let mut revoked = Vec::new();
        let mut frontier = vec![id.to_string()];
        while let Some(current) = frontier.pop() {
            if let Some(grant) = self.grants.get_mut(&current) {
                if !grant.revoked {
                    grant.revoked = true;
                    revoked.push(current.clone());
                }
            }
            let children: Vec<String> = self
                .grants
                .values()
                .filter(|grant| grant.parent.as_deref() == Some(current.as_str()))
                .map(|grant| grant.id.clone())
                .collect();
            frontier.extend(children);
        }
        revoked.sort();
        Ok(revoked)
    }

    /// Resolves a grant, refusing it if it or any ancestor was revoked.
    pub fn effective(&self, id: &str) -> Result<&Grant, AuthorityError> {
        let grant = self
            .grants
            .get(id)
            .ok_or_else(|| AuthorityError::UnknownGrant(id.to_string()))?;
        if grant.revoked {
            return Err(AuthorityError::Revoked(id.to_string()));
        }
        let mut cursor = grant.parent.clone();
        while let Some(parent_id) = cursor {
            let parent = self
                .grants
                .get(&parent_id)
                .ok_or_else(|| AuthorityError::UnknownGrant(parent_id.clone()))?;
            if parent.revoked {
                return Err(AuthorityError::Revoked(parent_id));
            }
            cursor = parent.parent.clone();
        }
        Ok(grant)
    }

    pub fn check(&self, id: &str, required: Capability) -> Result<(), AuthorityError> {
        let grant = self.effective(id)?;
        if grant.capabilities.contains(&required) {
            Ok(())
        } else {
            Err(AuthorityError::Insufficient {
                holder: grant.holder.clone(),
                required,
            })
        }
    }

    pub fn len(&self) -> usize {
        self.grants.len()
    }

    pub fn is_empty(&self) -> bool {
        self.grants.is_empty()
    }
}
