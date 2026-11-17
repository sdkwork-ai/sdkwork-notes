use http::StatusCode;
use sdkwork_notes_pages_service::domain::NotesActorContext;
use sdkwork_utils_rust::string::trim;
use sdkwork_web_core::WebRequestContext;

#[derive(Debug, Clone)]
pub struct AuthProblem {
    pub status: StatusCode,
    pub code: String,
    pub detail: String,
}

impl AuthProblem {
    pub fn new(status: StatusCode, code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            detail: detail.into(),
        }
    }
}

pub fn actor_context_from_web_request(
    context: &WebRequestContext,
) -> Result<NotesActorContext, AuthProblem> {
    let principal = context.principal.as_ref().ok_or_else(|| {
        AuthProblem::new(
            StatusCode::UNAUTHORIZED,
            "notes.auth.missing_principal",
            "authenticated request context is required",
        )
    })?;

    let organization_id = principal
        .organization_id()
        .map(str::to_owned)
        .unwrap_or_else(|| "0".to_string());

    Ok(NotesActorContext {
        tenant_id: trim(principal.tenant_id()).to_owned(),
        organization_id: trim(&organization_id).to_owned(),
        operator_id: trim(principal.user_id()).to_owned(),
        // APP_SDK_INTEGRATION_SPEC §5.2.1: forward the caller's own dual
        // tokens to user-scoped dependency calls (Drive app-api).
        caller: sdkwork_web_core::current_caller_credentials(),
    })
}
