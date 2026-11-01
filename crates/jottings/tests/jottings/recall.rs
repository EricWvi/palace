//! 今日回响 against real Git and real PostgreSQL: which jottings a day's pile draws, how a pile
//! holds for the day, and how completions count.
use super::{Repository, SHANGHAI, changes, database, indexed, run};
use crate::reading::{Fixture, fixture, millis, stamp};
use palace_db::{Database, DbError, RecallCard, RecallNext, RecallState};
use palace_jottings::{Jottings, JottingsError, Recall, RecallSize};
use pretty_assertions::assert_eq;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::{collections::BTreeMap, sync::Arc};
use testcontainers::{ContainerAsync, GenericImage};
use time::macros::date;
use uuid::Uuid;

/// Commits jottings created on October 1st, 2nd, … with bodies `c0`, `c1`, …, and indexes them.
async fn library(
    repository: &Repository,
    count: usize,
) -> (
    ContainerAsync<GenericImage>,
    Database,
    PgPool,
    Jottings,
    Vec<Fixture>,
) {
    let mut fixtures: Vec<Fixture> = (0..count)
        .map(|n| {
            fixture(
                &format!("c{n}.md"),
                &format!("2026-10-{:02}T09:00:00+08:00", n + 1),
                None,
                &[],
                &format!("c{n}"),
            )
        })
        .collect();
    let files: Vec<(&str, &str)> = fixtures
        .iter()
        .map(|fixture| (fixture.path.as_str(), fixture.text.as_str()))
        .collect();
    repository.write(&files);
    repository.commit("add");
    let (container, database, pool) = database().await;
    let jottings = Jottings::open(&repository.folder(), database.clone(), SHANGHAI).unwrap();
    indexed(count)(jottings.refresh().await.unwrap());
    stamp(&pool, &mut fixtures).await;
    (container, database, pool, jottings, fixtures)
}

/// Every live jotting's completion count; `None` when it is out of recall.
async fn counts(pool: &PgPool) -> BTreeMap<Uuid, Option<i32>> {
    sqlx::query_as::<_, (Uuid, Option<i32>)>("SELECT jotting_id,completions FROM jotting_recall")
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .collect()
}

/// The pile the rule draws on `day` from `candidates`: least completed first, ties by
/// SHA-256 of the day and id, computed here independently of the SQL.
fn drawn(
    day: &str,
    candidates: &[&Fixture],
    counts: &BTreeMap<Uuid, Option<i32>>,
    size: usize,
) -> Vec<Uuid> {
    let mut ids: Vec<Uuid> = candidates.iter().map(|f| f.listed.jotting.id).collect();
    ids.sort_by_key(|id| (counts[id], Sha256::digest(format!("{day}{id}")).to_vec()));
    ids.truncate(size);
    ids
}

/// The cards a pile of `ids` reads as, with the ones in `done` completed.
fn cards(fixtures: &[Fixture], ids: &[Uuid], done: &[Uuid]) -> Vec<RecallCard> {
    ids.iter()
        .map(|id| RecallCard {
            listed: fixtures
                .iter()
                .find(|f| f.listed.jotting.id == *id)
                .unwrap()
                .listed
                .clone(),
            done: done.contains(id),
        })
        .collect()
}

/// Waits until another session is blocked on a lock: the barrier that lets a test hold one
/// side of a race at the exact step under test.
pub(crate) async fn until_blocked(pool: &PgPool) {
    loop {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity WHERE wait_event_type='Lock'",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        if waiting > 0 {
            return;
        }
        tokio::task::yield_now().await;
    }
}

/// A day's pile draws the least completed jottings first and holds all day: completing,
/// writing, deleting and a new size leave it as drawn; the next day draws with the new size.
/// Core test case:
/// - `specs/test-cases/web/echo/jotting-reading.md#recall-must-draw-the-least-completed-jottings-once-a-day-and-keep-the-pile`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn recall_draws_least_completed_once_a_day() {
    run(async {
        let repository = Repository::new();
        let (_container, database, pool, jottings, fixtures) = library(&repository, 9).await;
        // Created at 00:30 Shanghai on the 20th, still the 19th in UTC.
        let mut today = fixture("today.md", "2026-10-20T00:30:00+08:00", None, &[], "new");
        repository.write(&[(today.path.as_str(), today.text.as_str())]);
        repository.commit("today");
        indexed(1)(jottings.refresh().await.unwrap());
        stamp(&pool, std::slice::from_mut(&mut today)).await;
        // c0–c3 were completed once before; c8 is out of recall.
        sqlx::query("UPDATE jotting_recall SET completions=1 WHERE jotting_id=ANY($1)")
            .bind(
                fixtures[..4]
                    .iter()
                    .map(|f| f.listed.jotting.id)
                    .collect::<Vec<_>>(),
            )
            .execute(&pool)
            .await
            .unwrap();
        let out = fixtures[8].listed.jotting.id;
        jottings
            .set_recall(out, RecallState::Excluded)
            .await
            .unwrap();

        let before = counts(&pool).await;
        let candidates: Vec<&Fixture> = fixtures[..8].iter().collect();
        let first = drawn("2026-10-20", &candidates, &before, 5);
        // The four never completed (c4–c7) come first, then one of those completed once.
        assert_eq!(
            first[..4]
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            fixtures[4..8].iter().map(|f| f.listed.jotting.id).collect()
        );
        let morning = jottings
            .recall(millis("2026-10-20T00:45:00+08:00"))
            .await
            .unwrap();
        assert_eq!(
            morning,
            Recall {
                date: date!(2026 - 10 - 20),
                cards: cards(&fixtures, &first, &[]),
            }
        );

        jottings
            .complete_recall(date!(2026 - 10 - 20), first[0], RecallNext::Keep)
            .await
            .unwrap();
        let mut later = fixture("later.md", "2026-10-20T10:00:00+08:00", None, &[], "later");
        repository.write(&[(later.path.as_str(), later.text.as_str())]);
        let deleted = fixtures
            .iter()
            .find(|f| f.listed.jotting.id == first[1])
            .unwrap();
        repository.remove(&deleted.path);
        repository.commit("write and delete");
        indexed(2)(jottings.refresh().await.unwrap());
        stamp(&pool, std::slice::from_mut(&mut later)).await;
        let resized = Jottings::open(&repository.folder(), database.clone(), SHANGHAI)
            .unwrap()
            .with_recall_size("3".parse::<RecallSize>().unwrap());
        let evening = resized
            .recall(millis("2026-10-20T23:59:00+08:00"))
            .await
            .unwrap();
        let mut kept = first.clone();
        kept.remove(1);
        assert_eq!(evening.cards, cards(&fixtures, &kept, &[first[0]]));

        // The next day draws three, the completed card now behind every other candidate, and
        // the unfinished ones of the 20th skipped.
        let next = resized
            .recall(millis("2026-10-21T08:00:00+08:00"))
            .await
            .unwrap();
        let after = counts(&pool).await;
        let skipped = &kept[1..];
        let candidates: Vec<&Fixture> = fixtures[..8]
            .iter()
            .chain([&today, &later])
            .filter(|f| {
                let id = f.listed.jotting.id;
                id != first[1] && !skipped.contains(&id)
            })
            .collect();
        let expected = drawn("2026-10-21", &candidates, &after, 3);
        let mut all = fixtures;
        all.extend([today, later]);
        assert_eq!(
            next,
            Recall {
                date: date!(2026 - 10 - 21),
                cards: cards(&all, &expected, &[]),
            }
        );
        assert!(!next.cards.iter().any(|card| card.listed.jotting.id == out));
    });
}

/// Two first requests of a day draw one pile: the second waits for the first and returns its
/// pile instead of drawing another.
/// Core test case:
/// - `specs/test-cases/web/echo/jotting-reading.md#recall-must-draw-the-least-completed-jottings-once-a-day-and-keep-the-pile`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn concurrent_first_requests_draw_one_pile() {
    run(async {
        let repository = Repository::new();
        let (_container, _database, pool, jottings, fixtures) = library(&repository, 6).await;
        let jottings = Arc::new(jottings);
        // The first request, held after it claimed the day and drew a single card.
        let mut first = pool.begin().await.unwrap();
        sqlx::query("INSERT INTO jotting_recall_pile(day) VALUES('2026-10-20')")
            .execute(&mut *first)
            .await
            .unwrap();
        let chosen = fixtures[3].listed.jotting.id;
        sqlx::query(
            "INSERT INTO jotting_recall_card(day,position,jotting_id) VALUES('2026-10-20',0,$1)",
        )
        .bind(chosen)
        .execute(&mut *first)
        .await
        .unwrap();
        let second = tokio::spawn({
            let jottings = jottings.clone();
            async move { jottings.recall(millis("2026-10-20T09:00:00+08:00")).await }
        });
        until_blocked(&pool).await;
        first.commit().await.unwrap();
        assert_eq!(
            second.await.unwrap().unwrap(),
            Recall {
                date: date!(2026 - 10 - 20),
                cards: cards(&fixtures, &[chosen], &[]),
            }
        );
        let piles: i64 = sqlx::query_scalar("SELECT count(*) FROM jotting_recall_card")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(piles, 1);
    });
}

/// Cards left unfinished are skipped by the very next pile only, even across a day without a
/// visit, and come back at the front after; only the latest two piles are kept.
/// Core test cases:
/// - `specs/test-cases/web/echo/jotting-reading.md#recall-must-skip-the-jottings-left-unfinished-in-the-previous-pile`
/// - `specs/test-cases/web/echo/jotting-reading.md#recall-must-keep-only-the-two-latest-piles`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn unfinished_cards_skip_the_next_pile_only() {
    run(async {
        let repository = Repository::new();
        let (_container, _database, pool, jottings, fixtures) = library(&repository, 6).await;
        let jottings = jottings.with_recall_size("3".parse::<RecallSize>().unwrap());
        let all: Vec<&Fixture> = fixtures.iter().collect();
        let ids = |cards: &[RecallCard]| -> Vec<Uuid> {
            cards.iter().map(|card| card.listed.jotting.id).collect()
        };

        let first = jottings
            .recall(millis("2026-10-20T09:00:00+08:00"))
            .await
            .unwrap();
        assert_eq!(
            ids(&first.cards),
            drawn("2026-10-20", &all, &counts(&pool).await, 3)
        );
        let read = first.cards[0].listed.jotting.id;
        let unfinished = ids(&first.cards[1..]);
        jottings
            .complete_recall(date!(2026 - 10 - 20), read, RecallNext::Keep)
            .await
            .unwrap();

        // Nobody opened 回响 on the 21st; the 22nd still skips the 20th's unfinished cards, so
        // only the three never drawn are left in the least layer.
        let third = jottings
            .recall(millis("2026-10-22T09:00:00+08:00"))
            .await
            .unwrap();
        let rest: Vec<&Fixture> = all
            .iter()
            .copied()
            .filter(|f| !ids(&first.cards).contains(&f.listed.jotting.id))
            .collect();
        assert_eq!(
            ids(&third.cards),
            drawn("2026-10-22", &rest, &counts(&pool).await, 3)
        );

        // The 23rd skips the 22nd's; the 20th's unfinished are back, ahead of the one read.
        let fourth = jottings
            .recall(millis("2026-10-23T09:00:00+08:00"))
            .await
            .unwrap();
        let back: Vec<&Fixture> = all
            .iter()
            .copied()
            .filter(|f| !ids(&third.cards).contains(&f.listed.jotting.id))
            .collect();
        let expected = drawn("2026-10-23", &back, &counts(&pool).await, 3);
        assert_eq!(ids(&fourth.cards), expected);
        assert_eq!(expected[2], read);
        assert_eq!(
            expected[..2]
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            unfinished.into_iter().collect()
        );

        let days: Vec<String> =
            sqlx::query_scalar("SELECT day::text FROM jotting_recall_pile ORDER BY day")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(days, ["2026-10-22", "2026-10-23"]);
    });
}

/// A completion counts once however often or concurrently it is sent; `retire` takes the
/// jotting out even when the card was done; only stored piles can be completed.
/// Core test case:
/// - `specs/test-cases/web/echo/jotting-reading.md#completing-a-recall-card-must-count-it-once-and-only-then`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn completing_a_card_counts_once() {
    run(async {
        let repository = Repository::new();
        let (_container, _database, pool, jottings, _fixtures) = library(&repository, 6).await;
        let jottings = Arc::new(jottings);
        let day = date!(2026 - 10 - 20);
        let pile = jottings
            .recall(millis("2026-10-20T09:00:00+08:00"))
            .await
            .unwrap();
        let card = pile.cards[0].listed.jotting.id;

        // Another completion of the same card, held after it marked the card done.
        let mut other = pool.begin().await.unwrap();
        sqlx::query("UPDATE jotting_recall_card SET done_at=date_trunc('milliseconds',now()) WHERE day='2026-10-20' AND jotting_id=$1")
            .bind(card)
            .execute(&mut *other)
            .await
            .unwrap();
        let racing = tokio::spawn({
            let jottings = jottings.clone();
            async move { jottings.complete_recall(day, card, RecallNext::Keep).await }
        });
        until_blocked(&pool).await;
        sqlx::query("UPDATE jotting_recall SET completions=completions+1 WHERE jotting_id=$1")
            .bind(card)
            .execute(&mut *other)
            .await
            .unwrap();
        other.commit().await.unwrap();
        assert_eq!(racing.await.unwrap().unwrap(), RecallState::Included);
        assert_eq!(counts(&pool).await[&card], Some(1));

        assert_eq!(
            jottings
                .complete_recall(day, card, RecallNext::Keep)
                .await
                .unwrap(),
            RecallState::Included
        );
        assert_eq!(counts(&pool).await[&card], Some(1));
        assert_eq!(
            jottings
                .complete_recall(day, card, RecallNext::Retire)
                .await
                .unwrap(),
            RecallState::Excluded
        );
        assert_eq!(counts(&pool).await[&card], None);
        let again = jottings
            .recall(millis("2026-10-20T20:00:00+08:00"))
            .await
            .unwrap();
        assert_eq!(
            (again.cards[0].done, again.cards[0].listed.recall),
            (true, RecallState::Excluded)
        );

        // After midnight the 20th's pile is the previous one and can still be completed.
        jottings
            .recall(millis("2026-10-21T00:10:00+08:00"))
            .await
            .unwrap();
        let late = pile.cards[1].listed.jotting.id;
        jottings
            .complete_recall(day, late, RecallNext::Keep)
            .await
            .unwrap();
        assert_eq!(counts(&pool).await[&late], Some(1));

        for (date, id) in [(day, Uuid::now_v7()), (date!(2026 - 10 - 19), late)] {
            assert!(matches!(
                jottings.complete_recall(date, id, RecallNext::Keep).await,
                Err(JottingsError::Database(DbError::NotFound))
            ));
        }
    });
}

/// New, revived and re-included jottings start at the least count; recall state never
/// publishes a version.
/// Core test case:
/// - `specs/test-cases/web/echo/jotting-reading.md#new-revived-and-re-included-jottings-must-start-at-the-least-completed-count`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn appearing_jottings_start_at_the_least_count() {
    run(async {
        let repository = Repository::new();
        let (_container, _database, pool, jottings, fixtures) = library(&repository, 3).await;
        let [a, b, c] = [0, 1, 2].map(|n| fixtures[n].listed.jotting.id);
        for (id, count) in [(a, 2), (b, 3), (c, 7)] {
            sqlx::query("UPDATE jotting_recall SET completions=$2 WHERE jotting_id=$1")
                .bind(id)
                .bind(count)
                .execute(&pool)
                .await
                .unwrap();
        }
        let published = changes(&jottings, 0).await;
        jottings.set_recall(b, RecallState::Excluded).await.unwrap();
        assert_eq!(changes(&jottings, 0).await, published);

        let d = fixture("d.md", "2026-10-09T09:00:00+08:00", None, &[], "d");
        repository.write(&[(d.path.as_str(), d.text.as_str())]);
        repository.remove(&fixtures[2].path);
        repository.commit("add d, delete c");
        indexed(2)(jottings.refresh().await.unwrap());
        assert_eq!(
            counts(&pool).await,
            BTreeMap::from([(a, Some(2)), (b, None), (d.listed.jotting.id, Some(2))])
        );
        // Moving d on leaves a alone at the least; c comes back there, not at its old 7.
        sqlx::query("UPDATE jotting_recall SET completions=4 WHERE jotting_id=$1")
            .bind(d.listed.jotting.id)
            .execute(&pool)
            .await
            .unwrap();
        repository.write(&[(fixtures[2].path.as_str(), fixtures[2].text.as_str())]);
        repository.commit("revive c");
        indexed(1)(jottings.refresh().await.unwrap());
        jottings.set_recall(b, RecallState::Included).await.unwrap();
        // Taking part again changes nothing for a jotting already in.
        jottings.set_recall(a, RecallState::Included).await.unwrap();
        assert_eq!(
            counts(&pool).await,
            BTreeMap::from([
                (a, Some(2)),
                (b, Some(2)),
                (c, Some(2)),
                (d.listed.jotting.id, Some(4)),
            ])
        );
        assert!(matches!(
            jottings
                .set_recall(Uuid::now_v7(), RecallState::Excluded)
                .await,
            Err(JottingsError::Database(DbError::NotFound))
        ));
    });
}
