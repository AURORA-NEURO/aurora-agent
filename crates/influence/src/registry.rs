//! The abstract-domain registry 43.11 asks for and `fiber-world/0.1` does not carry.
//!
//! `bioprism-fiber` reports `abstract_interpretation` as a deferred pass on every compile with the
//! reason *"43.11 requires an abstract-domain registry absent from fiber-world/0.1"*, and
//! `bioprism-examples` records the same sentence as the blocker on its `abstract_interpretation`
//! property. This module is that registry. What it is not is a change to the wire schema: a registry
//! is a compiler-side table of abstractions, and the reason the reference world still produces no
//! bound is that it declares no potentials for any abstraction to be *of*. [`crate::reference`] is
//! where that is measured.
//!
//! ## What the registry is for
//!
//! A compiler pass does not know at monomorphisation time which domain it will run in — it reads a
//! name out of a plan. So the registry is dynamic, and dynamic means type erasure, and type erasure
//! is exactly where one domain's abstraction can reach another domain's transformer. Two of the
//! three shipped domains are intervals of non-negative reals with identical arithmetic and
//! unrelated meanings, so this is not a hypothetical: passing a [`crate::domains::Displacement`] to
//! [`crate::domains::RatioIntervalDomain`]'s join would compute a perfectly plausible interval that
//! no theorem connects to anything.
//!
//! Every value therefore carries the [`DomainId`] it was built under, every operation checks it,
//! and a mismatch is [`DomainError::ForeignAbstractValue`] rather than a number. On the static path
//! the check is free and earlier: the domains have distinct `Element` types, so
//! `RatioIntervalDomain::join` will not compile against a `Displacement`.
//!
//! Registration is likewise refused rather than overwritten. Silently replacing a registered domain
//! would leave values built under the old one in the hands of the new one's transformers — the same
//! failure by a slower route.
//!
//! ## Why the standard set has two members and not three
//!
//! [`DomainRegistry::standard`] registers the two interval domains. [`crate::domains::SupportDomain`]
//! is indexed by the length of the potential it abstracts, so "the support domain" is a family and
//! a caller registers the member matching the factor in hand. That is the registry doing its job:
//! two lengths are two domains, and joining a length-three abstraction with a length-four one is a
//! question with no answer rather than a silently widened one.

use crate::domain::{AbstractDomain, DomainError, DomainId, FactClass};
use crate::domains::{DisplacementDomain, RatioIntervalDomain};
use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;

/// An abstract element together with the domain it belongs to.
///
/// The tag is not a convenience. It is the only thing standing between a type-erased registry and a
/// bound computed by feeding one lattice's element to another lattice's transformer.
pub struct AbstractValue {
    domain: DomainId,
    payload: Box<dyn Any>,
}

impl AbstractValue {
    /// Tags `element` with `domain`'s id.
    pub fn of<D: AbstractDomain>(domain: &D, element: D::Element) -> Self {
        AbstractValue {
            domain: domain.id(),
            payload: Box::new(element),
        }
    }

    pub fn domain(&self) -> &DomainId {
        &self.domain
    }

    /// The element, if this value belongs to `domain` and carries its element type.
    pub fn element<D: AbstractDomain>(&self, domain: &D) -> Result<&D::Element, DomainError> {
        if self.domain != domain.id() {
            return Err(DomainError::ForeignAbstractValue {
                expected: domain.id(),
                found: self.domain.clone(),
            });
        }
        self.payload
            .downcast_ref::<D::Element>()
            .ok_or_else(|| DomainError::ElementTypeMismatch {
                id: domain.id(),
            })
    }
}

impl fmt::Debug for AbstractValue {
    /// The payload is behind [`Any`] and has no `Debug`, so the domain is what can be shown. Use
    /// [`DomainRegistry::render`] for the element itself.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AbstractValue")
            .field("domain", &self.domain)
            .finish_non_exhaustive()
    }
}

/// The object-safe face of [`AbstractDomain`], over tagged values.
///
/// Every method that consumes a value returns `Result`, because in a type-erased world the argument
/// may belong to another domain and there is no sound element of *this* one to return instead.
pub trait ErasedDomain {
    fn id(&self) -> DomainId;
    fn abstracts(&self) -> FactClass;
    fn bottom(&self) -> AbstractValue;
    fn top(&self) -> AbstractValue;
    fn leq(&self, left: &AbstractValue, right: &AbstractValue) -> Result<bool, DomainError>;
    fn join(
        &self,
        left: &AbstractValue,
        right: &AbstractValue,
    ) -> Result<AbstractValue, DomainError>;
    fn meet(
        &self,
        left: &AbstractValue,
        right: &AbstractValue,
    ) -> Result<AbstractValue, DomainError>;
    fn widen(
        &self,
        previous: &AbstractValue,
        next: &AbstractValue,
    ) -> Result<AbstractValue, DomainError>;
    fn render(&self, value: &AbstractValue) -> Result<String, DomainError>;
}

impl<D: AbstractDomain> ErasedDomain for D {
    fn id(&self) -> DomainId {
        AbstractDomain::id(self)
    }

    fn abstracts(&self) -> FactClass {
        AbstractDomain::abstracts(self)
    }

    fn bottom(&self) -> AbstractValue {
        AbstractValue::of(self, AbstractDomain::bottom(self))
    }

    fn top(&self) -> AbstractValue {
        AbstractValue::of(self, AbstractDomain::top(self))
    }

    fn leq(&self, left: &AbstractValue, right: &AbstractValue) -> Result<bool, DomainError> {
        Ok(AbstractDomain::leq(
            self,
            left.element(self)?,
            right.element(self)?,
        ))
    }

    fn join(
        &self,
        left: &AbstractValue,
        right: &AbstractValue,
    ) -> Result<AbstractValue, DomainError> {
        let joined = AbstractDomain::join(self, left.element(self)?, right.element(self)?);
        Ok(AbstractValue::of(self, joined))
    }

    fn meet(
        &self,
        left: &AbstractValue,
        right: &AbstractValue,
    ) -> Result<AbstractValue, DomainError> {
        let met = AbstractDomain::meet(self, left.element(self)?, right.element(self)?);
        Ok(AbstractValue::of(self, met))
    }

    fn widen(
        &self,
        previous: &AbstractValue,
        next: &AbstractValue,
    ) -> Result<AbstractValue, DomainError> {
        let widened = AbstractDomain::widen(self, previous.element(self)?, next.element(self)?);
        Ok(AbstractValue::of(self, widened))
    }

    fn render(&self, value: &AbstractValue) -> Result<String, DomainError> {
        Ok(AbstractDomain::render(self, value.element(self)?))
    }
}

/// The table of domains a compiler pass may select from.
#[derive(Default)]
pub struct DomainRegistry {
    entries: BTreeMap<DomainId, Box<dyn ErasedDomain>>,
}

impl fmt::Debug for DomainRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DomainRegistry")
            .field("registered", &self.ids())
            .finish()
    }
}

impl DomainRegistry {
    pub fn new() -> Self {
        DomainRegistry::default()
    }

    /// The two domains the shipped analysis uses.
    ///
    /// [`crate::domains::SupportDomain`] is absent because it is indexed by the length of the
    /// potential it abstracts; a caller registers the member it needs. See the module header.
    pub fn standard() -> Result<Self, DomainError> {
        let mut registry = DomainRegistry::new();
        registry.register(RatioIntervalDomain)?;
        registry.register(DisplacementDomain)?;
        Ok(registry)
    }

    /// Refuses a duplicate id rather than replacing the domain behind it.
    pub fn register<D: AbstractDomain + 'static>(&mut self, domain: D) -> Result<(), DomainError> {
        let id = AbstractDomain::id(&domain);
        if self.entries.contains_key(&id) {
            return Err(DomainError::DuplicateRegistration { id });
        }
        self.entries.insert(id, Box::new(domain));
        Ok(())
    }

    pub fn ids(&self) -> Vec<DomainId> {
        self.entries.keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains(&self, id: &DomainId) -> bool {
        self.entries.contains_key(id)
    }

    /// The registered domains abstracting a given class of facts.
    ///
    /// More than one is the normal case and the point of the class: two abstractions of the same
    /// facts are alternatives a scheduler may choose between, while two abstractions of different
    /// classes are not comparable at all.
    pub fn abstracting(&self, class: FactClass) -> Vec<DomainId> {
        self.entries
            .iter()
            .filter(|(_, domain)| domain.abstracts() == class)
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn get(&self, id: &DomainId) -> Result<&dyn ErasedDomain, DomainError> {
        self.entries
            .get(id)
            .map(Box::as_ref)
            .ok_or_else(|| DomainError::UnregisteredDomain { id: id.clone() })
    }

    /// `a ⊔ b`, refusing when either value belongs to another domain.
    pub fn join(
        &self,
        id: &DomainId,
        left: &AbstractValue,
        right: &AbstractValue,
    ) -> Result<AbstractValue, DomainError> {
        self.get(id)?.join(left, right)
    }

    /// `a ⊓ b`, refusing when either value belongs to another domain.
    pub fn meet(
        &self,
        id: &DomainId,
        left: &AbstractValue,
        right: &AbstractValue,
    ) -> Result<AbstractValue, DomainError> {
        self.get(id)?.meet(left, right)
    }

    /// `widen(previous, next)`, refusing when either value belongs to another domain.
    pub fn widen(
        &self,
        id: &DomainId,
        previous: &AbstractValue,
        next: &AbstractValue,
    ) -> Result<AbstractValue, DomainError> {
        self.get(id)?.widen(previous, next)
    }

    pub fn leq(
        &self,
        id: &DomainId,
        left: &AbstractValue,
        right: &AbstractValue,
    ) -> Result<bool, DomainError> {
        self.get(id)?.leq(left, right)
    }

    pub fn top(&self, id: &DomainId) -> Result<AbstractValue, DomainError> {
        Ok(self.get(id)?.top())
    }

    pub fn bottom(&self, id: &DomainId) -> Result<AbstractValue, DomainError> {
        Ok(self.get(id)?.bottom())
    }

    pub fn render(&self, id: &DomainId, value: &AbstractValue) -> Result<String, DomainError> {
        self.get(id)?.render(value)
    }
}
