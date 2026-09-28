//! MCP mission API, execution, tracing, cancellation, and authoritative schema validation.
//!
//! Blueprint 11.11 keeps mission execution bounded and reviewable; these helpers preserve
//! its trace and input-validation boundary independently of the domain tool catalogue.
//! Validation implements the embedded catalogue's active JSON Schema constraints, including
//! patterned strings, dynamic map values, and property-name rules. Pattern syntax outside the
//! bounded subset in use by the catalogue is rejected rather than ignored.

use super::*;

enum ParallelPending<'a> {
    Blocked {
        step: &'a MissionStep,
        error: String,
    },
    Refused {
        step: &'a MissionStep,
        arguments_digest: Option<String>,
        bytes: usize,
        error: String,
    },
    Call {
        step: &'a MissionStep,
        arguments: Value,
        arguments_digest: String,
    },
}

struct ParallelCallOutcome {
    wire: Value,
    bytes: usize,
    is_error: bool,
    error: Option<String>,
}

fn encode_mission_report(report: MissionReport, context: &str) -> Result<Value, String> {
    let mut report = report;
    report.claim_lineage = mission_claim_lineage_with_review(
        &report.claim_requests,
        &report.results,
        report.evaluator_review.as_ref(),
    );
    let mut output = serde_json::to_value(report).map_err(|error| format!("{context}: {error}"))?;
    output["ok"] = json!(true);
    output["workflow"] = json!("agent_mission");
    output["mission_schema_version"] = json!(MISSION_SCHEMA_VERSION);
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn trace_event(
    report: &mut MissionReport,
    event: &str,
    wave: Option<usize>,
    step: Option<&MissionStep>,
    status: Option<&str>,
    arguments_digest: Option<String>,
    bytes: usize,
    detail: Option<String>,
) {
    report.execution_trace.push(MissionTraceEvent {
        sequence: report.execution_trace.len(),
        event: event.into(),
        wave,
        step_id: step.map(|value| value.id.clone()),
        tool: step.map(|value| value.tool.clone()),
        status: status.map(str::to_string),
        arguments_digest,
        bytes,
        detail,
    });
    if let (Some(observer), Some(event)) = (
        report.trace_observer.as_ref(),
        report.execution_trace.last(),
    ) {
        if let Ok(value) = serde_json::to_value(event) {
            (observer.0)(value);
        }
    }
}

fn cancellation_requested(cancellation: Option<&AtomicBool>) -> bool {
    cancellation.is_some_and(|flag| flag.load(Ordering::Acquire))
}

fn cancel_remaining_steps(report: &mut MissionReport, request: &MissionRequest, from_wave: usize) {
    let existing = report
        .results
        .iter()
        .map(|result| result.id.clone())
        .collect::<BTreeSet<_>>();
    let steps = request
        .steps
        .iter()
        .map(|step| (step.id.as_str(), step))
        .collect::<BTreeMap<_, _>>();
    let waves = report.plan.waves.clone();
    for (wave_index, wave) in waves.iter().enumerate().skip(from_wave) {
        for step_id in wave {
            if existing.contains(step_id) {
                continue;
            }
            let Some(step) = steps.get(step_id.as_str()) else {
                continue;
            };
            let detail = "mission cancellation was requested before this step was dispatched";
            report.cancelled += 1;
            report.results.push(MissionStepResult {
                id: step.id.clone(),
                tool: step.tool.clone(),
                status: "cancelled".into(),
                required: step.required,
                arguments_digest: None,
                bytes: 0,
                wire: None,
                error: Some(detail.into()),
            });
            trace_event(
                report,
                "step.cancelled",
                Some(wave_index),
                Some(step),
                Some("cancelled"),
                None,
                0,
                Some(detail.into()),
            );
        }
    }
}

fn finish_cancelled_mission(report: MissionReport, context: &str) -> Result<Value, String> {
    let mut report = report;
    report.mission_status = "cancelled".into();
    let returned_bytes = report.returned_bytes;
    trace_event(
        &mut report,
        "mission.cancelled",
        None,
        None,
        Some("cancelled"),
        None,
        returned_bytes,
        Some("cooperative cancellation stopped future dispatches; in-flight calls were allowed to return".into()),
    );
    trace_event(
        &mut report,
        "mission.completed",
        None,
        None,
        Some("cancelled"),
        None,
        returned_bytes,
        None,
    );
    encode_mission_report(report, context)
}

const MAX_MISSION_SCHEMA_BYTES: usize = 1_000_000;
const MAX_MISSION_SCHEMA_DEPTH: usize = 100;
const MAX_MISSION_SCHEMA_ISSUES: usize = 64;

/// Stack reservation for every tool dispatch thread and for the mission executor thread.
///
/// Unoptimised builds give the widest dispatch frames multi-megabyte activation records, and a
/// mission re-enters `call_tool` from inside its own execution frame; the default thread stack
/// aborted with `STATUS_STACK_OVERFLOW`. Sixteen mebibytes is double the reservation the CLI
/// worker thread uses for a single (non re-entrant) dispatch chain.
const TOOL_DISPATCH_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Run `body` on a scoped thread carrying [`TOOL_DISPATCH_STACK_BYTES`], resuming any panic on
/// the caller so panic behaviour is unchanged; only a failure to *start* the thread is returned.
///
/// Tool calls can enter large domain handlers and missions can re-enter dispatch while retaining
/// their execution frames. Keeping that work on a deliberately sized thread prevents correctness
/// from depending on the transport's or embedding API's caller stack. The static tool catalogue
/// has a separate cached JSON boundary and does not use this dispatch reservation.
pub(super) fn on_dispatch_stack<T: Send>(
    name: &'static str,
    body: impl FnOnce() -> T + Send,
) -> Result<T, std::io::Error> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name(name.into())
            .stack_size(TOOL_DISPATCH_STACK_BYTES)
            .spawn_scoped(scope, body)
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|payload| std::panic::resume_unwind(payload))
            })
    })
}

#[derive(Debug, Clone)]
pub(super) struct MissionSchemaIssue {
    pub(super) path: String,
    pub(super) code: &'static str,
    pub(super) message: String,
}

#[derive(Debug, Clone)]
pub(super) struct MissionSchemaReport {
    pub(super) digest: String,
    pub(super) issues: Vec<MissionSchemaIssue>,
}

pub(super) fn validate_mission_tool_arguments(
    tool: &str,
    arguments: &Value,
) -> Result<Option<MissionSchemaReport>, String> {
    // Borrow the cached schema directly. A mission step should pay for its one authoritative
    // schema lookup, not rebuild and clone all 893 tool definitions before every validation.
    let Some(definition) = find_tool_definition(tool) else {
        return Ok(None);
    };
    let schema = definition
        .get("inputSchema")
        .ok_or_else(|| format!("tool `{tool}` has no authoritative inputSchema"))?;
    let schema_bytes = serde_json::to_vec(schema)
        .map_err(|error| format!("cannot encode authoritative schema for `{tool}`: {error}"))?;
    if schema_bytes.len() > MAX_MISSION_SCHEMA_BYTES {
        return Err(format!(
            "authoritative schema for `{tool}` exceeds the {MAX_MISSION_SCHEMA_BYTES}-byte safety bound"
        ));
    }
    let digest = bioprism_ids::ContentHash::of_value(schema)
        .map_err(|error| format!("cannot hash authoritative schema for `{tool}`: {error}"))?
        .to_string();
    let mut report = MissionSchemaReport {
        digest,
        issues: Vec::new(),
    };
    validate_mission_schema_value(arguments, schema, schema, "", 0, &mut report);
    Ok(Some(report))
}

pub(super) fn schema_failure_detail(tool: &str, report: &MissionSchemaReport) -> Option<String> {
    if report.issues.is_empty() {
        return None;
    }
    let details = report
        .issues
        .iter()
        .take(8)
        .map(|issue| {
            let path = if issue.path.is_empty() {
                "$".to_string()
            } else {
                issue.path.clone()
            };
            format!("{} at {path}: {}", issue.code, issue.message)
        })
        .collect::<Vec<_>>();
    let omitted = report.issues.len().saturating_sub(details.len());
    let suffix = if omitted == 0 {
        String::new()
    } else {
        format!("; {omitted} additional issue(s) omitted")
    };
    Some(format!(
        "authoritative schema validation refused tool `{tool}` (schema_digest={}): {}{suffix}",
        report.digest,
        details.join("; ")
    ))
}

fn push_mission_schema_issue(
    report: &mut MissionSchemaReport,
    path: &str,
    code: &'static str,
    message: impl Into<String>,
) {
    if report.issues.len() < MAX_MISSION_SCHEMA_ISSUES {
        report.issues.push(MissionSchemaIssue {
            path: path.into(),
            code,
            message: message.into(),
        });
    }
}

fn mission_schema_path(path: &str, segment: &str) -> String {
    let escaped = segment.replace('~', "~0").replace('/', "~1");
    format!("{path}/{escaped}")
}

fn mission_schema_type_matches(value: &Value, schema_type: &str) -> bool {
    match schema_type {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "null" => value.is_null(),
        _ => true,
    }
}

#[derive(Debug, Clone)]
enum MissionPatternMatcher {
    Literal(char),
    Class(Vec<(char, char)>),
}

impl MissionPatternMatcher {
    fn matches(&self, value: char) -> bool {
        match self {
            MissionPatternMatcher::Literal(expected) => value == *expected,
            MissionPatternMatcher::Class(ranges) => ranges
                .iter()
                .any(|(start, end)| (*start..=*end).contains(&value)),
        }
    }
}

#[derive(Debug, Clone)]
struct MissionPatternAtom {
    matcher: MissionPatternMatcher,
    repetitions: usize,
    one_or_more: bool,
}

/// Match the bounded regular-expression subset used by the checked-in MCP catalogue.
///
/// The catalogue's patterns use anchors, ASCII character classes, `\\d`, literals, exact
/// repetition, and a trailing `+`. Unsupported syntax returns an error so a future schema change
/// cannot silently weaken mission admission.
fn mission_pattern_matches(value: &str, pattern: &str) -> Result<bool, &'static str> {
    const MAX_PATTERN_CHARS: usize = 512;
    const MAX_REPETITIONS: usize = 4096;

    let pattern_chars = pattern.chars().collect::<Vec<_>>();
    if pattern_chars.len() > MAX_PATTERN_CHARS {
        return Err("pattern exceeds the supported size bound");
    }
    if pattern_chars.len() < 2
        || pattern_chars.first() != Some(&'^')
        || pattern_chars.last() != Some(&'$')
    {
        return Err("pattern must be anchored at both ends");
    }

    let mut atoms = Vec::new();
    let mut cursor = 1;
    let end = pattern_chars.len() - 1;
    while cursor < end {
        let matcher = match pattern_chars[cursor] {
            '[' => {
                cursor += 1;
                let mut ranges = Vec::new();
                while cursor < end && pattern_chars[cursor] != ']' {
                    let start = pattern_chars[cursor];
                    if cursor + 2 < end
                        && pattern_chars[cursor + 1] == '-'
                        && pattern_chars[cursor + 2] != ']'
                    {
                        let finish = pattern_chars[cursor + 2];
                        if start > finish {
                            return Err("character class range is reversed");
                        }
                        ranges.push((start, finish));
                        cursor += 3;
                    } else {
                        ranges.push((start, start));
                        cursor += 1;
                    }
                }
                if cursor >= end || ranges.is_empty() {
                    return Err("character class is empty or unclosed");
                }
                cursor += 1;
                MissionPatternMatcher::Class(ranges)
            }
            '\\' => {
                if pattern_chars.get(cursor + 1) != Some(&'d') {
                    return Err("only the \\d character-class escape is supported");
                }
                cursor += 2;
                MissionPatternMatcher::Class(vec![('0', '9')])
            }
            literal if literal.is_ascii_alphanumeric() || matches!(literal, '-' | '_' | ':') => {
                cursor += 1;
                MissionPatternMatcher::Literal(literal)
            }
            _ => return Err("pattern contains unsupported syntax"),
        };

        let mut repetitions = 1;
        let mut one_or_more = false;
        if cursor < end {
            match pattern_chars[cursor] {
                '+' => {
                    one_or_more = true;
                    repetitions = 0;
                    cursor += 1;
                    if cursor != end {
                        return Err("the supported `+` quantifier must be final");
                    }
                }
                '{' => {
                    cursor += 1;
                    let start = cursor;
                    while cursor < end && pattern_chars[cursor].is_ascii_digit() {
                        cursor += 1;
                    }
                    if start == cursor || pattern_chars.get(cursor) != Some(&'}') {
                        return Err("exact repetition must use `{n}` syntax");
                    }
                    repetitions = pattern_chars[start..cursor]
                        .iter()
                        .collect::<String>()
                        .parse::<usize>()
                        .map_err(|_| "exact repetition is outside the supported range")?;
                    if repetitions > MAX_REPETITIONS {
                        return Err("exact repetition exceeds the supported bound");
                    }
                    cursor += 1;
                }
                '*' | '?' => return Err("only exact and trailing `+` quantifiers are supported"),
                _ => {}
            }
        }
        atoms.push(MissionPatternAtom {
            matcher,
            repetitions,
            one_or_more,
        });
    }

    let mut input = value.chars();
    for atom in atoms {
        for _ in 0..atom.repetitions {
            let Some(character) = input.next() else {
                return Ok(false);
            };
            if !atom.matcher.matches(character) {
                return Ok(false);
            }
        }
        if atom.one_or_more {
            let mut consumed = false;
            for character in input {
                if !atom.matcher.matches(character) {
                    return Ok(false);
                }
                consumed = true;
            }
            return Ok(consumed);
        }
    }
    Ok(input.next().is_none())
}

fn mission_schema_matches(
    value: &Value,
    schema: &Value,
    schema_root: &Value,
    path: &str,
    depth: usize,
) -> bool {
    let mut report = MissionSchemaReport {
        digest: String::new(),
        issues: Vec::new(),
    };
    validate_mission_schema_value(value, schema, schema_root, path, depth, &mut report);
    report.issues.is_empty()
}

fn validate_mission_schema_value(
    value: &Value,
    schema: &Value,
    schema_root: &Value,
    path: &str,
    depth: usize,
    report: &mut MissionSchemaReport,
) {
    if depth > MAX_MISSION_SCHEMA_DEPTH {
        push_mission_schema_issue(
            report,
            path,
            "schema_depth_exceeded",
            format!("schema nesting exceeds {MAX_MISSION_SCHEMA_DEPTH} levels"),
        );
        return;
    }
    let Some(schema_object) = schema.as_object() else {
        if schema == &Value::Bool(false) {
            push_mission_schema_issue(
                report,
                path,
                "schema_false",
                "the schema rejects every value",
            );
        }
        return;
    };

    if let Some(reference) = schema_object.get("$ref") {
        let Some(reference) = reference.as_str() else {
            push_mission_schema_issue(
                report,
                path,
                "invalid_schema_ref",
                "authoritative $ref must be a local JSON Pointer string",
            );
            return;
        };
        let Some(pointer) = reference.strip_prefix('#') else {
            push_mission_schema_issue(
                report,
                path,
                "unsupported_schema_ref",
                "external schema references are refused",
            );
            return;
        };
        if !pointer.is_empty() && !pointer.starts_with('/') {
            push_mission_schema_issue(
                report,
                path,
                "invalid_schema_ref",
                "local $ref must use a JSON Pointer fragment",
            );
            return;
        }
        let Some(target) = schema_root.pointer(pointer) else {
            push_mission_schema_issue(
                report,
                path,
                "unresolved_schema_ref",
                format!("local schema reference {reference:?} does not resolve"),
            );
            return;
        };
        if !target.is_object() && !target.is_boolean() {
            push_mission_schema_issue(
                report,
                path,
                "invalid_schema_ref_target",
                format!("local schema reference {reference:?} does not target a schema"),
            );
            return;
        }
        validate_mission_schema_value(value, target, schema_root, path, depth + 1, report);
    }

    if let Some(definitions) = schema_object.get("$defs") {
        let Some(definitions) = definitions.as_object() else {
            push_mission_schema_issue(
                report,
                path,
                "invalid_schema_definitions",
                "authoritative $defs must be an object of schemas",
            );
            return;
        };
        for (name, definition) in definitions {
            if !definition.is_object() && !definition.is_boolean() {
                push_mission_schema_issue(
                    report,
                    path,
                    "invalid_schema_definition",
                    format!(
                        "authoritative $defs entry {name:?} must be a schema object or boolean"
                    ),
                );
            }
        }
    }

    if let Some(types) = schema_object.get("type") {
        let matches = types
            .as_str()
            .map(|schema_type| mission_schema_type_matches(value, schema_type))
            .or_else(|| {
                types.as_array().map(|types| {
                    types
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|schema_type| mission_schema_type_matches(value, schema_type))
                })
            })
            .unwrap_or(false);
        if !matches {
            push_mission_schema_issue(
                report,
                path,
                "type_mismatch",
                format!("expected {}, received {}", types, mission_json_type(value)),
            );
            return;
        }
    }

    if let Some(enum_values) = schema_object.get("enum").and_then(Value::as_array) {
        if !enum_values.iter().any(|candidate| candidate == value) {
            push_mission_schema_issue(
                report,
                path,
                "enum_mismatch",
                "value is not in the allowed enum",
            );
        }
    }
    if let Some(constant) = schema_object.get("const") {
        if constant != value {
            push_mission_schema_issue(
                report,
                path,
                "const_mismatch",
                "value does not equal the required constant",
            );
        }
    }

    if let Some(pattern) = schema_object.get("pattern") {
        if let Some(pattern) = pattern.as_str() {
            if let Some(string) = value.as_str() {
                match mission_pattern_matches(string, pattern) {
                    Ok(true) => {}
                    Ok(false) => push_mission_schema_issue(
                        report,
                        path,
                        "pattern_mismatch",
                        "string does not match the required pattern",
                    ),
                    Err(_) => push_mission_schema_issue(
                        report,
                        path,
                        "unsupported_pattern",
                        "authoritative pattern uses unsupported or unsafe syntax",
                    ),
                }
            }
        } else {
            push_mission_schema_issue(
                report,
                path,
                "invalid_pattern_schema",
                "authoritative pattern must be a string",
            );
        }
    }

    if let Some(all_of) = schema_object.get("allOf").and_then(Value::as_array) {
        for branch in all_of {
            validate_mission_schema_value(value, branch, schema_root, path, depth + 1, report);
        }
    }
    if let Some(any_of) = schema_object.get("anyOf").and_then(Value::as_array) {
        if !any_of
            .iter()
            .any(|branch| mission_schema_matches(value, branch, schema_root, path, depth + 1))
        {
            push_mission_schema_issue(
                report,
                path,
                "any_of_mismatch",
                "value matches none of the allowed schemas",
            );
        }
    }
    if let Some(one_of) = schema_object.get("oneOf").and_then(Value::as_array) {
        let matches = one_of
            .iter()
            .filter(|branch| mission_schema_matches(value, branch, schema_root, path, depth + 1))
            .count();
        if matches != 1 {
            push_mission_schema_issue(
                report,
                path,
                "one_of_mismatch",
                format!("value matches {matches} schemas; exactly one is required"),
            );
        }
    }
    if let Some(not_schema) = schema_object.get("not") {
        if mission_schema_matches(value, not_schema, schema_root, path, depth + 1) {
            push_mission_schema_issue(
                report,
                path,
                "not_mismatch",
                "value matches a forbidden schema",
            );
        }
    }

    if let Some(object) = value.as_object() {
        if let Some(required) = schema_object.get("required").and_then(Value::as_array) {
            for required_name in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(required_name) {
                    push_mission_schema_issue(
                        report,
                        path,
                        "required_missing",
                        format!("required property `{required_name}` is missing"),
                    );
                }
            }
        }
        let properties = schema_object.get("properties").and_then(Value::as_object);
        if let Some(properties) = properties {
            for (name, child_schema) in properties {
                if let Some(child) = object.get(name) {
                    let child_path = mission_schema_path(path, name);
                    validate_mission_schema_value(
                        child,
                        child_schema,
                        schema_root,
                        &child_path,
                        depth + 1,
                        report,
                    );
                }
            }
        }
        if let Some(property_names_schema) = schema_object.get("propertyNames") {
            for name in object.keys() {
                let child_path = mission_schema_path(path, name);
                validate_mission_schema_value(
                    &Value::String(name.clone()),
                    property_names_schema,
                    schema_root,
                    &child_path,
                    depth + 1,
                    report,
                );
            }
        }
        if let Some(additional_schema) = schema_object.get("additionalProperties") {
            for (name, child) in object.iter().filter(|(name, _)| {
                !properties.is_some_and(|properties| properties.contains_key(*name))
            }) {
                let child_path = mission_schema_path(path, name);
                match additional_schema {
                    Value::Bool(false) => push_mission_schema_issue(
                        report,
                        &child_path,
                        "additional_property",
                        "additional properties are not allowed",
                    ),
                    Value::Bool(true) => {}
                    Value::Object(_) => validate_mission_schema_value(
                        child,
                        additional_schema,
                        schema_root,
                        &child_path,
                        depth + 1,
                        report,
                    ),
                    _ => push_mission_schema_issue(
                        report,
                        path,
                        "invalid_additional_properties_schema",
                        "authoritative additionalProperties must be a boolean or schema object",
                    ),
                }
            }
        }
        if let Some(minimum) = schema_object.get("minProperties").and_then(Value::as_u64) {
            if object.len() < minimum as usize {
                push_mission_schema_issue(
                    report,
                    path,
                    "min_properties",
                    format!("object has fewer than {minimum} properties"),
                );
            }
        }
        if let Some(maximum) = schema_object.get("maxProperties").and_then(Value::as_u64) {
            if object.len() > maximum as usize {
                push_mission_schema_issue(
                    report,
                    path,
                    "max_properties",
                    format!("object has more than {maximum} properties"),
                );
            }
        }
    }

    if let Some(array) = value.as_array() {
        if let Some(minimum) = schema_object.get("minItems").and_then(Value::as_u64) {
            if array.len() < minimum as usize {
                push_mission_schema_issue(
                    report,
                    path,
                    "min_items",
                    format!("array has fewer than {minimum} items"),
                );
            }
        }
        if let Some(maximum) = schema_object.get("maxItems").and_then(Value::as_u64) {
            if array.len() > maximum as usize {
                push_mission_schema_issue(
                    report,
                    path,
                    "max_items",
                    format!("array has more than {maximum} items"),
                );
            }
        }
        if schema_object.get("uniqueItems").and_then(Value::as_bool) == Some(true) {
            for (index, item) in array.iter().enumerate() {
                if array[..index].iter().any(|candidate| candidate == item) {
                    push_mission_schema_issue(
                        report,
                        path,
                        "unique_items",
                        "array items must be unique",
                    );
                    break;
                }
            }
        }
        if let Some(item_schema) = schema_object.get("items") {
            for (index, item) in array.iter().enumerate() {
                let item_path = mission_schema_path(path, &index.to_string());
                validate_mission_schema_value(
                    item,
                    item_schema,
                    schema_root,
                    &item_path,
                    depth + 1,
                    report,
                );
            }
        }
    }

    if let Some(string) = value.as_str() {
        let length = string.chars().count();
        if let Some(minimum) = schema_object.get("minLength").and_then(Value::as_u64) {
            if length < minimum as usize {
                push_mission_schema_issue(
                    report,
                    path,
                    "min_length",
                    format!("string is shorter than {minimum} characters"),
                );
            }
        }
        if let Some(maximum) = schema_object.get("maxLength").and_then(Value::as_u64) {
            if length > maximum as usize {
                push_mission_schema_issue(
                    report,
                    path,
                    "max_length",
                    format!("string is longer than {maximum} characters"),
                );
            }
        }
    }

    if let Some(number) = value.as_f64() {
        if let Some(minimum) = schema_object.get("minimum").and_then(Value::as_f64) {
            if number < minimum {
                push_mission_schema_issue(
                    report,
                    path,
                    "minimum",
                    format!("number is below {minimum}"),
                );
            }
        }
        if let Some(maximum) = schema_object.get("maximum").and_then(Value::as_f64) {
            if number > maximum {
                push_mission_schema_issue(
                    report,
                    path,
                    "maximum",
                    format!("number is above {maximum}"),
                );
            }
        }
        if let Some(minimum) = schema_object
            .get("exclusiveMinimum")
            .and_then(Value::as_f64)
        {
            if number <= minimum {
                push_mission_schema_issue(
                    report,
                    path,
                    "exclusive_minimum",
                    format!("number must be greater than {minimum}"),
                );
            }
        }
        if let Some(maximum) = schema_object
            .get("exclusiveMaximum")
            .and_then(Value::as_f64)
        {
            if number >= maximum {
                push_mission_schema_issue(
                    report,
                    path,
                    "exclusive_maximum",
                    format!("number must be less than {maximum}"),
                );
            }
        }
    }
}

fn mission_json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn validate_static_mission_schemas(request: &MissionRequest) -> Result<(), String> {
    for step in &request.steps {
        if !step.bindings.is_empty() {
            continue;
        }
        if let Some(report) = validate_mission_tool_arguments(&step.tool, &step.arguments)? {
            if let Some(detail) = schema_failure_detail(&step.tool, &report) {
                return Err(detail);
            }
        }
    }
    Ok(())
}

impl Server {
    /// Clone the server with an in-process observer for live mission trace projection.
    ///
    /// The observer is used only by the bounded HTTP job surface. It cannot alter dispatch,
    /// reports, or hashes; it receives a copy of each trace event after it is appended.
    pub fn with_mission_trace_observer(&self, observer: Arc<dyn Fn(Value) + Send + Sync>) -> Self {
        let mut server = self.clone();
        server.mission_trace_observer = Some(MissionTraceObserver(observer));
        server
    }
    /// Validate an agent mission without dispatching any nested tool.
    ///
    /// The HTTP job surface uses this before accepting work so malformed missions fail at
    /// submission time rather than becoming opaque background failures.
    pub fn validate_agent_mission(&self, arguments: &Value) -> Result<(), String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode agent mission input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("agent mission input exceeds the 20000000-byte safety bound".into());
        }
        let request: MissionRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid agent mission input: {error}"))?;
        plan_mission(&request).map_err(|error| format!("agent mission refused: {error}"))?;
        validate_static_mission_schemas(&request)
            .map_err(|error| format!("agent mission refused: {error}"))?;
        Ok(())
    }

    /// Produce the authoritative mission plan without dispatching any nested tool.
    ///
    /// This is the transport-facing preflight boundary. It preserves the caller's graph,
    /// allow-list, budgets, and schema-visible arguments, but forcibly changes the execution
    /// choice to preview so a request that was authored for execution cannot accidentally run
    /// through a preflight endpoint.
    pub fn preflight_agent_mission(&self, arguments: &Value) -> Result<Value, String> {
        self.validate_agent_mission(arguments)?;
        let mut value = arguments.clone();
        let object = value
            .as_object_mut()
            .ok_or_else(|| "agent mission input must be a JSON object".to_string())?;
        let policy = object.entry("policy").or_insert_with(|| json!({}));
        let policy = policy
            .as_object_mut()
            .ok_or_else(|| "agent mission policy must be a JSON object".to_string())?;
        policy.insert("execute".into(), json!(false));
        let mut report = self.agent_mission_with_cancellation(&value, None)?;
        report["preflight"] = json!(true);
        report["dispatch"] = json!("not_started");
        Ok(report)
    }

    /// Execute an agent mission while observing a shared cooperative cancellation flag.
    ///
    /// Cancellation is checked between nested tool calls and parallel batches. An in-flight tool
    /// call is never forcefully interrupted, so the returned report can distinguish completed
    /// work from steps that were prevented from being dispatched.
    pub fn execute_agent_mission_with_cancellation(
        &self,
        arguments: &Value,
        cancellation: &AtomicBool,
    ) -> Result<Value, String> {
        let result = self.agent_mission_with_cancellation(arguments, Some(cancellation));
        self.attach_workflow_reconciliation(arguments, result)
            .and_then(|report| self.index_mission_report(report))
    }
    pub(super) fn agent_mission(&self, arguments: &Value) -> Result<Value, String> {
        let result = self.agent_mission_with_cancellation(arguments, None);
        self.attach_workflow_reconciliation(arguments, result)
            .and_then(|report| self.index_mission_report(report))
    }

    /// Run the mission executor on a dedicated thread with an explicit stack reservation.
    ///
    /// `call_tool` already reserves a stack for the dispatch that reaches it, but the executor's
    /// own activation record is large enough that it must not share a reservation with the
    /// dispatch frame that entered it, and the direct public entry points
    /// (`preflight_agent_mission`, `execute_agent_mission_with_cancellation`) arrive on caller
    /// threads with unknown headroom. A panic inside the executor is resumed on the caller so
    /// panic behaviour is unchanged; only a failure to start the thread becomes a new, explicit
    /// refusal.
    fn agent_mission_with_cancellation(
        &self,
        arguments: &Value,
        cancellation: Option<&AtomicBool>,
    ) -> Result<Value, String> {
        std::thread::scope(|scope| {
            match std::thread::Builder::new()
                .name("bioprism-mcp-mission".into())
                .stack_size(TOOL_DISPATCH_STACK_BYTES)
                .spawn_scoped(scope, || {
                    self.agent_mission_with_cancellation_inner(arguments, cancellation)
                }) {
                Ok(handle) => handle
                    .join()
                    .unwrap_or_else(|payload| std::panic::resume_unwind(payload)),
                Err(error) => Err(format!("cannot start the mission executor thread: {error}")),
            }
        })
    }

    fn agent_mission_with_cancellation_inner(
        &self,
        arguments: &Value,
        cancellation: Option<&AtomicBool>,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode agent mission input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("agent mission input exceeds the 20000000-byte safety bound".into());
        }
        let request: MissionRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid agent mission input: {error}"))?;
        let plan =
            plan_mission(&request).map_err(|error| format!("agent mission refused: {error}"))?;
        validate_static_mission_schemas(&request)
            .map_err(|error| format!("agent mission refused: {error}"))?;
        let mut report = MissionReport {
            schema_version: MISSION_SCHEMA_VERSION.into(),
            plan,
            execution: if request.policy.execute {
                "executed".into()
            } else {
                "planned".into()
            },
            mission_status: if request.policy.execute {
                "running".into()
            } else {
                "planned".into()
            },
            succeeded: 0,
            refused: 0,
            blocked: 0,
            cancelled: 0,
            required_failures: 0,
            returned_bytes: 0,
            results: Vec::new(),
            execution_trace_schema_version: MISSION_TRACE_SCHEMA_VERSION.into(),
            execution_trace: Vec::new(),
            claim_requests: request.claim_requests.clone(),
            evaluator_review: request.evaluator_review.clone(),
            claim_lineage: json!({}),
            trace_observer: self.mission_trace_observer.clone(),
            guarantees: vec![
            "the mission DAG was validated before any nested tool call".into(),
            "nested calls preserve raw JSON-RPC result and refusal envelopes".into(),
            "dependent work is blocked when its prerequisite refuses or exceeds budget".into(),
            "validated JSON-pointer bindings can pass structured upstream results into dependent arguments".into(),
            "each nested call records a content digest of its post-binding input arguments".into(),
            "known tool arguments are checked against the authoritative tools/list schema before dispatch, including after bindings are materialized".into(),
        ],
            limitations: vec![
                if request.policy.execution_mode == "parallel_waves" {
                    format!(
                        "independent steps in each wave are dispatched in bounded batches of at most {} after a worst-case output reservation",
                        request.policy.max_parallelism
                    )
                } else {
                    "the MCP adapter executes the deterministic plan serially, even when waves are parallelizable".into()
                },
                "tool arguments and scientific interpretation remain caller-owned".into(),
                "this is an in-process bounded executor, not a distributed scheduler, durable queue, or external worker pool".into(),
            ],
        };
        let initial_status = report.mission_status.clone();
        trace_event(
            &mut report,
            "mission.started",
            None,
            None,
            Some(initial_status.as_str()),
            None,
            0,
            Some(format!(
                "execution_mode={}, max_parallelism={}",
                request.policy.execution_mode, request.policy.max_parallelism
            )),
        );

        if !request.policy.execute {
            trace_event(
                &mut report,
                "mission.completed",
                None,
                None,
                Some("planned"),
                None,
                0,
                Some("planning did not dispatch any nested tool".into()),
            );
            return encode_mission_report(report, "cannot encode agent mission plan");
        }

        if request.policy.execution_mode == "parallel_waves" {
            return self.execute_parallel_mission(&request, report, cancellation);
        }

        let steps = request
            .steps
            .iter()
            .map(|step| (step.id.as_str(), step))
            .collect::<BTreeMap<_, _>>();
        let mut failed = BTreeSet::new();
        let mut blocked = BTreeSet::new();
        let mut payloads: BTreeMap<String, Value> = BTreeMap::new();
        let mut abort = false;

        let waves = report.plan.waves.clone();
        for (wave_index, wave) in waves.iter().enumerate() {
            if cancellation_requested(cancellation) {
                cancel_remaining_steps(&mut report, &request, wave_index);
                return finish_cancelled_mission(
                    report,
                    "cannot encode cancelled agent mission report",
                );
            }
            trace_event(
                &mut report,
                "wave.started",
                Some(wave_index),
                None,
                Some("running"),
                None,
                0,
                None,
            );
            for step_id in wave {
                if cancellation_requested(cancellation) {
                    cancel_remaining_steps(&mut report, &request, wave_index);
                    return finish_cancelled_mission(
                        report,
                        "cannot encode cancelled agent mission report",
                    );
                }
                let step = steps
                    .get(step_id.as_str())
                    .ok_or_else(|| format!("mission plan lost step `{step_id}`"))?;
                if abort
                    || step.depends_on.iter().any(|dependency| {
                        failed.contains(dependency) || blocked.contains(dependency)
                    })
                {
                    blocked.insert(step.id.clone());
                    report.blocked += 1;
                    if step.required {
                        report.required_failures += 1;
                        abort = true;
                    }
                    let detail = "a prerequisite mission step refused or was blocked".to_string();
                    report.results.push(MissionStepResult {
                        id: step.id.clone(),
                        tool: step.tool.clone(),
                        status: "blocked".into(),
                        required: step.required,
                        arguments_digest: None,
                        bytes: 0,
                        wire: None,
                        error: Some(detail.clone()),
                    });
                    trace_event(
                        &mut report,
                        "step.blocked",
                        Some(wave_index),
                        Some(step),
                        Some("blocked"),
                        None,
                        0,
                        Some(detail),
                    );
                    continue;
                }

                let mut effective_arguments = step.arguments.clone();
                let binding_result = step.bindings.iter().try_for_each(|binding| {
                    let payload = payloads.get(&binding.from_step).ok_or_else(|| {
                        format!(
                            "binding source step `{}` has no successful structured payload",
                            binding.from_step
                        )
                    })?;
                    apply_binding(&mut effective_arguments, binding, payload)
                        .map_err(|error| error.to_string())
                });
                if let Err(error) = binding_result {
                    failed.insert(step.id.clone());
                    report.refused += 1;
                    if step.required {
                        report.required_failures += 1;
                    }
                    let detail = format!("mission binding refused: {error}");
                    report.results.push(MissionStepResult {
                        id: step.id.clone(),
                        tool: step.tool.clone(),
                        status: "refused".into(),
                        required: step.required,
                        arguments_digest: None,
                        bytes: 0,
                        wire: None,
                        error: Some(detail.clone()),
                    });
                    trace_event(
                        &mut report,
                        "step.refused",
                        Some(wave_index),
                        Some(step),
                        Some("refused"),
                        None,
                        0,
                        Some(detail),
                    );
                    abort |= step.required || request.policy.stop_on_error;
                    continue;
                }
                let arguments_digest = bioprism_ids::ContentHash::of_value(&effective_arguments)
                    .map_err(|error| {
                        format!("cannot hash mission step `{}` arguments: {error}", step.id)
                    })?
                    .to_string();

                if !step.bindings.is_empty() {
                    if let Some(schema_report) =
                        validate_mission_tool_arguments(&step.tool, &effective_arguments)?
                    {
                        if let Some(detail) = schema_failure_detail(&step.tool, &schema_report) {
                            failed.insert(step.id.clone());
                            report.refused += 1;
                            if step.required {
                                report.required_failures += 1;
                            }
                            report.results.push(MissionStepResult {
                                id: step.id.clone(),
                                tool: step.tool.clone(),
                                status: "refused".into(),
                                required: step.required,
                                arguments_digest: Some(arguments_digest.clone()),
                                bytes: 0,
                                wire: None,
                                error: Some(detail.clone()),
                            });
                            trace_event(
                                &mut report,
                                "step.refused",
                                Some(wave_index),
                                Some(step),
                                Some("refused"),
                                Some(arguments_digest),
                                0,
                                Some(detail),
                            );
                            abort |= step.required || request.policy.stop_on_error;
                            continue;
                        }
                    }
                }

                trace_event(
                    &mut report,
                    "step.started",
                    Some(wave_index),
                    Some(step),
                    Some("running"),
                    Some(arguments_digest.clone()),
                    0,
                    None,
                );

                let nested = Request {
                    id: Some(json!(step.id)),
                    method: "tools/call".into(),
                    params: json!({
                        "name": step.tool,
                        "arguments": effective_arguments,
                    }),
                };
                let wire = self.call_tool(&nested).to_json();
                let bytes = serde_json::to_vec(&wire)
                    .map_err(|error| format!("cannot measure mission step `{}`: {error}", step.id))?
                    .len();
                if bytes > request.policy.max_step_output_bytes {
                    failed.insert(step.id.clone());
                    report.refused += 1;
                    if step.required {
                        report.required_failures += 1;
                    }
                    let detail = format!(
                        "nested result is {bytes} bytes, above the per-step output budget of {}",
                        request.policy.max_step_output_bytes
                    );
                    report.results.push(MissionStepResult {
                        id: step.id.clone(),
                        tool: step.tool.clone(),
                        status: "refused".into(),
                        required: step.required,
                        arguments_digest: Some(arguments_digest.clone()),
                        bytes,
                        wire: None,
                        error: Some(detail.clone()),
                    });
                    trace_event(
                        &mut report,
                        "step.refused",
                        Some(wave_index),
                        Some(step),
                        Some("refused"),
                        Some(arguments_digest.clone()),
                        bytes,
                        Some(detail),
                    );
                    abort |= step.required || request.policy.stop_on_error;
                    continue;
                }
                if report.returned_bytes.saturating_add(bytes)
                    > request.policy.max_total_output_bytes
                {
                    failed.insert(step.id.clone());
                    report.refused += 1;
                    if step.required {
                        report.required_failures += 1;
                    }
                    let detail = format!(
                        "mission output budget of {} bytes would be exceeded",
                        request.policy.max_total_output_bytes
                    );
                    report.results.push(MissionStepResult {
                        id: step.id.clone(),
                        tool: step.tool.clone(),
                        status: "refused".into(),
                        required: step.required,
                        arguments_digest: Some(arguments_digest.clone()),
                        bytes,
                        wire: None,
                        error: Some(detail.clone()),
                    });
                    trace_event(
                        &mut report,
                        "step.refused",
                        Some(wave_index),
                        Some(step),
                        Some("refused"),
                        Some(arguments_digest.clone()),
                        bytes,
                        Some(detail),
                    );
                    abort |= step.required || request.policy.stop_on_error;
                    continue;
                }

                let is_error = wire
                    .pointer("/result/isError")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    || wire.get("error").is_some();
                let error = wire
                    .pointer("/result/content/0/text")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                report.returned_bytes += bytes;
                if is_error {
                    failed.insert(step.id.clone());
                    report.refused += 1;
                    if step.required {
                        report.required_failures += 1;
                    }
                    trace_event(
                        &mut report,
                        "step.refused",
                        Some(wave_index),
                        Some(step),
                        Some("refused"),
                        Some(arguments_digest.clone()),
                        bytes,
                        error.clone(),
                    );
                    report.results.push(MissionStepResult {
                        id: step.id.clone(),
                        tool: step.tool.clone(),
                        status: "refused".into(),
                        required: step.required,
                        arguments_digest: Some(arguments_digest.clone()),
                        bytes,
                        wire: Some(wire),
                        error,
                    });
                    abort |= step.required || request.policy.stop_on_error;
                } else {
                    let payload = wire
                        .pointer("/result/content/0/text")
                        .and_then(Value::as_str)
                        .and_then(|text| serde_json::from_str::<Value>(text).ok())
                        .unwrap_or_else(|| wire.clone());
                    payloads.insert(step.id.clone(), payload);
                    report.succeeded += 1;
                    trace_event(
                        &mut report,
                        "step.completed",
                        Some(wave_index),
                        Some(step),
                        Some("succeeded"),
                        Some(arguments_digest.clone()),
                        bytes,
                        None,
                    );
                    report.results.push(MissionStepResult {
                        id: step.id.clone(),
                        tool: step.tool.clone(),
                        status: "succeeded".into(),
                        required: step.required,
                        arguments_digest: Some(arguments_digest),
                        bytes,
                        wire: Some(wire),
                        error: None,
                    });
                }
            }
            trace_event(
                &mut report,
                "wave.completed",
                Some(wave_index),
                None,
                Some("completed"),
                None,
                0,
                None,
            );
        }

        report.mission_status = if report.required_failures > 0 {
            "failed".into()
        } else if report.refused > 0 || report.blocked > 0 {
            "partial".into()
        } else {
            "succeeded".into()
        };

        let mission_status = report.mission_status.clone();
        let returned_bytes = report.returned_bytes;
        trace_event(
            &mut report,
            "mission.completed",
            None,
            None,
            Some(mission_status.as_str()),
            None,
            returned_bytes,
            None,
        );

        encode_mission_report(report, "cannot encode agent mission report")
    }

    fn execute_parallel_mission(
        &self,
        request: &MissionRequest,
        mut report: MissionReport,
        cancellation: Option<&AtomicBool>,
    ) -> Result<Value, String> {
        let steps = request
            .steps
            .iter()
            .map(|step| (step.id.as_str(), step))
            .collect::<BTreeMap<_, _>>();
        let mut failed = BTreeSet::new();
        let mut blocked = BTreeSet::new();
        let mut payloads: BTreeMap<String, Value> = BTreeMap::new();
        let mut abort = false;

        let waves = report.plan.waves.clone();
        for (wave_index, wave) in waves.iter().enumerate() {
            if cancellation_requested(cancellation) {
                cancel_remaining_steps(&mut report, request, wave_index);
                return finish_cancelled_mission(
                    report,
                    "cannot encode cancelled parallel agent mission report",
                );
            }
            trace_event(
                &mut report,
                "wave.started",
                Some(wave_index),
                None,
                Some("running"),
                None,
                0,
                None,
            );
            let mut pending = Vec::new();
            let mut wave_abort = abort;
            for step_id in wave {
                let step = steps
                    .get(step_id.as_str())
                    .ok_or_else(|| format!("mission plan lost step `{step_id}`"))?;
                if abort
                    || step.depends_on.iter().any(|dependency| {
                        failed.contains(dependency) || blocked.contains(dependency)
                    })
                {
                    wave_abort = true;
                    pending.push(ParallelPending::Blocked {
                        step,
                        error: "a prerequisite mission step refused or was blocked".into(),
                    });
                    continue;
                }

                let mut effective_arguments = step.arguments.clone();
                let binding_result = step.bindings.iter().try_for_each(|binding| {
                    let payload = payloads.get(&binding.from_step).ok_or_else(|| {
                        format!(
                            "binding source step `{}` has no successful structured payload",
                            binding.from_step
                        )
                    })?;
                    apply_binding(&mut effective_arguments, binding, payload)
                        .map_err(|error| error.to_string())
                });
                if let Err(error) = binding_result {
                    if step.required || request.policy.stop_on_error {
                        wave_abort = true;
                    }
                    pending.push(ParallelPending::Refused {
                        step,
                        arguments_digest: None,
                        bytes: 0,
                        error: format!("mission binding refused: {error}"),
                    });
                    continue;
                }
                let arguments_digest = bioprism_ids::ContentHash::of_value(&effective_arguments)
                    .map_err(|error| {
                        format!("cannot hash mission step `{}` arguments: {error}", step.id)
                    })?
                    .to_string();
                if !step.bindings.is_empty() {
                    if let Some(schema_report) =
                        validate_mission_tool_arguments(&step.tool, &effective_arguments)?
                    {
                        if let Some(detail) = schema_failure_detail(&step.tool, &schema_report) {
                            if step.required || request.policy.stop_on_error {
                                wave_abort = true;
                            }
                            pending.push(ParallelPending::Refused {
                                step,
                                arguments_digest: Some(arguments_digest),
                                bytes: 0,
                                error: detail,
                            });
                            continue;
                        }
                    }
                }
                pending.push(ParallelPending::Call {
                    step,
                    arguments: effective_arguments,
                    arguments_digest,
                });
            }

            if wave_abort {
                for item in &mut pending {
                    if let ParallelPending::Call { step, .. } = item {
                        let step = *step;
                        *item = ParallelPending::Blocked {
                            step,
                            error: "parallel wave was not launched after a prerequisite or preparation refusal".into(),
                        };
                    }
                }
            } else {
                let call_count = pending
                    .iter()
                    .filter(|item| matches!(item, ParallelPending::Call { .. }))
                    .count();
                let reserved = request
                    .policy
                    .max_step_output_bytes
                    .saturating_mul(call_count);
                if report.returned_bytes.saturating_add(reserved)
                    > request.policy.max_total_output_bytes
                {
                    for item in &mut pending {
                        if let ParallelPending::Call {
                            step,
                            arguments_digest,
                            ..
                        } = item
                        {
                            let step = *step;
                            *item = ParallelPending::Refused {
                                step,
                                arguments_digest: Some(arguments_digest.clone()),
                                bytes: 0,
                                error: format!(
                                    "parallel wave worst-case output reservation of {reserved} bytes would exceed the remaining mission budget"
                                ),
                            };
                        }
                    }
                }
            }

            let call_entries = pending
                .iter()
                .filter_map(|item| match item {
                    ParallelPending::Call {
                        step, arguments, ..
                    } => Some((*step, arguments.clone())),
                    ParallelPending::Blocked { .. } | ParallelPending::Refused { .. } => None,
                })
                .collect::<Vec<_>>();
            let mut call_results = BTreeMap::new();
            for batch in call_entries.chunks(request.policy.max_parallelism) {
                if cancellation_requested(cancellation) {
                    cancel_remaining_steps(&mut report, request, wave_index);
                    return finish_cancelled_mission(
                        report,
                        "cannot encode cancelled parallel agent mission report",
                    );
                }
                for (step, _) in batch {
                    let arguments_digest = pending
                        .iter()
                        .find_map(|item| match item {
                            ParallelPending::Call {
                                step: pending_step,
                                arguments_digest,
                                ..
                            } if pending_step.id == step.id => Some(arguments_digest.clone()),
                            _ => None,
                        })
                        .ok_or_else(|| {
                            format!("parallel mission step `{}` has no argument digest", step.id)
                        })?;
                    trace_event(
                        &mut report,
                        "step.started",
                        Some(wave_index),
                        Some(step),
                        Some("running"),
                        Some(arguments_digest),
                        0,
                        None,
                    );
                }
                let batch_results = std::thread::scope(|scope| {
                    let handles = batch
                        .iter()
                        .map(|(step, arguments)| {
                            let step = *step;
                            let arguments = arguments.clone();
                            (
                                step.id.clone(),
                                scope.spawn(move || self.execute_parallel_call(step, arguments)),
                            )
                        })
                        .collect::<Vec<_>>();
                    let mut results = BTreeMap::new();
                    for (id, handle) in handles {
                        let result = handle
                            .join()
                            .map_err(|_| format!("parallel mission step `{id}` panicked"))??;
                        results.insert(id, result);
                    }
                    Ok::<_, String>(results)
                })?;
                call_results.extend(batch_results);
            }

            for item in pending {
                match item {
                    ParallelPending::Blocked { step, error } => {
                        blocked.insert(step.id.clone());
                        report.blocked += 1;
                        if step.required {
                            report.required_failures += 1;
                            abort = true;
                        }
                        let detail = error.clone();
                        report.results.push(MissionStepResult {
                            id: step.id.clone(),
                            tool: step.tool.clone(),
                            status: "blocked".into(),
                            required: step.required,
                            arguments_digest: None,
                            bytes: 0,
                            wire: None,
                            error: Some(error),
                        });
                        trace_event(
                            &mut report,
                            "step.blocked",
                            Some(wave_index),
                            Some(step),
                            Some("blocked"),
                            None,
                            0,
                            Some(detail),
                        );
                    }
                    ParallelPending::Refused {
                        step,
                        arguments_digest,
                        bytes,
                        error,
                    } => {
                        failed.insert(step.id.clone());
                        report.refused += 1;
                        if step.required {
                            report.required_failures += 1;
                        }
                        let detail = error.clone();
                        let trace_digest = arguments_digest.clone();
                        report.results.push(MissionStepResult {
                            id: step.id.clone(),
                            tool: step.tool.clone(),
                            status: "refused".into(),
                            required: step.required,
                            arguments_digest,
                            bytes,
                            wire: None,
                            error: Some(error),
                        });
                        trace_event(
                            &mut report,
                            "step.refused",
                            Some(wave_index),
                            Some(step),
                            Some("refused"),
                            trace_digest,
                            bytes,
                            Some(detail),
                        );
                        abort |= step.required || request.policy.stop_on_error;
                    }
                    ParallelPending::Call {
                        step,
                        arguments_digest,
                        ..
                    } => {
                        let outcome = call_results.remove(&step.id).ok_or_else(|| {
                            format!("parallel mission step `{}` has no result", step.id)
                        })?;
                        if outcome.bytes > request.policy.max_step_output_bytes {
                            failed.insert(step.id.clone());
                            report.refused += 1;
                            if step.required {
                                report.required_failures += 1;
                            }
                            let bytes = outcome.bytes;
                            let detail = format!(
                                "nested result is {bytes} bytes, above the per-step output budget of {}",
                                request.policy.max_step_output_bytes
                            );
                            trace_event(
                                &mut report,
                                "step.refused",
                                Some(wave_index),
                                Some(step),
                                Some("refused"),
                                Some(arguments_digest.clone()),
                                bytes,
                                Some(detail.clone()),
                            );
                            report.results.push(MissionStepResult {
                                id: step.id.clone(),
                                tool: step.tool.clone(),
                                status: "refused".into(),
                                required: step.required,
                                arguments_digest: Some(arguments_digest),
                                bytes,
                                wire: None,
                                error: Some(detail),
                            });
                            abort |= step.required || request.policy.stop_on_error;
                            continue;
                        }
                        if report.returned_bytes.saturating_add(outcome.bytes)
                            > request.policy.max_total_output_bytes
                        {
                            failed.insert(step.id.clone());
                            report.refused += 1;
                            if step.required {
                                report.required_failures += 1;
                            }
                            let bytes = outcome.bytes;
                            let detail = format!(
                                "mission output budget of {} bytes would be exceeded",
                                request.policy.max_total_output_bytes
                            );
                            trace_event(
                                &mut report,
                                "step.refused",
                                Some(wave_index),
                                Some(step),
                                Some("refused"),
                                Some(arguments_digest.clone()),
                                bytes,
                                Some(detail.clone()),
                            );
                            report.results.push(MissionStepResult {
                                id: step.id.clone(),
                                tool: step.tool.clone(),
                                status: "refused".into(),
                                required: step.required,
                                arguments_digest: Some(arguments_digest),
                                bytes,
                                wire: None,
                                error: Some(detail),
                            });
                            abort |= step.required || request.policy.stop_on_error;
                            continue;
                        }

                        report.returned_bytes += outcome.bytes;
                        if outcome.is_error {
                            failed.insert(step.id.clone());
                            report.refused += 1;
                            if step.required {
                                report.required_failures += 1;
                            }
                            let detail = outcome.error.clone();
                            trace_event(
                                &mut report,
                                "step.refused",
                                Some(wave_index),
                                Some(step),
                                Some("refused"),
                                Some(arguments_digest.clone()),
                                outcome.bytes,
                                detail,
                            );
                            report.results.push(MissionStepResult {
                                id: step.id.clone(),
                                tool: step.tool.clone(),
                                status: "refused".into(),
                                required: step.required,
                                arguments_digest: Some(arguments_digest),
                                bytes: outcome.bytes,
                                wire: Some(outcome.wire),
                                error: outcome.error,
                            });
                            abort |= step.required || request.policy.stop_on_error;
                        } else {
                            let payload = outcome
                                .wire
                                .pointer("/result/content/0/text")
                                .and_then(Value::as_str)
                                .and_then(|text| serde_json::from_str::<Value>(text).ok())
                                .unwrap_or_else(|| outcome.wire.clone());
                            payloads.insert(step.id.clone(), payload);
                            report.succeeded += 1;
                            trace_event(
                                &mut report,
                                "step.completed",
                                Some(wave_index),
                                Some(step),
                                Some("succeeded"),
                                Some(arguments_digest.clone()),
                                outcome.bytes,
                                None,
                            );
                            report.results.push(MissionStepResult {
                                id: step.id.clone(),
                                tool: step.tool.clone(),
                                status: "succeeded".into(),
                                required: step.required,
                                arguments_digest: Some(arguments_digest),
                                bytes: outcome.bytes,
                                wire: Some(outcome.wire),
                                error: None,
                            });
                        }
                    }
                }
            }
            trace_event(
                &mut report,
                "wave.completed",
                Some(wave_index),
                None,
                Some("completed"),
                None,
                0,
                None,
            );
        }

        report.mission_status = if report.required_failures > 0 {
            "failed".into()
        } else if report.refused > 0 || report.blocked > 0 {
            "partial".into()
        } else {
            "succeeded".into()
        };
        let mission_status = report.mission_status.clone();
        let returned_bytes = report.returned_bytes;
        trace_event(
            &mut report,
            "mission.completed",
            None,
            None,
            Some(mission_status.as_str()),
            None,
            returned_bytes,
            None,
        );
        encode_mission_report(report, "cannot encode parallel agent mission report")
    }

    fn execute_parallel_call(
        &self,
        step: &MissionStep,
        arguments: Value,
    ) -> Result<ParallelCallOutcome, String> {
        let nested = Request {
            id: Some(json!(step.id)),
            method: "tools/call".into(),
            params: json!({
                "name": step.tool,
                "arguments": arguments,
            }),
        };
        let wire = self.call_tool(&nested).to_json();
        let bytes = serde_json::to_vec(&wire)
            .map_err(|error| format!("cannot measure mission step `{}`: {error}", step.id))?
            .len();
        let is_error = wire
            .pointer("/result/isError")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || wire.get("error").is_some();
        let error = wire
            .pointer("/result/content/0/text")
            .and_then(Value::as_str)
            .map(str::to_string);
        Ok(ParallelCallOutcome {
            wire,
            bytes,
            is_error,
            error,
        })
    }
}

#[cfg(test)]
mod schema_tests {
    use super::{mission_pattern_matches, validate_mission_schema_value, MissionSchemaReport};
    use super::{Request, Server};
    use serde_json::{json, Value};
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn validate(value: &Value, schema: &Value) -> MissionSchemaReport {
        let mut report = MissionSchemaReport {
            digest: String::new(),
            issues: Vec::new(),
        };
        validate_mission_schema_value(value, schema, schema, "", 0, &mut report);
        report
    }

    fn initialized_server() -> Server {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root exists");
        let mut server = Server::new(root);
        let initialize =
            Request::parse(r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}"#)
                .expect("initialize parses");
        server.handle(&initialize).expect("initialize is answered");
        let initialized =
            Request::parse(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .expect("initialized notification parses");
        assert!(server.handle(&initialized).is_none());
        server
    }

    fn call_tool(server: &mut Server, name: &str, arguments: Value) -> Value {
        let request = Request::parse(
            &json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": { "name": name, "arguments": arguments },
            })
            .to_string(),
        )
        .expect("tool request parses");
        server
            .handle(&request)
            .expect("tool call is answered")
            .to_json()
    }

    fn collect_schema_contracts(
        schema: &Value,
        path: &str,
        keywords: &mut BTreeSet<String>,
        patterns: &mut Vec<(String, String)>,
    ) {
        let Some(object) = schema.as_object() else {
            return;
        };
        for (keyword, value) in object {
            if !matches!(keyword.as_str(), "description" | "default") {
                keywords.insert(keyword.clone());
            }
            match keyword.as_str() {
                "pattern" => {
                    if let Some(pattern) = value.as_str() {
                        patterns.push((path.to_string(), pattern.to_string()));
                    }
                }
                "properties" => {
                    if let Some(properties) = value.as_object() {
                        for (name, child_schema) in properties {
                            collect_schema_contracts(
                                child_schema,
                                &format!("{path}/properties/{name}"),
                                keywords,
                                patterns,
                            );
                        }
                    }
                }
                "$defs" => {
                    if let Some(definitions) = value.as_object() {
                        for (name, definition) in definitions {
                            collect_schema_contracts(
                                definition,
                                &format!("{path}/$defs/{name}"),
                                keywords,
                                patterns,
                            );
                        }
                    }
                }
                "items" | "additionalProperties" | "propertyNames" | "not" => {
                    collect_schema_contracts(
                        value,
                        &format!("{path}/{keyword}"),
                        keywords,
                        patterns,
                    );
                }
                "allOf" | "anyOf" | "oneOf" => {
                    if let Some(branches) = value.as_array() {
                        for (index, child_schema) in branches.iter().enumerate() {
                            collect_schema_contracts(
                                child_schema,
                                &format!("{path}/{keyword}/{index}"),
                                keywords,
                                patterns,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    #[test]
    fn catalogue_pattern_subset_matches_digest_date_and_tool_names() {
        assert!(mission_pattern_matches(&"a".repeat(64), "^[0-9a-f]{64}$").unwrap());
        assert!(!mission_pattern_matches(&"A".repeat(64), "^[0-9a-f]{64}$").unwrap());
        assert!(mission_pattern_matches(
            "2026-09-25T18:42:10Z",
            "^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$"
        )
        .unwrap());
        assert!(mission_pattern_matches("2045-01-31", "^\\d{4}-\\d{2}-\\d{2}$").unwrap());
        assert!(mission_pattern_matches("workspace_tool_2", "^[A-Za-z0-9_]+$").unwrap());
        assert!(!mission_pattern_matches("invalid-tool", "^[A-Za-z0-9_]+$").unwrap());
    }

    #[test]
    fn unsupported_patterns_fail_closed_instead_of_being_ignored() {
        assert!(mission_pattern_matches("left", "^(left|right)$").is_err());
        let report = validate(
            &json!("left"),
            &json!({"type": "string", "pattern": "^(left|right)$"}),
        );
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "unsupported_pattern"));
    }

    #[test]
    fn local_schema_refs_enforce_targets_and_refuse_unsafe_or_recursive_refs() {
        let schema = json!({
            "type": "object",
            "properties": {"id": {"$ref": "#/$defs/identifier"}},
            "$defs": {"identifier": {"type": "string", "minLength": 3}}
        });
        assert!(validate(&json!({"id": "abc"}), &schema).issues.is_empty());
        assert!(validate(&json!({"id": "ab"}), &schema)
            .issues
            .iter()
            .any(|issue| issue.code == "min_length"));

        let unresolved = validate(&json!("value"), &json!({"$ref": "#/$defs/missing"}));
        assert!(unresolved
            .issues
            .iter()
            .any(|issue| issue.code == "unresolved_schema_ref"));
        let external = validate(
            &json!("value"),
            &json!({"$ref": "https://example.org/schema"}),
        );
        assert!(external
            .issues
            .iter()
            .any(|issue| issue.code == "unsupported_schema_ref"));
        let scalar_target = validate(
            &json!("value"),
            &json!({"$defs": {"not_a_schema": "string"}, "$ref": "#/$defs/not_a_schema"}),
        );
        assert!(scalar_target
            .issues
            .iter()
            .any(|issue| issue.code == "invalid_schema_ref_target"));

        let recursive = json!({
            "$ref": "#/$defs/self",
            "$defs": {"self": {"$ref": "#/$defs/self"}}
        });
        assert!(validate(&json!(null), &recursive)
            .issues
            .iter()
            .any(|issue| issue.code == "schema_depth_exceeded"));
    }

    #[test]
    fn property_names_and_map_values_are_checked_without_fixed_properties() {
        let schema = json!({
            "type": "object",
            "propertyNames": {"type": "string", "minLength": 1},
            "additionalProperties": {"type": "string", "minLength": 1}
        });
        let valid = validate(&json!({"stage-a": "input.json"}), &schema);
        assert!(valid.issues.is_empty(), "{valid:?}");

        let invalid = validate(&json!({"": "", "stage-b": ""}), &schema);
        assert!(
            invalid
                .issues
                .iter()
                .filter(|issue| issue.code == "min_length")
                .count()
                >= 3
        );
        assert!(invalid
            .issues
            .iter()
            .any(|issue| issue.path == "/stage-b" && issue.code == "min_length"));
    }

    #[test]
    fn additional_properties_false_rejects_map_keys_without_fixed_properties() {
        let report = validate(
            &json!({"unexpected": true}),
            &json!({"type": "object", "additionalProperties": false}),
        );
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "additional_property"));
    }

    #[test]
    fn every_embedded_schema_keyword_and_pattern_is_supported() {
        let supported = [
            "$defs",
            "$ref",
            "additionalProperties",
            "allOf",
            "anyOf",
            "const",
            "enum",
            "exclusiveMaximum",
            "exclusiveMinimum",
            "items",
            "maximum",
            "maxItems",
            "maxLength",
            "maxProperties",
            "minimum",
            "minItems",
            "minLength",
            "minProperties",
            "not",
            "oneOf",
            "pattern",
            "properties",
            "propertyNames",
            "required",
            "type",
            "uniqueItems",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
        let mut keywords = BTreeSet::new();
        let mut patterns = Vec::new();
        for definition in crate::tool_definitions() {
            let tool_name = definition["name"].as_str().expect("tool name is present");
            collect_schema_contracts(
                &definition["inputSchema"],
                tool_name,
                &mut keywords,
                &mut patterns,
            );
        }
        let unsupported = keywords.difference(&supported).cloned().collect::<Vec<_>>();
        assert!(
            unsupported.is_empty(),
            "embedded schemas contain validation keywords without enforcement: {unsupported:?}"
        );
        for (path, pattern) in patterns {
            assert!(
                mission_pattern_matches("", &pattern).is_ok(),
                "the catalogue pattern at {path} must have an enforcement implementation"
            );
        }
    }

    #[test]
    fn direct_dispatch_enforces_patterns_and_dynamic_map_constraints() {
        let mut server = initialized_server();
        let bad_digest = call_tool(
            &mut server,
            "brain_job_submit",
            json!({
                "idempotency_key": "submit-1",
                "spec_digest": "not-a-sha256-digest",
                "domain": "software_engineering",
                "capability": "implementation",
                "risk_class": "low"
            }),
        );
        assert_eq!(bad_digest["result"]["isError"], true);
        assert!(bad_digest["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("pattern_mismatch"));

        let bad_key = call_tool(
            &mut server,
            "research_campaign_run_offline",
            json!({
                "spec_path": "campaign.json",
                "stage_input_paths": {"": "input.json"},
                "output_dir": "out"
            }),
        );
        assert_eq!(bad_key["result"]["isError"], true);
        assert!(bad_key["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("min_length at /stage_input_paths/"));

        let bad_value = call_tool(
            &mut server,
            "research_campaign_run_offline",
            json!({
                "spec_path": "campaign.json",
                "stage_input_paths": {"stage-a": ""},
                "output_dir": "out"
            }),
        );
        assert_eq!(bad_value["result"]["isError"], true);
        assert!(bad_value["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("min_length at /stage_input_paths/stage-a"));
    }
}
