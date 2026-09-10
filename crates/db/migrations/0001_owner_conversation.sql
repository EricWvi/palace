CREATE TABLE owner (
    id uuid PRIMARY KEY,
    email text NOT NULL,
    normalized_email text NOT NULL,
    disabled boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX owner_active_email ON owner(normalized_email) WHERE NOT disabled;
CREATE TABLE owner_identity (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    issuer text NOT NULL,
    subject text NOT NULL,
    disabled boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(issuer,subject), UNIQUE(owner_id,id)
);
CREATE UNIQUE INDEX owner_active_identity ON owner_identity(owner_id) WHERE NOT disabled;
CREATE TABLE conversation (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    title text NOT NULL CHECK(length(btrim(title)) > 0),
    source text NOT NULL CHECK(source IN ('chatgpt','gemini','grok')),
    session_id text NOT NULL CHECK(length(session_id) > 0),
    UNIQUE(owner_id,id), UNIQUE(owner_id,source,session_id)
);
CREATE TABLE message (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    conversation_id uuid NOT NULL,
    parent_message_id uuid,
    role text NOT NULL CHECK(role IN ('user','assistant')),
    content text NOT NULL,
    created_order bigint GENERATED ALWAYS AS IDENTITY,
    UNIQUE(owner_id,conversation_id,id),
    FOREIGN KEY(owner_id,conversation_id) REFERENCES conversation(owner_id,id),
    FOREIGN KEY(owner_id,conversation_id,parent_message_id) REFERENCES message(owner_id,conversation_id,id),
    CHECK(id IS DISTINCT FROM parent_message_id)
);
CREATE INDEX message_parent ON message(owner_id,conversation_id,parent_message_id,created_order,id);
CREATE TABLE conversation_import (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    conversation_id uuid NOT NULL,
    head_message_id uuid NOT NULL,
    input_digest bytea NOT NULL,
    message_count bigint NOT NULL CHECK(message_count > 0),
    idempotency_key text NOT NULL,
    result jsonb NOT NULL,
    imported_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(owner_id,idempotency_key),
    FOREIGN KEY(owner_id,conversation_id) REFERENCES conversation(owner_id,id),
    FOREIGN KEY(owner_id,conversation_id,head_message_id) REFERENCES message(owner_id,conversation_id,id)
);
-- Identity and tree structure are immutable until a later decision defines structural edits.
CREATE FUNCTION reject_owner_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.owner_id IS DISTINCT FROM OLD.owner_id THEN
        RAISE EXCEPTION 'owner assignment is immutable' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER owner_identity_immutable_owner BEFORE UPDATE ON owner_identity FOR EACH ROW EXECUTE FUNCTION reject_owner_change();
CREATE TRIGGER conversation_immutable_owner BEFORE UPDATE ON conversation FOR EACH ROW EXECUTE FUNCTION reject_owner_change();
CREATE TRIGGER message_immutable_owner BEFORE UPDATE ON message FOR EACH ROW EXECUTE FUNCTION reject_owner_change();
CREATE TRIGGER import_immutable_owner BEFORE UPDATE ON conversation_import FOR EACH ROW EXECUTE FUNCTION reject_owner_change();
CREATE FUNCTION reject_message_structure_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.id IS DISTINCT FROM OLD.id OR NEW.conversation_id IS DISTINCT FROM OLD.conversation_id OR NEW.parent_message_id IS DISTINCT FROM OLD.parent_message_id THEN
        RAISE EXCEPTION 'message structure is immutable' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER message_immutable_structure BEFORE UPDATE ON message FOR EACH ROW EXECUTE FUNCTION reject_message_structure_change();
-- Immediate parent existence and immutable IDs/parents make multi-row cycles impossible too.
CREATE FUNCTION require_existing_parent() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.parent_message_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM message WHERE owner_id=NEW.owner_id AND conversation_id=NEW.conversation_id AND id=NEW.parent_message_id) THEN
        RAISE EXCEPTION 'parent must exist before child' USING ERRCODE='23503';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER message_parent_exists BEFORE INSERT ON message FOR EACH ROW EXECUTE FUNCTION require_existing_parent();
