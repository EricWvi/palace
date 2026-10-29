-- 时刻 sleep: a finished night is a moment on the day it ended; a running sleep is one row per
-- owner until it ends. See specs/decisions/web/sleep/0-sleep-started-from-the-day-menu-pinned-as-a-dial.md.
CREATE EXTENSION IF NOT EXISTS btree_gist;

ALTER TABLE moment DROP CONSTRAINT moment_kind_check;
ALTER TABLE moment ADD CONSTRAINT moment_kind_check
    CHECK(kind IN ('conversation', 'article', 'sleep'));

CREATE TABLE sleep (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    kind text NOT NULL DEFAULT 'sleep' CHECK(kind = 'sleep'),
    -- [bedtime, wake time). The wake time is also the moment's occurred_at; the commit check
    -- below keeps the two equal, so the overlap rule can live on this one table.
    span tstzrange NOT NULL CHECK(
        lower_inc(span) AND NOT upper_inc(span) AND NOT lower_inf(span) AND NOT upper_inf(span)
        AND NOT isempty(span)
        AND upper(span) - lower(span) <= interval '24 hours'
        AND lower(span) = date_trunc('milliseconds', lower(span))
        AND upper(span) = date_trunc('milliseconds', upper(span))
    ),
    UNIQUE(owner_id, id),
    FOREIGN KEY(owner_id, id, kind) REFERENCES moment(owner_id, id, kind),
    -- Half-open spans let one night's waking meet the next night's bedtime.
    EXCLUDE USING gist (owner_id WITH =, span WITH &&)
);
CREATE TRIGGER sleep_immutable_owner BEFORE UPDATE ON sleep
    FOR EACH ROW EXECUTE FUNCTION reject_owner_change();

-- The running sleep, not yet a night. Starting again replaces it, which is also how a start that
-- was never ended goes away.
CREATE TABLE sleep_start (
    owner_id uuid PRIMARY KEY REFERENCES owner(id),
    started_at timestamptz NOT NULL CHECK(started_at = date_trunc('milliseconds', started_at))
);

-- Each kind proves its own detail; see 0009_moments.sql for why this check runs at commit. A
-- night's detail must also end exactly when its moment occurred.
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
    ) THEN
        RAISE EXCEPTION 'moment % has no detail of its kind', target;
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER sleep_detail_keeps_moment AFTER INSERT OR UPDATE OR DELETE ON sleep
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION require_moment_detail();
