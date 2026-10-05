-- Each path is one source session and carries its own title; a conversation keeps none.
ALTER TABLE conversation_path ADD COLUMN title text;
UPDATE conversation_path p SET title = c.title
FROM conversation c
WHERE c.owner_id = p.owner_id AND c.id = p.conversation_id;
ALTER TABLE conversation_path ALTER COLUMN title SET NOT NULL;
ALTER TABLE conversation_path ADD CHECK(length(btrim(title)) > 0);
ALTER TABLE conversation DROP COLUMN title;
