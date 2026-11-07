-- Jottings in inbox/ take no part in recall until they are sorted, so they must not hold the
-- least count down either: they are never drawn, so theirs would never grow. Leaving the inbox
-- counts as appearing, which keeps a long wait there from crowding the piles afterwards.
-- See specs/decisions/web/echo/20261107-inbox-jottings-take-no-part-in-recall.md.

-- `apart` leaves out the jotting being placed, whose own stale count must not set its new one.
CREATE FUNCTION jotting_recall_least(apart uuid) RETURNS integer LANGUAGE sql STABLE AS $$
    SELECT coalesce(min(r.completions), 0)
    FROM jotting_recall r JOIN jotting j ON j.id = r.jotting_id
    WHERE NOT starts_with(j.path, 'inbox/') AND r.jotting_id IS DISTINCT FROM apart
$$;

CREATE OR REPLACE FUNCTION follow_jotting_recall() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.state = 'deleted' THEN
        DELETE FROM jotting_recall WHERE jotting_id = NEW.id;
    ELSIF TG_OP = 'INSERT' OR OLD.state = 'deleted' THEN
        INSERT INTO jotting_recall(jotting_id, completions)
            VALUES (NEW.id, jotting_recall_least(NEW.id));
    ELSIF starts_with(OLD.path, 'inbox/') AND NOT starts_with(NEW.path, 'inbox/') THEN
        -- A jotting retired before it went back to the inbox stays retired.
        UPDATE jotting_recall SET completions = jotting_recall_least(NEW.id)
            WHERE jotting_id = NEW.id AND completions IS NOT NULL;
    END IF;
    RETURN NULL;
END $$;
DROP TRIGGER jotting_recall_follow ON jotting;
CREATE TRIGGER jotting_recall_follow AFTER INSERT OR UPDATE OF state, path ON jotting
    FOR EACH ROW EXECUTE FUNCTION follow_jotting_recall();
