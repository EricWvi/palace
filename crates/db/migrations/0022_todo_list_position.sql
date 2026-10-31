-- Lists of 行事 take the user's dragged order, with the same fractional-indexing keys as tasks.
-- See specs/decisions/web/todo/20261031-lists-in-dragged-order.md.
ALTER TABLE todo_list ADD COLUMN position text COLLATE "C";

-- Existing lists keep their creation order. Keys `a0`..`az`, then `b00`..`bzz`, are whole
-- integers in the library's format, so the page can insert between any two of them.
WITH ranked AS (
    SELECT id, (row_number() OVER (PARTITION BY owner_id ORDER BY id) - 1)::int AS n
    FROM todo_list
), digits(d) AS (
    VALUES ('0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz')
)
UPDATE todo_list
SET position = CASE
    WHEN n < 62 THEN 'a' || substr(d, n + 1, 1)
    ELSE 'b' || substr(d, (n - 62) / 62 + 1, 1) || substr(d, (n - 62) % 62 + 1, 1)
END
FROM ranked, digits
WHERE todo_list.id = ranked.id;

ALTER TABLE todo_list
    ALTER COLUMN position SET NOT NULL,
    ADD CHECK(position <> '' AND octet_length(position) <= 1024);
-- An owner's lists never share a key, so a key between two neighbours always exists.
CREATE UNIQUE INDEX todo_list_position ON todo_list(owner_id, position);
