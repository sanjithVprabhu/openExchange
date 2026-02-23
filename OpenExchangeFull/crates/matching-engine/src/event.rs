//! Event types for the matching engine
//!
//! These events are used for the event log to ensure determinism.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::Trade;

/// Event in the matching engine
///
/// These events are appended to the event log to ensure determinism
/// and enable crash recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MatchingEvent {
    /// An order was accepted into the book
    OrderAccepted {
        /// Order ID
        order_id: Uuid,
        /// Instrument ID
        instrument_id: String,
        /// Sequence number
        sequence: u64,
    },
    
    /// An order was cancelled
    OrderCancelled {
        /// Order ID
        order_id: Uuid,
        /// Instrument ID
        instrument_id: String,
        /// Sequence number
        sequence: u64,
    },
    
    /// A trade was executed
    TradeExecuted {
        /// Trade details
        trade: Trade,
        /// Sequence number
        sequence: u64,
    },
    
    /// Sequence was reset (for testing/recovery)
    SequenceReset {
        /// New sequence number
        sequence: u64,
    },
}

impl MatchingEvent {
    /// Get the sequence number for this event
    pub fn sequence(&self) -> u64 {
        match self {
            MatchingEvent::OrderAccepted { sequence, .. } => *sequence,
            MatchingEvent::OrderCancelled { sequence, .. } => *sequence,
            MatchingEvent::TradeExecuted { sequence, .. } => *sequence,
            MatchingEvent::SequenceReset { sequence, .. } => *sequence,
        }
    }
}
