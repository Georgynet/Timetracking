-- Free-form key/value store for UI preferences. Deliberately not columns on
-- `settings`: that row is the Jira connection, and every new preference here would
-- otherwise cost a migration. Values are stored as text and parsed by the reader,
-- which owns the default when a key is missing or unreadable.
CREATE TABLE preferences (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
