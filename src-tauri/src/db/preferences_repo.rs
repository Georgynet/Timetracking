use rusqlite::{params, Connection, OptionalExtension};

/// Reads one preference, or `None` if it was never set.
pub fn get(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM preferences WHERE key = ?1", params![key], |row| {
        row.get(0)
    })
    .optional()
}

pub fn set(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO preferences (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// Reads a preference as an `i64`, falling back to `default` when it is missing or
/// isn't a number — a preference is a convenience, never a reason to fail a read.
pub fn get_i64(conn: &Connection, key: &str, default: i64) -> rusqlite::Result<i64> {
    Ok(get(conn, key)?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(default))
}

pub fn set_i64(conn: &Connection, key: &str, value: i64) -> rusqlite::Result<()> {
    set(conn, key, &value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;

    #[test]
    fn round_trips_and_overwrites_a_value() {
        let conn = open_in_memory().unwrap();
        assert_eq!(get(&conn, "rows").unwrap(), None);

        set(&conn, "rows", "5").unwrap();
        assert_eq!(get(&conn, "rows").unwrap().as_deref(), Some("5"));

        set(&conn, "rows", "9").unwrap();
        assert_eq!(get(&conn, "rows").unwrap().as_deref(), Some("9"));
    }

    #[test]
    fn a_missing_or_unparseable_number_falls_back_to_the_default() {
        let conn = open_in_memory().unwrap();
        assert_eq!(get_i64(&conn, "rows", 4).unwrap(), 4);

        set(&conn, "rows", "not a number").unwrap();
        assert_eq!(get_i64(&conn, "rows", 4).unwrap(), 4, "a junk value must not break the read");

        set_i64(&conn, "rows", 7).unwrap();
        assert_eq!(get_i64(&conn, "rows", 4).unwrap(), 7);
    }
}
