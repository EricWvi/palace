//! Paths shared by the real Router and the OpenAPI operation annotations.

pub(super) const ME: &str = "/api/me";
pub(super) const SYNC: &str = "/api/sync";
pub(super) const IMPORT: &str = "/api/import";
pub(super) const IMPORT_FILE: &str = "/api/import/file";
pub(super) const TIMELINE: &str = "/api/timeline";
pub(super) const TIMELINE_OUTLINE: &str = "/api/timeline/outline";
pub(super) const CONVERSATIONS: &str = "/api/conversations";
pub(super) const CONVERSATION: &str = "/api/conversations/{id}";
pub(super) const PATHS: &str = "/api/conversations/{id}/paths";
pub(super) const PATH: &str = "/api/conversations/{id}/paths/{path_id}";
pub(super) const NOTES: &str = "/api/notes";
pub(super) const NOTE: &str = "/api/notes/{id}";
pub(super) const JOTTINGS: &str = "/api/jottings";
pub(super) const JOTTING_CHANGES: &str = "/api/jottings/changes";
pub(super) const JOTTING_OVERVIEW: &str = "/api/jottings/overview";
pub(super) const JOTTING_RECALL: &str = "/api/jottings/recall";
pub(super) const ARTICLES: &str = "/api/articles";
pub(super) const ARTICLE: &str = "/api/articles/{id}";
pub(super) const ARTICLE_ANNOTATIONS: &str = "/api/articles/{id}/annotations";
pub(super) const CONVERSATION_ANNOTATIONS: &str = "/api/conversations/{id}/annotations";
pub(super) const ANNOTATION: &str = "/api/annotations/{id}";
pub(super) const WEB_PAGES: &str = "/api/web-pages";
pub(super) const WEB_PAGE_LOOKUP: &str = "/api/web-pages/lookup";
pub(super) const WEB_PAGE_ANNOTATIONS: &str = "/api/web-pages/annotations";
pub(super) const WEB_PAGE_BINDING: &str = "/api/web-pages/binding";
pub(super) const WEB_ANNOTATION: &str = "/api/web-annotations/{id}";
pub(super) const ARTICLE_WEB_ANNOTATIONS: &str = "/api/articles/{id}/web-annotations";
pub(super) const ARTICLE_WEB_ANNOTATION_MIGRATION: &str =
    "/api/articles/{id}/web-annotations/migrate";
pub(super) const MEDIA: &str = "/api/media";
pub(super) const MEDIA_ITEM: &str = "/api/media/{id}";
pub(super) const ACCESS_TOKENS: &str = "/api/access-tokens";
pub(super) const ACCESS_TOKEN: &str = "/api/access-tokens/{id}";
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
            get TIMELINE_OUTLINE => timeline::outline,
            get CONVERSATIONS => conversation_list::list,
            get CONVERSATION => business::conversation,
            delete CONVERSATION => path_management::delete_conversation,
            post PATHS => path_management::create,
            get PATH => business::path,
            put PATH => path_management::update,
            delete PATH => path_management::delete,
            put PATH_METADATA => path_management::update_metadata,
            get NOTES => notes::list,
            get NOTE => notes::note,
            get JOTTING_CHANGES => jottings::changes,
            post JOTTINGS => jottings::upload,
            get JOTTINGS => jotting_reading::list,
            get JOTTING_OVERVIEW => jotting_reading::overview,
            get JOTTING_RECALL => jotting_reading::recall,
            get ARTICLES => articles::list,
            get ARTICLE => articles::article,
            put ARTICLE => articles::update,
            delete ARTICLE => articles::delete,
            get ARTICLE_ANNOTATIONS => annotations::article_list,
            post ARTICLE_ANNOTATIONS => annotations::article_create,
            get CONVERSATION_ANNOTATIONS => annotations::conversation_list,
            post CONVERSATION_ANNOTATIONS => annotations::conversation_create,
            put ANNOTATION => annotations::update,
            delete ANNOTATION => annotations::delete,
            get WEB_PAGES => web_pages::list,
            get WEB_PAGE_LOOKUP => web_pages::lookup,
            post WEB_PAGE_ANNOTATIONS => web_pages::create,
            put WEB_PAGE_BINDING => web_pages::bind,
            put WEB_ANNOTATION => web_pages::update,
            delete WEB_ANNOTATION => web_pages::delete,
            get ARTICLE_WEB_ANNOTATIONS => web_pages::article_list,
            post ARTICLE_WEB_ANNOTATION_MIGRATION => web_pages::migrate,
        }
    };
}
// Media carries file bodies, so it is mounted with its own body limit instead of the import one.
macro_rules! media_routes {
    ($consumer:ident) => {
        $consumer! {
            post MEDIA => media::upload,
            get MEDIA_ITEM => media::read,
            delete MEDIA_ITEM => media::delete,
        }
    };
}
// An article import carries its files, so it is mounted with the article body limit.
macro_rules! article_import_routes {
    ($consumer:ident) => {
        $consumer! {
            post ARTICLES => article_import::import,
        }
    };
}
// Token management accepts only a browser session, so it is mounted behind its own layer.
macro_rules! access_token_routes {
    ($consumer:ident) => {
        $consumer! {
            get ACCESS_TOKENS => access_tokens::list,
            post ACCESS_TOKENS => access_tokens::create,
            delete ACCESS_TOKEN => access_tokens::delete,
        }
    };
}
pub(super) use {
    access_token_routes, article_import_routes, auth_routes, business_routes, media_routes,
};
