//! Paths shared by the real Router and the OpenAPI operation annotations.

pub(super) const ME: &str = "/api/me";
pub(super) const SYNC: &str = "/api/sync";
pub(super) const IMPORT: &str = "/api/import";
pub(super) const IMPORT_FILE: &str = "/api/import/file";
pub(super) const TIMELINE: &str = "/api/timeline";
pub(super) const CONVERSATION: &str = "/api/conversations/{id}";
pub(super) const PATHS: &str = "/api/conversations/{id}/paths";
pub(super) const PATH: &str = "/api/conversations/{id}/paths/{path_id}";
pub(super) const PATH_METADATA: &str = "/api/conversations/{id}/paths/{path_id}/metadata";
pub(super) const LOGIN: &str = "/auth/login";
pub(super) const CALLBACK: &str = "/auth/callback";
pub(super) const LOGOUT: &str = "/auth/logout";
pub(super) const LOGOUT_ALL: &str = "/auth/logout-all";

// Consumers receive method, path and handler together; tests compare this exact mounted inventory.
macro_rules! auth_routes {
    ($consumer:ident) => {
        $consumer! {
            get LOGIN => handlers::login,
            get CALLBACK => handlers::callback,
            post LOGOUT => handlers::logout,
            post LOGOUT_ALL => handlers::logout_all,
        }
    };
}
macro_rules! business_routes {
    ($consumer:ident) => {
        $consumer! {
            get ME => business::me,
            get SYNC => sync::pull,
            post SYNC => sync::upload,
            post IMPORT => business::import_text,
            post IMPORT_FILE => business::import_file,
            get TIMELINE => timeline::timeline,
            get CONVERSATION => business::conversation,
            delete CONVERSATION => path_management::delete_conversation,
            post PATHS => path_management::create,
            get PATH => business::path,
            put PATH => path_management::update,
            delete PATH => path_management::delete,
            put PATH_METADATA => path_management::update_metadata,
        }
    };
}
pub(super) use {auth_routes, business_routes};
