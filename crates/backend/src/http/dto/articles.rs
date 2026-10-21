use chrono_tz::Tz;
use palace_domain::{ArticleMetadata, article_site};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// The `article` part of an import, sent before any file.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArticleImportFields {
    /// Nonblank after trimming; at most 1024 UTF-8 bytes; kept as sent.
    pub title: String,
    /// Markdown body, at most 4 MiB. Embedded files are inline images `![alt](media:<name>)`;
    /// the server replaces each address with `/api/media/<id>`. Never editable afterwards.
    pub content: String,
    /// Absolute http(s) URL of the original article, at most 2048 bytes; identifies the article
    /// within the owner, compared byte for byte.
    pub url: String,
    /// Optional; blank means none; at most 256 UTF-8 bytes.
    #[serde(default)]
    pub author: Option<String>,
    /// Publish date: `YYYY-MM-DD` (midnight in the deployment's zone) or RFC 3339 with an offset.
    pub date: String,
}

/// One `article` part, then one `file` part per embedded file.
// Documents the form only: the handler streams the parts, so no value is ever built.
#[expect(dead_code, reason = "OpenAPI schema of a streamed multipart body")]
#[derive(ToSchema)]
pub(crate) struct ArticleImportForm {
    /// JSON text of ArticleImportFields; must be the first part.
    #[schema(value_type = ArticleImportFields, content_media_type = "application/json")]
    pub article: Vec<u8>,
    /// Embedded files. Each filename is the `<name>` the body uses, 1-128 characters of
    /// `[A-Za-z0-9._-]`, unique within the import; the part's Content-Type is recorded. Every
    /// file must be embedded and every embedded name must have a file.
    #[schema(value_type = Option<Vec<String>>, format = Binary)]
    pub file: Option<Vec<Vec<u8>>>,
}

/// The fields an edit replaces; the body never changes.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArticleUpdate {
    pub title: String,
    /// Another article of the owner already at this URL is a conflict.
    pub url: String,
    #[serde(default)]
    pub author: Option<String>,
    pub date: String,
}

/// An article opened for reading.
#[derive(Serialize, ToSchema)]
pub(crate) struct Article {
    /// Equal to the article's moment id.
    pub id: Uuid,
    pub title: String,
    /// Markdown body; its embedded files are `/api/media/<id>` addresses listed in `media`.
    pub content: String,
    pub url: String,
    #[schema(required = true)]
    pub author: Option<String>,
    /// The URL's host without `www.`.
    pub site: String,
    /// Unix epoch milliseconds of the publish date.
    pub published_at: i64,
    /// The publish date as an edit takes it back: `YYYY-MM-DD` when only a day was given,
    /// otherwise RFC 3339 at the deployment's offset.
    pub date: String,
    /// Unix epoch milliseconds of the import, the article's place on the timeline.
    pub occurred_at: i64,
    pub media: Vec<ArticleMedia>,
}

/// One embedded file; the type tells an image from a video.
#[derive(Serialize, ToSchema)]
pub(crate) struct ArticleMedia {
    pub id: Uuid,
    pub content_type: String,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ArticleListQuery {
    /// Case-insensitive literal substring of the title, author or body; blank means no search.
    pub q: Option<String>,
    /// `next_cursor` from the previous page, listed with the same `near`.
    pub cursor: Option<String>,
    /// A page address: articles on the same host (ignoring `www.` and case) come first, those
    /// sharing more leading path segments before others, then latest import first. Combines
    /// with `q`, which only filters.
    pub near: Option<String>,
}

/// One page of the 摘星 article list.
#[derive(Serialize, ToSchema)]
pub(crate) struct ArticlePage {
    /// Articles, latest publish date first, or nearest `near` first.
    pub items: Vec<ArticleListItem>,
    /// Opaque token for the next page; null on the last page.
    #[schema(required = true)]
    pub next_cursor: Option<String>,
    /// Number of articles the owner has, regardless of `q`.
    pub total: i64,
}

/// One article as the list shows it.
#[derive(Serialize, ToSchema)]
pub(crate) struct ArticleListItem {
    pub id: Uuid,
    pub title: String,
    #[schema(required = true)]
    pub author: Option<String>,
    /// The original address, which the extension shows to tell candidates apart.
    pub url: String,
    /// The URL's host without `www.`, shown where there is no author.
    pub site: String,
    /// Unix epoch milliseconds of the publish date.
    pub published_at: i64,
    /// The publish date as written back by the server; see Article.date.
    pub date: String,
}

impl Article {
    /// Spells the publish date in the deployment's zone, the zone a bare day was read in.
    pub(crate) fn new(value: palace_db::Article, timezone: Tz) -> Self {
        let published = value.metadata.published();
        Self {
            id: value.id,
            title: value.metadata.title().to_owned(),
            content: value.content,
            url: value.metadata.url().to_owned(),
            author: value.metadata.author().map(str::to_owned),
            site: article_site(value.metadata.url()),
            published_at: published.at,
            date: published.to_text(timezone),
            occurred_at: value.occurred_at,
            media: value
                .media
                .into_iter()
                .map(|media| ArticleMedia {
                    id: media.id,
                    content_type: media.content_type,
                })
                .collect(),
        }
    }
}

impl ArticleListItem {
    /// Spells the publish date in the deployment's zone, as the detail does.
    fn new(id: Uuid, metadata: &ArticleMetadata, timezone: Tz) -> Self {
        let published = metadata.published();
        Self {
            id,
            title: metadata.title().to_owned(),
            author: metadata.author().map(str::to_owned),
            url: metadata.url().to_owned(),
            site: article_site(metadata.url()),
            published_at: published.at,
            date: published.to_text(timezone),
        }
    }
}

impl ArticlePage {
    /// Encodes the cursor as the opaque token clients send back unchanged.
    pub(crate) fn new(value: palace_db::ArticlePage, timezone: Tz) -> Self {
        Self {
            items: value
                .items
                .iter()
                .map(|item| ArticleListItem::new(item.id, &item.metadata, timezone))
                .collect(),
            next_cursor: value.next_cursor.map(|cursor| cursor.to_string()),
            total: value.total,
        }
    }
}
