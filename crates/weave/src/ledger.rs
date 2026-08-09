//! Append-only, hash-chained ledgers.
//!
//! Blueprint 23.07 (commitments) and 23.08 (epistemic state). Both are append-only: a claim that
//! is later challenged is not deleted, and a commitment that is discharged is not erased. The
//! record is what happened, not the current summary.
//!
//! Each entry chains the previous entry's digest, so any edit to history invalidates every
//! subsequent link — the tamper-evidence 23.49 requires of "causal event ordering and hashing".

use crate::act::{Act, ActKind};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerEvent {
    pub seq: usize,
    pub id: String,
    pub act: Act,
    /// Digest of the previous entry, or the empty string for the first.
    pub previous: String,
    pub hash: String,
}

#[derive(Debug, Clone, Default)]
pub struct Ledger {
    events: Vec<LedgerEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainStatus {
    Intact,
    Broken { at_seq: usize, reason: String },
}

impl ChainStatus {
    pub fn is_intact(&self) -> bool {
        matches!(self, ChainStatus::Intact)
    }
}

impl Ledger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn events(&self) -> &[LedgerEvent] {
        &self.events
    }

    pub fn get(&self, id: &str) -> Option<&LedgerEvent> {
        self.events.iter().find(|event| event.id == id)
    }

    pub fn head(&self) -> String {
        self.events
            .last()
            .map(|event| event.hash.clone())
            .unwrap_or_default()
    }

    pub fn append(&mut self, act: Act) -> LedgerEvent {
        let seq = self.events.len();
        let previous = self.head();
        let id = format!("evt-{seq:06}");

        let body = json!({
            "seq": seq,
            "id": id,
            "act": act,
            "previous": previous,
        });
        let hash = ContentHash::of_value(&body)
            .expect("act payload is finite JSON")
            .as_str()
            .to_string();

        let event = LedgerEvent {
            seq,
            id,
            act,
            previous,
            hash,
        };
        self.events.push(event.clone());
        event
    }

    /// Recomputes the chain, the way an auditor must before trusting a trace.
    pub fn verify_chain(&self) -> ChainStatus {
        let mut expected_previous = String::new();
        for event in &self.events {
            if event.previous != expected_previous {
                return ChainStatus::Broken {
                    at_seq: event.seq,
                    reason: "previous digest does not match the prior entry".into(),
                };
            }
            let body = json!({
                "seq": event.seq,
                "id": event.id,
                "act": event.act,
                "previous": event.previous,
            });
            let recomputed = ContentHash::of_value(&body)
                .expect("finite JSON")
                .as_str()
                .to_string();
            if recomputed != event.hash {
                return ChainStatus::Broken {
                    at_seq: event.seq,
                    reason: "entry digest does not match its contents".into(),
                };
            }
            expected_previous = event.hash.clone();
        }
        ChainStatus::Intact
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitmentState {
    Open,
    Discharged,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Commitment {
    pub id: String,
    pub debtor: String,
    pub creditor: String,
    pub subject: Value,
    pub state: CommitmentState,
}

/// Derives commitments from the act history.
///
/// A projection, not a separate source of truth: the ledger is authoritative and this is a view
/// over it, so the two can never disagree.
pub fn commitments(ledger: &Ledger) -> BTreeMap<String, Commitment> {
    let mut open: BTreeMap<String, Commitment> = BTreeMap::new();

    for event in ledger.events() {
        match event.act.kind {
            ActKind::Accept => {
                if let Some(proposal_id) = &event.act.in_reply_to {
                    if let Some(proposal) = ledger.get(proposal_id) {
                        open.insert(
                            event.id.clone(),
                            Commitment {
                                id: event.id.clone(),
                                // The proposer undertakes; the acceptor holds them to it.
                                debtor: proposal.act.from.clone(),
                                creditor: event.act.from.clone(),
                                subject: proposal.act.payload.clone(),
                                state: CommitmentState::Open,
                            },
                        );
                    }
                }
            }
            ActKind::Discharge => {
                if let Some(accept_id) = &event.act.in_reply_to {
                    if let Some(commitment) = open.get_mut(accept_id) {
                        commitment.state = CommitmentState::Discharged;
                    }
                }
            }
            _ => {}
        }
    }
    open
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpistemicEntry {
    pub claim_id: String,
    pub claimant: String,
    pub content: Value,
    /// Acts disputing this claim. The claim is not removed when challenged.
    pub challenges: Vec<String>,
}

impl EpistemicEntry {
    pub fn is_contested(&self) -> bool {
        !self.challenges.is_empty()
    }
}

/// Derives the epistemic state: claims, and what disputes them.
///
/// 23.08 requires contradiction to be *preserved* rather than resolved into a score. A contested
/// claim stays in the record alongside its challenges, and the reader decides.
pub fn epistemic_state(ledger: &Ledger) -> BTreeMap<String, EpistemicEntry> {
    let mut claims: BTreeMap<String, EpistemicEntry> = BTreeMap::new();

    for event in ledger.events() {
        match event.act.kind {
            ActKind::Claim => {
                claims.insert(
                    event.id.clone(),
                    EpistemicEntry {
                        claim_id: event.id.clone(),
                        claimant: event.act.from.clone(),
                        content: event.act.payload.clone(),
                        challenges: Vec::new(),
                    },
                );
            }
            ActKind::Challenge => {
                if let Some(target) = &event.act.in_reply_to {
                    if let Some(entry) = claims.get_mut(target) {
                        entry.challenges.push(event.id.clone());
                    }
                }
            }
            _ => {}
        }
    }
    claims
}
