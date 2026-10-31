-- 时刻 日课 and 工夫: habits with a count per local day, and activities as moments at their start.
-- See specs/decisions/web/habits/0-habits-as-seals-and-activities-as-moments.md.
CREATE TABLE habit (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    name text NOT NULL CHECK(btrim(name) <> '' AND octet_length(name) <= 256),
    -- The one character its seal shows.
    mark text NOT NULL CHECK(char_length(mark) = 1),
    times_per_day smallint NOT NULL CHECK(times_per_day BETWEEN 1 AND 10),
    created_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(created_at = date_trunc('milliseconds', created_at)),
    -- Retired habits leave the day page and take no checks; their checks stay for the history.
    retired_at timestamptz CHECK(retired_at = date_trunc('milliseconds', retired_at)),
    UNIQUE(owner_id, id)
);
CREATE TRIGGER habit_immutable_owner BEFORE UPDATE ON habit
    FOR EACH ROW EXECUTE FUNCTION reject_owner_change();
CREATE INDEX habit_by_owner ON habit(owner_id, created_at, id);

-- How many times a habit was done on one of the reader's local days. A day not done has no
-- row, so a count is never zero. Lowering a habit's times a day keeps these as they were; the
-- page caps what it shows.
CREATE TABLE habit_check (
    owner_id uuid NOT NULL,
    habit_id uuid NOT NULL,
    day date NOT NULL,
    count smallint NOT NULL CHECK(count BETWEEN 1 AND 10),
    PRIMARY KEY(owner_id, habit_id, day),
    FOREIGN KEY(owner_id, habit_id) REFERENCES habit(owner_id, id)
);
CREATE INDEX habit_check_by_day ON habit_check(owner_id, day);

CREATE TABLE activity_type (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    name text NOT NULL CHECK(btrim(name) <> '' AND octet_length(name) <= 256),
    created_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(created_at = date_trunc('milliseconds', created_at)),
    UNIQUE(owner_id, id),
    -- Adding the same name twice gives back the first, so one kind of activity is one line.
    UNIQUE(owner_id, name)
);
CREATE TRIGGER activity_type_immutable_owner BEFORE UPDATE ON activity_type
    FOR EACH ROW EXECUTE FUNCTION reject_owner_change();

ALTER TABLE moment DROP CONSTRAINT moment_kind_check;
ALTER TABLE moment ADD CONSTRAINT moment_kind_check
    CHECK(kind IN ('conversation', 'article', 'sleep', 'activity'));

-- An activity's start is its moment's occurred_at; the detail keeps only what the moment lacks.
CREATE TABLE activity (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    kind text NOT NULL DEFAULT 'activity' CHECK(kind = 'activity'),
    type_id uuid NOT NULL,
    minutes integer NOT NULL CHECK(minutes BETWEEN 1 AND 720),
    note text CHECK(btrim(note) <> '' AND octet_length(note) <= 1024),
    UNIQUE(owner_id, id),
    FOREIGN KEY(owner_id, id, kind) REFERENCES moment(owner_id, id, kind),
    -- The owner's own type only.
    FOREIGN KEY(owner_id, type_id) REFERENCES activity_type(owner_id, id)
);
CREATE TRIGGER activity_immutable_owner BEFORE UPDATE ON activity
    FOR EACH ROW EXECUTE FUNCTION reject_owner_change();

-- Each kind proves its own detail; see 0009_moments.sql for why this check runs at commit.
CREATE OR REPLACE FUNCTION require_moment_detail() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    target uuid;
BEGIN
    IF TG_OP = 'DELETE' THEN
        target := OLD.id;
    ELSE
        target := NEW.id;
    END IF;
    IF EXISTS(
        SELECT 1 FROM moment m
        WHERE m.id = target
          AND NOT (m.kind = 'conversation' AND EXISTS(SELECT 1 FROM conversation_path p WHERE p.id = m.id))
          AND NOT (m.kind = 'article' AND EXISTS(SELECT 1 FROM article a WHERE a.id = m.id))
          AND NOT (m.kind = 'sleep' AND EXISTS(
              SELECT 1 FROM sleep s WHERE s.id = m.id AND upper(s.span) = m.occurred_at))
          AND NOT (m.kind = 'activity' AND EXISTS(SELECT 1 FROM activity a WHERE a.id = m.id))
    ) THEN
        RAISE EXCEPTION 'moment % has no detail of its kind', target;
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER activity_detail_keeps_moment AFTER INSERT OR UPDATE OR DELETE ON activity
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION require_moment_detail();
