//! Durable single-owner sync state; transient responses cannot acknowledge later local mutations.
mod engine;
mod store;
#[cfg(test)]
mod tests;
pub use engine::{RoundResult, SyncClient, SyncTransport};
pub use store::{LocalRecord, Mutation, Replica, SyncError};

mod http;
pub use http::HttpTransport;
