//! The crate's own citation set, and the scanner that checks it.
//!
//! `tools/coverage.sh` counts a blueprint module as covered when its `NN.MM` token appears
//! anywhere under `crates/`. Coverage therefore moves when a crate *writes an id*, not when it
//! implements anything, and the cheapest way to raise the number is to mention a module while
//! explaining that it is out of scope. `crates/devplat` built two mechanisms against that and
//! this is the same idea in the form this crate needs: [`scan`] reimplements the coverage
//! script's token rule, [`DECLARED`] is the list of modules actually implemented here, and a test
//! runs the first over every file in the crate and asserts it equals the second.
//!
//! The consequence is deliberate. Section 12 has fifteen other modules and eight of them are
//! relevant to something here — the event and object models, content-addressed object storage,
//! the search and vector projections, the cache, the queue, idempotency, backup, and the quota
//! model. Every one is discussed in this crate's documentation **by title and by owning crate**,
//! and not one of their ids appears, so `tools/coverage.sh` will keep reporting them exactly as
//! it did before this crate existed. That is the correct outcome: they are `bioprism-ledger`'s,
//! `bioprism-store`'s and `bioprism-infra`'s, and a citation here would move a number without
//! moving a capability.
//!
//! [`scan`] takes text rather than a path. This crate performs no I/O; the test supplies the
//! bytes.

use std::collections::BTreeSet;

/// The seven blueprint modules this crate implements, and the complete set of ids it may contain.
pub const DECLARED: [&str; 7] = [
    "12.02", "12.03", "12.12", "12.13", "12.14", "12.15", "12.16",
];

pub fn declared() -> BTreeSet<String> {
    DECLARED.iter().map(|id| id.to_string()).collect()
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Every blueprint module id in `text`, by the rule `tools/coverage.sh` uses.
///
/// The script's expression is `\b(0[1-9]|[1-4][0-9])\.[0-9]{2}\b`. Reproduced by hand because
/// this workspace builds offline against pinned dependencies and has no regex crate; the cost of
/// that is that a change to the script has to be mirrored here, and the test that would catch the
/// drift does not exist, which is worth knowing.
pub fn scan(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut found = BTreeSet::new();
    for start in 0..bytes.len() {
        if start + 5 > bytes.len() {
            break;
        }
        let window = &bytes[start..start + 5];
        if !(window[0].is_ascii_digit()
            && window[1].is_ascii_digit()
            && window[2] == b'.'
            && window[3].is_ascii_digit()
            && window[4].is_ascii_digit())
        {
            continue;
        }
        if start > 0 && is_word(bytes[start - 1]) {
            continue;
        }
        if start + 5 < bytes.len() && is_word(bytes[start + 5]) {
            continue;
        }
        let section = (window[0] - b'0') * 10 + (window[1] - b'0');
        if !(1..=49).contains(&section) {
            continue;
        }
        found.insert(text[start..start + 5].to_string());
    }
    found
}
