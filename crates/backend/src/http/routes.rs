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
pub(super) const SLEEP_RUNNING: &str = "/api/sleep/running";
pub(super) const SLEEP_END: &str = "/api/sleep/running/end";
pub(super) const SLEEP_NIGHTS: &str = "/api/sleep/nights";
pub(super) const SLEEP_NIGHT: &str = "/api/sleep/nights/{id}";
pub(super) const HABITS: &str = "/api/habits";
pub(super) const HABIT: &str = "/api/habits/{id}";
pub(super) const HABIT_RETIRE: &str = "/api/habits/{id}/retire";
pub(super) const HABIT_CHECK: &str = "/api/habits/{id}/checks/{day}";
pub(super) const ACTIVITY_TYPES: &str = "/api/activity-types";
pub(super) const ACTIVITIES: &str = "/api/activities";
pub(super) const ACTIVITY: &str = "/api/activities/{id}";
pub(super) const TODOS: &str = "/api/todos";
pub(super) const TODO_PLAN: &str = "/api/todos/plan";
pub(super) const TODO: &str = "/api/todos/{id}";
pub(super) const TODO_BODY: &str = "/api/todos/{id}/body";
pub(super) const TODO_DONE: &str = "/api/todos/{id}/done";
pub(super) const TODO_COMPLETE: &str = "/api/todos/{id}/complete";
pub(super) const TODO_RESTORE: &str = "/api/todos/{id}/restore";
pub(super) const TODO_LISTS: &str = "/api/todo-lists";
pub(super) const TODO_LIST: &str = "/api/todo-lists/{id}";
pub(super) const TODO_LIST_COMPLETED: &str = "/api/todo-lists/{id}/completed";
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
            get SLEEP_RUNNING => sleep::running,
            put SLEEP_RUNNING => sleep::start,
            post SLEEP_END => sleep::end,
            get SLEEP_NIGHTS => sleep::nights,
            post SLEEP_NIGHTS => sleep::record,
            put SLEEP_NIGHT => sleep::update,
            delete SLEEP_NIGHT => sleep::delete,
            get HABITS => habits::board,
            post HABITS => habits::create,
            put HABIT => habits::update,
            post HABIT_RETIRE => habits::retire,
            put HABIT_CHECK => habits::check,
            get ACTIVITY_TYPES => activities::types,
            post ACTIVITY_TYPES => activities::add_type,
            get ACTIVITIES => activities::list,
            post ACTIVITIES => activities::record,
            put ACTIVITY => activities::update,
            delete ACTIVITY => activities::delete,
            get TODOS => todos::board,
            post TODOS => todos::create,
            post TODO_PLAN => todos::plan,
            put TODO => todos::update,
            delete TODO => todos::delete,
            get TODO_BODY => todos::body,
            put TODO_BODY => todos::set_body,
            put TODO_DONE => todos::done,
            post TODO_COMPLETE => todos::complete,
            post TODO_RESTORE => todos::restore,
            post TODO_LISTS => todo_lists::create,
            put TODO_LIST => todo_lists::update,
            delete TODO_LIST => todo_lists::delete,
            get TODO_LIST_COMPLETED => todo_lists::completed,
            delete TODO_LIST_COMPLETED => todo_lists::clear_completed,
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
