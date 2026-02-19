//! Graceful shutdown utilities using CancellationToken
//!
//! This module provides shutdown coordination using `tokio_util::sync::CancellationToken`,
//! which is designed for exactly this use case. Unlike oneshot channels:
//! - Tokens can be cloned and shared across multiple tasks
//! - Child tokens can be created that are cancelled when parent is cancelled
//! - Cancellation can be checked without consuming the token
//! - Manual cancellation is supported

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

/// A shutdown controller that coordinates graceful shutdown across multiple components.
///
/// # Example
///
/// ```ignore
/// let shutdown = ShutdownController::new();
///
/// // Clone token for each server
/// let http_token = shutdown.child_token();
/// let grpc_token = shutdown.child_token();
///
/// // Start servers with their tokens
/// tokio::spawn(async move {
///     http_server.run(http_token).await;
/// });
///
/// // Wait for Ctrl+C or manual shutdown
/// shutdown.wait_for_shutdown().await;
///
/// // Or trigger manual shutdown
/// shutdown.shutdown();
/// ```
#[derive(Clone)]
pub struct ShutdownController {
    token: CancellationToken,
}

impl Default for ShutdownController {
    fn default() -> Self {
        Self::new()
    }
}

impl ShutdownController {
    /// Create a new shutdown controller
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new(),
        }
    }

    /// Create a new shutdown controller that listens for Ctrl+C
    ///
    /// This spawns a background task that will cancel the token when Ctrl+C is received.
    pub fn with_ctrl_c() -> Self {
        let controller = Self::new();
        let token = controller.token.clone();

        tokio::spawn(async move {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    info!("Received Ctrl+C, initiating graceful shutdown...");
                    token.cancel();
                }
                Err(e) => {
                    warn!("Failed to listen for Ctrl+C: {}", e);
                }
            }
        });

        controller
    }

    /// Get a child token that will be cancelled when this controller is cancelled.
    ///
    /// Child tokens can also be cancelled independently without affecting the parent.
    pub fn child_token(&self) -> CancellationToken {
        self.token.child_token()
    }

    /// Get a clone of the main token
    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    /// Trigger shutdown manually
    pub fn shutdown(&self) {
        info!("Manual shutdown triggered");
        self.token.cancel();
    }

    /// Check if shutdown has been triggered
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// Wait for shutdown to be triggered (either Ctrl+C or manual)
    pub async fn wait_for_shutdown(&self) {
        self.token.cancelled().await;
    }

    /// Wait for shutdown with a custom action before cancellation
    pub async fn wait_for_shutdown_with<F, Fut>(&self, on_shutdown: F)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        self.token.cancelled().await;
        on_shutdown().await;
    }
}

/// Create a shutdown signal that listens for Ctrl+C
///
/// This is a convenience function that returns just the token for simple use cases.
pub fn shutdown_signal() -> CancellationToken {
    ShutdownController::with_ctrl_c().token()
}

/// Utility to run a future until shutdown is signalled
///
/// Returns `Some(result)` if the future completed, `None` if shutdown was triggered first.
pub async fn run_until_shutdown<F, T>(token: CancellationToken, future: F) -> Option<T>
where
    F: std::future::Future<Output = T>,
{
    tokio::select! {
        result = future => Some(result),
        _ = token.cancelled() => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_manual_shutdown() {
        let controller = ShutdownController::new();
        let token = controller.child_token();

        assert!(!controller.is_cancelled());
        assert!(!token.is_cancelled());

        controller.shutdown();

        assert!(controller.is_cancelled());
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_child_token_independence() {
        let controller = ShutdownController::new();
        let child1 = controller.child_token();
        let child2 = controller.child_token();

        // Cancelling child1 doesn't affect parent or child2
        child1.cancel();

        assert!(child1.is_cancelled());
        assert!(!child2.is_cancelled());
        assert!(!controller.is_cancelled());

        // But cancelling parent affects all children
        controller.shutdown();

        assert!(child2.is_cancelled());
    }

    #[tokio::test]
    async fn test_run_until_shutdown() {
        let token = CancellationToken::new();

        // Test: future completes before shutdown
        let result = run_until_shutdown(token.clone(), async { 42 }).await;
        assert_eq!(result, Some(42));

        // Test: shutdown before future completes
        let token2 = CancellationToken::new();
        let token2_clone = token2.clone();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            token2_clone.cancel();
        });

        let result = run_until_shutdown(token2, async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            42
        })
        .await;

        assert_eq!(result, None);
    }
}
