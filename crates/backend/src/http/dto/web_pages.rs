use super::{Annotation, AnnotationStyle};
use palace_domain::WebPageState;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// A new annotation on a web page; the page is created with its first annotation.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WebAnnotationCreate {
    /// The page address without its fragment: absolute http(s), at most 2048 bytes, never
    /// normalized further.
    pub url: String,
    /// The tab title; blank means the address, longer than 1024 bytes is cut.
    pub title: String,
    /// The selected text; nonblank, at most 16 KiB.
    pub quote: String,
    /// Up to 32 UTF-16 code units of page text just before the quote; empty at the start.
    pub prefix: String,
    /// Up to 32 UTF-16 code units of page text just after the quote; empty at the end.
    pub suffix: String,
    /// Where the quote starts, as a fraction of the page text in [0, 1].
    pub position: f64,
    #[serde(default)]
    pub style: Option<AnnotationStyle>,
    /// Plain text, nonblank, at most 16 KiB. A style, a note or both are required.
    #[serde(default)]
    pub note: Option<String>,
}

/// The parts an edit replaces together, and the tab title to record on the page.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WebAnnotationUpdate {
    /// The tab title; blank means the page address.
    pub title: String,
    #[serde(default)]
    pub style: Option<AnnotationStyle>,
    #[serde(default)]
    pub note: Option<String>,
}

/// Binds the page at `url` to an article, creating the page when it has no annotations.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WebPageBinding {
    pub url: String,
    /// The tab title; blank means the address.
    pub title: String,
    /// An article of the owner.
    pub article_id: Uuid,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct WebPageLookup {
    /// The page address without its fragment, exactly as it was annotated.
    pub url: String,
}

/// An annotation the extension left on a web page.
#[derive(Serialize, ToSchema)]
pub(crate) struct WebAnnotation {
    pub id: Uuid,
    pub page_id: Uuid,
    pub quote: String,
    pub prefix: String,
    pub suffix: String,
    pub position: f64,
    #[schema(required = true)]
    pub style: Option<AnnotationStyle>,
    #[schema(required = true)]
    pub note: Option<String>,
    /// Unix epoch milliseconds.
    pub created_at: i64,
    pub updated_at: i64,
}

/// A web page the reader annotated or bound.
#[derive(Serialize, ToSchema)]
pub(crate) struct WebPage {
    pub id: Uuid,
    pub url: String,
    pub title: String,
    /// The article the page is bound to; null while the page takes annotations of its own.
    #[schema(required = true)]
    pub article_id: Option<Uuid>,
    /// Unix epoch milliseconds of the last annotation, edit or binding.
    pub updated_at: i64,
}

/// One row of the extension's list.
#[derive(Serialize, ToSchema)]
pub(crate) struct WebPageListItem {
    pub id: Uuid,
    pub url: String,
    pub title: String,
    #[schema(required = true)]
    pub article_id: Option<Uuid>,
    pub updated_at: i64,
    /// Annotations still on the page: all of them while unbound, the unmatched ones after.
    pub annotation_count: i64,
}

/// Every web page of the owner, most recently changed first; not paged.
#[derive(Serialize, ToSchema)]
pub(crate) struct WebPageList {
    pub items: Vec<WebPageListItem>,
}

/// A web page with the annotations still on it, oldest first.
#[derive(Serialize, ToSchema)]
pub(crate) struct WebPageDetail {
    pub page: WebPage,
    pub annotations: Vec<WebAnnotation>,
}

/// The web annotations left on every page bound to an article, oldest first.
#[derive(Serialize, ToSchema)]
pub(crate) struct WebAnnotationList {
    pub items: Vec<WebAnnotation>,
}

/// Where the article page found each web annotation in the rendered body.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WebAnnotationMigration {
    pub items: Vec<WebAnnotationMigrationItem>,
}

/// One web annotation and its article anchor.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WebAnnotationMigrationItem {
    /// A web annotation on a page bound to this article; one already gone is skipped.
    pub web_annotation_id: Uuid,
    /// UTF-16 code units into the rendered body text, inclusive.
    pub start: i64,
    /// Exclusive; after `start`.
    pub end: i64,
    /// The rendered text in `[start, end)`; exactly `end - start` UTF-16 units.
    pub quote: String,
}

/// The article annotations a migration created; skipped items create none.
#[derive(Serialize, ToSchema)]
pub(crate) struct MigratedAnnotations {
    pub items: Vec<Annotation>,
}

impl From<palace_db::WebPage> for WebPage {
    fn from(value: palace_db::WebPage) -> Self {
        Self {
            id: value.id,
            url: value.url,
            title: value.title,
            article_id: match value.state {
                WebPageState::Unbound => None,
                WebPageState::Bound(article) => Some(article),
            },
            updated_at: value.updated_at,
        }
    }
}

impl From<palace_db::WebAnnotation> for WebAnnotation {
    fn from(value: palace_db::WebAnnotation) -> Self {
        Self {
            id: value.id,
            page_id: value.page_id,
            quote: value.anchor.quote().to_owned(),
            prefix: value.anchor.prefix().to_owned(),
            suffix: value.anchor.suffix().to_owned(),
            position: value.anchor.position(),
            style: value.content.style().map(AnnotationStyle::from),
            note: value.content.note().map(str::to_owned),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<Vec<palace_db::WebPageSummary>> for WebPageList {
    fn from(value: Vec<palace_db::WebPageSummary>) -> Self {
        Self {
            items: value
                .into_iter()
                .map(|summary| {
                    let page = WebPage::from(summary.page);
                    WebPageListItem {
                        id: page.id,
                        url: page.url,
                        title: page.title,
                        article_id: page.article_id,
                        updated_at: page.updated_at,
                        annotation_count: summary.annotation_count,
                    }
                })
                .collect(),
        }
    }
}

impl From<palace_db::WebPageDetail> for WebPageDetail {
    fn from(value: palace_db::WebPageDetail) -> Self {
        Self {
            page: value.page.into(),
            annotations: value.annotations.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<Vec<palace_db::WebAnnotation>> for WebAnnotationList {
    fn from(value: Vec<palace_db::WebAnnotation>) -> Self {
        Self {
            items: value.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<Vec<palace_db::Annotation>> for MigratedAnnotations {
    fn from(value: Vec<palace_db::Annotation>) -> Self {
        Self {
            items: value.into_iter().map(Into::into).collect(),
        }
    }
}
