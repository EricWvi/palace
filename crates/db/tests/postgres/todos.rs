use super::database;
use palace_db::{
    CompletedTodo, Database, DbError, OwnerScope, Todo, TodoEdit, TodoList, TodoPlace,
};
use palace_domain::{
    LocalDay, TodoListName, TodoPlacement, TodoPlan, TodoPlanInput, TodoPosition, TodoTitle,
};
use pretty_assertions::assert_eq;
use uuid::Uuid;

async fn owner(db: &Database, subject: &str) -> OwnerScope {
    db.resolve_identity("https://idp", subject, &format!("{subject}@example.com"))
        .await
        .unwrap()
        .scope()
}

/// Adds a list at `position` among the owner's lists.
async fn list(db: &Database, owner: OwnerScope, name: &str, position: &str) -> Uuid {
    db.insert_todo_list(owner, &TodoListName::parse(name).unwrap(), &key(position))
        .await
        .unwrap()
        .id
}

fn key(key: &str) -> TodoPosition {
    TodoPosition::parse(key.into()).unwrap()
}

fn day(text: &str) -> LocalDay {
    LocalDay::parse(text, "day").unwrap()
}

/// Adds an open task at `position` in `list`, or in the inbox when there is none.
async fn task(
    db: &Database,
    owner: OwnerScope,
    list: Option<Uuid>,
    title: &str,
    position: &str,
) -> Todo {
    let placement = TodoPlacement::parse(list, TodoPlanInput::Open).unwrap();
    db.insert_todo(
        owner,
        &placement,
        &TodoTitle::parse(title).unwrap(),
        &key(position),
    )
    .await
    .unwrap()
}

/// An edit that keeps everything but the plan.
fn replan(todo: &Todo, plan: TodoPlanInput) -> TodoEdit {
    let list = match &todo.place {
        TodoPlace::Inbox => None,
        TodoPlace::List { list, .. } => Some(*list),
    };
    TodoEdit {
        placement: TodoPlacement::parse(list, plan).unwrap(),
        title: TodoTitle::parse(&todo.title).unwrap(),
        link: None,
        position: todo.position.clone(),
    }
}

fn planned(todo: &Todo) -> TodoPlan {
    match &todo.place {
        TodoPlace::List { plan, .. } => plan.clone(),
        TodoPlace::Inbox => TodoPlan::Open,
    }
}

/// Another owner can neither see nor change a task or a list, nor add tasks to someone else's
/// list, and every such request reads as not found.
/// Core test case:
/// - `specs/test-cases/web/todo/todos.md#tasks-and-lists-must-stay-within-the-owner-scope`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn todos_must_stay_within_the_owner_scope() {
    let (_container, db, _pool) = database().await;
    let a = owner(&db, "a").await;
    let b = owner(&db, "b").await;
    let work = list(&db, a, "工作", "a0").await;
    let todo = task(&db, a, Some(work), "ai infra", "a0").await;
    let board = db.todo_board(a).await.unwrap();

    let placement = TodoPlacement::parse(Some(work), TodoPlanInput::Open).unwrap();
    let title = TodoTitle::parse("偷放").unwrap();
    let today = day("2026-10-23");
    let name = TodoListName::parse("改名").unwrap();
    let refused = [
        db.insert_todo(b, &placement, &title, &key("a1"))
            .await
            .map(|_| ()),
        db.update_todo(b, todo.id, &replan(&todo, TodoPlanInput::Open))
            .await
            .map(|_| ()),
        db.set_todo_body(b, todo.id, None).await,
        db.todo_body(b, todo.id).await.map(|_| ()),
        db.set_todo_done(b, todo.id, today, true).await.map(|_| ()),
        db.complete_todo(b, todo.id).await,
        db.delete_todo(b, todo.id).await,
        db.update_todo_list(b, work, &name, &key("a5"))
            .await
            .map(|_| ()),
        db.completed_todos(b, work).await.map(|_| ()),
        db.clear_completed_todos(b, work).await,
        db.delete_todo_list(b, work).await,
    ];
    for result in refused {
        assert!(matches!(result, Err(DbError::NotFound)), "{result:?}");
    }
    // A's own task cannot be moved into B's list either.
    let theirs = list(&db, b, "别人的", "a0").await;
    let mut edit = replan(&todo, TodoPlanInput::Open);
    edit.placement = TodoPlacement::parse(Some(theirs), TodoPlanInput::Open).unwrap();
    assert!(matches!(
        db.update_todo(a, todo.id, &edit).await,
        Err(DbError::NotFound)
    ));
    // Planning B's ids from A, or A's from B, touches nothing.
    db.plan_todos(b, today, &[todo.id]).await.unwrap();
    assert_eq!(db.todo_board(a).await.unwrap(), board);
    assert_eq!(db.todo_board(b).await.unwrap().todos, Vec::<Todo>::new());
}

/// The inbox holds no plan and nothing finished; shelved and a day exclude each other; done
/// lives only on a planned day and goes with any change of plan; finishing drops the plan.
/// Core test case:
/// - `specs/test-cases/web/todo/todos.md#a-task-must-keep-one-plan-and-the-inbox-none`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn a_task_must_keep_one_plan_and_the_inbox_none() {
    let (_container, db, pool) = database().await;
    let a = owner(&db, "a").await;
    let work = list(&db, a, "工作", "a0").await;
    let inbox = task(&db, a, None, "先收下", "a0").await;
    let todo = task(&db, a, Some(work), "读书", "a0").await;
    let today = day("2026-10-23");

    assert!(matches!(
        db.complete_todo(a, inbox.id).await,
        Err(DbError::Conflict)
    ));
    for statement in [
        "UPDATE todo SET planned_on='2026-10-23' WHERE list_id IS NULL",
        "UPDATE todo SET shelved=true WHERE list_id IS NULL",
        "UPDATE todo SET shelved=true,planned_on='2026-10-23' WHERE list_id IS NOT NULL",
        "UPDATE todo SET done=true WHERE list_id IS NOT NULL",
        "UPDATE todo SET completed_at=date_trunc('milliseconds',now()),planned_on='2026-10-23' WHERE list_id IS NOT NULL",
    ] {
        let error = sqlx::query(statement).execute(&pool).await.unwrap_err();
        assert_eq!(
            error
                .as_database_error()
                .and_then(sqlx::error::DatabaseError::code)
                .as_deref(),
            Some("23514"),
            "{statement}"
        );
    }

    let on_today = db
        .update_todo(a, todo.id, &replan(&todo, TodoPlanInput::Day(today)))
        .await
        .unwrap();
    let done = db.set_todo_done(a, todo.id, today, true).await.unwrap();
    assert_eq!(
        planned(&done),
        TodoPlan::Day {
            day: today,
            done: true
        }
    );
    // Rewriting the same day keeps done; any other plan drops it, count untouched.
    let same = db
        .update_todo(a, todo.id, &replan(&on_today, TodoPlanInput::Day(today)))
        .await
        .unwrap();
    assert_eq!(same, done);
    let shelved = db
        .update_todo(a, todo.id, &replan(&on_today, TodoPlanInput::Shelved))
        .await
        .unwrap();
    assert_eq!(
        (planned(&shelved), shelved.done_days),
        (TodoPlan::Shelved, 1)
    );

    db.update_todo(a, todo.id, &replan(&on_today, TodoPlanInput::Day(today)))
        .await
        .unwrap();
    db.complete_todo(a, todo.id).await.unwrap();
    let finished: (Option<String>, bool, bool) =
        sqlx::query_as("SELECT planned_on::text,shelved,done FROM todo WHERE id=$1")
            .bind(todo.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(finished, (None, false, false));
    // A finished task takes no edits and no done; it comes back unscheduled.
    assert!(matches!(
        db.update_todo(a, todo.id, &replan(&on_today, TodoPlanInput::Open))
            .await,
        Err(DbError::Conflict)
    ));
    assert!(matches!(
        db.set_todo_done(a, todo.id, today, false).await,
        Err(DbError::Conflict)
    ));
    let restored = db.restore_todo(a, todo.id, &key("Zz")).await.unwrap();
    assert_eq!(
        (planned(&restored), restored.position, restored.done_days),
        (TodoPlan::Open, key("Zz"), 1)
    );
}

/// Done counts a day once whatever the repeats, only on the planned day; planning a day takes
/// only open tasks in lists that are unscheduled or on a past day.
/// Core test case:
/// - `specs/test-cases/web/todo/todos.md#done-for-the-day-must-count-each-day-once`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn done_for_the_day_must_count_each_day_once() {
    let (_container, db, _pool) = database().await;
    let a = owner(&db, "a").await;
    let work = list(&db, a, "工作", "a0").await;
    let yesterday = day("2026-10-22");
    let today = day("2026-10-23");
    let tomorrow = day("2026-10-24");
    let lapsed = task(&db, a, Some(work), "昨天的", "a0").await;
    let fresh = task(&db, a, Some(work), "没排的", "a1").await;
    let shelved = task(&db, a, Some(work), "搁置的", "a2").await;
    let later = task(&db, a, Some(work), "明天的", "a3").await;
    let inbox = task(&db, a, None, "收件箱", "a0").await;
    db.update_todo(
        a,
        lapsed.id,
        &replan(&lapsed, TodoPlanInput::Day(yesterday)),
    )
    .await
    .unwrap();
    db.set_todo_done(a, lapsed.id, yesterday, true)
        .await
        .unwrap();
    db.update_todo(a, shelved.id, &replan(&shelved, TodoPlanInput::Shelved))
        .await
        .unwrap();
    db.update_todo(a, later.id, &replan(&later, TodoPlanInput::Day(tomorrow)))
        .await
        .unwrap();

    db.plan_todos(
        a,
        today,
        &[lapsed.id, fresh.id, shelved.id, later.id, inbox.id],
    )
    .await
    .unwrap();
    let plans: Vec<(String, TodoPlan, i32)> = db
        .todo_board(a)
        .await
        .unwrap()
        .todos
        .iter()
        .map(|todo| (todo.title.clone(), planned(todo), todo.done_days))
        .collect();
    let on = |day: &LocalDay| TodoPlan::Day {
        day: *day,
        done: false,
    };
    assert_eq!(
        plans,
        [
            ("昨天的".into(), on(&today), 1),
            ("收件箱".into(), TodoPlan::Open, 0),
            ("没排的".into(), on(&today), 0),
            ("搁置的".into(), TodoPlan::Shelved, 0),
            ("明天的".into(), on(&tomorrow), 0),
        ]
    );

    for done in [true, true] {
        db.set_todo_done(a, fresh.id, today, done).await.unwrap();
    }
    let counted = db.set_todo_done(a, fresh.id, today, true).await.unwrap();
    assert_eq!(counted.done_days, 1);
    for done in [false, false] {
        db.set_todo_done(a, fresh.id, today, done).await.unwrap();
    }
    let undone = db.set_todo_done(a, fresh.id, today, false).await.unwrap();
    assert_eq!(undone.done_days, 0);
    // A page still on yesterday cannot count a task that moved to today.
    assert!(matches!(
        db.set_todo_done(a, fresh.id, yesterday, true).await,
        Err(DbError::Conflict)
    ));
    assert!(matches!(
        db.set_todo_done(a, inbox.id, today, true).await,
        Err(DbError::Conflict)
    ));
}

/// Open tasks in one place never share a key, the inbox included; finished tasks free theirs.
/// Core test case:
/// - `specs/test-cases/web/todo/todos.md#task-order-must-come-from-unique-well-formed-keys`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn open_tasks_in_one_place_must_not_share_a_key() {
    let (_container, db, pool) = database().await;
    let a = owner(&db, "a").await;
    let b = owner(&db, "b").await;
    let work = list(&db, a, "工作", "a0").await;
    let life = list(&db, a, "个人", "a1").await;
    let first = task(&db, a, Some(work), "第一", "a0").await;
    task(&db, a, Some(life), "别的集合", "a0").await;
    task(&db, a, None, "收件箱", "a0").await;
    task(&db, b, None, "别人的收件箱", "a0").await;
    let placement = |list| TodoPlacement::parse(list, TodoPlanInput::Open).unwrap();
    let title = TodoTitle::parse("撞键").unwrap();
    for place in [Some(work), None] {
        assert!(matches!(
            db.insert_todo(a, &placement(place), &title, &key("a0"))
                .await,
            Err(DbError::Conflict)
        ));
    }
    // Moving onto a taken key is refused too.
    let second = task(&db, a, Some(work), "第二", "a1").await;
    let mut edit = replan(&second, TodoPlanInput::Open);
    edit.position = key("a0");
    assert!(matches!(
        db.update_todo(a, second.id, &edit).await,
        Err(DbError::Conflict)
    ));

    db.complete_todo(a, first.id).await.unwrap();
    let reused = task(&db, a, Some(work), "接手", "a0").await;
    assert!(matches!(
        db.restore_todo(a, first.id, &key("a0")).await,
        Err(DbError::Conflict)
    ));
    db.restore_todo(a, first.id, &key("Zz")).await.unwrap();
    let order: Vec<String> = db
        .todo_board(a)
        .await
        .unwrap()
        .todos
        .into_iter()
        .filter(|todo| matches!(todo.place, TodoPlace::List { list, .. } if list == work))
        .map(|todo| todo.title)
        .collect();
    assert_eq!(order, ["第一", "接手", "第二"]);
    // Keys compare as bytes: "Zz" sorts before "a0", as in JavaScript.
    let collation: String =
        sqlx::query_scalar("SELECT collation_name FROM information_schema.columns WHERE table_name='todo' AND column_name='position'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((collation, reused.position), ("C".to_owned(), key("a0")));
}

/// Lists sort by their keys as bytes, an owner's lists never share one, and moving or renaming
/// rewrites only the list itself.
/// Core test case:
/// - `specs/test-cases/web/todo/todos.md#list-order-must-come-from-unique-well-formed-keys`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn lists_must_not_share_a_key() {
    let (_container, db, pool) = database().await;
    let a = owner(&db, "a").await;
    let b = owner(&db, "b").await;
    let work = list(&db, a, "工作", "a0").await;
    let life = list(&db, a, "个人", "a1").await;
    let study = list(&db, a, "学习", "a2").await;
    // Another owner may hold the same key.
    list(&db, b, "别人的", "a0").await;
    let name = |text: &str| TodoListName::parse(text).unwrap();
    assert!(matches!(
        db.insert_todo_list(a, &name("撞键"), &key("a1")).await,
        Err(DbError::Conflict)
    ));
    assert!(matches!(
        db.update_todo_list(a, study, &name("学习"), &key("a0"))
            .await,
        Err(DbError::Conflict)
    ));

    // Moving 学习 above everything, "Zz" before "a0" as bytes, then renaming 工作 in place.
    db.update_todo_list(a, study, &name("学习"), &key("Zz"))
        .await
        .unwrap();
    db.update_todo_list(a, work, &name("职业"), &key("a0"))
        .await
        .unwrap();
    let lists = db.todo_board(a).await.unwrap().lists;
    let collation: String =
        sqlx::query_scalar("SELECT collation_name FROM information_schema.columns WHERE table_name='todo_list' AND column_name='position'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (lists, collation),
        (
            vec![
                TodoList {
                    id: study,
                    name: "学习".into(),
                    position: key("Zz"),
                },
                TodoList {
                    id: work,
                    name: "职业".into(),
                    position: key("a0"),
                },
                TodoList {
                    id: life,
                    name: "个人".into(),
                    position: key("a1"),
                },
            ],
            "C".to_owned()
        )
    );
}

/// Deleting a list deletes its open and finished tasks; clearing deletes only the finished ones.
/// Core test case:
/// - `specs/test-cases/web/todo/todos.md#a-list-must-take-its-tasks-with-it`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn a_list_must_take_its_tasks_with_it() {
    let (_container, db, pool) = database().await;
    let a = owner(&db, "a").await;
    let work = list(&db, a, "工作", "a0").await;
    let life = list(&db, a, "个人", "a1").await;
    let open = task(&db, a, Some(work), "开着", "a0").await;
    let finished = task(&db, a, Some(work), "做完", "a1").await;
    let elsewhere = task(&db, a, Some(life), "别处", "a0").await;
    db.complete_todo(a, finished.id).await.unwrap();
    let completed = db.completed_todos(a, work).await.unwrap();
    assert_eq!(
        completed,
        [CompletedTodo {
            id: finished.id,
            title: "做完".into(),
            link: None,
            has_body: false,
            done_days: 0,
            completed_at: completed[0].completed_at,
        }]
    );

    db.clear_completed_todos(a, work).await.unwrap();
    assert_eq!(db.completed_todos(a, work).await.unwrap(), []);
    assert_eq!(
        db.todo_board(a).await.unwrap().todos,
        [open.clone(), elsewhere.clone()]
    );

    db.complete_todo(a, open.id).await.unwrap();
    task(&db, a, Some(work), "又一条", "a0").await;
    db.delete_todo_list(a, work).await.unwrap();
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM todo WHERE list_id=$1")
        .bind(work)
        .fetch_one(&pool)
        .await
        .unwrap();
    let board = db.todo_board(a).await.unwrap();
    assert_eq!(
        (left, board.lists, board.todos),
        (
            0,
            vec![TodoList {
                id: life,
                name: "个人".into(),
                position: key("a1"),
            }],
            vec![elsewhere]
        )
    );
}
