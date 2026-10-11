-- Every timestamp leaves the API as epoch milliseconds. Storing exactly that precision means a
-- value a client reads back compares equal to the stored one, which keyset cursors rely on.
-- Early imports took `now()` and still carry microseconds; truncation moves them by under 1 ms.
-- Rewriting moments queues their deferred detail check, and ALTER TABLE refuses a table with
-- pending trigger events, so the checks run per statement here.
SET CONSTRAINTS ALL IMMEDIATE;
UPDATE owner SET created_at = date_trunc('milliseconds', created_at),
    updated_at = date_trunc('milliseconds', updated_at);
ALTER TABLE owner
    ALTER COLUMN created_at SET DEFAULT date_trunc('milliseconds', now()),
    ALTER COLUMN updated_at SET DEFAULT date_trunc('milliseconds', now()),
    ADD CHECK(created_at = date_trunc('milliseconds', created_at)),
    ADD CHECK(updated_at = date_trunc('milliseconds', updated_at));

UPDATE owner_identity SET created_at = date_trunc('milliseconds', created_at),
    last_seen_at = date_trunc('milliseconds', last_seen_at);
ALTER TABLE owner_identity
    ALTER COLUMN created_at SET DEFAULT date_trunc('milliseconds', now()),
    ALTER COLUMN last_seen_at SET DEFAULT date_trunc('milliseconds', now()),
    ADD CHECK(created_at = date_trunc('milliseconds', created_at)),
    ADD CHECK(last_seen_at = date_trunc('milliseconds', last_seen_at));

UPDATE conversation_import SET occurred_at = date_trunc('milliseconds', occurred_at),
    created_at = date_trunc('milliseconds', created_at);
ALTER TABLE conversation_import
    ALTER COLUMN created_at SET DEFAULT date_trunc('milliseconds', now()),
    ADD CHECK(occurred_at = date_trunc('milliseconds', occurred_at)),
    ADD CHECK(created_at = date_trunc('milliseconds', created_at));

UPDATE conversation_path SET created_at = date_trunc('milliseconds', created_at);
ALTER TABLE conversation_path
    ALTER COLUMN created_at SET DEFAULT date_trunc('milliseconds', now()),
    ADD CHECK(created_at = date_trunc('milliseconds', created_at)),
    ADD CHECK(updated_at = date_trunc('milliseconds', updated_at));

UPDATE moment SET occurred_at = date_trunc('milliseconds', occurred_at);
ALTER TABLE moment ADD CHECK(occurred_at = date_trunc('milliseconds', occurred_at));
