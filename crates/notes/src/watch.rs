use crate::{CACHE_FILE, NoteLibrary};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::{sync::Arc, time::Duration};

/// Keeps a library in step with its cache file until dropped.
pub struct NoteWatch {
    _watcher: notify::RecommendedWatcher,
    poll: tokio::task::JoinHandle<()>,
}

impl Drop for NoteWatch {
    /// Stops polling together with the file events.
    fn drop(&mut self) {
        self.poll.abort();
    }
}

/// Reloads the library whenever its cache file changes; `None` for a library without a folder.
///
/// Only the folder's own entries are watched, not the notes below it: the script replaces the
/// cache by rename, which a watch on the file itself would lose with the old inode, and editing
/// notes must not wake the server at all. File events do not arrive on every mount (NFS, SMB),
/// so a poll compares the cache's stamp every `poll` as well.
pub fn watch(library: Arc<NoteLibrary>, poll: Duration) -> notify::Result<Option<NoteWatch>> {
    let Some(root) = library.root().map(ToOwned::to_owned) else {
        return Ok(None);
    };
    let events = library.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
        // Watcher errors leave the poll to catch up; reading the cache is what matters.
        let Ok(event) = event else { return };
        let touches_cache = !matches!(event.kind, EventKind::Access(_))
            && event
                .paths
                .iter()
                .any(|path| path.file_name().is_some_and(|name| name == CACHE_FILE));
        if touches_cache {
            events.reload();
        }
    })?;
    watcher.watch(&root, RecursiveMode::NonRecursive)?;
    let poll = tokio::spawn(async move {
        let mut interval = tokio::time::interval(poll);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // The first tick completes at once, and the library has just loaded.
        interval.tick().await;
        loop {
            interval.tick().await;
            let library = library.clone();
            let _ = tokio::task::spawn_blocking(move || library.refresh()).await;
        }
    });
    Ok(Some(NoteWatch {
        _watcher: watcher,
        poll,
    }))
}
