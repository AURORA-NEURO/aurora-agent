//! The pointwise product of a domain with itself, indexed by a region's variables.
//!
//! An analysis that carries one abstract element per site needs a lattice over vectors, and the
//! pointwise construction is the standard one: order, join, meet and widening all act coordinate by
//! coordinate. It is included as a domain rather than open-coded in [`crate::gibbs`] because that
//! is the claim being tested — that the trait in [`crate::domain`] is a lattice interface a solver
//! can be written against, not a shape three concrete types happen to have.
//!
//! ## Termination of the pointwise widening
//!
//! A vector stabilises exactly when every coordinate does. Each coordinate widens with the inner
//! domain's operator, which terminates every ascending chain by hypothesis, and the arity is finite,
//! so the vector chain stabilises after at most the sum of the coordinates' bounds.
//!
//! ## Arity
//!
//! The arity is part of the [`DomainId`]. Two arities are two registry entries, and a vector of the
//! wrong length reaching an operation directly degrades to `⊤` — sound, never silently tighter, and
//! unreachable through the registry.

use crate::domain::{AbstractDomain, DomainId, FactClass};

/// The registered name prefix of [`ProductDomain`].
pub const PRODUCT_DOMAIN_PREFIX: &str = "product";

/// The pointwise product `Dⁿ`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductDomain<D> {
    inner: D,
    arity: usize,
}

impl<D: AbstractDomain> ProductDomain<D> {
    pub fn new(inner: D, arity: usize) -> Self {
        ProductDomain { inner, arity }
    }

    pub fn inner(&self) -> &D {
        &self.inner
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    fn normalise(&self, element: &[D::Element]) -> Vec<D::Element> {
        if element.len() == self.arity {
            element.to_vec()
        } else {
            self.top()
        }
    }
}

impl<D: AbstractDomain> AbstractDomain for ProductDomain<D> {
    type Element = Vec<D::Element>;
    type Concrete = Vec<D::Concrete>;

    fn id(&self) -> DomainId {
        DomainId::new(format!(
            "{PRODUCT_DOMAIN_PREFIX}/{}/{}",
            self.inner.id(),
            self.arity
        ))
    }

    fn abstracts(&self) -> FactClass {
        self.inner.abstracts()
    }

    fn bottom(&self) -> Vec<D::Element> {
        vec![self.inner.bottom(); self.arity]
    }

    fn top(&self) -> Vec<D::Element> {
        vec![self.inner.top(); self.arity]
    }

    fn leq(&self, left: &Self::Element, right: &Self::Element) -> bool {
        let (left, right) = (self.normalise(left), self.normalise(right));
        left.iter()
            .zip(&right)
            .all(|(low, high)| self.inner.leq(low, high))
    }

    fn join(&self, left: &Self::Element, right: &Self::Element) -> Self::Element {
        let (left, right) = (self.normalise(left), self.normalise(right));
        left.iter()
            .zip(&right)
            .map(|(low, high)| self.inner.join(low, high))
            .collect()
    }

    fn meet(&self, left: &Self::Element, right: &Self::Element) -> Self::Element {
        let (left, right) = (self.normalise(left), self.normalise(right));
        left.iter()
            .zip(&right)
            .map(|(low, high)| self.inner.meet(low, high))
            .collect()
    }

    fn widen(&self, previous: &Self::Element, next: &Self::Element) -> Self::Element {
        let (previous, next) = (self.normalise(previous), self.normalise(next));
        previous
            .iter()
            .zip(&next)
            .map(|(before, after)| self.inner.widen(before, after))
            .collect()
    }

    fn concretises(&self, element: &Self::Element, concrete: &Self::Concrete) -> bool {
        let element = self.normalise(element);
        element.len() == concrete.len()
            && element
                .iter()
                .zip(concrete)
                .all(|(abstracted, value)| self.inner.concretises(abstracted, value))
    }

    fn render(&self, element: &Self::Element) -> String {
        let rendered: Vec<String> = self
            .normalise(element)
            .iter()
            .map(|coordinate| self.inner.render(coordinate))
            .collect();
        format!("({})", rendered.join(", "))
    }
}
