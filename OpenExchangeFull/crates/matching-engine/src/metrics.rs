//! Metrics for the Matching Engine
//!
//! This module provides metrics collection for monitoring the matching engine.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Simple atomic counter
#[derive(Debug)]
pub struct Counter {
    value: AtomicU64,
}

impl Counter {
    pub fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
        }
    }

    pub fn increment(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple gauge for current values
#[derive(Debug)]
pub struct Gauge {
    value: AtomicU64,
}

impl Gauge {
    pub fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
        }
    }

    pub fn set(&self, value: u64) {
        self.value.store(value, Ordering::Relaxed);
    }

    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    pub fn increment(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    pub fn decrement(&self) {
        self.value.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Default for Gauge {
    fn default() -> Self {
        Self::new()
    }
}

/// Histogram for tracking latencies (simple implementation)
/// For production, consider using the `metrics` crate
#[derive(Debug)]
pub struct Histogram {
    count: AtomicU64,
    sum: AtomicU64,
    min: AtomicU64,
    max: AtomicU64,
}

impl Histogram {
    pub fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            sum: AtomicU64::new(0),
            min: AtomicU64::new(u64::MAX),
            max: AtomicU64::new(0),
        }
    }

    pub fn record(&self, value_us: u64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(value_us, Ordering::Relaxed);

        // Update min
        let current_min = self.min.load(Ordering::Relaxed);
        if value_us < current_min {
            self.min.store(value_us, Ordering::Relaxed);
        }

        // Update max
        let current_max = self.max.load(Ordering::Relaxed);
        if value_us > current_max {
            self.max.store(value_us, Ordering::Relaxed);
        }
    }

    pub fn get_stats(&self) -> HistogramStats {
        let count = self.count.load(Ordering::Relaxed);
        let sum = self.sum.load(Ordering::Relaxed);
        
        HistogramStats {
            count,
            sum_us: sum,
            avg_us: if count > 0 { sum / count } else { 0 },
            min_us: self.min.load(Ordering::Relaxed),
            max_us: self.max.load(Ordering::Relaxed),
        }
    }

    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
        self.sum.store(0, Ordering::Relaxed);
        self.min.store(u64::MAX, Ordering::Relaxed);
        self.max.store(0, Ordering::Relaxed);
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HistogramStats {
    pub count: u64,
    pub sum_us: u64,
    pub avg_us: u64,
    pub min_us: u64,
    pub max_us: u64,
}

/// Metrics for the matching engine
#[derive(Debug)]
pub struct MatchingEngineMetrics {
    pub orders_received: Counter,
    pub orders_matched: Counter,
    pub orders_rejected: Counter,
    pub trades_executed: Counter,
    pub order_processing_latency: Histogram,
    pub order_book_depth: Gauge,
    pub spread: Gauge,
}

impl MatchingEngineMetrics {
    pub fn new() -> Self {
        Self {
            orders_received: Counter::new(),
            orders_matched: Counter::new(),
            orders_rejected: Counter::new(),
            trades_executed: Counter::new(),
            order_processing_latency: Histogram::new(),
            order_book_depth: Gauge::new(),
            spread: Gauge::new(),
        }
    }

    pub fn record_order_received(&self) {
        self.orders_received.increment();
    }

    pub fn record_order_matched(&self) {
        self.orders_matched.increment();
    }

    pub fn record_order_rejected(&self) {
        self.orders_rejected.increment();
    }

    pub fn record_trade(&self, quantity: u32) {
        for _ in 0..quantity {
            self.trades_executed.increment();
        }
    }

    pub fn record_latency(&self, duration: Duration) {
        let us = duration.as_micros() as u64;
        self.order_processing_latency.record(us);
    }

    pub fn set_order_book_depth(&self, depth: u64) {
        self.order_book_depth.set(depth);
    }

    pub fn set_spread(&self, spread_bps: u64) {
        self.spread.set(spread_bps);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let latency_stats = self.order_processing_latency.get_stats();
        
        MetricsSnapshot {
            orders_received: self.orders_received.get(),
            orders_matched: self.orders_matched.get(),
            orders_rejected: self.orders_rejected.get(),
            trades_executed: self.trades_executed.get(),
            order_processing_latency_avg_us: latency_stats.avg_us,
            order_processing_latency_min_us: latency_stats.min_us,
            order_processing_latency_max_us: latency_stats.max_us,
            order_book_depth: self.order_book_depth.get(),
            spread_bps: self.spread.get(),
        }
    }

    pub fn reset(&self) {
        self.orders_received.reset();
        self.orders_matched.reset();
        self.orders_rejected.reset();
        self.trades_executed.reset();
        self.order_processing_latency.reset();
    }
}

impl Default for MatchingEngineMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsSnapshot {
    pub orders_received: u64,
    pub orders_matched: u64,
    pub orders_rejected: u64,
    pub trades_executed: u64,
    pub order_processing_latency_avg_us: u64,
    pub order_processing_latency_min_us: u64,
    pub order_processing_latency_max_us: u64,
    pub order_book_depth: u64,
    pub spread_bps: u64,
}
