use std::path::Path;

use rusqlite::Connection;

use super::migrations::migrations;

#[derive(Debug, thiserror::Error)]
pub enum DbInitError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("database migration failed: {0}")]
    Migration(#[from] rusqlite_migration::Error),
}

/// Opens (creating if needed) the app database at `path`, enables foreign key
/// enforcement (off by default in SQLite), and applies all pending migrations.
pub fn open_app_db(path: &Path) -> Result<Connection, DbInitError> {
    let mut conn = Connection::open(path)?;
    init_connection(&mut conn)?;
    Ok(conn)
}

/// Opens an in-memory database with migrations applied, for tests.
#[cfg(test)]
pub fn open_in_memory() -> Result<Connection, DbInitError> {
    let mut conn = Connection::open_in_memory()?;
    init_connection(&mut conn)?;
    Ok(conn)
}

fn init_connection(conn: &mut Connection) -> Result<(), DbInitError> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrations().to_latest(conn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_cleanly_to_a_fresh_db() {
        let conn = open_in_memory().expect("migrations should apply");
        let table_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('settings','tasks','time_entries')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 3);
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let conn = open_in_memory().unwrap();
        let result = conn.execute(
            "INSERT INTO time_entries (task_id, started_at) VALUES (999, '2026-01-01T00:00:00Z')",
            [],
        );
        assert!(result.is_err(), "insert with a dangling task_id should fail");
    }
}
