//! Deciding what one commit changes in the index. Planning is a pure function of the index, the
//! commit's files and the parsed new blobs, so every rule of the reconciliation table can be
//! tested without Git or PostgreSQL; [`crate::Jottings`] only gathers inputs and applies steps.
use crate::repo::TreeFile;
use gix::ObjectId;
use palace_db::{IndexedJotting, JottingLocation};
use palace_domain::{Jotting, JottingCensus, JottingError, JottingProblem};
use std::collections::{HashMap, HashSet};
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

/// Plans the steps that make the index match `files`, or every problem that makes the commit
/// unusable as a whole. `parsed` holds each unknown blob; `inbox_has` tells whether a pending
/// upload's file is still in the work tree.
pub(crate) fn plan(
    index: &[IndexedJotting],
    files: &[TreeFile],
    parsed: &HashMap<ObjectId, Result<Jotting, JottingError>>,
    inbox_has: impl Fn(&str) -> bool,
) -> Result<Vec<Step>, Vec<JottingProblem>> {
    let known = known_blobs(index);
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
                if !present.contains(id) && !inbox_has(&location.path) =>
            {
                steps.push(Step::Delete(*id));
            }
            IndexedJotting::Committed { .. }
            | IndexedJotting::Pending { .. }
            | IndexedJotting::Deleted { .. } => {}
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
