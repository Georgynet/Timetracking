use std::sync::Mutex;

use rusqlite::Connection;

use crate::db::time_entries_repo;
use crate::jira::JiraClient;

#[derive(Debug, Clone)]
pub struct SyncOutcome {
    pub entry_id: i64,
    pub task_key: String,
    /// `Ok(jira_worklog_id)` on success, `Err(message)` on failure — the row is left
    /// completely untouched on failure, so a later retry naturally re-selects it.
    pub result: Result<String, String>,
}

/// Pushes every pending (`is_synced = 0`) entry to Jira, oldest first. A failing
/// record (network error, invalid ticket, etc.) is recorded and skipped — it is never
/// mutated, and the rest of the batch keeps going. The DB mutex is only held for the
/// brief read/write around each request, never across the network `.await`, so the
/// rest of the app isn't blocked while a sync is in flight.
pub async fn sync_all(conn: &Mutex<Connection>, client: &dyn JiraClient) -> Vec<SyncOutcome> {
    let pending = {
        let guard = conn.lock().unwrap();
        time_entries_repo::list_pending_sync(&guard).unwrap_or_default()
    };

    let mut outcomes = Vec::with_capacity(pending.len());
    for entry in pending {
        let comment = entry.comment.as_deref();
        let seconds = entry.duration_seconds.unwrap_or(0);

        let result = match &entry.jira_worklog_id {
            Some(worklog_id) => {
                client
                    .update_worklog(&entry.task_key, worklog_id, entry.started_at, seconds, comment)
                    .await
            }
            None => {
                client
                    .add_worklog(&entry.task_key, entry.started_at, seconds, comment)
                    .await
            }
        };

        let outcome = match result {
            Ok(worklog) => {
                let guard = conn.lock().unwrap();
                let _ = time_entries_repo::mark_synced(&guard, entry.id, &worklog.id);
                SyncOutcome { entry_id: entry.id, task_key: entry.task_key.clone(), result: Ok(worklog.id) }
            }
            Err(e) => SyncOutcome {
                entry_id: entry.id,
                task_key: entry.task_key.clone(),
                result: Err(e.to_string()),
            },
        };
        outcomes.push(outcome);
    }
    outcomes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;
    use crate::db::tasks_repo;
    use crate::db::time_entries_repo::{self, EntryUpdate};
    use crate::jira::fake_client::FakeJiraClient;
    use crate::jira::models::JiraError;
    use chrono::{TimeZone, Utc};

    fn now() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 10, 12, 0, 0).unwrap()
    }

    #[tokio::test]
    async fn a_never_synced_entry_is_created_via_add_worklog() {
        let conn = Mutex::new(open_in_memory().unwrap());
        let task_id = {
            let guard = conn.lock().unwrap();
            tasks_repo::upsert_favorite_task(&guard, "PROJ-1", "Ticket", now()).unwrap().id
        };
        let entry_id = {
            let guard = conn.lock().unwrap();
            time_entries_repo::insert_manual(&guard, task_id, now(), now(), 3600, None).unwrap().id
        };

        let client = FakeJiraClient::default();
        let outcomes = sync_all(&conn, &client).await;

        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].result.is_ok());
        assert_eq!(outcomes[0].entry_id, entry_id);

        let calls = client.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].worklog_id.is_none(), "a never-synced entry must POST, not PUT");

        let guard = conn.lock().unwrap();
        let reloaded = time_entries_repo::get_by_id(&guard, entry_id).unwrap().unwrap();
        assert!(reloaded.is_synced);
        assert!(reloaded.jira_worklog_id.is_some());
    }

    #[tokio::test]
    async fn editing_a_synced_entry_then_syncing_sends_an_update_not_a_duplicate() {
        let conn = Mutex::new(open_in_memory().unwrap());
        let task_id = {
            let guard = conn.lock().unwrap();
            tasks_repo::upsert_favorite_task(&guard, "PROJ-1", "Ticket", now()).unwrap().id
        };
        let entry_id = {
            let guard = conn.lock().unwrap();
            let entry = time_entries_repo::insert_manual(&guard, task_id, now(), now(), 3600, None).unwrap();
            time_entries_repo::mark_synced(&guard, entry.id, "existing-worklog-1").unwrap();
            time_entries_repo::update_entry(
                &guard,
                entry.id,
                EntryUpdate { duration_seconds: Some(1800), ..Default::default() },
                now(),
            )
            .unwrap();
            entry.id
        };

        let client = FakeJiraClient::default();
        let outcomes = sync_all(&conn, &client).await;

        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].result.is_ok());

        let calls = client.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].worklog_id.as_deref(), Some("existing-worklog-1"), "must PUT the existing worklog id");

        let guard = conn.lock().unwrap();
        let reloaded = time_entries_repo::get_by_id(&guard, entry_id).unwrap().unwrap();
        assert!(reloaded.is_synced);
    }

    #[tokio::test]
    async fn one_failure_does_not_block_the_rest_of_the_batch() {
        let conn = Mutex::new(open_in_memory().unwrap());
        let (bad_task_id, good_task_id) = {
            let guard = conn.lock().unwrap();
            let bad = tasks_repo::upsert_favorite_task(&guard, "BAD-1", "Broken ticket", now()).unwrap().id;
            let good = tasks_repo::upsert_favorite_task(&guard, "GOOD-1", "Fine ticket", now()).unwrap().id;
            (bad, good)
        };
        let (bad_entry_id, good_entry_id) = {
            let guard = conn.lock().unwrap();
            let bad = time_entries_repo::insert_manual(&guard, bad_task_id, now(), now(), 600, None).unwrap().id;
            let later = now() + chrono::Duration::hours(1);
            let good = time_entries_repo::insert_manual(&guard, good_task_id, later, later, 600, None).unwrap().id;
            (bad, good)
        };

        let mut client = FakeJiraClient::default();
        client.worklog_outcomes.insert(
            "BAD-1".to_string(),
            Err(JiraError::NotFound("BAD-1".to_string())),
        );

        let outcomes = sync_all(&conn, &client).await;
        assert_eq!(outcomes.len(), 2);

        let bad_outcome = outcomes.iter().find(|o| o.entry_id == bad_entry_id).unwrap();
        assert!(bad_outcome.result.is_err());
        let good_outcome = outcomes.iter().find(|o| o.entry_id == good_entry_id).unwrap();
        assert!(good_outcome.result.is_ok());

        let guard = conn.lock().unwrap();
        let bad_reloaded = time_entries_repo::get_by_id(&guard, bad_entry_id).unwrap().unwrap();
        assert!(!bad_reloaded.is_synced, "failed entry must remain unsynced, untouched, for a later retry");
        let good_reloaded = time_entries_repo::get_by_id(&guard, good_entry_id).unwrap().unwrap();
        assert!(good_reloaded.is_synced);
    }
}
