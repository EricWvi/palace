-- Preserve the user-selected conversation time independently of record creation.
ALTER TABLE conversation_import RENAME COLUMN imported_at TO occurred_at;
ALTER TABLE conversation_import ALTER COLUMN occurred_at DROP DEFAULT;

-- Historical creation times were not recorded; existing rows receive migration time.
ALTER TABLE conversation_import ADD COLUMN created_at timestamptz NOT NULL DEFAULT now();
