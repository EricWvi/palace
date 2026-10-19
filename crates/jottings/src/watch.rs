use crate::Jottings;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use palace_logging::palace_error;
use std::{sync::Arc, time::Duration};
use tokio::sync::Notify;

/// Keeps the index following the branch's commits until dropped.
pub struct JottingWatch {
    _watcher: notify::RecommendedWatcher,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for JottingWatch {
    /// Stops reconciling together with the file events.
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Reconciles whenever a ref file changes, and reads HEAD every `poll` for mounts whose file
/// events never arrive.
///
/// Only the git dir, the common dir and the branch refs are watched, never the work tree: the
/// agent's edits must not wake the server, a commit is what counts. A commit touches many files
/// in these directories; the notifications collapse into one pending wake-up, and a wake-up
/// whose HEAD was already looked at only reads HEAD.
pub fn watch(jottings: Arc<Jottings>, poll: Duration) -> notify::Result<JottingWatch> {
    let wake = Arc::new(Notify::new());
    let events = wake.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
        // Watcher errors leave the poll to catch up.
        if let Ok(event) = event
            && !matches!(event.kind, EventKind::Access(_))
        {
            events.notify_one();
        }
    })?;
    let (git_dir, common_dir, heads) = jottings.ref_dirs();
    watcher.watch(&git_dir, RecursiveMode::NonRecursive)?;
    if common_dir != git_dir {
        watcher.watch(&common_dir, RecursiveMode::NonRecursive)?;
    }
    // Branch names may contain `/`, so their ref files can sit in subdirectories.
    watcher.watch(&heads, RecursiveMode::Recursive)?;
    let task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(poll);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = interval.tick() => {}
                () = wake.notified() => {}
            }
            if let Err(error) = jottings.refresh().await {
                palace_error!(message = "Jotting reconciliation failed; retrying on the next commit or poll", error = %error);
            }
        }
    });
    Ok(JottingWatch {
        _watcher: watcher,
        task,
    })
}
