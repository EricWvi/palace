CREATE SEQUENCE business_server_version AS bigint START WITH 1 INCREMENT BY 1 NO CYCLE CACHE 1;
CREATE TABLE sync_record (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    updated_at bigint NOT NULL,
    is_deleted boolean NOT NULL,
    body jsonb NOT NULL,
    server_version bigint NOT NULL,
    UNIQUE(owner_id,id)
);
CREATE INDEX sync_record_pull ON sync_record(owner_id,server_version);
CREATE TRIGGER sync_record_immutable_owner BEFORE UPDATE ON sync_record FOR EACH ROW EXECUTE FUNCTION reject_owner_change();
-- Even administrative writes cannot publish outside the same commit-order lock or choose a version.
CREATE FUNCTION publish_sync_record() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(734281001);
    IF TG_OP='UPDATE' THEN
        IF NEW.id IS DISTINCT FROM OLD.id THEN RAISE EXCEPTION 'record identity is immutable' USING ERRCODE='23514'; END IF;
        IF NEW.updated_at<=OLD.updated_at THEN RETURN NULL; END IF;
    END IF;
    NEW.server_version:=nextval('business_server_version');
    RETURN NEW;
END $$;
CREATE TRIGGER sync_record_publish BEFORE INSERT OR UPDATE ON sync_record FOR EACH ROW EXECUTE FUNCTION publish_sync_record();
CREATE FUNCTION reject_sync_delete() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'retain a tombstone instead of deleting sync records' USING ERRCODE='23514';
END $$;
CREATE TRIGGER sync_record_keep_tombstone BEFORE DELETE ON sync_record FOR EACH ROW EXECUTE FUNCTION reject_sync_delete();
