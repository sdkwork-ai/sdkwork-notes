use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use sdkwork_notes_pages_service::error::NotesProductError;
use sdkwork_utils_rust::{legacy_wire_result_code, SdkWorkApiResponse, SdkWorkResultCode};
use sdkwork_web_core::{
    problem_response, WebFrameworkError, WebFrameworkErrorKind, WebRequestContext,
};
use serde::Serialize;

pub type ApiResult<T> = Result<T, ApiProblem>;

#[derive(Debug, Clone)]
pub struct ApiProblem {
    pub message: String,
    status: StatusCode,
    code: SdkWorkResultCode,
}

impl ApiProblem {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: StatusCode::BAD_REQUEST,
            code: SdkWorkResultCode::ValidationError,
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: StatusCode::FORBIDDEN,
            code: SdkWorkResultCode::PermissionRequired,
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: StatusCode::NOT_FOUND,
            code: SdkWorkResultCode::NotFound,
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: StatusCode::CONFLICT,
            code: SdkWorkResultCode::Conflict,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: SdkWorkResultCode::InternalError,
        }
    }

    pub fn from_auth(problem: crate::actor_context::AuthProblem) -> Self {
        let code = match problem.code.as_str() {
            "notes.auth.missing_principal" => SdkWorkResultCode::AuthenticationRequired,
            "notes.auth.tenant_mismatch" => SdkWorkResultCode::TenantAccessDenied,
            "notes.auth.organization_mismatch" => SdkWorkResultCode::OrganizationAccessDenied,
            "notes.auth.operator_mismatch" => SdkWorkResultCode::InsufficientScope,
            "notes.permission_denied" => SdkWorkResultCode::PermissionRequired,
            _ => legacy_wire_result_code(&problem.code),
        };
        Self {
            message: problem.detail,
            status: problem.status,
            code,
        }
    }

    pub fn from_web_framework(error: sdkwork_web_core::WebFrameworkError) -> Self {
        let status = error.status();
        Self {
            message: error.message,
            status,
            code: match error.kind {
                WebFrameworkErrorKind::BadRequest => SdkWorkResultCode::ValidationError,
                WebFrameworkErrorKind::Forbidden => SdkWorkResultCode::PermissionRequired,
                WebFrameworkErrorKind::NotFound => SdkWorkResultCode::NotFound,
                WebFrameworkErrorKind::Conflict => SdkWorkResultCode::Conflict,
                WebFrameworkErrorKind::MissingCredentials => {
                    SdkWorkResultCode::AuthenticationRequired
                }
                WebFrameworkErrorKind::DependencyUnavailable => {
                    SdkWorkResultCode::ServiceUnavailable
                }
                _ => SdkWorkResultCode::InternalError,
            },
        }
    }

    fn framework_error(&self) -> WebFrameworkError {
        WebFrameworkError {
            kind: match self.code {
                SdkWorkResultCode::ValidationError
                | SdkWorkResultCode::MalformedRequest
                | SdkWorkResultCode::InvalidParameter
                | SdkWorkResultCode::MissingRequiredField => WebFrameworkErrorKind::BadRequest,
                SdkWorkResultCode::AuthenticationRequired
                | SdkWorkResultCode::TokenExpired
                | SdkWorkResultCode::InvalidToken
                | SdkWorkResultCode::SessionRevoked => WebFrameworkErrorKind::MissingCredentials,
                SdkWorkResultCode::PermissionRequired
                | SdkWorkResultCode::InsufficientScope
                | SdkWorkResultCode::TenantAccessDenied
                | SdkWorkResultCode::OrganizationAccessDenied => WebFrameworkErrorKind::Forbidden,
                SdkWorkResultCode::NotFound => WebFrameworkErrorKind::NotFound,
                SdkWorkResultCode::Conflict => WebFrameworkErrorKind::Conflict,
                SdkWorkResultCode::ServiceUnavailable => {
                    WebFrameworkErrorKind::DependencyUnavailable
                }
                _ => WebFrameworkErrorKind::InternalServerError,
            },
            message: self.message.clone(),
            retry_after_seconds: None,
            auth_profile: None,
            failed_stage: None,
            reason: None,
        }
    }

    pub fn into_response_for(&self, ctx: &WebRequestContext) -> Response {
        problem_response(&self.framework_error(), ctx.problem_correlation())
    }
}

pub fn map_product_error(error: NotesProductError) -> ApiProblem {
    match error {
        NotesProductError::Validation(detail) => ApiProblem::bad_request(detail),
        NotesProductError::Conflict(detail) => ApiProblem::conflict(detail),
        NotesProductError::NotFound(detail) => ApiProblem::not_found(detail),
        NotesProductError::PermissionDenied(detail) => ApiProblem::forbidden(detail),
        NotesProductError::Internal(_) => {
            ApiProblem::internal("An unexpected error occurred".to_string())
        }
    }
}

pub fn success_json<T: Serialize>(
    ctx: &WebRequestContext,
    data: T,
) -> Result<Response, ApiProblem> {
    success_response(ctx, StatusCode::OK, data)
}

pub fn created_json<T: Serialize>(
    ctx: &WebRequestContext,
    data: T,
) -> Result<Response, ApiProblem> {
    success_response(ctx, StatusCode::CREATED, data)
}

pub fn accepted_json<T: Serialize>(
    ctx: &WebRequestContext,
    data: T,
) -> Result<Response, ApiProblem> {
    success_response(ctx, StatusCode::ACCEPTED, data)
}

fn success_response<T: Serialize>(
    ctx: &WebRequestContext,
    status: StatusCode,
    data: T,
) -> Result<Response, ApiProblem> {
    let trace_id = ctx.resolved_trace_id();
    let envelope = SdkWorkApiResponse::success(data, trace_id.clone());
    let mut response = (status, Json(envelope)).into_response();
    attach_trace_header(&mut response, &trace_id);
    Ok(response)
}

fn attach_trace_header(response: &mut Response, trace_id: &str) {
    if let Ok(value) = HeaderValue::from_str(trace_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-sdkwork-trace-id"), value);
    }
}

pub fn finish_api_json<T: Serialize>(ctx: &WebRequestContext, result: ApiResult<T>) -> Response {
    match result {
        Ok(data) => {
            success_json(ctx, data).unwrap_or_else(|problem| problem.into_response_for(ctx))
        }
        Err(problem) => problem.into_response_for(ctx),
    }
}

pub fn finish_created_json<T: Serialize>(
    ctx: &WebRequestContext,
    result: ApiResult<T>,
) -> Response {
    match result {
        Ok(data) => {
            created_json(ctx, data).unwrap_or_else(|problem| problem.into_response_for(ctx))
        }
        Err(problem) => problem.into_response_for(ctx),
    }
}

pub fn finish_accepted_json<T: Serialize>(
    ctx: &WebRequestContext,
    result: ApiResult<T>,
) -> Response {
    match result {
        Ok(data) => {
            accepted_json(ctx, data).unwrap_or_else(|problem| problem.into_response_for(ctx))
        }
        Err(problem) => problem.into_response_for(ctx),
    }
}
