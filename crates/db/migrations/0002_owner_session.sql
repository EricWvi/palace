CREATE TABLE owner_session (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    owner_identity_id uuid NOT NULL,
    secret_hash bytea NOT NULL UNIQUE,
    previous_secret_hash bytea,
    previous_valid_until bigint,
    generation bigint NOT NULL DEFAULT 0,
    created_at bigint NOT NULL,
    last_seen_at bigint NOT NULL,
    last_identity_verified_at bigint NOT NULL,
    refresh_credential bytea NOT NULL,
    rotated_at bigint NOT NULL,
    revoked_at bigint,
    revoke_reason text,
    revocation_pending boolean NOT NULL DEFAULT false,
    FOREIGN KEY(owner_id,owner_identity_id) REFERENCES owner_identity(owner_id,id)
);
CREATE INDEX owner_session_identity ON owner_session(owner_id,owner_identity_id);
CREATE INDEX owner_session_previous ON owner_session(previous_secret_hash);
CREATE TRIGGER session_immutable_owner BEFORE UPDATE ON owner_session FOR EACH ROW EXECUTE FUNCTION reject_owner_change();
-- Local disablement persists revocation before an external provider can be contacted.
CREATE FUNCTION revoke_disabled_sessions() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.disabled AND NOT OLD.disabled THEN
        IF TG_TABLE_NAME = 'owner' THEN
            UPDATE owner_session SET revoked_at=extract(epoch FROM now())::bigint,revoke_reason='owner disabled',revocation_pending=true WHERE owner_id=NEW.id AND revoked_at IS NULL;
        ELSE
            UPDATE owner_session SET revoked_at=extract(epoch FROM now())::bigint,revoke_reason='identity disabled',revocation_pending=true WHERE owner_identity_id=NEW.id AND revoked_at IS NULL;
        END IF;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER owner_disable_sessions AFTER UPDATE ON owner FOR EACH ROW EXECUTE FUNCTION revoke_disabled_sessions();
CREATE TRIGGER identity_disable_sessions AFTER UPDATE ON owner_identity FOR EACH ROW EXECUTE FUNCTION revoke_disabled_sessions();
