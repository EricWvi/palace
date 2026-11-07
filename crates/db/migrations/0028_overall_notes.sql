-- 总批: one block of text on a whole article, conversation path or web page, where an annotation
-- belongs to a run of words. See
-- specs/decisions/web/stars/20261107-overall-notes-on-articles-paths-and-web-pages.md.

-- Clearing a note deletes its row, so counting versions per row would hand out an old number
-- again and let a stale page past the check. A sequence never repeats one.
CREATE SEQUENCE overall_note_version;

CREATE TABLE overall_note (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    article_id uuid,
    conversation_id uuid,
    path_id uuid,
    page_id uuid,
    text text NOT NULL CHECK(btrim(text) <> '' AND octet_length(text) <= 65536),
    version bigint NOT NULL DEFAULT nextval('overall_note_version'),
    updated_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(updated_at = date_trunc('milliseconds', updated_at)),
    -- Exactly one target; a path is named with its conversation, as its key requires.
    CHECK(num_nonnulls(article_id, path_id, page_id) = 1),
    CHECK((conversation_id IS NULL) = (path_id IS NULL)),
    -- Deleting the document deletes what was written on it, in the same transaction.
    FOREIGN KEY(owner_id, article_id) REFERENCES article(owner_id, id) ON DELETE CASCADE,
    FOREIGN KEY(owner_id, conversation_id, path_id)
        REFERENCES conversation_path(owner_id, conversation_id, id) ON DELETE CASCADE,
    FOREIGN KEY(owner_id, page_id) REFERENCES web_page(owner_id, id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX overall_note_article ON overall_note(owner_id, article_id)
    WHERE article_id IS NOT NULL;
CREATE UNIQUE INDEX overall_note_path ON overall_note(owner_id, conversation_id, path_id)
    WHERE path_id IS NOT NULL;
CREATE UNIQUE INDEX overall_note_page ON overall_note(owner_id, page_id)
    WHERE page_id IS NOT NULL;
CREATE TRIGGER overall_note_immutable_owner BEFORE UPDATE ON overall_note
    FOR EACH ROW EXECUTE FUNCTION reject_owner_change();

-- An overall note keeps an unbound page in the list just as an annotation does, so this replaces
-- the check of 0019_web_annotations.sql and also runs when a note leaves.
CREATE OR REPLACE FUNCTION require_unbound_page_annotation() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    target uuid;
BEGIN
    IF TG_TABLE_NAME = 'web_page' THEN
        target := NEW.id;
    ELSE
        target := OLD.page_id;
    END IF;
    IF EXISTS(
        SELECT 1 FROM web_page p
        WHERE p.id = target AND p.article_id IS NULL
          AND NOT EXISTS(SELECT 1 FROM web_annotation w WHERE w.page_id = p.id)
          AND NOT EXISTS(SELECT 1 FROM overall_note n WHERE n.page_id = p.id)
    ) THEN
        RAISE EXCEPTION 'unbound web page % has neither annotation nor overall note', target
            USING ERRCODE = '23514';
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER overall_note_keeps_page AFTER DELETE ON overall_note
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION require_unbound_page_annotation();
