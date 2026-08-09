//! The `bioprism` command-line interface.
//!
//! Implements the local-first slice of blueprint 40.13. Four invariants from that contract are
//! enforced here rather than left to convention:
//!
//! * `--json` emits exactly one document on stdout and nothing else, so it is pipeable;
//! * `--dry-run` performs no filesystem writes;
//! * every command prints a reproducible follow-up command in human mode;
//! * every failure maps to a documented exit code (see [`exit::ExitCode`]).

mod args;
mod explain;
mod exit;
mod io;

use args::{Command, CompileOptions, Family, GenerateOptions, Invocation, Parsed, Profile};
use bioprism_fiber::compile;
use bioprism_scope::DimensionRegistry;
use bioprism_section::{CertificateProfile, ContextCertificate, OracleStatus};
use bioprism_world::{validate, Severity};
use exit::{CliError, CliResult, ExitCode};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    let parsed = match args::parse(raw) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("error [{}]: {}", error.code.slug(), error.message);
            eprintln!("\nRun `bioprism --help` for usage.");
            std::process::exit(error.code.as_i32());
        }
    };

    let invocation = match parsed {
        Parsed::Help => {
            print!("{}", args::help());
            std::process::exit(ExitCode::Ok.as_i32());
        }
        Parsed::Version => {
            println!("bioprism {}", env!("CARGO_PKG_VERSION"));
            std::process::exit(ExitCode::Ok.as_i32());
        }
        Parsed::Run(invocation) => invocation,
    };

    let json_mode = invocation.json;
    match run(&invocation) {
        Ok(outcome) => {
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&outcome.document).expect("serialisable")
                );
            } else {
                print!("{}", outcome.human);
            }
            std::process::exit(outcome.code.as_i32());
        }
        Err(error) => {
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&error.to_json()).expect("serialisable")
                );
            } else {
                match &error.subject {
                    Some(subject) => {
                        eprintln!("error [{}]: {}: {}", error.code.slug(), subject, error.message)
                    }
                    None => eprintln!("error [{}]: {}", error.code.slug(), error.message),
                }
            }
            std::process::exit(error.code.as_i32());
        }
    }
}

struct Outcome {
    code: ExitCode,
    document: Value,
    human: String,
}

impl Outcome {
    fn ok(document: Value, human: String) -> Self {
        Outcome {
            code: ExitCode::Ok,
            document,
            human,
        }
    }

    fn failing_if(mut self, condition: bool) -> Self {
        if condition {
            self.code = ExitCode::AssertionFailed;
        }
        self
    }
}

fn run(invocation: &Invocation) -> CliResult<Outcome> {
    match &invocation.command {
        Command::WorldValidate { world } => world_validate(world),
        Command::WorldShow { world } => world_show(world),
        Command::WorldGenerate(options) => world_generate(options),
        Command::WorldIndex { world, store, dry_run } => world_index(world, store, *dry_run),
        Command::PrismFork { world, query, bundle_out, minimize } => {
            prism_fork(world, query, bundle_out.as_deref(), *minimize)
        }
        Command::PrismMinimize { world } => prism_minimize(world),
        Command::MutateFamily { world, out_dir } => mutate_family(world, out_dir.as_deref()),
        Command::ContextExplain { world, query } => context_explain(world, query),
        Command::ContextCompile(options) => context_compile(options),
        Command::ContextVerify { certificate } => context_verify(certificate),
        Command::ContextCompare { world, query, markdown } => {
            context_compare(world, query, *markdown)
        }
    }
}

fn world_validate(world_path: &Path) -> CliResult<Outcome> {
    let world = io::load_world(world_path)?;
    let report = validate(&world, &DimensionRegistry::default());
    let errors = report
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warnings = report.diagnostics.len() - errors;

    let document = json!({
        "ok": errors == 0,
        "world_id": world.world_id.as_str(),
        "world_sha256": world.content_hash().as_str(),
        "counts": {
            "facts": world.facts.len(),
            "factors": world.factors.len(),
            "events": world.events.len(),
        },
        "errors": errors,
        "warnings": warnings,
        "diagnostics": report.diagnostics,
    });

    let mut human = format!(
        "world {} — {} facts, {} factors, {} events\n{} error(s), {} warning(s)\n",
        world.world_id,
        world.facts.len(),
        world.factors.len(),
        world.events.len(),
        errors,
        warnings
    );
    for diagnostic in &report.diagnostics {
        let label = match diagnostic.severity {
            Severity::Error => "error",
            Severity::Warning => "warn",
        };
        human.push_str(&format!(
            "  {label:<6} {:<34} {} — {}\n",
            diagnostic.code, diagnostic.subject, diagnostic.message
        ));
    }
    human.push_str(&format!(
        "\nNext: bioprism context explain --world {} --query <query.json>\n",
        world_path.display()
    ));

    Ok(Outcome::ok(document, human).failing_if(errors > 0))
}

fn world_show(world_path: &Path) -> CliResult<Outcome> {
    let world = io::load_world(world_path)?;

    let mut tag_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for fact in &world.facts {
        for tag in &fact.tags {
            *tag_counts.entry(tag.as_str()).or_default() += 1;
        }
    }
    let mut kind_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for factor in &world.factors {
        *kind_counts.entry(factor.kind.as_str()).or_default() += 1;
    }

    let document = json!({
        "ok": true,
        "world_id": world.world_id.as_str(),
        "world_sha256": world.content_hash().as_str(),
        "description": world.description,
        "counts": {
            "facts": world.facts.len(),
            "factors": world.factors.len(),
            "events": world.events.len(),
        },
        "fact_tags": tag_counts,
        "factor_kinds": kind_counts,
        "event_managed_variables": world.event_managed_variables(),
    });

    let mut human = format!("world {}\n", world.world_id);
    if let Some(description) = &world.description {
        human.push_str(&format!("  {description}\n"));
    }
    human.push_str(&format!(
        "  {} facts, {} factors, {} events\n  sha256 {}\n\nfact tags\n",
        world.facts.len(),
        world.factors.len(),
        world.events.len(),
        world.content_hash()
    ));
    for (tag, count) in &tag_counts {
        human.push_str(&format!("  {tag:<22} {count}\n"));
    }
    human.push_str("\nfactor kinds\n");
    for (kind, count) in &kind_counts {
        human.push_str(&format!("  {kind:<22} {count}\n"));
    }
    human.push_str(&format!(
        "\nNext: bioprism world validate --world {}\n",
        world_path.display()
    ));

    Ok(Outcome::ok(document, human))
}

fn world_generate(options: &GenerateOptions) -> CliResult<Outcome> {
    let spec = match options.family {
        Family::ReferenceLike => bioprism_worldgen::WorldSpec::reference_like(options.distractors),
        Family::Discriminating => bioprism_worldgen::WorldSpec::discriminating(options.distractors),
    };
    let generated = bioprism_worldgen::generate(&spec);

    let mut written = Vec::new();
    if let Some(path) = &options.world_out {
        written.push(io::write_artifact(path, &generated.world, options.dry_run)?);
    }
    if let Some(path) = &options.query_out {
        written.push(io::write_artifact(path, &generated.query, options.dry_run)?);
    }

    let facts = generated.world["facts"].as_array().map(Vec::len).unwrap_or(0);
    let factors = generated.world["factors"].as_array().map(Vec::len).unwrap_or(0);

    let document = json!({
        "ok": true,
        "world_id": generated.world["world_id"],
        "family": match options.family {
            Family::ReferenceLike => "reference-like",
            Family::Discriminating => "discriminating",
        },
        "counts": { "facts": facts, "factors": factors },
        "dry_run": options.dry_run,
        "artifacts": written
            .iter()
            .map(|a| json!({
                "path": a.path.display().to_string(),
                "bytes": a.bytes,
                "written": a.written,
            }))
            .collect::<Vec<_>>(),
    });

    let mut human = format!(
        "generated {} — {} facts, {} factors
",
        generated.world["world_id"].as_str().unwrap_or_default(),
        facts,
        factors
    );
    for artifact in &written {
        human.push_str(&format!(
            "  {} {} ({} bytes)
",
            if artifact.written { "wrote" } else { "would write" },
            artifact.path.display(),
            artifact.bytes
        ));
    }
    human.push_str("
Next: bioprism context compare --world <world.json> --query <query.json>
");

    Ok(Outcome::ok(document, human))
}

fn world_index(world_path: &Path, store_path: &Path, dry_run: bool) -> CliResult<Outcome> {
    let raw = io::read_json(world_path)?;
    io::refuse_to_rebind_store(store_path, &raw)?;

    if dry_run {
        return Ok(Outcome::ok(
            json!({ "ok": true, "dry_run": true, "store": store_path.display().to_string() }),
            format!("would index {} into {}
", world_path.display(), store_path.display()),
        ));
    }

    let manifest = bioprism_store::build(&raw, store_path)
        .map_err(|e| CliError::from_store(store_path, e))?;

    let document = json!({
        "ok": true,
        "world_id": manifest.world_id,
        "world_sha256": manifest.world_sha256,
        "store": store_path.display().to_string(),
        "counts": {
            "facts": manifest.total_facts,
            "factors": manifest.total_factors,
            "events": manifest.events.len(),
        },
    });
    let human = format!(
        "indexed {} — {} facts, {} factors
  store {}
  sha256 {}

Next: bioprism context explain --world {} --query <query.json>
",
        manifest.world_id,
        manifest.total_facts,
        manifest.total_factors,
        store_path.display(),
        manifest.world_sha256,
        store_path.display()
    );
    Ok(Outcome::ok(document, human))
}

fn context_compile(options: &CompileOptions) -> CliResult<Outcome> {
    let world = io::load_source(&options.world)?;
    let query = io::load_query(&options.query)?;
    let profile = match options.profile {
        Profile::Reference => CertificateProfile::Reference,
        Profile::Extended => CertificateProfile::Extended,
    };

    let out = compile(world.as_ref(), &query).map_err(CliError::from_compile)?;

    let certificate_document = out
        .certificate
        .to_json(profile)
        .map_err(|e| CliError::internal(e.to_string()))?;
    let section_document = out.section.to_json();

    let mut written = Vec::new();
    if let Some(path) = &options.certificate_out {
        written.push(io::write_artifact(path, &certificate_document, options.dry_run)?);
    }
    if let Some(path) = &options.section_out {
        written.push(io::write_artifact(path, &section_document, options.dry_run)?);
    }

    let digest = certificate_document["certificate_sha256"]
        .as_str()
        .unwrap_or_default();
    let invalid = out.certificate.oracle.status == OracleStatus::Invalid;

    let document = json!({
        "ok": true,
        "world_id": out.certificate.world_id,
        "query_id": out.certificate.query_id,
        "oracle": {
            "kind": out.certificate.oracle.oracle_kind,
            "status": out.certificate.oracle.status.as_str(),
            "witnesses": out.certificate.oracle.witness_kinds(),
        },
        "selected_facts": out.certificate.selected_facts.len(),
        "selected_factors": out.certificate.selected_factors.len(),
        "omitted_facts": out.certificate.omissions.total_facts,
        "protected_closure": out.certificate.protected_closure.len(),
        "protected_closure_satisfied": out.protected_closure_satisfied(),
        "supports_sufficiency_claim": out.certificate.manifest.supports_sufficiency_claim(),
        "certificate_sha256": digest,
        "profile": match profile {
            CertificateProfile::Reference => "reference",
            CertificateProfile::Extended => "extended",
        },
        "dry_run": options.dry_run,
        "artifacts": written
            .iter()
            .map(|a| json!({
                "path": a.path.display().to_string(),
                "bytes": a.bytes,
                "written": a.written,
            }))
            .collect::<Vec<_>>(),
    });

    let mut human = format!(
        "compiled {} facts and {} factors, omitted {} facts\noracle {} → {}\ncertificate sha256 {}\n",
        out.certificate.selected_facts.len(),
        out.certificate.selected_factors.len(),
        out.certificate.omissions.total_facts,
        out.certificate.oracle.oracle_kind,
        out.certificate.oracle.status.as_str(),
        digest
    );
    for witness in out.certificate.oracle.witness_kinds() {
        human.push_str(&format!("  witness {witness}\n"));
    }
    if !out.protected_closure_satisfied() {
        human.push_str(&format!(
            "  WARNING mandatory protected closure not delivered: {} facts withheld\n",
            out.trace.dropped_protected.len()
        ));
    }
    for artifact in &written {
        human.push_str(&format!(
            "  {} {} ({} bytes)\n",
            if artifact.written { "wrote" } else { "would write" },
            artifact.path.display(),
            artifact.bytes
        ));
    }
    human.push_str(&format!(
        "\nNext: bioprism context explain --world {} --query {}\n",
        options.world.display(),
        options.query.display()
    ));

    Ok(Outcome::ok(document, human).failing_if(invalid && options.fail_on_invalid))
}

fn context_explain(world_path: &Path, query_path: &Path) -> CliResult<Outcome> {
    let world = io::load_source(world_path)?;
    let query = io::load_query(query_path)?;
    let out = compile(world.as_ref(), &query).map_err(CliError::from_compile)?;

    let document = json!({
        "ok": true,
        "world_id": out.certificate.world_id,
        "query_id": out.certificate.query_id,
        "backend": out.certificate.plan.backend.as_str(),
        "passes": out.trace.passes
            .iter()
            .map(|p| json!({ "name": p.name, "retained": p.retained, "note": p.note }))
            .collect::<Vec<_>>(),
        "deferred_passes": out.trace.deferred_passes
            .iter()
            .map(|(name, reason)| json!({ "name": name, "reason": reason }))
            .collect::<Vec<_>>(),
        "selection": {
            "fact_ratio": out.certificate.plan.fact_selection_ratio(),
            "factor_ratio": out.certificate.plan.factor_selection_ratio(),
            "max_selected_factor_arity": out.certificate.plan.max_selected_factor_arity,
        },
        "omission_manifest": out.certificate.manifest,
        "supports_sufficiency_claim": out.certificate.manifest.supports_sufficiency_claim(),
        "unmatched_protected_tags": out.trace.unmatched_protected_tags,
        "dropped_protected": out.trace.dropped_protected,
    });

    let mut human = explain::render(&out);
    human.push_str(&format!(
        "\nNext: bioprism context compile --world {} --query {} --certificate-out cert.json\n",
        world_path.display(),
        query_path.display()
    ));

    Ok(Outcome::ok(document, human))
}

fn context_compare(world_path: &Path, query_path: &Path, markdown: bool) -> CliResult<Outcome> {
    let world = io::load_world(world_path)?;
    let query = io::load_query(query_path)?;

    let panel = bioprism_baseline::default_panel();
    let borrowed: Vec<&dyn bioprism_baseline::ContextStrategy> =
        panel.iter().map(|boxed| boxed.as_ref()).collect();
    let comparison = bioprism_baseline::compare(&world, &query, &borrowed);

    let mut human = comparison.to_markdown();
    if !markdown {
        human.push_str(&format!(
            "
Next: bioprism context explain --world {} --query {}
",
            world_path.display(),
            query_path.display()
        ));
    }

    // 43.41 stop rule: FIBER dropping a decisive witness, or failing to deliver the mandatory
    // protected closure, blocks advancement.
    let fiber_inadmissible = comparison
        .results
        .iter()
        .any(|r| r.name == "fiber" && !r.admissible());

    Ok(Outcome::ok(comparison.to_json(), human).failing_if(fiber_inadmissible))
}

fn prism_fork(
    world_path: &Path,
    query_path: &Path,
    bundle_out: Option<&Path>,
    with_minimization: bool,
) -> CliResult<Outcome> {
    use bioprism_prism::{
        matched_fork, minimize_world, render_table, Architecture, DecisionCell, InputRef,
        ResultBundle,
    };

    let world_raw = io::read_json(world_path)?;
    let query_raw = io::read_json(query_path)?;
    let world = io::load_world(world_path)?;
    let query = io::load_query(query_path)?;

    // Freeze the cell from the full-context verdict, so the acceptance contract is derived from
    // evidence rather than asserted by the operator.
    let reference = compile(&world, &query).map_err(CliError::from_compile)?;
    let mut cell = DecisionCell::new(
        format!("dc_{}", query.query_id.as_str()),
        "context policy under a frozen world and query",
        InputRef::new(world_path.display().to_string(), &world_raw),
        InputRef::new(query_path.display().to_string(), &query_raw),
    )
    .accepting(reference.certificate.oracle.status);
    for kind in reference.certificate.oracle.witness_kinds() {
        cell = cell.requiring_witness(kind);
    }

    let fork = matched_fork(&cell, &world, &query, &Architecture::default_panel());
    let regression_free = fork.is_regression_free();
    let human = render_table(&fork);

    let mut bundle = ResultBundle::new(cell, fork.clone());
    if with_minimization {
        bundle = bundle.with_minimization(minimize_world(&world));
    }
    let attested = bundle.attest();

    let mut artifacts = Vec::new();
    if let Some(path) = bundle_out {
        artifacts.push(io::write_artifact(path, &attested, false)?);
    }

    let mut document = json!({
        "ok": true,
        "cell_id": fork.cell_id,
        "passing": fork.passing,
        "failing": fork.failing,
        "unjudged": fork.unjudged,
        "not_attempted": fork.not_attempted,
        "attribution": fork.attribution,
        "arms": fork.arms,
        "bundle_sha256": attested["bundle_sha256"],
    });
    if let Some(map) = document.as_object_mut() {
        map.insert(
            "artifacts".into(),
            json!(artifacts
                .iter()
                .map(|a| json!({ "path": a.path.display().to_string(), "bytes": a.bytes }))
                .collect::<Vec<_>>()),
        );
    }

    let mut human = human;
    for artifact in &artifacts {
        human.push_str(&format!("
wrote {} ({} bytes)
", artifact.path.display(), artifact.bytes));
    }
    human.push_str(&format!(
        "
Next: bioprism prism minimize --world {}
",
        world_path.display()
    ));

    Ok(Outcome::ok(document, human).failing_if(!regression_free))
}

fn mutate_family(world_path: &Path, out_dir: Option<&Path>) -> CliResult<Outcome> {
    use bioprism_mutation::{generate, measure, standard_suite};

    let world_raw = io::read_json(world_path)?;
    let family = generate(&world_raw, &standard_suite()).map_err(CliError::from_mutation)?;
    let diversity = measure(std::slice::from_ref(&family));

    let mut written = Vec::new();
    if let Some(directory) = out_dir {
        for (id, world) in &family.worlds {
            let safe: String = id
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '.' { c } else { '_' })
                .collect();
            written.push(io::write_artifact(&directory.join(format!("{safe}.json")), world, false)?);
        }
    }

    let document = json!({
        "ok": true,
        "parent_id": family.parent_id,
        "parent_sha256": family.parent_sha256,
        "accepted": family.accepted,
        "rejected": family.rejected,
        "duplicates": family.duplicates,
        "yield_rate": family.yield_rate(),
        "diversity": diversity,
        "headline": diversity.headline(),
        "publishable": diversity.is_publishable(),
        "artifacts_written": written.len(),
    });

    let mut human = format!(
        "parent {} ({}...)

{} accepted, {} rejected, {} duplicate(s); yield {:.0}%

",
        family.parent_id,
        &family.parent_sha256[..12],
        family.accepted.len(),
        family.rejected.len(),
        family.duplicates.len(),
        family.yield_rate() * 100.0
    );
    human.push_str("| Instance | Family | Verdict | Witnesses |
|---|---|---|---:|
");
    for instance in &family.accepted {
        human.push_str(&format!(
            "| {} | {} | {} | {} |
",
            instance.mutation_id,
            instance.family,
            instance.status,
            instance.witnesses.len()
        ));
    }
    for rejection in &family.rejected {
        human.push_str(&format!("
- rejected `{}`: {}
", rejection.mutation_id, rejection.reason));
    }
    human.push_str(&format!("
{}
", diversity.headline()));
    human.push_str(&format!("publishable as a benchmark family: {}
", diversity.is_publishable()));
    if !written.is_empty() {
        human.push_str(&format!("
wrote {} world(s)
", written.len()));
    }
    human.push_str(&format!(
        "
Next: bioprism prism fork --world {} --query <query.json>
",
        world_path.display()
    ));

    Ok(Outcome::ok(document, human))
}

fn prism_minimize(world_path: &Path) -> CliResult<Outcome> {
    use bioprism_prism::{minimize_world, preserves};

    let world = io::load_world(world_path)?;
    let result = minimize_world(&world);
    let verified = preserves(&world, &result);

    let document = json!({
        "ok": verified,
        "started_from": result.started_from,
        "minimal": result.minimal,
        "removed": result.removed,
        "reduction_ratio": result.reduction_ratio(),
        "preserved_status": result.preserved_status,
        "preserved_witnesses": result.preserved_witnesses,
        "oracle_evaluations": result.evaluations,
        "guarantee": result.guarantee,
        "reverified": verified,
    });

    let human = format!(
        "minimized {} facts to {} ({:.2}% of the world), preserving {} with witnesses {}
           {} oracle evaluations
  re-verified: {}
  {}

{}

Next: bioprism prism fork --world {} --query <query.json>
",
        result.started_from,
        result.minimal.len(),
        result.reduction_ratio() * 100.0,
        result.preserved_status,
        result.preserved_witnesses.join(", "),
        result.evaluations,
        verified,
        result.guarantee,
        result.minimal.join("
"),
        world_path.display()
    );

    Ok(Outcome::ok(document, human).failing_if(!verified))
}

fn context_verify(path: &Path) -> CliResult<Outcome> {
    let document = io::read_json(path)?;
    let verification = ContextCertificate::verify(&document)
        .map_err(|e| CliError::invalid(e.to_string()).about(path.display().to_string()))?;

    use bioprism_section::CertificateVerification::*;
    let (ok, detail) = match &verification {
        Valid => (true, "digest verifies".to_string()),
        DigestMismatch { claimed, recomputed } => (
            false,
            format!("digest mismatch: claims {claimed}, recomputes to {recomputed}"),
        ),
        Malformed(reason) => (false, format!("malformed certificate: {reason}")),
    };

    let document = json!({
        "ok": ok,
        "certificate": path.display().to_string(),
        "verification": detail,
        "schema_version": document.get("schema_version"),
    });
    let human = format!(
        "{}: {}\n\nNext: bioprism world validate --world <world.json>\n",
        path.display(),
        detail
    );

    Ok(Outcome::ok(document, human).failing_if(!ok))
}
