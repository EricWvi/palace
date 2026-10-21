-- Annotations the browser extension leaves on web pages, kept by page address until the reader
-- binds the page to an imported article and the article page migrates them. See
-- specs/decisions/extension/0-web-page-annotations-bound-and-migrated-into-articles.md.
CREATE TABLE web_page (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    -- The address without its fragment, kept as the browser showed it.
    url text NOT NULL CHECK(url <> '' AND octet_length(url) <= 2048),
    -- The tab title when the page was last annotated or bound.
    title text NOT NULL CHECK(btrim(title) <> '' AND octet_length(title) <= 1024),
    -- NULL while the page takes annotations of its own; afterwards the article they move to.
    article_id uuid,
    created_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(created_at = date_trunc('milliseconds', created_at)),
    updated_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(updated_at = date_trunc('milliseconds', updated_at)),
    UNIQUE(owner_id, url),
    UNIQUE(owner_id, id),
    -- Deleting the article deletes the pages bound to it and what was left on them.
    FOREIGN KEY(owner_id, article_id) REFERENCES article(owner_id, id) ON DELETE CASCADE
);
CREATE INDEX web_page_article ON web_page(owner_id, article_id) WHERE article_id IS NOT NULL;
CREATE TRIGGER web_page_immutable_owner BEFORE UPDATE ON web_page
    FOR EACH ROW EXECUTE FUNCTION reject_owner_change();

CREATE TABLE web_annotation (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL,
    page_id uuid NOT NULL,
    quote text NOT NULL CHECK(btrim(quote) <> ''),
    -- At most 32 UTF-16 code units each, checked by the domain; empty at either end of the page.
    prefix text NOT NULL CHECK(char_length(prefix) <= 32),
    suffix text NOT NULL CHECK(char_length(suffix) <= 32),
    -- Where the quote starts as a fraction of the page text.
    position double precision NOT NULL CHECK(position >= 0 AND position <= 1),
    style text CHECK(style IN ('yellow', 'green', 'blue', 'rose', 'underline', 'wavy')),
    note text CHECK(btrim(note) <> ''),
    created_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(created_at = date_trunc('milliseconds', created_at)),
    updated_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(updated_at = date_trunc('milliseconds', updated_at)),
    CHECK(style IS NOT NULL OR note IS NOT NULL),
    FOREIGN KEY(owner_id, page_id) REFERENCES web_page(owner_id, id) ON DELETE CASCADE
);
CREATE INDEX web_annotation_page ON web_annotation(owner_id, page_id, id);

-- As for article annotations, only the style and the note change.
CREATE FUNCTION reject_web_annotation_anchor_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.owner_id IS DISTINCT FROM OLD.owner_id
        OR NEW.page_id IS DISTINCT FROM OLD.page_id
        OR NEW.quote IS DISTINCT FROM OLD.quote
        OR NEW.prefix IS DISTINCT FROM OLD.prefix
        OR NEW.suffix IS DISTINCT FROM OLD.suffix
        OR NEW.position IS DISTINCT FROM OLD.position THEN
        RAISE EXCEPTION 'a web annotation keeps its anchor and page' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER web_annotation_immutable_anchor BEFORE UPDATE ON web_annotation
    FOR EACH ROW EXECUTE FUNCTION reject_web_annotation_anchor_change();

-- An unbound page exists only for its annotations, so the extension's list never shows an empty
-- one. Writers delete the page with its last annotation; this check, run at commit like the
-- moment detail check (0009_moments.sql), catches any writer that forgets.
CREATE FUNCTION require_unbound_page_annotation() RETURNS trigger LANGUAGE plpgsql AS $$
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
    ) THEN
        RAISE EXCEPTION 'unbound web page % has no annotation', target USING ERRCODE = '23514';
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER web_page_requires_annotation AFTER INSERT OR UPDATE ON web_page
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION require_unbound_page_annotation();
CREATE CONSTRAINT TRIGGER web_annotation_keeps_page AFTER DELETE ON web_annotation
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION require_unbound_page_annotation();
