//! Server HTTP and verified OIDC integration.
mod oidc;
pub use oidc::{LoginProof, LoginRedirect, OidcProvider};
mod http;
pub use http::{LoginProvider, Server, router};

#[cfg(feature = "test-server")]
pub use http::fixed_user_router;
