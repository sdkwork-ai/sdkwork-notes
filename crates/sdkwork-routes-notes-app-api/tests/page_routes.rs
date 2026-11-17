use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use http::{Method, Request, StatusCode};
use sdkwork_notes_pages_repository_sqlx::install_sqlite_schema;
use sdkwork_notes_pages_repository_sqlx::notes_store::SqlNotesStore;
use sdkwork_notes_pages_service::domain::{
    ClaimAiJobCommand, CompleteAiJobCommand, CompleteAiSuggestionInput, CreateAiJobCommand,
    CreatePageCommand, CreateWorkspaceCommand, DrivePageContentSnapshot, DriveVersionPage,
    DriveVersionSummary, NotesActorContext, PageInfo, PageKind,
};
use sdkwork_notes_pages_service::ports::{
    CreateDrivePageContentCommand, DrivePageContentPort, ListDrivePageContentVersionsCommand,
    ReadDrivePageContentCommand, RestoreDrivePageContentVersionCommand,
    UpdateDrivePageContentCommand,
};
use sdkwork_notes_pages_service::service::NotesService;
use sdkwork_routes_notes_app_api::routes::build_router;
use sdkwork_routes_notes_app_api::wrap_router_with_dev_web_framework;
use serde_json::json;
use sqlx::any::AnyPoolOptions;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::util::ServiceExt;

#[tokio::test]
async fn app_api_routes_create_page_and_update_drive_backed_content() {
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
    let app = wrap_router_with_dev_web_framework(build_router(service));

    let workspace_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/app/v3/api/notes/workspaces",
            json!({
                "id": "workspace-001",
                "ownerSubjectType": "user",
                "ownerSubjectId": "30",
                "name": "Product Lab",
                "description": "AI notes workspace",
                "driveSpaceId": "drive-space-001"
            }),
        ))
        .await
        .expect("workspace request should be handled");
    assert_eq!(workspace_response.status(), StatusCode::CREATED);

    let second_workspace_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/app/v3/api/notes/workspaces",
            json!({
                "id": "workspace-002",
                "ownerSubjectType": "user",
                "ownerSubjectId": "30",
                "name": "Research Lab",
                "driveSpaceId": "drive-space-002"
            }),
        ))
        .await
        .expect("second workspace request should be handled");
    assert_eq!(second_workspace_response.status(), StatusCode::CREATED);

    let workspace_list_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/app/v3/api/notes/workspaces?page=1&page_size=1")
                .body(Body::empty())
                .expect("workspace list request should be built"),
        )
        .await
        .expect("workspace list request should be handled");
    assert_eq!(workspace_list_response.status(), StatusCode::OK);
    let workspace_list_payload = read_envelope_data(workspace_list_response).await;
    assert_eq!(
        workspace_list_payload["items"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(workspace_list_payload["pageInfo"]["hasMore"], true);

    let create_page_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/app/v3/api/notes/workspaces/workspace-001/pages",
            json!({
                "id": "page-001",
                "title": "Roadmap",
                "initialContent": { "blocks": [{ "type": "paragraph", "text": "hello" }] },
                "contentType": "application/vnd.sdkwork.notes.page+json"
            }),
        ))
        .await
        .expect("create page request should be handled");
    assert_eq!(create_page_response.status(), StatusCode::CREATED);
    let create_page_payload = read_envelope_data(create_page_response).await;
    assert_eq!(
        create_page_payload["driveNodeId"].as_str(),
        Some("drive-node-page-001")
    );
    assert_eq!(
        create_page_payload["currentDriveVersionId"].as_str(),
        Some("drive-version-page-001-v1")
    );
    assert_eq!(
        create_page_payload["currentDriveVersionNo"].as_str(),
        Some("1")
    );

    let create_release_page_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/app/v3/api/notes/workspaces/workspace-001/pages",
            json!({
                "id": "page-002",
                "title": "Release notes",
                "initialContent": { "blocks": [] },
                "contentType": "application/vnd.sdkwork.notes.page+json"
            }),
        ))
        .await
        .expect("create second page request should be handled");
    assert_eq!(create_release_page_response.status(), StatusCode::CREATED);

    let create_child_page_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/app/v3/api/notes/workspaces/workspace-001/pages",
            json!({
                "id": "page-child",
                "title": "Roadmap child",
                "parentPageId": "page-001",
                "initialContent": { "blocks": [] },
                "contentType": "application/vnd.sdkwork.notes.page+json"
            }),
        ))
        .await
        .expect("create child page request should be handled");
    assert_eq!(create_child_page_response.status(), StatusCode::CREATED);

    let bootstrap_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/app/v3/api/notes/workspaces/workspace-001/bootstrap")
                .body(Body::empty())
                .expect("bootstrap request should be built"),
        )
        .await
        .expect("bootstrap request should be handled");
    assert_eq!(bootstrap_response.status(), StatusCode::OK);
    let bootstrap_payload = read_envelope_data(bootstrap_response).await;
    assert_eq!(bootstrap_payload["workspace"]["id"], "workspace-001");
    assert!(!bootstrap_payload["rootPages"]
        .as_array()
        .expect("rootPages should be an array")
        .iter()
        .any(|page| page["id"] == "page-child"));
    assert!(bootstrap_payload["objectTypes"]
        .as_array()
        .expect("objectTypes should be an array")
        .iter()
        .any(|object_type| object_type["code"] == "database"));

    let page_list_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri(
                    "/app/v3/api/notes/workspaces/workspace-001/pages?page=1&page_size=20&q=release",
                )
                .body(Body::empty())
                .expect("page list request should be built"),
        )
        .await
        .expect("page list request should be handled");
    assert_eq!(page_list_response.status(), StatusCode::OK);
    let page_list_payload = read_envelope_data(page_list_response).await;
    assert_eq!(page_list_payload["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(page_list_payload["items"][0]["id"], "page-002");

    let oversized_page_query_uri = format!(
        "/app/v3/api/notes/workspaces/workspace-001/pages?page=1&page_size=20&q={}",
        "x".repeat(201)
    );
    let oversized_page_query_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri(oversized_page_query_uri)
                .body(Body::empty())
                .expect("oversized page query request should be built"),
        )
        .await
        .expect("oversized page query request should be handled");
    assert_eq!(
        oversized_page_query_response.status(),
        StatusCode::BAD_REQUEST
    );

    let metadata_update_response = app
        .clone()
        .oneshot(json_request(
            Method::PATCH,
            "/app/v3/api/notes/pages/page-001",
            json!({
                "title": "Roadmap v2",
                "favorite": true,
                "archiveStatus": "archived",
                "publishStatus": "unlisted",
                "expectedVersion": "1"
            }),
        ))
        .await
        .expect("metadata update request should be handled");
    assert_eq!(metadata_update_response.status(), StatusCode::OK);
    let metadata_update_payload = read_envelope_data(metadata_update_response).await;
    assert_eq!(metadata_update_payload["title"], "Roadmap v2");
    assert_eq!(metadata_update_payload["favorite"], true);
    assert_eq!(
        metadata_update_payload["currentDriveVersionId"].as_str(),
        Some("drive-version-page-001-v1")
    );

    let stale_metadata_update_response = app
        .clone()
        .oneshot(json_request(
            Method::PATCH,
            "/app/v3/api/notes/pages/page-001",
            json!({
                "title": "stale title",
                "expectedVersion": "1"
            }),
        ))
        .await
        .expect("stale metadata update request should be handled");
    assert_eq!(
        stale_metadata_update_response.status(),
        StatusCode::CONFLICT
    );

    let update_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/app/v3/api/notes/pages/page-001/content",
            json!({
                "content": { "blocks": [{ "type": "paragraph", "text": "hello v2" }] },
                "contentType": "application/vnd.sdkwork.notes.page+json",
                "expectedDriveVersionId": "drive-version-page-001-v1"
            }),
        ))
        .await
        .expect("update content request should be handled");
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_payload = read_envelope_data(update_response).await;
    assert_eq!(
        update_payload["driveVersionId"].as_str(),
        Some("drive-version-page-001-v2")
    );
    assert_eq!(update_payload["driveVersionNo"].as_str(), Some("2"));

    let content_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/app/v3/api/notes/pages/page-001/content")
                .body(Body::empty())
                .expect("content request should be built"),
        )
        .await
        .expect("content request should be handled");
    assert_eq!(content_response.status(), StatusCode::OK);
    let content_payload = read_envelope_data(content_response).await;
    assert_eq!(content_payload["content"]["blocks"][0]["text"], "hello v2");

    let versions_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/app/v3/api/notes/pages/page-001/versions?page=1&page_size=20")
                .body(Body::empty())
                .expect("versions request should be built"),
        )
        .await
        .expect("versions request should be handled");
    assert_eq!(versions_response.status(), StatusCode::OK);
    let versions_payload = read_envelope_data(versions_response).await;
    assert_eq!(
        versions_payload["items"][0]["driveVersionId"].as_str(),
        Some("drive-version-page-001-v2")
    );
    assert_eq!(
        versions_payload["items"][0]["driveVersionNo"].as_str(),
        Some("2")
    );
    assert_eq!(
        versions_payload["items"][0]["versionKind"].as_str(),
        Some("auto")
    );
    assert_eq!(versions_payload["pageInfo"]["page"], 1);

    let restore_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/app/v3/api/notes/pages/page-001/versions/drive-version-page-001-v1/restore",
            json!({
                "expectedCurrentDriveVersionId": "drive-version-page-001-v2"
            }),
        ))
        .await
        .expect("restore page version request should be handled");
    assert_eq!(restore_response.status(), StatusCode::OK);
    let restore_payload = read_envelope_data(restore_response).await;
    assert_eq!(
        restore_payload["driveVersionId"].as_str(),
        Some("drive-version-page-001-v3")
    );
    assert_eq!(restore_payload["driveVersionNo"].as_str(), Some("3"));
    assert_eq!(restore_payload["content"]["blocks"][0]["text"], "hello");

    let search_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/app/v3/api/notes/search?workspace_id=workspace-001&q=v2&page=1&page_size=20")
                .body(Body::empty())
                .expect("search request should be built"),
        )
        .await
        .expect("search request should be handled");
    assert_eq!(search_response.status(), StatusCode::OK);
    let search_payload = read_envelope_data(search_response).await;
    assert_eq!(search_payload["items"][0]["page"]["id"], "page-001");
    assert_eq!(
        search_payload["items"][0]["sourceDriveVersionNo"].as_str(),
        Some("3")
    );
    assert_eq!(
        search_payload["items"][0]["sourceDriveVersionId"].as_str(),
        Some("drive-version-page-001-v3")
    );
    assert_eq!(search_payload["items"][0]["highlights"][0], "Roadmap v2");

    let oversized_search_query_uri = format!(
        "/app/v3/api/notes/search?workspace_id=workspace-001&q={}&page=1&page_size=20",
        "x".repeat(201)
    );
    let oversized_search_query_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri(oversized_search_query_uri)
                .body(Body::empty())
                .expect("oversized search query request should be built"),
        )
        .await
        .expect("oversized search query request should be handled");
    assert_eq!(
        oversized_search_query_response.status(),
        StatusCode::BAD_REQUEST
    );

    let ai_job_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::POST)
                .uri("/app/v3/api/notes/ai_jobs")
                .header("content-type", "application/json")
                .header("Idempotency-Key", "ai-job-create-001")
                .body(Body::from(
                    json!({
                        "workspaceId": "workspace-001",
                        "jobType": "summarize",
                        "targetType": "page",
                        "targetId": "page-001",
                        "prompt": "Summarize this page",
                        "contextPolicy": { "source": "current_page" }
                    })
                    .to_string(),
                ))
                .expect("AI job request should be built"),
        )
        .await
        .expect("AI job request should be handled");
    assert_eq!(ai_job_response.status(), StatusCode::ACCEPTED);
    let ai_job_payload = read_envelope_data(ai_job_response).await;
    assert_eq!(ai_job_payload["workspaceId"], "workspace-001");
    assert_eq!(ai_job_payload["jobType"], "summarize");
    assert_eq!(ai_job_payload["targetType"], "page");
    assert_eq!(ai_job_payload["targetId"], "page-001");
    assert_eq!(ai_job_payload["status"], "queued");
    let ai_job_id = ai_job_payload["id"]
        .as_str()
        .expect("AI job response should include id")
        .to_string();

    let ai_job_replay_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::POST)
                .uri("/app/v3/api/notes/ai_jobs")
                .header("content-type", "application/json")
                .header("Idempotency-Key", "ai-job-create-001")
                .body(Body::from(
                    json!({
                        "workspaceId": "workspace-001",
                        "jobType": "summarize",
                        "targetType": "page",
                        "targetId": "page-001",
                        "prompt": "Summarize this page",
                        "contextPolicy": { "source": "current_page" }
                    })
                    .to_string(),
                ))
                .expect("AI job replay request should be built"),
        )
        .await
        .expect("AI job replay request should be handled");
    assert_eq!(ai_job_replay_response.status(), StatusCode::ACCEPTED);
    let ai_job_replay_payload = read_envelope_data(ai_job_replay_response).await;
    assert_eq!(ai_job_replay_payload["id"], ai_job_id);

    let forbidden_route_response = app
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/app/v3/api/notes/notes")
                .body(Body::empty())
                .expect("forbidden route request should be built"),
        )
        .await
        .expect("forbidden route request should be handled");
    assert_eq!(forbidden_route_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn app_api_routes_remote_apply_updates_page_metadata_and_archives_page() {
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
    let app = wrap_router_with_dev_web_framework(build_router(service));

    app.clone()
        .oneshot(json_request(
            Method::POST,
            "/app/v3/api/notes/workspaces",
            json!({
                "id": "workspace-remote-apply",
                "ownerSubjectType": "user",
                "ownerSubjectId": "30",
                "name": "Remote Apply Lab",
                "driveSpaceId": "drive-space-remote-apply"
            }),
        ))
        .await
        .expect("workspace request should be handled");

    let create_page_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/app/v3/api/notes/workspaces/workspace-remote-apply/pages",
            json!({
                "id": "page-remote-apply",
                "title": "Before remote apply",
                "initialContent": { "blocks": [] },
                "contentType": "application/vnd.sdkwork.notes.page+json"
            }),
        ))
        .await
        .expect("create page request should be handled");
    assert_eq!(create_page_response.status(), StatusCode::CREATED);
    let create_page_payload = read_envelope_data(create_page_response).await;
    let remote_cursor = create_page_payload["currentDriveVersionId"]
        .as_str()
        .expect("currentDriveVersionId should be present")
        .to_string();

    let upsert_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/app/v3/api/notes/pages/page-remote-apply/remote_apply",
            json!({
                "idempotencyKey": "remote-apply-upsert-001",
                "taskId": "sync-task-upsert-001",
                "entityType": "note",
                "entityId": "page-remote-apply",
                "operation": "upsert",
                "baseRemoteCursor": remote_cursor,
                "mutation": {
                    "patch": {
                        "title": "After remote apply"
                    }
                }
            }),
        ))
        .await
        .expect("remote apply upsert request should be handled");
    assert_eq!(upsert_response.status(), StatusCode::OK);
    let upsert_payload = read_envelope_data(upsert_response).await;
    assert_eq!(upsert_payload["outcome"], "applied");
    assert_eq!(upsert_payload["taskId"], "sync-task-upsert-001");
    assert!(upsert_payload["remoteCursor"].is_string());

    let page_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/app/v3/api/notes/pages/page-remote-apply")
                .body(Body::empty())
                .expect("page request should be built"),
        )
        .await
        .expect("page request should be handled");
    assert_eq!(page_response.status(), StatusCode::OK);
    let page_payload = read_envelope_data(page_response).await;
    assert_eq!(page_payload["title"], "After remote apply");

    let archive_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/app/v3/api/notes/pages/page-remote-apply/remote_apply",
            json!({
                "idempotencyKey": "remote-apply-delete-001",
                "taskId": "sync-task-delete-001",
                "entityType": "note",
                "entityId": "page-remote-apply",
                "operation": "delete",
                "mutation": {
                    "intent": "move-to-trash"
                }
            }),
        ))
        .await
        .expect("remote apply delete request should be handled");
    assert_eq!(archive_response.status(), StatusCode::OK);
    let archive_payload = read_envelope_data(archive_response).await;
    assert_eq!(archive_payload["outcome"], "applied");

    let archived_page_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/app/v3/api/notes/pages/page-remote-apply")
                .body(Body::empty())
                .expect("archived page request should be built"),
        )
        .await
        .expect("archived page request should be handled");
    assert_eq!(archived_page_response.status(), StatusCode::OK);
    let archived_page_payload = read_envelope_data(archived_page_response).await;
    assert_eq!(archived_page_payload["archiveStatus"], "archived");

    let permanent_delete_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/app/v3/api/notes/pages/page-remote-apply/remote_apply",
            json!({
                "idempotencyKey": "remote-apply-permanent-delete-001",
                "taskId": "sync-task-permanent-delete-001",
                "entityType": "note",
                "entityId": "page-remote-apply",
                "operation": "permanent-delete",
                "mutation": {
                    "intent": "permanent-delete"
                }
            }),
        ))
        .await
        .expect("remote apply permanent delete request should be handled");
    assert_eq!(permanent_delete_response.status(), StatusCode::OK);
    let permanent_delete_payload = read_envelope_data(permanent_delete_response).await;
    assert_eq!(permanent_delete_payload["outcome"], "applied");

    let deleted_page_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/app/v3/api/notes/pages/page-remote-apply")
                .body(Body::empty())
                .expect("deleted page request should be built"),
        )
        .await
        .expect("deleted page request should be handled");
    assert_eq!(deleted_page_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn app_api_routes_update_content_preserves_existing_content_metadata_when_omitted() {
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
    let app = wrap_router_with_dev_web_framework(build_router(service));

    let workspace_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/app/v3/api/notes/workspaces",
            json!({
                "id": "workspace-custom-metadata",
                "ownerSubjectType": "user",
                "ownerSubjectId": "30",
                "name": "Canvas Lab",
                "driveSpaceId": "drive-space-custom-metadata"
            }),
        ))
        .await
        .expect("workspace request should be handled");
    assert_eq!(workspace_response.status(), StatusCode::CREATED);

    let create_page_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/app/v3/api/notes/workspaces/workspace-custom-metadata/pages",
            json!({
                "id": "page-canvas-001",
                "title": "Canvas",
                "initialContent": { "nodes": [{ "id": "node-1", "type": "text" }] },
                "contentType": "application/vnd.sdkwork.notes.canvas+json",
                "contentSchemaVersion": "2"
            }),
        ))
        .await
        .expect("create page request should be handled");
    assert_eq!(create_page_response.status(), StatusCode::CREATED);
    let create_page_payload = read_envelope_data(create_page_response).await;
    assert_eq!(
        create_page_payload["contentType"].as_str(),
        Some("application/vnd.sdkwork.notes.canvas+json")
    );
    assert_eq!(
        create_page_payload["contentSchemaVersion"].as_str(),
        Some("2")
    );

    let update_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/app/v3/api/notes/pages/page-canvas-001/content",
            json!({
                "content": { "nodes": [{ "id": "node-1", "type": "text", "text": "updated" }] },
                "expectedDriveVersionId": "drive-version-page-canvas-001-v1"
            }),
        ))
        .await
        .expect("update content request should be handled");
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_payload = read_envelope_data(update_response).await;
    assert_eq!(
        update_payload["contentType"].as_str(),
        Some("application/vnd.sdkwork.notes.canvas+json")
    );
    assert_eq!(update_payload["contentSchemaVersion"].as_str(), Some("2"));

    let page_response = app
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/app/v3/api/notes/pages/page-canvas-001")
                .body(Body::empty())
                .expect("page request should be built"),
        )
        .await
        .expect("page request should be handled");
    assert_eq!(page_response.status(), StatusCode::OK);
    let page_payload = read_envelope_data(page_response).await;
    assert_eq!(
        page_payload["contentType"].as_str(),
        Some("application/vnd.sdkwork.notes.canvas+json")
    );
    assert_eq!(page_payload["contentSchemaVersion"].as_str(), Some("2"));
}

#[tokio::test]
async fn app_api_routes_list_page_ai_suggestions() {
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
    seed_completed_ai_suggestion(&service).await;
    let app = wrap_router_with_dev_web_framework(build_router(service));

    let suggestions_response = app
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/app/v3/api/notes/pages/page-001/ai_suggestions?page=1&page_size=20")
                .body(Body::empty())
                .expect("page AI suggestions request should be built"),
        )
        .await
        .expect("page AI suggestions request should be handled");
    assert_eq!(suggestions_response.status(), StatusCode::OK);
    let suggestions_payload = read_envelope_data(suggestions_response).await;
    assert_eq!(
        suggestions_payload["items"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(suggestions_payload["items"][0]["pageId"], "page-001");
    assert_eq!(suggestions_payload["items"][0]["suggestionType"], "summary");
    assert_eq!(suggestions_payload["items"][0]["status"], "proposed");
    assert_eq!(
        suggestions_payload["items"][0]["sourceDriveVersionId"],
        "drive-version-page-001-v1"
    );
    assert_eq!(suggestions_payload["items"][0]["sourceDriveVersionNo"], "1");
    assert_eq!(
        suggestions_payload["items"][0]["payload"]["summary"],
        "Roadmap is ready."
    );
    assert_eq!(suggestions_payload["pageInfo"]["hasMore"], false);
}

#[tokio::test]
async fn app_api_routes_create_ai_suggestion_feedback() {
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
    seed_completed_ai_suggestion(&service).await;
    let app = wrap_router_with_dev_web_framework(build_router(service));

    let suggestions_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/app/v3/api/notes/pages/page-001/ai_suggestions?page=1&page_size=20")
                .body(Body::empty())
                .expect("page AI suggestions request should be built"),
        )
        .await
        .expect("page AI suggestions request should be handled");
    assert_eq!(suggestions_response.status(), StatusCode::OK);
    let suggestions_payload = read_envelope_data(suggestions_response).await;
    let suggestion_id = suggestions_payload["items"][0]["id"]
        .as_str()
        .expect("suggestion id should be present")
        .to_string();

    let feedback_response = app
        .oneshot(json_request(
            Method::POST,
            &format!("/app/v3/api/notes/ai_suggestions/{suggestion_id}/feedback"),
            json!({
                "feedbackType": "helpful",
                "feedbackText": "Useful summary"
            }),
        ))
        .await
        .expect("AI suggestion feedback request should be handled");
    assert_eq!(feedback_response.status(), StatusCode::OK);
    let feedback_payload = read_envelope_data(feedback_response).await;
    assert_eq!(feedback_payload["suggestionId"], suggestion_id);
    assert_eq!(feedback_payload["feedbackType"], "helpful");
    assert_eq!(feedback_payload["feedbackText"], "Useful summary");
}

#[tokio::test]
async fn app_api_routes_accept_and_reject_ai_suggestions() {
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
    seed_two_completed_ai_suggestions(&service).await;
    let app = wrap_router_with_dev_web_framework(build_router(service));

    let suggestions_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/app/v3/api/notes/pages/page-001/ai_suggestions?page=1&page_size=20")
                .body(Body::empty())
                .expect("page AI suggestions request should be built"),
        )
        .await
        .expect("page AI suggestions request should be handled");
    assert_eq!(suggestions_response.status(), StatusCode::OK);
    let suggestions_payload = read_envelope_data(suggestions_response).await;
    let items = suggestions_payload["items"]
        .as_array()
        .expect("suggestions should be an array");
    let summary_id = items
        .iter()
        .find(|item| item["suggestionType"] == "summary")
        .and_then(|item| item["id"].as_str())
        .expect("summary suggestion should exist")
        .to_string();
    let tag_id = items
        .iter()
        .find(|item| item["suggestionType"] == "tag")
        .and_then(|item| item["id"].as_str())
        .expect("tag suggestion should exist")
        .to_string();

    let accept_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            &format!("/app/v3/api/notes/ai_suggestions/{summary_id}/accept"),
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
            &format!("/app/v3/api/notes/ai_suggestions/{tag_id}/reject"),
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
            &format!("/app/v3/api/notes/ai_suggestions/{summary_id}/reject"),
            json!({}),
        ))
        .await
        .expect("conflicting AI suggestion reject request should be handled");
    assert_eq!(conflicting_reject_response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn app_api_routes_apply_accepted_ai_suggestion() {
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
    seed_applicable_ai_suggestion(&service).await;
    let app = wrap_router_with_dev_web_framework(build_router(service));

    let suggestions_response = app
        .clone()
        .oneshot(
            auth_request_builder()
                .method(Method::GET)
                .uri("/app/v3/api/notes/pages/page-001/ai_suggestions?page=1&page_size=20")
                .body(Body::empty())
                .expect("page AI suggestions request should be built"),
        )
        .await
        .expect("page AI suggestions request should be handled");
    assert_eq!(suggestions_response.status(), StatusCode::OK);
    let suggestions_payload = read_envelope_data(suggestions_response).await;
    let suggestion_id = suggestions_payload["items"][0]["id"]
        .as_str()
        .expect("suggestion id should be present")
        .to_string();

    let accept_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            &format!("/app/v3/api/notes/ai_suggestions/{suggestion_id}/accept"),
            json!({}),
        ))
        .await
        .expect("AI suggestion accept request should be handled");
    assert_eq!(accept_response.status(), StatusCode::OK);

    let apply_response = app
        .oneshot(json_request(
            Method::POST,
            &format!("/app/v3/api/notes/ai_suggestions/{suggestion_id}/apply"),
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

async fn seed_completed_ai_suggestion(
    service: &NotesService<SqlNotesStore, FakeDrivePageContentPort>,
) {
    let actor = NotesActorContext {
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
            target_id: Some(page.id),
            prompt: Some("Summarize this page".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "app-ai-suggestion-001".to_string(),
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
            context: actor,
            ai_job_id: job.id,
            suggestions: vec![CompleteAiSuggestionInput {
                page_id: Some("page-001".to_string()),
                suggestion_type: "summary".to_string(),
                payload: json!({ "summary": "Roadmap is ready." }),
            }],
        })
        .await
        .expect("AI job should be completed");
}

async fn seed_applicable_ai_suggestion(
    service: &NotesService<SqlNotesStore, FakeDrivePageContentPort>,
) {
    let actor = NotesActorContext {
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
            idempotency_key: "app-ai-suggestion-apply-001".to_string(),
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
            context: actor,
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
}

async fn seed_two_completed_ai_suggestions(
    service: &NotesService<SqlNotesStore, FakeDrivePageContentPort>,
) {
    let actor = NotesActorContext {
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
            target_id: Some(page.id),
            prompt: Some("Summarize and tag this page".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "app-ai-suggestion-decision-001".to_string(),
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
            context: actor,
            ai_job_id: job.id,
            suggestions: vec![
                CompleteAiSuggestionInput {
                    page_id: Some("page-001".to_string()),
                    suggestion_type: "summary".to_string(),
                    payload: json!({ "summary": "Roadmap is ready." }),
                },
                CompleteAiSuggestionInput {
                    page_id: Some("page-001".to_string()),
                    suggestion_type: "tag".to_string(),
                    payload: json!({ "tag": "launch" }),
                },
            ],
        })
        .await
        .expect("AI job should be completed");
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

fn query_request(method: Method, uri: &str) -> Request<Body> {
    auth_request_builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .expect("query request should be built")
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
    versions: Arc<Mutex<BTreeMap<String, Vec<DrivePageContentSnapshot>>>>,
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
            .insert(command.page_id.clone(), snapshot.clone());
        self.versions
            .lock()
            .await
            .entry(command.page_id)
            .or_default()
            .push(snapshot.clone());
        Ok(snapshot)
    }

    async fn update_page_content(
        &self,
        command: UpdateDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, sdkwork_notes_pages_service::error::NotesProductError>
    {
        let snapshot = DrivePageContentSnapshot {
            drive_space_id: command.drive_space_id.clone(),
            drive_node_id: command.drive_node_id.clone(),
            drive_uri: command.drive_uri.clone(),
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
            .insert(command.page_id.clone(), snapshot.clone());
        self.versions
            .lock()
            .await
            .entry(command.page_id)
            .or_default()
            .push(snapshot.clone());
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
        command: RestoreDrivePageContentVersionCommand,
    ) -> Result<DrivePageContentSnapshot, sdkwork_notes_pages_service::error::NotesProductError>
    {
        let source = self
            .versions
            .lock()
            .await
            .get(&command.page_id)
            .and_then(|versions| {
                versions
                    .iter()
                    .find(|snapshot| snapshot.drive_version_id == command.drive_version_id)
                    .cloned()
            })
            .ok_or_else(|| {
                sdkwork_notes_pages_service::error::NotesProductError::NotFound(
                    "page content version not found".to_string(),
                )
            })?;
        let next_version_no = self
            .versions
            .lock()
            .await
            .get(&command.page_id)
            .and_then(|versions| {
                versions
                    .iter()
                    .map(|snapshot| snapshot.drive_version_no)
                    .max()
            })
            .unwrap_or(0)
            + 1;
        let mut snapshot = source;
        snapshot.drive_version_id = format!("drive-version-{}-v{next_version_no}", command.page_id);
        snapshot.drive_version_no = next_version_no;
        self.records
            .lock()
            .await
            .insert(command.page_id.clone(), snapshot.clone());
        self.versions
            .lock()
            .await
            .entry(command.page_id)
            .or_default()
            .push(snapshot.clone());
        Ok(snapshot)
    }

    async fn list_page_content_versions(
        &self,
        command: ListDrivePageContentVersionsCommand,
    ) -> Result<DriveVersionPage, sdkwork_notes_pages_service::error::NotesProductError> {
        let current = self
            .records
            .lock()
            .await
            .get(&command.page_id)
            .cloned()
            .ok_or_else(|| {
                sdkwork_notes_pages_service::error::NotesProductError::NotFound(
                    "page content not found".to_string(),
                )
            })?;

        Ok(DriveVersionPage {
            items: vec![
                DriveVersionSummary {
                    drive_version_id: current.drive_version_id,
                    drive_version_no: current.drive_version_no,
                    version_kind: "auto".to_string(),
                    version_label: Some("Autosave".to_string()),
                    change_summary: Some("Autosave".to_string()),
                    created_at: "2026-06-08T00:00:02Z".to_string(),
                },
                DriveVersionSummary {
                    drive_version_id: format!("drive-version-{}-v1", command.page_id),
                    drive_version_no: 1,
                    version_kind: "initial".to_string(),
                    version_label: Some("Initial".to_string()),
                    change_summary: Some("Initial page".to_string()),
                    created_at: "2026-06-08T00:00:01Z".to_string(),
                },
            ],
            page_info: PageInfo {
                page: command.page,
                page_size: command.page_size,
                has_more: false,
                next_cursor: None,
            },
        })
    }
}
