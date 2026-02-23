//! Result types for matching operations

use super::domain::{BookOrder, Trade};

/// Result of a matching operation
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Trades generated from this matching operation
    pub trades: Vec<Trade>,
    /// Remaining order (if not fully filled)
    pub remaining_order: Option<BookOrder>,
    /// Whether the remaining order should be inserted into the book
    pub should_insert: bool,
}

impl MatchResult {
    /// No match occurred, order not filled
    pub fn no_match(order: BookOrder, should_insert: bool) -> Self {
        Self {
            trades: vec![],
            remaining_order: Some(order),
            should_insert,
        }
    }

    /// Order was fully matched
    pub fn fully_matched(trades: Vec<Trade>) -> Self {
        Self {
            trades,
            remaining_order: None,
            should_insert: false,
        }
    }

    /// Order was partially matched
    pub fn partial_match(trades: Vec<Trade>, remaining: BookOrder, should_insert: bool) -> Self {
        Self {
            trades,
            remaining_order: Some(remaining),
            should_insert,
        }
    }

    /// Order was cancelled (rejected, FOK failure, etc.)
    pub fn cancelled(order: BookOrder) -> Self {
        Self {
            trades: vec![],
            remaining_order: Some(order),
            should_insert: false,
        }
    }

    /// Check if any trades were generated
    pub fn has_trades(&self) -> bool {
        !self.trades.is_empty()
    }

    /// Total quantity filled
    pub fn filled_quantity(&self) -> u32 {
        self.trades.iter().map(|t| t.quantity).sum()
    }
}

/// Result of a cancel operation
#[derive(Debug, Clone)]
pub struct CancelResult {
    /// Whether the order was found and removed
    pub cancelled: bool,
    /// The order that was cancelled (if found)
    pub order: Option<BookOrder>,
}

impl CancelResult {
    /// Order was successfully cancelled
    pub fn cancelled(order: BookOrder) -> Self {
        Self {
            cancelled: true,
            order: Some(order),
        }
    }

    /// Order was not found (already filled or didn't exist)
    pub fn not_found() -> Self {
        Self {
            cancelled: false,
            order: None,
        }
    }
}
