use crate::{Database, DbError, OwnerScope};
use palace_domain::{
    EXCERPT_SOURCE_CHARS, InputError, InputErrorKind, Role, Source, article_site, excerpt,
};
use serde::Serialize;
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

/// The widest range one timeline request may cover; a local day is at most 25 hours.
const MAX_RANGE_MS: i64 = 48 * 60 * 60 * 1000;

/// One entry of a day's timeline: identity and position from `moment`, card fields from its kind.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Moment {
    pub id: Uuid,
    pub occurred_at: i64,
    pub detail: MomentDetail,
}
/// Card fields per kind, so a moment can never carry another kind's fields.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub enum MomentDetail {
    Conversation(ConversationCard),
    Article(ArticleCard),
    Sleep(SleepCard),
}
/// A sleep moment is one finished night; it occurred when the sleeper woke.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SleepCard {
    pub started_at: i64,
}
/// A conversation moment is one path: its own title and messages, the tree's source.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ConversationCard {
    pub conversation_id: Uuid,
    pub title: String,
    pub source: Source,
    pub message_count: i64,
    pub excerpt: Vec<ExcerptLine>,
}
/// An article moment is shown by its title and who wrote it, or where it came from.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ArticleCard {
    pub title: String,
    pub author: Option<String>,
    /// The URL's host without `www.`, shown where the article names no author.
    pub site: String,
}
/// The plain-text opening of one message on the path.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ExcerptLine {
    pub role: Role,
    pub text: String,
}
/// One entry of a day's outline: enough to draw a placeholder of the right shape before the
/// cards arrive, and to match it to its card by identity.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct MomentOutline {
    pub id: Uuid,
    pub kind: MomentKind,
}
/// The closed set of moment kinds, mirroring the `moment.kind` check constraint.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum MomentKind {
    Conversation,
    Article,
    Sleep,
}

impl Database {
    /// Lists the moments in `[start, end)` epoch milliseconds, ordered by time then id.
    ///
    /// The caller computes the range from its own time zone, so the server never fixes which
    /// local day a moment belongs to.
    pub async fn timeline(
        &self,
        owner: OwnerScope,
        start: i64,
        end: i64,
    ) -> Result<Vec<Moment>, DbError> {
        let mut tx = self.pool.begin().await?;
        // Cards and moments must come from the same snapshot, or a concurrent delete could leave
        // a moment without its card.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
        let moments = day_moments(&mut *tx, owner, start, end).await?;
        let mut conversations = Vec::new();
        let mut articles = Vec::new();
        let mut nights = Vec::new();
        for (outline, _) in &moments {
            match outline.kind {
                MomentKind::Conversation => conversations.push(outline.id),
                MomentKind::Article => articles.push(outline.id),
                MomentKind::Sleep => nights.push(outline.id),
            }
        }
        let mut cards = conversation_cards(&mut tx, owner, &conversations).await?;
        let mut article_cards = article_cards(&mut tx, owner, &articles).await?;
        let mut sleep_cards = sleep_cards(&mut tx, owner, &nights).await?;
        tx.commit().await?;
        moments
            .into_iter()
            .map(|(outline, occurred_at)| {
                let detail = match outline.kind {
                    MomentKind::Conversation => MomentDetail::Conversation(
                        cards.remove(&outline.id).ok_or(DbError::Conflict)?,
                    ),
                    MomentKind::Article => MomentDetail::Article(
                        article_cards.remove(&outline.id).ok_or(DbError::Conflict)?,
                    ),
                    MomentKind::Sleep => MomentDetail::Sleep(
                        sleep_cards.remove(&outline.id).ok_or(DbError::Conflict)?,
                    ),
                };
                Ok(Moment {
                    id: outline.id,
                    occurred_at,
                    detail,
                })
            })
            .collect()
    }

    /// Lists the identity and kind of the moments `timeline` would return for the same range, in
    /// the same order, without reading any details.
    pub async fn day_outline(
        &self,
        owner: OwnerScope,
        start: i64,
        end: i64,
    ) -> Result<Vec<MomentOutline>, DbError> {
        Ok(day_moments(&self.pool, owner, start, end)
            .await?
            .into_iter()
            .map(|(outline, _)| outline)
            .collect())
    }
}

/// Scans one range of `moment` in timeline order. Both the timeline and its outline read through
/// here, so they cannot disagree on validation, range bounds or order.
async fn day_moments<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    owner: OwnerScope,
    start: i64,
    end: i64,
) -> Result<Vec<(MomentOutline, i64)>, DbError> {
    if start >= end || end - start > MAX_RANGE_MS {
        return Err(InputError::new(
            InputErrorKind::Field,
            "end",
            "expected start < end within 48 hours",
        )
        .into());
    }
    let rows = sqlx::query("SELECT id,kind,(extract(epoch FROM occurred_at)*1000)::bigint AS occurred_at FROM moment WHERE owner_id=$1 AND occurred_at>=to_timestamp($2::double precision/1000.0) AND occurred_at<to_timestamp($3::double precision/1000.0) ORDER BY occurred_at,id")
        .bind(owner.id()).bind(start).bind(end).fetch_all(executor).await?;
    rows.into_iter()
        .map(|row| {
            let kind = match row.try_get::<String, _>("kind")?.as_str() {
                "conversation" => MomentKind::Conversation,
                "article" => MomentKind::Article,
                "sleep" => MomentKind::Sleep,
                // The database check constraint admits no other kind.
                _ => return Err(DbError::Conflict),
            };
            Ok((
                MomentOutline {
                    id: row.try_get("id")?,
                    kind,
                },
                row.try_get("occurred_at")?,
            ))
        })
        .collect()
}

/// Reads the card fields of many conversation moments with a fixed number of queries.
async fn conversation_cards(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: OwnerScope,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, ConversationCard>, DbError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    // Walk each path from its head to the root carrying only ids, then read the head of the
    // two messages nearest the root; neither a long path nor a long answer loads its whole text.
    // `left` counts characters, so the cut never splits one.
    let rows = sqlx::query("WITH RECURSIVE chain(path_id,id,parent_message_id,depth) AS (SELECT p.id,m.id,m.parent_message_id,0 FROM conversation_path p JOIN message m ON m.owner_id=p.owner_id AND m.id=p.head_message_id WHERE p.owner_id=$1 AND p.id=ANY($2) UNION ALL SELECT c.path_id,m.id,m.parent_message_id,c.depth+1 FROM chain c JOIN message m ON m.owner_id=$1 AND m.id=c.parent_message_id), ranked AS (SELECT path_id,id,row_number() OVER (PARTITION BY path_id ORDER BY depth DESC) AS position FROM chain) SELECT r.path_id,m.role,left(m.content,$3) AS content FROM ranked r JOIN message m ON m.owner_id=$1 AND m.id=r.id WHERE r.position<=2 ORDER BY r.path_id,r.position")
        .bind(owner.id()).bind(ids).bind(EXCERPT_SOURCE_CHARS as i32).fetch_all(&mut **tx).await?;
    let mut excerpts: HashMap<Uuid, Vec<ExcerptLine>> = HashMap::new();
    for row in rows {
        let role: Role = serde_json::from_value(serde_json::Value::String(row.try_get("role")?))
            .map_err(|_| DbError::Conflict)?;
        excerpts
            .entry(row.try_get("path_id")?)
            .or_default()
            .push(ExcerptLine {
                role,
                text: excerpt(row.try_get("content")?),
            });
    }
    let rows = sqlx::query("SELECT p.id,p.conversation_id,p.title,c.source,p.message_count FROM conversation_path p JOIN conversation c ON c.owner_id=p.owner_id AND c.id=p.conversation_id WHERE p.owner_id=$1 AND p.id=ANY($2)")
        .bind(owner.id()).bind(ids).fetch_all(&mut **tx).await?;
    rows.into_iter()
        .map(|row| {
            let id: Uuid = row.try_get("id")?;
            let source: Source =
                serde_json::from_value(serde_json::Value::String(row.try_get("source")?))
                    .map_err(|_| DbError::Conflict)?;
            Ok((
                id,
                ConversationCard {
                    conversation_id: row.try_get("conversation_id")?,
                    title: row.try_get("title")?,
                    source,
                    message_count: row.try_get("message_count")?,
                    excerpt: excerpts.remove(&id).unwrap_or_default(),
                },
            ))
        })
        .collect()
}

/// Reads the card fields of many article moments in one query.
async fn article_cards(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: OwnerScope,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, ArticleCard>, DbError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows =
        sqlx::query("SELECT id,title,author,url FROM article WHERE owner_id=$1 AND id=ANY($2)")
            .bind(owner.id())
            .bind(ids)
            .fetch_all(&mut **tx)
            .await?;
    rows.into_iter()
        .map(|row| {
            Ok((
                row.try_get("id")?,
                ArticleCard {
                    title: row.try_get("title")?,
                    author: row.try_get("author")?,
                    site: article_site(row.try_get("url")?),
                },
            ))
        })
        .collect()
}

/// Reads the bedtimes of many sleep moments in one query.
async fn sleep_cards(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: OwnerScope,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, SleepCard>, DbError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query("SELECT id,(extract(epoch FROM lower(span))*1000)::bigint AS started_at FROM sleep WHERE owner_id=$1 AND id=ANY($2)")
        .bind(owner.id())
        .bind(ids)
        .fetch_all(&mut **tx)
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok((
                row.try_get("id")?,
                SleepCard {
                    started_at: row.try_get("started_at")?,
                },
            ))
        })
        .collect()
}
