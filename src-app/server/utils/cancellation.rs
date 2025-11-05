use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};
use uuid::Uuid;

/// Cancellation token for tracking and cancelling operations
#[derive(Debug, Clone)]
pub struct CancellationToken {
    receiver: Arc<RwLock<Option<oneshot::Receiver<()>>>>,
}

impl CancellationToken {
    fn new(_download_id: Uuid, receiver: oneshot::Receiver<()>) -> Self {
        Self {
            receiver: Arc::new(RwLock::new(Some(receiver))),
        }
    }

    /// Check if cancellation has been requested
    pub async fn is_cancelled(&self) -> bool {
        let mut receiver_guard = self.receiver.write().await;
        if let Some(mut receiver) = receiver_guard.take() {
            match receiver.try_recv() {
                Ok(_) => true,
                Err(oneshot::error::TryRecvError::Empty) => {
                    // Put the receiver back since it wasn't consumed
                    *receiver_guard = Some(receiver);
                    false
                }
                Err(oneshot::error::TryRecvError::Closed) => true,
            }
        } else {
            // Receiver was already consumed, meaning cancellation was requested
            true
        }
    }
}

/// Global cancellation tracker for managing download cancellations
pub struct CancellationTracker {
    cancellation_senders: RwLock<HashMap<Uuid, oneshot::Sender<()>>>,
}

impl CancellationTracker {
    pub fn new() -> Self {
        Self {
            cancellation_senders: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new cancellation token for a download
    pub async fn create_token(&self, download_id: Uuid) -> CancellationToken {
        let (sender, receiver) = oneshot::channel();

        let mut senders = self.cancellation_senders.write().await;
        senders.insert(download_id, sender);

        CancellationToken::new(download_id, receiver)
    }

    /// Cancel a download by its ID
    pub async fn cancel_download(&self, download_id: Uuid) -> bool {
        let mut senders = self.cancellation_senders.write().await;
        if let Some(sender) = senders.remove(&download_id) {
            // Send cancellation signal
            let _ = sender.send(());
            true
        } else {
            false
        }
    }

    /// Remove a download from tracking (cleanup when download completes)
    pub async fn remove_download(&self, download_id: Uuid) {
        let mut senders = self.cancellation_senders.write().await;
        senders.remove(&download_id);
    }
}

// Global instance
pub static CANCELLATION_TRACKER: once_cell::sync::Lazy<CancellationTracker> =
    once_cell::sync::Lazy::new(|| CancellationTracker::new());

/// Convenience function to create a cancellation token
pub async fn create_cancellation_token(download_id: Uuid) -> CancellationToken {
    CANCELLATION_TRACKER.create_token(download_id).await
}

/// Convenience function to cancel a download
pub async fn cancel_download(download_id: Uuid) -> bool {
    CANCELLATION_TRACKER.cancel_download(download_id).await
}

/// Convenience function to remove a download from tracking
pub async fn remove_download_tracking(download_id: Uuid) {
    CANCELLATION_TRACKER.remove_download(download_id).await
}
