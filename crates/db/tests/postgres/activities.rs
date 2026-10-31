use super::database;
use palace_db::{
    Activity, ActivityCard, ActivityType, Database, DbError, MomentDetail, MomentKind,
    MomentOutline, OwnerScope, TypeAdded,
};
use palace_domain::{ActivityInput, ActivityTypeName, InstantRange};
use pretty_assertions::assert_eq;
use uuid::Uuid;

const MINUTE: i64 = 60 * 1000;
const DAY: i64 = 24 * 60 * MINUTE;
// 2026-10-31 07:05 UTC; the tests never depend on a local day.
const RUN: i64 = 1_793_430_300_000;

async fn owner(db: &Database, subject: &str) -> OwnerScope {
    db.resolve_identity("https://idp", subject, &format!("{subject}@example.com"))
        .await
        .unwrap()
        .scope()
}

async fn add_type(db: &Database, owner: OwnerScope, name: &str) -> ActivityType {
    db.add_activity_type(owner, &ActivityTypeName::parse(name).unwrap())
        .await
        .unwrap()
        .0
}

fn input(type_id: Uuid, started_at: i64, minutes: i64, note: Option<&str>) -> ActivityInput {
    ActivityInput::parse(type_id, started_at, minutes, note).unwrap()
}

/// An activity is one moment at its start plus its detail: the timeline shows its card, a
/// correction moves it without a new identity, and neither half can be removed alone.
/// Core test case:
/// - `specs/test-cases/web/habits/habits.md#an-activity-must-be-one-moment-timed-at-its-start`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn an_activity_must_be_one_moment_timed_at_its_start() {
    let (_container, db, pool) = database().await;
    let me = owner(&db, "runner").await;
    let running = add_type(&db, me, "跑步").await;
    let run = db
        .record_activity(me, &input(running.id, RUN, 35, Some("晨跑 5 公里")))
        .await
        .unwrap();
    assert_eq!(
        run,
        Activity {
            id: run.id,
            type_id: running.id,
            type_name: "跑步".into(),
            started_at: RUN,
            minutes: 35,
            note: Some("晨跑 5 公里".into()),
        }
    );
    assert_eq!(
        db.timeline(me, RUN - MINUTE, RUN + MINUTE)
            .await
            .unwrap()
            .into_iter()
            .map(|moment| (moment.id, moment.occurred_at, moment.detail))
            .collect::<Vec<_>>(),
        vec![(
            run.id,
            RUN,
            MomentDetail::Activity(ActivityCard {
                type_id: running.id,
                type_name: "跑步".into(),
                minutes: 35,
                note: Some("晨跑 5 公里".into()),
            })
        )]
    );
    assert_eq!(
        db.day_outline(me, RUN - MINUTE, RUN + MINUTE)
            .await
            .unwrap(),
        vec![MomentOutline {
            id: run.id,
            kind: MomentKind::Activity
        }]
    );

    // A correction carries the moment to the next day under the same identity.
    let moved = db
        .update_activity(me, run.id, &input(running.id, RUN + DAY, 40, None))
        .await
        .unwrap();
    assert_eq!(
        moved,
        Activity {
            started_at: RUN + DAY,
            minutes: 40,
            note: None,
            ..run.clone()
        }
    );
    assert_eq!(
        db.activities(me, InstantRange::parse(RUN - DAY, RUN + 2 * DAY).unwrap())
            .await
            .unwrap(),
        vec![moved.clone()]
    );

    // The detail alone cannot go.
    let detail_only = sqlx::query("DELETE FROM activity WHERE id=$1")
        .bind(run.id)
        .execute(&pool)
        .await;
    assert!(detail_only.is_err());

    db.delete_activity(me, run.id).await.unwrap();
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM moment WHERE id=$1")
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
    assert!(matches!(
        db.delete_activity(me, run.id).await,
        Err(DbError::NotFound)
    ));
}

/// Adding a name twice finds the first type; an activity can only use the owner's own types, and
/// another owner's activities are not there.
/// Core test case:
/// - `specs/test-cases/web/habits/habits.md#activity-types-must-be-unique-and-owned`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn activity_types_must_be_unique_and_owned() {
    let (_container, db, _pool) = database().await;
    let a = owner(&db, "a").await;
    let b = owner(&db, "b").await;
    let name = ActivityTypeName::parse(" 跑步 ").unwrap();
    let (first, added) = db.add_activity_type(a, &name).await.unwrap();
    assert_eq!(added, TypeAdded::Created);
    assert_eq!(
        db.add_activity_type(a, &name).await.unwrap(),
        (first.clone(), TypeAdded::Existing)
    );
    let theirs = add_type(&db, b, "跑步").await;
    let strength = add_type(&db, a, "力量").await;
    assert_eq!(
        db.activity_types(a).await.unwrap(),
        vec![first.clone(), strength]
    );

    // B's type is not A's to use; nothing is written.
    assert!(matches!(
        db.record_activity(a, &input(theirs.id, RUN, 30, None))
            .await,
        Err(DbError::NotFound)
    ));
    let range = InstantRange::parse(RUN - DAY, RUN + DAY).unwrap();
    assert_eq!(db.activities(a, range).await.unwrap(), vec![]);

    let run = db
        .record_activity(a, &input(first.id, RUN, 30, None))
        .await
        .unwrap();
    assert!(matches!(
        db.update_activity(a, run.id, &input(theirs.id, RUN, 30, None))
            .await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.update_activity(b, run.id, &input(theirs.id, RUN, 30, None))
            .await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.delete_activity(b, run.id).await,
        Err(DbError::NotFound)
    ));
    assert_eq!(db.activities(b, range).await.unwrap(), vec![]);
    assert_eq!(db.activities(a, range).await.unwrap(), vec![run]);
}
