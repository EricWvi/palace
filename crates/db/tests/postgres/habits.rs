use super::database;
use palace_db::{Database, DbError, Habit, HabitBoard, HabitCheck, OwnerScope};
use palace_domain::{
    HabitCount, HabitDays, HabitInput, HabitMark, HabitName, HabitTimes, LocalDay,
};
use pretty_assertions::assert_eq;

async fn owner(db: &Database, subject: &str) -> OwnerScope {
    db.resolve_identity("https://idp", subject, &format!("{subject}@example.com"))
        .await
        .unwrap()
        .scope()
}

fn input(name: &str, mark: &str, times: i64) -> HabitInput {
    HabitInput {
        name: HabitName::parse(name).unwrap(),
        mark: HabitMark::parse(mark).unwrap(),
        times_per_day: HabitTimes::parse(times).unwrap(),
    }
}

fn day(text: &str) -> LocalDay {
    LocalDay::parse(text, "day").unwrap()
}

fn count(value: i64) -> HabitCount {
    HabitCount::parse(value).unwrap()
}

async fn board(db: &Database, owner: OwnerScope, from: &str, to: &str) -> HabitBoard {
    db.habit_board(owner, HabitDays::parse(from, to).unwrap())
        .await
        .unwrap()
}

/// A check is a count set for a day, so repeating one changes nothing, zero leaves no row, and a
/// count above the habit's times a day is refused without touching what was there. Lowering
/// the times a day keeps the history as written.
/// Core test case:
/// - `specs/test-cases/web/habits/habits.md#a-check-must-be-set-never-accumulated`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn a_check_must_be_set_never_accumulated() {
    let (_container, db, _pool) = database().await;
    let me = owner(&db, "practiser").await;
    let workout = db.insert_habit(me, &input("锻炼", "锻", 2)).await.unwrap();
    let on = day("2026-10-31");

    let once = db
        .set_habit_check(me, workout.id, on, count(1))
        .await
        .unwrap();
    let again = db
        .set_habit_check(me, workout.id, on, count(1))
        .await
        .unwrap();
    assert_eq!(once, again);
    assert_eq!(
        board(&db, me, "2026-10-31", "2026-10-31").await.checks,
        vec![HabitCheck {
            habit_id: workout.id,
            day: on,
            count: 1
        }]
    );

    db.set_habit_check(me, workout.id, on, count(2))
        .await
        .unwrap();
    let too_many = db.set_habit_check(me, workout.id, on, count(3)).await;
    assert!(matches!(too_many, Err(DbError::Input(error)) if error.path == "count"));
    assert_eq!(
        board(&db, me, "2026-10-31", "2026-10-31").await.checks[0].count,
        2
    );

    // Lowering the times a day leaves the day at two; the page caps what it shows.
    let lowered = db
        .update_habit(me, workout.id, &input("锻炼", "锻", 1))
        .await
        .unwrap();
    assert_eq!(lowered.times_per_day, 1);
    assert_eq!(
        board(&db, me, "2026-10-31", "2026-10-31").await.checks[0].count,
        2
    );

    assert_eq!(
        db.set_habit_check(me, workout.id, on, count(0))
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        db.set_habit_check(me, workout.id, on, count(0))
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        board(&db, me, "2026-10-31", "2026-10-31").await.checks,
        vec![]
    );
}

/// Retiring keeps a habit's checks for the history but takes it off every day and refuses any
/// further write; a retired habit is listed only where it has checks.
/// Core test case:
/// - `specs/test-cases/web/habits/habits.md#a-retired-habit-must-keep-its-checks-but-take-no-more`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn a_retired_habit_must_keep_its_checks_but_take_no_more() {
    let (_container, db, _pool) = database().await;
    let me = owner(&db, "practiser").await;
    let reading = db.insert_habit(me, &input("读书", "读", 1)).await.unwrap();
    let unused = db.insert_habit(me, &input("冥想", "冥", 1)).await.unwrap();
    let kept = db.insert_habit(me, &input("拉伸", "伸", 1)).await.unwrap();
    db.set_habit_check(me, reading.id, day("2026-10-20"), count(1))
        .await
        .unwrap();

    for id in [reading.id, unused.id, reading.id] {
        db.retire_habit(me, id).await.unwrap();
    }
    assert!(matches!(
        db.set_habit_check(me, reading.id, day("2026-10-31"), count(1))
            .await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.update_habit(me, reading.id, &input("阅读", "阅", 1))
            .await,
        Err(DbError::NotFound)
    ));

    let retired = Habit {
        retired: true,
        ..reading.clone()
    };
    assert_eq!(
        board(&db, me, "2026-10-01", "2026-10-31").await,
        HabitBoard {
            habits: vec![retired, kept.clone()],
            checks: vec![HabitCheck {
                habit_id: reading.id,
                day: day("2026-10-20"),
                count: 1
            }],
        }
    );
    assert_eq!(
        board(&db, me, "2026-10-21", "2026-10-31").await,
        HabitBoard {
            habits: vec![kept],
            checks: vec![],
        }
    );
}

/// Another owner's habits are not there: not listed, not editable, not checkable, not retirable.
/// Core test case:
/// - `specs/test-cases/web/habits/habits.md#activity-types-must-be-unique-and-owned`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn habits_must_stay_in_their_owner_scope() {
    let (_container, db, _pool) = database().await;
    let a = owner(&db, "a").await;
    let b = owner(&db, "b").await;
    let mine = db.insert_habit(a, &input("锻炼", "锻", 2)).await.unwrap();
    db.set_habit_check(a, mine.id, day("2026-10-31"), count(1))
        .await
        .unwrap();

    assert_eq!(
        board(&db, b, "2026-10-31", "2026-10-31").await,
        HabitBoard {
            habits: vec![],
            checks: vec![]
        }
    );
    assert!(matches!(
        db.set_habit_check(b, mine.id, day("2026-10-31"), count(0))
            .await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.update_habit(b, mine.id, &input("偷", "偷", 1)).await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.retire_habit(b, mine.id).await,
        Err(DbError::NotFound)
    ));
    assert_eq!(
        board(&db, a, "2026-10-31", "2026-10-31").await,
        HabitBoard {
            habits: vec![mine.clone()],
            checks: vec![HabitCheck {
                habit_id: mine.id,
                day: day("2026-10-31"),
                count: 1
            }]
        }
    );
}
