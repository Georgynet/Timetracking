CREATE TABLE work_days (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    work_date TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT
);

CREATE INDEX idx_work_days_date ON work_days(work_date);

-- At most one open workday at a time, same pattern as idx_time_entries_single_running.
CREATE UNIQUE INDEX idx_work_days_single_running ON work_days((1)) WHERE ended_at IS NULL;

CREATE TABLE work_breaks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    work_day_id INTEGER NOT NULL REFERENCES work_days(id),
    started_at TEXT NOT NULL,
    ended_at TEXT
);

CREATE INDEX idx_work_breaks_work_day_id ON work_breaks(work_day_id);

-- At most one open break at a time (there can only be one open workday anyway).
CREATE UNIQUE INDEX idx_work_breaks_single_running ON work_breaks((1)) WHERE ended_at IS NULL;
