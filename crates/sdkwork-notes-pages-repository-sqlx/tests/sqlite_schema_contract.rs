use sdkwork_notes_pages_repository_sqlx::install_sqlite_schema;
use sdkwork_notes_pages_repository_sqlx::notes_store::SqlNotesStore;
use sdkwork_notes_pages_service::domain::{
    DrivePageContentSnapshot, NewPage, NewWorkspace, NotesActorContext, PageKind, PageMetadataPatch,
};
use sdkwork_notes_pages_service::error::NotesProductError;
use sdkwork_notes_pages_service::ports::NotesRepository;
use serde_json::json;
use sqlx::any::AnyPoolOptions;
use sqlx::Row;

#[tokio::test]
async fn installs_phase1_workspace_and_page_schema() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");

    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let workspace_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name='notes_workspace'",
    )
    .fetch_one(&pool)
    .await
    .expect("notes_workspace table count should be readable");
    assert_eq!(workspace_count, 1);

    let page_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name='notes_page'",
    )
    .fetch_one(&pool)
    .await
    .expect("notes_page table count should be readable");
    assert_eq!(page_count, 1);

    for required_index in [
        "ix_notes_workspace_recent",
        "ux_notes_workspace_drive_space",
        "ix_notes_page_workspace_recent",
    ] {
        assert!(
            index_exists(&pool, required_index).await,
            "{required_index} should exist for tenant-scoped list/query performance"
        );
    }

    let page_columns = table_columns(&pool, "notes_page").await;
    for required_column in [
        "drive_space_id",
        "drive_node_id",
        "drive_uri",
        "current_drive_version_id",
        "current_drive_version_no",
    ] {
        assert!(
            page_columns.contains(&required_column.to_string()),
            "notes_page should include {required_column}"
        );
    }

    for required_column in ["current_drive_version_id", "current_drive_version_no"] {
        assert!(
            column_is_not_null(&pool, "notes_page", required_column).await,
            "notes_page.{required_column} should be NOT NULL because Notes pages are always Drive-version-backed"
        );
    }

    let forbidden_columns = [
        ["storage", "object"].join("_"),
        ["upload", "session"].join("_"),
        "buck".to_string() + "et",
        ["object", "key"].join("_"),
        ["revision", "id"].join("_"),
    ];
    for forbidden_column in forbidden_columns {
        assert!(
            !page_columns
                .iter()
                .any(|column| column.contains(&forbidden_column)),
            "notes_page must not own Drive storage/version lifecycle column {forbidden_column}"
        );
    }

    let ai_job_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name='notes_ai_job'",
    )
    .fetch_one(&pool)
    .await
    .expect("notes_ai_job table count should be readable");
    assert_eq!(ai_job_count, 1);

    let search_projection_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name='notes_page_search_projection'",
    )
    .fetch_one(&pool)
    .await
    .expect("notes_page_search_projection table count should be readable");
    assert_eq!(search_projection_count, 1);
    let search_projection_columns = table_columns(&pool, "notes_page_search_projection").await;
    for required_column in [
        "page_id",
        "drive_node_id",
        "source_drive_version_id",
        "source_drive_version_no",
        "title_snapshot",
        "plain_text",
        "index_status",
        "rebuild_version",
    ] {
        assert!(
            search_projection_columns.contains(&required_column.to_string()),
            "notes_page_search_projection should include {required_column}"
        );
    }
    assert!(
        index_exists(&pool, "ix_notes_page_search_projection_status").await,
        "notes_page_search_projection should have a tenant/workspace/status rebuild index"
    );
    assert!(
        index_exists(&pool, "ux_notes_page_search_projection_page_version").await,
        "notes_page_search_projection should avoid duplicate page/version projection rows"
    );

    let ai_job_source_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name='notes_ai_job_source'",
    )
    .fetch_one(&pool)
    .await
    .expect("notes_ai_job_source table count should be readable");
    assert_eq!(ai_job_source_count, 1);

    let ai_job_columns = table_columns(&pool, "notes_ai_job").await;
    for required_column in [
        "workspace_id",
        "job_type",
        "target_type",
        "target_id",
        "status",
        "idempotency_key",
        "request_payload_hash",
    ] {
        assert!(
            ai_job_columns.contains(&required_column.to_string()),
            "notes_ai_job should include {required_column}"
        );
    }

    let ai_job_source_columns = table_columns(&pool, "notes_ai_job_source").await;
    for required_column in [
        "job_id",
        "source_type",
        "source_id",
        "drive_node_id",
        "drive_version_id",
        "drive_version_no",
        "permission_snapshot_hash",
    ] {
        assert!(
            ai_job_source_columns.contains(&required_column.to_string()),
            "notes_ai_job_source should include {required_column}"
        );
    }

    let ai_suggestion_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name='notes_ai_suggestion'",
    )
    .fetch_one(&pool)
    .await
    .expect("notes_ai_suggestion table count should be readable");
    assert_eq!(ai_suggestion_count, 1);

    let ai_suggestion_columns = table_columns(&pool, "notes_ai_suggestion").await;
    for required_column in [
        "workspace_id",
        "page_id",
        "ai_job_id",
        "suggestion_type",
        "status",
        "source_drive_node_id",
        "source_drive_version_id",
        "source_drive_version_no",
        "payload_json",
    ] {
        assert!(
            ai_suggestion_columns.contains(&required_column.to_string()),
            "notes_ai_suggestion should include {required_column}"
        );
    }

    let create_sql: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='notes_ai_suggestion'",
    )
    .fetch_one(&pool)
    .await
    .expect("notes_ai_suggestion create sql should be readable");
    assert!(
        create_sql.contains("'applied'"),
        "notes_ai_suggestion status constraint should include applied"
    );

    let ai_feedback_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name='notes_ai_feedback'",
    )
    .fetch_one(&pool)
    .await
    .expect("notes_ai_feedback table count should be readable");
    assert_eq!(ai_feedback_count, 1);

    let ai_feedback_columns = table_columns(&pool, "notes_ai_feedback").await;
    for required_column in [
        "workspace_id",
        "job_id",
        "suggestion_id",
        "feedback_type",
        "feedback_text",
        "created_by",
        "created_at",
    ] {
        assert!(
            ai_feedback_columns.contains(&required_column.to_string()),
            "notes_ai_feedback should include {required_column}"
        );
    }

    for forbidden_column in [
        ["storage", "object"].join("_"),
        ["upload", "session"].join("_"),
        "buck".to_string() + "et",
        ["object", "key"].join("_"),
    ] {
        assert!(
            !ai_job_columns
                .iter()
                .chain(ai_job_source_columns.iter())
                .chain(ai_suggestion_columns.iter())
                .chain(ai_feedback_columns.iter())
                .any(|column| column.contains(&forbidden_column)),
            "AI job tables must not own Drive storage lifecycle column {forbidden_column}"
        );
    }
}

#[test]
fn sql_repository_uses_positive_lifecycle_predicates_for_indexed_queries() {
    let source = include_str!("../src/notes_store.rs");

    assert!(
        !source.contains("lifecycle_status != 'deleted'"),
        "repository queries should use positive lifecycle predicates so tenant/workspace/status indexes can be used predictably"
    );
    assert!(
        !source.contains("$3 IS NULL OR p.workspace_id=$3"),
        "search queries should branch optional workspace filters instead of using nullable OR predicates that weaken index access paths"
    );
    assert!(
        !source.contains("$3 IS NULL OR j.workspace_id=$3"),
        "AI job list queries should branch optional workspace filters instead of using nullable OR predicates that weaken index access paths"
    );
}

#[tokio::test]
async fn sql_repository_rejects_stale_metadata_and_drive_pointer_writes_atomically() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let repository = SqlNotesStore::new(pool);
    let context = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };
    repository
        .insert_workspace(NewWorkspace {
            id: "workspace-001".to_string(),
            context: context.clone(),
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
        .expect("workspace should be inserted");
    repository
        .insert_page(NewPage {
            id: "page-001".to_string(),
            context: context.clone(),
            workspace_id: "workspace-001".to_string(),
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            drive_snapshot: drive_snapshot("page-001", 1),
        })
        .await
        .expect("page should be inserted");

    let stale_metadata = repository
        .update_page_metadata(
            &context,
            "page-001",
            &PageMetadataPatch {
                title: "Roadmap stale".to_string(),
                favorite: true,
                archive_status: "active".to_string(),
                publish_status: "private".to_string(),
                parent_page_id: None,
            },
            Some(2),
        )
        .await;
    assert!(matches!(
        stale_metadata,
        Err(NotesProductError::Conflict(_))
    ));
    let unchanged = repository
        .find_page(&context, "page-001")
        .await
        .expect("page should still be readable");
    assert_eq!(unchanged.title, "Roadmap");
    assert_eq!(unchanged.version, 1);

    let stale_drive_pointer = repository
        .update_page_drive_snapshot(
            &context,
            "page-001",
            &drive_snapshot("page-001", 2),
            "drive-version-page-001-stale",
        )
        .await;
    assert!(matches!(
        stale_drive_pointer,
        Err(NotesProductError::Conflict(_))
    ));
    let unchanged = repository
        .find_page(&context, "page-001")
        .await
        .expect("page should still be readable after stale Drive pointer update");
    assert_eq!(
        unchanged.current_drive_version_id,
        "drive-version-page-001-v1"
    );
    assert_eq!(unchanged.current_drive_version_no, 1);
    assert_eq!(unchanged.version, 1);

    let updated = repository
        .update_page_drive_snapshot(
            &context,
            "page-001",
            &drive_snapshot("page-001", 2),
            "drive-version-page-001-v1",
        )
        .await
        .expect("matching Drive pointer should advance page refs");
    assert_eq!(
        updated.current_drive_version_id,
        "drive-version-page-001-v2"
    );
    assert_eq!(updated.current_drive_version_no, 2);
    assert_eq!(updated.version, 2);
}

#[tokio::test]
async fn sql_repository_search_uses_current_indexed_projection_and_ignores_stale_versions() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let repository = SqlNotesStore::new(pool.clone());
    let context = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };
    repository
        .insert_workspace(NewWorkspace {
            id: "workspace-001".to_string(),
            context: context.clone(),
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
        .expect("workspace should be inserted");
    repository
        .insert_page(NewPage {
            id: "page-001".to_string(),
            context: context.clone(),
            workspace_id: "workspace-001".to_string(),
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            drive_snapshot: drive_snapshot("page-001", 1),
        })
        .await
        .expect("page should be inserted");

    insert_search_projection(
        &pool,
        "projection-current",
        "page-001",
        "drive-version-page-001-v1",
        1,
        "Apollo launch plan includes indexed-only context",
    )
    .await;
    insert_search_projection(
        &pool,
        "projection-stale",
        "page-001",
        "drive-version-page-001-stale",
        1,
        "Legacy stale content should not be searchable",
    )
    .await;

    let indexed_matches = repository
        .search_pages(&context, Some("workspace-001"), Some("indexed-only"), 1, 20)
        .await
        .expect("search should read current indexed projection");
    assert_eq!(indexed_matches.len(), 1);
    assert_eq!(indexed_matches[0].id, "page-001");
    assert_eq!(
        indexed_matches[0].snippet.as_deref(),
        Some("Apollo launch plan includes indexed-only context")
    );

    let stale_matches = repository
        .search_pages(&context, Some("workspace-001"), Some("Legacy stale"), 1, 20)
        .await
        .expect("search should ignore stale projection versions");
    assert!(
        stale_matches.is_empty(),
        "stale projection rows must not satisfy search for the current page"
    );
}

#[tokio::test]
async fn sql_repository_search_treats_like_wildcards_as_literal_query_text() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let repository = SqlNotesStore::new(pool);
    let context = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };
    repository
        .insert_workspace(NewWorkspace {
            id: "workspace-001".to_string(),
            context: context.clone(),
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
        .expect("workspace should be inserted");
    repository
        .insert_page(NewPage {
            id: "page-roadmap".to_string(),
            context: context.clone(),
            workspace_id: "workspace-001".to_string(),
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            drive_snapshot: drive_snapshot("page-roadmap", 1),
        })
        .await
        .expect("roadmap page should be inserted");
    repository
        .insert_page(NewPage {
            id: "page-literal-percent".to_string(),
            context: context.clone(),
            workspace_id: "workspace-001".to_string(),
            title: "50% rollout".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            drive_snapshot: drive_snapshot("page-literal-percent", 1),
        })
        .await
        .expect("literal percent page should be inserted");
    repository
        .insert_page(NewPage {
            id: "page-literal-underscore".to_string(),
            context: context.clone(),
            workspace_id: "workspace-001".to_string(),
            title: "build_tag".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            drive_snapshot: drive_snapshot("page-literal-underscore", 1),
        })
        .await
        .expect("literal underscore page should be inserted");

    let percent_page_matches = repository
        .list_pages(&context, "workspace-001", 1, 20, Some("%"))
        .await
        .expect("literal percent list query should run");
    assert_eq!(percent_page_matches.len(), 1);
    assert_eq!(percent_page_matches[0].id, "page-literal-percent");

    let underscore_search_matches = repository
        .search_pages(&context, Some("workspace-001"), Some("_"), 1, 20)
        .await
        .expect("literal underscore search query should run");
    assert_eq!(underscore_search_matches.len(), 1);
    assert_eq!(underscore_search_matches[0].id, "page-literal-underscore");
}

#[tokio::test]
async fn sql_repository_apply_ai_suggestion_updates_page_pointer_and_suggestion_status_atomically()
{
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let repository = SqlNotesStore::new(pool.clone());
    let context = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };
    repository
        .insert_workspace(NewWorkspace {
            id: "workspace-001".to_string(),
            context: context.clone(),
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
        .expect("workspace should be inserted");
    repository
        .insert_page(NewPage {
            id: "page-001".to_string(),
            context: context.clone(),
            workspace_id: "workspace-001".to_string(),
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            drive_snapshot: drive_snapshot("page-001", 1),
        })
        .await
        .expect("page should be inserted");
    insert_ai_suggestion(&pool, "suggestion-accepted", "accepted").await;

    let applied = repository
        .apply_ai_suggestion(
            &context,
            "suggestion-accepted",
            "page-001",
            &drive_snapshot("page-001", 2),
            "drive-version-page-001-v1",
        )
        .await
        .expect("accepted suggestion apply should update page and suggestion atomically");
    assert_eq!(applied.status, "applied");
    let page = repository
        .find_page(&context, "page-001")
        .await
        .expect("page should be readable after apply");
    assert_eq!(page.current_drive_version_id, "drive-version-page-001-v2");
    assert_eq!(page.current_drive_version_no, 2);

    insert_ai_suggestion(&pool, "suggestion-rejected", "rejected").await;
    let stale_apply = repository
        .apply_ai_suggestion(
            &context,
            "suggestion-rejected",
            "page-001",
            &drive_snapshot("page-001", 3),
            "drive-version-page-001-v2",
        )
        .await;
    assert!(matches!(stale_apply, Err(NotesProductError::Conflict(_))));
    let page = repository
        .find_page(&context, "page-001")
        .await
        .expect("page should still be readable after rejected suggestion apply");
    assert_eq!(
        page.current_drive_version_id, "drive-version-page-001-v2",
        "transaction must roll back page pointer when suggestion cannot move to applied"
    );
    assert_eq!(page.current_drive_version_no, 2);
}

#[tokio::test]
async fn sql_repository_cancel_ai_job_rejects_concurrent_terminal_state_change() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let repository = SqlNotesStore::new(pool.clone());
    let context = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };
    seed_workspace_page_and_ai_job(&repository, &context, "ai-job-concurrent-cancel").await;

    sqlx::query(
        r#"
        CREATE TRIGGER notes_ai_job_cancel_concurrent_terminal
        BEFORE UPDATE OF status ON notes_ai_job
        WHEN OLD.id='ai-job-concurrent-cancel'
          AND OLD.status='queued'
          AND NEW.status='canceled'
        BEGIN
          UPDATE notes_ai_job
             SET status='succeeded',
                 result_json='{"completedBy":"worker-race"}',
                 updated_at=CURRENT_TIMESTAMP,
                 version=version + 1
           WHERE id=OLD.id;
          SELECT RAISE(IGNORE);
        END
        "#,
    )
    .execute(&pool)
    .await
    .expect("concurrent cancel trigger should be installed");

    let cancel_result = repository
        .cancel_ai_job(&context, "ai-job-concurrent-cancel")
        .await;
    assert!(matches!(cancel_result, Err(NotesProductError::Conflict(_))));

    let job = repository
        .find_ai_job(&context, "ai-job-concurrent-cancel")
        .await
        .expect("AI job should remain readable after concurrent terminal change");
    assert_eq!(job.status, "succeeded");
}

#[tokio::test]
async fn sql_repository_complete_ai_job_rolls_back_suggestions_when_final_update_is_lost() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");
    install_sqlite_schema(&pool)
        .await
        .expect("notes sqlite schema should install");

    let repository = SqlNotesStore::new(pool.clone());
    let context = NotesActorContext {
        tenant_id: "100001".to_string(),
        organization_id: "0".to_string(),
        operator_id: "30".to_string(),
        caller: sdkwork_notes_pages_service::domain::CallerCredentials::default(),
    };
    seed_workspace_page_and_ai_job(&repository, &context, "ai-job-concurrent-complete").await;
    repository
        .claim_ai_job(&context, "ai-job-concurrent-complete")
        .await
        .expect("AI job should be running before completion");

    sqlx::query(
        r#"
        CREATE TRIGGER notes_ai_job_complete_concurrent_terminal
        BEFORE UPDATE OF status ON notes_ai_job
        WHEN OLD.id='ai-job-concurrent-complete'
          AND OLD.status='running'
          AND NEW.status='succeeded'
        BEGIN
          UPDATE notes_ai_job
             SET status='failed',
                 error_code='worker_race',
                 error_message='Another worker failed the job first.',
                 updated_at=CURRENT_TIMESTAMP,
                 version=version + 1
           WHERE id=OLD.id;
          SELECT RAISE(IGNORE);
        END
        "#,
    )
    .execute(&pool)
    .await
    .expect("concurrent completion trigger should be installed");

    let complete_result = repository
        .complete_ai_job(
            &context,
            "ai-job-concurrent-complete",
            vec![sdkwork_notes_pages_service::domain::NewAiSuggestion {
                id: "ai-suggestion-rollback".to_string(),
                context: context.clone(),
                workspace_id: "workspace-001".to_string(),
                page_id: "page-001".to_string(),
                ai_job_id: "ai-job-concurrent-complete".to_string(),
                suggestion_type: "summary".to_string(),
                payload: json!({ "summary": "should be rolled back" }),
                source_drive_node_id: Some("drive-node-page-001".to_string()),
                source_drive_version_id: Some("drive-version-page-001-v1".to_string()),
                source_drive_version_no: Some(1),
            }],
        )
        .await;
    assert!(matches!(
        complete_result,
        Err(NotesProductError::Conflict(_))
    ));

    let suggestion_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(1)
         FROM notes_ai_suggestion
         WHERE id='ai-suggestion-rollback'",
    )
    .fetch_one(&pool)
    .await
    .expect("suggestion count should be readable");
    assert_eq!(
        suggestion_count, 0,
        "completion transaction must roll back inserted suggestions when job state update loses the race"
    );

    let job = repository
        .find_ai_job(&context, "ai-job-concurrent-complete")
        .await
        .expect("AI job should remain readable after lost completion update");
    assert_eq!(
        job.status, "running",
        "the SQLite trigger simulation rolls back with the transaction; the assertion above proves suggestions did not leak"
    );
}

#[tokio::test]
async fn install_rejects_existing_notes_page_schema_with_nullable_current_drive_version_refs() {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite in-memory pool should be created");

    sqlx::query(
        r#"
        CREATE TABLE notes_workspace (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            organization_id TEXT NOT NULL DEFAULT '0',
            owner_subject_type TEXT NOT NULL,
            owner_subject_id TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            drive_space_id TEXT NOT NULL,
            default_page_content_type TEXT NOT NULL DEFAULT 'application/vnd.sdkwork.notes.page+json',
            default_page_schema_version TEXT NOT NULL DEFAULT '1',
            ai_index_policy_code TEXT NOT NULL DEFAULT 'default',
            lifecycle_status TEXT NOT NULL DEFAULT 'active',
            version INTEGER NOT NULL DEFAULT 1,
            created_by TEXT NOT NULL,
            updated_by TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("legacy notes_workspace table should be created");

    sqlx::query(
        r#"
        CREATE TABLE notes_page (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            organization_id TEXT NOT NULL DEFAULT '0',
            workspace_id TEXT NOT NULL,
            title TEXT NOT NULL,
            page_kind TEXT NOT NULL DEFAULT 'doc',
            parent_page_id TEXT,
            folder_drive_node_id TEXT,
            drive_space_id TEXT NOT NULL,
            drive_node_id TEXT NOT NULL,
            drive_uri TEXT NOT NULL,
            current_drive_version_id TEXT,
            current_drive_version_no INTEGER,
            content_type TEXT NOT NULL,
            content_schema_version TEXT NOT NULL DEFAULT '1',
            content_hash TEXT,
            snippet TEXT,
            icon TEXT,
            cover_asset_id TEXT,
            favorite INTEGER NOT NULL DEFAULT 0,
            archive_status TEXT NOT NULL DEFAULT 'active',
            publish_status TEXT NOT NULL DEFAULT 'private',
            word_count INTEGER NOT NULL DEFAULT 0,
            task_count INTEGER NOT NULL DEFAULT 0,
            drive_lifecycle_status_snapshot TEXT,
            lifecycle_status TEXT NOT NULL DEFAULT 'active',
            version INTEGER NOT NULL DEFAULT 1,
            created_by TEXT NOT NULL,
            updated_by TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            deleted_at TEXT,
            FOREIGN KEY (workspace_id) REFERENCES notes_workspace(id) ON DELETE CASCADE,
            FOREIGN KEY (parent_page_id) REFERENCES notes_page(id) ON DELETE SET NULL,
            CHECK (current_drive_version_no IS NULL OR current_drive_version_no >= 1)
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("legacy notes_page table should be created");

    let error = install_sqlite_schema(&pool)
        .await
        .expect_err("nullable current Drive version refs should be rejected");
    let message = error.to_string();
    assert!(
        message.contains("notes_page.current_drive_version_id"),
        "schema drift error should name notes_page.current_drive_version_id, got: {message}"
    );
    assert!(
        message.contains("notes_page.current_drive_version_no"),
        "schema drift error should name notes_page.current_drive_version_no, got: {message}"
    );
    assert!(
        message.contains("migration") || message.contains("backfill"),
        "schema drift error should explain that migration/backfill is required, got: {message}"
    );
}

fn drive_snapshot(page_id: &str, version_no: i64) -> DrivePageContentSnapshot {
    DrivePageContentSnapshot {
        drive_space_id: "drive-space-001".to_string(),
        drive_node_id: format!("drive-node-{page_id}"),
        drive_uri: format!("drive://spaces/drive-space-001/nodes/drive-node-{page_id}"),
        drive_version_id: format!("drive-version-{page_id}-v{version_no}"),
        drive_version_no: version_no,
        content_type: "application/vnd.sdkwork.notes.page+json".to_string(),
        content_schema_version: "1".to_string(),
        content_hash: Some(format!("sha256:v{version_no}")),
        snippet: Some(format!("hello v{version_no}")),
        word_count: version_no,
        task_count: 0,
        content: json!({ "blocks": [{ "type": "paragraph", "text": format!("hello v{version_no}") }] }),
    }
}

async fn insert_search_projection(
    pool: &sqlx::AnyPool,
    id: &str,
    page_id: &str,
    source_drive_version_id: &str,
    source_drive_version_no: i64,
    plain_text: &str,
) {
    sqlx::query(
        "INSERT INTO notes_page_search_projection (
            id, tenant_id, organization_id, workspace_id, page_id, drive_node_id,
            source_drive_version_id, source_drive_version_no, title_snapshot,
            plain_text, snippet, index_status, indexed_at, rebuild_version
         ) VALUES (
            $1, '100001', '0', 'workspace-001', $2, 'drive-node-page-001',
            $3, $4, 'Roadmap', $5, $5, 'indexed', CURRENT_TIMESTAMP, 1
         )
         ON CONFLICT(tenant_id, organization_id, page_id, source_drive_version_id) DO UPDATE SET
            id=excluded.id,
            plain_text=excluded.plain_text,
            snippet=excluded.snippet,
            index_status='indexed',
            indexed_at=CURRENT_TIMESTAMP",
    )
    .bind(id)
    .bind(page_id)
    .bind(source_drive_version_id)
    .bind(source_drive_version_no)
    .bind(plain_text)
    .execute(pool)
    .await
    .expect("search projection should be inserted");
}

async fn insert_ai_suggestion(pool: &sqlx::AnyPool, id: &str, status: &str) {
    sqlx::query(
        "INSERT OR IGNORE INTO notes_ai_job (
            id, tenant_id, organization_id, workspace_id, job_type, target_type,
            target_id, context_policy_snapshot, status, idempotency_key,
            request_payload_hash, lifecycle_status, version, created_by, updated_by
         ) VALUES (
            'ai-job-001', '100001', '0', 'workspace-001',
            'rewrite', 'page', 'page-001', '{}', 'succeeded',
            'ai-job-001', 'hash-001', 'active', 1, '30', '30'
         )",
    )
    .execute(pool)
    .await
    .expect("AI job should be inserted");

    sqlx::query(
        "INSERT INTO notes_ai_suggestion (
            id, tenant_id, organization_id, workspace_id, page_id, ai_job_id,
            suggestion_type, status, source_drive_node_id, source_drive_version_id,
            source_drive_version_no, payload_json, lifecycle_status, version,
            created_by, updated_by
         ) VALUES (
            $1, '100001', '0', 'workspace-001', 'page-001',
            'ai-job-001', 'rewrite', $2, 'drive-node-page-001',
            'drive-version-page-001-v1', 1,
            '{\"content\":{\"blocks\":[{\"type\":\"paragraph\",\"text\":\"AI rewrite\"}]}}',
            'active', 1, '30', '30'
         )",
    )
    .bind(id)
    .bind(status)
    .execute(pool)
    .await
    .expect("AI suggestion should be inserted");
}

async fn seed_workspace_page_and_ai_job(
    repository: &SqlNotesStore,
    context: &NotesActorContext,
    ai_job_id: &str,
) {
    repository
        .insert_workspace(NewWorkspace {
            id: "workspace-001".to_string(),
            context: context.clone(),
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
        .expect("workspace should be inserted");
    repository
        .insert_page(NewPage {
            id: "page-001".to_string(),
            context: context.clone(),
            workspace_id: "workspace-001".to_string(),
            title: "Roadmap".to_string(),
            page_kind: PageKind::Doc,
            parent_page_id: None,
            folder_drive_node_id: None,
            drive_snapshot: drive_snapshot("page-001", 1),
        })
        .await
        .expect("page should be inserted");
    repository
        .insert_ai_job(sdkwork_notes_pages_service::domain::NewAiJob {
            id: ai_job_id.to_string(),
            context: context.clone(),
            workspace_id: "workspace-001".to_string(),
            job_type: "summarize".to_string(),
            target_type: "page".to_string(),
            target_id: Some("page-001".to_string()),
            prompt_snapshot: Some("Summarize this page".to_string()),
            context_policy_snapshot: json!({ "source": "current_page" }),
            status: "queued".to_string(),
            idempotency_key: ai_job_id.to_string(),
            request_payload_hash: format!("hash-{ai_job_id}"),
            sources: Vec::new(),
        })
        .await
        .expect("AI job should be inserted");
}

async fn table_columns(pool: &sqlx::AnyPool, table: &str) -> Vec<String> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!("PRAGMA table_info({table})")))
        .fetch_all(pool)
        .await
        .expect("table columns should be readable");
    rows.into_iter().map(|row| row.get("name")).collect()
}

async fn index_exists(pool: &sqlx::AnyPool, index_name: &str) -> bool {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(1) FROM sqlite_master WHERE type='index' AND name=$1")
            .bind(index_name)
            .fetch_one(pool)
            .await
            .expect("index metadata should be readable");
    count == 1
}

async fn column_is_not_null(pool: &sqlx::AnyPool, table: &str, column: &str) -> bool {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!("PRAGMA table_info({table})")))
        .fetch_all(pool)
        .await
        .expect("table columns should be readable");
    rows.into_iter().any(|row| {
        let name: String = row.get("name");
        let not_null: i64 = row.get("notnull");
        name == column && not_null == 1
    })
}
