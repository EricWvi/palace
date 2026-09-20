//! Paths shared by the real Router and the OpenAPI operation annotations.

pub(super) const ME: &str = "/api/me";
pub(super) const SYNC: &str = "/api/sync";
pub(super) const IMPORT: &str = "/api/import";
pub(super) const IMPORT_FILE: &str = "/api/import/file";
pub(super) const CONVERSATIONS: &str = "/api/conversations";
pub(super) const CONVERSATION: &str = "/api/conversations/{id}";
pub(super) const PATHS: &str = "/api/conversations/{id}/paths";
pub(super) const PATH: &str = "/api/conversations/{id}/paths/{path_id}";
pub(super) const LOGIN: &str = "/auth/login";
pub(super) const CALLBACK: &str = "/auth/callback";
pub(super) const LOGOUT: &str = "/auth/logout";
pub(super) const LOGOUT_ALL: &str = "/auth/logout-all";
