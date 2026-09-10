use crate::{Mutation, Replica, SyncError};
use palace_domain::{Record, ServerVersion, SyncPage, UploadResult};
use std::{future::Future, sync::Mutex};

/// Carries owner-authenticated record requests; implementations must return committed per-record results.
pub trait SyncTransport: Send + Sync {
    /// Uploads a finite persisted snapshot without advancing the pull cursor.
    fn upload(
        &self,
        records: Vec<Record>,
    ) -> impl Future<Output = Result<Vec<UploadResult>, SyncError>> + Send;
    /// Returns the next committed page in the transport's authenticated owner scope.
    fn pull(
        &self,
        cursor: ServerVersion,
    ) -> impl Future<Output = Result<SyncPage, SyncError>> + Send;
}
#[derive(Debug, PartialEq, Eq)]
pub struct RoundResult {
    pub acknowledged: usize,
    pub pulled: usize,
    pub upload_failed: bool,
}
/// Owns one runtime sync scope; the round mutex excludes overlapping rounds while edits remain available.
pub struct SyncClient {
    replica: Mutex<Replica>,
    round: tokio::sync::Mutex<()>,
}
impl SyncClient {
    pub fn new(replica: Replica) -> Self {
        Self {
            replica: Mutex::new(replica),
            round: tokio::sync::Mutex::new(()),
        }
    }
    /// Persists edits while network requests run, without holding the synchronization round lock.
    pub fn edit(&self, record: Record) -> Result<Mutation, SyncError> {
        self.replica
            .lock()
            .map_err(|_| SyncError::Poisoned)?
            .edit(record)
    }
    /// Reads a current business record without blocking an in-flight synchronization round.
    pub fn record(&self, id: uuid::Uuid) -> Result<Option<crate::LocalRecord>, SyncError> {
        self.replica
            .lock()
            .map_err(|_| SyncError::Poisoned)?
            .record(id)
    }
    /// Runs upload before pull with finite deadlines; failed uploads remain pending and do not block consumption.
    pub async fn synchronize<T: SyncTransport>(
        &self,
        transport: &T,
    ) -> Result<RoundResult, SyncError> {
        let _round = self.round.lock().await;
        let snapshots = self
            .replica
            .lock()
            .map_err(|_| SyncError::Poisoned)?
            .pending()?;
        let mut acknowledged = 0;
        let mut upload_failed = false;
        if !snapshots.is_empty() {
            let records = snapshots
                .iter()
                .map(|snapshot| snapshot.record.clone())
                .collect();
            match tokio::time::timeout(
                std::time::Duration::from_secs(15),
                transport.upload(records),
            )
            .await
            {
                Ok(Ok(results)) if results.len() == snapshots.len() => {
                    for (snapshot, result) in snapshots.iter().zip(&results) {
                        self.replica
                            .lock()
                            .map_err(|_| SyncError::Poisoned)?
                            .acknowledge(snapshot, result)?;
                        acknowledged += 1;
                    }
                }
                Ok(Ok(_)) | Ok(Err(_)) | Err(_) => upload_failed = true,
            }
        }
        let cursor = self
            .replica
            .lock()
            .map_err(|_| SyncError::Poisoned)?
            .cursor()?;
        let page = tokio::time::timeout(std::time::Duration::from_secs(15), transport.pull(cursor))
            .await
            .map_err(|_| SyncError::Transport)??;
        self.replica
            .lock()
            .map_err(|_| SyncError::Poisoned)?
            .apply_page(&page)?;
        Ok(RoundResult {
            acknowledged,
            pulled: page.records.len(),
            upload_failed,
        })
    }
}
