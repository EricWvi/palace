-- Edits and deletions made on the web to committed jottings, written to the work tree and
-- published at once but not yet in any commit. Reconciliation reads the work tree only for
-- these files, to keep the web's result until a commit carries it or the work tree undoes it.
-- Uploads never committed have no row: they stay in inbox/ and follow the upload rules.
-- See specs/decisions/server/jotting/20261101-edit-and-delete-jottings-from-the-web-in-the-work-tree-until-committed.md.
CREATE TABLE jotting_web_write (
    jotting_id uuid PRIMARY KEY REFERENCES jotting(id),
    -- Where the work tree holds the web's result: the file written, or the file deleted.
    path text NOT NULL CHECK(path <> ''),
    -- The jotting's blob in the commit the first web write started from; later writes keep it.
    base_oid bytea NOT NULL CHECK(octet_length(base_oid) > 0),
    -- The blob of the file the web wrote last; null when the web deleted the jotting.
    written_oid bytea CHECK(octet_length(written_oid) > 0),
    written_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(written_at = date_trunc('milliseconds', written_at))
);
