-- A Moment owns identity, kind and timeline position; each kind keeps its details in its own table.
CREATE TABLE moment (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    kind text NOT NULL CHECK(kind IN ('conversation')),
    occurred_at timestamptz NOT NULL,
    UNIQUE(owner_id, id, kind)
);
CREATE INDEX moment_timeline ON moment(owner_id, occurred_at, id);
CREATE TRIGGER moment_immutable_owner BEFORE UPDATE ON moment
    FOR EACH ROW EXECUTE FUNCTION reject_owner_change();

-- Every existing path becomes a conversation moment with the same identity and time.
INSERT INTO moment(id, owner_id, kind, occurred_at)
SELECT id, owner_id, 'conversation', occurred_at FROM conversation_path;
ALTER TABLE conversation_path
    ADD COLUMN kind text NOT NULL DEFAULT 'conversation' CHECK(kind = 'conversation');
ALTER TABLE conversation_path
    ADD FOREIGN KEY(owner_id, id, kind) REFERENCES moment(owner_id, id, kind);
ALTER TABLE conversation_path DROP COLUMN occurred_at;

-- A foreign key cannot say "this moment has a detail", so a deferred check runs at commit.
-- Writers may insert the moment before its detail, or delete the detail before its moment.
CREATE FUNCTION require_moment_detail() RETURNS trigger LANGUAGE plpgsql AS $$
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
    ) THEN
        RAISE EXCEPTION 'moment % has no detail of its kind', target;
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER moment_requires_detail AFTER INSERT OR UPDATE ON moment
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION require_moment_detail();
CREATE CONSTRAINT TRIGGER path_detail_keeps_moment AFTER DELETE ON conversation_path
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION require_moment_detail();
