-- 今日回响: how often each jotting was recalled, and the stored daily piles. This is Palace's own
-- state, not part of the Git index, so none of it changes a jotting's version.
-- See specs/decisions/web/echo/20261101-recall-least-completed-first-from-a-stored-daily-pile.md.

-- Exactly the live jottings have a row, kept so by the trigger below.
CREATE TABLE jotting_recall (
    jotting_id uuid PRIMARY KEY REFERENCES jotting(id),
    -- Times a recall card of it was completed; null when it no longer takes part in recall.
    completions integer CHECK(completions >= 0)
);
-- The least count is read whenever a jotting appears or takes part again.
CREATE INDEX jotting_recall_least ON jotting_recall(completions);

-- Jottings indexed before recall counted anything start level.
INSERT INTO jotting_recall(jotting_id, completions)
    SELECT id, 0 FROM jotting WHERE state <> 'deleted';

-- A jotting that appears, new or revived, starts at the least count of the moment: it joins the
-- round under way instead of crowding the piles until it catches up. A tombstone drops the row,
-- so a revived jotting starts afresh, in recall. Kept in the database so neither the commit
-- reconciliation nor uploads have to remember it.
CREATE FUNCTION follow_jotting_recall() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state = 'deleted' THEN
        DELETE FROM jotting_recall WHERE jotting_id = NEW.id;
    ELSIF TG_OP = 'INSERT' OR OLD.state = 'deleted' THEN
        INSERT INTO jotting_recall(jotting_id, completions)
            VALUES (NEW.id, coalesce((SELECT min(completions) FROM jotting_recall), 0));
    END IF;
    RETURN NULL;
END $$;
CREATE TRIGGER jotting_recall_follow AFTER INSERT OR UPDATE OF state ON jotting
    FOR EACH ROW EXECUTE FUNCTION follow_jotting_recall();

-- One drawn pile per local day; a day without a visit has none. Only the latest two are kept.
CREATE TABLE jotting_recall_pile (
    day date PRIMARY KEY,
    drawn_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(drawn_at = date_trunc('milliseconds', drawn_at))
);

CREATE TABLE jotting_recall_card (
    day date NOT NULL REFERENCES jotting_recall_pile(day) ON DELETE CASCADE,
    position smallint NOT NULL CHECK(position >= 0),
    jotting_id uuid NOT NULL REFERENCES jotting(id),
    -- Set once, when the card is first completed; only then does its count grow.
    done_at timestamptz CHECK(done_at = date_trunc('milliseconds', done_at)),
    PRIMARY KEY(day, position),
    UNIQUE(day, jotting_id)
);
