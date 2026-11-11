use crate::domain::{
    AcceptAiSuggestionCommand, AiFeedback, AiFeedbackPage, AiJob, AiJobPage, AiJobSource,
    AiSuggestion, AiSuggestionPage, ApplyAiSuggestionCommand, ClaimAiJobCommand,
    CompleteAiJobCommand, CreateAiFeedbackCommand, CreateAiJobCommand, CreatePageCommand,
    CreateWorkspaceCommand, DriveVersionPage, FailAiJobCommand, ListAiJobsQuery, ListAiSuggestionFeedbackQuery,
    ListPageAiSuggestionsQuery, NewAiFeedback, NewAiJob, NewAiJobSource, NewAiSuggestion, NewPage,
    NewWorkspace, NotesActorContext, Page, PageContent, PageInfo, PageKind, PageMetadataPatch, PageSummary,
    PageSummaryPage, RejectAiSuggestionCommand, RemoteApplyConflict, RemoteApplyMutation,
    RemoteApplyPageCommand, RemoteApplyPageResult, RestorePageVersionCommand, SearchResult,
    SearchResultPage, UpdatePageContentCommand, UpdatePageMetadataCommand, Workspace,
    WorkspaceBootstrap, WorkspacePage,
};
use crate::error::NotesProductError;
use crate::ports::{
    CreateDrivePageContentCommand, DrivePageContentPort, ListDrivePageContentVersionsCommand,
    NotesRepository, ReadDrivePageContentCommand, RestoreDrivePageContentVersionCommand,
    UpdateDrivePageContentCommand,
};
use sdkwork_utils_rust::string::{is_blank, trim};

#[derive(Clone)]
pub struct NotesService<R, D>
where
    R: NotesRepository,
    D: DrivePageContentPort,
{
    repository: R,
    drive: D,
}

impl<R, D> NotesService<R, D>
where
    R: NotesRepository,
    D: DrivePageContentPort,
{
    pub fn new(repository: R, drive: D) -> Self {
        Self { repository, drive }
    }

    pub async fn create_workspace(
        &self,
        command: CreateWorkspaceCommand,
    ) -> Result<Workspace, NotesProductError> {
        let context = normalize_actor_context(command.context)?;
        let workspace_id = normalize_required_string("workspace id", &command.id)?;
        let owner_subject_type = normalize_owner_subject_type(&command.owner_subject_type)?;
        let owner_subject_id =
            normalize_required_string("owner subject id", &command.owner_subject_id)?;
        let name = normalize_required_string("workspace name", &command.name)?;
        validate_max_chars("workspace name", &name, 120)?;
        let description = normalize_optional_string(command.description);
        if let Some(description) = description.as_deref() {
            validate_max_chars("workspace description", description, 2000)?;
        }
        let drive_space_id = normalize_required_string("drive space id", &command.drive_space_id)?;
        let default_page_content_type = normalize_required_string(
            "default page content type",
            &command.default_page_content_type,
        )?;
        validate_max_chars("default page content type", &default_page_content_type, 255)?;
        let default_page_schema_version = normalize_required_string(
            "default page schema version",
            &command.default_page_schema_version,
        )?;
        validate_max_chars(
            "default page schema version",
            &default_page_schema_version,
            32,
        )?;
        let ai_index_policy_code =
            normalize_required_string("AI index policy code", &command.ai_index_policy_code)?;
        validate_max_chars("AI index policy code", &ai_index_policy_code, 64)?;

        self.repository
            .insert_workspace(NewWorkspace {
                id: workspace_id,
                context,
                owner_subject_type,
                owner_subject_id,
                name,
                description,
                drive_space_id,
                default_page_content_type,
                default_page_schema_version,
                ai_index_policy_code,
            })
            .await
    }

    pub async fn create_page(&self, command: CreatePageCommand) -> Result<Page, NotesProductError> {
        let context = normalize_actor_context(command.context)?;
        let page_id = normalize_required_string("page id", &command.id)?;
        let title = normalize_required_string("page title", &command.title)?;
        if title.chars().count() > 200 {
            return Err(NotesProductError::Validation(
                "page title must be at most 200 characters".to_string(),
            ));
        }
        let workspace_id = normalize_required_string("workspace id", &command.workspace_id)?;
        let parent_page_id = normalize_optional_string(command.parent_page_id);
        let folder_drive_node_id = normalize_optional_string(command.folder_drive_node_id);
        if parent_page_id.is_some() && folder_drive_node_id.is_some() {
            return Err(NotesProductError::Validation(
                "parentPageId and folderDriveNodeId cannot both be set".to_string(),
            ));
        }
        let content_type = normalize_required_string("content type", &command.content_type)?;
        let content_schema_version =
            normalize_required_string("content schema version", &command.content_schema_version)?;
        validate_page_content_metadata(&content_type, &content_schema_version)?;
        if !command.initial_content.is_object() {
            return Err(NotesProductError::Validation(
                "initialContent must be an object".to_string(),
            ));
        }

        let workspace = self
            .repository
            .find_workspace(&context, &workspace_id)
            .await?;
        if self.repository.page_id_is_reserved(&page_id).await? {
            return Err(NotesProductError::Conflict(
                "page already exists".to_string(),
            ));
        }
        match self.repository.find_page(&context, &page_id).await {
            Ok(_) => {
                return Err(NotesProductError::Conflict(
                    "page already exists".to_string(),
                ));
            }
            Err(NotesProductError::NotFound(_)) => {}
            Err(error) => return Err(error),
        }
        if let Some(parent_page_id) = parent_page_id.as_deref() {
            let parent_page = self.repository.find_page(&context, parent_page_id).await?;
            if parent_page.workspace_id != workspace.id {
                return Err(NotesProductError::NotFound(
                    "parent page not found in workspace".to_string(),
                ));
            }
        }
        let drive_snapshot = self
            .drive
            .create_page_content(CreateDrivePageContentCommand {
                tenant_id: context.tenant_id.clone(),
                organization_id: context.organization_id.clone(),
                operator_id: context.operator_id.clone(),
                workspace_id: workspace.id.clone(),
                page_id: page_id.clone(),
                title: title.clone(),
                drive_space_id: workspace.drive_space_id.clone(),
                parent_page_id: parent_page_id.clone(),
                folder_drive_node_id: folder_drive_node_id.clone(),
                content: command.initial_content,
                content_type: content_type.clone(),
                content_schema_version: content_schema_version.clone(),
                change_summary: normalize_optional_string(command.change_summary),
            })
            .await?;
        validate_drive_snapshot("create page content", &drive_snapshot)?;
        validate_drive_snapshot_matches_expected_refs(
            "create page content",
            &drive_snapshot,
            Some(&workspace.drive_space_id),
            None,
            None,
        )?;
        validate_drive_snapshot_matches_expected_content_metadata(
            "create page content",
            &drive_snapshot,
            &content_type,
            &content_schema_version,
        )?;

        self.repository
            .insert_page(NewPage {
                id: page_id,
                context,
                workspace_id: workspace.id,
                title,
                page_kind: command.page_kind,
                parent_page_id,
                folder_drive_node_id,
                drive_snapshot,
            })
            .await
            .map_err(|error| {
                notes_persistence_failed_after_drive_write(
                    "Drive page content create succeeded",
                    "Notes page was not persisted",
                    error,
                )
            })
    }

    pub async fn get_page(
        &self,
        context: &NotesActorContext,
        page_id: &str,
    ) -> Result<Page, NotesProductError> {
        let context = normalize_actor_context(context.clone())?;
        let page_id = normalize_required_string("page id", page_id)?;
        self.repository.find_page(&context, &page_id).await
    }

    pub async fn list_workspaces(
        &self,
        query: crate::domain::ListWorkspacesQuery,
    ) -> Result<WorkspacePage, NotesProductError> {
        let context = normalize_actor_context(query.context)?;
        let page = normalize_page(query.page)?;
        let page_size = normalize_page_size(query.page_size)?;
        let mut items = self
            .repository
            .list_workspaces(&context, page, page_size)
            .await?;
        let page_info = page_info_from_extra_item(&mut items, page, page_size);

        Ok(WorkspacePage { items, page_info })
    }

    pub async fn list_pages(
        &self,
        query: crate::domain::ListPagesQuery,
    ) -> Result<PageSummaryPage, NotesProductError> {
        let context = normalize_actor_context(query.context)?;
        let workspace_id = normalize_required_string("workspace id", &query.workspace_id)?;
        let page = normalize_page(query.page)?;
        let page_size = normalize_page_size(query.page_size)?;

        self.repository
            .find_workspace(&context, &workspace_id)
            .await?;

        let q = normalize_optional_query("q", query.q.as_deref())?;
        let mut pages = self
            .repository
            .list_pages(&context, &workspace_id, page, page_size, q)
            .await?;
        let page_info = page_info_from_extra_item(&mut pages, page, page_size);
        let items = pages.into_iter().map(PageSummary::from).collect();

        Ok(PageSummaryPage { items, page_info })
    }

    pub async fn get_workspace_bootstrap(
        &self,
        context: &NotesActorContext,
        workspace_id: &str,
    ) -> Result<WorkspaceBootstrap, NotesProductError> {
        let context = normalize_actor_context(context.clone())?;
        let workspace_id = normalize_required_string("workspace id", workspace_id)?;
        let workspace = self
            .repository
            .find_workspace(&context, &workspace_id)
            .await?;
        let root_pages = self
            .repository
            .list_root_pages(&context, &workspace_id, 200)
            .await?
            .into_iter()
            .map(PageSummary::from)
            .collect::<Vec<_>>();
        let change_token = workspace_change_token(&workspace, &root_pages);

        Ok(WorkspaceBootstrap {
            workspace,
            root_pages,
            object_types: built_in_object_types(),
            change_token: Some(change_token),
        })
    }

    pub async fn update_page_metadata(
        &self,
        command: UpdatePageMetadataCommand,
    ) -> Result<Page, NotesProductError> {
        let context = normalize_actor_context(command.context)?;
        let page_id = normalize_required_string("page id", &command.page_id)?;
        let current = self.repository.find_page(&context, &page_id).await?;
        let mut expected_version_for_update = None;

        if let Some(expected_version) = command.expected_version.as_deref() {
            let expected_version = normalize_required_string("expectedVersion", expected_version)?
                .parse::<i64>()
                .map_err(|_| {
                    NotesProductError::Validation(
                        "expectedVersion must be an int64 string".to_string(),
                    )
                })?;
            if expected_version != current.version {
                return Err(NotesProductError::Conflict(
                    "page version has changed".to_string(),
                ));
            }
            expected_version_for_update = Some(expected_version);
        }

        if command.title.is_none()
            && command.favorite.is_none()
            && command.archive_status.is_none()
            && command.publish_status.is_none()
            && command.parent_page_id.is_none()
        {
            return Ok(current);
        }

        if let Some(parent_page_id) = command.parent_page_id.as_ref() {
            if let Some(parent_page_id) = parent_page_id.as_deref() {
                let parent_page = self.repository.find_page(&context, parent_page_id).await?;
                if parent_page.workspace_id != current.workspace_id {
                    return Err(NotesProductError::NotFound(
                        "parent page not found in workspace".to_string(),
                    ));
                }
                if parent_page_id == page_id {
                    return Err(NotesProductError::Validation(
                        "page cannot be its own parent".to_string(),
                    ));
                }
            }
        }

        let title = command
            .title
            .map(|value| normalize_required_string("page title", &value))
            .transpose()?
            .unwrap_or_else(|| current.title.clone());
        if title.chars().count() > 200 {
            return Err(NotesProductError::Validation(
                "page title must be at most 200 characters".to_string(),
            ));
        }

        let archive_status = command
            .archive_status
            .map(|value| normalize_required_string("archiveStatus", &value))
            .transpose()?
            .unwrap_or_else(|| current.archive_status.clone());
        if !matches!(archive_status.as_str(), "active" | "archived") {
            return Err(NotesProductError::Validation(
                "archiveStatus is invalid".to_string(),
            ));
        }

        let publish_status = command
            .publish_status
            .map(|value| normalize_required_string("publishStatus", &value))
            .transpose()?
            .unwrap_or_else(|| current.publish_status.clone());
        if !matches!(
            publish_status.as_str(),
            "private" | "published" | "unlisted"
        ) {
            return Err(NotesProductError::Validation(
                "publishStatus is invalid".to_string(),
            ));
        }

        self.repository
            .update_page_metadata(
                &context,
                &page_id,
                &PageMetadataPatch {
                    title,
                    favorite: command.favorite.unwrap_or(current.favorite),
                    archive_status,
                    publish_status,
                    parent_page_id: command.parent_page_id,
                },
                expected_version_for_update,
            )
            .await
    }

    pub async fn remote_apply_page(
        &self,
        command: RemoteApplyPageCommand,
    ) -> Result<RemoteApplyPageResult, NotesProductError> {
        let context = normalize_actor_context(command.context)?;
        let page_id = normalize_required_string("page id", &command.page_id)?;
        let idempotency_key = normalize_required_string("idempotencyKey", &command.idempotency_key)?;
        let task_id = normalize_required_string("taskId", &command.task_id)?;
        let entity_type = normalize_required_string("entityType", &command.entity_type)?;
        let entity_id = normalize_required_string("entityId", &command.entity_id)?;
        let operation = normalize_required_string("operation", &command.operation)?;

        if page_id != entity_id {
            return Err(NotesProductError::Validation(
                "path page id must match request entityId".to_string(),
            ));
        }
        if entity_type != "note" {
            return Err(NotesProductError::Validation(
                "request entityType must remain note".to_string(),
            ));
        }
        let _ = idempotency_key;

        let current = match self.repository.find_page(&context, &page_id).await {
            Ok(page) => page,
            Err(NotesProductError::NotFound(_)) => {
                return Ok(remote_apply_conflict_result(
                    task_id,
                    command.base_remote_cursor,
                    "deleted-remotely",
                    "page not found",
                ));
            }
            Err(error) => return Err(error),
        };

        if let Some(base_remote_cursor) = normalize_optional_string(command.base_remote_cursor) {
            if base_remote_cursor != current.current_drive_version_id {
                return Ok(remote_apply_conflict_result(
                    task_id,
                    Some(current.current_drive_version_id),
                    "stale-base-version",
                    "Remote cursor does not match the current page version.",
                ));
            }
        }

        let applied_page = match operation.as_str() {
            "upsert" => {
                let RemoteApplyMutation::UpsertPatch {
                    title,
                    content,
                    parent_id,
                    is_favorite,
                    publish_status,
                } = command.mutation
                else {
                    return Err(NotesProductError::Validation(
                        "upsert remote apply requires a patch mutation".to_string(),
                    ));
                };

                if title.is_some() || is_favorite.is_some() || publish_status.is_some() || parent_id.is_some() {
                    self.update_page_metadata(UpdatePageMetadataCommand {
                        context: context.clone(),
                        page_id: page_id.clone(),
                        title,
                        favorite: is_favorite,
                        archive_status: None,
                        publish_status,
                        parent_page_id: parent_id,
                        expected_version: None,
                    })
                    .await?;
                }

                if let Some(content_text) = content {
                    let refreshed = self.repository.find_page(&context, &page_id).await?;
                    let content_value = serde_json::json!({ "text": content_text });
                    self.update_page_content(UpdatePageContentCommand {
                        context: context.clone(),
                        page_id: page_id.clone(),
                        content: content_value,
                        content_type: None,
                        content_schema_version: None,
                        change_summary: None,
                        expected_drive_version_id: Some(refreshed.current_drive_version_id),
                        create_checkpoint: false,
                    })
                    .await?;
                }

                self.repository.find_page(&context, &page_id).await?
            }
            "move" => {
                let RemoteApplyMutation::Move { target_parent_id } = command.mutation else {
                    return Err(NotesProductError::Validation(
                        "move remote apply requires targetParentId".to_string(),
                    ));
                };
                self.update_page_metadata(UpdatePageMetadataCommand {
                    context: context.clone(),
                    page_id: page_id.clone(),
                    title: None,
                    favorite: None,
                    archive_status: None,
                    publish_status: None,
                    parent_page_id: Some(target_parent_id),
                    expected_version: None,
                })
                .await?
            }
            "delete" => {
                self.update_page_metadata(UpdatePageMetadataCommand {
                    context: context.clone(),
                    page_id: page_id.clone(),
                    title: None,
                    favorite: None,
                    archive_status: Some("archived".to_string()),
                    publish_status: None,
                    parent_page_id: None,
                    expected_version: None,
                })
                .await?
            }
            "restore" => {
                self.update_page_metadata(UpdatePageMetadataCommand {
                    context: context.clone(),
                    page_id: page_id.clone(),
                    title: None,
                    favorite: None,
                    archive_status: Some("active".to_string()),
                    publish_status: None,
                    parent_page_id: None,
                    expected_version: None,
                })
                .await?
            }
            "permanent-delete" => {
                if !matches!(command.mutation, RemoteApplyMutation::PermanentDeleteIntent) {
                    return Err(NotesProductError::Validation(
                        "permanent-delete remote apply requires a permanent-delete intent"
                            .to_string(),
                    ));
                }
                if current.archive_status != "archived" && current.page_kind != PageKind::Folder {
                    return Ok(remote_apply_conflict_result(
                        task_id,
                        Some(current.current_drive_version_id),
                        "page-not-in-trash",
                        "Permanent delete requires the page to be archived first.",
                    ));
                }
                let remote_cursor = current.current_drive_version_id.clone();
                let applied_at = current.updated_at.clone();
                self.repository.delete_page(&context, &page_id).await?;
                return Ok(RemoteApplyPageResult {
                    outcome: "applied".to_string(),
                    task_id,
                    remote_cursor: Some(remote_cursor),
                    applied_at: Some(applied_at),
                    conflict: None,
                });
            }
            _ => {
                return Err(NotesProductError::Validation(format!(
                    "unsupported remote apply operation \"{operation}\""
                )));
            }
        };

        Ok(RemoteApplyPageResult {
            outcome: "applied".to_string(),
            task_id,
            remote_cursor: Some(applied_page.current_drive_version_id),
            applied_at: Some(applied_page.updated_at),
            conflict: None,
        })
    }

    pub async fn update_page_content(
        &self,
        command: UpdatePageContentCommand,
    ) -> Result<PageContent, NotesProductError> {
        let context = normalize_actor_context(command.context)?;
        let page_id = normalize_required_string("page id", &command.page_id)?;
        if !command.content.is_object() {
            return Err(NotesProductError::Validation(
                "content must be an object".to_string(),
            ));
        }
        let page = self.repository.find_page(&context, &page_id).await?;
        let expected_drive_version_id =
            normalize_optional_string(command.expected_drive_version_id);
        if let Some(expected_drive_version_id) = expected_drive_version_id.as_deref() {
            if page.current_drive_version_id != expected_drive_version_id {
                return Err(NotesProductError::Conflict(
                    "page Drive version has changed".to_string(),
                ));
            }
        }
        let drive_expected_drive_version_id = expected_drive_version_id
            .clone()
            .unwrap_or_else(|| page.current_drive_version_id.clone());
        let content_type = normalize_optional_string(command.content_type)
            .unwrap_or_else(|| page.content_type.clone());
        let content_schema_version = normalize_optional_string(command.content_schema_version)
            .unwrap_or_else(|| page.content_schema_version.clone());
        validate_page_content_metadata(&content_type, &content_schema_version)?;
        let drive_snapshot = self
            .drive
            .update_page_content(UpdateDrivePageContentCommand {
                tenant_id: context.tenant_id.clone(),
                organization_id: context.organization_id.clone(),
                operator_id: context.operator_id.clone(),
                workspace_id: page.workspace_id.clone(),
                page_id: page.id.clone(),
                drive_space_id: page.drive_space_id.clone(),
                drive_node_id: page.drive_node_id.clone(),
                drive_uri: page.drive_uri.clone(),
                current_drive_version_id: page.current_drive_version_id.clone(),
                content: command.content,
                content_type: content_type.clone(),
                content_schema_version: content_schema_version.clone(),
                change_summary: normalize_optional_string(command.change_summary),
                expected_drive_version_id: Some(drive_expected_drive_version_id),
                create_checkpoint: command.create_checkpoint,
            })
            .await?;
        validate_drive_snapshot("update page content", &drive_snapshot)?;
        validate_drive_snapshot_matches_expected_refs(
            "update page content",
            &drive_snapshot,
            Some(&page.drive_space_id),
            Some(&page.drive_node_id),
            Some(&page.drive_uri),
        )?;
        validate_drive_snapshot_advances_expected_version(
            "update page content",
            &drive_snapshot,
            &page.current_drive_version_id,
            page.current_drive_version_no,
        )?;
        validate_drive_snapshot_matches_expected_content_metadata(
            "update page content",
            &drive_snapshot,
            &content_type,
            &content_schema_version,
        )?;

        self.repository
            .update_page_drive_snapshot(
                &context,
                &page.id,
                &drive_snapshot,
                &page.current_drive_version_id,
            )
            .await
            .map_err(|error| {
                notes_persistence_failed_after_drive_write(
                    "Drive page content update succeeded",
                    "Notes page pointer was not advanced",
                    error,
                )
            })?;

        Ok(PageContent {
            page_id: page.id,
            drive_node_id: drive_snapshot.drive_node_id,
            drive_version_id: drive_snapshot.drive_version_id,
            drive_version_no: drive_snapshot.drive_version_no,
            content_type: drive_snapshot.content_type,
            content_schema_version: drive_snapshot.content_schema_version,
            content: drive_snapshot.content,
        })
    }

    pub async fn get_page_content(
        &self,
        context: &NotesActorContext,
        page_id: &str,
    ) -> Result<PageContent, NotesProductError> {
        let context = normalize_actor_context(context.clone())?;
        let page_id = normalize_required_string("page id", page_id)?;
        let page = self.repository.find_page(&context, &page_id).await?;
        let drive_snapshot = self
            .drive
            .read_page_content(ReadDrivePageContentCommand {
                tenant_id: context.tenant_id.clone(),
                organization_id: context.organization_id.clone(),
                operator_id: context.operator_id.clone(),
                page_id: page.id.clone(),
                drive_space_id: page.drive_space_id.clone(),
                drive_node_id: page.drive_node_id.clone(),
                drive_uri: page.drive_uri.clone(),
                current_drive_version_id: page.current_drive_version_id.clone(),
                content_type: page.content_type.clone(),
                content_schema_version: page.content_schema_version.clone(),
            })
            .await?;
        validate_drive_snapshot("read page content", &drive_snapshot)?;
        validate_drive_snapshot_matches_expected_refs(
            "read page content",
            &drive_snapshot,
            Some(&page.drive_space_id),
            Some(&page.drive_node_id),
            Some(&page.drive_uri),
        )?;
        validate_drive_snapshot_matches_expected_version(
            "read page content",
            &drive_snapshot,
            &page.current_drive_version_id,
            page.current_drive_version_no,
        )?;
        validate_drive_snapshot_matches_expected_content_metadata(
            "read page content",
            &drive_snapshot,
            &page.content_type,
            &page.content_schema_version,
        )?;

        Ok(PageContent {
            page_id: page.id,
            drive_node_id: drive_snapshot.drive_node_id,
            drive_version_id: drive_snapshot.drive_version_id,
            drive_version_no: drive_snapshot.drive_version_no,
            content_type: drive_snapshot.content_type,
            content_schema_version: drive_snapshot.content_schema_version,
            content: drive_snapshot.content,
        })
    }

    pub async fn list_page_versions(
        &self,
        query: crate::domain::ListPageVersionsQuery,
    ) -> Result<DriveVersionPage, NotesProductError> {
        let context = normalize_actor_context(query.context)?;
        let page_id = normalize_required_string("page id", &query.page_id)?;
        let page_number = normalize_page(query.page)?;
        let page_size = normalize_page_size(query.page_size)?;
        let page = self.repository.find_page(&context, &page_id).await?;

        let versions = self
            .drive
            .list_page_content_versions(ListDrivePageContentVersionsCommand {
                tenant_id: context.tenant_id,
                organization_id: context.organization_id,
                operator_id: context.operator_id,
                page_id: page.id,
                drive_space_id: page.drive_space_id,
                drive_node_id: page.drive_node_id,
                drive_uri: page.drive_uri,
                current_drive_version_id: page.current_drive_version_id,
                page: page_number,
                page_size,
            })
            .await?;
        validate_drive_version_page("list page versions", &versions, page_number, page_size)?;
        Ok(versions)
    }

    pub async fn restore_page_version(
        &self,
        command: RestorePageVersionCommand,
    ) -> Result<PageContent, NotesProductError> {
        let context = normalize_actor_context(command.context)?;
        let page_id = normalize_required_string("page id", &command.page_id)?;
        let drive_version_id =
            normalize_required_string("driveVersionId", &command.drive_version_id)?;
        let expected_current_drive_version_id =
            normalize_optional_string(command.expected_current_drive_version_id);
        let page = self.repository.find_page(&context, &page_id).await?;
        if let Some(expected_current_drive_version_id) =
            expected_current_drive_version_id.as_deref()
        {
            if page.current_drive_version_id != expected_current_drive_version_id {
                return Err(NotesProductError::Conflict(
                    "page Drive version has changed".to_string(),
                ));
            }
        }
        let drive_expected_current_drive_version_id = expected_current_drive_version_id
            .clone()
            .unwrap_or_else(|| page.current_drive_version_id.clone());

        let drive_snapshot = self
            .drive
            .restore_page_content_version(RestoreDrivePageContentVersionCommand {
                tenant_id: context.tenant_id.clone(),
                organization_id: context.organization_id.clone(),
                operator_id: context.operator_id.clone(),
                page_id: page.id.clone(),
                drive_space_id: page.drive_space_id.clone(),
                drive_node_id: page.drive_node_id.clone(),
                drive_uri: page.drive_uri.clone(),
                current_drive_version_id: page.current_drive_version_id.clone(),
                drive_version_id,
                expected_current_drive_version_id: Some(drive_expected_current_drive_version_id),
            })
            .await?;
        validate_drive_snapshot("restore page version", &drive_snapshot)?;
        validate_drive_snapshot_matches_expected_refs(
            "restore page version",
            &drive_snapshot,
            Some(&page.drive_space_id),
            Some(&page.drive_node_id),
            Some(&page.drive_uri),
        )?;
        validate_drive_snapshot_advances_expected_version(
            "restore page version",
            &drive_snapshot,
            &page.current_drive_version_id,
            page.current_drive_version_no,
        )?;

        self.repository
            .update_page_drive_snapshot(
                &context,
                &page.id,
                &drive_snapshot,
                &page.current_drive_version_id,
            )
            .await
            .map_err(|error| {
                notes_persistence_failed_after_drive_write(
                    "Drive page content restore succeeded",
                    "Notes page pointer was not advanced",
                    error,
                )
            })?;

        Ok(PageContent {
            page_id: page.id,
            drive_node_id: drive_snapshot.drive_node_id,
            drive_version_id: drive_snapshot.drive_version_id,
            drive_version_no: drive_snapshot.drive_version_no,
            content_type: drive_snapshot.content_type,
            content_schema_version: drive_snapshot.content_schema_version,
            content: drive_snapshot.content,
        })
    }

    pub async fn query_search(
        &self,
        query: crate::domain::SearchQuery,
    ) -> Result<SearchResultPage, NotesProductError> {
        let context = normalize_actor_context(query.context)?;
        let page = normalize_page(query.page)?;
        let page_size = normalize_page_size(query.page_size)?;
        let q = normalize_optional_query("q", query.q.as_deref())?;

        let workspace_id = query
            .workspace_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let mut pages = self
            .repository
            .search_pages(&context, workspace_id, q, page, page_size)
            .await?;
        let page_info = page_info_from_extra_item(&mut pages, page, page_size);
        let items = pages
            .into_iter()
            .map(|page| search_result_from_page(page, q))
            .collect();

        Ok(SearchResultPage { items, page_info })
    }

    pub async fn create_ai_job(
        &self,
        command: CreateAiJobCommand,
    ) -> Result<AiJob, NotesProductError> {
        let context = normalize_actor_context(command.context)?;
        let workspace_id = normalize_required_string("workspace id", &command.workspace_id)?;
        let idempotency_key =
            normalize_required_string("idempotency key", &command.idempotency_key)?;
        let job_type = normalize_ai_job_type(&command.job_type)?;
        let target_type = normalize_ai_target_type(&command.target_type)?;
        let target_id = command
            .target_id
            .as_ref().map(|value| trim(value))
            .filter(|value| !value.is_empty());
        let prompt_snapshot = command
            .prompt
            .as_ref().map(|value| trim(value))
            .filter(|value| !value.is_empty());
        if prompt_snapshot
            .as_ref()
            .is_some_and(|value| value.chars().count() > 8000)
        {
            return Err(NotesProductError::Validation(
                "prompt must be at most 8000 characters".to_string(),
            ));
        }
        let context_policy_snapshot = command
            .context_policy
            .unwrap_or_else(|| serde_json::json!({}));
        if !context_policy_snapshot.is_object() {
            return Err(NotesProductError::Validation(
                "contextPolicy must be an object".to_string(),
            ));
        }

        validate_ai_target(&target_type, target_id.as_deref())?;
        let request_payload_hash = ai_job_payload_hash(
            &workspace_id,
            &job_type,
            &target_type,
            target_id.as_deref(),
            prompt_snapshot.as_deref(),
            &context_policy_snapshot,
        )?;

        if let Some(existing) = self
            .repository
            .find_ai_job_by_idempotency_key(&context, &idempotency_key)
            .await?
        {
            if existing.request_payload_hash == request_payload_hash {
                return Ok(existing);
            }
            return Err(NotesProductError::Conflict(
                "idempotency key was already used for a different AI job request".to_string(),
            ));
        }

        let workspace = self
            .repository
            .find_workspace(&context, &workspace_id)
            .await?;
        let sources = self
            .ai_job_sources_for_target(&context, &workspace.id, &target_type, target_id.as_deref())
            .await?;
        let job_id = stable_ai_job_id(&context, &idempotency_key, &request_payload_hash);

        match self
            .repository
            .insert_ai_job(NewAiJob {
                id: job_id,
                context: context.clone(),
                workspace_id: workspace.id,
                job_type,
                target_type,
                target_id,
                prompt_snapshot,
                context_policy_snapshot,
                status: "queued".to_string(),
                idempotency_key: idempotency_key.clone(),
                request_payload_hash: request_payload_hash.clone(),
                sources,
            })
            .await
        {
            Ok(job) => Ok(job),
            Err(NotesProductError::Conflict(message)) => {
                if let Some(existing) = self
                    .repository
                    .find_ai_job_by_idempotency_key(&context, &idempotency_key)
                    .await?
                {
                    if existing.request_payload_hash == request_payload_hash {
                        return Ok(existing);
                    }
                }
                Err(NotesProductError::Conflict(message))
            }
            Err(error) => Err(error),
        }
    }

    pub async fn list_ai_jobs(
        &self,
        query: ListAiJobsQuery,
    ) -> Result<AiJobPage, NotesProductError> {
        let context = normalize_actor_context(query.context)?;
        let page = normalize_page(query.page)?;
        let page_size = normalize_page_size(query.page_size)?;
        let workspace_id = query
            .workspace_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let mut items = self
            .repository
            .list_ai_jobs(&context, workspace_id, page, page_size)
            .await?;
        let page_info = page_info_from_extra_item(&mut items, page, page_size);

        Ok(AiJobPage { items, page_info })
    }

    pub async fn get_ai_job(
        &self,
        context: &NotesActorContext,
        ai_job_id: &str,
    ) -> Result<AiJob, NotesProductError> {
        let context = normalize_actor_context(context.clone())?;
        let ai_job_id = normalize_required_string("AI job id", ai_job_id)?;
        self.repository.find_ai_job(&context, &ai_job_id).await
    }

    pub async fn cancel_ai_job(
        &self,
        context: &NotesActorContext,
        ai_job_id: &str,
    ) -> Result<AiJob, NotesProductError> {
        let context = normalize_actor_context(context.clone())?;
        let ai_job_id = normalize_required_string("AI job id", ai_job_id)?;
        self.repository.cancel_ai_job(&context, &ai_job_id).await
    }

    pub async fn claim_ai_job(
        &self,
        command: ClaimAiJobCommand,
    ) -> Result<AiJob, NotesProductError> {
        let context = normalize_actor_context(command.context)?;
        let ai_job_id = normalize_required_string("AI job id", &command.ai_job_id)?;
        self.repository.claim_ai_job(&context, &ai_job_id).await
    }

    pub async fn complete_ai_job(
        &self,
        command: CompleteAiJobCommand,
    ) -> Result<AiJob, NotesProductError> {
        let context = normalize_actor_context(command.context)?;
        let ai_job_id = normalize_required_string("AI job id", &command.ai_job_id)?;
        if command.suggestions.is_empty() {
            return Err(NotesProductError::Validation(
                "suggestions must not be empty".to_string(),
            ));
        }

        let job = self.repository.find_ai_job(&context, &ai_job_id).await?;
        if job.status != "running" {
            return Err(NotesProductError::Conflict(
                "AI job must be running before completion".to_string(),
            ));
        }
        let sources = self
            .repository
            .list_ai_job_sources(&context, &ai_job_id)
            .await?;

        let mut new_suggestions = Vec::with_capacity(command.suggestions.len());
        for (index, suggestion) in command.suggestions.into_iter().enumerate() {
            let suggestion_type = normalize_ai_suggestion_type(&suggestion.suggestion_type)?;
            if !suggestion.payload.is_object() {
                return Err(NotesProductError::Validation(
                    "suggestion payload must be an object".to_string(),
                ));
            }
            let page_id = suggestion
                .page_id
                .as_ref().map(|value| trim(value))
                .filter(|value| !value.is_empty())
                .or_else(|| page_id_from_ai_job(&job, &sources))
                .ok_or_else(|| {
                    NotesProductError::Validation(
                        "pageId is required for this suggestion".to_string(),
                    )
                })?;
            let page = self.repository.find_page(&context, &page_id).await?;
            if page.workspace_id != job.workspace_id {
                return Err(NotesProductError::NotFound(
                    "page not found in AI job workspace".to_string(),
                ));
            }
            let source = source_for_page(&sources, &page_id);
            if has_page_sources(&sources) && source.is_none() {
                return Err(NotesProductError::Validation(
                    "suggestion pageId must match an AI job page source".to_string(),
                ));
            }
            let payload_hash = serde_json::to_string(&suggestion.payload).map_err(|error| {
                NotesProductError::Internal(format!(
                    "serialize AI suggestion payload failed: {error}"
                ))
            })?;
            let id = stable_ai_suggestion_id(
                &context,
                &job.id,
                &page_id,
                &suggestion_type,
                index,
                &payload_hash,
            );
            new_suggestions.push(NewAiSuggestion {
                id,
                context: context.clone(),
                workspace_id: job.workspace_id.clone(),
                page_id,
                ai_job_id: job.id.clone(),
                suggestion_type,
                payload: suggestion.payload,
                source_drive_node_id: source.and_then(|source| source.drive_node_id.clone()),
                source_drive_version_id: source.and_then(|source| source.drive_version_id.clone()),
                source_drive_version_no: source.and_then(|source| source.drive_version_no),
            });
        }

        self.repository
            .complete_ai_job(&context, &ai_job_id, new_suggestions)
            .await
    }

    pub async fn fail_ai_job(&self, command: FailAiJobCommand) -> Result<AiJob, NotesProductError> {
        let context = normalize_actor_context(command.context)?;
        let ai_job_id = normalize_required_string("AI job id", &command.ai_job_id)?;
        let error_code = normalize_required_string("error code", &command.error_code)?;
        validate_max_chars("error code", &error_code, 64)?;
        let error_message = normalize_required_string("error message", &command.error_message)?;
        validate_max_chars("error message", &error_message, 2000)?;
        self.repository
            .fail_ai_job(&context, &ai_job_id, &error_code, &error_message)
            .await
    }

    pub async fn list_page_ai_suggestions(
        &self,
        query: ListPageAiSuggestionsQuery,
    ) -> Result<AiSuggestionPage, NotesProductError> {
        let context = normalize_actor_context(query.context)?;
        let page_id = normalize_required_string("page id", &query.page_id)?;
        let page = normalize_page(query.page)?;
        let page_size = normalize_page_size(query.page_size)?;
        self.repository.find_page(&context, &page_id).await?;

        let mut items = self
            .repository
            .list_page_ai_suggestions(&context, &page_id, page, page_size)
            .await?;
        let page_info = page_info_from_extra_item(&mut items, page, page_size);

        Ok(AiSuggestionPage { items, page_info })
    }

    pub async fn accept_ai_suggestion(
        &self,
        command: AcceptAiSuggestionCommand,
    ) -> Result<AiSuggestion, NotesProductError> {
        let context = normalize_actor_context(command.context)?;
        let ai_suggestion_id =
            normalize_required_string("AI suggestion id", &command.ai_suggestion_id)?;
        self.decide_ai_suggestion(&context, &ai_suggestion_id, "accepted", "rejected")
            .await
    }

    pub async fn reject_ai_suggestion(
        &self,
        command: RejectAiSuggestionCommand,
    ) -> Result<AiSuggestion, NotesProductError> {
        let context = normalize_actor_context(command.context)?;
        let ai_suggestion_id =
            normalize_required_string("AI suggestion id", &command.ai_suggestion_id)?;
        self.decide_ai_suggestion(&context, &ai_suggestion_id, "rejected", "accepted")
            .await
    }

    pub async fn apply_ai_suggestion(
        &self,
        command: ApplyAiSuggestionCommand,
    ) -> Result<PageContent, NotesProductError> {
        let context = normalize_actor_context(command.context)?;
        let ai_suggestion_id =
            normalize_required_string("AI suggestion id", &command.ai_suggestion_id)?;
        let expected_drive_version_id =
            normalize_optional_string(command.expected_drive_version_id);
        let suggestion = self
            .repository
            .find_ai_suggestion(&context, &ai_suggestion_id)
            .await?;

        match suggestion.status.as_str() {
            "applied" => {
                return self.get_page_content(&context, &suggestion.page_id).await;
            }
            "accepted" => {}
            "proposed" => {
                return Err(NotesProductError::Conflict(
                    "AI suggestion must be accepted before apply".to_string(),
                ));
            }
            "rejected" | "dismissed" => {
                return Err(NotesProductError::Conflict(
                    "AI suggestion decision is already terminal".to_string(),
                ));
            }
            other => {
                return Err(NotesProductError::Internal(format!(
                    "invalid persisted AI suggestion status: {other}"
                )));
            }
        }

        let page = self
            .repository
            .find_page(&context, &suggestion.page_id)
            .await?;
        if page.workspace_id != suggestion.workspace_id {
            return Err(NotesProductError::NotFound(
                "page not found in AI suggestion workspace".to_string(),
            ));
        }
        if let Some(source_drive_version_id) = suggestion.source_drive_version_id.as_deref() {
            if page.current_drive_version_id != source_drive_version_id {
                return Err(NotesProductError::Conflict(
                    "AI suggestion source Drive version is stale".to_string(),
                ));
            }
        }
        if let Some(source_drive_version_no) = suggestion.source_drive_version_no {
            if page.current_drive_version_no != source_drive_version_no {
                return Err(NotesProductError::Conflict(
                    "AI suggestion source Drive version is stale".to_string(),
                ));
            }
        }
        if let Some(expected_drive_version_id) = expected_drive_version_id.as_deref() {
            if page.current_drive_version_id != expected_drive_version_id {
                return Err(NotesProductError::Conflict(
                    "page Drive version has changed".to_string(),
                ));
            }
        }

        let content = suggestion.payload.get("content").ok_or_else(|| {
            NotesProductError::Validation("suggestion payload content is required".to_string())
        })?;
        if !content.is_object() {
            return Err(NotesProductError::Validation(
                "suggestion payload content must be an object".to_string(),
            ));
        }
        let content = content.clone();
        let content_type = optional_payload_string(&suggestion.payload, "contentType")?
            .unwrap_or_else(|| page.content_type.clone());
        let content_schema_version =
            optional_payload_string(&suggestion.payload, "contentSchemaVersion")?
                .unwrap_or_else(|| page.content_schema_version.clone());
        validate_page_content_metadata(&content_type, &content_schema_version)?;

        let drive_snapshot = self
            .drive
            .update_page_content(UpdateDrivePageContentCommand {
                tenant_id: context.tenant_id.clone(),
                organization_id: context.organization_id.clone(),
                operator_id: context.operator_id.clone(),
                workspace_id: page.workspace_id.clone(),
                page_id: page.id.clone(),
                drive_space_id: page.drive_space_id.clone(),
                drive_node_id: page.drive_node_id.clone(),
                drive_uri: page.drive_uri.clone(),
                current_drive_version_id: page.current_drive_version_id.clone(),
                content,
                content_type: content_type.clone(),
                content_schema_version: content_schema_version.clone(),
                change_summary: Some(format!("Apply AI suggestion {}", suggestion.id)),
                expected_drive_version_id: expected_drive_version_id
                    .or_else(|| suggestion.source_drive_version_id.clone())
                    .or_else(|| Some(page.current_drive_version_id.clone())),
                create_checkpoint: command.create_checkpoint,
            })
            .await?;
        validate_drive_snapshot("apply AI suggestion", &drive_snapshot)?;
        validate_drive_snapshot_matches_expected_refs(
            "apply AI suggestion",
            &drive_snapshot,
            Some(&page.drive_space_id),
            Some(&page.drive_node_id),
            Some(&page.drive_uri),
        )?;
        validate_drive_snapshot_advances_expected_version(
            "apply AI suggestion",
            &drive_snapshot,
            &page.current_drive_version_id,
            page.current_drive_version_no,
        )?;
        validate_drive_snapshot_matches_expected_content_metadata(
            "apply AI suggestion",
            &drive_snapshot,
            &content_type,
            &content_schema_version,
        )?;

        let applied_suggestion = self
            .repository
            .apply_ai_suggestion(
                &context,
                &suggestion.id,
                &page.id,
                &drive_snapshot,
                &page.current_drive_version_id,
            )
            .await
            .map_err(|error| {
                notes_persistence_failed_after_drive_write(
                    "Drive page content update for AI suggestion succeeded",
                    "Notes AI suggestion apply was not committed",
                    error,
                )
            })?;
        if applied_suggestion.status != "applied" {
            return Err(NotesProductError::Conflict(
                "AI suggestion status changed before apply completed".to_string(),
            ));
        }

        Ok(PageContent {
            page_id: page.id,
            drive_node_id: drive_snapshot.drive_node_id,
            drive_version_id: drive_snapshot.drive_version_id,
            drive_version_no: drive_snapshot.drive_version_no,
            content_type: drive_snapshot.content_type,
            content_schema_version: drive_snapshot.content_schema_version,
            content: drive_snapshot.content,
        })
    }

    pub async fn create_ai_feedback(
        &self,
        command: CreateAiFeedbackCommand,
    ) -> Result<AiFeedback, NotesProductError> {
        let context = normalize_actor_context(command.context)?;
        let ai_suggestion_id =
            normalize_required_string("AI suggestion id", &command.ai_suggestion_id)?;
        let feedback_type = normalize_ai_feedback_type(&command.feedback_type)?;
        let feedback_text = command
            .feedback_text
            .as_ref().map(|value| trim(value))
            .filter(|value| !value.is_empty());
        if feedback_text
            .as_ref()
            .is_some_and(|value| value.chars().count() > 2000)
        {
            return Err(NotesProductError::Validation(
                "feedbackText must be at most 2000 characters".to_string(),
            ));
        }

        let suggestion = self
            .repository
            .find_ai_suggestion(&context, &ai_suggestion_id)
            .await?;
        let feedback_id = stable_ai_feedback_id(
            &context,
            &suggestion.id,
            &feedback_type,
            feedback_text.as_deref(),
        );

        match self
            .repository
            .find_ai_feedback(&context, &feedback_id)
            .await
        {
            Ok(existing) => return Ok(existing),
            Err(NotesProductError::NotFound(_)) => {}
            Err(error) => return Err(error),
        }

        match self
            .repository
            .insert_ai_feedback(NewAiFeedback {
                id: feedback_id.clone(),
                context: context.clone(),
                workspace_id: suggestion.workspace_id,
                job_id: suggestion.ai_job_id,
                suggestion_id: Some(suggestion.id),
                feedback_type,
                feedback_text,
            })
            .await
        {
            Ok(feedback) => Ok(feedback),
            Err(NotesProductError::Conflict(message)) => {
                match self
                    .repository
                    .find_ai_feedback(&context, &feedback_id)
                    .await
                {
                    Ok(existing) => Ok(existing),
                    Err(NotesProductError::NotFound(_)) => {
                        Err(NotesProductError::Conflict(message))
                    }
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error),
        }
    }

    pub async fn list_ai_suggestion_feedback(
        &self,
        query: ListAiSuggestionFeedbackQuery,
    ) -> Result<AiFeedbackPage, NotesProductError> {
        let context = normalize_actor_context(query.context)?;
        let ai_suggestion_id =
            normalize_required_string("AI suggestion id", &query.ai_suggestion_id)?;
        let page = normalize_page(query.page)?;
        let page_size = normalize_page_size(query.page_size)?;
        self.repository
            .find_ai_suggestion(&context, &ai_suggestion_id)
            .await?;

        let mut items = self
            .repository
            .list_ai_suggestion_feedback(&context, &ai_suggestion_id, page, page_size)
            .await?;
        let page_info = page_info_from_extra_item(&mut items, page, page_size);

        Ok(AiFeedbackPage { items, page_info })
    }

    async fn ai_job_sources_for_target(
        &self,
        context: &NotesActorContext,
        workspace_id: &str,
        target_type: &str,
        target_id: Option<&str>,
    ) -> Result<Vec<NewAiJobSource>, NotesProductError> {
        if target_type != "page" {
            return Ok(Vec::new());
        }

        let page_id = target_id.ok_or_else(|| {
            NotesProductError::Validation("targetId is required for this targetType".to_string())
        })?;
        let page = self.repository.find_page(context, page_id).await?;
        if page.workspace_id != workspace_id {
            return Err(NotesProductError::NotFound(
                "page not found in workspace".to_string(),
            ));
        }

        Ok(vec![NewAiJobSource {
            id: stable_ai_job_source_id(
                context,
                workspace_id,
                "page",
                &page.id,
                Some(page.current_drive_version_id.as_str()),
                Some(page.current_drive_version_no),
            ),
            source_type: "page".to_string(),
            source_id: Some(page.id.clone()),
            drive_node_id: Some(page.drive_node_id),
            drive_version_id: Some(page.current_drive_version_id),
            drive_version_no: Some(page.current_drive_version_no),
            permission_snapshot_hash: stable_permission_snapshot_hash(
                context,
                workspace_id,
                "page",
                &page.id,
            ),
        }])
    }

    async fn decide_ai_suggestion(
        &self,
        context: &NotesActorContext,
        ai_suggestion_id: &str,
        target_status: &str,
        conflicting_status: &str,
    ) -> Result<AiSuggestion, NotesProductError> {
        require_non_empty("AI suggestion id", ai_suggestion_id)?;
        let current = self
            .repository
            .find_ai_suggestion(context, ai_suggestion_id)
            .await?;
        match current.status.as_str() {
            "proposed" => {
                let updated = self
                    .repository
                    .decide_ai_suggestion(context, ai_suggestion_id, target_status)
                    .await?;
                if updated.status == target_status {
                    return Ok(updated);
                }
                if updated.status == conflicting_status {
                    return Err(NotesProductError::Conflict(
                        "AI suggestion decision is already terminal".to_string(),
                    ));
                }
                Err(NotesProductError::Conflict(format!(
                    "AI suggestion status cannot be decided from {}",
                    updated.status
                )))
            }
            status if status == target_status => Ok(current),
            status if status == conflicting_status => Err(NotesProductError::Conflict(
                "AI suggestion decision is already terminal".to_string(),
            )),
            other => Err(NotesProductError::Conflict(format!(
                "AI suggestion status cannot be decided from {other}"
            ))),
        }
    }
}

fn require_non_empty(field: &str, value: &str) -> Result<(), NotesProductError> {
    if is_blank(Some(value)) {
        return Err(NotesProductError::Validation(format!(
            "{field} is required"
        )));
    }
    Ok(())
}

fn remote_apply_conflict_result(
    task_id: String,
    remote_cursor: Option<String>,
    code: &str,
    message: &str,
) -> RemoteApplyPageResult {
    RemoteApplyPageResult {
        outcome: "conflict".to_string(),
        task_id,
        remote_cursor,
        applied_at: None,
        conflict: Some(RemoteApplyConflict {
            code: code.to_string(),
            message: message.to_string(),
            occurred_at: chrono_like_timestamp(),
        }),
    }
}

fn chrono_like_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}Z", duration.as_secs(), duration.subsec_millis())
}

fn normalize_required_string(field: &str, value: &str) -> Result<String, NotesProductError> {
    require_non_empty(field, value)?;
    Ok(trim(value))
}

fn normalize_actor_context(
    context: NotesActorContext,
) -> Result<NotesActorContext, NotesProductError> {
    Ok(NotesActorContext {
        tenant_id: normalize_required_string("tenant id", &context.tenant_id)?,
        organization_id: normalize_required_string("organization id", &context.organization_id)?,
        operator_id: normalize_required_string("operator id", &context.operator_id)?,
    })
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| trim(&value))
        .filter(|value| !is_blank(Some(value.as_str())))
}

fn normalize_optional_query<'a>(
    field: &str,
    value: Option<&'a str>,
) -> Result<Option<&'a str>, NotesProductError> {
    let value = value.map(str::trim).filter(|value| !is_blank(Some(value)));
    if let Some(value) = value {
        validate_max_chars(field, value, 200)?;
    }
    Ok(value)
}

fn validate_max_chars(field: &str, value: &str, max_chars: usize) -> Result<(), NotesProductError> {
    if value.chars().count() > max_chars {
        return Err(NotesProductError::Validation(format!(
            "{field} must be at most {max_chars} characters"
        )));
    }
    Ok(())
}

fn validate_page_content_metadata(
    content_type: &str,
    content_schema_version: &str,
) -> Result<(), NotesProductError> {
    validate_max_chars("content type", content_type, 255)?;
    validate_max_chars("content schema version", content_schema_version, 32)?;
    Ok(())
}

fn normalize_owner_subject_type(value: &str) -> Result<String, NotesProductError> {
    let value = normalize_required_string("owner subject type", value)?;
    if matches!(value.as_str(), "user" | "group" | "organization" | "app") {
        return Ok(value);
    }
    Err(NotesProductError::Validation(
        "ownerSubjectType is invalid".to_string(),
    ))
}

fn normalize_ai_job_type(value: &str) -> Result<String, NotesProductError> {
    let value = normalize_required_string("job type", value)?;
    if matches!(
        value.as_str(),
        "summarize" | "rewrite" | "extract_tasks" | "answer" | "organize" | "generate"
    ) {
        return Ok(value);
    }
    Err(NotesProductError::Validation(
        "jobType is invalid".to_string(),
    ))
}

fn normalize_ai_target_type(value: &str) -> Result<String, NotesProductError> {
    let value = normalize_required_string("target type", value)?;
    if matches!(
        value.as_str(),
        "page" | "collection" | "workspace" | "selection"
    ) {
        return Ok(value);
    }
    Err(NotesProductError::Validation(
        "targetType is invalid".to_string(),
    ))
}

fn normalize_ai_suggestion_type(value: &str) -> Result<String, NotesProductError> {
    let value = normalize_required_string("suggestion type", value)?;
    if matches!(
        value.as_str(),
        "summary" | "rewrite" | "tag" | "property_update" | "link_create" | "task_create"
    ) {
        return Ok(value);
    }
    Err(NotesProductError::Validation(
        "suggestionType is invalid".to_string(),
    ))
}

fn normalize_ai_feedback_type(value: &str) -> Result<String, NotesProductError> {
    let value = normalize_required_string("feedback type", value)?;
    if matches!(
        value.as_str(),
        "accepted" | "rejected" | "edited" | "helpful" | "not_helpful"
    ) {
        return Ok(value);
    }
    Err(NotesProductError::Validation(
        "feedbackType is invalid".to_string(),
    ))
}

fn optional_payload_string(
    payload: &serde_json::Value,
    field: &str,
) -> Result<Option<String>, NotesProductError> {
    let Some(value) = payload.get(field) else {
        return Ok(None);
    };
    let Some(value) = value.as_str() else {
        return Err(NotesProductError::Validation(format!(
            "suggestion payload {field} must be a string"
        )));
    };
    let value = trim(value);
    if is_blank(Some(value.as_str())) {
        return Ok(None);
    }
    Ok(Some(value))
}

fn validate_ai_target(target_type: &str, target_id: Option<&str>) -> Result<(), NotesProductError> {
    match target_type {
        "page" | "collection" if target_id.is_none() => Err(NotesProductError::Validation(
            "targetId is required for this targetType".to_string(),
        )),
        _ => Ok(()),
    }
}

fn validate_drive_snapshot(
    context: &str,
    snapshot: &crate::domain::DrivePageContentSnapshot,
) -> Result<(), NotesProductError> {
    for (field, value) in [
        ("driveSpaceId", snapshot.drive_space_id.as_str()),
        ("driveNodeId", snapshot.drive_node_id.as_str()),
        ("driveUri", snapshot.drive_uri.as_str()),
        ("driveVersionId", snapshot.drive_version_id.as_str()),
        ("contentType", snapshot.content_type.as_str()),
        (
            "contentSchemaVersion",
            snapshot.content_schema_version.as_str(),
        ),
    ] {
        if is_blank(Some(value)) {
            return Err(NotesProductError::Internal(format!(
                "Drive snapshot from {context} returned blank {field}"
            )));
        }
    }
    if snapshot.drive_version_no < 1 {
        return Err(NotesProductError::Internal(format!(
            "Drive snapshot from {context} returned invalid driveVersionNo"
        )));
    }
    if snapshot.content_type.chars().count() > 255 {
        return Err(NotesProductError::Internal(format!(
            "Drive snapshot from {context} returned oversized contentType"
        )));
    }
    if snapshot.content_schema_version.chars().count() > 32 {
        return Err(NotesProductError::Internal(format!(
            "Drive snapshot from {context} returned oversized contentSchemaVersion"
        )));
    }
    if !snapshot.content.is_object() {
        return Err(NotesProductError::Internal(format!(
            "Drive snapshot from {context} returned non-object page content"
        )));
    }
    let expected_drive_uri = format!(
        "drive://spaces/{}/nodes/{}",
        snapshot.drive_space_id, snapshot.drive_node_id
    );
    if snapshot.drive_uri != expected_drive_uri {
        return Err(NotesProductError::Internal(format!(
            "Drive snapshot from {context} returned inconsistent driveUri"
        )));
    }
    Ok(())
}

fn validate_drive_snapshot_matches_expected_refs(
    context: &str,
    snapshot: &crate::domain::DrivePageContentSnapshot,
    expected_drive_space_id: Option<&str>,
    expected_drive_node_id: Option<&str>,
    expected_drive_uri: Option<&str>,
) -> Result<(), NotesProductError> {
    if let Some(expected_drive_space_id) = expected_drive_space_id {
        if snapshot.drive_space_id != expected_drive_space_id {
            return Err(NotesProductError::Internal(format!(
                "Drive snapshot from {context} returned unexpected driveSpaceId"
            )));
        }
    }
    if let Some(expected_drive_node_id) = expected_drive_node_id {
        if snapshot.drive_node_id != expected_drive_node_id {
            return Err(NotesProductError::Internal(format!(
                "Drive snapshot from {context} returned unexpected driveNodeId"
            )));
        }
    }
    if let Some(expected_drive_uri) = expected_drive_uri {
        if snapshot.drive_uri != expected_drive_uri {
            return Err(NotesProductError::Internal(format!(
                "Drive snapshot from {context} returned unexpected driveUri"
            )));
        }
    }
    Ok(())
}

fn validate_drive_snapshot_matches_expected_version(
    context: &str,
    snapshot: &crate::domain::DrivePageContentSnapshot,
    expected_drive_version_id: &str,
    expected_drive_version_no: i64,
) -> Result<(), NotesProductError> {
    if snapshot.drive_version_id != expected_drive_version_id {
        return Err(NotesProductError::Internal(format!(
            "Drive snapshot from {context} returned unexpected driveVersionId"
        )));
    }
    if snapshot.drive_version_no != expected_drive_version_no {
        return Err(NotesProductError::Internal(format!(
            "Drive snapshot from {context} returned unexpected driveVersionNo"
        )));
    }
    Ok(())
}

fn validate_drive_snapshot_matches_expected_content_metadata(
    context: &str,
    snapshot: &crate::domain::DrivePageContentSnapshot,
    expected_content_type: &str,
    expected_content_schema_version: &str,
) -> Result<(), NotesProductError> {
    if snapshot.content_type != expected_content_type {
        return Err(NotesProductError::Internal(format!(
            "Drive snapshot from {context} returned unexpected contentType"
        )));
    }
    if snapshot.content_schema_version != expected_content_schema_version {
        return Err(NotesProductError::Internal(format!(
            "Drive snapshot from {context} returned unexpected contentSchemaVersion"
        )));
    }
    Ok(())
}

fn validate_drive_snapshot_advances_expected_version(
    context: &str,
    snapshot: &crate::domain::DrivePageContentSnapshot,
    current_drive_version_id: &str,
    current_drive_version_no: i64,
) -> Result<(), NotesProductError> {
    if snapshot.drive_version_id == current_drive_version_id {
        return Err(NotesProductError::Internal(format!(
            "Drive snapshot from {context} did not advance driveVersionId"
        )));
    }
    if snapshot.drive_version_no <= current_drive_version_no {
        return Err(NotesProductError::Internal(format!(
            "Drive snapshot from {context} did not advance driveVersionNo"
        )));
    }
    Ok(())
}

fn validate_drive_version_page(
    context: &str,
    page: &DriveVersionPage,
    expected_page: i64,
    expected_page_size: i64,
) -> Result<(), NotesProductError> {
    if page.page_info.page != expected_page {
        return Err(NotesProductError::Internal(format!(
            "Drive version page from {context} returned unexpected page"
        )));
    }
    if page.page_info.page_size != expected_page_size {
        return Err(NotesProductError::Internal(format!(
            "Drive version page from {context} returned unexpected pageSize"
        )));
    }
    if page.items.len() > expected_page_size as usize {
        return Err(NotesProductError::Internal(format!(
            "Drive version page from {context} returned too many items"
        )));
    }
    if page
        .page_info
        .next_cursor
        .as_ref()
        .is_some_and(|cursor| is_blank(Some(cursor.as_str())))
    {
        return Err(NotesProductError::Internal(format!(
            "Drive version page from {context} returned blank nextCursor"
        )));
    }
    for version in &page.items {
        if is_blank(Some(version.drive_version_id.as_str())) {
            return Err(NotesProductError::Internal(format!(
                "Drive version page from {context} returned blank driveVersionId"
            )));
        }
        if version.drive_version_no < 1 {
            return Err(NotesProductError::Internal(format!(
                "Drive version page from {context} returned invalid driveVersionNo"
            )));
        }
        if is_blank(Some(version.version_kind.as_str())) {
            return Err(NotesProductError::Internal(format!(
                "Drive version page from {context} returned blank versionKind"
            )));
        }
        if is_blank(Some(version.created_at.as_str())) {
            return Err(NotesProductError::Internal(format!(
                "Drive version page from {context} returned blank createdAt"
            )));
        }
    }
    Ok(())
}

fn notes_persistence_failed_after_drive_write(
    drive_outcome: &str,
    notes_outcome: &str,
    error: NotesProductError,
) -> NotesProductError {
    NotesProductError::Internal(format!(
        "{drive_outcome}, but {notes_outcome}; reconciliation is required: {error}"
    ))
}

fn ai_job_payload_hash(
    workspace_id: &str,
    job_type: &str,
    target_type: &str,
    target_id: Option<&str>,
    prompt: Option<&str>,
    context_policy: &serde_json::Value,
) -> Result<String, NotesProductError> {
    let payload = serde_json::json!({
        "workspaceId": workspace_id,
        "jobType": job_type,
        "targetType": target_type,
        "targetId": target_id,
        "prompt": prompt,
        "contextPolicy": context_policy,
    });
    let serialized = serde_json::to_string(&payload).map_err(|error| {
        NotesProductError::Internal(format!("serialize AI job payload failed: {error}"))
    })?;
    Ok(format!("fnv1a64:{:016x}", fnv1a64(serialized.as_bytes())))
}

fn stable_ai_job_id(
    context: &NotesActorContext,
    idempotency_key: &str,
    request_payload_hash: &str,
) -> String {
    let raw = format!(
        "{}:{}:{}:{}:{}",
        context.tenant_id,
        context.organization_id,
        context.operator_id,
        idempotency_key,
        request_payload_hash
    );
    format!("ai-job-{:016x}", fnv1a64(raw.as_bytes()))
}

fn stable_ai_job_source_id(
    context: &NotesActorContext,
    workspace_id: &str,
    source_type: &str,
    source_id: &str,
    drive_version_id: Option<&str>,
    drive_version_no: Option<i64>,
) -> String {
    let raw = format!(
        "{}:{}:{}:{}:{}:{}:{}",
        context.tenant_id,
        context.organization_id,
        workspace_id,
        source_type,
        source_id,
        drive_version_id.unwrap_or(""),
        drive_version_no.unwrap_or_default()
    );
    format!("ai-job-source-{:016x}", fnv1a64(raw.as_bytes()))
}

fn stable_ai_suggestion_id(
    context: &NotesActorContext,
    ai_job_id: &str,
    page_id: &str,
    suggestion_type: &str,
    index: usize,
    payload_hash: &str,
) -> String {
    let raw = format!(
        "{}:{}:{}:{}:{}:{}:{}:{}",
        context.tenant_id,
        context.organization_id,
        context.operator_id,
        ai_job_id,
        page_id,
        suggestion_type,
        index,
        payload_hash
    );
    format!("ai-suggestion-{:016x}", fnv1a64(raw.as_bytes()))
}

fn stable_ai_feedback_id(
    context: &NotesActorContext,
    ai_suggestion_id: &str,
    feedback_type: &str,
    feedback_text: Option<&str>,
) -> String {
    let raw = format!(
        "{}:{}:{}:{}:{}:{}",
        context.tenant_id,
        context.organization_id,
        context.operator_id,
        ai_suggestion_id,
        feedback_type,
        feedback_text.unwrap_or("")
    );
    format!("ai-feedback-{:016x}", fnv1a64(raw.as_bytes()))
}

fn page_id_from_ai_job(job: &AiJob, sources: &[AiJobSource]) -> Option<String> {
    if job.target_type == "page" {
        if let Some(target_id) = job.target_id.clone() {
            return Some(target_id);
        }
    }

    sources
        .iter()
        .find(|source| source.source_type == "page")
        .and_then(|source| source.source_id.clone())
}

fn source_for_page<'a>(sources: &'a [AiJobSource], page_id: &str) -> Option<&'a AiJobSource> {
    sources
        .iter()
        .find(|source| source.source_type == "page" && source.source_id.as_deref() == Some(page_id))
}

fn has_page_sources(sources: &[AiJobSource]) -> bool {
    sources.iter().any(|source| source.source_type == "page")
}

fn stable_permission_snapshot_hash(
    context: &NotesActorContext,
    workspace_id: &str,
    source_type: &str,
    source_id: &str,
) -> String {
    let raw = format!(
        "{}:{}:{}:{}:{}",
        context.tenant_id, context.organization_id, workspace_id, source_type, source_id
    );
    format!("permission:fnv1a64:{:016x}", fnv1a64(raw.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn normalize_page(page: i64) -> Result<i64, NotesProductError> {
    if page < 1 {
        return Err(NotesProductError::Validation(
            "page must be greater than or equal to 1".to_string(),
        ));
    }
    Ok(page)
}

fn normalize_page_size(page_size: i64) -> Result<i64, NotesProductError> {
    if !(1..=200).contains(&page_size) {
        return Err(NotesProductError::Validation(
            "page_size must be between 1 and 200".to_string(),
        ));
    }
    Ok(page_size)
}

fn page_info_from_extra_item<T>(items: &mut Vec<T>, page: i64, page_size: i64) -> PageInfo {
    let has_more = items.len() > page_size as usize;
    if has_more {
        items.truncate(page_size as usize);
    }

    PageInfo {
        page,
        page_size,
        has_more,
        next_cursor: None,
    }
}

fn built_in_object_types() -> Vec<crate::domain::ObjectTypeSummary> {
    [
        ("doc", "Document"),
        ("article", "Article"),
        ("code", "Code"),
        ("log", "Log"),
        ("database", "Database"),
        ("canvas", "Canvas"),
    ]
    .into_iter()
    .map(|(code, name)| crate::domain::ObjectTypeSummary {
        id: format!("notes-object-type-{code}"),
        code: code.to_string(),
        name: name.to_string(),
    })
    .collect()
}

fn workspace_change_token(workspace: &Workspace, root_pages: &[PageSummary]) -> String {
    let latest_page_update = root_pages
        .iter()
        .map(|page| page.updated_at.as_str())
        .max()
        .unwrap_or(workspace.updated_at.as_str());
    format!(
        "{}:{}:{}",
        workspace.id, workspace.version, latest_page_update
    )
}

fn search_result_from_page(page: Page, q: Option<&str>) -> SearchResult {
    let source_drive_version_no = page.current_drive_version_no.to_string();
    let source_drive_version_id = Some(page.current_drive_version_id.clone());
    let highlights = search_highlights(&page, q);

    SearchResult {
        page: PageSummary::from(page),
        highlights,
        source_drive_version_id,
        source_drive_version_no,
    }
}

fn search_highlights(page: &Page, q: Option<&str>) -> Vec<String> {
    let Some(q) = q else {
        return Vec::new();
    };
    let q = q.to_lowercase();
    if page.title.to_lowercase().contains(&q) {
        return vec![page.title.clone()];
    }
    if let Some(snippet) = page.snippet.as_ref() {
        if snippet.to_lowercase().contains(&q) {
            return vec![snippet.clone()];
        }
    }
    Vec::new()
}
