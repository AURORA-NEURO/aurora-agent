//! Scoped, expiring capability tokens instead of ambient credentials.
//!
//! Blueprint 05.08's secret broker, and the invariant that untrusted benchmark code "receives no
//! ambient credentials". The shape follows from that sentence: a tool never holds an API key, it
//! holds a `Capability` naming one resource and one operation, valid until the trial's task clock
//! passes an expiry. Redeeming it is the only way to reach a secret, redemption happens inside the
//! runtime, and the secret is wrapped in a type whose `Debug` and `Serialize` both redact.
//!
//! The redaction is not decoration. Capabilities and tapes are serialized, published and diffed; a
//! secret that reached either would be leaked to everyone who reads a bundle, forever, and no later
//! rotation un-publishes it. So the only way to get at the value is [`SecretRef::expose`], named to
//! be greppable, and nothing in this crate calls it.
//!
//! Deliberately **not** implemented: issuing real provider credentials, and any transport. A broker
//! that minted cloud tokens would need a cloud, which is the container provider's business (05.03,
//! declared unavailable here). What this type contributes is the *shape* of the guarantee — scope,
//! expiry, revocation, redaction — against which a real broker can be conformance-tested.

use crate::error::RuntimeError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// A secret value that will not print itself.
///
/// `Debug` redacts, `Display` redacts, and there is no `Serialize`. Getting the value requires
/// calling [`SecretRef::expose`], which exists so the places that do it are findable.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretRef(String);

impl SecretRef {
    pub fn new(value: impl Into<String>) -> Self {
        SecretRef(value.into())
    }

    /// Yields the secret. Every call site is a place to look during a leak review.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretRef(<redacted>)")
    }
}

impl fmt::Display for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// A narrow, expiring permission to use one resource for one operation.
///
/// Serializable on purpose — it travels to the tool that will use it — and it carries no secret, so
/// travelling costs nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    pub id: String,
    pub resource: String,
    pub operation: String,
    /// Expiry on the *task* clock (05.07), not the wall clock, so a token's lifetime is part of the
    /// trial's semantics rather than a race against the machine's speed.
    pub expires_at_task_millis: u64,
}

/// Holds the secrets and issues capabilities against them.
#[derive(Debug, Clone, Default)]
pub struct SecretBroker {
    secrets: BTreeMap<String, SecretRef>,
    issued: BTreeMap<String, Capability>,
    revoked: BTreeMap<String, ()>,
    next: u64,
}

impl SecretBroker {
    pub fn new() -> Self {
        SecretBroker::default()
    }

    pub fn with_secret(mut self, resource: impl Into<String>, value: impl Into<String>) -> Self {
        self.secrets
            .insert(resource.into(), SecretRef::new(value.into()));
        self
    }

    /// Issues a capability for one resource and operation.
    ///
    /// Refuses for an unknown resource rather than issuing a token that would fail later: a
    /// capability that cannot be redeemed is worse than no capability, because the failure surfaces
    /// at the moment of use, far from the plan that got it wrong.
    pub fn issue(
        &mut self,
        resource: &str,
        operation: &str,
        now_task_millis: u64,
        ttl_millis: u64,
    ) -> Result<Capability, RuntimeError> {
        if !self.secrets.contains_key(resource) {
            return Err(RuntimeError::UnknownResource {
                resource: resource.to_string(),
            });
        }
        let capability = Capability {
            id: format!("cap-{:06}", self.next),
            resource: resource.to_string(),
            operation: operation.to_string(),
            expires_at_task_millis: now_task_millis.saturating_add(ttl_millis),
        };
        self.next += 1;
        self.issued.insert(capability.id.clone(), capability.clone());
        Ok(capability)
    }

    /// Exchanges a capability for the secret it scopes.
    ///
    /// Checks revocation, then expiry, then scope. The order matters for the diagnostic: a revoked
    /// token that is also expired should say "revoked", because that is the fact an operator acted
    /// on and the expiry is incidental.
    pub fn redeem(
        &self,
        capability: &Capability,
        operation: &str,
        now_task_millis: u64,
    ) -> Result<&SecretRef, RuntimeError> {
        if self.revoked.contains_key(&capability.id) {
            return Err(RuntimeError::CapabilityRevoked {
                id: capability.id.clone(),
            });
        }
        if now_task_millis > capability.expires_at_task_millis {
            return Err(RuntimeError::CapabilityExpired {
                id: capability.id.clone(),
                expires_at_task_millis: capability.expires_at_task_millis,
                now_task_millis,
            });
        }
        if capability.operation != operation {
            return Err(RuntimeError::OperationNotCovered {
                id: capability.id.clone(),
                resource: capability.resource.clone(),
                covered: capability.operation.clone(),
                requested: operation.to_string(),
            });
        }
        self.secrets
            .get(&capability.resource)
            .ok_or_else(|| RuntimeError::UnknownResource {
                resource: capability.resource.clone(),
            })
    }

    pub fn revoke(&mut self, id: &str) {
        self.revoked.insert(id.to_string(), ());
    }

    /// Revokes everything, as trial cleanup must (05.06: a credential that outlives its trial is a
    /// security incident, not an inconvenience).
    pub fn revoke_all(&mut self) {
        let ids: Vec<String> = self.issued.keys().cloned().collect();
        for id in ids {
            self.revoked.insert(id, ());
        }
    }

    pub fn is_revoked(&self, id: &str) -> bool {
        self.revoked.contains_key(id)
    }

    pub fn issued(&self) -> impl Iterator<Item = &Capability> {
        self.issued.values()
    }
}
