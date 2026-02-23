//! Event log for the matching engine
//!
//! The event log ensures determinism by recording all matching events
//! in sequence order. This enables crash recovery and replay.

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::event::MatchingEvent;

/// In-memory event log
pub struct EventLog {
    /// Events stored in sequence order
    events: Vec<MatchingEvent>,
    /// Current sequence number
    sequence: u64,
}

impl EventLog {
    /// Create a new event log
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            sequence: 0,
        }
    }

    /// Append an event to the log
    pub fn append(&mut self, event: MatchingEvent) {
        self.sequence = event.sequence();
        self.events.push(event);
        debug!(sequence = self.sequence, "Event appended to log");
    }

    /// Get events from a sequence number onwards
    pub fn get_from(&self, from_sequence: u64) -> Vec<MatchingEvent> {
        self.events
            .iter()
            .filter(|e| e.sequence() >= from_sequence)
            .cloned()
            .collect()
    }

    /// Get current sequence number
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Get total number of events
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if log is empty
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Clear the log
    pub fn clear(&mut self) {
        self.events.clear();
        self.sequence = 0;
    }

    /// Set sequence (for replay)
    pub fn set_sequence(&mut self, seq: u64) {
        self.sequence = seq;
    }
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe event log wrapper
pub type SharedEventLog = Arc<RwLock<EventLog>>;

/// Create a new shared event log
pub fn create_event_log() -> SharedEventLog {
    Arc::new(RwLock::new(EventLog::new()))
}
