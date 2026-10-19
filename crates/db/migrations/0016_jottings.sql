-- 札记 (jottings) are Markdown files in a Git repository; this table is the server's index of
-- them and the source of their sync feed. Jottings belong to the deployment, not to an owner.
-- See specs/decisions/server/jotting/0-jottings-indexed-from-git-commits.md.
CREATE SEQUENCE jotting_server_version AS bigint START WITH 1 INCREMENT BY 1 NO CYCLE CACHE 1;
CREATE TABLE jotting (
    id uuid PRIMARY KEY,
    server_version bigint NOT NULL,
    -- pending: uploaded and written to inbox/, not yet in any indexed commit;
    -- committed: as found in the indexed commit; deleted: a tombstone kept for sync.
    state text NOT NULL CHECK(state IN ('pending', 'committed', 'deleted')),
    -- Digest of the first upload, set only for uploaded jottings and never changed, so a retry
    -- is recognized after the agent edits, moves or deletes the jotting.
    upload_digest bytea CHECK(octet_length(upload_digest) = 32),
    -- Where the file is, relative to the jottings folder, and the Git blob id of its bytes.
    path text CHECK(path <> ''),
    blob_oid bytea CHECK(octet_length(blob_oid) > 0),
    created_at timestamptz CHECK(created_at = date_trunc('milliseconds', created_at)),
    title text CHECK(title <> ''),
    tags text[],
    body text,
    CHECK(state <> 'pending' OR upload_digest IS NOT NULL),
    -- A tombstone keeps nothing but its id; every live jotting has its file and content.
    CHECK(CASE WHEN state = 'deleted'
        THEN path IS NULL AND blob_oid IS NULL AND created_at IS NULL AND title IS NULL
            AND tags IS NULL AND body IS NULL
        ELSE path IS NOT NULL AND blob_oid IS NOT NULL AND created_at IS NOT NULL
            AND tags IS NOT NULL AND body IS NOT NULL
    END)
);
CREATE INDEX jotting_pull ON jotting(server_version);

-- Versions are allocated under one transaction lock held to commit, so versions become visible
-- in increasing order and a client cursor never skips a late commit. Only a change clients can
-- see takes a version: moving a file or committing an uploaded jotting unchanged keeps it.
CREATE FUNCTION publish_jotting() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(734281002);
    IF TG_OP = 'UPDATE' THEN
        IF NEW.id IS DISTINCT FROM OLD.id OR NEW.upload_digest IS DISTINCT FROM OLD.upload_digest THEN
            RAISE EXCEPTION 'jotting identity and upload digest are immutable' USING ERRCODE = '23514';
        END IF;
        IF (NEW.state = 'deleted') = (OLD.state = 'deleted')
            AND NEW.created_at IS NOT DISTINCT FROM OLD.created_at
            AND NEW.title IS NOT DISTINCT FROM OLD.title
            AND NEW.tags IS NOT DISTINCT FROM OLD.tags
            AND NEW.body IS NOT DISTINCT FROM OLD.body THEN
            NEW.server_version := OLD.server_version;
            RETURN NEW;
        END IF;
    END IF;
    NEW.server_version := nextval('jotting_server_version');
    RETURN NEW;
END $$;
CREATE TRIGGER jotting_publish BEFORE INSERT OR UPDATE ON jotting
    FOR EACH ROW EXECUTE FUNCTION publish_jotting();

CREATE FUNCTION reject_jotting_delete() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'keep a jotting tombstone instead of deleting the row' USING ERRCODE = '23514';
END $$;
CREATE TRIGGER jotting_keep_tombstone BEFORE DELETE ON jotting
    FOR EACH ROW EXECUTE FUNCTION reject_jotting_delete();
