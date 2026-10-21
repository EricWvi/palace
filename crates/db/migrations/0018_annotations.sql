-- Reader annotations on article bodies and conversation messages. Both texts never change once
-- stored, so an annotation anchors to character offsets in the rendered text, with the selected
-- text kept to check them. See specs/decisions/web/stars/20261021-annotations-on-articles-and-messages.md.
CREATE TABLE annotation (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    article_id uuid,
    conversation_id uuid,
    message_id uuid,
    -- UTF-16 code units into the rendered text of the article body or the message.
    start_offset integer NOT NULL CHECK(start_offset >= 0),
    end_offset integer NOT NULL,
    quote text NOT NULL CHECK(btrim(quote) <> ''),
    style text CHECK(style IN ('yellow', 'green', 'blue', 'rose', 'underline', 'wavy')),
    note text CHECK(btrim(note) <> ''),
    created_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(created_at = date_trunc('milliseconds', created_at)),
    updated_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(updated_at = date_trunc('milliseconds', updated_at)),
    CHECK(start_offset < end_offset),
    -- A mark with neither shows nothing on the page.
    CHECK(style IS NOT NULL OR note IS NOT NULL),
    -- Exactly one target: an article body, or one message of one conversation.
    CHECK((article_id IS NOT NULL AND conversation_id IS NULL AND message_id IS NULL)
        OR (article_id IS NULL AND conversation_id IS NOT NULL AND message_id IS NOT NULL)),
    -- Deleting the text deletes what was written on it, in the same transaction.
    FOREIGN KEY(owner_id, article_id) REFERENCES article(owner_id, id) ON DELETE CASCADE,
    FOREIGN KEY(owner_id, conversation_id, message_id)
        REFERENCES message(owner_id, conversation_id, id) ON DELETE CASCADE
);
CREATE INDEX annotation_article ON annotation(owner_id, article_id, id) WHERE article_id IS NOT NULL;
CREATE INDEX annotation_conversation ON annotation(owner_id, conversation_id, id)
    WHERE conversation_id IS NOT NULL;
CREATE INDEX annotation_message ON annotation(message_id) WHERE message_id IS NOT NULL;

-- Only the style and the note change; the anchor and the target are what the annotation is.
CREATE FUNCTION reject_annotation_anchor_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.owner_id IS DISTINCT FROM OLD.owner_id
        OR NEW.article_id IS DISTINCT FROM OLD.article_id
        OR NEW.conversation_id IS DISTINCT FROM OLD.conversation_id
        OR NEW.message_id IS DISTINCT FROM OLD.message_id
        OR NEW.start_offset IS DISTINCT FROM OLD.start_offset
        OR NEW.end_offset IS DISTINCT FROM OLD.end_offset
        OR NEW.quote IS DISTINCT FROM OLD.quote THEN
        RAISE EXCEPTION 'an annotation keeps its anchor and target' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER annotation_immutable_anchor BEFORE UPDATE ON annotation
    FOR EACH ROW EXECUTE FUNCTION reject_annotation_anchor_change();
