use crate::domain::{
    AiFeedback, AiJob, AiJobSource, AiSuggestion, DrivePageContentSnapshot, DriveVersionPage,
    NewAiFeedback, NewAiJob, NewAiSuggestion, NewPage, NewWorkspace, NotesActorContext, Page,
    PageMetadataPatch, Workspace,
};
use crate::error::NotesProductError;
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait NotesRepository: Clone + Send + Sync + 'static {
    async fn insert_workspace(
        &self,
        workspace: NewWorkspace,
    ) -> Result<Workspace, NotesProductError>;

    async fn find_workspace(
        &self,
        context: &NotesActorContext,
        workspace_id: &str,
    ) -> Result<Workspace, NotesProductError>;

    async fn list_workspaces(
        &self,
        context: &NotesActorContext,
        page: i64,
        page_size: i64,
    ) -> Result<Vec<Workspace>, NotesProductError>;

    async fn insert_page(&self, page: NewPage) -> Result<Page, NotesProductError>;

    async fn page_id_is_reserved(&self, page_id: &str) -> Result<bool, NotesProductError>;

    async fn find_page(
        &self,
        context: &NotesActorContext,
        page_id: &str,
    ) -> Result<Page, NotesProductError>;

    async fn list_pages(
        &self,
        context: &NotesActorContext,
        workspace_id: &str,
        page: i64,
        page_size: i64,
        q: Option<&str>,
    ) -> Result<Vec<Page>, NotesProductError>;

    async fn list_root_pages(
        &self,
        context: &NotesActorContext,
        workspace_id: &str,
        limit: i64,
    ) -> Result<Vec<Page>, NotesProductError>;

    async fn search_pages(
        &self,
        context: &NotesActorContext,
        workspace_id: Option<&str>,
        q: Option<&str>,
        page: i64,
        page_size: i64,
    ) -> Result<Vec<Page>, NotesProductError>;

    async fn update_page_metadata(
        &self,
        context: &NotesActorContext,
        page_id: &str,
        patch: &PageMetadataPatch,
        expected_version: Option<i64>,
    ) -> Result<Page, NotesProductError>;

    async fn delete_page(
        &self,
        context: &NotesActorContext,
        page_id: &str,
    ) -> Result<(), NotesProductError>;

    async fn update_page_drive_snapshot(
        &self,
        context: &NotesActorContext,
        page_id: &str,
        snapshot: &DrivePageContentSnapshot,
        expected_current_drive_version_id: &str,
    ) -> Result<Page, NotesProductError>;

    async fn find_ai_job_by_idempotency_key(
        &self,
        context: &NotesActorContext,
        idempotency_key: &str,
    ) -> Result<Option<AiJob>, NotesProductError>;

    async fn insert_ai_job(&self, job: NewAiJob) -> Result<AiJob, NotesProductError>;

    async fn list_ai_jobs(
        &self,
        context: &NotesActorContext,
        workspace_id: Option<&str>,
        page: i64,
        page_size: i64,
    ) -> Result<Vec<AiJob>, NotesProductError>;

    async fn find_ai_job(
        &self,
        context: &NotesActorContext,
        ai_job_id: &str,
    ) -> Result<AiJob, NotesProductError>;

    async fn cancel_ai_job(
        &self,
        context: &NotesActorContext,
        ai_job_id: &str,
    ) -> Result<AiJob, NotesProductError>;

    async fn claim_ai_job(
        &self,
        context: &NotesActorContext,
        ai_job_id: &str,
    ) -> Result<AiJob, NotesProductError>;

    async fn list_ai_job_sources(
        &self,
        context: &NotesActorContext,
        ai_job_id: &str,
    ) -> Result<Vec<AiJobSource>, NotesProductError>;

    async fn fail_ai_job(
        &self,
        context: &NotesActorContext,
        ai_job_id: &str,
        error_code: &str,
        error_message: &str,
    ) -> Result<AiJob, NotesProductError>;

    async fn complete_ai_job(
        &self,
        context: &NotesActorContext,
        ai_job_id: &str,
        suggestions: Vec<NewAiSuggestion>,
    ) -> Result<AiJob, NotesProductError>;

    async fn list_page_ai_suggestions(
        &self,
        context: &NotesActorContext,
        page_id: &str,
        page: i64,
        page_size: i64,
    ) -> Result<Vec<AiSuggestion>, NotesProductError>;

    async fn find_ai_suggestion(
        &self,
        context: &NotesActorContext,
        ai_suggestion_id: &str,
    ) -> Result<AiSuggestion, NotesProductError>;

    async fn decide_ai_suggestion(
        &self,
        context: &NotesActorContext,
        ai_suggestion_id: &str,
        status: &str,
    ) -> Result<AiSuggestion, NotesProductError>;

    async fn apply_ai_suggestion(
        &self,
        context: &NotesActorContext,
        ai_suggestion_id: &str,
        page_id: &str,
        snapshot: &DrivePageContentSnapshot,
        expected_current_drive_version_id: &str,
    ) -> Result<AiSuggestion, NotesProductError>;

    async fn find_ai_feedback(
        &self,
        context: &NotesActorContext,
        ai_feedback_id: &str,
    ) -> Result<AiFeedback, NotesProductError>;

    async fn insert_ai_feedback(
        &self,
        feedback: NewAiFeedback,
    ) -> Result<AiFeedback, NotesProductError>;

    async fn list_ai_suggestion_feedback(
        &self,
        context: &NotesActorContext,
        ai_suggestion_id: &str,
        page: i64,
        page_size: i64,
    ) -> Result<Vec<AiFeedback>, NotesProductError>;
}

#[async_trait]
pub trait DrivePageContentPort: Clone + Send + Sync + 'static {
    async fn create_page_content(
        &self,
        command: CreateDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, NotesProductError>;

    async fn update_page_content(
        &self,
        command: UpdateDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, NotesProductError>;

    async fn read_page_content(
        &self,
        command: ReadDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, NotesProductError>;

    async fn restore_page_content_version(
        &self,
        command: RestoreDrivePageContentVersionCommand,
    ) -> Result<DrivePageContentSnapshot, NotesProductError>;

    async fn list_page_content_versions(
        &self,
        command: ListDrivePageContentVersionsCommand,
    ) -> Result<DriveVersionPage, NotesProductError>;
}

#[derive(Debug, Clone)]
pub struct CreateDrivePageContentCommand {
    pub tenant_id: String,
    pub organization_id: String,
    pub operator_id: String,
    pub workspace_id: String,
    pub page_id: String,
    pub title: String,
    pub drive_space_id: String,
    pub parent_page_id: Option<String>,
    pub folder_drive_node_id: Option<String>,
    pub content: Value,
    pub content_type: String,
    pub content_schema_version: String,
    pub change_summary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateDrivePageContentCommand {
    pub tenant_id: String,
    pub organization_id: String,
    pub operator_id: String,
    pub workspace_id: String,
    pub page_id: String,
    pub drive_space_id: String,
    pub drive_node_id: String,
    pub drive_uri: String,
    pub current_drive_version_id: String,
    pub content: Value,
    pub content_type: String,
    pub content_schema_version: String,
    pub change_summary: Option<String>,
    pub expected_drive_version_id: Option<String>,
    pub create_checkpoint: bool,
}

#[derive(Debug, Clone)]
pub struct ReadDrivePageContentCommand {
    pub tenant_id: String,
    pub organization_id: String,
    /// The actor the page belongs to; the in-process Drive port authorizes the
    /// read with Drive's `reader` role for this subject (MODULE_SPEC.md §4.2).
    pub operator_id: String,
    pub page_id: String,
    pub drive_space_id: String,
    pub drive_node_id: String,
    pub drive_uri: String,
    pub current_drive_version_id: String,
    /// The page's recorded content type, echoed by the Drive port so the read
    /// snapshot keeps Notes' content metadata unchanged.
    pub content_type: String,
    /// The page's recorded content schema version (Notes metadata, not stored
    /// by Drive).
    pub content_schema_version: String,
}

#[derive(Debug, Clone)]
pub struct RestoreDrivePageContentVersionCommand {
    pub tenant_id: String,
    pub organization_id: String,
    pub operator_id: String,
    pub page_id: String,
    pub drive_space_id: String,
    pub drive_node_id: String,
    pub drive_uri: String,
    pub current_drive_version_id: String,
    pub drive_version_id: String,
    pub expected_current_drive_version_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ListDrivePageContentVersionsCommand {
    pub tenant_id: String,
    pub organization_id: String,
    /// The actor the page belongs to; the in-process Drive port authorizes the
    /// listing with Drive's `reader` role for this subject.
    pub operator_id: String,
    pub page_id: String,
    pub drive_space_id: String,
    pub drive_node_id: String,
    pub drive_uri: String,
    pub current_drive_version_id: String,
    pub page: i64,
    pub page_size: i64,
}
