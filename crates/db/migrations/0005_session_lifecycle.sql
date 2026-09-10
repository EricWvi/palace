CREATE FUNCTION enforce_session_lifecycle() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.id IS DISTINCT FROM OLD.id OR NEW.owner_identity_id IS DISTINCT FROM OLD.owner_identity_id OR NEW.created_at IS DISTINCT FROM OLD.created_at THEN
        RAISE EXCEPTION 'session identity is immutable' USING ERRCODE='23514';
    END IF;
    IF OLD.revoked_at IS NOT NULL AND NEW.revoked_at IS DISTINCT FROM OLD.revoked_at THEN
        RAISE EXCEPTION 'session revocation is irreversible' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER session_lifecycle BEFORE UPDATE ON owner_session FOR EACH ROW EXECUTE FUNCTION enforce_session_lifecycle();
