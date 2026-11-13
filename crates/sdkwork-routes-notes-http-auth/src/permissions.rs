use http::StatusCode;
use sdkwork_web_core::WebRequestContext;

use crate::actor_context::AuthProblem;

pub fn require_permission(
    context: &WebRequestContext,
    permission: &str,
) -> Result<(), AuthProblem> {
    let principal = context.principal.as_ref().ok_or_else(|| {
        AuthProblem::new(
            StatusCode::UNAUTHORIZED,
            "notes.auth.missing_principal",
            "authenticated request context is required",
        )
    })?;

    if sdkwork_web_core::request_context::permission_scope_matches_any(
        &principal.scopes.permission_scope,
        permission,
    ) {
        return Ok(());
    }

    Err(AuthProblem::new(
        StatusCode::FORBIDDEN,
        "notes.permission_denied",
        format!("permission `{permission}` is required"),
    ))
}
