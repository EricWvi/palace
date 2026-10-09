-- The 摘星 conversation list pages paths from the most recently changed one.
CREATE INDEX conversation_path_recent ON conversation_path(owner_id, updated_at DESC, id DESC);

-- Searching runs an unanchored ILIKE over message bodies, the largest data in the database.
-- Trigrams let it skip non-matching messages; matching is still decided by ILIKE, so a query too
-- short to yield a trigram only loses the speed-up, never a result. pg_trgm is a trusted
-- extension, so the database owner can create it without superuser rights.
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE INDEX message_content_trgm ON message USING gin (content gin_trgm_ops);
