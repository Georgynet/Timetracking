CREATE TABLE settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    jira_base_url TEXT,
    jira_email TEXT
);

CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    jira_key TEXT NOT NULL UNIQUE,
    summary TEXT NOT NULL,
    is_favorite INTEGER NOT NULL DEFAULT 0,
    is_assigned_to_me INTEGER NOT NULL DEFAULT 0,
    last_synced_at TEXT
);

CREATE TABLE time_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL REFERENCES tasks(id),
    started_at TEXT NOT NULL,
    ended_at TEXT,
    duration_seconds INTEGER,
    comment TEXT,
    is_synced INTEGER NOT NULL DEFAULT 0,
    jira_worklog_id TEXT,
    created_manually INTEGER NOT NULL DEFAULT 0,
    edited_at TEXT
);

CREATE INDEX idx_time_entries_task_id ON time_entries(task_id);
CREATE INDEX idx_time_entries_unsynced ON time_entries(is_synced) WHERE is_synced = 0;

-- At most one running timer at a time: every row with ended_at IS NULL indexes the
-- same constant, so a second concurrent insert collides. App logic in timer::engine
-- is the primary enforcement point; this is a DB-level backstop.
CREATE UNIQUE INDEX idx_time_entries_single_running ON time_entries((1)) WHERE ended_at IS NULL;
