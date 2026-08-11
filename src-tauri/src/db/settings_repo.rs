use rusqlite::{params, Connection, OptionalExtension};

use super::models::SettingsRow;

pub fn get_settings(conn: &Connection) -> rusqlite::Result<Option<SettingsRow>> {
    conn.query_row(
        "SELECT jira_base_url, jira_email FROM settings WHERE id = 1",
        [],
        |row| {
            Ok(SettingsRow {
                jira_base_url: row.get(0)?,
                jira_email: row.get(1)?,
            })
        },
    )
    .optional()
}

pub fn save_settings(conn: &Connection, base_url: &str, email: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (id, jira_base_url, jira_email) VALUES (1, ?1, ?2) \
         ON CONFLICT(id) DO UPDATE SET jira_base_url = excluded.jira_base_url, jira_email = excluded.jira_email",
        params![base_url, email],
    )?;
    Ok(())
}

pub fn clear_settings(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM settings WHERE id = 1", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;

    #[test]
    fn round_trips_settings() {
        let conn = open_in_memory().unwrap();
        assert!(get_settings(&conn).unwrap().is_none());

        save_settings(&conn, "https://company.atlassian.net", "me@company.com").unwrap();
        let settings = get_settings(&conn).unwrap().unwrap();
        assert_eq!(settings.jira_base_url.as_deref(), Some("https://company.atlassian.net"));
        assert_eq!(settings.jira_email.as_deref(), Some("me@company.com"));

        clear_settings(&conn).unwrap();
        assert!(get_settings(&conn).unwrap().is_none());
    }
}
