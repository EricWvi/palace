-- A task finished on the day it is planned on keeps that day, done, so 今日 still lists it,
-- struck through. Any other finished task leaves its plan behind as before.
-- See specs/decisions/web/todo/20261101-finish-on-its-day-and-stay-in-today.md.
ALTER TABLE todo DROP CONSTRAINT todo_check3;
ALTER TABLE todo ADD CONSTRAINT todo_finished_plan CHECK(
    completed_at IS NULL
    OR (NOT shelved AND ((planned_on IS NULL AND NOT done) OR (planned_on IS NOT NULL AND done)))
);
-- 今日 reads the tasks finished on one day.
CREATE INDEX todo_finished_on ON todo(owner_id, planned_on)
    WHERE completed_at IS NOT NULL AND planned_on IS NOT NULL;
