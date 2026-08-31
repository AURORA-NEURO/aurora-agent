//! The tabular adapter: CSV and TSV under an explicit mapping policy.
//!
//! Tables are the modality where silent loss is easiest and most damaging. A column header is
//! a name with no schema behind it, so every reader has to decide what `dose` means, whether
//! `2.5` is milligrams or milligrams per kilogram, whether `chr7:140753336` is GRCh37 or
//! GRCh38, and whether `BRAF` is a symbol or an HGNC-versioned mapping. Each of those
//! decisions is one of 28.00's forbidden silent coercions, and a reader that makes them
//! quietly produces a world that is plausible and wrong.
//!
//! This adapter therefore takes a [`TabularProfile`] — 40.17's "mapping policy" input — and
//! declines to act without one. Anything the policy does not describe is reported as loss:
//!
//! - a header column with no rule becomes [`LossKind::UnmappedColumn`];
//! - a column whose type the policy leaves to the adapter, and which the adapter cannot settle
//!   unambiguously from the data, becomes [`LossKind::TypeUndetermined`] and is carried as
//!   text — the type is lost, the characters are not;
//! - a magnitude whose unit the policy says cannot be carried becomes
//!   [`LossKind::UnpreservedUnit`], and likewise for coordinate frames and ontology versions;
//! - a decimal literal that does not survive the round trip through `f64` becomes
//!   [`LossKind::PrecisionReduced`], named at the exact cell.
//!
//! Dates are not parsed. 28.00 forbids "converting event dates to relative order without
//! preserving source dates", and every date column in clinical data is ambiguous between
//! day-first and month-first until someone says which; the policy can declare such a column
//! `Text`, which carries the characters verbatim and leaves the interpretation to a layer that
//! knows the study.

use crate::adapter::{Adapter, AdapterManifest, ConformanceLevel};
use crate::csv::{Table, COMMA};
use crate::error::AdapterError;
use crate::fact::{FactDraft, ValueQualifiers};
use crate::ingestion::Ingestion;
use crate::location::SourceLocation;
use crate::loss::{LossAudit, LossKind, LossSeverity};
use crate::source::{Source, SourceManifest};
use bioprism_ids::{python_repr_f64, ContentHash, VariableName};
use bioprism_scope::ScopeKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const TABULAR_ADAPTER: &str = "bioprism.tabular";
pub const TABULAR_ADAPTER_VERSION: &str = "0.1.0";

/// The types a cell may hold once the policy has spoken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    Text,
    Integer,
    Real,
    Boolean,
}

impl ValueType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ValueType::Text => "text",
            ValueType::Integer => "integer",
            ValueType::Real => "real",
            ValueType::Boolean => "boolean",
        }
    }
}

/// Who decides a column's type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypePolicy {
    /// The policy states the type. A cell that contradicts it is an error, not a coercion.
    Declared(ValueType),
    /// The adapter examines every cell in the column and accepts only an unambiguous answer.
    ///
    /// Note that `Text` is *not* a possible outcome of this examination. Falling back to text
    /// would be a guess dressed as a determination: a column of strings might be identifiers,
    /// categorical codes, dates or ontology terms, and those are different things. When the
    /// data do not settle the question the column is carried as text *and* declared lost.
    Determine,
}

/// Whether the unit of a measurement survives the mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitPolicy {
    /// The quantity has no unit.
    Dimensionless,
    /// The unit is known and travels inside the emitted value.
    Carried(String),
    /// The unit is known but the target variable cannot express it. Declared as loss so that a
    /// downstream reader knows the bare number is not dimensionless.
    Dropped { unit: String, reason: String },
}

/// Whether the coordinate frame of a position survives the mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FramePolicy {
    /// The value is not positional.
    NotPositional,
    /// The frame — a genome build, an imaging space — travels inside the emitted value.
    Carried(String),
    /// The frame is known and not carried. This is the one 28.00 singles out as "changing
    /// reference genome coordinates", and it is silent unless declared here.
    Dropped { frame: String, reason: String },
}

/// Whether a controlled-vocabulary term is mapped, and against which version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OntologyPolicy {
    /// The value is not a vocabulary term.
    NotOntological,
    /// Mapped against a named, versioned vocabulary.
    Mapped { ontology: String, version: String },
    /// Known to be a vocabulary term but carried as free text. A mapping without a recorded
    /// version is this case too, not [`OntologyPolicy::Mapped`].
    Unmapped { ontology: String },
}

/// How one column becomes evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableMapping {
    /// The variable the emitted facts provide, in the sense of 43.04.
    pub variable: String,
    pub value_type: TypePolicy,
    pub unit: UnitPolicy,
    pub frame: FramePolicy,
    pub ontology: OntologyPolicy,
    /// Tags for the emitted facts. `protected` here is what the compiler's protected closure
    /// (43.13) keys on, so it is a policy decision rather than something inferred from a name.
    pub tags: BTreeSet<String>,
}

impl VariableMapping {
    /// A dimensionless, non-positional, non-ontological variable whose type the adapter is
    /// asked to determine. The starting point for the builder methods below.
    pub fn new(variable: impl Into<String>) -> Self {
        VariableMapping {
            variable: variable.into(),
            value_type: TypePolicy::Determine,
            unit: UnitPolicy::Dimensionless,
            frame: FramePolicy::NotPositional,
            ontology: OntologyPolicy::NotOntological,
            tags: BTreeSet::new(),
        }
    }

    pub fn typed(mut self, value_type: ValueType) -> Self {
        self.value_type = TypePolicy::Declared(value_type);
        self
    }

    pub fn unit(mut self, unit: UnitPolicy) -> Self {
        self.unit = unit;
        self
    }

    pub fn frame(mut self, frame: FramePolicy) -> Self {
        self.frame = frame;
        self
    }

    pub fn ontology(mut self, ontology: OntologyPolicy) -> Self {
        self.ontology = ontology;
        self
    }

    pub fn tagged<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    fn validate(&self) -> Result<(), AdapterError> {
        VariableName::parse(self.variable.clone()).map_err(|error| {
            AdapterError::InvalidSource(format!(
                "variable mapping {:?} is not a valid variable: {error}",
                self.variable
            ))
        })?;
        if self.tags.len() > 16_384 {
            return Err(AdapterError::InvalidSource(
                "variable mapping tags exceed their item bound".into(),
            ));
        }
        for tag in &self.tags {
            validate_text("variable tag", tag, 512)?;
        }
        validate_unit(&self.unit)?;
        validate_frame(&self.frame)?;
        validate_ontology(&self.ontology)?;
        Ok(())
    }
}

/// What a column is for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnRole {
    /// The column binds a scope dimension on every fact from the same record. An empty cell in
    /// such a column is an error: 40.17 invariant 2 forbids guessing identity, and emitting the
    /// fact without the binding would silently widen the scope it is claimed to hold in.
    ScopeDimension { dimension: String },
    /// The column becomes facts.
    Variable(VariableMapping),
    /// The column's text is appended to the provenance of every fact from the same record.
    Provenance,
    /// The column is deliberately not carried. Still a loss, and still reported — the point of
    /// declaring it is to record the author's reason and severity, not to make it disappear.
    Ignored {
        reason: String,
        severity: LossSeverity,
    },
}

/// The mapping policy for one table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabularProfile {
    /// Value bound to the `dataset` scope dimension on every emitted fact, so that facts from
    /// two tables never appear to hold in the same scope by accident.
    pub dataset: String,
    pub columns: BTreeMap<String, ColumnRole>,
    /// Tags added to every emitted fact, on top of each variable's own.
    pub tags: BTreeSet<String>,
    /// Whether an empty cell means "absent" (a JSON null) or the empty value of its type.
    ///
    /// 28.00 forbids "treating missing values as zero"; neither answer here does that, but the
    /// two are genuinely different claims about the source and only its author knows which
    /// convention it follows, so there is no default that is safe to apply silently.
    pub empty_is_null: bool,
    /// Field delimiter, as a single ASCII byte.
    pub delimiter: u8,
}

impl TabularProfile {
    pub fn new(dataset: impl Into<String>) -> Self {
        TabularProfile {
            dataset: dataset.into(),
            columns: BTreeMap::new(),
            tags: BTreeSet::new(),
            empty_is_null: true,
            delimiter: COMMA,
        }
    }

    pub fn column(mut self, name: impl Into<String>, role: ColumnRole) -> Self {
        self.columns.insert(name.into(), role);
        self
    }

    pub fn scope(self, name: impl Into<String>, dimension: impl Into<String>) -> Self {
        let dimension = dimension.into();
        self.column(name, ColumnRole::ScopeDimension { dimension })
    }

    pub fn variable(self, name: impl Into<String>, mapping: VariableMapping) -> Self {
        self.column(name, ColumnRole::Variable(mapping))
    }

    pub fn ignore(
        self,
        name: impl Into<String>,
        reason: impl Into<String>,
        severity: LossSeverity,
    ) -> Self {
        self.column(
            name,
            ColumnRole::Ignored {
                reason: reason.into(),
                severity,
            },
        )
    }

    pub fn tagged<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    pub fn with_delimiter(mut self, delimiter: u8) -> Self {
        self.delimiter = delimiter;
        self
    }

    /// Digest of the policy, recorded in the source manifest so that two worlds built from the
    /// same bytes under different policies are distinguishable without diffing their facts.
    pub fn digest(&self) -> Option<ContentHash> {
        let value = serde_json::to_value(self).ok()?;
        ContentHash::of_value(&value).ok()
    }

    /// Scope dimensions this policy may bind, for the adapter manifest.
    pub fn scope_dimensions(&self) -> BTreeSet<String> {
        let mut dimensions = BTreeSet::from(["dataset".to_string()]);
        for role in self.columns.values() {
            if let ColumnRole::ScopeDimension { dimension } = role {
                dimensions.insert(dimension.clone());
            }
        }
        dimensions
    }

    pub fn validate(&self) -> Result<(), AdapterError> {
        validate_text("dataset", &self.dataset, 512)?;
        if self.columns.len() > 16_384 {
            return Err(AdapterError::InvalidSource(
                "tabular mapping exceeds its column bound".into(),
            ));
        }
        if self.tags.len() > 16_384 {
            return Err(AdapterError::InvalidSource(
                "tabular profile tags exceed their item bound".into(),
            ));
        }
        for tag in &self.tags {
            validate_text("tabular profile tag", tag, 512)?;
        }
        if self.delimiter == 0 || matches!(self.delimiter, b'"' | b'\r' | b'\n') {
            return Err(AdapterError::InvalidSource(
                "tabular delimiter conflicts with CSV syntax".into(),
            ));
        }
        let mut scope_dimensions = BTreeSet::new();
        for (column, role) in &self.columns {
            validate_text("mapping column", column, 512)?;
            match role {
                ColumnRole::ScopeDimension { dimension } => {
                    validate_text("scope dimension", dimension, 512)?;
                    if !scope_dimensions.insert(dimension) {
                        return Err(AdapterError::InvalidSource(format!(
                            "scope dimension {dimension:?} is mapped by more than one column"
                        )));
                    }
                }
                ColumnRole::Variable(mapping) => mapping.validate()?,
                ColumnRole::Provenance => {}
                ColumnRole::Ignored { reason, .. } => {
                    validate_text("ignored-column reason", reason, 1024)?;
                }
            }
        }
        Ok(())
    }
}

/// Normalizes a delimited table into facts under a [`TabularProfile`].
#[derive(Debug, Clone)]
pub struct TabularAdapter {
    profile: TabularProfile,
}

impl TabularAdapter {
    pub fn new(profile: TabularProfile) -> Self {
        TabularAdapter { profile }
    }

    pub fn profile(&self) -> &TabularProfile {
        &self.profile
    }
}

impl Adapter for TabularAdapter {
    fn name(&self) -> &str {
        TABULAR_ADAPTER
    }

    fn manifest(&self) -> AdapterManifest {
        AdapterManifest::new(
            TABULAR_ADAPTER,
            TABULAR_ADAPTER_VERSION,
            ConformanceLevel::Normalize,
        )
        .accepting(crate::probe::CSV_FORMATS)
        .declaring([
            LossKind::UnmappedColumn,
            LossKind::UnpreservedUnit,
            LossKind::CoordinateFrameNotCarried,
            LossKind::PrecisionReduced,
            LossKind::ProvenanceUnavailable,
            LossKind::OntologyTermUnmapped,
            LossKind::TypeUndetermined,
        ])
        .binding(self.profile.scope_dimensions())
    }

    fn ingest(&self, source: &Source) -> Result<Ingestion, AdapterError> {
        self.profile.validate()?;
        source.require_format(TABULAR_ADAPTER, &crate::probe::CSV_FORMATS)?;
        let bytes = source.as_bytes(TABULAR_ADAPTER)?;
        let table = Table::parse_with(&source.id, bytes, self.profile.delimiter)?;

        let mut audit = LossAudit::new();
        let mut facts = Vec::new();

        for column in self.profile.columns.keys() {
            if table.column_index(column).is_none() {
                return Err(AdapterError::schema_drift(
                    SourceLocation::column(&source.id, column),
                    format!("column {column:?} named by the mapping policy"),
                ));
            }
        }

        let mut resolved: BTreeMap<&str, ValueType> = BTreeMap::new();
        for header in table.headers() {
            let location = table.column_location(header);
            match self.profile.columns.get(header) {
                None => {
                    audit.record(
                        LossKind::UnmappedColumn,
                        LossSeverity::Degrading,
                        location,
                        "the mapping policy has no rule for this column, so its content is \
                         absent from the world",
                    );
                }
                Some(ColumnRole::Ignored { reason, severity }) => {
                    audit.record(
                        LossKind::UnmappedColumn,
                        *severity,
                        location,
                        format!("deliberately not carried: {reason}"),
                    );
                }
                Some(ColumnRole::ScopeDimension { .. }) | Some(ColumnRole::Provenance) => {
                    audit.mapped(location);
                }
                Some(ColumnRole::Variable(mapping)) => {
                    audit.mapped(location.clone());
                    declare_qualifier_losses(&mut audit, mapping, &location);
                    match determine_type(mapping, &table, header) {
                        Some(value_type) => {
                            resolved.insert(header.as_str(), value_type);
                        }
                        None => {
                            audit.record(
                                LossKind::TypeUndetermined,
                                LossSeverity::Degrading,
                                location,
                                "the cells do not settle this column's type unambiguously; it \
                                 is carried as text rather than guessed",
                            );
                        }
                    }
                }
            }
        }

        let source_provenance = match &source.provenance {
            Some(provenance) if !provenance.is_empty() => provenance.strings(),
            _ => {
                audit.record(
                    LossKind::ProvenanceUnavailable,
                    LossSeverity::Degrading,
                    SourceLocation::source(&source.id),
                    "the caller supplied no upstream accession, version or retrieval time, and \
                     the adapter will not infer one from the filesystem",
                );
                Vec::new()
            }
        };

        for (index, record) in table.records().iter().enumerate() {
            let ordinal = index as u64 + 1;
            let mut scope = ScopeKey::new().exact("dataset", &self.profile.dataset);
            let mut row_provenance = source_provenance.clone();

            for (position, header) in table.headers().iter().enumerate() {
                match self.profile.columns.get(header) {
                    Some(ColumnRole::ScopeDimension { dimension }) => {
                        let cell = record[position].trim();
                        if cell.is_empty() {
                            return Err(AdapterError::ambiguous_identity(
                                table.cell_location(ordinal, header),
                                dimension.clone(),
                            ));
                        }
                        scope = scope.exact(dimension, cell);
                    }
                    Some(ColumnRole::Provenance) => {
                        row_provenance.push(format!("{header}={}", record[position]));
                    }
                    _ => {}
                }
            }

            for (position, header) in table.headers().iter().enumerate() {
                let Some(ColumnRole::Variable(mapping)) = self.profile.columns.get(header) else {
                    continue;
                };
                let location = table.cell_location(ordinal, header);
                let cell = &record[position];
                let value = encode_cell(
                    &mut audit,
                    cell,
                    resolved.get(header.as_str()).copied(),
                    self.profile.empty_is_null,
                    &location,
                )?;

                let qualifiers = qualifiers_for(mapping);
                let mut provenance = row_provenance.clone();
                provenance.push(location.locator());

                let mut draft = FactDraft::new(
                    format!(
                        "fact.{}.r{ordinal}.{}",
                        self.profile.dataset, mapping.variable
                    ),
                    &mapping.variable,
                    qualifiers.apply(value),
                    scope.clone(),
                    location,
                )
                .provenances(provenance)
                .tags(self.profile.tags.iter().cloned());
                draft = draft.tags(mapping.tags.iter().cloned());
                facts.push(draft.build()?);
            }
        }

        let manifest = SourceManifest {
            source_id: source.id.clone(),
            declared_format: source.declared_format.clone(),
            source_digest: ContentHash::of_bytes(bytes),
            byte_length: Some(bytes.len() as u64),
            adapter: TABULAR_ADAPTER.to_string(),
            adapter_version: TABULAR_ADAPTER_VERSION.to_string(),
            profile_digest: self.profile.digest(),
            provenance: source.provenance.clone(),
        };

        Ingestion::new(manifest, facts, audit.finish())
    }
}

fn validate_unit(unit: &UnitPolicy) -> Result<(), AdapterError> {
    match unit {
        UnitPolicy::Dimensionless => Ok(()),
        UnitPolicy::Carried(value) => validate_text("unit", value, 256),
        UnitPolicy::Dropped { unit, reason } => {
            validate_text("dropped unit", unit, 256)?;
            validate_text("dropped unit reason", reason, 1024)
        }
    }
}

fn validate_frame(frame: &FramePolicy) -> Result<(), AdapterError> {
    match frame {
        FramePolicy::NotPositional => Ok(()),
        FramePolicy::Carried(value) => validate_text("frame", value, 256),
        FramePolicy::Dropped { frame, reason } => {
            validate_text("dropped frame", frame, 256)?;
            validate_text("dropped frame reason", reason, 1024)
        }
    }
}

fn validate_ontology(ontology: &OntologyPolicy) -> Result<(), AdapterError> {
    match ontology {
        OntologyPolicy::NotOntological => Ok(()),
        OntologyPolicy::Mapped { ontology, version } => {
            validate_text("ontology", ontology, 256)?;
            validate_text("ontology version", version, 256)
        }
        OntologyPolicy::Unmapped { ontology } => validate_text("unmapped ontology", ontology, 256),
    }
}

fn validate_text(field: &str, value: &str, maximum: usize) -> Result<(), AdapterError> {
    if value.is_empty() || value.trim() != value {
        return Err(AdapterError::InvalidSource(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > maximum || value.chars().any(char::is_control) {
        return Err(AdapterError::InvalidSource(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

/// Records the losses implied by a variable's qualifier policies, once per column.
fn declare_qualifier_losses(
    audit: &mut LossAudit,
    mapping: &VariableMapping,
    location: &SourceLocation,
) {
    if let UnitPolicy::Dropped { unit, reason } = &mapping.unit {
        audit.record(
            LossKind::UnpreservedUnit,
            LossSeverity::Blocking,
            location.clone(),
            format!("measured in {unit} but carried as a bare magnitude: {reason}"),
        );
    }
    if let FramePolicy::Dropped { frame, reason } = &mapping.frame {
        audit.record(
            LossKind::CoordinateFrameNotCarried,
            LossSeverity::Blocking,
            location.clone(),
            format!("expressed in frame {frame} but carried without it: {reason}"),
        );
    }
    if let OntologyPolicy::Unmapped { ontology } = &mapping.ontology {
        audit.record(
            LossKind::OntologyTermUnmapped,
            LossSeverity::Degrading,
            location.clone(),
            format!(
                "terms belong to {ontology} but are carried as free text with no mapping version"
            ),
        );
    }
}

fn qualifiers_for(mapping: &VariableMapping) -> ValueQualifiers {
    let mut qualifiers = ValueQualifiers::default();
    if let UnitPolicy::Carried(unit) = &mapping.unit {
        qualifiers.unit = Some(unit.clone());
    }
    if let FramePolicy::Carried(frame) = &mapping.frame {
        qualifiers.frame = Some(frame.clone());
    }
    if let OntologyPolicy::Mapped { ontology, version } = &mapping.ontology {
        qualifiers.ontology = Some(ontology.clone());
        qualifiers.ontology_version = Some(version.clone());
    }
    qualifiers
}

/// Settles a column's type, or returns `None` when the data do not settle it.
///
/// The scan covers every non-empty cell rather than a sample. A sampled inference is a
/// different kind of guess: it is right until the row that breaks it, and that row is usually
/// the interesting one.
fn determine_type(mapping: &VariableMapping, table: &Table, header: &str) -> Option<ValueType> {
    if let TypePolicy::Declared(value_type) = mapping.value_type {
        return Some(value_type);
    }
    let cells = table.column(header)?;
    let present: Vec<&str> = cells
        .iter()
        .map(|cell| cell.trim())
        .filter(|cell| !cell.is_empty())
        .collect();
    if present.is_empty() {
        return None;
    }
    if present
        .iter()
        .all(|cell| *cell == "true" || *cell == "false")
    {
        return Some(ValueType::Boolean);
    }
    if present.iter().all(|cell| cell.parse::<i64>().is_ok()) {
        return Some(ValueType::Integer);
    }
    if present
        .iter()
        .all(|cell| cell.parse::<f64>().is_ok_and(f64::is_finite))
    {
        return Some(ValueType::Real);
    }
    None
}

/// Turns one cell into a JSON value, recording any precision loss against that exact cell.
fn encode_cell(
    audit: &mut LossAudit,
    cell: &str,
    value_type: Option<ValueType>,
    empty_is_null: bool,
    location: &SourceLocation,
) -> Result<Value, AdapterError> {
    let trimmed = cell.trim();
    if trimmed.is_empty() && empty_is_null {
        return Ok(Value::Null);
    }

    match value_type {
        None | Some(ValueType::Text) => Ok(Value::String(cell.to_string())),
        Some(ValueType::Boolean) => match trimmed {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            other => Err(AdapterError::type_mismatch(
                location.clone(),
                "boolean, spelled exactly true or false",
                other,
            )),
        },
        Some(ValueType::Integer) => trimmed
            .parse::<i64>()
            .map(Value::from)
            .map_err(|_| AdapterError::type_mismatch(location.clone(), "integer", trimmed)),
        Some(ValueType::Real) => {
            let parsed = trimmed.parse::<f64>().map_err(|_| {
                AdapterError::type_mismatch(location.clone(), "real number", trimmed)
            })?;
            if !parsed.is_finite() {
                return Err(AdapterError::type_mismatch(
                    location.clone(),
                    "finite real number",
                    trimmed,
                ));
            }
            if !round_trips(trimmed, parsed) {
                audit.record(
                    LossKind::PrecisionReduced,
                    LossSeverity::Degrading,
                    location.clone(),
                    format!(
                        "the literal {trimmed} is not representable in f64; the world carries \
                         {} instead",
                        python_repr_f64(parsed)
                    ),
                );
            }
            Ok(Value::from(parsed))
        }
    }
}

/// True when the emitted `f64` denotes the same decimal number the source wrote.
///
/// Comparing the raw strings would flag `40` against `40.0`; comparing the parsed doubles
/// would flag nothing, since re-parsing the emitted form always yields the same double. The
/// comparison that answers the actual question is between the two literals reduced to a
/// normalized (sign, significant digits, exponent) form.
fn round_trips(literal: &str, parsed: f64) -> bool {
    match (
        normalize_decimal(literal),
        normalize_decimal(&python_repr_f64(parsed)),
    ) {
        (Some(source), Some(emitted)) => source == emitted,
        _ => false,
    }
}

/// Reduces a decimal literal to `(negative, significant digits, exponent)`.
fn normalize_decimal(text: &str) -> Option<(bool, String, i32)> {
    let text = text.trim();
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };

    let (mantissa, exponent) = match rest.find(['e', 'E']) {
        Some(index) => (&rest[..index], rest[index + 1..].parse::<i32>().ok()?),
        None => (rest, 0),
    };
    let (integral, fractional) = match mantissa.find('.') {
        Some(index) => (&mantissa[..index], &mantissa[index + 1..]),
        None => (mantissa, ""),
    };
    if integral.is_empty() && fractional.is_empty() {
        return None;
    }
    if !integral.chars().all(|c| c.is_ascii_digit())
        || !fractional.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }

    let digits = format!("{integral}{fractional}");
    let exponent = exponent.checked_sub(i32::try_from(fractional.len()).ok()?)?;
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some((false, "0".to_string(), 0));
    }
    let trimmed = digits.trim_end_matches('0');
    let removed = i32::try_from(digits.len() - trimmed.len()).ok()?;
    Some((
        negative,
        trimmed.to_string(),
        exponent.checked_add(removed)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_numbers_written_differently_normalize_to_the_same_form() {
        assert_eq!(normalize_decimal("40"), normalize_decimal("40.0"));
        assert_eq!(normalize_decimal("0.5"), normalize_decimal("5e-1"));
        assert_eq!(normalize_decimal("-0.0"), normalize_decimal("0"));
    }

    #[test]
    fn a_literal_f64_can_hold_exactly_round_trips() {
        assert!(round_trips("2.5", 2.5));
        assert!(round_trips("40", 40.0));
    }

    #[test]
    fn a_literal_with_more_digits_than_f64_can_hold_does_not_round_trip() {
        let literal = "0.12345678901234567890123";
        let parsed: f64 = literal.parse().unwrap();
        assert!(!round_trips(literal, parsed));
    }

    #[test]
    fn a_profile_rejects_duplicate_scope_dimensions() {
        let profile = TabularProfile::new("dataset")
            .scope("subject_a", "subject")
            .scope("subject_b", "subject");
        assert!(matches!(
            profile.validate(),
            Err(AdapterError::InvalidSource(_))
        ));
    }

    #[test]
    fn a_profile_rejects_an_empty_carried_unit() {
        let profile = TabularProfile::new("dataset").variable(
            "dose",
            VariableMapping::new("dose").unit(UnitPolicy::Carried("".into())),
        );
        assert!(matches!(
            profile.validate(),
            Err(AdapterError::InvalidSource(_))
        ));
    }
}
