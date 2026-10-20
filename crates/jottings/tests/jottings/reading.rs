//! Live reads for the web 回响 page against real Git and real PostgreSQL.
use super::{Repository, SHANGHAI, database, indexed, run};
use palace_db::{
    DayCount, JottingListRequest, JottingOverview, ListedJotting, SearchTerm, TagCount,
};
use palace_domain::{InputError, InputErrorKind, Jotting};
use palace_jottings::{Jottings, JottingsError, Recall};
use pretty_assertions::assert_eq;
use sha2::{Digest, Sha256};
use testcontainers::{ContainerAsync, GenericImage};
use time::{OffsetDateTime, format_description::well_known::Rfc3339, macros::date};
use uuid::Uuid;

/// One jotting as both its file and what the reads return for it.
struct Fixture {
    path: String,
    text: String,
    listed: ListedJotting,
}

/// A jotting file at `path` with the given frontmatter, and its expected listing.
fn fixture(path: &str, created: &str, title: Option<&str>, tags: &[&str], body: &str) -> Fixture {
    let id = Uuid::now_v7();
    let mut text = format!("---\nid: {id}\ncreated: {created}\n");
    if let Some(title) = title {
        text.push_str(&format!("title: {title}\n"));
    }
    if !tags.is_empty() {
        text.push_str(&format!("tags: [{}]\n", tags.join(", ")));
    }
    text.push_str(&format!("---\n\n{body}"));
    Fixture {
        path: path.to_owned(),
        text,
        listed: ListedJotting {
            jotting: Jotting {
                id,
                created_at: millis(created),
                title: title.map(str::to_owned),
                tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
                body: body.to_owned(),
            },
            pending: path.starts_with("inbox/"),
        },
    }
}

/// Commits the fixtures and returns an index that matches the commit, with its database.
async fn indexed_jottings(
    repository: &Repository,
    fixtures: &[&Fixture],
) -> (ContainerAsync<GenericImage>, Jottings) {
    let files: Vec<(&str, &str)> = fixtures
        .iter()
        .map(|fixture| (fixture.path.as_str(), fixture.text.as_str()))
        .collect();
    repository.write(&files);
    repository.commit("add");
    let (container, database, _pool) = database().await;
    let jottings = Jottings::open(&repository.folder(), database, SHANGHAI).unwrap();
    indexed(fixtures.len())(jottings.refresh().await.unwrap());
    (container, jottings)
}

fn request(tag: Option<&str>, q: Option<&str>, limit: u32) -> JottingListRequest {
    JottingListRequest {
        tag: tag.map(str::to_owned),
        search: q.and_then(SearchTerm::parse),
        after: None,
        limit,
    }
}

/// Follows `next_cursor` to the end, returning every page's jottings in order and each total.
async fn every_page(
    jottings: &Jottings,
    mut request: JottingListRequest,
) -> (Vec<ListedJotting>, Vec<i64>) {
    let (mut all, mut totals) = (vec![], vec![]);
    loop {
        let page = jottings.list(&request).await.unwrap();
        all.extend(page.jottings);
        totals.push(page.total);
        match page.next_cursor {
            Some(cursor) => request.after = Some(cursor),
            None => return (all, totals),
        }
    }
}

/// Epoch milliseconds of an RFC 3339 time.
fn millis(time: &str) -> i64 {
    i64::try_from(
        OffsetDateTime::parse(time, &Rfc3339)
            .unwrap()
            .unix_timestamp_nanos()
            / 1_000_000,
    )
    .unwrap()
}

/// A jotting uploaded from a client, as the list shows it once written to the inbox.
fn uploaded(created: &str, body: &str) -> ListedJotting {
    ListedJotting {
        jotting: Jotting {
            id: Uuid::now_v7(),
            created_at: millis(created),
            title: None,
            tags: vec![],
            body: body.into(),
        },
        pending: true,
    }
}

/// Pages are disjoint and ordered by time then id across equal times; jottings written or
/// deleted while paging neither repeat nor shift the rest, and tombstones never show.
/// Core test case:
/// - `specs/test-cases/web/echo/jotting-reading.md#jotting-list-must-page-every-live-jotting-exactly-once-newest-first`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn the_list_pages_every_live_jotting_once() {
    run(async {
        let repository = Repository::new();
        let same: Vec<Fixture> = (0..4)
            .map(|n| {
                fixture(
                    &format!("same-{n}.md"),
                    "2026-10-18T09:00:00+08:00",
                    None,
                    &[],
                    "same",
                )
            })
            .collect();
        let older = fixture("older.md", "2026-10-01T09:00:00+08:00", None, &[], "older");
        let gone = fixture("gone.md", "2026-09-01T09:00:00+08:00", None, &[], "gone");
        let newest = fixture(
            "newest.md",
            "2026-10-19T09:00:00+08:00",
            None,
            &[],
            "newest",
        );
        let mut fixtures: Vec<&Fixture> = same.iter().collect();
        fixtures.extend([&older, &gone, &newest]);
        let (_container, jottings) = indexed_jottings(&repository, &fixtures).await;

        let first = jottings.list(&request(None, None, 3)).await.unwrap();
        // Written and deleted between two pages: the new one is newer than the cursor, the
        // deleted one was not read yet.
        let late = uploaded("2026-10-20T09:00:00+08:00", "late");
        jottings.upload(late.jotting.clone()).await.unwrap();
        repository.remove("gone.md");
        // The commit also takes in the upload, still under inbox/.
        repository.commit("delete");
        indexed(2)(jottings.refresh().await.unwrap());
        let (rest, _) = every_page(
            &jottings,
            JottingListRequest {
                after: first.next_cursor,
                ..request(None, None, 3)
            },
        )
        .await;

        let mut same_ids: Vec<&ListedJotting> = same.iter().map(|f| &f.listed).collect();
        same_ids.sort_by_key(|listed| std::cmp::Reverse(listed.jotting.id));
        let mut expected = vec![newest.listed.clone()];
        expected.extend(same_ids.into_iter().cloned());
        expected.push(older.listed.clone());
        let mut seen = first.jottings.clone();
        seen.extend(rest);
        assert_eq!(seen, expected);
        assert_eq!(first.total, 7);

        let (now, totals) = every_page(&jottings, request(None, None, 200)).await;
        assert_eq!(totals, [7]);
        assert!(
            now.iter()
                .all(|listed| listed.jotting.id != gone.listed.jotting.id)
        );
        assert_eq!(now[0], late);
    });
}

/// A tag matches exactly; a term matches title, body or any tag regardless of case; together
/// they intersect, and the total always equals what paging returns.
/// Core test case:
/// - `specs/test-cases/web/echo/jotting-reading.md#tag-and-query-filters-must-intersect-and-report-a-matching-total`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn tag_and_query_filters_intersect() {
    run(async {
        let repository = Repository::new();
        let in_title = fixture(
            "t.md",
            "2026-10-05T09:00:00+08:00",
            Some("Rust 笔记"),
            &["阅读"],
            "x",
        );
        let in_body = fixture(
            "b.md",
            "2026-10-04T09:00:00+08:00",
            None,
            &["读"],
            "learning RUST",
        );
        let in_tag = fixture(
            "g.md",
            "2026-10-03T09:00:00+08:00",
            None,
            &["rustacean"],
            "y",
        );
        let neither = fixture("n.md", "2026-10-02T09:00:00+08:00", None, &["阅读"], "z");
        let (_container, jottings) =
            indexed_jottings(&repository, &[&in_title, &in_body, &in_tag, &neither]).await;

        let ids = |listed: Vec<ListedJotting>| -> Vec<Uuid> {
            listed.into_iter().map(|listed| listed.jotting.id).collect()
        };
        let id = |fixture: &Fixture| fixture.listed.jotting.id;
        for (tag, q, expected) in [
            (Some("读"), None, vec![id(&in_body)]),
            (Some("阅读"), None, vec![id(&in_title), id(&neither)]),
            (
                None,
                Some("rust"),
                vec![id(&in_title), id(&in_body), id(&in_tag)],
            ),
            (Some("阅读"), Some("rust"), vec![id(&in_title)]),
            (Some("阅读"), Some("100%"), vec![]),
        ] {
            let (listed, totals) = every_page(&jottings, request(tag, q, 1)).await;
            let count = i64::try_from(listed.len()).unwrap();
            assert!(totals.iter().all(|total| *total == count));
            assert_eq!(ids(listed), expected);
        }
    });
}

/// Pending follows the file's place, not its commit: committed but unsorted is pending, moved
/// out but uncommitted is not, and a folder merely named like inbox is not.
/// Core test case:
/// - `specs/test-cases/web/echo/jotting-reading.md#pending-must-mean-the-jotting-file-sits-under-inbox`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn pending_means_the_file_sits_under_inbox() {
    run(async {
        let repository = Repository::new();
        let committed = fixture("inbox/c.md", "2026-10-04T09:00:00+08:00", None, &[], "c");
        let sorted = fixture("2026/s.md", "2026-10-03T09:00:00+08:00", None, &[], "s");
        let lookalike = fixture("inboxes/l.md", "2026-10-02T09:00:00+08:00", None, &[], "l");
        let (_container, jottings) =
            indexed_jottings(&repository, &[&committed, &sorted, &lookalike]).await;
        let upload = uploaded("2026-10-05T09:00:00+08:00", "u");
        jottings.upload(upload.jotting.clone()).await.unwrap();

        let page = jottings.list(&request(None, None, 60)).await.unwrap();
        let pending: Vec<(Uuid, bool)> = page
            .jottings
            .iter()
            .map(|listed| (listed.jotting.id, listed.pending))
            .collect();
        assert_eq!(
            pending,
            [
                (upload.jotting.id, true),
                (committed.listed.jotting.id, true),
                (sorted.listed.jotting.id, false),
                (lookalike.listed.jotting.id, false),
            ]
        );
    });
}

/// Days are Shanghai days even across UTC midnight; the range is contiguous with empty days;
/// counts leave tombstones out; tags order by count, then name.
/// Core test case:
/// - `specs/test-cases/web/echo/jotting-reading.md#overview-must-bucket-jottings-by-the-deployments-local-day`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn overview_buckets_by_the_local_day() {
    run(async {
        let repository = Repository::new();
        // 23:59 and 00:01 Shanghai are both 15:59 and 16:01 UTC of the same UTC day.
        let late = fixture("a.md", "2026-10-17T23:59:00+08:00", None, &["b", "a"], "x");
        let early = fixture("b.md", "2026-10-18T00:01:00+08:00", None, &["a"], "y");
        let other = fixture("c.md", "2026-10-20T08:00:00+08:00", None, &["c", "b"], "z");
        let gone = fixture(
            "d.md",
            "2026-10-19T08:00:00+08:00",
            None,
            &["c", "gone"],
            "w",
        );
        let (_container, jottings) =
            indexed_jottings(&repository, &[&late, &early, &other, &gone]).await;
        repository.remove("d.md");
        repository.commit("delete");
        indexed(1)(jottings.refresh().await.unwrap());

        let now = millis("2026-10-20T09:00:00+08:00");
        let tag = |name: &str, count| TagCount {
            name: name.into(),
            count,
        };
        let day = |date, count| DayCount { date, count };
        assert_eq!(
            jottings.overview(date!(2026 - 10 - 16), now).await.unwrap(),
            JottingOverview {
                total: 3,
                days: 3,
                tags: vec![tag("a", 2), tag("b", 2), tag("c", 1)],
                daily: vec![
                    day(date!(2026 - 10 - 16), 0),
                    day(date!(2026 - 10 - 17), 1),
                    day(date!(2026 - 10 - 18), 1),
                    day(date!(2026 - 10 - 19), 0),
                    day(date!(2026 - 10 - 20), 1),
                ],
            }
        );
        for from in [date!(2026 - 10 - 21), date!(2025 - 10 - 19)] {
            assert!(matches!(
                jottings.overview(from, now).await,
                Err(JottingsError::Input(error)) if error == InputError::new(
                    InputErrorKind::Field,
                    "from",
                    "expected a day within the last year",
                )
            ));
        }
    });
}

/// The pile is the first five candidates by SHA-256 of the local day and id; it holds all day,
/// ignores jottings created today, and moves on at local midnight.
/// Core test case:
/// - `specs/test-cases/web/echo/jotting-reading.md#recall-must-return-the-same-pile-all-day-and-exclude-todays-jottings`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn recall_holds_for_the_day_and_skips_today() {
    run(async {
        let repository = Repository::new();
        let candidates: Vec<Fixture> = (0..8)
            .map(|n| {
                fixture(
                    &format!("{n}.md"),
                    &format!("2026-10-{:02}T09:00:00+08:00", n + 1),
                    None,
                    &[],
                    "old",
                )
            })
            .collect();
        // Created at 00:30 Shanghai on the 20th, still the 19th in UTC.
        let today = fixture("today.md", "2026-10-20T00:30:00+08:00", None, &[], "new");
        let mut fixtures: Vec<&Fixture> = candidates.iter().collect();
        fixtures.push(&today);
        let (_container, jottings) = indexed_jottings(&repository, &fixtures).await;

        let pile = |day: &str, pool: &[ListedJotting]| -> Vec<ListedJotting> {
            let mut pool = pool.to_vec();
            pool.sort_by_key(|listed| {
                Sha256::digest(format!("{day}{}", listed.jotting.id)).to_vec()
            });
            pool.truncate(5);
            pool
        };
        let mut pool: Vec<ListedJotting> = candidates.iter().map(|f| f.listed.clone()).collect();
        let morning = jottings
            .recall(millis("2026-10-20T00:45:00+08:00"))
            .await
            .unwrap();
        assert_eq!(
            morning,
            Recall {
                date: date!(2026 - 10 - 20),
                jottings: pile("2026-10-20", &pool),
            }
        );
        let another = uploaded("2026-10-20T10:00:00+08:00", "another");
        jottings.upload(another.jotting.clone()).await.unwrap();
        let evening = jottings
            .recall(millis("2026-10-20T23:59:00+08:00"))
            .await
            .unwrap();
        assert_eq!(evening, morning);

        pool.extend([today.listed.clone(), another]);
        assert_eq!(
            jottings
                .recall(millis("2026-10-21T00:00:00+08:00"))
                .await
                .unwrap(),
            Recall {
                date: date!(2026 - 10 - 21),
                jottings: pile("2026-10-21", &pool),
            }
        );

        // Deleted jottings drop out, and fewer than five candidates all show.
        for n in 0..7 {
            repository.remove(&format!("{n}.md"));
        }
        // The upload is committed along with the deletions.
        repository.commit("delete most");
        indexed(8)(jottings.refresh().await.unwrap());
        let gone: Vec<&ListedJotting> = candidates[..7].iter().map(|f| &f.listed).collect();
        let kept: Vec<ListedJotting> = pool
            .into_iter()
            .filter(|listed| !gone.contains(&listed))
            .collect();
        assert_eq!(kept.len(), 3);
        assert_eq!(
            jottings
                .recall(millis("2026-10-21T08:00:00+08:00"))
                .await
                .unwrap(),
            Recall {
                date: date!(2026 - 10 - 21),
                jottings: pile("2026-10-21", &kept),
            }
        );
    });
}
