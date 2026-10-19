//! 札记 (jottings): Markdown files in a Git repository, maintained by a local agent and indexed by
//! the server one commit at a time.
//!
//! The server never scans the work tree. It watches the branch refs, and when HEAD moves it
//! reconciles the PostgreSQL index with that commit's tree, reading only blobs the index does not
//! already hold. Uploads from offline clients are written to `inbox/` and indexed at once; the
//! agent commits them later. The server never writes `.git`.
//! See `specs/decisions/server/jotting/0-jottings-indexed-from-git-commits.md`.
mod library;
mod reconcile;
mod repo;
mod watch;

pub use library::{INBOX, JottingService, Jottings, JottingsError, Reconciled, Uploaded};
pub use repo::{GitError, OpenError};
pub use watch::{JottingWatch, watch};
