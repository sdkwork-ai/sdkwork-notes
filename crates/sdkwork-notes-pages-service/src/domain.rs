use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Verified caller dual credentials (`APP_SDK_INTEGRATION_SPEC.md` §5.2.1).
pub use sdkwork_utils_rust::caller_identity::CallerCredentials;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotesActorContext {
    pub tenant_id: String,
    pub organization_id: String,
    pub operator_id: String,
    /// The authenticated caller's dual tokens, forwarded to every outbound
    /// user-scoped dependency call. Empty on a service-identity path.
    pub caller: CallerCredentials,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageKind {
    Doc,
    Article,
    Code,
    Log,
    Database,
    Canvas,
    Folder,
}

impl PageKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Doc => "doc",
            Self::Article => "article",
            Self::Code => "code",
            Self::Log => "log",
            Self::Database => "database",
            Self::Canvas => "canvas",
            Self::Folder => "folder",
        }
    }

    pub fn try_from_str(value: &str) -> Option<Self> {
        match value {
            "doc" => Some(Self::Doc),
            "article" => Some(Self::Article),
            "code" => Some(Self::Code),
            "log" => Some(Self::Log),
            "database" => Some(Self::Database),
            "canvas" => Some(Self::Canvas),
            "folder" => Some(Self::Folder),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub tenant_id: String,
    pub organization_id: String,
    pub owner_subject_type: String,
    pub owner_subject_id: String,
    pub name: String,
    pub description: Option<String>,
    pub drive_space_id: String,
    pub default_page_content_type: String,
    pub default_page_schema_version: String,
    pub ai_index_policy_code: String,
    pub lifecycle_status: String,
    pub version: i64,
    pub created_by: String,
    pub updated_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub id: String,
    pub tenant_id: String,
    pub organization_id: String,
    pub workspace_id: String,
    pub title: String,
    pub page_kind: PageKind,
    pub parent_page_id: Option<String>,
    pub folder_drive_node_id: Option<String>,
    pub drive_space_id: String,
    pub drive_node_id: String,
    pub drive_uri: String,
    pub current_drive_version_id: String,
    pub current_drive_version_no: i64,
    pub content_type: String,
    pub content_schema_version: String,
    pub content_hash: Option<String>,
    pub snippet: Option<String>,
    pub icon: Option<String>,
    pub cover_asset_id: Option<String>,
    pub favorite: bool,
    pub archive_status: String,
    pub publish_status: String,
    pub word_count: i64,
    pub task_count: i64,
    pub drive_lifecycle_status_snapshot: Option<String>,
    pub lifecycle_status: String,
    pub version: i64,
    pub created_by: String,
    pub updated_by: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageContent {
    pub page_id: String,
    pub drive_node_id: String,
    pub drive_version_id: String,
    pub drive_version_no: i64,
    pub content_type: String,
    pub content_schema_version: String,
    pub content: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageSummary {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub snippet: Option<String>,
    pub page_kind: PageKind,
    pub drive_node_id: String,
    pub current_drive_version_no: String,
    pub favorite: bool,
    pub updated_at: String,
}

impl From<Page> for PageSummary {
    fn from(page: Page) -> Self {
        Self {
            id: page.id,
            workspace_id: page.workspace_id,
            title: page.title,
            snippet: page.snippet,
            page_kind: page.page_kind,
            drive_node_id: page.drive_node_id,
            current_drive_version_no: page.current_drive_version_no.to_string(),
            favorite: page.favorite,
            updated_at: page.updated_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageInfo {
    pub page: i64,
    pub page_size: i64,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveVersionSummary {
    pub drive_version_id: String,
    pub drive_version_no: i64,
    pub version_kind: String,
    pub version_label: Option<String>,
    pub change_summary: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePage {
    pub items: Vec<Workspace>,
    pub page_info: PageInfo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageSummaryPage {
    pub items: Vec<PageSummary>,
    pub page_info: PageInfo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveVersionPage {
    pub items: Vec<DriveVersionSummary>,
    pub page_info: PageInfo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub page: PageSummary,
    pub highlights: Vec<String>,
    pub source_drive_version_id: Option<String>,
    pub source_drive_version_no: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResultPage {
    pub items: Vec<SearchResult>,
    pub page_info: PageInfo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiJobPage {
    pub items: Vec<AiJob>,
    pub page_info: PageInfo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiJob {
    pub id: String,
    pub tenant_id: String,
    pub organization_id: String,
    pub workspace_id: String,
    pub job_type: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub status: String,
    pub result: Option<Value>,
    pub source_count: i64,
    pub suggestion_count: i64,
    pub idempotency_key: String,
    pub request_payload_hash: String,
    pub created_by: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiSuggestionPage {
    pub items: Vec<AiSuggestion>,
    pub page_info: PageInfo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiFeedbackPage {
    pub items: Vec<AiFeedback>,
    pub page_info: PageInfo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiSuggestion {
    pub id: String,
    pub tenant_id: String,
    pub organization_id: String,
    pub workspace_id: String,
    pub page_id: String,
    pub ai_job_id: String,
    pub suggestion_type: String,
    pub status: String,
    pub source_drive_node_id: Option<String>,
    pub source_drive_version_id: Option<String>,
    pub source_drive_version_no: Option<i64>,
    pub payload: Value,
    pub created_by: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiFeedback {
    pub id: String,
    pub tenant_id: String,
    pub organization_id: String,
    pub workspace_id: String,
    pub job_id: String,
    pub suggestion_id: Option<String>,
    pub feedback_type: String,
    pub feedback_text: Option<String>,
    pub created_by: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiJobSource {
    pub id: String,
    pub tenant_id: String,
    pub organization_id: String,
    pub workspace_id: String,
    pub job_id: String,
    pub source_type: String,
    pub source_id: Option<String>,
    pub drive_node_id: Option<String>,
    pub drive_version_id: Option<String>,
    pub drive_version_no: Option<i64>,
    pub permission_snapshot_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectTypeSummary {
    pub id: String,
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceBootstrap {
    pub workspace: Workspace,
    pub root_pages: Vec<PageSummary>,
    pub object_types: Vec<ObjectTypeSummary>,
    pub change_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrivePageContentSnapshot {
    pub drive_space_id: String,
    pub drive_node_id: String,
    pub drive_uri: String,
    pub drive_version_id: String,
    pub drive_version_no: i64,
    pub content_type: String,
    pub content_schema_version: String,
    pub content_hash: Option<String>,
    pub snippet: Option<String>,
    pub word_count: i64,
    pub task_count: i64,
    pub content: Value,
}

#[derive(Debug, Clone)]
pub struct CreateWorkspaceCommand {
    pub id: String,
    pub context: NotesActorContext,
    pub owner_subject_type: String,
    pub owner_subject_id: String,
    pub name: String,
    pub description: Option<String>,
    pub drive_space_id: String,
    pub default_page_content_type: String,
    pub default_page_schema_version: String,
    pub ai_index_policy_code: String,
}

#[derive(Debug, Clone)]
pub struct CreatePageCommand {
    pub id: String,
    pub context: NotesActorContext,
    pub workspace_id: String,
    pub title: String,
    pub page_kind: PageKind,
    pub parent_page_id: Option<String>,
    pub folder_drive_node_id: Option<String>,
    pub initial_content: Value,
    pub content_type: String,
    pub content_schema_version: String,
    pub change_summary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdatePageContentCommand {
    pub context: NotesActorContext,
    pub page_id: String,
    pub content: Value,
    pub content_type: Option<String>,
    pub content_schema_version: Option<String>,
    pub change_summary: Option<String>,
    pub expected_drive_version_id: Option<String>,
    pub create_checkpoint: bool,
}

#[derive(Debug, Clone)]
pub struct RestorePageVersionCommand {
    pub context: NotesActorContext,
    pub page_id: String,
    pub drive_version_id: String,
    pub expected_current_drive_version_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ListWorkspacesQuery {
    pub context: NotesActorContext,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone)]
pub struct ListPagesQuery {
    pub context: NotesActorContext,
    pub workspace_id: String,
    pub page: i64,
    pub page_size: i64,
    pub q: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ListPageVersionsQuery {
    pub context: NotesActorContext,
    pub page_id: String,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub context: NotesActorContext,
    pub workspace_id: Option<String>,
    pub q: Option<String>,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone)]
pub struct ListAiJobsQuery {
    pub context: NotesActorContext,
    pub workspace_id: Option<String>,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone)]
pub struct ListPageAiSuggestionsQuery {
    pub context: NotesActorContext,
    pub page_id: String,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone)]
pub struct ListAiSuggestionFeedbackQuery {
    pub context: NotesActorContext,
    pub ai_suggestion_id: String,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone)]
pub struct AcceptAiSuggestionCommand {
    pub context: NotesActorContext,
    pub ai_suggestion_id: String,
}

#[derive(Debug, Clone)]
pub struct RejectAiSuggestionCommand {
    pub context: NotesActorContext,
    pub ai_suggestion_id: String,
}

#[derive(Debug, Clone)]
pub struct ApplyAiSuggestionCommand {
    pub context: NotesActorContext,
    pub ai_suggestion_id: String,
    pub expected_drive_version_id: Option<String>,
    pub create_checkpoint: bool,
}

#[derive(Debug, Clone)]
pub struct CreateAiFeedbackCommand {
    pub context: NotesActorContext,
    pub ai_suggestion_id: String,
    pub feedback_type: String,
    pub feedback_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClaimAiJobCommand {
    pub context: NotesActorContext,
    pub ai_job_id: String,
}

#[derive(Debug, Clone)]
pub struct CompleteAiJobCommand {
    pub context: NotesActorContext,
    pub ai_job_id: String,
    pub suggestions: Vec<CompleteAiSuggestionInput>,
}

#[derive(Debug, Clone)]
pub struct FailAiJobCommand {
    pub context: NotesActorContext,
    pub ai_job_id: String,
    pub error_code: String,
    pub error_message: String,
}

#[derive(Debug, Clone)]
pub struct CompleteAiSuggestionInput {
    pub page_id: Option<String>,
    pub suggestion_type: String,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct CreateAiJobCommand {
    pub context: NotesActorContext,
    pub workspace_id: String,
    pub job_type: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub prompt: Option<String>,
    pub context_policy: Option<Value>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct UpdatePageMetadataCommand {
    pub context: NotesActorContext,
    pub page_id: String,
    pub title: Option<String>,
    pub favorite: Option<bool>,
    pub archive_status: Option<String>,
    pub publish_status: Option<String>,
    pub parent_page_id: Option<Option<String>>,
    pub expected_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PageMetadataPatch {
    pub title: String,
    pub favorite: bool,
    pub archive_status: String,
    pub publish_status: String,
    pub parent_page_id: Option<Option<String>>,
}

#[derive(Debug, Clone)]
pub struct RemoteApplyConflict {
    pub code: String,
    pub message: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone)]
pub struct RemoteApplyPageResult {
    pub outcome: String,
    pub task_id: String,
    pub remote_cursor: Option<String>,
    pub applied_at: Option<String>,
    pub conflict: Option<RemoteApplyConflict>,
}

#[derive(Debug, Clone)]
pub enum RemoteApplyMutation {
    UpsertPatch {
        title: Option<String>,
        content: Option<String>,
        parent_id: Option<Option<String>>,
        is_favorite: Option<bool>,
        publish_status: Option<String>,
    },
    Move {
        target_parent_id: Option<String>,
    },
    TrashIntent,
    RestoreIntent,
    PermanentDeleteIntent,
}

#[derive(Debug, Clone)]
pub struct RemoteApplyPageCommand {
    pub context: NotesActorContext,
    pub page_id: String,
    pub idempotency_key: String,
    pub task_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub operation: String,
    pub local_revision: Option<i64>,
    pub base_remote_cursor: Option<String>,
    pub mutation: RemoteApplyMutation,
}

#[derive(Debug, Clone)]
pub struct NewWorkspace {
    pub id: String,
    pub context: NotesActorContext,
    pub owner_subject_type: String,
    pub owner_subject_id: String,
    pub name: String,
    pub description: Option<String>,
    pub drive_space_id: String,
    pub default_page_content_type: String,
    pub default_page_schema_version: String,
    pub ai_index_policy_code: String,
}

#[derive(Debug, Clone)]
pub struct NewPage {
    pub id: String,
    pub context: NotesActorContext,
    pub workspace_id: String,
    pub title: String,
    pub page_kind: PageKind,
    pub parent_page_id: Option<String>,
    pub folder_drive_node_id: Option<String>,
    pub drive_snapshot: DrivePageContentSnapshot,
}

#[derive(Debug, Clone)]
pub struct NewAiJob {
    pub id: String,
    pub context: NotesActorContext,
    pub workspace_id: String,
    pub job_type: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub prompt_snapshot: Option<String>,
    pub context_policy_snapshot: Value,
    pub status: String,
    pub idempotency_key: String,
    pub request_payload_hash: String,
    pub sources: Vec<NewAiJobSource>,
}

#[derive(Debug, Clone)]
pub struct NewAiJobSource {
    pub id: String,
    pub source_type: String,
    pub source_id: Option<String>,
    pub drive_node_id: Option<String>,
    pub drive_version_id: Option<String>,
    pub drive_version_no: Option<i64>,
    pub permission_snapshot_hash: String,
}

#[derive(Debug, Clone)]
pub struct NewAiSuggestion {
    pub id: String,
    pub context: NotesActorContext,
    pub workspace_id: String,
    pub page_id: String,
    pub ai_job_id: String,
    pub suggestion_type: String,
    pub payload: Value,
    pub source_drive_node_id: Option<String>,
    pub source_drive_version_id: Option<String>,
    pub source_drive_version_no: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewAiFeedback {
    pub id: String,
    pub context: NotesActorContext,
    pub workspace_id: String,
    pub job_id: String,
    pub suggestion_id: Option<String>,
    pub feedback_type: String,
    pub feedback_text: Option<String>,
}
