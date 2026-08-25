//! The query region: the thing a physical backend is asked to cost and run.
//!
//! Blueprint 43.18 states the normative decision this whole crate turns on: "optimize physical
//! execution around query-specific width and representation statistics; never use total graph size
//! as the primary complexity predictor." A [`QueryRegion`] is therefore not a world. It is the
//! part of the world one query reaches, together with the domain cardinalities that decide what
//! elimination will actually cost. The reference world in `fixtures/` has 756 factors; the split
//! integrity query reaches six of them.
//!
//! ## Structure and valuation are separate
//!
//! A region carries a factor *signature* — an ordered scope — and, optionally, a factor *table*.
//! The split is not fussiness. The `fiber-world/0.1` wire schema declares each factor's inputs,
//! outputs and kind and never its potential, so a region sliced out of a world is genuinely
//! structural: its width, its elimination order and its cost are all well defined, and there is
//! nothing to multiply. Backends refuse such a region at `execute` with
//! [`crate::Declined::MissingFactorTable`] rather than inventing a table and returning a number
//! that means nothing. [`QueryRegion::with_uniform_tables`] supplies the one valuation that is
//! defensible without domain knowledge — the all-ones factor, under which sum-product counts
//! assignments — and says so in the region's assumptions.

use crate::error::RegionError;
use crate::semiring::Semiring;
use bioprism_world::WorldSource;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Cap on a table this crate will materialise on the caller's behalf.
pub const MAX_MATERIALISED_TABLE_ENTRIES: usize = 1 << 22;

/// One factor in the region.
///
/// `scope` is ordered and the table is row-major over it with the *last* variable varying
/// fastest. The order is part of the factor's identity because it fixes the table layout; the
/// elimination machinery never depends on it.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionFactor {
    id: String,
    scope: Vec<String>,
    table: Option<Vec<f64>>,
}

impl RegionFactor {
    /// A factor known only by its signature. Costable, not executable.
    pub fn structural(id: impl Into<String>, scope: impl IntoIterator<Item = impl Into<String>>) -> Self {
        RegionFactor {
            id: id.into(),
            scope: scope.into_iter().map(Into::into).collect(),
            table: None,
        }
    }

    /// A factor with a potential, row-major over `scope`.
    pub fn with_table(
        id: impl Into<String>,
        scope: impl IntoIterator<Item = impl Into<String>>,
        table: Vec<f64>,
    ) -> Self {
        RegionFactor {
            id: id.into(),
            scope: scope.into_iter().map(Into::into).collect(),
            table: Some(table),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn scope(&self) -> &[String] {
        &self.scope
    }

    pub fn table(&self) -> Option<&[f64]> {
        self.table.as_deref()
    }

    /// Number of variables in the factor's scope.
    ///
    /// 43.18 lists factor arity among the collected statistics. It is a cheap statistic and a
    /// weak predictor: a region of arity-2 factors can have any induced width at all.
    pub fn arity(&self) -> usize {
        self.scope.len()
    }
}

/// How a variable's domain size was obtained.
///
/// 43.18's runtime contract requires a width estimate to state its "assumptions", and the single
/// largest assumption in a world-derived region is the cardinality of a variable no fact pins
/// down. Tracking which variables were defaulted lets [`crate::Estimate::uncertainty`] be a
/// measured fraction rather than a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardinalitySource {
    /// Read off the value of the fact providing this variable.
    Observed,
    /// No fact provides the variable; the policy default was used.
    Assumed,
}

/// How a fact's JSON value is turned into a domain size.
///
/// Crude, and labelled as such. A JSON array of five entries is read as a five-valued domain; a
/// scalar is read as a pinned constant. The blueprint specifies "domain cardinalities" as an
/// input to the cost model and never says how a `fiber-world/0.1` value maps to one, so this is
/// a decision of the implementation and is reported as an assumption on every estimate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardinalityPolicy {
    /// Domain size for a variable that no fact provides.
    pub default_cardinality: usize,
    /// Domain size for a scalar-valued fact.
    pub scalar_cardinality: usize,
}

impl Default for CardinalityPolicy {
    fn default() -> Self {
        CardinalityPolicy {
            default_cardinality: 2,
            scalar_cardinality: 1,
        }
    }
}

impl CardinalityPolicy {
    pub fn of_value(&self, value: &Value) -> usize {
        match value {
            Value::Bool(_) => 2,
            Value::Array(items) => items.len().max(1),
            Value::Object(fields) => fields.len().max(1),
            Value::Null | Value::String(_) | Value::Number(_) => self.scalar_cardinality,
        }
    }
}

/// Where the region came from, for the plan descriptor 43.36 requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RegionProvenance {
    pub compiled_fact_count: usize,
    pub total_fact_count: usize,
    pub total_factor_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryRegion {
    label: String,
    semiring: Semiring,
    cardinality: BTreeMap<String, usize>,
    source: BTreeMap<String, CardinalitySource>,
    factors: Vec<RegionFactor>,
    free: BTreeSet<String>,
    assumptions: Vec<String>,
    provenance: RegionProvenance,
}

impl QueryRegion {
    pub fn builder(label: impl Into<String>) -> QueryRegionBuilder {
        QueryRegionBuilder {
            label: label.into(),
            semiring: Semiring::SumProduct,
            cardinality: BTreeMap::new(),
            source: BTreeMap::new(),
            factors: Vec::new(),
            free: BTreeSet::new(),
            assumptions: Vec::new(),
            provenance: RegionProvenance::default(),
        }
    }

    /// Slices the region one query reaches, backwards from its targets.
    ///
    /// This mirrors the dependency slice of 43.17. It is reimplemented here rather than imported
    /// because the backend portfolio sits *beneath* the compiler: a physical backend must be
    /// costable from a region alone, with no compiler in the link. The two implementations agree
    /// on selected factors by construction — a factor enters when it produces a needed variable —
    /// and `bioprism-fiber`'s version additionally reports the needed-variable set.
    ///
    /// ## Two sets, because a factor's outputs are not a slicing frontier
    ///
    /// 43.17 admits a variable to the slice only when some factor producing a needed variable
    /// *consumes* it, so the walk expands a selected factor's inputs and never its outputs. On a
    /// single-output factor the distinction is invisible — the one output is already needed, which
    /// is why the factor was selected — and every fixture in the workspace was of that shape, so
    /// the divergence below could not be observed. Give a factor two outputs and it becomes
    /// load-bearing: the sibling output is produced by the compiled program rather than required by
    /// it, and walking backwards from it selects factors that compute a value nothing consumes.
    /// `compiled_factor_count` on the certificate is `len(selected_factor_ids)` in the CPython
    /// reference and indexes the factor documents the Decision Section delivers, so a region that
    /// selected more factors than that would make the certificate's own count wrong.
    ///
    /// The sibling output does still belong to the region: it sits in its producer's scope,
    /// elimination has to size it, and [`QueryRegionBuilder::build`] rejects a factor naming a
    /// variable the region never declared. So `variables` collects inputs and outputs alike while
    /// `frontier` — the set that gates the stack, and the exact analogue of `bioprism-fiber`'s
    /// `Slice::needed_variables` — collects targets and inputs only.
    ///
    /// The resulting region is structural. `factors` carry signatures and no tables.
    pub fn from_world_slice<S, I>(
        source: &S,
        label: impl Into<String>,
        targets: I,
        policy: &CardinalityPolicy,
    ) -> Result<QueryRegion, RegionError>
    where
        S: WorldSource + ?Sized,
        I: IntoIterator<Item: AsRef<str>>,
    {
        let mut variables: BTreeSet<String> = BTreeSet::new();
        let mut frontier: BTreeSet<String> = BTreeSet::new();
        let mut stack: Vec<String> = Vec::new();
        let mut free: BTreeSet<String> = BTreeSet::new();

        for target in targets {
            let target = target.as_ref().to_string();
            free.insert(target.clone());
            variables.insert(target.clone());
            if frontier.insert(target.clone()) {
                stack.push(target);
            }
        }

        let mut selected: BTreeSet<String> = BTreeSet::new();
        let mut factors: Vec<RegionFactor> = Vec::new();

        while let Some(variable) = stack.pop() {
            for factor_id in source.producer_ids(&variable) {
                if !selected.insert(factor_id.clone()) {
                    continue;
                }
                let Some(factor) = source.factor(&factor_id) else {
                    continue;
                };
                let mut scope: Vec<String> = Vec::new();
                for name in factor.inputs.iter().chain(factor.outputs.iter()) {
                    let name = name.as_str().to_string();
                    if !scope.contains(&name) {
                        scope.push(name.clone());
                    }
                    variables.insert(name);
                }
                for input in &factor.inputs {
                    let input = input.as_str().to_string();
                    if frontier.insert(input.clone()) {
                        stack.push(input);
                    }
                }
                factors.push(RegionFactor::structural(factor_id, scope));
            }
        }

        factors.sort_by(|left, right| left.id.cmp(&right.id));

        let mut builder = QueryRegion::builder(label);
        let mut assumed = 0usize;
        let mut compiled_facts = 0usize;
        for variable in &variables {
            match source.fact_providing(variable) {
                Some(fact) => {
                    compiled_facts += 1;
                    builder = builder.observed_variable(variable, policy.of_value(&fact.value));
                }
                None => {
                    assumed += 1;
                    builder = builder.assumed_variable(variable, policy.default_cardinality);
                }
            }
        }
        for factor in factors {
            builder = builder.factor(factor);
        }
        for variable in &free {
            builder = builder.free(variable);
        }

        builder = builder
            .assumption(format!(
                "domain cardinality read from each providing fact's JSON value (array/object length, bool = 2, scalar = {})",
                policy.scalar_cardinality
            ))
            .assumption(format!(
                "{assumed} of {} region variables have no providing fact and were assigned the default cardinality {}",
                variables.len(),
                policy.default_cardinality
            ))
            .provenance(RegionProvenance {
                compiled_fact_count: compiled_facts,
                total_fact_count: source.total_facts(),
                total_factor_count: source.total_factors(),
            });

        builder.build()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn semiring(&self) -> Semiring {
        self.semiring
    }

    pub fn factors(&self) -> &[RegionFactor] {
        &self.factors
    }

    pub fn variables(&self) -> impl Iterator<Item = &str> {
        self.cardinality.keys().map(String::as_str)
    }

    pub fn variable_count(&self) -> usize {
        self.cardinality.len()
    }

    pub fn cardinality(&self) -> &BTreeMap<String, usize> {
        &self.cardinality
    }

    pub fn cardinality_of(&self, variable: &str) -> Option<usize> {
        self.cardinality.get(variable).copied()
    }

    pub fn cardinality_source(&self, variable: &str) -> Option<CardinalitySource> {
        self.source.get(variable).copied()
    }

    /// Variables the query keeps. Never eliminated.
    pub fn free_variables(&self) -> &BTreeSet<String> {
        &self.free
    }

    /// Variables the query aggregates away.
    pub fn bound_variables(&self) -> BTreeSet<String> {
        self.cardinality
            .keys()
            .filter(|name| !self.free.contains(*name))
            .cloned()
            .collect()
    }

    pub fn assumptions(&self) -> &[String] {
        &self.assumptions
    }

    pub fn provenance(&self) -> RegionProvenance {
        self.provenance
    }

    pub fn has_tables(&self) -> bool {
        self.factors.iter().all(|factor| factor.table.is_some())
    }

    /// The largest factor arity in the region.
    pub fn max_factor_arity(&self) -> usize {
        self.factors
            .iter()
            .map(RegionFactor::arity)
            .max()
            .unwrap_or(0)
    }

    /// Size of the full joint assignment space. The thing width exists to avoid touching.
    pub fn joint_entries(&self) -> f64 {
        self.cardinality
            .values()
            .map(|card| *card as f64)
            .product::<f64>()
    }

    /// Size of the answer table.
    pub fn free_entries(&self) -> f64 {
        self.free
            .iter()
            .map(|name| self.cardinality[name] as f64)
            .product::<f64>()
    }

    /// Fraction of region variables whose cardinality was defaulted rather than observed.
    ///
    /// 43.18: "estimates carry uncertainty". This is the only component of that uncertainty the
    /// crate can measure honestly — the error in the cardinality *policy itself* is unmeasurable
    /// without ground-truth domains, and is not folded in here.
    pub fn assumed_cardinality_fraction(&self) -> f64 {
        if self.cardinality.is_empty() {
            return 0.0;
        }
        let assumed = self
            .source
            .values()
            .filter(|source| matches!(source, CardinalitySource::Assumed))
            .count();
        assumed as f64 / self.cardinality.len() as f64
    }

    /// Attaches the all-ones potential to every factor that lacks one.
    ///
    /// Under sum-product this makes the region a counting problem whose answer is the product of
    /// the bound variables' cardinalities — correct, and useful for exercising an executor on a
    /// world-derived structure, but not evidence about anything biological. The region records
    /// that in its assumptions so nothing downstream can present the result as a measurement.
    pub fn with_uniform_tables(mut self) -> Result<QueryRegion, RegionError> {
        for factor in &mut self.factors {
            if factor.table.is_some() {
                continue;
            }
            let entries: f64 = factor
                .scope
                .iter()
                .map(|name| self.cardinality[name] as f64)
                .product();
            if entries > MAX_MATERIALISED_TABLE_ENTRIES as f64 {
                return Err(RegionError::UniformTableTooLarge {
                    factor: factor.id.clone(),
                    entries,
                    cap: MAX_MATERIALISED_TABLE_ENTRIES,
                });
            }
            factor.table = Some(vec![1.0; entries as usize]);
        }
        self.assumptions.push(
            "factor potentials are uniformly one; results count assignments and carry no evidential content"
                .to_string(),
        );
        Ok(self)
    }
}

pub struct QueryRegionBuilder {
    label: String,
    semiring: Semiring,
    cardinality: BTreeMap<String, usize>,
    source: BTreeMap<String, CardinalitySource>,
    factors: Vec<RegionFactor>,
    free: BTreeSet<String>,
    assumptions: Vec<String>,
    provenance: RegionProvenance,
}

impl QueryRegionBuilder {
    pub fn semiring(mut self, semiring: Semiring) -> Self {
        self.semiring = semiring;
        self
    }

    /// Declares a variable whose domain size is known.
    pub fn variable(self, name: impl Into<String>, cardinality: usize) -> Self {
        self.observed_variable(name, cardinality)
    }

    pub fn observed_variable(mut self, name: impl Into<String>, cardinality: usize) -> Self {
        let name = name.into();
        self.cardinality.insert(name.clone(), cardinality);
        self.source.insert(name, CardinalitySource::Observed);
        self
    }

    pub fn assumed_variable(mut self, name: impl Into<String>, cardinality: usize) -> Self {
        let name = name.into();
        self.cardinality.insert(name.clone(), cardinality);
        self.source.insert(name, CardinalitySource::Assumed);
        self
    }

    pub fn factor(mut self, factor: RegionFactor) -> Self {
        self.factors.push(factor);
        self
    }

    pub fn free(mut self, name: impl Into<String>) -> Self {
        self.free.insert(name.into());
        self
    }

    pub fn assumption(mut self, note: impl Into<String>) -> Self {
        self.assumptions.push(note.into());
        self
    }

    pub fn provenance(mut self, provenance: RegionProvenance) -> Self {
        self.provenance = provenance;
        self
    }

    pub fn build(self) -> Result<QueryRegion, RegionError> {
        for (name, card) in &self.cardinality {
            if *card == 0 {
                return Err(RegionError::ZeroCardinality {
                    variable: name.clone(),
                });
            }
        }

        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for factor in &self.factors {
            if !seen.insert(factor.id.as_str()) {
                return Err(RegionError::DuplicateFactorId {
                    factor: factor.id.clone(),
                });
            }
            let mut scope_seen: BTreeSet<&str> = BTreeSet::new();
            let mut expected = 1usize;
            for variable in &factor.scope {
                if !self.cardinality.contains_key(variable) {
                    return Err(RegionError::UnknownVariable {
                        factor: factor.id.clone(),
                        variable: variable.clone(),
                    });
                }
                if !scope_seen.insert(variable.as_str()) {
                    return Err(RegionError::RepeatedScopeVariable {
                        factor: factor.id.clone(),
                        variable: variable.clone(),
                    });
                }
                expected = expected.saturating_mul(self.cardinality[variable]);
            }
            if let Some(table) = &factor.table {
                if table.len() != expected {
                    return Err(RegionError::TableSizeMismatch {
                        factor: factor.id.clone(),
                        expected,
                        actual: table.len(),
                    });
                }
                for (index, value) in table.iter().enumerate() {
                    if !self.semiring.admits(*value) {
                        return Err(RegionError::InadmissibleTableEntry {
                            factor: factor.id.clone(),
                            index,
                            value: *value,
                            semiring: self.semiring.as_str(),
                        });
                    }
                }
            }
        }

        for variable in &self.free {
            if !self.cardinality.contains_key(variable) {
                return Err(RegionError::FreeVariableNotInRegion {
                    variable: variable.clone(),
                });
            }
        }

        Ok(QueryRegion {
            label: self.label,
            semiring: self.semiring,
            cardinality: self.cardinality,
            source: self.source,
            factors: self.factors,
            free: self.free,
            assumptions: self.assumptions,
            provenance: self.provenance,
        })
    }
}
