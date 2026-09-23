use sdkwork_web_core::encode_unsigned_test_jwt;
use serde_json::json;

pub const TEST_TENANT_ID: &str = "100001";
pub const TEST_ORGANIZATION_ID: &str = "0";
pub const TEST_OPERATOR_ID: &str = "30";

pub fn test_auth_claim_token(tenant_id: &str, organization_id: &str, user_id: &str) -> String {
    encode_unsigned_test_jwt(json!({
        "tenant_id": tenant_id,
        "organization_id": organization_id,
        "user_id": user_id,
        "session_id": "session-test",
        "app_id": "appbase",
        "auth_level": "password",
        // token-claims-gate: legacy-fixture — constructs a pre-slimming credential so this
        // test can assert the claim is no longer an authorization source.
        "permission_scope": "notes.*,notes.backend.*"
    }))
}

pub fn test_access_claim_token(tenant_id: &str, organization_id: &str, user_id: &str) -> String {
    encode_unsigned_test_jwt(json!({
        "tenant_id": tenant_id,
        "organization_id": organization_id,
        "user_id": user_id,
        "session_id": "session-test",
        "app_id": "appbase",
        "environment": "prod",
        "deployment_mode": "saas"
    }))
}

pub fn default_test_auth_claim_token() -> String {
    test_auth_claim_token(TEST_TENANT_ID, TEST_ORGANIZATION_ID, TEST_OPERATOR_ID)
}

pub fn default_test_access_claim_token() -> String {
    test_access_claim_token(TEST_TENANT_ID, TEST_ORGANIZATION_ID, TEST_OPERATOR_ID)
}
