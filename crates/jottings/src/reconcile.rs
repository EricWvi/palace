//! Deciding what one commit changes in the index. Planning is a pure function of the index, the
//! commit's files and the parsed new blobs, so every rule of the reconciliation table can be
//! tested without Git or PostgreSQL; [`crate::Jottings`] only gathers inputs and applies steps.
use crate::repo::TreeFile;
use gix::ObjectId;
use palace_db::{IndexedJotting, JottingLocation, WebWrite};
use palace_domain::{Jotting, JottingCensus, JottingError, JottingProblem};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

/// One write to the index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// A jotting the index has never seen.
    Insert(Jotting, JottingLocation),
    /// New content for an indexed jotting or tombstone; the database assigns a version only when
    /// the content clients see differs.
    Update(Jotting, JottingLocation),
    /// The commit holds bytes the index already has, at another path or for the first time in a
    /// commit; the content is unchanged.
    Relocate(Uuid, JottingLocation),
    Delete(Uuid),
    /// A web write the commit left out is still in the work tree at this path, where the commit
    /// has the jotting now; the index keeps the web's result.
    Follow(Uuid, String),
    /// The commit decides the jotting again; its web write is no longer kept.
    Forget(Uuid),
}

/// Blobs of the commit the index does not already hold; only these need to be read and parsed.
pub(crate) fn unknown_blobs(index: &[IndexedJotting], files: &[TreeFile]) -> Vec<ObjectId> {
    let known = known_blobs(index);
    let mut unknown: Vec<ObjectId> = files
        .iter()
        .filter(|file| !known.contains_key(file.oid.as_bytes()))
        .map(|file| file.oid)
        .collect();
    unknown.sort_unstable();
    unknown.dedup();
    unknown
}

/// What reconciliation may look at in the work tree: whether a pending upload's file is still
/// there, and the blob of a file a web write left there. Nothing else of the work tree is read.
pub(crate) trait WorkTree {
    /// Whether the file at `path` exists.
    fn has(&self, path: &str) -> bool;
    /// The Git blob id of the file at `path`, or `None` when there is no file.
    fn blob(&self, path: &str) -> Option<ObjectId>;
}

/// Plans the steps that make the index match `files`, or every problem that makes the commit
/// unusable as a whole. `parsed` holds each unknown blob; `web` holds the web writes no commit
/// carried yet, which the work tree may still hold.
pub(crate) fn plan(
    index: &[IndexedJotting],
    files: &[TreeFile],
    parsed: &HashMap<ObjectId, Result<Jotting, JottingError>>,
    web: &[WebWrite],
    work_tree: &impl WorkTree,
) -> Result<Vec<Step>, Vec<JottingProblem>> {
    let known = known_blobs(index);
    // Ordered, so the steps come out the same on every run.
    let web: BTreeMap<Uuid, &WebWrite> = web.iter().map(|write| (write.id, write)).collect();
    let indexed: HashSet<Uuid> = index
        .iter()
        .map(|entry| match entry {
            IndexedJotting::Pending { id, .. }
            | IndexedJotting::Committed { id, .. }
            | IndexedJotting::Deleted { id } => *id,
        })
        .collect();
    let mut census = JottingCensus::default();
    let mut steps = Vec::new();
    let mut present = HashSet::new();
    for file in files {
        let location = JottingLocation {
            path: file.path.clone(),
            blob_oid: file.oid.as_bytes().to_vec(),
        };
        if let Some(&(id, indexed)) = known.get(file.oid.as_bytes()) {
            census.record(&file.path, id);
            present.insert(id);
            // Known bytes of a web-written jotting are the web's own: the commit carries them.
            if web.contains_key(&id) {
                steps.push(Step::Forget(id));
            }
            if !matches!(indexed, IndexedJotting::Committed { location: current, .. } if current == &location)
            {
                steps.push(Step::Relocate(id, location));
            }
            continue;
        }
        let parsed = parsed
            .get(&file.oid)
            .cloned()
            .unwrap_or(Err(JottingError::NoFrontmatter));
        let Some(jotting) = census.admit(&file.path, parsed) else {
            continue;
        };
        present.insert(jotting.id);
        if let Some(write) = web.get(&jotting.id) {
            // The commit still has the bytes the web started from: the web's result stands as
            // long as the work tree, where the commit has the file, still holds it.
            if file.oid.as_bytes() == write.base_oid.as_slice() {
                let held = work_tree
                    .blob(&file.path)
                    .map(|oid| oid.as_bytes().to_vec());
                if held == write.written_oid {
                    if write.path != file.path {
                        steps.push(Step::Follow(jotting.id, file.path.clone()));
                    }
                    continue;
                }
            }
            steps.push(Step::Forget(jotting.id));
        }
        steps.push(if indexed.contains(&jotting.id) {
            Step::Update(jotting, location)
        } else {
            Step::Insert(jotting, location)
        });
    }
    census.finish()?;
    for entry in index {
        match entry {
            IndexedJotting::Committed { id, .. } if !present.contains(id) => {
                steps.push(Step::Delete(*id));
            }
            // An upload waits for its commit as long as its file is in the work tree; deleted
            // before ever being committed, it becomes a tombstone like any other jotting.
            IndexedJotting::Pending { id, location }
                if !present.contains(id) && !work_tree.has(&location.path) =>
            {
                steps.push(Step::Delete(*id));
            }
            IndexedJotting::Committed { .. }
            | IndexedJotting::Pending { .. }
            | IndexedJotting::Deleted { .. } => {}
        }
    }
    // A web write whose jotting the commit no longer has is decided by the commit: an edit
    // becomes a deletion above, a deletion stays one.
    for id in web.keys() {
        if !present.contains(id) {
            steps.push(Step::Forget(*id));
        }
    }
    Ok(steps)
}

/// Live index entries by blob id; tombstones have no bytes.
fn known_blobs(index: &[IndexedJotting]) -> HashMap<&[u8], (Uuid, &IndexedJotting)> {
    index
        .iter()
        .filter_map(|entry| match entry {
            IndexedJotting::Pending { id, location }
            | IndexedJotting::Committed { id, location } => {
                Some((location.blob_oid.as_slice(), (*id, entry)))
            }
            IndexedJotting::Deleted { .. } => None,
        })
        .collect()
}

#[cfg(test)]
mod tests;
