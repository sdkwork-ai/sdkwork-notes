use async_trait::async_trait;
use sdkwork_notes_pages_repository_sqlx::install_sqlite_schema;
use sdkwork_notes_pages_repository_sqlx::notes_store::SqlNotesStore;
use sdkwork_notes_pages_service::domain::{
    AcceptAiSuggestionCommand, AiFeedback, AiJob, AiJobSource, AiSuggestion,
    ApplyAiSuggestionCommand, ClaimAiJobCommand, CompleteAiJobCommand, CompleteAiSuggestionInput,
    CreateAiFeedbackCommand, CreateAiJobCommand, CreatePageCommand, CreateWorkspaceCommand,
    DrivePageContentSnapshot, DriveVersionPage, DriveVersionSummary, ListAiJobsQuery,
    ListAiSuggestionFeedbackQuery, ListPageAiSuggestionsQuery, ListPageVersionsQuery,
    ListPagesQuery, ListWorkspacesQuery, NewAiFeedback, NewAiJob, NewAiSuggestion, NewPage,
    NewWorkspace, NotesActorContext, Page, PageInfo, PageKind, PageMetadataPatch,
    RejectAiSuggestionCommand, RestorePageVersionCommand, SearchQuery, UpdatePageContentCommand,
    UpdatePageMetadataCommand, Workspace,
};
use sdkwork_notes_pages_service::error::NotesProductError;
use sdkwork_notes_pages_service::ports::{
    CreateDrivePageContentCommand, DrivePageContentPort, ListDrivePageContentVersionsCommand,
    NotesRepository, ReadDrivePageContentCommand, RestoreDrivePageContentVersionCommand,
    UpdateDrivePageContentCommand,
};
use sdkwork_notes_pages_service::service::NotesService;
use serde_json::json;
use sqlx::any::AnyPoolOptions;
use sqlx::Row;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn page_content_lifecycle_stores_drive_refs_and_updates_current_version() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool.clone()), drive.clone());
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
            description: Some("AI notes workspace".to_string()),
            drive_space_id: "drive-space-001".to_string(),
            default_page_content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            default_page_schema_version: "1".to_string(),
            ai_index_policy_code: "default".to_string(),
        })
        .await
        .expect("workspace should be created");
    assert_eq!(workspace.drive_space_id, "drive-space-001");

    let page = service
        .create_page(CreatePageCommand {
            id: "page-001".to_string(),
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "hello" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await
        .expect("page should be created");

    assert_eq!(page.drive_space_id, "drive-space-001");
    assert_eq!(page.drive_node_id, "drive-node-page-001");
    assert_eq!(
        page.drive_uri,
        "drive://spaces/drive-space-001/nodes/drive-node-page-001"
    );
    assert_eq!(
        page.current_drive_version_id.as_str(),
        "drive-version-page-001-v1"
    );
    assert_eq!(page.current_drive_version_no, 1);

    let updated = service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "hello v2" }] }),
            content_type: Some("application/vnd.sdkwork.notes.page+json".to_string()),
            content_schema_version: Some("1".to_string()),
            change_summary: Some("Autosave".to_string()),
            expected_drive_version_id: Some("drive-version-page-001-v1".to_string()),
            create_checkpoint: false,
        })
        .await
        .expect("page content should update");
    assert_eq!(updated.drive_version_id, "drive-version-page-001-v2");
    assert_eq!(updated.drive_version_no, 2);

    let refreshed_page = service
        .get_page(&actor, &page.id)
        .await
        .expect("page should be readable");
    assert_eq!(
        refreshed_page.current_drive_version_id.as_str(),
        "drive-version-page-001-v2"
    );
    assert_eq!(refreshed_page.current_drive_version_no, 2);

    let content = service
        .get_page_content(&actor, &page.id)
        .await
        .expect("page content should be read through Drive");
    assert_eq!(content.content["blocks"][0]["text"], "hello v2");
    assert_eq!(content.drive_version_id, "drive-version-page-001-v2");
}

#[tokio::test]
async fn page_workflows_normalize_context_and_resource_ids_before_repository_and_drive_access() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };
    let padded_actor = NotesActorContext {
        tenant_id: " 100001 ".to_string(),
        organization_id: " 0 ".to_string(),
        operator_id: " 30 ".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
        .create_workspace(CreateWorkspaceCommand {
            id: " workspace-001 ".to_string(),
            context: actor.clone(),
            owner_subject_type: " user ".to_string(),
            owner_subject_id: " 30 ".to_string(),
            name: " Product Lab ".to_string(),
            description: None,
            drive_space_id: " drive-space-001 ".to_string(),
            default_page_content_type: " application/vnd.sdkwork.notes.page+json ".to_string(),
            default_page_schema_version: " 1 ".to_string(),
            ai_index_policy_code: " default ".to_string(),
        })
        .await
        .expect("workspace creation should normalize metadata");

    let page = service
        .create_page(CreatePageCommand {
            id: " page-001 ".to_string(),
            context: padded_actor.clone(),
            workspace_id: " workspace-001 ".to_string(),
            title: " Roadmap ".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "hello" }] }),
            content_type: " application/vnd.sdkwork.notes.page+json ".to_string(),
            content_schema_version: " 1 ".to_string(),
            change_summary: Some(" Initial page ".to_string()),
        })
        .await
        .expect("page creation should use normalized context and ids");
    assert_eq!(page.id, "page-001");
    assert_eq!(page.tenant_id, "100001");
    assert_eq!(page.organization_id, "0");
    assert_eq!(page.created_by, "30");
    assert_eq!(page.title, "Roadmap");

    let listed = service
        .list_pages(ListPagesQuery {
            context: padded_actor.clone(),
            workspace_id: " workspace-001 ".to_string(),
            page: 1,
            page_size: 20,
            q: Some(" roadmap ".to_string()),
        })
        .await
        .expect("page list should use normalized context and workspace id");
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0].id, "page-001");

    let bootstrap = service
        .get_workspace_bootstrap(&padded_actor, " workspace-001 ")
        .await
        .expect("workspace bootstrap should use normalized context and workspace id");
    assert_eq!(bootstrap.root_pages.len(), 1);
    assert_eq!(bootstrap.root_pages[0].id, "page-001");

    let updated_page = service
        .update_page_metadata(UpdatePageMetadataCommand {
            context: padded_actor.clone(),
            page_id: " page-001 ".to_string(),
            title: Some(" Roadmap v2 ".to_string()),
            favorite: Some(true),
            archive_status: Some(" active ".to_string()),
            publish_status: Some(" private ".to_string()),
            parent_page_id: None,
            expected_version: Some(" 1 ".to_string()),
        })
        .await
        .expect("metadata update should normalize context, ids, and patch values");
    assert_eq!(updated_page.title, "Roadmap v2");
    assert!(updated_page.favorite);

    let content = service
        .update_page_content(UpdatePageContentCommand {
            context: padded_actor.clone(),
            page_id: " page-001 ".to_string(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "hello v2" }] }),
            content_type: Some(" application/vnd.sdkwork.notes.page+json ".to_string()),
            content_schema_version: Some(" 1 ".to_string()),
            change_summary: Some(" Autosave ".to_string()),
            expected_drive_version_id: Some(" drive-version-page-001-v1 ".to_string()),
            create_checkpoint: false,
        })
        .await
        .expect("content update should use normalized context and page id");
    assert_eq!(content.page_id, "page-001");
    assert_eq!(content.drive_version_id, "drive-version-page-001-v2");

    let versions = service
        .list_page_versions(ListPageVersionsQuery {
            context: padded_actor,
            page_id: " page-001 ".to_string(),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("version list should use normalized context and page id");
    assert_eq!(versions.items.len(), 2);

    let request = drive
        .last_version_list_request()
        .await
        .expect("Drive version list request should be recorded");
    assert_eq!(request.tenant_id, "100001");
    assert_eq!(request.organization_id, "0");
    assert_eq!(request.page_id, "page-001");
}

#[tokio::test]
async fn update_page_content_validates_payload_before_drive_content_is_written() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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
            workspace_id: workspace.id,
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

    let drive_update_count = drive.update_count("page-001").await;
    let invalid_content = service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!("plain text is not the notes page envelope"),
            content_type: Some("application/vnd.sdkwork.notes.page+json".to_string()),
            content_schema_version: Some("1".to_string()),
            change_summary: Some("Invalid content".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: false,
        })
        .await;
    assert!(matches!(
        invalid_content,
        Err(NotesProductError::Validation(_))
    ));
    assert_eq!(drive.update_count("page-001").await, drive_update_count);
}

#[tokio::test]
async fn update_page_content_rejects_oversized_content_metadata_before_drive_content_is_written() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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
            workspace_id: workspace.id,
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

    let drive_update_count = drive.update_count("page-001").await;
    let oversized_content_type = service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [] }),
            content_type: Some("x".repeat(256)),
            content_schema_version: Some("1".to_string()),
            change_summary: Some("Invalid update".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: false,
        })
        .await;
    assert!(matches!(
        oversized_content_type,
        Err(NotesProductError::Validation(_))
    ));
    assert_eq!(drive.update_count("page-001").await, drive_update_count);

    let oversized_schema_version = service
        .update_page_content(UpdatePageContentCommand {
            context: actor,
            page_id: page.id,
            content: json!({ "blocks": [] }),
            content_type: Some("application/vnd.sdkwork.notes.page+json".to_string()),
            content_schema_version: Some("v".repeat(33)),
            change_summary: Some("Invalid update".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id),
            create_checkpoint: false,
        })
        .await;
    assert!(matches!(
        oversized_schema_version,
        Err(NotesProductError::Validation(_))
    ));
    assert_eq!(drive.update_count("page-001").await, drive_update_count);
}

#[tokio::test]
async fn create_page_rejects_invalid_drive_snapshot_before_notes_page_is_persisted() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    drive.invalidate_next_create_drive_version_id().await;
    let service = NotesService::new(SqlNotesStore::new(pool.clone()), drive);
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

    let result = service
        .create_page(CreatePageCommand {
            id: "page-invalid-drive-version".to_string(),
            context: actor.clone(),
            workspace_id: workspace.id,
            title: "Invalid Drive Snapshot".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await;
    assert!(matches!(result, Err(NotesProductError::Internal(_))));
    let page_count: i64 =
        sqlx::query_scalar("SELECT COUNT(1) FROM notes_page WHERE id='page-invalid-drive-version'")
            .fetch_one(&pool)
            .await
            .expect("notes_page count should be readable");
    assert_eq!(page_count, 0);
}

#[tokio::test]
async fn create_page_rejects_drive_snapshot_with_unexpected_content_metadata_before_notes_page_is_persisted(
) {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool.clone()), drive.clone());
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

    drive
        .make_next_create_snapshot_use_wrong_content_metadata()
        .await;
    let result = service
        .create_page(CreatePageCommand {
            id: "page-wrong-content-metadata".to_string(),
            context: actor,
            workspace_id: workspace.id,
            title: "Wrong Drive Metadata".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await;
    assert!(matches!(result, Err(NotesProductError::Internal(_))));

    let page_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(1) FROM notes_page WHERE id='page-wrong-content-metadata'",
    )
    .fetch_one(&pool)
    .await
    .expect("notes_page count should be readable");
    assert_eq!(page_count, 0);
}

#[tokio::test]
async fn create_page_reports_reconciliation_when_notes_insert_fails_after_drive_content_is_created()
{
    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(ConcurrentPageInsertRepository, drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    let result = service
        .create_page(CreatePageCommand {
            id: "page-001".to_string(),
            context: actor,
            workspace_id: "workspace-001".to_string(),
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "hello" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await;

    assert_reconciliation_required(result, "Drive page content create succeeded");
    assert_eq!(drive.create_count("page-001").await, 1);
}

#[tokio::test]
async fn update_page_content_rejects_invalid_drive_snapshot_before_notes_page_is_advanced() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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
            workspace_id: workspace.id,
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

    drive.invalidate_next_update_drive_version_id().await;
    let result = service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "v2" }] }),
            content_type: Some("application/vnd.sdkwork.notes.page+json".to_string()),
            content_schema_version: Some("1".to_string()),
            change_summary: Some("Invalid Drive update".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: false,
        })
        .await;
    assert!(matches!(result, Err(NotesProductError::Internal(_))));

    let unchanged_page = service
        .get_page(&actor, &page.id)
        .await
        .expect("page metadata should still be readable");
    assert_eq!(
        unchanged_page.current_drive_version_id,
        page.current_drive_version_id
    );
    assert_eq!(
        unchanged_page.current_drive_version_no,
        page.current_drive_version_no
    );
}

#[tokio::test]
async fn update_page_content_rejects_drive_snapshot_with_unexpected_content_metadata_before_notes_page_is_advanced(
) {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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
            workspace_id: workspace.id,
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

    drive
        .make_next_update_snapshot_use_wrong_content_metadata()
        .await;
    let result = service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "v2" }] }),
            content_type: Some("application/vnd.sdkwork.notes.page+json".to_string()),
            content_schema_version: Some("1".to_string()),
            change_summary: Some("Wrong content metadata".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: false,
        })
        .await;
    assert!(matches!(result, Err(NotesProductError::Internal(_))));

    let unchanged_page = service
        .get_page(&actor, &page.id)
        .await
        .expect("page metadata should still be readable");
    assert_eq!(
        unchanged_page.current_drive_version_id,
        page.current_drive_version_id
    );
    assert_eq!(
        unchanged_page.current_drive_version_no,
        page.current_drive_version_no
    );
    assert_eq!(unchanged_page.content_type, page.content_type);
    assert_eq!(
        unchanged_page.content_schema_version,
        page.content_schema_version
    );
}

#[tokio::test]
async fn update_page_content_rejects_drive_snapshot_that_does_not_advance_version() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
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
            workspace_id: "workspace-001".to_string(),
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

    drive
        .make_next_update_snapshot_reuse_current_drive_version()
        .await;
    let result = service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "v2" }] }),
            content_type: Some("application/vnd.sdkwork.notes.page+json".to_string()),
            content_schema_version: Some("1".to_string()),
            change_summary: Some("Non-advancing Drive update".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: false,
        })
        .await;
    assert!(matches!(result, Err(NotesProductError::Internal(_))));

    let unchanged_page = service
        .get_page(&actor, &page.id)
        .await
        .expect("page metadata should still be readable");
    assert_eq!(
        unchanged_page.current_drive_version_id,
        page.current_drive_version_id
    );
    assert_eq!(
        unchanged_page.current_drive_version_no,
        page.current_drive_version_no
    );
    assert_eq!(unchanged_page.version, page.version);
}

#[tokio::test]
async fn update_page_content_rejects_drive_snapshot_for_wrong_node_before_notes_page_is_advanced() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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
            workspace_id: workspace.id,
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

    drive.make_next_update_snapshot_use_wrong_drive_node().await;
    let result = service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "v2" }] }),
            content_type: Some("application/vnd.sdkwork.notes.page+json".to_string()),
            content_schema_version: Some("1".to_string()),
            change_summary: Some("Wrong Drive node".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: false,
        })
        .await;
    assert!(matches!(result, Err(NotesProductError::Internal(_))));

    let unchanged_page = service
        .get_page(&actor, &page.id)
        .await
        .expect("page metadata should still be readable");
    assert_eq!(unchanged_page.drive_node_id, page.drive_node_id);
    assert_eq!(
        unchanged_page.current_drive_version_id,
        page.current_drive_version_id
    );
    assert_eq!(
        unchanged_page.current_drive_version_no,
        page.current_drive_version_no
    );
}

#[tokio::test]
async fn update_page_content_rejects_non_object_drive_content_before_notes_page_is_advanced() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
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
            workspace_id: "workspace-001".to_string(),
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

    drive
        .make_next_update_snapshot_use_non_object_content()
        .await;
    let result = service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "v2" }] }),
            content_type: None,
            content_schema_version: None,
            change_summary: Some("Invalid Drive content".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: false,
        })
        .await;
    assert!(matches!(result, Err(NotesProductError::Internal(_))));

    let unchanged_page = service
        .get_page(&actor, &page.id)
        .await
        .expect("page metadata should still be readable");
    assert_eq!(
        unchanged_page.current_drive_version_id,
        page.current_drive_version_id
    );
    assert_eq!(
        unchanged_page.current_drive_version_no,
        page.current_drive_version_no
    );
}

#[tokio::test]
async fn get_page_content_rejects_drive_snapshot_that_does_not_match_current_notes_pointer() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
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
            workspace_id: "workspace-001".to_string(),
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "hello" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await
        .expect("page should be created");

    drive
        .make_next_read_snapshot_use_wrong_drive_version()
        .await;
    let result = service.get_page_content(&actor, &page.id).await;
    assert!(matches!(result, Err(NotesProductError::Internal(_))));
}

#[tokio::test]
async fn update_page_content_preserves_existing_content_metadata_when_request_omits_it() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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
            id: "page-custom-schema".to_string(),
            context: actor.clone(),
            workspace_id: workspace.id,
            title: "Custom Schema".to_string(),
            page_kind: PageKind::Canvas,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [], "schema": "custom" }),
            content_type: "application/vnd.sdkwork.notes.canvas+json".to_string(),
            content_schema_version: "2".to_string(),
            change_summary: Some("Initial custom page".to_string()),
        })
        .await
        .expect("page should be created");

    let updated = service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "canvas", "text": "v2" }] }),
            content_type: None,
            content_schema_version: None,
            change_summary: Some("Content-only update".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: false,
        })
        .await
        .expect("content-only update should preserve current content metadata");
    assert_eq!(
        updated.content_type,
        "application/vnd.sdkwork.notes.canvas+json"
    );
    assert_eq!(updated.content_schema_version, "2");

    let refreshed_page = service
        .get_page(&actor, &page.id)
        .await
        .expect("page should be readable");
    assert_eq!(
        refreshed_page.content_type,
        "application/vnd.sdkwork.notes.canvas+json"
    );
    assert_eq!(refreshed_page.content_schema_version, "2");
}

#[tokio::test]
async fn update_page_content_defaults_drive_expected_version_to_current_notes_pointer() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
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
            workspace_id: "workspace-001".to_string(),
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
        .update_page_content(UpdatePageContentCommand {
            context: actor,
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "v2" }] }),
            content_type: None,
            content_schema_version: None,
            change_summary: Some("Autosave".to_string()),
            expected_drive_version_id: None,
            create_checkpoint: false,
        })
        .await
        .expect("page content should update");

    let request = drive
        .last_update_request()
        .await
        .expect("Drive update request should be recorded");
    assert_eq!(
        request.expected_drive_version_id.as_deref(),
        Some(page.current_drive_version_id.as_str())
    );
}

#[tokio::test]
async fn update_page_content_rejects_stale_expected_drive_version_before_drive_write() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
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
            workspace_id: "workspace-001".to_string(),
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
    let stale_drive_version_id = page.current_drive_version_id.clone();

    service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "v2" }] }),
            content_type: None,
            content_schema_version: None,
            change_summary: Some("Autosave".to_string()),
            expected_drive_version_id: Some(stale_drive_version_id.clone()),
            create_checkpoint: false,
        })
        .await
        .expect("page content should update to v2");
    let drive_update_count = drive.update_count(&page.id).await;

    let stale_result = service
        .update_page_content(UpdatePageContentCommand {
            context: actor,
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "stale v3" }] }),
            content_type: None,
            content_schema_version: None,
            change_summary: Some("Stale autosave".to_string()),
            expected_drive_version_id: Some(stale_drive_version_id),
            create_checkpoint: false,
        })
        .await;

    assert!(matches!(stale_result, Err(NotesProductError::Conflict(_))));
    assert_eq!(drive.update_count(&page.id).await, drive_update_count);
}

#[tokio::test]
async fn create_page_rejects_duplicate_page_id_before_drive_content_is_written() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
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

    service
        .create_page(CreatePageCommand {
            id: "page-001".to_string(),
            context: actor.clone(),
            workspace_id: "workspace-001".to_string(),
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
    let drive_create_count = drive.create_count("page-001").await;

    let duplicate = service
        .create_page(CreatePageCommand {
            id: "page-001".to_string(),
            context: actor,
            workspace_id: "workspace-001".to_string(),
            title: "Duplicate Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "text": "duplicate" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Duplicate page".to_string()),
        })
        .await;

    assert!(matches!(duplicate, Err(NotesProductError::Conflict(_))));
    assert_eq!(drive.create_count("page-001").await, drive_create_count);
}

#[tokio::test]
async fn create_page_rejects_globally_reserved_page_id_before_drive_content_is_written() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
    let first_actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };
    let second_actor = NotesActorContext {
        tenant_id: "100002".to_string(),
        organization_id: "300002".to_string(),
        operator_id: "user-002".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
        .create_workspace(CreateWorkspaceCommand {
            id: "workspace-001".to_string(),
            context: first_actor.clone(),
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
        .expect("first workspace should be created");
    service
        .create_workspace(CreateWorkspaceCommand {
            id: "workspace-002".to_string(),
            context: second_actor.clone(),
            owner_subject_type: "user".to_string(),
            owner_subject_id: "user-002".to_string(),
            name: "Research Lab".to_string(),
            description: None,
            drive_space_id: "drive-space-002".to_string(),
            default_page_content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            default_page_schema_version: "1".to_string(),
            ai_index_policy_code: "default".to_string(),
        })
        .await
        .expect("second workspace should be created");

    service
        .create_page(CreatePageCommand {
            id: "page-global-001".to_string(),
            context: first_actor,
            workspace_id: "workspace-001".to_string(),
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
        .expect("first page should be created");
    let drive_create_count = drive.create_count("page-global-001").await;

    let duplicate = service
        .create_page(CreatePageCommand {
            id: "page-global-001".to_string(),
            context: second_actor,
            workspace_id: "workspace-002".to_string(),
            title: "Tenant-local duplicate".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "text": "duplicate" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Duplicate page".to_string()),
        })
        .await;

    assert!(matches!(duplicate, Err(NotesProductError::Conflict(_))));
    assert_eq!(
        drive.create_count("page-global-001").await,
        drive_create_count
    );
}

#[tokio::test]
async fn create_page_validates_metadata_before_drive_content_is_written() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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

    let too_long_title_result = service
        .create_page(CreatePageCommand {
            id: "page-title-too-long".to_string(),
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            title: "T".repeat(513),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Invalid create".to_string()),
        })
        .await;
    assert!(matches!(
        too_long_title_result,
        Err(NotesProductError::Validation(_))
    ));
    assert!(!drive.has_page("page-title-too-long").await);

    let missing_parent_result = service
        .create_page(CreatePageCommand {
            id: "page-missing-parent".to_string(),
            context: actor,
            workspace_id: workspace.id,
            title: "Child page".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: Some("missing-parent".to_string()),
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Invalid parent".to_string()),
        })
        .await;
    assert!(matches!(
        missing_parent_result,
        Err(NotesProductError::NotFound(_))
    ));
    assert!(!drive.has_page("page-missing-parent").await);
}

#[tokio::test]
async fn create_page_rejects_oversized_content_metadata_before_drive_content_is_written() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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

    let oversized_content_type = service
        .create_page(CreatePageCommand {
            id: "page-content-type-too-long".to_string(),
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            title: "Invalid content type".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [] }),
            content_type: "x".repeat(256),
            content_schema_version: "1".to_string(),
            change_summary: Some("Invalid create".to_string()),
        })
        .await;
    assert!(matches!(
        oversized_content_type,
        Err(NotesProductError::Validation(_))
    ));
    assert_eq!(drive.create_count("page-content-type-too-long").await, 0);

    let oversized_schema_version = service
        .create_page(CreatePageCommand {
            id: "page-schema-version-too-long".to_string(),
            context: actor,
            workspace_id: workspace.id,
            title: "Invalid schema version".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "v".repeat(33),
            change_summary: Some("Invalid create".to_string()),
        })
        .await;
    assert!(matches!(
        oversized_schema_version,
        Err(NotesProductError::Validation(_))
    ));
    assert_eq!(drive.create_count("page-schema-version-too-long").await, 0);
}

#[tokio::test]
async fn create_workspace_normalizes_and_validates_metadata_before_sql_constraints() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    let workspace = service
        .create_workspace(CreateWorkspaceCommand {
            id: " workspace-trimmed ".to_string(),
            context: actor.clone(),
            owner_subject_type: " user ".to_string(),
            owner_subject_id: " 30 ".to_string(),
            name: " Product Lab ".to_string(),
            description: Some(" AI notes workspace ".to_string()),
            drive_space_id: " drive-space-trimmed ".to_string(),
            default_page_content_type: " application/vnd.sdkwork.notes.page+json ".to_string(),
            default_page_schema_version: " 1 ".to_string(),
            ai_index_policy_code: " default ".to_string(),
        })
        .await
        .expect("workspace metadata should be normalized before insert");
    assert_eq!(workspace.id, "workspace-trimmed");
    assert_eq!(workspace.owner_subject_type, "user");
    assert_eq!(workspace.owner_subject_id, "30");
    assert_eq!(workspace.name, "Product Lab");
    assert_eq!(workspace.description.as_deref(), Some("AI notes workspace"));
    assert_eq!(workspace.drive_space_id, "drive-space-trimmed");
    assert_eq!(
        workspace.default_page_content_type,
        "application/vnd.sdkwork.notes.page+json"
    );
    assert_eq!(workspace.default_page_schema_version, "1");
    assert_eq!(workspace.ai_index_policy_code, "default");

    let invalid_owner_type = service
        .create_workspace(CreateWorkspaceCommand {
            id: "workspace-invalid-owner".to_string(),
            context: actor.clone(),
            owner_subject_type: "team".to_string(),
            owner_subject_id: "30".to_string(),
            name: "Invalid Owner".to_string(),
            description: None,
            drive_space_id: "drive-space-invalid-owner".to_string(),
            default_page_content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            default_page_schema_version: "1".to_string(),
            ai_index_policy_code: "default".to_string(),
        })
        .await;
    assert!(matches!(
        invalid_owner_type,
        Err(NotesProductError::Validation(_))
    ));

    let too_long_name = service
        .create_workspace(CreateWorkspaceCommand {
            id: "workspace-name-too-long".to_string(),
            context: actor,
            owner_subject_type: "user".to_string(),
            owner_subject_id: "30".to_string(),
            name: "W".repeat(121),
            description: None,
            drive_space_id: "drive-space-name-too-long".to_string(),
            default_page_content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            default_page_schema_version: "1".to_string(),
            ai_index_policy_code: "default".to_string(),
        })
        .await;
    assert!(matches!(
        too_long_name,
        Err(NotesProductError::Validation(_))
    ));
}

#[tokio::test]
async fn read_models_list_bootstrap_and_update_page_metadata_without_drive_content_changes() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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
            description: Some("AI notes workspace".to_string()),
            drive_space_id: "drive-space-001".to_string(),
            default_page_content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            default_page_schema_version: "1".to_string(),
            ai_index_policy_code: "default".to_string(),
        })
        .await
        .expect("workspace should be created");

    service
        .create_workspace(CreateWorkspaceCommand {
            id: "workspace-002".to_string(),
            context: actor.clone(),
            owner_subject_type: "user".to_string(),
            owner_subject_id: "30".to_string(),
            name: "Research Lab".to_string(),
            description: None,
            drive_space_id: "drive-space-002".to_string(),
            default_page_content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            default_page_schema_version: "1".to_string(),
            ai_index_policy_code: "default".to_string(),
        })
        .await
        .expect("second workspace should be created");

    let first_page = service
        .create_page(CreatePageCommand {
            id: "page-roadmap".to_string(),
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            title: "Apollo roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "roadmap" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial roadmap".to_string()),
        })
        .await
        .expect("root page should be created");

    service
        .create_page(CreatePageCommand {
            id: "page-child".to_string(),
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            title: "Apollo child detail".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: Some(first_page.id.clone()),
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial child".to_string()),
        })
        .await
        .expect("child page should be created");

    service
        .create_page(CreatePageCommand {
            id: "page-meeting".to_string(),
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            title: "Release meeting".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial meeting".to_string()),
        })
        .await
        .expect("second root page should be created");

    let workspaces = service
        .list_workspaces(ListWorkspacesQuery {
            context: actor.clone(),
            page: 1,
            page_size: 1,
        })
        .await
        .expect("workspaces should be listed");
    assert_eq!(workspaces.items.len(), 1);
    assert_eq!(workspaces.page_info.page, 1);
    assert_eq!(workspaces.page_info.page_size, 1);
    assert!(workspaces.page_info.has_more);

    let roadmap_pages = service
        .list_pages(ListPagesQuery {
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            page: 1,
            page_size: 20,
            q: Some("roadmap".to_string()),
        })
        .await
        .expect("pages should be searchable by title");
    assert_eq!(roadmap_pages.items.len(), 1);
    assert_eq!(roadmap_pages.items[0].id, "page-roadmap");
    assert_eq!(
        roadmap_pages.items[0].current_drive_version_no.as_str(),
        "1"
    );

    let oversized_query = service
        .list_pages(ListPagesQuery {
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            page: 1,
            page_size: 20,
            q: Some("x".repeat(201)),
        })
        .await;
    assert!(matches!(
        oversized_query,
        Err(NotesProductError::Validation(_))
    ));

    let bootstrap = service
        .get_workspace_bootstrap(&actor, &workspace.id)
        .await
        .expect("workspace bootstrap should be returned");
    assert_eq!(bootstrap.workspace.id, workspace.id);
    assert_eq!(bootstrap.root_pages.len(), 2);
    assert!(bootstrap
        .root_pages
        .iter()
        .all(|page| page.id != "page-child"));
    assert!(bootstrap
        .object_types
        .iter()
        .any(|object_type| object_type.code == "database"));

    let updated_page = service
        .update_page_metadata(UpdatePageMetadataCommand {
            context: actor.clone(),
            page_id: first_page.id.clone(),
            title: Some("Apollo roadmap v2".to_string()),
            favorite: Some(true),
            archive_status: Some("archived".to_string()),
            publish_status: Some("unlisted".to_string()),
            parent_page_id: None,
            expected_version: Some(first_page.version.to_string()),
        })
        .await
        .expect("page metadata should update");
    assert_eq!(updated_page.title, "Apollo roadmap v2");
    assert!(updated_page.favorite);
    assert_eq!(
        updated_page.current_drive_version_id,
        first_page.current_drive_version_id
    );
    assert_eq!(
        updated_page.current_drive_version_no,
        first_page.current_drive_version_no
    );

    let stale_update = service
        .update_page_metadata(UpdatePageMetadataCommand {
            context: actor,
            page_id: first_page.id,
            title: Some("stale title".to_string()),
            favorite: None,
            archive_status: None,
            publish_status: None,
            parent_page_id: None,
            expected_version: Some("1".to_string()),
        })
        .await;
    assert!(matches!(stale_update, Err(NotesProductError::Conflict(_))));
}

#[tokio::test]
async fn page_versions_are_listed_from_drive_without_notes_revision_rows() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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
            workspace_id: workspace.id,
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "hello" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await
        .expect("page should be created");

    service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "hello v2" }] }),
            content_type: Some("application/vnd.sdkwork.notes.page+json".to_string()),
            content_schema_version: Some("1".to_string()),
            change_summary: Some("Autosave".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: false,
        })
        .await
        .expect("page content should update");

    let versions = service
        .list_page_versions(ListPageVersionsQuery {
            context: actor,
            page_id: page.id,
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page versions should be listed through Drive");

    assert_eq!(versions.page_info.page, 1);
    assert_eq!(versions.page_info.page_size, 20);
    assert!(!versions.page_info.has_more);
    assert_eq!(versions.items.len(), 2);
    assert_eq!(
        versions.items[0].drive_version_id,
        "drive-version-page-001-v2"
    );
    assert_eq!(versions.items[0].drive_version_no, 2);
    assert_eq!(versions.items[0].version_kind, "auto");
    assert_eq!(
        versions.items[1].drive_version_id,
        "drive-version-page-001-v1"
    );

    let request = drive
        .last_version_list_request()
        .await
        .expect("Drive version list request should be recorded");
    assert_eq!(request.drive_space_id, "drive-space-001");
    assert_eq!(request.drive_node_id, "drive-node-page-001");
    assert_eq!(
        request.drive_uri,
        "drive://spaces/drive-space-001/nodes/drive-node-page-001"
    );
}

#[tokio::test]
async fn page_versions_reject_invalid_drive_version_summaries() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
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
            workspace_id: "workspace-001".to_string(),
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

    drive.invalidate_next_version_list_summary().await;
    let result = service
        .list_page_versions(ListPageVersionsQuery {
            context: actor,
            page_id: page.id,
            page: 1,
            page_size: 20,
        })
        .await;

    assert!(matches!(result, Err(NotesProductError::Internal(_))));
}

#[tokio::test]
async fn page_versions_reject_unbounded_drive_version_pages() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
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
            workspace_id: "workspace-001".to_string(),
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

    drive.make_next_version_list_unbounded().await;
    let result = service
        .list_page_versions(ListPageVersionsQuery {
            context: actor,
            page_id: page.id,
            page: 1,
            page_size: 1,
        })
        .await;

    assert!(matches!(result, Err(NotesProductError::Internal(_))));
}

#[tokio::test]
async fn page_versions_reject_drive_page_info_mismatches() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
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
            workspace_id: "workspace-001".to_string(),
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

    drive.make_next_version_list_page_info_mismatch().await;
    let result = service
        .list_page_versions(ListPageVersionsQuery {
            context: actor,
            page_id: page.id,
            page: 2,
            page_size: 10,
        })
        .await;

    assert!(matches!(result, Err(NotesProductError::Internal(_))));
}

#[tokio::test]
async fn restore_page_version_creates_drive_owned_restore_version_and_advances_notes_pointer() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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
            workspace_id: workspace.id,
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "hello" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await
        .expect("page should be created");

    service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "hello v2" }] }),
            content_type: Some("application/vnd.sdkwork.notes.page+json".to_string()),
            content_schema_version: Some("1".to_string()),
            change_summary: Some("Autosave".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: false,
        })
        .await
        .expect("page content should update before restore");

    let restored = service
        .restore_page_version(RestorePageVersionCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            drive_version_id: page.current_drive_version_id.clone(),
            expected_current_drive_version_id: Some("drive-version-page-001-v2".to_string()),
        })
        .await
        .expect("page version should be restored through Drive");

    assert_eq!(restored.drive_version_id, "drive-version-page-001-v3");
    assert_eq!(restored.drive_version_no, 3);
    assert_eq!(restored.content["blocks"][0]["text"], "hello");

    let refreshed_page = service
        .get_page(&actor, &page.id)
        .await
        .expect("page should be readable after restore");
    assert_eq!(
        refreshed_page.current_drive_version_id,
        "drive-version-page-001-v3"
    );
    assert_eq!(refreshed_page.current_drive_version_no, 3);

    let request = drive
        .last_restore_request()
        .await
        .expect("Drive restore request should be recorded");
    assert_eq!(request.drive_version_id, "drive-version-page-001-v1");
    assert_eq!(
        request.current_drive_version_id,
        "drive-version-page-001-v2"
    );
    assert_eq!(
        request.expected_current_drive_version_id.as_deref(),
        Some("drive-version-page-001-v2")
    );
}

#[tokio::test]
async fn restore_page_version_defaults_expected_current_drive_version_to_notes_pointer() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
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
            workspace_id: "workspace-001".to_string(),
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "hello" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await
        .expect("page should be created");

    service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "hello v2" }] }),
            content_type: None,
            content_schema_version: None,
            change_summary: Some("Autosave".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: false,
        })
        .await
        .expect("page content should update before restore");

    service
        .restore_page_version(RestorePageVersionCommand {
            context: actor,
            page_id: page.id.clone(),
            drive_version_id: page.current_drive_version_id.clone(),
            expected_current_drive_version_id: None,
        })
        .await
        .expect("page version should restore");

    let request = drive
        .last_restore_request()
        .await
        .expect("Drive restore request should be recorded");
    assert_eq!(
        request.expected_current_drive_version_id.as_deref(),
        Some("drive-version-page-001-v2")
    );
}

#[tokio::test]
async fn restore_page_version_reports_reconciliation_when_notes_pointer_changes_after_drive_restore(
) {
    let drive = FakeDrivePageContentPort::default();
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };
    drive
        .create_page_content(CreateDrivePageContentCommand {
            tenant_id: actor.tenant_id.clone(),
            organization_id: actor.organization_id.clone(),
            operator_id: actor.operator_id.clone(),
            caller: actor.caller.clone(),
            workspace_id: "workspace-001".to_string(),
            page_id: "page-001".to_string(),
            title: "Roadmap".to_string(),
            drive_space_id: "drive-space-001".to_string(),
            parent_page_id: None,
            folder_drive_node_id: None,
            content: json!({ "blocks": [{ "type": "paragraph", "text": "hello" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await
        .expect("Drive content seed should be created");
    let service = NotesService::new(ConcurrentDriveSnapshotRepository, drive.clone());

    let result = service
        .restore_page_version(RestorePageVersionCommand {
            context: actor,
            page_id: "page-001".to_string(),
            drive_version_id: "drive-version-page-001-v1".to_string(),
            expected_current_drive_version_id: Some("drive-version-page-001-v1".to_string()),
        })
        .await;

    assert_reconciliation_required(result, "Drive page content restore succeeded");
    let request = drive
        .last_restore_request()
        .await
        .expect("Drive restore request should be recorded");
    assert_eq!(request.drive_version_id, "drive-version-page-001-v1");
}

#[tokio::test]
async fn restore_page_version_rejects_drive_snapshot_that_does_not_advance_version() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
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
            workspace_id: "workspace-001".to_string(),
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "hello" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await
        .expect("page should be created");
    let current = service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "hello v2" }] }),
            content_type: None,
            content_schema_version: None,
            change_summary: Some("Autosave".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: false,
        })
        .await
        .expect("page content should update before restore");

    drive
        .make_next_restore_snapshot_reuse_current_drive_version()
        .await;
    let result = service
        .restore_page_version(RestorePageVersionCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            drive_version_id: page.current_drive_version_id,
            expected_current_drive_version_id: Some(current.drive_version_id),
        })
        .await;
    assert!(matches!(result, Err(NotesProductError::Internal(_))));

    let unchanged_page = service
        .get_page(&actor, &page.id)
        .await
        .expect("page should remain readable after rejected restore");
    assert_eq!(
        unchanged_page.current_drive_version_id,
        "drive-version-page-001-v2"
    );
    assert_eq!(unchanged_page.current_drive_version_no, 2);
}

#[tokio::test]
async fn restore_page_version_rejects_drive_snapshot_with_oversized_content_metadata_before_notes_pointer_is_advanced(
) {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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
            workspace_id: workspace.id,
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "hello" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await
        .expect("page should be created");
    let current = service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "hello v2" }] }),
            content_type: None,
            content_schema_version: None,
            change_summary: Some("Autosave".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: false,
        })
        .await
        .expect("page content should update before restore");

    drive
        .make_next_restore_snapshot_use_oversized_content_metadata()
        .await;
    let restore_result = service
        .restore_page_version(RestorePageVersionCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            drive_version_id: page.current_drive_version_id,
            expected_current_drive_version_id: Some(current.drive_version_id),
        })
        .await;
    assert!(matches!(
        restore_result,
        Err(NotesProductError::Internal(_))
    ));

    let unchanged_page = service
        .get_page(&actor, &page.id)
        .await
        .expect("page should remain readable after rejected restore");
    assert_eq!(
        unchanged_page.current_drive_version_id,
        "drive-version-page-001-v2"
    );
    assert_eq!(unchanged_page.current_drive_version_no, 2);
    assert_eq!(
        unchanged_page.content_type,
        "application/vnd.sdkwork.notes.page+json"
    );
    assert_eq!(unchanged_page.content_schema_version, "1");
}

#[tokio::test]
async fn update_page_metadata_rejects_when_repository_version_changed_after_service_read() {
    let service = NotesService::new(
        ConcurrentMetadataRepository,
        FakeDrivePageContentPort::default(),
    );
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    let stale_result = service
        .update_page_metadata(UpdatePageMetadataCommand {
            context: actor.clone(),
            page_id: "page-001".to_string(),
            title: Some("Stale client title".to_string()),
            favorite: None,
            archive_status: None,
            publish_status: None,
            parent_page_id: None,
            expected_version: Some("1".to_string()),
        })
        .await;

    assert!(matches!(stale_result, Err(NotesProductError::Conflict(_))));
}

#[tokio::test]
async fn update_page_content_rejects_when_notes_current_drive_version_changed_before_db_advance() {
    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(ConcurrentDriveSnapshotRepository, drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    let result = service
        .update_page_content(UpdatePageContentCommand {
            context: actor,
            page_id: "page-001".to_string(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "stale content" }] }),
            content_type: Some("application/vnd.sdkwork.notes.page+json".to_string()),
            content_schema_version: Some("1".to_string()),
            change_summary: Some("Stale update".to_string()),
            expected_drive_version_id: Some("drive-version-page-001-v1".to_string()),
            create_checkpoint: false,
        })
        .await;

    assert_reconciliation_required(result, "Drive page content update succeeded");
    assert_eq!(drive.update_count("page-001").await, 1);
}

#[tokio::test]
async fn search_query_returns_page_summaries_with_drive_version_provenance() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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

    service
        .create_workspace(CreateWorkspaceCommand {
            id: "workspace-002".to_string(),
            context: actor.clone(),
            owner_subject_type: "user".to_string(),
            owner_subject_id: "30".to_string(),
            name: "Research Lab".to_string(),
            description: None,
            drive_space_id: "drive-space-002".to_string(),
            default_page_content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            default_page_schema_version: "1".to_string(),
            ai_index_policy_code: "default".to_string(),
        })
        .await
        .expect("second workspace should be created");

    let roadmap = service
        .create_page(CreatePageCommand {
            id: "page-roadmap".to_string(),
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            title: "Apollo roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "launch notes" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial roadmap".to_string()),
        })
        .await
        .expect("roadmap page should be created");

    service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: roadmap.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "roadmap launch v2" }] }),
            content_type: Some("application/vnd.sdkwork.notes.page+json".to_string()),
            content_schema_version: Some("1".to_string()),
            change_summary: Some("Roadmap update".to_string()),
            expected_drive_version_id: Some(roadmap.current_drive_version_id),
            create_checkpoint: true,
        })
        .await
        .expect("roadmap content should update");

    service
        .create_page(CreatePageCommand {
            id: "page-other-workspace".to_string(),
            context: actor.clone(),
            workspace_id: "workspace-002".to_string(),
            title: "Apollo roadmap outside scope".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial other page".to_string()),
        })
        .await
        .expect("other workspace page should be created");

    let search = service
        .query_search(SearchQuery {
            context: actor,
            workspace_id: Some(workspace.id),
            q: Some("roadmap".to_string()),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("search should return current page projections");

    assert_eq!(search.page_info.page, 1);
    assert_eq!(search.page_info.page_size, 20);
    assert!(!search.page_info.has_more);
    assert_eq!(search.items.len(), 1);
    assert_eq!(search.items[0].page.id, "page-roadmap");
    assert_eq!(search.items[0].page.current_drive_version_no.as_str(), "2");
    assert_eq!(
        search.items[0].source_drive_version_id.as_deref(),
        Some("drive-version-page-roadmap-v2")
    );
    assert_eq!(search.items[0].source_drive_version_no, "2");
    assert_eq!(search.items[0].highlights, vec!["Apollo roadmap"]);
}

#[tokio::test]
async fn search_query_highlights_current_projection_snippet_when_match_comes_from_index() {
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
        SqlNotesStore::new(pool.clone()),
        FakeDrivePageContentPort::default(),
    );
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
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
            id: "page-indexed-only".to_string(),
            context: actor.clone(),
            workspace_id: "workspace-001".to_string(),
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "hello" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await
        .expect("page should be created");

    sqlx::query(
        "UPDATE notes_page_search_projection
         SET plain_text='Indexed body mentions Atlas only here',
             snippet='Atlas projection snippet',
             index_status='indexed',
             indexed_at=CURRENT_TIMESTAMP
         WHERE tenant_id='100001'
           AND organization_id='0'
           AND page_id=$1",
    )
    .bind(&page.id)
    .execute(&pool)
    .await
    .expect("current search projection should be updated");

    let search = service
        .query_search(SearchQuery {
            context: actor,
            workspace_id: Some("workspace-001".to_string()),
            q: Some("Atlas".to_string()),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("search should match indexed projection text");

    assert_eq!(search.items.len(), 1);
    assert_eq!(search.items[0].page.id, "page-indexed-only");
    assert_eq!(
        search.items[0].page.snippet.as_deref(),
        Some("Atlas projection snippet")
    );
    assert_eq!(search.items[0].highlights, vec!["Atlas projection snippet"]);
}

#[tokio::test]
async fn ai_job_creation_records_page_source_drive_version_provenance_and_idempotency() {
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
        SqlNotesStore::new(pool.clone()),
        FakeDrivePageContentPort::default(),
    );
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
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "hello" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await
        .expect("page should be created");

    service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "hello v2" }] }),
            content_type: Some("application/vnd.sdkwork.notes.page+json".to_string()),
            content_schema_version: Some("1".to_string()),
            change_summary: Some("Autosave".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: true,
        })
        .await
        .expect("page content should update");

    let command = CreateAiJobCommand {
        context: actor.clone(),
        workspace_id: workspace.id.clone(),
        job_type: "summarize".to_string(),
        target_type: "page".to_string(),
        target_id: Some(page.id.clone()),
        prompt: Some("Summarize this page".to_string()),
        context_policy: Some(json!({ "source": "current_page" })),
        idempotency_key: "ai-job-create-001".to_string(),
    };

    let job = service
        .create_ai_job(command.clone())
        .await
        .expect("AI job should be queued");
    assert_eq!(job.workspace_id, workspace.id);
    assert_eq!(job.job_type, "summarize");
    assert_eq!(job.target_type, "page");
    assert_eq!(job.target_id.as_deref(), Some("page-001"));
    assert_eq!(job.status, "queued");
    assert!(job.result.is_none());

    let source_row = sqlx::query(
        "SELECT source_type, source_id, drive_node_id, drive_version_id, drive_version_no
         FROM notes_ai_job_source
         WHERE tenant_id=$1 AND organization_id=$2 AND job_id=$3",
    )
    .bind(&actor.tenant_id)
    .bind(&actor.organization_id)
    .bind(&job.id)
    .fetch_one(&pool)
    .await
    .expect("AI job source should be persisted");

    let source_type: String = source_row.get("source_type");
    let source_id: String = source_row.get("source_id");
    let drive_node_id: String = source_row.get("drive_node_id");
    let drive_version_id: String = source_row.get("drive_version_id");
    let drive_version_no: i64 = source_row.get("drive_version_no");
    assert_eq!(source_type, "page");
    assert_eq!(source_id, "page-001");
    assert_eq!(drive_node_id, "drive-node-page-001");
    assert_eq!(drive_version_id, "drive-version-page-001-v2");
    assert_eq!(drive_version_no, 2);

    let replay = service
        .create_ai_job(command.clone())
        .await
        .expect("same idempotency key and payload should replay existing AI job");
    assert_eq!(replay.id, job.id);

    let conflicting_replay = service
        .create_ai_job(CreateAiJobCommand {
            prompt: Some("Use the same key for a different request".to_string()),
            ..command
        })
        .await;
    assert!(matches!(
        conflicting_replay,
        Err(NotesProductError::Conflict(_))
    ));
}

#[tokio::test]
async fn ai_job_creation_replays_existing_job_when_concurrent_idempotent_insert_wins_race() {
    let repository = ConcurrentAiJobIdempotencyRepository::default();
    let service = NotesService::new(repository.clone(), FakeDrivePageContentPort::default());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    let job = service
        .create_ai_job(CreateAiJobCommand {
            context: actor,
            workspace_id: "workspace-001".to_string(),
            job_type: "summarize".to_string(),
            target_type: "page".to_string(),
            target_id: Some("page-001".to_string()),
            prompt: Some("Summarize this page".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-job-concurrent-001".to_string(),
        })
        .await
        .expect("concurrent duplicate idempotency insert should replay existing AI job");

    assert_eq!(job.status, "queued");
    assert_eq!(job.idempotency_key, "ai-job-concurrent-001");
    assert_eq!(job.target_id.as_deref(), Some("page-001"));
    assert_eq!(repository.find_attempts(), 2);
    assert_eq!(repository.insert_attempts(), 1);
}

#[tokio::test]
async fn page_ai_job_creation_rejects_missing_target_id_before_repository_reads() {
    let repository = PanicOnAiJobTargetRepository;
    let service = NotesService::new(repository, FakeDrivePageContentPort::default());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    let result = service
        .create_ai_job(CreateAiJobCommand {
            context: actor,
            workspace_id: "workspace-001".to_string(),
            job_type: "summarize".to_string(),
            target_type: "page".to_string(),
            target_id: None,
            prompt: Some("Summarize this page".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-job-missing-page-target-001".to_string(),
        })
        .await;

    assert!(matches!(result, Err(NotesProductError::Validation(_))));
}

#[tokio::test]
async fn ai_job_creation_hashes_and_resolves_normalized_request_values() {
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

    let normalized_job = service
        .create_ai_job(CreateAiJobCommand {
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            job_type: "summarize".to_string(),
            target_type: "page".to_string(),
            target_id: Some(page.id.clone()),
            prompt: Some("Summarize this page".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-job-normalization-001".to_string(),
        })
        .await
        .expect("normalized AI job should be created");

    let replay_with_spaces = service
        .create_ai_job(CreateAiJobCommand {
            context: actor,
            workspace_id: " workspace-001 ".to_string(),
            job_type: " summarize ".to_string(),
            target_type: " page ".to_string(),
            target_id: Some(" page-001 ".to_string()),
            prompt: Some(" Summarize this page ".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: " ai-job-normalization-001 ".to_string(),
        })
        .await
        .expect("equivalent normalized AI job request should replay");

    assert_eq!(replay_with_spaces.id, normalized_job.id);
    assert_eq!(replay_with_spaces.workspace_id, "workspace-001");
    assert_eq!(replay_with_spaces.target_id.as_deref(), Some("page-001"));
}

#[tokio::test]
async fn ai_workflows_normalize_context_and_resource_ids_before_repository_and_drive_access() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };
    let padded_actor = NotesActorContext {
        tenant_id: " 100001 ".to_string(),
        organization_id: " 0 ".to_string(),
        operator_id: " 30 ".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
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
            workspace_id: "workspace-001".to_string(),
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "hello" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await
        .expect("page should be created");

    let job = service
        .create_ai_job(CreateAiJobCommand {
            context: padded_actor.clone(),
            workspace_id: " workspace-001 ".to_string(),
            job_type: " summarize ".to_string(),
            target_type: " page ".to_string(),
            target_id: Some(" page-001 ".to_string()),
            prompt: Some(" Summarize ".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: " ai-normalized-001 ".to_string(),
        })
        .await
        .expect("AI job should be created with normalized request values");

    let listed_jobs = service
        .list_ai_jobs(ListAiJobsQuery {
            context: padded_actor.clone(),
            workspace_id: Some(" workspace-001 ".to_string()),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("AI jobs should be listed with normalized workspace filter");
    assert_eq!(listed_jobs.items.len(), 1);
    assert_eq!(listed_jobs.items[0].id, job.id);

    service
        .get_ai_job(&padded_actor, &format!(" {} ", job.id))
        .await
        .expect("AI job should be retrieved with normalized id");
    service
        .claim_ai_job(ClaimAiJobCommand {
            context: padded_actor.clone(),
            ai_job_id: format!(" {} ", job.id),
        })
        .await
        .expect("AI job should be claimed with normalized id");
    service
        .complete_ai_job(CompleteAiJobCommand {
            context: padded_actor.clone(),
            ai_job_id: format!(" {} ", job.id),
            suggestions: vec![CompleteAiSuggestionInput {
                page_id: Some(" page-001 ".to_string()),
                suggestion_type: " summary ".to_string(),
                payload: json!({
                    "content": { "blocks": [{ "type": "paragraph", "text": "AI draft" }] },
                    "contentType": " application/vnd.sdkwork.notes.page+json ",
                    "contentSchemaVersion": " 1 "
                }),
            }],
        })
        .await
        .expect("AI job should complete with normalized suggestion inputs");

    let suggestions = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: padded_actor.clone(),
            page_id: " page-001 ".to_string(),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("AI suggestions should be listed with normalized page id");
    assert_eq!(suggestions.items.len(), 1);
    let suggestion = suggestions.items[0].clone();
    assert_eq!(suggestion.page_id, page.id);

    service
        .accept_ai_suggestion(AcceptAiSuggestionCommand {
            context: padded_actor.clone(),
            ai_suggestion_id: format!(" {} ", suggestion.id),
        })
        .await
        .expect("AI suggestion should be accepted with normalized id");
    let content = service
        .apply_ai_suggestion(ApplyAiSuggestionCommand {
            context: padded_actor.clone(),
            ai_suggestion_id: format!(" {} ", suggestion.id),
            expected_drive_version_id: Some(" drive-version-page-001-v1 ".to_string()),
            create_checkpoint: true,
        })
        .await
        .expect("AI suggestion should apply with normalized ids and Drive expectation");
    assert_eq!(content.page_id, "page-001");
    assert_eq!(content.content["blocks"][0]["text"], "AI draft");

    let feedback = service
        .create_ai_feedback(CreateAiFeedbackCommand {
            context: padded_actor.clone(),
            ai_suggestion_id: format!(" {} ", suggestion.id),
            feedback_type: " helpful ".to_string(),
            feedback_text: Some(" Useful ".to_string()),
        })
        .await
        .expect("AI feedback should be recorded with normalized suggestion id");
    assert_eq!(feedback.feedback_text.as_deref(), Some("Useful"));

    let feedback_page = service
        .list_ai_suggestion_feedback(ListAiSuggestionFeedbackQuery {
            context: padded_actor,
            ai_suggestion_id: format!(" {} ", suggestion.id),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("AI feedback should be listed with normalized suggestion id");
    assert_eq!(feedback_page.items.len(), 1);
    assert_eq!(feedback_page.items[0].id, feedback.id);

    assert_eq!(drive.update_count("page-001").await, 1);
}

#[tokio::test]
async fn backend_ai_job_admin_lists_retrieves_and_cancels_jobs() {
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
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "admin-001".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    let workspace = service
        .create_workspace(CreateWorkspaceCommand {
            id: "workspace-001".to_string(),
            context: actor.clone(),
            owner_subject_type: "user".to_string(),
            owner_subject_id: "admin-001".to_string(),
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
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "hello" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial page".to_string()),
        })
        .await
        .expect("page should be created");

    let job = service
        .create_ai_job(CreateAiJobCommand {
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            job_type: "summarize".to_string(),
            target_type: "page".to_string(),
            target_id: Some(page.id),
            prompt: Some("Summarize this page".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-job-admin-001".to_string(),
        })
        .await
        .expect("AI job should be created");

    let jobs = service
        .list_ai_jobs(ListAiJobsQuery {
            context: actor.clone(),
            workspace_id: Some(workspace.id.clone()),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("AI jobs should be listed");
    assert_eq!(jobs.items.len(), 1);
    assert_eq!(jobs.items[0].id, job.id);
    assert_eq!(jobs.items[0].source_count, 1);
    assert_eq!(jobs.items[0].suggestion_count, 0);
    assert!(!jobs.page_info.has_more);

    let retrieved = service
        .get_ai_job(&actor, &job.id)
        .await
        .expect("AI job should be retrieved");
    assert_eq!(retrieved.id, job.id);
    assert_eq!(retrieved.status, "queued");
    assert_eq!(retrieved.source_count, 1);

    let canceled = service
        .cancel_ai_job(&actor, &job.id)
        .await
        .expect("AI job should cancel");
    assert_eq!(canceled.status, "canceled");
    assert_eq!(canceled.source_count, 1);

    let second_cancel = service
        .cancel_ai_job(&actor, &job.id)
        .await
        .expect("canceling an already canceled AI job should be idempotent");
    assert_eq!(second_cancel.status, "canceled");
}

#[tokio::test]
async fn ai_job_claim_is_exclusive_and_rejects_second_worker_claim() {
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
    let creator = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "worker-creator".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };
    let first_worker = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "worker-001".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };
    let second_worker = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "worker-002".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    let workspace = service
        .create_workspace(CreateWorkspaceCommand {
            id: "workspace-001".to_string(),
            context: creator.clone(),
            owner_subject_type: "user".to_string(),
            owner_subject_id: "worker-creator".to_string(),
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
            context: creator.clone(),
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
            context: creator.clone(),
            workspace_id: workspace.id,
            job_type: "summarize".to_string(),
            target_type: "page".to_string(),
            target_id: Some(page.id),
            prompt: Some("Summarize this page".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-job-exclusive-claim-001".to_string(),
        })
        .await
        .expect("AI job should be created");

    let claimed = service
        .claim_ai_job(ClaimAiJobCommand {
            context: first_worker,
            ai_job_id: job.id.clone(),
        })
        .await
        .expect("first worker should claim the queued AI job");
    assert_eq!(claimed.status, "running");

    let second_claim = service
        .claim_ai_job(ClaimAiJobCommand {
            context: second_worker.clone(),
            ai_job_id: job.id.clone(),
        })
        .await;
    assert!(matches!(second_claim, Err(NotesProductError::Conflict(_))));

    let running = service
        .get_ai_job(&second_worker, &job.id)
        .await
        .expect("AI job should remain readable after rejected claim");
    assert_eq!(running.status, "running");
}

#[tokio::test]
async fn ai_job_worker_claims_completes_and_lists_page_suggestions() {
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
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "worker-001".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    let workspace = service
        .create_workspace(CreateWorkspaceCommand {
            id: "workspace-001".to_string(),
            context: actor.clone(),
            owner_subject_type: "user".to_string(),
            owner_subject_id: "worker-001".to_string(),
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

    service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "launch plan v2" }] }),
            content_type: Some("application/vnd.sdkwork.notes.page+json".to_string()),
            content_schema_version: Some("1".to_string()),
            change_summary: Some("Content selected for AI".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: true,
        })
        .await
        .expect("page content should update before AI job is created");

    let job = service
        .create_ai_job(CreateAiJobCommand {
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            job_type: "summarize".to_string(),
            target_type: "page".to_string(),
            target_id: Some(page.id.clone()),
            prompt: Some("Summarize this page".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-job-worker-001".to_string(),
        })
        .await
        .expect("AI job should be created");

    let claimed = service
        .claim_ai_job(ClaimAiJobCommand {
            context: actor.clone(),
            ai_job_id: job.id.clone(),
        })
        .await
        .expect("AI job should be claimed");
    assert_eq!(claimed.status, "running");
    assert_eq!(claimed.suggestion_count, 0);

    let completed = service
        .complete_ai_job(CompleteAiJobCommand {
            context: actor.clone(),
            ai_job_id: job.id.clone(),
            suggestions: vec![CompleteAiSuggestionInput {
                page_id: Some(page.id.clone()),
                suggestion_type: "summary".to_string(),
                payload: json!({
                    "summary": "Launch plan v2 is ready for review.",
                    "confidence": "high"
                }),
            }],
        })
        .await
        .expect("AI job should complete with a suggestion");
    assert_eq!(completed.status, "succeeded");
    assert_eq!(completed.source_count, 1);
    assert_eq!(completed.suggestion_count, 1);

    let suggestions = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor.clone(),
            page_id: page.id.clone(),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page AI suggestions should be listed");
    assert_eq!(suggestions.items.len(), 1);
    assert_eq!(suggestions.items[0].page_id, page.id);
    assert_eq!(suggestions.items[0].ai_job_id, job.id);
    assert_eq!(suggestions.items[0].suggestion_type, "summary");
    assert_eq!(suggestions.items[0].status, "proposed");
    assert_eq!(
        suggestions.items[0].source_drive_version_id.as_deref(),
        Some("drive-version-page-001-v2")
    );
    assert_eq!(suggestions.items[0].source_drive_version_no, Some(2));
    assert_eq!(
        suggestions.items[0].payload["summary"],
        "Launch plan v2 is ready for review."
    );
    assert!(!suggestions.page_info.has_more);

    let second_complete = service
        .complete_ai_job(CompleteAiJobCommand {
            context: actor,
            ai_job_id: job.id,
            suggestions: vec![CompleteAiSuggestionInput {
                page_id: Some("page-001".to_string()),
                suggestion_type: "summary".to_string(),
                payload: json!({ "summary": "duplicate" }),
            }],
        })
        .await;
    assert!(matches!(
        second_complete,
        Err(NotesProductError::Conflict(_))
    ));
}

#[tokio::test]
async fn page_target_ai_job_rejects_suggestions_for_unscoped_pages() {
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
    let source_page = service
        .create_page(CreatePageCommand {
            id: "page-001".to_string(),
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            title: "Source".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "source" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial source".to_string()),
        })
        .await
        .expect("source page should be created");
    let unrelated_page = service
        .create_page(CreatePageCommand {
            id: "page-002".to_string(),
            context: actor.clone(),
            workspace_id: workspace.id.clone(),
            title: "Unrelated".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "other" }] }),
            content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            content_schema_version: "1".to_string(),
            change_summary: Some("Initial unrelated".to_string()),
        })
        .await
        .expect("unrelated page should be created");

    let job = service
        .create_ai_job(CreateAiJobCommand {
            context: actor.clone(),
            workspace_id: workspace.id,
            job_type: "summarize".to_string(),
            target_type: "page".to_string(),
            target_id: Some(source_page.id),
            prompt: Some("Summarize this page".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-job-page-scope-001".to_string(),
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

    let complete_result = service
        .complete_ai_job(CompleteAiJobCommand {
            context: actor.clone(),
            ai_job_id: job.id,
            suggestions: vec![CompleteAiSuggestionInput {
                page_id: Some(unrelated_page.id.clone()),
                suggestion_type: "summary".to_string(),
                payload: json!({ "summary": "wrong page" }),
            }],
        })
        .await;
    assert!(matches!(
        complete_result,
        Err(NotesProductError::Validation(_))
    ));

    let unrelated_suggestions = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor,
            page_id: unrelated_page.id,
            page: 1,
            page_size: 20,
        })
        .await
        .expect("unrelated page suggestions should be readable");
    assert!(unrelated_suggestions.items.is_empty());
}

#[tokio::test]
async fn ai_suggestion_decisions_accept_reject_and_conflict() {
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
            target_id: Some(page.id.clone()),
            prompt: Some("Summarize and tag this page".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-suggestion-decision-001".to_string(),
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
                    payload: json!({ "summary": "Launch plan is ready." }),
                },
                CompleteAiSuggestionInput {
                    page_id: Some(page.id.clone()),
                    suggestion_type: "tag".to_string(),
                    payload: json!({ "tag": "launch" }),
                },
            ],
        })
        .await
        .expect("AI job should complete with suggestions");

    let suggestions = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor.clone(),
            page_id: page.id.clone(),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page suggestions should be listed");
    assert_eq!(suggestions.items.len(), 2);
    let summary_suggestion = suggestions
        .items
        .iter()
        .find(|suggestion| suggestion.suggestion_type == "summary")
        .expect("summary suggestion should exist");
    let tag_suggestion = suggestions
        .items
        .iter()
        .find(|suggestion| suggestion.suggestion_type == "tag")
        .expect("tag suggestion should exist");
    assert_eq!(summary_suggestion.status, "proposed");
    assert_eq!(tag_suggestion.status, "proposed");

    let accepted = service
        .accept_ai_suggestion(AcceptAiSuggestionCommand {
            context: actor.clone(),
            ai_suggestion_id: summary_suggestion.id.clone(),
        })
        .await
        .expect("summary suggestion should be accepted");
    assert_eq!(accepted.status, "accepted");
    assert_eq!(accepted.page_id, page.id);

    let accepted_replay = service
        .accept_ai_suggestion(AcceptAiSuggestionCommand {
            context: actor.clone(),
            ai_suggestion_id: summary_suggestion.id.clone(),
        })
        .await
        .expect("accepting an accepted suggestion should be idempotent");
    assert_eq!(accepted_replay.status, "accepted");

    let rejected = service
        .reject_ai_suggestion(RejectAiSuggestionCommand {
            context: actor.clone(),
            ai_suggestion_id: tag_suggestion.id.clone(),
        })
        .await
        .expect("tag suggestion should be rejected");
    assert_eq!(rejected.status, "rejected");

    let conflicting_reject = service
        .reject_ai_suggestion(RejectAiSuggestionCommand {
            context: actor,
            ai_suggestion_id: summary_suggestion.id.clone(),
        })
        .await;
    assert!(matches!(
        conflicting_reject,
        Err(NotesProductError::Conflict(_))
    ));
}

#[tokio::test]
async fn ai_suggestion_accept_rejects_when_concurrent_decision_wins_race() {
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
        SqlNotesStore::new(pool.clone()),
        FakeDrivePageContentPort::default(),
    );
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
            target_id: Some(page.id.clone()),
            prompt: Some("Summarize this page".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-suggestion-concurrent-decision-001".to_string(),
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
                page_id: Some(page.id.clone()),
                suggestion_type: "summary".to_string(),
                payload: json!({ "summary": "Roadmap is ready." }),
            }],
        })
        .await
        .expect("AI job should be completed");
    let suggestions = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor.clone(),
            page_id: page.id,
            page: 1,
            page_size: 20,
        })
        .await
        .expect("suggestion should be listed");
    let suggestion_id = suggestions.items[0].id.clone();

    let trigger_sql = format!(
        r#"
        CREATE TRIGGER notes_ai_suggestion_accept_concurrent_reject
        BEFORE UPDATE OF status ON notes_ai_suggestion
        WHEN OLD.id='{suggestion_id}'
          AND OLD.status='proposed'
          AND NEW.status='accepted'
        BEGIN
          UPDATE notes_ai_suggestion
             SET status='rejected',
                 updated_at=CURRENT_TIMESTAMP,
                 version=version + 1
           WHERE id=OLD.id;
          SELECT RAISE(IGNORE);
        END
        "#
    );
    sqlx::query(sqlx::AssertSqlSafe(trigger_sql))
        .execute(&pool)
        .await
        .expect("concurrent suggestion decision trigger should be installed");

    let accept_result = service
        .accept_ai_suggestion(AcceptAiSuggestionCommand {
            context: actor,
            ai_suggestion_id: suggestion_id,
        })
        .await;
    assert!(matches!(accept_result, Err(NotesProductError::Conflict(_))));
}

#[tokio::test]
async fn accepted_ai_suggestion_applies_drive_backed_page_content() {
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
            target_id: Some(page.id.clone()),
            prompt: Some("Rewrite this page into concise launch notes".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-suggestion-apply-001".to_string(),
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
                page_id: Some(page.id.clone()),
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
        .expect("AI job should complete with an applicable suggestion");

    let suggestions = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor.clone(),
            page_id: page.id.clone(),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page suggestions should be listed");
    let suggestion = suggestions
        .items
        .iter()
        .find(|suggestion| suggestion.suggestion_type == "summary")
        .expect("summary suggestion should exist");

    service
        .accept_ai_suggestion(AcceptAiSuggestionCommand {
            context: actor.clone(),
            ai_suggestion_id: suggestion.id.clone(),
        })
        .await
        .expect("summary suggestion should be accepted");

    let applied = service
        .apply_ai_suggestion(ApplyAiSuggestionCommand {
            context: actor.clone(),
            ai_suggestion_id: suggestion.id.clone(),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: true,
        })
        .await
        .expect("accepted suggestion should apply through Drive");
    assert_eq!(applied.page_id, page.id);
    assert_eq!(applied.drive_node_id, "drive-node-page-001");
    assert_eq!(applied.drive_version_id, "drive-version-page-001-v2");
    assert_eq!(applied.drive_version_no, 2);
    assert_eq!(applied.content["blocks"][0]["text"], "Launch notes");

    let refreshed_suggestions = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor.clone(),
            page_id: page.id.clone(),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page suggestions should be listed after apply");
    let applied_suggestion = refreshed_suggestions
        .items
        .iter()
        .find(|item| item.id == suggestion.id)
        .expect("applied suggestion should still be listed");
    assert_eq!(applied_suggestion.status, "applied");

    let refreshed_page = service
        .get_page(&actor, &page.id)
        .await
        .expect("page should be readable after suggestion apply");
    assert_eq!(
        refreshed_page.current_drive_version_id.as_str(),
        "drive-version-page-001-v2"
    );
}

#[tokio::test]
async fn ai_suggestion_apply_rejects_oversized_content_metadata_before_drive_content_is_written() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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
            job_type: "rewrite".to_string(),
            target_type: "page".to_string(),
            target_id: Some(page.id.clone()),
            prompt: Some("Rewrite this page".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-suggestion-metadata-too-long-001".to_string(),
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
                page_id: Some(page.id.clone()),
                suggestion_type: "rewrite".to_string(),
                payload: json!({
                    "content": { "blocks": [{ "type": "paragraph", "text": "AI draft" }] },
                    "contentType": "x".repeat(256),
                    "contentSchemaVersion": "1"
                }),
            }],
        })
        .await
        .expect("AI job should complete with oversized content metadata suggestion");
    let suggestion = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor.clone(),
            page_id: page.id.clone(),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page suggestions should be listed")
        .items[0]
        .clone();
    service
        .accept_ai_suggestion(AcceptAiSuggestionCommand {
            context: actor.clone(),
            ai_suggestion_id: suggestion.id.clone(),
        })
        .await
        .expect("suggestion should be accepted");

    let drive_update_count = drive.update_count("page-001").await;
    let apply_result = service
        .apply_ai_suggestion(ApplyAiSuggestionCommand {
            context: actor,
            ai_suggestion_id: suggestion.id,
            expected_drive_version_id: Some(page.current_drive_version_id),
            create_checkpoint: true,
        })
        .await;

    assert!(matches!(
        apply_result,
        Err(NotesProductError::Validation(_))
    ));
    assert_eq!(drive.update_count("page-001").await, drive_update_count);
}

#[tokio::test]
async fn stale_ai_suggestion_apply_requires_current_drive_version() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "initial" }] }),
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
            job_type: "rewrite".to_string(),
            target_type: "page".to_string(),
            target_id: Some(page.id.clone()),
            prompt: Some("Rewrite".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-suggestion-stale-apply-001".to_string(),
        })
        .await
        .expect("AI job should be created from page version 1");
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
                page_id: Some(page.id.clone()),
                suggestion_type: "rewrite".to_string(),
                payload: json!({
                    "content": { "blocks": [{ "type": "paragraph", "text": "stale rewrite" }] }
                }),
            }],
        })
        .await
        .expect("AI job should complete with suggestion");
    let suggestion = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor.clone(),
            page_id: page.id.clone(),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page suggestions should be listed")
        .items[0]
        .clone();
    service
        .accept_ai_suggestion(AcceptAiSuggestionCommand {
            context: actor.clone(),
            ai_suggestion_id: suggestion.id.clone(),
        })
        .await
        .expect("suggestion should be accepted");

    service
        .update_page_content(UpdatePageContentCommand {
            context: actor.clone(),
            page_id: page.id.clone(),
            content: json!({ "blocks": [{ "type": "paragraph", "text": "manual v2" }] }),
            content_type: Some("application/vnd.sdkwork.notes.page+json".to_string()),
            content_schema_version: Some("1".to_string()),
            change_summary: Some("Manual update".to_string()),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: false,
        })
        .await
        .expect("manual edit should advance page to version 2");
    let drive_update_count = drive.update_count(&page.id).await;

    let apply_result = service
        .apply_ai_suggestion(ApplyAiSuggestionCommand {
            context: actor.clone(),
            ai_suggestion_id: suggestion.id.clone(),
            expected_drive_version_id: None,
            create_checkpoint: true,
        })
        .await;
    assert!(matches!(apply_result, Err(NotesProductError::Conflict(_))));
    assert_eq!(drive.update_count(&page.id).await, drive_update_count);

    let still_accepted = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor.clone(),
            page_id: page.id.clone(),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page suggestions should remain readable")
        .items
        .into_iter()
        .find(|item| item.id == suggestion.id)
        .expect("stale suggestion should remain present");
    assert_eq!(still_accepted.status, "accepted");

    let content = service
        .get_page_content(&actor, &page.id)
        .await
        .expect("page content should still be readable");
    assert_eq!(content.content["blocks"][0]["text"], "manual v2");
}

#[tokio::test]
async fn ai_suggestion_apply_does_not_advance_notes_pointer_when_suggestion_status_changes_during_apply(
) {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = RejectSuggestionDuringDriveUpdatePort::default();
    let service = NotesService::new(SqlNotesStore::new(pool.clone()), drive.clone());
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
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "initial" }] }),
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
            job_type: "rewrite".to_string(),
            target_type: "page".to_string(),
            target_id: Some(page.id.clone()),
            prompt: Some("Rewrite".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-suggestion-racy-apply-001".to_string(),
        })
        .await
        .expect("AI job should be created from page version 1");
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
                page_id: Some(page.id.clone()),
                suggestion_type: "rewrite".to_string(),
                payload: json!({
                    "content": { "blocks": [{ "type": "paragraph", "text": "AI rewrite" }] }
                }),
            }],
        })
        .await
        .expect("AI job should complete with suggestion");
    let suggestion = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor.clone(),
            page_id: page.id.clone(),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page suggestions should be listed")
        .items[0]
        .clone();
    service
        .accept_ai_suggestion(AcceptAiSuggestionCommand {
            context: actor.clone(),
            ai_suggestion_id: suggestion.id.clone(),
        })
        .await
        .expect("suggestion should be accepted");

    drive
        .reject_suggestion_during_next_update(pool.clone(), suggestion.id.clone())
        .await;
    let apply_result = service
        .apply_ai_suggestion(ApplyAiSuggestionCommand {
            context: actor.clone(),
            ai_suggestion_id: suggestion.id,
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: true,
        })
        .await;
    assert_reconciliation_required(
        apply_result,
        "Drive page content update for AI suggestion succeeded",
    );

    let refreshed_page = service
        .get_page(&actor, &page.id)
        .await
        .expect("page should remain readable after failed apply");
    assert_eq!(
        refreshed_page.current_drive_version_id, page.current_drive_version_id,
        "Notes must not advance its current Drive pointer when suggestion status no longer applies"
    );
}

#[tokio::test]
async fn ai_suggestion_apply_rejects_drive_snapshot_that_does_not_advance_version() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "initial" }] }),
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
            job_type: "rewrite".to_string(),
            target_type: "page".to_string(),
            target_id: Some(page.id.clone()),
            prompt: Some("Rewrite".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-suggestion-non-advancing-apply-001".to_string(),
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
                page_id: Some(page.id.clone()),
                suggestion_type: "rewrite".to_string(),
                payload: json!({
                    "content": { "blocks": [{ "type": "paragraph", "text": "AI rewrite" }] }
                }),
            }],
        })
        .await
        .expect("AI job should complete with suggestion");
    let suggestion = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor.clone(),
            page_id: page.id.clone(),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page suggestions should be listed")
        .items[0]
        .clone();
    service
        .accept_ai_suggestion(AcceptAiSuggestionCommand {
            context: actor.clone(),
            ai_suggestion_id: suggestion.id.clone(),
        })
        .await
        .expect("suggestion should be accepted");

    drive
        .make_next_update_snapshot_reuse_current_drive_version()
        .await;
    let result = service
        .apply_ai_suggestion(ApplyAiSuggestionCommand {
            context: actor.clone(),
            ai_suggestion_id: suggestion.id.clone(),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: true,
        })
        .await;
    assert!(matches!(result, Err(NotesProductError::Internal(_))));

    let refreshed_page = service
        .get_page(&actor, &page.id)
        .await
        .expect("page should remain readable after rejected apply");
    assert_eq!(
        refreshed_page.current_drive_version_id,
        page.current_drive_version_id
    );
    let still_accepted = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor,
            page_id: page.id,
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page suggestions should remain readable")
        .items
        .into_iter()
        .find(|item| item.id == suggestion.id)
        .expect("suggestion should still exist");
    assert_eq!(still_accepted.status, "accepted");
}

#[tokio::test]
async fn ai_suggestion_apply_rejects_drive_snapshot_with_unexpected_content_metadata_before_notes_pointer_is_advanced(
) {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let drive = FakeDrivePageContentPort::default();
    let service = NotesService::new(SqlNotesStore::new(pool), drive.clone());
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
            initial_content: json!({ "blocks": [{ "type": "paragraph", "text": "initial" }] }),
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
            job_type: "rewrite".to_string(),
            target_type: "page".to_string(),
            target_id: Some(page.id.clone()),
            prompt: Some("Rewrite".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-suggestion-wrong-content-metadata-apply-001".to_string(),
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
                page_id: Some(page.id.clone()),
                suggestion_type: "rewrite".to_string(),
                payload: json!({
                    "content": { "blocks": [{ "type": "paragraph", "text": "AI rewrite" }] }
                }),
            }],
        })
        .await
        .expect("AI job should complete with suggestion");
    let suggestion = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor.clone(),
            page_id: page.id.clone(),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page suggestions should be listed")
        .items[0]
        .clone();
    service
        .accept_ai_suggestion(AcceptAiSuggestionCommand {
            context: actor.clone(),
            ai_suggestion_id: suggestion.id.clone(),
        })
        .await
        .expect("suggestion should be accepted");

    drive
        .make_next_update_snapshot_use_wrong_content_metadata()
        .await;
    let result = service
        .apply_ai_suggestion(ApplyAiSuggestionCommand {
            context: actor.clone(),
            ai_suggestion_id: suggestion.id.clone(),
            expected_drive_version_id: Some(page.current_drive_version_id.clone()),
            create_checkpoint: true,
        })
        .await;
    assert!(matches!(result, Err(NotesProductError::Internal(_))));

    let refreshed_page = service
        .get_page(&actor, &page.id)
        .await
        .expect("page should remain readable after rejected apply");
    assert_eq!(
        refreshed_page.current_drive_version_id,
        page.current_drive_version_id
    );
    assert_eq!(refreshed_page.content_type, page.content_type);
    assert_eq!(
        refreshed_page.content_schema_version,
        page.content_schema_version
    );
    let still_accepted = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor,
            page_id: page.id,
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page suggestions should remain readable")
        .items
        .into_iter()
        .find(|item| item.id == suggestion.id)
        .expect("suggestion should still exist");
    assert_eq!(still_accepted.status, "accepted");
}

#[tokio::test]
async fn proposed_ai_suggestion_cannot_be_applied() {
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
            target_id: Some(page.id.clone()),
            prompt: Some("Summarize".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-suggestion-apply-conflict-001".to_string(),
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
                page_id: Some(page.id.clone()),
                suggestion_type: "summary".to_string(),
                payload: json!({
                    "content": { "blocks": [{ "type": "paragraph", "text": "proposed" }] }
                }),
            }],
        })
        .await
        .expect("AI job should complete with a suggestion");

    let suggestions = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor.clone(),
            page_id: page.id.clone(),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page suggestions should be listed");
    let suggestion = suggestions.items[0].clone();

    let apply_result = service
        .apply_ai_suggestion(ApplyAiSuggestionCommand {
            context: actor,
            ai_suggestion_id: suggestion.id,
            expected_drive_version_id: Some(page.current_drive_version_id),
            create_checkpoint: true,
        })
        .await;
    assert!(matches!(apply_result, Err(NotesProductError::Conflict(_))));
}

#[tokio::test]
async fn ai_suggestion_feedback_is_recorded_for_quality_loop() {
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
            workspace_id: workspace.id.clone(),
            job_type: "summarize".to_string(),
            target_type: "page".to_string(),
            target_id: Some(page.id.clone()),
            prompt: Some("Summarize this page".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-suggestion-feedback-001".to_string(),
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
            ai_job_id: job.id.clone(),
            suggestions: vec![CompleteAiSuggestionInput {
                page_id: Some(page.id.clone()),
                suggestion_type: "summary".to_string(),
                payload: json!({ "summary": "Roadmap is ready." }),
            }],
        })
        .await
        .expect("AI job should complete with suggestion");

    let suggestions = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor.clone(),
            page_id: page.id.clone(),
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page suggestions should be listed");
    let suggestion = suggestions.items[0].clone();

    let feedback = service
        .create_ai_feedback(CreateAiFeedbackCommand {
            context: actor.clone(),
            ai_suggestion_id: suggestion.id.clone(),
            feedback_type: "helpful".to_string(),
            feedback_text: Some("Useful summary for launch review".to_string()),
        })
        .await
        .expect("AI suggestion feedback should be recorded");
    assert_eq!(feedback.workspace_id, workspace.id);
    assert_eq!(feedback.job_id, job.id);
    assert_eq!(
        feedback.suggestion_id.as_deref(),
        Some(suggestion.id.as_str())
    );
    assert_eq!(feedback.feedback_type, "helpful");
    assert_eq!(
        feedback.feedback_text.as_deref(),
        Some("Useful summary for launch review")
    );

    let replay = service
        .create_ai_feedback(CreateAiFeedbackCommand {
            context: actor.clone(),
            ai_suggestion_id: suggestion.id.clone(),
            feedback_type: "helpful".to_string(),
            feedback_text: Some("Useful summary for launch review".to_string()),
        })
        .await
        .expect("same feedback payload should be idempotent");
    assert_eq!(replay.id, feedback.id);

    let feedback_page = service
        .list_ai_suggestion_feedback(ListAiSuggestionFeedbackQuery {
            context: actor,
            ai_suggestion_id: suggestion.id,
            page: 1,
            page_size: 20,
        })
        .await
        .expect("AI suggestion feedback should be listed");
    assert_eq!(feedback_page.items.len(), 1);
    assert_eq!(feedback_page.items[0].id, feedback.id);
    assert!(!feedback_page.page_info.has_more);
}

#[tokio::test]
async fn ai_suggestion_feedback_replays_when_concurrent_insert_wins_race() {
    let repository = ConcurrentAiFeedbackIdempotencyRepository::default();
    let service = NotesService::new(repository.clone(), FakeDrivePageContentPort::default());
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    let feedback = service
        .create_ai_feedback(CreateAiFeedbackCommand {
            context: actor,
            ai_suggestion_id: "ai-suggestion-concurrent-feedback-001".to_string(),
            feedback_type: "helpful".to_string(),
            feedback_text: Some("Useful summary for launch review".to_string()),
        })
        .await
        .expect("concurrent duplicate feedback insert should replay existing feedback");

    assert_eq!(
        feedback.suggestion_id.as_deref(),
        Some("ai-suggestion-concurrent-feedback-001")
    );
    assert_eq!(feedback.feedback_type, "helpful");
    assert_eq!(
        feedback.feedback_text.as_deref(),
        Some("Useful summary for launch review")
    );
    assert_eq!(repository.find_feedback_attempts(), 2);
    assert_eq!(repository.insert_feedback_attempts(), 1);
}

#[tokio::test]
async fn invalid_ai_suggestion_feedback_type_is_rejected() {
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
    let actor = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };

    service
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
            workspace_id: "workspace-001".to_string(),
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
            workspace_id: "workspace-001".to_string(),
            job_type: "summarize".to_string(),
            target_type: "page".to_string(),
            target_id: Some(page.id.clone()),
            prompt: Some("Summarize".to_string()),
            context_policy: Some(json!({ "source": "current_page" })),
            idempotency_key: "ai-suggestion-feedback-invalid-001".to_string(),
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
                page_id: Some(page.id.clone()),
                suggestion_type: "summary".to_string(),
                payload: json!({ "summary": "Roadmap is ready." }),
            }],
        })
        .await
        .expect("AI job should complete with suggestion");

    let suggestions = service
        .list_page_ai_suggestions(ListPageAiSuggestionsQuery {
            context: actor.clone(),
            page_id: page.id,
            page: 1,
            page_size: 20,
        })
        .await
        .expect("page suggestions should be listed");

    let result = service
        .create_ai_feedback(CreateAiFeedbackCommand {
            context: actor,
            ai_suggestion_id: suggestions.items[0].id.clone(),
            feedback_type: "confusing".to_string(),
            feedback_text: None,
        })
        .await;
    assert!(matches!(result, Err(NotesProductError::Validation(_))));
}

#[derive(Clone, Default)]
struct ConcurrentMetadataRepository;

#[async_trait]
impl NotesRepository for ConcurrentMetadataRepository {
    async fn insert_workspace(&self, _: NewWorkspace) -> Result<Workspace, NotesProductError> {
        unreachable!("concurrency test does not insert workspaces")
    }

    async fn find_workspace(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Workspace, NotesProductError> {
        unreachable!("concurrency test does not read workspaces")
    }

    async fn list_workspaces(
        &self,
        _: &NotesActorContext,
        _: i64,
        _: i64,
    ) -> Result<Vec<Workspace>, NotesProductError> {
        unreachable!("concurrency test does not list workspaces")
    }

    async fn insert_page(&self, _: NewPage) -> Result<Page, NotesProductError> {
        unreachable!("concurrency test does not insert pages")
    }

    async fn page_id_is_reserved(&self, _: &str) -> Result<bool, NotesProductError> {
        Ok(false)
    }

    async fn find_page(
        &self,
        context: &NotesActorContext,
        page_id: &str,
    ) -> Result<Page, NotesProductError> {
        Ok(test_page(context, page_id, 1))
    }

    async fn list_pages(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
        _: Option<&str>,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("concurrency test does not list pages")
    }

    async fn list_root_pages(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("concurrency test does not list root pages")
    }

    async fn search_pages(
        &self,
        _: &NotesActorContext,
        _: Option<&str>,
        _: Option<&str>,
        _: i64,
        _: i64,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("concurrency test does not search pages")
    }

    async fn update_page_metadata(
        &self,
        context: &NotesActorContext,
        page_id: &str,
        patch: &PageMetadataPatch,
        expected_version: Option<i64>,
    ) -> Result<Page, NotesProductError> {
        if expected_version != Some(2) {
            return Err(NotesProductError::Conflict(
                "page version has changed".to_string(),
            ));
        }
        let mut page = test_page(context, page_id, 3);
        page.title.clone_from(&patch.title);
        page.favorite = patch.favorite;
        page.archive_status.clone_from(&patch.archive_status);
        page.publish_status.clone_from(&patch.publish_status);
        Ok(page)
    }

    async fn update_page_drive_snapshot(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &DrivePageContentSnapshot,
        _: &str,
    ) -> Result<Page, NotesProductError> {
        unreachable!("concurrency test does not update Drive snapshots")
    }

    async fn delete_page(&self, _: &NotesActorContext, _: &str) -> Result<(), NotesProductError> {
        unreachable!("concurrency test does not delete pages")
    }

    async fn find_ai_job_by_idempotency_key(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Option<AiJob>, NotesProductError> {
        unreachable!("concurrency test does not use AI jobs")
    }

    async fn insert_ai_job(&self, _: NewAiJob) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not insert AI jobs")
    }

    async fn list_ai_jobs(
        &self,
        _: &NotesActorContext,
        _: Option<&str>,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiJob>, NotesProductError> {
        unreachable!("concurrency test does not list AI jobs")
    }

    async fn find_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not find AI jobs")
    }

    async fn cancel_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not cancel AI jobs")
    }

    async fn claim_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not claim AI jobs")
    }

    async fn list_ai_job_sources(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Vec<AiJobSource>, NotesProductError> {
        unreachable!("concurrency test does not list AI job sources")
    }

    async fn fail_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("test fake does not fail AI jobs")
    }

    async fn complete_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: Vec<NewAiSuggestion>,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not complete AI jobs")
    }

    async fn list_page_ai_suggestions(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiSuggestion>, NotesProductError> {
        unreachable!("concurrency test does not list AI suggestions")
    }

    async fn find_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("concurrency test does not find AI suggestions")
    }

    async fn decide_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("concurrency test does not decide AI suggestions")
    }

    async fn apply_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
        _: &DrivePageContentSnapshot,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("concurrency test does not apply AI suggestions")
    }

    async fn find_ai_feedback(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiFeedback, NotesProductError> {
        unreachable!("concurrency test does not find AI feedback")
    }

    async fn insert_ai_feedback(&self, _: NewAiFeedback) -> Result<AiFeedback, NotesProductError> {
        unreachable!("concurrency test does not insert AI feedback")
    }

    async fn list_ai_suggestion_feedback(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiFeedback>, NotesProductError> {
        unreachable!("concurrency test does not list AI feedback")
    }
}

fn test_page(context: &NotesActorContext, page_id: &str, version: i64) -> Page {
    Page {
        id: page_id.to_string(),
        tenant_id: context.tenant_id.clone(),
        organization_id: context.organization_id.clone(),
        workspace_id: "workspace-001".to_string(),
        title: "Roadmap".to_string(),
        page_kind: PageKind::Doc,
        parent_page_id: None,
        folder_drive_node_id: None,
        drive_space_id: "drive-space-001".to_string(),
        drive_node_id: "drive-node-page-001".to_string(),
        drive_uri: "drive://spaces/drive-space-001/nodes/drive-node-page-001".to_string(),
        current_drive_version_id: "drive-version-page-001-v1".to_string(),
        current_drive_version_no: 1,
        content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
        content_schema_version: "1".to_string(),
        content_hash: Some("sha256:first".to_string()),
        snippet: Some("hello".to_string()),
        icon: None,
        cover_asset_id: None,
        favorite: false,
        archive_status: "active".to_string(),
        publish_status: "private".to_string(),
        word_count: 1,
        task_count: 0,
        drive_lifecycle_status_snapshot: None,
        lifecycle_status: "active".to_string(),
        version,
        created_by: context.operator_id.clone(),
        updated_by: context.operator_id.clone(),
        created_at: "2026-06-08T00:00:00Z".to_string(),
        updated_at: "2026-06-08T00:00:00Z".to_string(),
        deleted_at: None,
    }
}

fn test_workspace(context: &NotesActorContext, workspace_id: &str) -> Workspace {
    Workspace {
        id: workspace_id.to_string(),
        tenant_id: context.tenant_id.clone(),
        organization_id: context.organization_id.clone(),
        owner_subject_type: "user".to_string(),
        owner_subject_id: context.operator_id.clone(),
        name: "Product Lab".to_string(),
        description: None,
        drive_space_id: "drive-space-001".to_string(),
        default_page_content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
        default_page_schema_version: "1".to_string(),
        ai_index_policy_code: "default".to_string(),
        lifecycle_status: "active".to_string(),
        version: 1,
        created_by: context.operator_id.clone(),
        updated_by: context.operator_id.clone(),
        created_at: "2026-06-08T00:00:00Z".to_string(),
        updated_at: "2026-06-08T00:00:00Z".to_string(),
    }
}

fn assert_reconciliation_required<T>(result: Result<T, NotesProductError>, expected_prefix: &str) {
    let Err(NotesProductError::Internal(message)) = result else {
        panic!("expected reconciliation-required internal error");
    };
    assert!(
        message.starts_with(expected_prefix),
        "unexpected reconciliation error: {message}"
    );
    assert!(
        message.contains("Notes page pointer was not advanced")
            || message.contains("Notes page was not persisted")
            || message.contains("Notes AI suggestion apply was not committed"),
        "reconciliation error must identify the uncommitted Notes state: {message}"
    );
    assert!(
        message.contains("reconciliation is required"),
        "reconciliation error must ask for reconciliation: {message}"
    );
}

#[derive(Clone, Default)]
struct ConcurrentPageInsertRepository;

#[async_trait]
impl NotesRepository for ConcurrentPageInsertRepository {
    async fn insert_workspace(&self, _: NewWorkspace) -> Result<Workspace, NotesProductError> {
        unreachable!("concurrency test does not insert workspaces")
    }

    async fn find_workspace(
        &self,
        context: &NotesActorContext,
        workspace_id: &str,
    ) -> Result<Workspace, NotesProductError> {
        Ok(test_workspace(context, workspace_id))
    }

    async fn list_workspaces(
        &self,
        _: &NotesActorContext,
        _: i64,
        _: i64,
    ) -> Result<Vec<Workspace>, NotesProductError> {
        unreachable!("concurrency test does not list workspaces")
    }

    async fn insert_page(&self, _: NewPage) -> Result<Page, NotesProductError> {
        Err(NotesProductError::Conflict(
            "insert notes_page failed".to_string(),
        ))
    }

    async fn page_id_is_reserved(&self, _: &str) -> Result<bool, NotesProductError> {
        Ok(false)
    }

    async fn find_page(&self, _: &NotesActorContext, _: &str) -> Result<Page, NotesProductError> {
        Err(NotesProductError::NotFound("page not found".to_string()))
    }

    async fn list_pages(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
        _: Option<&str>,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("concurrency test does not list pages")
    }

    async fn list_root_pages(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("concurrency test does not list root pages")
    }

    async fn search_pages(
        &self,
        _: &NotesActorContext,
        _: Option<&str>,
        _: Option<&str>,
        _: i64,
        _: i64,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("concurrency test does not search pages")
    }

    async fn update_page_metadata(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &PageMetadataPatch,
        _: Option<i64>,
    ) -> Result<Page, NotesProductError> {
        unreachable!("concurrency test does not update metadata")
    }

    async fn update_page_drive_snapshot(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &DrivePageContentSnapshot,
        _: &str,
    ) -> Result<Page, NotesProductError> {
        unreachable!("concurrency test does not update Drive snapshots")
    }

    async fn delete_page(&self, _: &NotesActorContext, _: &str) -> Result<(), NotesProductError> {
        unreachable!("concurrency test does not delete pages")
    }

    async fn find_ai_job_by_idempotency_key(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Option<AiJob>, NotesProductError> {
        unreachable!("concurrency test does not use AI jobs")
    }

    async fn insert_ai_job(&self, _: NewAiJob) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not insert AI jobs")
    }

    async fn list_ai_jobs(
        &self,
        _: &NotesActorContext,
        _: Option<&str>,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiJob>, NotesProductError> {
        unreachable!("concurrency test does not list AI jobs")
    }

    async fn find_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not find AI jobs")
    }

    async fn cancel_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not cancel AI jobs")
    }

    async fn claim_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not claim AI jobs")
    }

    async fn list_ai_job_sources(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Vec<AiJobSource>, NotesProductError> {
        unreachable!("concurrency test does not list AI job sources")
    }

    async fn fail_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("test fake does not fail AI jobs")
    }

    async fn complete_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: Vec<NewAiSuggestion>,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not complete AI jobs")
    }

    async fn list_page_ai_suggestions(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiSuggestion>, NotesProductError> {
        unreachable!("concurrency test does not list AI suggestions")
    }

    async fn find_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("concurrency test does not find AI suggestions")
    }

    async fn decide_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("concurrency test does not decide AI suggestions")
    }

    async fn apply_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
        _: &DrivePageContentSnapshot,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("concurrency test does not apply AI suggestions")
    }

    async fn find_ai_feedback(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiFeedback, NotesProductError> {
        unreachable!("concurrency test does not find AI feedback")
    }

    async fn insert_ai_feedback(&self, _: NewAiFeedback) -> Result<AiFeedback, NotesProductError> {
        unreachable!("concurrency test does not insert AI feedback")
    }

    async fn list_ai_suggestion_feedback(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiFeedback>, NotesProductError> {
        unreachable!("concurrency test does not list AI feedback")
    }
}

#[derive(Clone, Default)]
struct ConcurrentAiJobIdempotencyRepository {
    find_attempts: Arc<AtomicUsize>,
    insert_attempts: Arc<AtomicUsize>,
    inserted_job: Arc<Mutex<Option<AiJob>>>,
}

impl ConcurrentAiJobIdempotencyRepository {
    fn find_attempts(&self) -> usize {
        self.find_attempts.load(Ordering::SeqCst)
    }

    fn insert_attempts(&self) -> usize {
        self.insert_attempts.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Default)]
struct PanicOnAiJobTargetRepository;

#[async_trait]
impl NotesRepository for PanicOnAiJobTargetRepository {
    async fn insert_workspace(&self, _: NewWorkspace) -> Result<Workspace, NotesProductError> {
        unreachable!("target validation should run before workspace insert")
    }

    async fn find_workspace(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Workspace, NotesProductError> {
        unreachable!("target validation should run before workspace read")
    }

    async fn list_workspaces(
        &self,
        _: &NotesActorContext,
        _: i64,
        _: i64,
    ) -> Result<Vec<Workspace>, NotesProductError> {
        unreachable!("target validation should run before workspace list")
    }

    async fn insert_page(&self, _: NewPage) -> Result<Page, NotesProductError> {
        unreachable!("target validation should run before page insert")
    }

    async fn page_id_is_reserved(&self, _: &str) -> Result<bool, NotesProductError> {
        unreachable!("target validation should run before page reservation check")
    }

    async fn find_page(&self, _: &NotesActorContext, _: &str) -> Result<Page, NotesProductError> {
        unreachable!("target validation should run before page read")
    }

    async fn list_pages(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
        _: Option<&str>,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("target validation should run before page list")
    }

    async fn list_root_pages(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("target validation should run before root page list")
    }

    async fn search_pages(
        &self,
        _: &NotesActorContext,
        _: Option<&str>,
        _: Option<&str>,
        _: i64,
        _: i64,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("target validation should run before search")
    }

    async fn update_page_metadata(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &PageMetadataPatch,
        _: Option<i64>,
    ) -> Result<Page, NotesProductError> {
        unreachable!("target validation should run before metadata update")
    }

    async fn update_page_drive_snapshot(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &DrivePageContentSnapshot,
        _: &str,
    ) -> Result<Page, NotesProductError> {
        unreachable!("target validation should run before Drive pointer update")
    }

    async fn delete_page(&self, _: &NotesActorContext, _: &str) -> Result<(), NotesProductError> {
        unreachable!("target validation should run before page delete")
    }

    async fn find_ai_job_by_idempotency_key(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Option<AiJob>, NotesProductError> {
        unreachable!("target validation should run before AI idempotency read")
    }

    async fn insert_ai_job(&self, _: NewAiJob) -> Result<AiJob, NotesProductError> {
        unreachable!("target validation should run before AI job insert")
    }

    async fn list_ai_jobs(
        &self,
        _: &NotesActorContext,
        _: Option<&str>,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiJob>, NotesProductError> {
        unreachable!("target validation should run before AI job list")
    }

    async fn find_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("target validation should run before AI job read")
    }

    async fn cancel_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("target validation should run before AI job cancel")
    }

    async fn claim_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("target validation should run before AI job claim")
    }

    async fn list_ai_job_sources(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Vec<AiJobSource>, NotesProductError> {
        unreachable!("target validation should run before AI job source list")
    }

    async fn fail_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("test fake does not fail AI jobs")
    }

    async fn complete_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: Vec<NewAiSuggestion>,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("target validation should run before AI job completion")
    }

    async fn list_page_ai_suggestions(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiSuggestion>, NotesProductError> {
        unreachable!("target validation should run before AI suggestion list")
    }

    async fn find_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("target validation should run before AI suggestion read")
    }

    async fn decide_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("target validation should run before AI suggestion decision")
    }

    async fn apply_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
        _: &DrivePageContentSnapshot,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("target validation should run before AI suggestion apply")
    }

    async fn find_ai_feedback(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiFeedback, NotesProductError> {
        unreachable!("target validation should run before AI feedback read")
    }

    async fn insert_ai_feedback(&self, _: NewAiFeedback) -> Result<AiFeedback, NotesProductError> {
        unreachable!("target validation should run before AI feedback insert")
    }

    async fn list_ai_suggestion_feedback(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiFeedback>, NotesProductError> {
        unreachable!("target validation should run before AI feedback list")
    }
}

#[async_trait]
impl NotesRepository for ConcurrentAiJobIdempotencyRepository {
    async fn insert_workspace(&self, _: NewWorkspace) -> Result<Workspace, NotesProductError> {
        unreachable!("concurrency test does not insert workspaces")
    }

    async fn find_workspace(
        &self,
        context: &NotesActorContext,
        workspace_id: &str,
    ) -> Result<Workspace, NotesProductError> {
        Ok(Workspace {
            id: workspace_id.to_string(),
            tenant_id: context.tenant_id.clone(),
            organization_id: context.organization_id.clone(),
            owner_subject_type: "user".to_string(),
            owner_subject_id: context.operator_id.clone(),
            name: "Product Lab".to_string(),
            description: None,
            drive_space_id: "drive-space-001".to_string(),
            default_page_content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
            default_page_schema_version: "1".to_string(),
            ai_index_policy_code: "default".to_string(),
            lifecycle_status: "active".to_string(),
            version: 1,
            created_by: context.operator_id.clone(),
            updated_by: context.operator_id.clone(),
            created_at: "2026-06-08T00:00:00Z".to_string(),
            updated_at: "2026-06-08T00:00:00Z".to_string(),
        })
    }

    async fn list_workspaces(
        &self,
        _: &NotesActorContext,
        _: i64,
        _: i64,
    ) -> Result<Vec<Workspace>, NotesProductError> {
        unreachable!("concurrency test does not list workspaces")
    }

    async fn insert_page(&self, _: NewPage) -> Result<Page, NotesProductError> {
        unreachable!("concurrency test does not insert pages")
    }

    async fn page_id_is_reserved(&self, _: &str) -> Result<bool, NotesProductError> {
        Ok(false)
    }

    async fn find_page(
        &self,
        context: &NotesActorContext,
        page_id: &str,
    ) -> Result<Page, NotesProductError> {
        Ok(test_page(context, page_id, 1))
    }

    async fn list_pages(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
        _: Option<&str>,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("concurrency test does not list pages")
    }

    async fn list_root_pages(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("concurrency test does not list root pages")
    }

    async fn search_pages(
        &self,
        _: &NotesActorContext,
        _: Option<&str>,
        _: Option<&str>,
        _: i64,
        _: i64,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("concurrency test does not search pages")
    }

    async fn update_page_metadata(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &PageMetadataPatch,
        _: Option<i64>,
    ) -> Result<Page, NotesProductError> {
        unreachable!("concurrency test does not update metadata")
    }

    async fn update_page_drive_snapshot(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &DrivePageContentSnapshot,
        _: &str,
    ) -> Result<Page, NotesProductError> {
        unreachable!("concurrency test does not update Drive snapshots")
    }

    async fn delete_page(&self, _: &NotesActorContext, _: &str) -> Result<(), NotesProductError> {
        unreachable!("concurrency test does not delete pages")
    }

    async fn find_ai_job_by_idempotency_key(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Option<AiJob>, NotesProductError> {
        self.find_attempts.fetch_add(1, Ordering::SeqCst);
        Ok(self.inserted_job.lock().await.clone())
    }

    async fn insert_ai_job(&self, job: NewAiJob) -> Result<AiJob, NotesProductError> {
        self.insert_attempts.fetch_add(1, Ordering::SeqCst);
        let inserted = AiJob {
            id: job.id,
            tenant_id: job.context.tenant_id,
            organization_id: job.context.organization_id,
            workspace_id: job.workspace_id,
            job_type: job.job_type,
            target_type: job.target_type,
            target_id: job.target_id,
            status: job.status,
            result: None,
            source_count: job.sources.len() as i64,
            suggestion_count: 0,
            idempotency_key: job.idempotency_key,
            request_payload_hash: job.request_payload_hash,
            created_by: job.context.operator_id,
            created_at: "2026-06-08T00:00:00Z".to_string(),
        };
        self.inserted_job.lock().await.replace(inserted);
        Err(NotesProductError::Conflict(
            "insert notes_ai_job failed".to_string(),
        ))
    }

    async fn list_ai_jobs(
        &self,
        _: &NotesActorContext,
        _: Option<&str>,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiJob>, NotesProductError> {
        unreachable!("concurrency test does not list AI jobs")
    }

    async fn find_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not find AI jobs")
    }

    async fn cancel_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not cancel AI jobs")
    }

    async fn claim_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not claim AI jobs")
    }

    async fn list_ai_job_sources(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Vec<AiJobSource>, NotesProductError> {
        unreachable!("concurrency test does not list AI job sources")
    }

    async fn fail_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("test fake does not fail AI jobs")
    }

    async fn complete_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: Vec<NewAiSuggestion>,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not complete AI jobs")
    }

    async fn list_page_ai_suggestions(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiSuggestion>, NotesProductError> {
        unreachable!("concurrency test does not list AI suggestions")
    }

    async fn find_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("concurrency test does not find AI suggestions")
    }

    async fn decide_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("concurrency test does not decide AI suggestions")
    }

    async fn apply_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
        _: &DrivePageContentSnapshot,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("concurrency test does not apply AI suggestions")
    }

    async fn find_ai_feedback(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiFeedback, NotesProductError> {
        unreachable!("concurrency test does not find AI feedback")
    }

    async fn insert_ai_feedback(&self, _: NewAiFeedback) -> Result<AiFeedback, NotesProductError> {
        unreachable!("concurrency test does not insert AI feedback")
    }

    async fn list_ai_suggestion_feedback(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiFeedback>, NotesProductError> {
        unreachable!("concurrency test does not list AI feedback")
    }
}

#[derive(Clone, Default)]
struct ConcurrentAiFeedbackIdempotencyRepository {
    find_feedback_attempts: Arc<AtomicUsize>,
    insert_feedback_attempts: Arc<AtomicUsize>,
    inserted_feedback: Arc<Mutex<Option<AiFeedback>>>,
}

impl ConcurrentAiFeedbackIdempotencyRepository {
    fn find_feedback_attempts(&self) -> usize {
        self.find_feedback_attempts.load(Ordering::SeqCst)
    }

    fn insert_feedback_attempts(&self) -> usize {
        self.insert_feedback_attempts.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl NotesRepository for ConcurrentAiFeedbackIdempotencyRepository {
    async fn insert_workspace(&self, _: NewWorkspace) -> Result<Workspace, NotesProductError> {
        unreachable!("concurrency test does not insert workspaces")
    }

    async fn find_workspace(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Workspace, NotesProductError> {
        unreachable!("concurrency test does not read workspaces")
    }

    async fn list_workspaces(
        &self,
        _: &NotesActorContext,
        _: i64,
        _: i64,
    ) -> Result<Vec<Workspace>, NotesProductError> {
        unreachable!("concurrency test does not list workspaces")
    }

    async fn insert_page(&self, _: NewPage) -> Result<Page, NotesProductError> {
        unreachable!("concurrency test does not insert pages")
    }

    async fn page_id_is_reserved(&self, _: &str) -> Result<bool, NotesProductError> {
        Ok(false)
    }

    async fn find_page(&self, _: &NotesActorContext, _: &str) -> Result<Page, NotesProductError> {
        unreachable!("concurrency test does not read pages")
    }

    async fn list_pages(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
        _: Option<&str>,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("concurrency test does not list pages")
    }

    async fn list_root_pages(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("concurrency test does not list root pages")
    }

    async fn search_pages(
        &self,
        _: &NotesActorContext,
        _: Option<&str>,
        _: Option<&str>,
        _: i64,
        _: i64,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("concurrency test does not search pages")
    }

    async fn update_page_metadata(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &PageMetadataPatch,
        _: Option<i64>,
    ) -> Result<Page, NotesProductError> {
        unreachable!("concurrency test does not update metadata")
    }

    async fn update_page_drive_snapshot(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &DrivePageContentSnapshot,
        _: &str,
    ) -> Result<Page, NotesProductError> {
        unreachable!("concurrency test does not update Drive snapshots")
    }

    async fn delete_page(&self, _: &NotesActorContext, _: &str) -> Result<(), NotesProductError> {
        unreachable!("concurrency test does not delete pages")
    }

    async fn find_ai_job_by_idempotency_key(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Option<AiJob>, NotesProductError> {
        unreachable!("concurrency test does not use AI job idempotency")
    }

    async fn insert_ai_job(&self, _: NewAiJob) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not insert AI jobs")
    }

    async fn list_ai_jobs(
        &self,
        _: &NotesActorContext,
        _: Option<&str>,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiJob>, NotesProductError> {
        unreachable!("concurrency test does not list AI jobs")
    }

    async fn find_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not find AI jobs")
    }

    async fn cancel_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not cancel AI jobs")
    }

    async fn claim_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not claim AI jobs")
    }

    async fn list_ai_job_sources(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Vec<AiJobSource>, NotesProductError> {
        unreachable!("concurrency test does not list AI job sources")
    }

    async fn fail_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("test fake does not fail AI jobs")
    }

    async fn complete_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: Vec<NewAiSuggestion>,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not complete AI jobs")
    }

    async fn list_page_ai_suggestions(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiSuggestion>, NotesProductError> {
        unreachable!("concurrency test does not list AI suggestions")
    }

    async fn find_ai_suggestion(
        &self,
        context: &NotesActorContext,
        ai_suggestion_id: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        Ok(AiSuggestion {
            id: ai_suggestion_id.to_string(),
            tenant_id: context.tenant_id.clone(),
            organization_id: context.organization_id.clone(),
            workspace_id: "workspace-001".to_string(),
            page_id: "page-001".to_string(),
            ai_job_id: "ai-job-001".to_string(),
            suggestion_type: "summary".to_string(),
            status: "accepted".to_string(),
            source_drive_node_id: Some("drive-node-page-001".to_string()),
            source_drive_version_id: Some("drive-version-page-001-v1".to_string()),
            source_drive_version_no: Some(1),
            payload: json!({ "summary": "Roadmap is ready." }),
            created_by: context.operator_id.clone(),
            created_at: "2026-06-08T00:00:00Z".to_string(),
        })
    }

    async fn decide_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("concurrency test does not decide AI suggestions")
    }

    async fn apply_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
        _: &DrivePageContentSnapshot,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("concurrency test does not apply AI suggestions")
    }

    async fn find_ai_feedback(
        &self,
        _: &NotesActorContext,
        ai_feedback_id: &str,
    ) -> Result<AiFeedback, NotesProductError> {
        self.find_feedback_attempts.fetch_add(1, Ordering::SeqCst);
        self.inserted_feedback
            .lock()
            .await
            .clone()
            .filter(|feedback| feedback.id == ai_feedback_id)
            .ok_or_else(|| NotesProductError::NotFound("AI feedback not found".to_string()))
    }

    async fn insert_ai_feedback(
        &self,
        feedback: NewAiFeedback,
    ) -> Result<AiFeedback, NotesProductError> {
        self.insert_feedback_attempts.fetch_add(1, Ordering::SeqCst);
        self.inserted_feedback.lock().await.replace(AiFeedback {
            id: feedback.id,
            tenant_id: feedback.context.tenant_id,
            organization_id: feedback.context.organization_id,
            workspace_id: feedback.workspace_id,
            job_id: feedback.job_id,
            suggestion_id: feedback.suggestion_id,
            feedback_type: feedback.feedback_type,
            feedback_text: feedback.feedback_text,
            created_by: feedback.context.operator_id,
            created_at: "2026-06-08T00:00:00Z".to_string(),
        });
        Err(NotesProductError::Conflict(
            "insert notes_ai_feedback failed".to_string(),
        ))
    }

    async fn list_ai_suggestion_feedback(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiFeedback>, NotesProductError> {
        unreachable!("concurrency test does not list AI feedback")
    }
}

#[derive(Clone, Default)]
struct ConcurrentDriveSnapshotRepository;

#[async_trait]
impl NotesRepository for ConcurrentDriveSnapshotRepository {
    async fn insert_workspace(&self, _: NewWorkspace) -> Result<Workspace, NotesProductError> {
        unreachable!("concurrency test does not insert workspaces")
    }

    async fn find_workspace(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Workspace, NotesProductError> {
        unreachable!("concurrency test does not read workspaces")
    }

    async fn list_workspaces(
        &self,
        _: &NotesActorContext,
        _: i64,
        _: i64,
    ) -> Result<Vec<Workspace>, NotesProductError> {
        unreachable!("concurrency test does not list workspaces")
    }

    async fn insert_page(&self, _: NewPage) -> Result<Page, NotesProductError> {
        unreachable!("concurrency test does not insert pages")
    }

    async fn page_id_is_reserved(&self, _: &str) -> Result<bool, NotesProductError> {
        Ok(false)
    }

    async fn find_page(
        &self,
        context: &NotesActorContext,
        page_id: &str,
    ) -> Result<Page, NotesProductError> {
        Ok(test_page(context, page_id, 1))
    }

    async fn list_pages(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
        _: Option<&str>,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("concurrency test does not list pages")
    }

    async fn list_root_pages(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("concurrency test does not list root pages")
    }

    async fn search_pages(
        &self,
        _: &NotesActorContext,
        _: Option<&str>,
        _: Option<&str>,
        _: i64,
        _: i64,
    ) -> Result<Vec<Page>, NotesProductError> {
        unreachable!("concurrency test does not search pages")
    }

    async fn update_page_metadata(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &PageMetadataPatch,
        _: Option<i64>,
    ) -> Result<Page, NotesProductError> {
        unreachable!("concurrency test does not update metadata")
    }

    async fn update_page_drive_snapshot(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &DrivePageContentSnapshot,
        expected_current_drive_version_id: &str,
    ) -> Result<Page, NotesProductError> {
        if expected_current_drive_version_id != "drive-version-page-001-v2" {
            return Err(NotesProductError::Conflict(
                "page Drive version has changed".to_string(),
            ));
        }
        unreachable!("stale drive version should conflict before returning")
    }

    async fn delete_page(&self, _: &NotesActorContext, _: &str) -> Result<(), NotesProductError> {
        unreachable!("concurrency test does not delete pages")
    }

    async fn find_ai_job_by_idempotency_key(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Option<AiJob>, NotesProductError> {
        unreachable!("concurrency test does not use AI jobs")
    }

    async fn insert_ai_job(&self, _: NewAiJob) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not insert AI jobs")
    }

    async fn list_ai_jobs(
        &self,
        _: &NotesActorContext,
        _: Option<&str>,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiJob>, NotesProductError> {
        unreachable!("concurrency test does not list AI jobs")
    }

    async fn find_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not find AI jobs")
    }

    async fn cancel_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not cancel AI jobs")
    }

    async fn claim_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not claim AI jobs")
    }

    async fn list_ai_job_sources(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<Vec<AiJobSource>, NotesProductError> {
        unreachable!("concurrency test does not list AI job sources")
    }

    async fn fail_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("test fake does not fail AI jobs")
    }

    async fn complete_ai_job(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: Vec<NewAiSuggestion>,
    ) -> Result<AiJob, NotesProductError> {
        unreachable!("concurrency test does not complete AI jobs")
    }

    async fn list_page_ai_suggestions(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiSuggestion>, NotesProductError> {
        unreachable!("concurrency test does not list AI suggestions")
    }

    async fn find_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("concurrency test does not find AI suggestions")
    }

    async fn decide_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("concurrency test does not decide AI suggestions")
    }

    async fn apply_ai_suggestion(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: &str,
        _: &DrivePageContentSnapshot,
        _: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        unreachable!("concurrency test does not apply AI suggestions")
    }

    async fn find_ai_feedback(
        &self,
        _: &NotesActorContext,
        _: &str,
    ) -> Result<AiFeedback, NotesProductError> {
        unreachable!("concurrency test does not find AI feedback")
    }

    async fn insert_ai_feedback(&self, _: NewAiFeedback) -> Result<AiFeedback, NotesProductError> {
        unreachable!("concurrency test does not insert AI feedback")
    }

    async fn list_ai_suggestion_feedback(
        &self,
        _: &NotesActorContext,
        _: &str,
        _: i64,
        _: i64,
    ) -> Result<Vec<AiFeedback>, NotesProductError> {
        unreachable!("concurrency test does not list AI feedback")
    }
}

#[derive(Clone, Default)]
struct FakeDrivePageContentPort {
    records: Arc<Mutex<BTreeMap<String, DrivePageContentSnapshot>>>,
    versions: Arc<Mutex<BTreeMap<String, Vec<DrivePageContentSnapshot>>>>,
    create_counts: Arc<Mutex<BTreeMap<String, usize>>>,
    update_counts: Arc<Mutex<BTreeMap<String, usize>>>,
    last_update_request: Arc<Mutex<Option<UpdateDrivePageContentCommand>>>,
    last_version_list_request: Arc<Mutex<Option<ListDrivePageContentVersionsCommand>>>,
    last_restore_request: Arc<Mutex<Option<RestoreDrivePageContentVersionCommand>>>,
    invalid_next_create_drive_version_id: Arc<Mutex<bool>>,
    invalid_next_update_drive_version_id: Arc<Mutex<bool>>,
    next_create_wrong_content_metadata: Arc<Mutex<bool>>,
    next_update_wrong_content_metadata: Arc<Mutex<bool>>,
    next_update_wrong_drive_node: Arc<Mutex<bool>>,
    next_update_non_object_content: Arc<Mutex<bool>>,
    next_update_reuse_current_drive_version: Arc<Mutex<bool>>,
    next_restore_reuse_current_drive_version: Arc<Mutex<bool>>,
    next_restore_oversized_content_metadata: Arc<Mutex<bool>>,
    next_read_wrong_drive_version: Arc<Mutex<bool>>,
    next_version_list_invalid_summary: Arc<Mutex<bool>>,
    next_version_list_unbounded: Arc<Mutex<bool>>,
    next_version_list_page_info_mismatch: Arc<Mutex<bool>>,
}

impl FakeDrivePageContentPort {
    async fn invalidate_next_create_drive_version_id(&self) {
        *self.invalid_next_create_drive_version_id.lock().await = true;
    }

    async fn invalidate_next_update_drive_version_id(&self) {
        *self.invalid_next_update_drive_version_id.lock().await = true;
    }

    async fn make_next_create_snapshot_use_wrong_content_metadata(&self) {
        *self.next_create_wrong_content_metadata.lock().await = true;
    }

    async fn make_next_update_snapshot_use_wrong_content_metadata(&self) {
        *self.next_update_wrong_content_metadata.lock().await = true;
    }

    async fn make_next_update_snapshot_use_wrong_drive_node(&self) {
        *self.next_update_wrong_drive_node.lock().await = true;
    }

    async fn make_next_update_snapshot_use_non_object_content(&self) {
        *self.next_update_non_object_content.lock().await = true;
    }

    async fn make_next_update_snapshot_reuse_current_drive_version(&self) {
        *self.next_update_reuse_current_drive_version.lock().await = true;
    }

    async fn make_next_restore_snapshot_reuse_current_drive_version(&self) {
        *self.next_restore_reuse_current_drive_version.lock().await = true;
    }

    async fn make_next_restore_snapshot_use_oversized_content_metadata(&self) {
        *self.next_restore_oversized_content_metadata.lock().await = true;
    }

    async fn make_next_read_snapshot_use_wrong_drive_version(&self) {
        *self.next_read_wrong_drive_version.lock().await = true;
    }

    async fn invalidate_next_version_list_summary(&self) {
        *self.next_version_list_invalid_summary.lock().await = true;
    }

    async fn make_next_version_list_unbounded(&self) {
        *self.next_version_list_unbounded.lock().await = true;
    }

    async fn make_next_version_list_page_info_mismatch(&self) {
        *self.next_version_list_page_info_mismatch.lock().await = true;
    }

    async fn last_update_request(&self) -> Option<UpdateDrivePageContentCommand> {
        self.last_update_request.lock().await.clone()
    }

    async fn last_version_list_request(&self) -> Option<ListDrivePageContentVersionsCommand> {
        self.last_version_list_request.lock().await.clone()
    }

    async fn last_restore_request(&self) -> Option<RestoreDrivePageContentVersionCommand> {
        self.last_restore_request.lock().await.clone()
    }

    async fn has_page(&self, page_id: &str) -> bool {
        self.records.lock().await.contains_key(page_id)
    }

    async fn update_count(&self, page_id: &str) -> usize {
        self.update_counts
            .lock()
            .await
            .get(page_id)
            .copied()
            .unwrap_or(0)
    }

    async fn create_count(&self, page_id: &str) -> usize {
        self.create_counts
            .lock()
            .await
            .get(page_id)
            .copied()
            .unwrap_or(0)
    }
}

#[async_trait]
impl DrivePageContentPort for FakeDrivePageContentPort {
    async fn create_page_content(
        &self,
        command: CreateDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, sdkwork_notes_pages_service::error::NotesProductError>
    {
        {
            let mut create_counts = self.create_counts.lock().await;
            let count = create_counts.entry(command.page_id.clone()).or_insert(0);
            *count += 1;
        }

        let mut snapshot = DrivePageContentSnapshot {
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
        if take_bool(&self.invalid_next_create_drive_version_id).await {
            snapshot.drive_version_id = " ".to_string();
        }
        if take_bool(&self.next_create_wrong_content_metadata).await {
            snapshot.content_type = "application/vnd.sdkwork.notes.unexpected+json".to_string();
            snapshot.content_schema_version = "unexpected".to_string();
        }
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
        *self.last_update_request.lock().await = Some(command.clone());
        {
            let mut update_counts = self.update_counts.lock().await;
            let count = update_counts.entry(command.page_id.clone()).or_insert(0);
            *count += 1;
        }

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
        let mut snapshot = snapshot;
        if take_bool(&self.invalid_next_update_drive_version_id).await {
            snapshot.drive_version_id = " ".to_string();
        }
        if take_bool(&self.next_update_wrong_content_metadata).await {
            snapshot.content_type = "application/vnd.sdkwork.notes.unexpected+json".to_string();
            snapshot.content_schema_version = "unexpected".to_string();
        }
        if take_bool(&self.next_update_wrong_drive_node).await {
            snapshot.drive_node_id = format!("drive-node-wrong-{}", command.page_id);
            snapshot.drive_uri = format!(
                "drive://spaces/{}/nodes/{}",
                snapshot.drive_space_id, snapshot.drive_node_id
            );
        }
        if take_bool(&self.next_update_non_object_content).await {
            snapshot.content = json!("not-a-page-object");
        }
        if take_bool(&self.next_update_reuse_current_drive_version).await {
            snapshot.drive_version_id = command.current_drive_version_id.clone();
            snapshot.drive_version_no = 1;
        }
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

    async fn restore_page_content_version(
        &self,
        command: RestoreDrivePageContentVersionCommand,
    ) -> Result<DrivePageContentSnapshot, sdkwork_notes_pages_service::error::NotesProductError>
    {
        *self.last_restore_request.lock().await = Some(command.clone());
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
        if take_bool(&self.next_restore_reuse_current_drive_version).await {
            snapshot.drive_version_id = command.current_drive_version_id.clone();
            snapshot.drive_version_no = next_version_no - 1;
        }
        if take_bool(&self.next_restore_oversized_content_metadata).await {
            snapshot.content_type = "x".repeat(256);
            snapshot.content_schema_version = "v".repeat(33);
        }
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
        let mut snapshot = self
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
        if take_bool(&self.next_read_wrong_drive_version).await {
            snapshot.drive_version_id = format!("drive-version-{}-wrong", command.page_id);
            snapshot.drive_version_no += 1;
        }
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

        self.last_version_list_request
            .lock()
            .await
            .replace(command.clone());

        let mut items = vec![DriveVersionSummary {
            drive_version_id: current.drive_version_id,
            drive_version_no: current.drive_version_no,
            version_kind: "auto".to_string(),
            version_label: Some("Autosave".to_string()),
            change_summary: Some("Autosave".to_string()),
            created_at: "2026-06-08T00:00:02Z".to_string(),
        }];
        if take_bool(&self.next_version_list_invalid_summary).await {
            items[0].drive_version_id = " ".to_string();
            items[0].drive_version_no = 0;
        }

        if command.page == 1 && command.page_size > 1 {
            items.push(DriveVersionSummary {
                drive_version_id: format!("drive-version-{}-v1", command.page_id),
                drive_version_no: 1,
                version_kind: "initial".to_string(),
                version_label: Some("Initial".to_string()),
                change_summary: Some("Initial page".to_string()),
                created_at: "2026-06-08T00:00:01Z".to_string(),
            });
        }
        if take_bool(&self.next_version_list_unbounded).await {
            items.push(DriveVersionSummary {
                drive_version_id: format!("drive-version-{}-overflow", command.page_id),
                drive_version_no: current.drive_version_no + 1,
                version_kind: "overflow".to_string(),
                version_label: Some("Overflow".to_string()),
                change_summary: Some("Unbounded page".to_string()),
                created_at: "2026-06-08T00:00:03Z".to_string(),
            });
        }

        let mut page_info = PageInfo {
            page: command.page,
            page_size: command.page_size,
            has_more: false,
            next_cursor: None,
        };
        if take_bool(&self.next_version_list_page_info_mismatch).await {
            page_info.page = command.page + 1;
            page_info.page_size = command.page_size + 1;
        }

        Ok(DriveVersionPage { items, page_info })
    }
}

#[derive(Clone, Default)]
struct RejectSuggestionDuringDriveUpdatePort {
    inner: FakeDrivePageContentPort,
    reject_during_next_update: Arc<Mutex<Option<(sqlx::AnyPool, String)>>>,
}

impl RejectSuggestionDuringDriveUpdatePort {
    async fn reject_suggestion_during_next_update(
        &self,
        pool: sqlx::AnyPool,
        suggestion_id: String,
    ) {
        self.reject_during_next_update
            .lock()
            .await
            .replace((pool, suggestion_id));
    }
}

#[async_trait]
impl DrivePageContentPort for RejectSuggestionDuringDriveUpdatePort {
    async fn create_page_content(
        &self,
        command: CreateDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, sdkwork_notes_pages_service::error::NotesProductError>
    {
        self.inner.create_page_content(command).await
    }

    async fn update_page_content(
        &self,
        command: UpdateDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, sdkwork_notes_pages_service::error::NotesProductError>
    {
        let snapshot = self.inner.update_page_content(command).await?;
        let reject_target = self.reject_during_next_update.lock().await.take();
        if let Some((pool, suggestion_id)) = reject_target {
            sqlx::query(
                "UPDATE notes_ai_suggestion
                 SET status='rejected', updated_at=CURRENT_TIMESTAMP, version=version + 1
                 WHERE id=$1",
            )
            .bind(suggestion_id)
            .execute(&pool)
            .await
            .expect("test should be able to reject suggestion during Drive update");
        }
        Ok(snapshot)
    }

    async fn read_page_content(
        &self,
        command: ReadDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, sdkwork_notes_pages_service::error::NotesProductError>
    {
        self.inner.read_page_content(command).await
    }

    async fn restore_page_content_version(
        &self,
        command: RestoreDrivePageContentVersionCommand,
    ) -> Result<DrivePageContentSnapshot, sdkwork_notes_pages_service::error::NotesProductError>
    {
        self.inner.restore_page_content_version(command).await
    }

    async fn list_page_content_versions(
        &self,
        command: ListDrivePageContentVersionsCommand,
    ) -> Result<DriveVersionPage, sdkwork_notes_pages_service::error::NotesProductError> {
        self.inner.list_page_content_versions(command).await
    }
}

async fn take_bool(value: &Mutex<bool>) -> bool {
    let mut value = value.lock().await;
    let current = *value;
    *value = false;
    current
}
