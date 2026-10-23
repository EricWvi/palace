-- 行事: lists and the tasks in them. A task with no list sits in the inbox, the buffer for tasks
-- whose list is not decided yet. See specs/decisions/web/todo/0-today-and-lists-with-fractional-order.md.
CREATE TABLE todo_list (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    name text NOT NULL CHECK(btrim(name) <> '' AND octet_length(name) <= 256),
    created_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(created_at = date_trunc('milliseconds', created_at)),
    updated_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(updated_at = date_trunc('milliseconds', updated_at)),
    UNIQUE(owner_id, id)
);
CREATE TRIGGER todo_list_immutable_owner BEFORE UPDATE ON todo_list
    FOR EACH ROW EXECUTE FUNCTION reject_owner_change();

CREATE TABLE todo (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    -- NULL is the inbox.
    list_id uuid,
    title text NOT NULL CHECK(btrim(title) <> '' AND octet_length(title) <= 1024),
    -- A fractional-indexing key written by the page. "C" collation compares bytes, the order
    -- JavaScript strings and the key format assume.
    position text COLLATE "C" NOT NULL CHECK(position <> '' AND octet_length(position) <= 1024),
    link text CHECK(link <> '' AND octet_length(link) <= 2048),
    body text CHECK(octet_length(body) <= 262144),
    -- The plan: no day and not shelved is unscheduled. A day in the past reads as unscheduled.
    planned_on date,
    shelved boolean NOT NULL DEFAULT false,
    -- Done for the planned day; it never outlives a change of plan.
    done boolean NOT NULL DEFAULT false,
    done_days integer NOT NULL DEFAULT 0 CHECK(done_days >= 0),
    completed_at timestamptz CHECK(completed_at = date_trunc('milliseconds', completed_at)),
    created_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(created_at = date_trunc('milliseconds', created_at)),
    updated_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(updated_at = date_trunc('milliseconds', updated_at)),
    CHECK(NOT (shelved AND planned_on IS NOT NULL)),
    CHECK(NOT done OR planned_on IS NOT NULL),
    -- The inbox is never planned from and holds nothing finished.
    CHECK(list_id IS NOT NULL OR (planned_on IS NULL AND NOT shelved AND completed_at IS NULL)),
    -- A finished task leaves every plan behind; restoring it starts unscheduled.
    CHECK(completed_at IS NULL OR (planned_on IS NULL AND NOT shelved AND NOT done)),
    -- Deleting a list deletes its tasks in the same statement.
    FOREIGN KEY(owner_id, list_id) REFERENCES todo_list(owner_id, id) ON DELETE CASCADE
);
CREATE TRIGGER todo_immutable_owner BEFORE UPDATE ON todo
    FOR EACH ROW EXECUTE FUNCTION reject_owner_change();
-- Open tasks in one place never share a key, so a key between two neighbours always exists.
-- The inbox is a NULL list, which must collide with itself too.
CREATE UNIQUE INDEX todo_open_position ON todo(owner_id, list_id, position) NULLS NOT DISTINCT
    WHERE completed_at IS NULL;
CREATE INDEX todo_completed ON todo(owner_id, list_id, completed_at DESC)
    WHERE completed_at IS NOT NULL;
