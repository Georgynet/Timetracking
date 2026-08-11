//! Exercises the *real* `ReqwestJiraClient` HTTP code path against a local mock
//! server (via `wiremock`) — the closest verification possible without live Jira
//! Cloud credentials. Confirms endpoint paths (notably the current `search/jql`
//! endpoint, not the deprecated `search`), headers, and request/response shapes.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{TimeZone, Utc};
use serde_json::json;
use timetracking_lib::jira::client_trait::JiraClient;
use timetracking_lib::jira::models::JiraError;
use timetracking_lib::jira::reqwest_client::ReqwestJiraClient;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn expected_auth_header() -> String {
    format!("Basic {}", STANDARD.encode("me@example.com:secret-token"))
}

fn client(server: &MockServer) -> ReqwestJiraClient {
    ReqwestJiraClient::new(server.uri(), "me@example.com", "secret-token")
}

#[tokio::test]
async fn get_myself_sends_basic_auth_and_parses_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .and(header("Authorization", expected_auth_header().as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accountId": "abc123",
            "displayName": "Ada Lovelace",
            "emailAddress": "me@example.com"
        })))
        .mount(&server)
        .await;

    let result = client(&server).get_myself().await.unwrap();
    assert_eq!(result.account_id, "abc123");
    assert_eq!(result.display_name, "Ada Lovelace");
}

#[tokio::test]
async fn search_issues_uses_the_current_search_jql_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/search/jql"))
        .and(header("Authorization", expected_auth_header().as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issues": [{
                "key": "PROJ-1",
                "fields": {
                    "summary": "Fix the thing",
                    "status": {"name": "In Progress"},
                    "project": {"key": "PROJ"}
                }
            }]
        })))
        .mount(&server)
        .await;
    // Guard against ever regressing onto the deprecated GET /rest/api/3/search.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let issues = client(&server)
        .search_issues("assignee = currentUser()", 50)
        .await
        .unwrap();

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].key, "PROJ-1");
    assert_eq!(issues[0].summary, "Fix the thing");
    assert_eq!(issues[0].status.as_deref(), Some("In Progress"));
    assert_eq!(issues[0].project.as_deref(), Some("PROJ"));
}

#[tokio::test]
async fn add_worklog_wraps_the_comment_as_atlassian_document_format() {
    let server = MockServer::start().await;
    let started = Utc.with_ymd_and_hms(2026, 8, 10, 9, 0, 0).unwrap();
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog"))
        .and(body_json(json!({
            "started": "2026-08-10T09:00:00.000+0000",
            "timeSpentSeconds": 1800,
            "comment": {
                "type": "doc",
                "version": 1,
                "content": [{
                    "type": "paragraph",
                    "content": [{"type": "text", "text": "daily standup"}]
                }]
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "10050"})))
        .mount(&server)
        .await;

    let worklog = client(&server)
        .add_worklog("PROJ-1", started, 1800, Some("daily standup"))
        .await
        .unwrap();
    assert_eq!(worklog.id, "10050");
}

#[tokio::test]
async fn add_worklog_omits_comment_entirely_when_none_given() {
    let server = MockServer::start().await;
    let started = Utc.with_ymd_and_hms(2026, 8, 10, 9, 0, 0).unwrap();
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog"))
        .and(body_json(json!({
            "started": "2026-08-10T09:00:00.000+0000",
            "timeSpentSeconds": 60
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "10051"})))
        .mount(&server)
        .await;

    let worklog = client(&server).add_worklog("PROJ-1", started, 60, None).await.unwrap();
    assert_eq!(worklog.id, "10051");
}

#[tokio::test]
async fn update_worklog_sends_a_put_to_the_specific_worklog_id() {
    let server = MockServer::start().await;
    let started = Utc.with_ymd_and_hms(2026, 8, 10, 9, 0, 0).unwrap();
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/PROJ-1/worklog/10050"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "10050"})))
        .mount(&server)
        .await;

    let worklog = client(&server)
        .update_worklog("PROJ-1", "10050", started, 900, None)
        .await
        .unwrap();
    assert_eq!(worklog.id, "10050");
}

#[tokio::test]
async fn a_401_response_maps_to_auth_error_not_a_raw_status_leak() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&server)
        .await;

    let result = client(&server).get_myself().await;
    assert!(matches!(result, Err(JiraError::Auth)));
}

#[tokio::test]
async fn a_404_response_maps_to_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/PROJ-999"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Issue does not exist"))
        .mount(&server)
        .await;

    let result = client(&server).get_issue("PROJ-999").await;
    assert!(matches!(result, Err(JiraError::NotFound(_))));
}
