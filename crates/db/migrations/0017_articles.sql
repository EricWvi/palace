-- 摘星 articles: web articles imported with their own media, each a moment on the day it was
-- imported. See specs/decisions/web/stars/20261020-articles-imported-with-media.md.
ALTER TABLE moment DROP CONSTRAINT moment_kind_check;
ALTER TABLE moment ADD CONSTRAINT moment_kind_check CHECK(kind IN ('conversation', 'article'));

CREATE TABLE article (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    kind text NOT NULL DEFAULT 'article' CHECK(kind = 'article'),
    title text NOT NULL CHECK(btrim(title) <> '' AND octet_length(title) <= 1024),
    -- The body is written once at import; image addresses inside it point at its own media.
    content text NOT NULL CHECK(btrim(content) <> ''),
    -- Kept exactly as imported: a retried import must find the article by the same bytes.
    url text NOT NULL CHECK(url <> '' AND octet_length(url) <= 2048),
    author text CHECK(btrim(author) <> '' AND octet_length(author) <= 256),
    published_at timestamptz NOT NULL
        CHECK(published_at = date_trunc('milliseconds', published_at)),
    -- 'day' when the importer gave only a date, read as midnight in the deployment's zone.
    published_precision text NOT NULL CHECK(published_precision IN ('day', 'instant')),
    UNIQUE(owner_id, url),
    UNIQUE(owner_id, id),
    FOREIGN KEY(owner_id, id, kind) REFERENCES moment(owner_id, id, kind)
);
CREATE TRIGGER article_immutable_owner BEFORE UPDATE ON article
    FOR EACH ROW EXECUTE FUNCTION reject_owner_change();
CREATE FUNCTION reject_article_content_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.content IS DISTINCT FROM OLD.content THEN
        RAISE EXCEPTION 'an article body is never edited' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER article_immutable_content BEFORE UPDATE ON article
    FOR EACH ROW EXECUTE FUNCTION reject_article_content_change();
-- The 摘星 list pages by publish date, newest first.
CREATE INDEX article_published ON article(owner_id, published_at DESC, id DESC);
-- Substring search over bodies, as for messages (0011_conversation_list.sql).
CREATE INDEX article_content_trgm ON article USING gin (content gin_trgm_ops);

-- The media an article embeds. A media row belongs to at most one article, and only rows the
-- import itself created ever do, so deleting the article can delete its media with it.
CREATE TABLE article_media (
    owner_id uuid NOT NULL,
    article_id uuid NOT NULL,
    media_id uuid NOT NULL UNIQUE,
    PRIMARY KEY(article_id, media_id),
    FOREIGN KEY(owner_id, article_id) REFERENCES article(owner_id, id),
    FOREIGN KEY(owner_id, media_id) REFERENCES media(owner_id, id)
);

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
    ) THEN
        RAISE EXCEPTION 'moment % has no detail of its kind', target;
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER article_detail_keeps_moment AFTER DELETE ON article
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION require_moment_detail();
