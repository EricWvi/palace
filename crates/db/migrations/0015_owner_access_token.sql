-- An access token authenticates CLI and service callers as its owner. Only the digest of the
-- token is stored, plus a masked form computed at creation for display; the full value cannot
-- be recovered from the database. Deleting the row is the revocation.
CREATE TABLE owner_access_token (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    owner_identity_id uuid NOT NULL,
    note text NOT NULL CHECK(note <> '' AND char_length(note) <= 100),
    secret_hash bytea NOT NULL UNIQUE CHECK(octet_length(secret_hash) = 32),
    masked text NOT NULL CHECK(masked <> ''),
    created_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', now())
        CHECK(created_at = date_trunc('milliseconds', created_at)),
    last_used_at timestamptz CHECK(last_used_at = date_trunc('milliseconds', last_used_at)),
    -- Disabling the identity a token was issued under must reach the token, as it does sessions.
    FOREIGN KEY(owner_id, owner_identity_id) REFERENCES owner_identity(owner_id, id),
    UNIQUE(owner_id, id)
);
CREATE INDEX owner_access_token_list ON owner_access_token(owner_id, created_at DESC, id DESC);
CREATE TRIGGER access_token_immutable_owner BEFORE UPDATE ON owner_access_token
    FOR EACH ROW EXECUTE FUNCTION reject_owner_change();
