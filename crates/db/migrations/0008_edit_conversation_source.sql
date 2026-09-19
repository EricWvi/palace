ALTER TABLE conversation_path
    DROP CONSTRAINT conversation_path_owner_id_conversation_id_source_fkey;
ALTER TABLE conversation_path
    ADD FOREIGN KEY(owner_id,conversation_id,source)
    REFERENCES conversation(owner_id,id,source)
    ON UPDATE CASCADE;
