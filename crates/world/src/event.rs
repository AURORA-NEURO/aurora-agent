//! The causal event structure.
//!
//! Blueprint 43.09: evidence availability is a partially ordered event structure, not one total
//! timeline. The distinction that matters for leakage is between `event_time` (when something
//! happened) and `availability_time` (when it could legally be read). A result generated in
//! June but describing a January specimen is *not* available to a January decision, and
//! collapsing the two fields is exactly the temporal-leakage bug the first vertical slice
//! (43.41) is built to catch.

use crate::error::WorldError;
use crate::json::{object, required_str, string_list};
use bioprism_ids::{EventId, VariableName};
use bioprism_scope::Timestamp;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct CausalEvent {
    pub id: EventId,
    pub event_time: Timestamp,
    pub availability_time: Timestamp,
    pub produces: Vec<VariableName>,
    pub causal_parents: Vec<EventId>,
}

impl CausalEvent {
    pub fn from_json(value: &Value) -> Result<Self, WorldError> {
        let map = object(value, "event")?;
        let raw_id = required_str(map, "id", "event")?;
        let subject = format!("event {raw_id}");

        let id = EventId::parse(raw_id.clone())
            .map_err(|e| WorldError::Identifier { subject: subject.clone(), message: e.to_string() })?;

        let timestamp = |field: &'static str| -> Result<Timestamp, WorldError> {
            let text = required_str(map, field, &subject)?;
            Timestamp::parse(&text).map_err(|e| WorldError::Timestamp {
                subject: format!("{subject}.{field}"),
                message: e.to_string(),
            })
        };

        Ok(CausalEvent {
            id,
            event_time: timestamp("event_time")?,
            availability_time: timestamp("availability_time")?,
            produces: string_list(map, "produces", &subject)?
                .into_iter()
                .map(|name| {
                    VariableName::parse(name).map_err(|e| WorldError::Identifier {
                        subject: subject.clone(),
                        message: e.to_string(),
                    })
                })
                .collect::<Result<_, _>>()?,
            causal_parents: string_list(map, "causal_parents", &subject)?
                .into_iter()
                .map(|name| {
                    EventId::parse(name).map_err(|e| WorldError::Identifier {
                        subject: subject.clone(),
                        message: e.to_string(),
                    })
                })
                .collect::<Result<_, _>>()?,
        })
    }

    /// Whether this event's products may be read by a decision taken at `cut`.
    pub fn is_available_at(&self, cut: Timestamp) -> bool {
        self.availability_time <= cut
    }

    /// True when a result was recorded as available before it happened, which is either a clock
    /// error or a backdated artifact. Reported as a diagnostic, never silently corrected.
    pub fn is_backdated(&self) -> bool {
        self.availability_time < self.event_time
    }
}
