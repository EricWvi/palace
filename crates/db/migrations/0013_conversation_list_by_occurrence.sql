-- The 摘星 conversation list pages moments of one kind from the latest occurrence. The timeline
-- index leads with the time, so a per-kind list would have to skip every other kind on the way.
DROP INDEX conversation_path_recent;
CREATE INDEX moment_kind_recent ON moment(owner_id, kind, occurred_at DESC, id DESC);
