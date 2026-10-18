-- A media file's bytes live in object storage; this row is its owner-scoped identity and the
-- internal key that locates the object. The id is the only reference ever handed out, so the
-- key can differ from the id (imported legacy objects keep their original key).
CREATE TABLE media (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    object_key text NOT NULL UNIQUE CHECK(object_key <> ''),
    content_type text NOT NULL CHECK(content_type <> ''),
    byte_size bigint NOT NULL CHECK(byte_size >= 0),
    sha256 bytea NOT NULL CHECK(octet_length(sha256) = 32),
    file_name text CHECK(file_name <> ''),
    created_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(created_at = date_trunc('milliseconds', created_at)),
    UNIQUE(owner_id, id)
);
CREATE TRIGGER media_immutable_owner BEFORE UPDATE ON media
    FOR EACH ROW EXECUTE FUNCTION reject_owner_change();
