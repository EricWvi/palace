-- Existing conversations have one linear path. Keep all conversation/message identities.
ALTER TABLE conversation ADD UNIQUE(owner_id, id, source);
CREATE TABLE conversation_path (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owner(id),
    conversation_id uuid NOT NULL,
    source text NOT NULL,
    session_id text NOT NULL CHECK(length(session_id) > 0),
    head_message_id uuid NOT NULL,
    message_count bigint NOT NULL CHECK(message_count > 0),
    occurred_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT date_trunc('milliseconds', clock_timestamp()),
    UNIQUE(owner_id, source, session_id),
    UNIQUE(owner_id, conversation_id, id),
    FOREIGN KEY(owner_id, conversation_id, source) REFERENCES conversation(owner_id, id, source),
    FOREIGN KEY(owner_id, conversation_id, head_message_id) REFERENCES message(owner_id, conversation_id, id)
);
INSERT INTO conversation_path(id,owner_id,conversation_id,source,session_id,head_message_id,message_count,occurred_at,created_at,updated_at)
SELECT c.id,c.owner_id,c.id,c.source,c.session_id,
       (SELECT m.id FROM message m WHERE m.conversation_id=c.id ORDER BY m.created_order DESC,m.id DESC LIMIT 1),
       (SELECT count(*) FROM message m WHERE m.conversation_id=c.id),
       i.occurred_at,i.created_at,date_trunc('milliseconds',i.created_at)
FROM conversation c JOIN LATERAL (
    SELECT occurred_at,created_at FROM conversation_import WHERE conversation_id=c.id
    ORDER BY created_at DESC,id DESC LIMIT 1
) i ON true;
ALTER TABLE conversation DROP COLUMN session_id;

-- Import is an operation receipt; Path owns the current external session and occurrence time.
ALTER TABLE conversation_import ADD COLUMN path_id uuid;
UPDATE conversation_import SET path_id=conversation_id,
    result=result || jsonb_build_object('path_id',conversation_id);
ALTER TABLE conversation_import ALTER COLUMN path_id SET NOT NULL;
ALTER TABLE conversation_import ADD FOREIGN KEY(owner_id,conversation_id,path_id)
    REFERENCES conversation_path(owner_id,conversation_id,id);
CREATE TRIGGER path_immutable_owner BEFORE UPDATE ON conversation_path
    FOR EACH ROW EXECUTE FUNCTION reject_owner_change();
