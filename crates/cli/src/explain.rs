//! Explain-plan rendering.
//!
//! Blueprint 43.16 asks for "explain-plan output modeled after database systems". The point is
//! not decoration: a reader must be able to see which passes ran, what each retained, which
//! backend was chosen, and — most importantly — which passes did *not* run and why, before
//! trusting a compact context.

use bioprism_fiber::CompileOutput;
use bioprism_section::InfluenceClass;
use std::fmt::Write as _;

pub fn render(out: &CompileOutput) -> String {
    let cert = &out.certificate;
    let plan = &cert.plan;
    let mut text = String::new();

    let _ = writeln!(text, "FIBER COMPILE PLAN");
    let _ = writeln!(text, "  world   {}", cert.world_id);
    let _ = writeln!(text, "  query   {}", cert.query_id);
    let _ = writeln!(text, "  backend {}", plan.backend.as_str());
    match &plan.fallback {
        None => {
            let _ = writeln!(text, "  fallback none");
        }
        Some(fallback) => {
            let _ = writeln!(
                text,
                "  fallback {:?} — {}",
                fallback.reason, fallback.detail
            );
        }
    }
    let _ = writeln!(text);

    let _ = writeln!(text, "PASSES");
    for (position, pass) in out.trace.passes.iter().enumerate() {
        let _ = writeln!(
            text,
            "  {}. {:<18} retained {:<6} {}",
            position + 1,
            pass.name,
            pass.retained,
            pass.note
        );
    }
    let _ = writeln!(text);

    let _ = writeln!(text, "SELECTION");
    let _ = writeln!(
        text,
        "  facts    {:>6} / {:<6} ({:.2}% of world)",
        plan.compiled_fact_count,
        plan.total_fact_count,
        plan.fact_selection_ratio() * 100.0
    );
    let _ = writeln!(
        text,
        "  factors  {:>6} / {:<6} ({:.2}% of world)",
        plan.compiled_factor_count,
        plan.total_factor_count,
        plan.factor_selection_ratio() * 100.0
    );
    let _ = writeln!(
        text,
        "  max selected factor arity {}",
        plan.max_selected_factor_arity
    );
    let _ = writeln!(
        text,
        "  protected closure {} facts, all retained: {}",
        cert.protected_closure.len(),
        out.protected_closure_satisfied()
    );
    let _ = writeln!(text);

    let _ = writeln!(text, "OMISSIONS BY INFLUENCE");
    if cert.manifest.groups.is_empty() {
        let _ = writeln!(text, "  (nothing omitted)");
    }
    for group in &cert.manifest.groups {
        let _ = writeln!(
            text,
            "  {:<24} {:>6}  {}",
            group.influence.as_str(),
            group.count,
            group.reason
        );
    }
    let _ = writeln!(
        text,
        "  supports sufficiency claim: {}",
        cert.manifest.supports_sufficiency_claim()
    );
    if cert.manifest.count_in(InfluenceClass::Unknown) > 0 {
        let _ = writeln!(
            text,
            "  WARNING unknown-influence omissions present; this context is not sufficient"
        );
    }
    let _ = writeln!(text);

    let _ = writeln!(text, "PASSES NOT RUN");
    for (name, reason) in &out.trace.deferred_passes {
        let _ = writeln!(text, "  {name:<22} {reason}");
    }
    let _ = writeln!(text);

    let _ = writeln!(text, "ORACLE");
    let _ = writeln!(
        text,
        "  {} → {}",
        cert.oracle.oracle_kind,
        cert.oracle.status.as_str()
    );
    for kind in cert.oracle.witness_kinds() {
        let _ = writeln!(text, "    witness {kind}");
    }

    if !out.trace.unmatched_protected_tags.is_empty() {
        let _ = writeln!(text);
        let _ = writeln!(
            text,
            "WARNING protected tags matching no fact: {}",
            out.trace.unmatched_protected_tags.join(", ")
        );
    }
    if !out.trace.dropped_protected.is_empty() {
        let _ = writeln!(text);
        let _ = writeln!(
            text,
            "WARNING mandatory closure not delivered; {} protected facts withheld: {}",
            out.trace.dropped_protected.len(),
            out.trace.dropped_protected.join(", ")
        );
    }

    text
}
