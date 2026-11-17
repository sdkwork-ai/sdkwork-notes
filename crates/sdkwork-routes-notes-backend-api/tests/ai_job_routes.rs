use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use http::{Method, Request, StatusCode};
use sdkwork_notes_pages_repository_sqlx::install_sqlite_schema;
use sdkwork_notes_pages_repository_sqlx::notes_store::SqlNotesStore;
use sdkwork_notes_pages_service::domain::{
    ClaimAiJobCommand, CompleteAiJobCommand, CompleteAiSuggestionInput, CreateAiFeedbackCommand,
    CreateAiJobCommand, CreatePageCommand, CreateWorkspaceCommand, DrivePageContentSnapshot,
    DriveVersionPage, ListPageAiSuggestionsQuery, PageInfo, PageKind,
};
use sdkwork_notes_pages_service::ports::{
    CreateDrivePageContentCommand, DrivePageContentPort, ListDrivePageContentVersionsCommand,
    ReadDrivePageContentCommand, RestoreDrivePageContentVersionCommand,
    UpdateDrivePageContentCommand,
};
use sdkwork_notes_pages_service::service::NotesService;
use sdkwork_routes_notes_backend_api::routes::build_router;
use sdkwork_routes_notes_backend_api::wrap_router_with_dev_web_framework;
use serde_json::json;
use sqlx::any::AnyPoolOptions;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::util::ServiceExt;

#[tokio::test]
async fn backend_api_routes_list_retrieve_and_cancel_ai_jobs() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let service = NotesService::new(
        SqlNotesStore::new(pool),
        FakeDrivePageContentPort::default(),
    );
    seed_ai_job(&service).await;
    let app = wrap_router_with_dev_web_framework(build_router(service));

    let list_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/backend/v3/api/notes/ai_jobs?workspace_id=workspace-001&page=1&page_size=20")
                .body(Body::empty())
                .expect("AI job list request should be built"),
        )
        .await
        .expect("AI job list request should be handled");
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_payload = read_envelope_data(list_response).await;
    let ai_job_id = list_payload["items"][0]["id"]
        .as_str()
        .expect("AI job list should include id")
        .to_string();
    assert_eq!(list_payload["items"][0]["workspaceId"], "workspace-001");
    assert_eq!(list_payload["items"][0]["status"], "queued");
    assert_eq!(list_payload["items"][0]["sourceCount"], "1");
    assert_eq!(list_payload["items"][0]["suggestionCount"], "0");

    let retrieve_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri(format!("/backend/v3/api/notes/ai_jobs/{ai_job_id}"))
                .body(Body::empty())
                .expect("AI job retrieve request should be built"),
        )
        .await
        .expect("AI job retrieve request should be handled");
    assert_eq!(retrieve_response.status(), StatusCode::OK);
    let retrieve_payload = read_envelope_data(retrieve_response).await;
    assert_eq!(retrieve_payload["id"], ai_job_id);
    assert_eq!(retrieve_payload["sourceCount"], "1");

    let cancel_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::POST)
                .uri(format!("/backend/v3/api/notes/ai_jobs/{ai_job_id}/cancel"))
                .body(Body::empty())
                .expect("AI job cancel request should be built"),
        )
        .await
        .expect("AI job cancel request should be handled");
    assert_eq!(cancel_response.status(), StatusCode::OK);
    let cancel_payload = read_envelope_data(cancel_response).await;
    assert_eq!(cancel_payload["id"], ai_job_id);
    assert_eq!(cancel_payload["status"], "canceled");
}

#[tokio::test]
async fn backend_api_routes_claim_and_complete_ai_jobs() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let service = NotesService::new(
        SqlNotesStore::new(pool),
        FakeDrivePageContentPort::default(),
    );
    seed_ai_job(&service).await;
    let app = wrap_router_with_dev_web_framework(build_router(service));

    let list_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/backend/v3/api/notes/ai_jobs?workspace_id=workspace-001&page=1&page_size=20")
                .body(Body::empty())
                .expect("AI job list request should be built"),
        )
        .await
        .expect("AI job list request should be handled");
    let list_payload = read_envelope_data(list_response).await;
    let ai_job_id = list_payload["items"][0]["id"]
        .as_str()
        .expect("AI job list should include id")
        .to_string();

    let claim_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::POST)
                .uri(format!("/backend/v3/api/notes/ai_jobs/{ai_job_id}/claim"))
                .body(Body::empty())
                .expect("AI job claim request should be built"),
        )
        .await
        .expect("AI job claim request should be handled");
    assert_eq!(claim_response.status(), StatusCode::OK);
    let claim_payload = read_envelope_data(claim_response).await;
    assert_eq!(claim_payload["id"], ai_job_id);
    assert_eq!(claim_payload["status"], "running");

    let complete_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::POST)
                .uri(format!(
                    "/backend/v3/api/notes/ai_jobs/{ai_job_id}/complete"
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "suggestions": [
                            {
                                "pageId": "page-001",
                                "suggestionType": "summary",
                                "payload": {
                                    "summary": "Roadmap is ready.",
                                    "confidence": "high"
                                }
                            }
                        ]
                    })
                    .to_string(),
                ))
                .expect("AI job complete request should be built"),
        )
        .await
        .expect("AI job complete request should be handled");
    assert_eq!(complete_response.status(), StatusCode::OK);
    let complete_payload = read_envelope_data(complete_response).await;
    assert_eq!(complete_payload["id"], ai_job_id);
    assert_eq!(complete_payload["status"], "succeeded");
    assert_eq!(complete_payload["suggestionCount"], "1");
}

#[tokio::test]
async fn backend_api_routes_accept_and_reject_ai_suggestions() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let service = NotesService::new(
        SqlNotesStore::new(pool),
        FakeDrivePageContentPort::default(),
    );
    let (summary_id, tag_id) = seed_ai_suggestions(&service).await;
    let app = wrap_router_with_dev_web_framework(build_router(service));

    let accept_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            &format!("/backend/v3/api/notes/ai_suggestions/{summary_id}/accept"),
            json!({}),
        ))
        .await
        .expect("AI suggestion accept request should be handled");
    assert_eq!(accept_response.status(), StatusCode::OK);
    let accept_payload = read_envelope_data(accept_response).await;
    assert_eq!(accept_payload["id"], summary_id);
    assert_eq!(accept_payload["status"], "accepted");

    let reject_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            &format!("/backend/v3/api/notes/ai_suggestions/{tag_id}/reject"),
            json!({}),
        ))
        .await
        .expect("AI suggestion reject request should be handled");
    assert_eq!(reject_response.status(), StatusCode::OK);
    let reject_payload = read_envelope_data(reject_response).await;
    assert_eq!(reject_payload["id"], tag_id);
    assert_eq!(reject_payload["status"], "rejected");

    let conflicting_reject_response = app
        .oneshot(json_request(
            Method::POST,
            &format!("/backend/v3/api/notes/ai_suggestions/{summary_id}/reject"),
            json!({}),
        ))
        .await
        .expect("conflicting AI suggestion reject request should be handled");
    assert_eq!(conflicting_reject_response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn backend_api_routes_apply_accepted_ai_suggestion() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let service = NotesService::new(
        SqlNotesStore::new(pool),
        FakeDrivePageContentPort::default(),
    );
    let summary_id = seed_applicable_ai_suggestion(&service).await;
    let app = wrap_router_with_dev_web_framework(build_router(service));

    let accept_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            &format!("/backend/v3/api/notes/ai_suggestions/{summary_id}/accept"),
            json!({}),
        ))
        .await
        .expect("AI suggestion accept request should be handled");
    assert_eq!(accept_response.status(), StatusCode::OK);

    let apply_response = app
        .oneshot(json_request(
            Method::POST,
            &format!("/backend/v3/api/notes/ai_suggestions/{summary_id}/apply"),
            json!({
                "expectedDriveVersionId": "drive-version-page-001-v1",
                "createCheckpoint": true
            }),
        ))
        .await
        .expect("AI suggestion apply request should be handled");
    assert_eq!(apply_response.status(), StatusCode::OK);
    let apply_payload = read_envelope_data(apply_response).await;
    assert_eq!(apply_payload["pageId"], "page-001");
    assert_eq!(apply_payload["driveNodeId"], "drive-node-page-001");
    assert_eq!(apply_payload["driveVersionId"], "drive-version-page-001-v2");
    assert_eq!(apply_payload["driveVersionNo"], "2");
    assert_eq!(
        apply_payload["content"]["blocks"][0]["text"],
        "Launch notes"
    );
}

#[tokio::test]
async fn backend_api_routes_list_ai_suggestion_feedback() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let service = NotesService::new(
        SqlNotesStore::new(pool),
        FakeDrivePageContentPort::default(),
    );
    let (summary_id, _) = seed_ai_suggestions(&service).await;
    service
        .create_ai_feedback(CreateAiFeedbackCommand {
            context: sdkwork_notes_pages_service::domain::NotesActorContext {
                tenant_id: "100001".to_string(),
                organization_id: "0".to_string(),
                operator_id: "30".to_string(),
                caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
            },
            ai_suggestion_id: summary_id.clone(),
            feedback_type: "helpful".to_string(),
            feedback_text: Some("Useful summary".to_string()),
        })
        .await
        .expect("AI feedback should be created");
    let app = wrap_router_with_dev_web_framework(build_router(service));

    let feedback_response = app
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri(format!(
                    "/backend/v3/api/notes/ai_suggestions/{summary_id}/feedback?page=1&page_size=20"
                ))
                .body(Body::empty())
                .expect("AI suggestion feedback request should be built"),
        )
        .await
        .expect("AI suggestion feedback request should be handled");
    assert_eq!(feedback_response.status(), StatusCode::OK);
    let feedback_payload = read_envelope_data(feedback_response).await;
    assert_eq!(feedback_payload["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(feedback_payload["items"][0]["suggestionId"], summary_id);
    assert_eq!(feedback_payload["items"][0]["feedbackType"], "helpful");
    assert_eq!(
        feedback_payload["items"][0]["feedbackText"],
        "Useful summary"
    );
    assert_eq!(feedback_payload["pageInfo"]["hasMore"], false);
}

async fn seed_ai_job(service: &NotesService<SqlNotesStore, FakeDrivePageContentPort>) {
    let actor = sdkwork_notes_pages_service::domain::NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };
    let workspace = service
        .create_workspace(CreateWorkspaceCommand {
            id: "workspace-001".to_string(),
            context: actor.clone(),
            owner_subject_type: "user".to_string(),
            owner_subject_id: "30".to_string(),
            name: "Product Lab".to_string(),
            description: None,
            drive_space_id: "drive-space-001".to_string(),
            default_page_content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            default_page_schema_version: "1".to_string(),
            ai_index_policy_code: "default".to_string(),
        })
        .await
        .expect("workspace should be created");
    let page = service
        .create_page(CreatePageCommand {
            id: "page-001".to_string(),
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await
        .expect("page should be created");
    service
        .create_ai_job(CreateAiJobCommand {
            context: actor,
            workspace_id: workspace.id,
            job_type: "summarize".to_string(),
            target_type: "page".to_string(),
            target_id: Some(page.id),
            prompt: Some("Summarize this page".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "backend-ai-job-001".to_string(),
        })
        .await
        .expect("AI job should be created");
}

async fn seed_ai_suggestions(
    service: &NotesService<SqlNotesStore, FakeDrivePageContentPort>,
) -> (String, String) {
    let actor = sdkwork_notes_pages_service::domain::NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };
    let workspace = service
        .create_workspace(CreateWorkspaceCommand {
            id: "workspace-001".to_string(),
            context: actor.clone(),
            owner_subject_type: "user".to_string(),
            owner_subject_id: "30".to_string(),
            name: "Product Lab".to_string(),
            description: None,
            drive_space_id: "drive-space-001".to_string(),
            default_page_content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            default_page_schema_version: "1".to_string(),
            ai_index_policy_code: "default".to_string(),
        })
        .await
        .expect("workspace should be created");
    let page = service
        .create_page(CreatePageCommand {
            id: "page-001".to_string(),
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await
        .expect("page should be created");
    let job = service
        .create_ai_job(CreateAiJobCommand {
            context: actor.clone(),
            workspace_id: workspace.id,
            job_type: "summarize".to_string(),
            target_type: "page".to_string(),
            target_id: Some(page.id.clone()),
            prompt: Some("Summarize and tag this page".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "backend-ai-suggestion-decision-001".to_string(),
        })
        .await
        .expect("AI job should be created");
    service
        .claim_ai_job(ClaimAiJobCommand {
            context: actor.clone(),
            ai_job_id: job.id.clone(),
        })
        .await
        .expect("AI job should be claimed");
    service
        .complete_ai_job(CompleteAiJobCommand {
            context: actor.clone(),
            ai_job_id: job.id,
            suggestions: vec![
                CompleteAiSuggestionInput {
                    page_id: Some(page.id.clone()),
                    suggestion_type: "summary".to_string(),
                    payload: json!({ "summary": "Roadmap is ready." }),
                },
                CompleteAiSuggestionInput {
                    page_id: Some(page.id.clone()),
                    suggestion_type: "tag".to_string(),
                    payload: json!({ "tag": "launch" }),
                },
            ],
        })
        .await
        .expect("AI job should be completed");

    let suggestions = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor,
            page_id: page.id,
            page: 1,
            page_size: 20,
        })
        .await
        .expect("AI suggestions should be listed");
    let summary_id = suggestions
        .items
        .iter()
        .find(|suggestion| suggestion.suggestion_type == "summary")
        .expect("summary suggestion should exist")
        .id
        .clone();
    let tag_id = suggestions
        .items
        .iter()
        .find(|suggestion| suggestion.suggestion_type == "tag")
        .expect("tag suggestion should exist")
        .id
        .clone();
    (summary_id, tag_id)
}

async fn seed_applicable_ai_suggestion(
    service: &NotesService<SqlNotesStore, FakeDrivePageContentPort>,
) -> String {
    let actor = sdkwork_notes_pages_service::domain::NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };
    let workspace = service
        .create_workspace(CreateWorkspaceCommand {
            id: "workspace-001".to_string(),
            context: actor.clone(),
            owner_subject_type: "user".to_string(),
            owner_subject_id: "30".to_string(),
            name: "Product Lab".to_string(),
            description: None,
            drive_space_id: "drive-space-001".to_string(),
            default_page_content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            default_page_schema_version: "1".to_string(),
            ai_index_policy_code: "default".to_string(),
        })
        .await
        .expect("workspace should be created");
    let page = service
        .create_page(CreatePageCommand {
            id: "page-001".to_string(),
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "launch plan" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await
        .expect("page should be created");
    let job = service
        .create_ai_job(CreateAiJobCommand {
            context: actor.clone(),
            workspace_id: workspace.id,
            job_type: "summarize".to_string(),
            target_type: "page".to_string(),
            target_id: Some(page.id),
            prompt: Some("Rewrite this page into concise launch notes".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "backend-ai-suggestion-apply-001".to_string(),
        })
        .await
        .expect("AI job should be created");
    service
        .claim_ai_job(ClaimAiJobCommand {
            context: actor.clone(),
            ai_job_id: job.id.clone(),
        })
        .await
        .expect("AI job should be claimed");
    service
        .complete_ai_job(CompleteAiJobCommand {
            context: actor.clone(),
            ai_job_id: job.id,
            suggestions: vec![CompleteAiSuggestionInput {
                page_id: Some("page-001".to_string()),
                suggestion_type: "summary".to_string(),
                payload: json!({
                    "contentType": "application/vnd.sdkwork.notes.page+json",
                    "contentSchemaVersion": "1",
                    "content": {
                        "blocks": [
                            { "type": "heading", "text": "Launch notes" },
                            { "type": "paragraph", "text": "Launch plan is ready for review." }
                        ]
                    }
                }),
            }],
        })
        .await
        .expect("AI job should be completed");

    let suggestions = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor,
            page_id: "page-001".to_string(),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("AI suggestions should be listed");
    suggestions.items[0].id.clone()
}

fn auth_request_builder() -> http::request::Builder {
    Request::builder()
        .header(
            "authorization",
            format!(
                "Bearer {}",
                sdkwork_routes_notes_http_auth::test_support::default_test_auth_claim_token()
            ),
        )
        .header(
            "access-token",
            sdkwork_routes_notes_http_auth::test_support::default_test_access_claim_token(),
        )
}

fn json_request(method: Method, uri: &str, body: serde_json::Value) -> Request<Body> {
    auth_request_builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("json request should be built")
}

async fn read_envelope_data(response: axum::response::Response) -> serde_json::Value {
    let payload = read_json(response).await;
    assert_eq!(payload["code"], 0);
    payload["data"].clone()
}

async fn read_json(response: axum::response::Response) -> serde_json::Value {
    serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable"),
    )
    .expect("response should be json")
}

#[derive(Clone, Default)]
struct FakeDrivePageContentPort {
    records: Arc<Mutex<BTreeMap<String, DrivePageContentSnapshot>>>,
}

#[async_trait]
impl DrivePageContentPort for FakeDrivePageContentPort {
    async fn create_page_content(
        &self,
        command: CreateDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, sdkwork_notes_pages_service::error::NotesProductError>
    {
        let snapshot = DrivePageContentSnapshot {
            drive_space_id: command.drive_space_id.clone(),
            drive_node_id: format!("drive-node-{}", command.page_id),
            drive_uri: format!(
                "drive://spaces/{}/nodes/drive-node-{}",
                command.drive_space_id, command.page_id
            ),
            drive_version_id: format!("drive-version-{}-v1", command.page_id),
            drive_version_no: 1,
            content_type: command.content_type,
            content_schema_version: command.content_schema_version,
            content_hash: Some("sha256:first".to_string()),
            snippet: Some("hello".to_string()),
            word_count: 1,
            task_count: 0,
            content: command.content,
        };
        self.records
            .lock()
            .await
            .insert(command.page_id, snapshot.clone());
        Ok(snapshot)
    }

    async fn update_page_content(
        &self,
        command: UpdateDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, sdkwork_notes_pages_service::error::NotesProductError>
    {
        let snapshot = DrivePageContentSnapshot {
            drive_space_id: command.drive_space_id,
            drive_node_id: command.drive_node_id,
            drive_uri: command.drive_uri,
            drive_version_id: format!("drive-version-{}-v2", command.page_id),
            drive_version_no: 2,
            content_type: command.content_type,
            content_schema_version: command.content_schema_version,
            content_hash: Some("sha256:second".to_string()),
            snippet: Some("hello v2".to_string()),
            word_count: 2,
            task_count: 0,
            content: command.content,
        };
        self.records
            .lock()
            .await
            .insert(command.page_id, snapshot.clone());
        Ok(snapshot)
    }

    async fn read_page_content(
        &self,
        command: ReadDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, sdkwork_notes_pages_service::error::NotesProductError>
    {
        self.records
            .lock()
            .await
            .get(&command.page_id)
            .cloned()
            .ok_or_else(|| {
                sdkwork_notes_pages_service::error::NotesProductError::NotFound(
                    "page content not found".to_string(),
                )
            })
    }

    async fn restore_page_content_version(
        &self,
        _: RestoreDrivePageContentVersionCommand,
    ) -> Result<DrivePageContentSnapshot, sdkwork_notes_pages_service::error::NotesProductError>
    {
        unreachable!("backend AI routes must not restore Drive page content versions")
    }

    async fn list_page_content_versions(
        &self,
        command: ListDrivePageContentVersionsCommand,
    ) -> Result<DriveVersionPage, sdkwork_notes_pages_service::error::NotesProductError> {
        Ok(DriveVersionPage {
            items: Vec::new(),
            page_info: PageInfo {
                page: command.page,
                page_size: command.page_size,
                has_more: false,
                next_cursor: None,
            },
        })
    }
}
