//! The shipped worlds as bytes.
//!
//! Every world here is a pure function of its spec, so a fixture is redundant in principle. It is
//! not redundant in practice: a fixture is what lets a consumer in another language, or a reviewer
//! with no Rust toolchain, look at the world a claim is made about. `crates/worldgen` ships its
//! family the same way, under `fixtures/generated/`.
//!
//! The files are `include_str!`'d rather than read at runtime so that
//! `tests/fixtures_are_current.rs` compares *the committed bytes* against a fresh build. A test
//! that read the file it had just written would pass unconditionally.
//!
//! `.gitattributes` pins `*.json` to `eol=lf`, so the comparison is byte-exact on every checkout.
//! That rule exists because a CRLF/LF digest mismatch reproduces on one machine and not another.

use crate::builder::BioWorld;
use crate::error::BioWorldError;
use serde_json::Value;

/// §38.08 at the discriminating corner.
pub const TRIAL_ELIGIBILITY_TEMPORAL_FIREWALL_WORLD: &str =
    include_str!("../fixtures/trial_eligibility_temporal_firewall_world.json");
pub const TRIAL_ELIGIBILITY_TEMPORAL_FIREWALL_QUERY: &str =
    include_str!("../fixtures/trial_eligibility_temporal_firewall_query.json");

/// §38.08 at the reference world's corner. The unfavourable control.
pub const TRIAL_ELIGIBILITY_REFERENCE_SHAPED_CONTROL_WORLD: &str =
    include_str!("../fixtures/trial_eligibility_firewall_reference_shaped_control_world.json");
pub const TRIAL_ELIGIBILITY_REFERENCE_SHAPED_CONTROL_QUERY: &str =
    include_str!("../fixtures/trial_eligibility_firewall_reference_shaped_control_query.json");

/// §38.02 with nothing discriminating collected.
pub const POST_TREATMENT_UNDERDETERMINATION_WORLD: &str =
    include_str!("../fixtures/post_treatment_underdetermination_world.json");
pub const POST_TREATMENT_UNDERDETERMINATION_QUERY: &str =
    include_str!("../fixtures/post_treatment_underdetermination_query.json");

/// §38.02 with perfusion collected.
pub const POST_TREATMENT_RESOLVED_CONTROL_WORLD: &str =
    include_str!("../fixtures/post_treatment_resolved_control_world.json");
pub const POST_TREATMENT_RESOLVED_CONTROL_QUERY: &str =
    include_str!("../fixtures/post_treatment_resolved_control_query.json");

/// Slice id, world bytes, query bytes — in catalogue order.
pub const SHIPPED: [(&str, &str, &str); 4] = [
    (
        "trial-eligibility-temporal-firewall",
        TRIAL_ELIGIBILITY_TEMPORAL_FIREWALL_WORLD,
        TRIAL_ELIGIBILITY_TEMPORAL_FIREWALL_QUERY,
    ),
    (
        "trial-eligibility-firewall-reference-shaped-control",
        TRIAL_ELIGIBILITY_REFERENCE_SHAPED_CONTROL_WORLD,
        TRIAL_ELIGIBILITY_REFERENCE_SHAPED_CONTROL_QUERY,
    ),
    (
        "post-treatment-underdetermination",
        POST_TREATMENT_UNDERDETERMINATION_WORLD,
        POST_TREATMENT_UNDERDETERMINATION_QUERY,
    ),
    (
        "post-treatment-resolved-control",
        POST_TREATMENT_RESOLVED_CONTROL_WORLD,
        POST_TREATMENT_RESOLVED_CONTROL_QUERY,
    ),
];

/// Loads a shipped world from its bytes, exactly as a consumer without this crate's builder would.
pub fn load(world_text: &str) -> Result<BioWorld, BioWorldError> {
    BioWorld::from_json_str(world_text)
}

/// The query document beside a shipped world.
pub fn load_query(query_text: &str) -> Result<Value, BioWorldError> {
    serde_json::from_str(query_text).map_err(|source| BioWorldError::WorldRejected {
        world_id: "<query>".into(),
        message: source.to_string(),
    })
}
