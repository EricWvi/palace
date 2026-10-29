use super::database;
use palace_db::{Database, DbError, MomentDetail, OwnerScope, SleepCard, SleepNight};
use palace_domain::{SleepRange, SleepSpan};
use pretty_assertions::assert_eq;
use sqlx::PgPool;

const HOUR: i64 = 60 * 60 * 1000;
// 2025-10-28 23:30 UTC; the tests never depend on a local day.
const BED: i64 = 1_761_694_200_000;

async fn owner(db: &Database, subject: &str) -> OwnerScope {
    db.resolve_identity("https://idp", subject, &format!("{subject}@example.com"))
        .await
        .unwrap()
        .scope()
}

fn span(started_at: i64, ended_at: i64) -> SleepSpan {
    SleepSpan::parse(started_at, ended_at).unwrap()
}

/// Everything recorded for the owner around `at`: nights waking within 100 hours of it.
async fn nights(db: &Database, owner: OwnerScope, at: i64) -> Vec<SleepNight> {
    db.sleep_nights(
        owner,
        SleepRange::parse(at - 100 * HOUR, at + 100 * HOUR).unwrap(),
    )
    .await
    .unwrap()
}

/// Puts the owner's running start `hours` back, as if it had been running that long.
async fn started_hours_ago(pool: &PgPool, owner: OwnerScope, hours: i64) -> i64 {
    sqlx::query_scalar("UPDATE sleep_start SET started_at=date_trunc('milliseconds',now())-make_interval(hours=>$2::int) WHERE owner_id=$1 RETURNING (extract(epoch FROM started_at)*1000)::bigint")
        .bind(owner.id())
        .bind(hours as i32)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// A night is one sleep moment plus its detail, ending exactly when the moment occurred; the
/// database refuses to let either half drift from the other.
/// Core test case:
/// - `specs/test-cases/web/sleep/sleep.md#a-night-must-be-one-moment-whose-wake-time-is-its-place-on-the-timeline`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn a_night_must_be_one_moment_ending_at_its_wake_time() {
    let (_container, db, pool) = database().await;
    let me = owner(&db, "sleeper").await;
    let night = db
        .record_sleep_night(me, span(BED, BED + 7 * HOUR))
        .await
        .unwrap();
    let timeline = db
        .timeline(me, BED + 6 * HOUR, BED + 8 * HOUR)
        .await
        .unwrap();
    assert_eq!(
        timeline
            .into_iter()
            .map(|moment| (moment.id, moment.occurred_at, moment.detail))
            .collect::<Vec<_>>(),
        vec![(
            night.id,
            BED + 7 * HOUR,
            MomentDetail::Sleep(SleepCard { started_at: BED })
        )]
    );

    // Moving the moment alone, or the detail alone, fails at commit.
    let moved_moment =
        sqlx::query("UPDATE moment SET occurred_at=occurred_at+interval '1 hour' WHERE id=$1")
            .bind(night.id)
            .execute(&pool)
            .await;
    let moved_detail = sqlx::query("UPDATE sleep SET span=tstzrange(lower(span),upper(span)+interval '1 hour','[)') WHERE id=$1")
        .bind(night.id)
        .execute(&pool)
        .await;
    let orphaned = sqlx::query("DELETE FROM sleep WHERE id=$1")
        .bind(night.id)
        .execute(&pool)
        .await;
    assert_eq!(
        [
            moved_moment.is_err(),
            moved_detail.is_err(),
            orphaned.is_err()
        ],
        [true, true, true]
    );
    // A day-and-more span is refused by the table itself, whoever writes it.
    let too_long = sqlx::query("UPDATE sleep SET span=tstzrange(upper(span)-interval '25 hours',upper(span),'[)') WHERE id=$1")
        .bind(night.id)
        .execute(&pool)
        .await;
    assert!(too_long.is_err());
    assert_eq!(nights(&db, me, BED).await, vec![night]);
}

/// Two nights of one owner never overlap, though one may end when the next begins and another
/// owner's nights are no obstacle.
/// Core test case:
/// - `specs/test-cases/web/sleep/sleep.md#nights-must-never-overlap`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn nights_must_never_overlap() {
    let (_container, db, _pool) = database().await;
    let me = owner(&db, "sleeper").await;
    let other = owner(&db, "neighbour").await;
    let first = db
        .record_sleep_night(me, span(BED, BED + 7 * HOUR))
        .await
        .unwrap();
    let overlapping = db
        .record_sleep_night(me, span(BED + 6 * HOUR, BED + 9 * HOUR))
        .await;
    let adjacent = db
        .record_sleep_night(me, span(BED + 7 * HOUR, BED + 8 * HOUR))
        .await
        .unwrap();
    let theirs = db
        .record_sleep_night(other, span(BED, BED + 7 * HOUR))
        .await
        .unwrap();
    // Correcting a night onto its neighbour is refused and leaves both as they were.
    let moved = db
        .update_sleep_night(me, adjacent.id, span(BED + 6 * HOUR, BED + 8 * HOUR))
        .await;
    assert_eq!(
        (
            matches!(overlapping, Err(DbError::Conflict)),
            matches!(moved, Err(DbError::Conflict))
        ),
        (true, true)
    );
    assert_eq!(nights(&db, me, BED).await, vec![first, adjacent]);
    assert_eq!(nights(&db, other, BED).await, vec![theirs]);
}

/// One running start per owner; starting again replaces it, including a forgotten one.
/// Core test case:
/// - `specs/test-cases/web/sleep/sleep.md#starting-must-replace-any-earlier-start`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn starting_must_replace_any_earlier_start() {
    let (_container, db, pool) = database().await;
    let me = owner(&db, "sleeper").await;
    assert_eq!(db.running_sleep(me).await.unwrap(), None);
    db.start_sleep(me).await.unwrap();
    let forgotten = started_hours_ago(&pool, me, 20).await;
    let again = db.start_sleep(me).await.unwrap();
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM sleep_start WHERE owner_id=$1")
        .bind(me.id())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        (db.running_sleep(me).await.unwrap(), rows, again > forgotten),
        (Some(again), 1, true)
    );
    // Starting writes no night.
    assert_eq!(nights(&db, me, BED).await, vec![]);
}

/// Ending turns exactly the start the caller saw into a night; repeating it returns the same
/// night, and a failed end keeps the start.
/// Core test case:
/// - `specs/test-cases/web/sleep/sleep.md#ending-must-turn-the-seen-start-into-one-night-exactly-once`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn ending_must_turn_the_seen_start_into_one_night_once() {
    let (_container, db, pool) = database().await;
    let me = owner(&db, "sleeper").await;
    db.start_sleep(me).await.unwrap();
    let started = started_hours_ago(&pool, me, 7).await;

    // A start the caller did not see is not ended.
    let stale = db.end_sleep(me, started - 1).await;
    assert!(matches!(stale, Err(DbError::NotFound)));
    assert_eq!(db.running_sleep(me).await.unwrap(), Some(started));

    let night = db.end_sleep(me, started).await.unwrap();
    let retried = db.end_sleep(me, started).await.unwrap();
    assert_eq!(
        (
            night.started_at,
            night.ended_at - started >= 7 * HOUR,
            retried,
            db.running_sleep(me).await.unwrap()
        ),
        (started, true, night, None)
    );
    assert_eq!(nights(&db, me, started).await, vec![night]);
}

/// An end that cannot become a night (too long, or overlapping) writes nothing and keeps the
/// start for the next start to replace.
/// Core test case:
/// - `specs/test-cases/web/sleep/sleep.md#ending-must-turn-the-seen-start-into-one-night-exactly-once`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn a_failed_end_must_keep_the_start() {
    let (_container, db, pool) = database().await;
    let me = owner(&db, "sleeper").await;
    db.start_sleep(me).await.unwrap();
    let forgotten = started_hours_ago(&pool, me, 25).await;
    let too_long = db.end_sleep(me, forgotten).await;

    db.start_sleep(me).await.unwrap();
    let started = started_hours_ago(&pool, me, 2).await;
    // A night recorded by hand over the same hours blocks the end.
    let now: i64 = sqlx::query_scalar("SELECT (extract(epoch FROM now())*1000)::bigint")
        .fetch_one(&pool)
        .await
        .unwrap();
    let recorded = db
        .record_sleep_night(me, span(started + HOUR, now))
        .await
        .unwrap();
    let overlapping = db.end_sleep(me, started).await;
    assert_eq!(
        (
            matches!(too_long, Err(DbError::Input(_))),
            matches!(overlapping, Err(DbError::Conflict)),
            db.running_sleep(me).await.unwrap()
        ),
        (true, true, Some(started))
    );
    assert_eq!(nights(&db, me, started).await, vec![recorded]);
}

/// Correcting a night keeps its identity and moves its moment with the wake time; deleting
/// takes the moment with it.
/// Core test case:
/// - `specs/test-cases/web/sleep/sleep.md#correcting-or-deleting-a-night-must-move-or-remove-its-moment`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn correcting_or_deleting_a_night_must_move_or_remove_its_moment() {
    let (_container, db, pool) = database().await;
    let me = owner(&db, "sleeper").await;
    let night = db
        .record_sleep_night(me, span(BED, BED + 7 * HOUR))
        .await
        .unwrap();
    let later = BED + 30 * HOUR;
    let moved = db
        .update_sleep_night(me, night.id, span(later, later + 6 * HOUR))
        .await
        .unwrap();
    let timeline = db
        .timeline(me, later + 5 * HOUR, later + 7 * HOUR)
        .await
        .unwrap();
    assert_eq!(
        (
            moved,
            timeline
                .iter()
                .map(|moment| (moment.id, moment.occurred_at))
                .collect::<Vec<_>>(),
            db.timeline(me, BED + 6 * HOUR, BED + 8 * HOUR)
                .await
                .unwrap()
                .len()
        ),
        (
            SleepNight {
                id: night.id,
                started_at: later,
                ended_at: later + 6 * HOUR
            },
            vec![(night.id, later + 6 * HOUR)],
            0
        )
    );

    db.delete_sleep_night(me, night.id).await.unwrap();
    let moments: i64 = sqlx::query_scalar("SELECT count(*) FROM moment WHERE id=$1")
        .bind(night.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((nights(&db, me, BED).await, moments), (vec![], 0));
}

/// Another owner can neither see nor change someone's nights or running start; each attempt
/// reads as not found and changes nothing.
/// Core test case:
/// - `specs/test-cases/web/sleep/sleep.md#sleep-must-stay-within-the-owner-scope`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn sleep_must_stay_within_the_owner_scope() {
    let (_container, db, pool) = database().await;
    let me = owner(&db, "sleeper").await;
    let other = owner(&db, "neighbour").await;
    let night = db
        .record_sleep_night(me, span(BED, BED + 7 * HOUR))
        .await
        .unwrap();
    db.start_sleep(me).await.unwrap();
    let started = started_hours_ago(&pool, me, 3).await;

    let updated = db
        .update_sleep_night(other, night.id, span(BED, BED + HOUR))
        .await;
    let deleted = db.delete_sleep_night(other, night.id).await;
    let ended = db.end_sleep(other, started).await;
    assert_eq!(
        (
            matches!(updated, Err(DbError::NotFound)),
            matches!(deleted, Err(DbError::NotFound)),
            matches!(ended, Err(DbError::NotFound)),
            nights(&db, other, BED).await,
            db.running_sleep(other).await.unwrap(),
            db.timeline(other, BED + 6 * HOUR, BED + 8 * HOUR)
                .await
                .unwrap()
                .len(),
        ),
        (true, true, true, vec![], None, 0)
    );
    assert_eq!(
        (
            nights(&db, me, BED).await,
            db.running_sleep(me).await.unwrap()
        ),
        (vec![night], Some(started))
    );
}
