//! Opt-in split-deployment Drive adapter (MODULE_SPEC.md §4.2.3).
//!
//! This module is **never** selected by default. Notes reaches Drive in-process
//! (`drive_in_process.rs`); a deployment that really runs Drive in another
//! process or container opts in with:
//!
//! ```text
//! SDKWORK_NOTES_DRIVE_MODE=http
//! SDKWORK_DRIVE_FACADE_URL=https://drive.sdkwork.com
//! ```
//!
//! The opt-in is validated at startup: [`HttpDrivePageContentPort::from_env`]
//! fails when neither the configured endpoint nor the process service registry
//! yields a Drive base URL, so a misspelled split deployment aborts the boot
//! instead of failing the first page save. The variable is recorded in
//! `specs/backend-module-call-boundary.allow.json` with its reason.

use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};

use sdkwork_drive_app_sdk_generated_rust::{
    CompleteUploadSessionRequest, CompletedUploadPart, CreateDownloadUrlRequest,
    CreateUploadSessionRequest, FileVersion, MarkUploaderPartUploadedRequest, NodeCommandRequest,
    PrepareUploaderUploadRequest, PresignUploadPartRequest, SdkworkAppClient, SdkworkError,
};
use sdkwork_notes_pages_service::domain::{
    DrivePageContentSnapshot, DriveVersionPage, DriveVersionSummary, PageInfo,
};
use sdkwork_notes_pages_service::error::NotesProductError;
use sdkwork_notes_pages_service::ports::{
    CreateDrivePageContentCommand, DrivePageContentPort, ListDrivePageContentVersionsCommand,
    ReadDrivePageContentCommand, RestoreDrivePageContentVersionCommand,
    UpdateDrivePageContentCommand,
};
use sdkwork_utils_rust::service_base_url::registry_base_url_or;
use sdkwork_utils_rust::string::{is_blank, trim};

use crate::service::drive_page_content_mapping::{
    build_snapshot, build_version_summary, PageSnapshotInput,
};

/// The only endpoint variable an HTTP-mode Notes deployment may read. Read here
/// (not in `drive_port.rs`) so the default path cannot reference it.
pub(crate) const DRIVE_FACADE_URL_ENV: &str = "SDKWORK_DRIVE_FACADE_URL";

const NOTES_RESOURCE_TYPE: &str = "notes_page";
const NOTES_UPLOAD_PROFILE: &str = "generic";
const NOTES_CONTENT_SCENE: &str = "notes_page_content";
const NOTES_CONTENT_SOURCE: &str = "notes_product_service";

/// Content schema version the replaced adapter reported after a restore (the
/// restore command carries no page metadata, and the value only reaches Notes'
/// post-restore snapshot check).
const RESTORE_CONTENT_SCHEMA_VERSION: &str = "notes/v1";

/// Split-deployment Drive adapter backed by the generated Drive App SDK.
#[derive(Clone)]
pub struct HttpDrivePageContentPort {
    client: SdkworkAppClient,
    http: Client,
}

impl HttpDrivePageContentPort {
    /// Builds the adapter for an explicit `SDKWORK_NOTES_DRIVE_MODE=http`
    /// deployment.
    ///
    /// # Errors
    ///
    /// Fails at startup when no Drive base URL is configured or discoverable.
    pub fn from_env() -> Result<Self, String> {
        let configured = std::env::var(DRIVE_FACADE_URL_ENV).ok().unwrap_or_default();
        // DISCOVERY_SPEC §12.1: registry-first, configured URL as degradation.
        let base_url = registry_base_url_or("sdkwork-drive", &trim(&configured));
        if is_blank(Some(base_url.as_str())) {
            return Err(format!(
                "SDKWORK_NOTES_DRIVE_MODE=http requires {DRIVE_FACADE_URL_ENV} (or a healthy \
                 sdkwork-drive entry in the process service registry); refusing to boot without \
                 a Drive endpoint"
            ));
        }
        let client = SdkworkAppClient::new_with_base_url(base_url)
            .map_err(|error| format!("initialize Drive SDK client failed: {error}"))?;
        if let Ok(token) = std::env::var("SDKWORK_ACCESS_TOKEN") {
            let trimmed = trim(&token);
            if !is_blank(Some(trimmed.as_str())) {
                client.set_access_token(trimmed.as_str());
            }
        }
        let http = Client::builder()
            .build()
            .map_err(|error| format!("initialize Drive upload HTTP client failed: {error}"))?;
        Ok(Self { client, http })
    }
}

#[async_trait]
impl DrivePageContentPort for HttpDrivePageContentPort {
    async fn create_page_content(
        &self,
        command: CreateDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, NotesProductError> {
        let body = serialize_page_content(&command.content)?;
        let (node_id, space_id, version) = self
            .create_node_and_upload(
                &command.tenant_id,
                &command.organization_id,
                &command.operator_id,
                &command.page_id,
                &command.title,
                &command.drive_space_id,
                command.folder_drive_node_id.as_deref(),
                &command.content_type,
                &body,
            )
            .await?;
        Ok(snapshot_from_version(
            &space_id,
            &node_id,
            &version,
            &command.content_type,
            &command.content_schema_version,
            command.content,
        ))
    }

    async fn update_page_content(
        &self,
        command: UpdateDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, NotesProductError> {
        if let Some(expected) = command.expected_drive_version_id.as_deref() {
            let current = self
                .latest_file_version(&command.drive_node_id, &command.tenant_id)
                .await?;
            if current.id != expected {
                return Err(NotesProductError::Conflict(
                    "page Drive version has changed".to_string(),
                ));
            }
        }
        let body = serialize_page_content(&command.content)?;
        let version = self
            .upload_new_version(
                &command.tenant_id,
                &command.operator_id,
                &command.drive_space_id,
                &command.drive_node_id,
                &command.page_id,
                &command.content_type,
                &body,
            )
            .await?;
        Ok(snapshot_from_version(
            &command.drive_space_id,
            &command.drive_node_id,
            &version,
            &command.content_type,
            &command.content_schema_version,
            command.content,
        ))
    }

    async fn read_page_content(
        &self,
        command: ReadDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, NotesProductError> {
        let version = self
            .resolve_version(
                &command.drive_node_id,
                &command.tenant_id,
                &command.current_drive_version_id,
            )
            .await?;
        let content = self
            .download_json_content(&command.tenant_id, &command.drive_node_id)
            .await?;
        Ok(snapshot_from_version(
            &command.drive_space_id,
            &command.drive_node_id,
            &version,
            &command.content_type,
            &command.content_schema_version,
            content,
        ))
    }

    async fn restore_page_content_version(
        &self,
        command: RestoreDrivePageContentVersionCommand,
    ) -> Result<DrivePageContentSnapshot, NotesProductError> {
        if let Some(expected) = command.expected_current_drive_version_id.as_deref() {
            let current = self
                .latest_file_version(&command.drive_node_id, &command.tenant_id)
                .await?;
            if current.id != expected {
                return Err(NotesProductError::Conflict(
                    "page Drive version has changed".to_string(),
                ));
            }
        }
        let before = self
            .latest_file_version(&command.drive_node_id, &command.tenant_id)
            .await?;
        self.client
            .drive()
            .versions_restore(
                &command.drive_node_id,
                &command.drive_version_id,
                &NodeCommandRequest::default(),
            )
            .await
            .map_err(map_drive_error("restore page content version"))?;
        let after = self
            .latest_file_version(&command.drive_node_id, &command.tenant_id)
            .await?;
        if after.version_no <= before.version_no {
            return Err(NotesProductError::Internal(
                "Drive restore did not advance node version".to_string(),
            ));
        }
        let content = self
            .download_json_content(&command.tenant_id, &command.drive_node_id)
            .await?;
        Ok(snapshot_from_version(
            &command.drive_space_id,
            &command.drive_node_id,
            &after,
            &after.content_type,
            RESTORE_CONTENT_SCHEMA_VERSION,
            content,
        ))
    }

    async fn list_page_content_versions(
        &self,
        command: ListDrivePageContentVersionsCommand,
    ) -> Result<DriveVersionPage, NotesProductError> {
        let mut items = self
            .list_all_versions(&command.drive_node_id, &command.tenant_id)
            .await?;
        items.sort_by(|left, right| right.version_no.cmp(&left.version_no));
        let start = ((command.page.max(1) - 1) * command.page_size.max(1)) as usize;
        let end = start.saturating_add(command.page_size.max(1) as usize);
        let page_items: Vec<DriveVersionSummary> = items
            .iter()
            .skip(start)
            .take(command.page_size.max(1) as usize)
            .map(|version| {
                build_version_summary(
                    &version.id,
                    version.version_no.parse::<i64>().unwrap_or(0),
                    &version.created_at,
                )
            })
            .collect();
        let has_more = end < items.len();
        Ok(DriveVersionPage {
            items: page_items,
            page_info: PageInfo {
                page: command.page,
                page_size: command.page_size,
                has_more,
                next_cursor: if has_more {
                    Some((command.page + 1).to_string())
                } else {
                    None
                },
            },
        })
    }
}

impl HttpDrivePageContentPort {
    async fn create_node_and_upload(
        &self,
        tenant_id: &str,
        _organization_id: &str,
        operator_id: &str,
        page_id: &str,
        title: &str,
        drive_space_id: &str,
        parent_node_id: Option<&str>,
        content_type: &str,
        body: &[u8],
    ) -> Result<(String, String, FileVersion), NotesProductError> {
        let epoch_ms = current_epoch_ms();
        let upload_item_id = format!("notes-upload-{page_id}-{epoch_ms}");
        let task_id = format!("notes-task-{page_id}-{epoch_ms}");
        let checksum = sha256_prefixed(body);
        let chunk_size = body.len().max(8) as i64;
        let prepared = self
            .client
            .drive()
            .uploader_uploads_create(&PrepareUploaderUploadRequest {
                id: upload_item_id.clone(),
                task_id,
                app_resource_type: NOTES_RESOURCE_TYPE.to_string(),
                app_resource_id: page_id.to_string(),
                upload_profile_code: Some(NOTES_UPLOAD_PROFILE.to_string()),
                file_fingerprint: checksum.clone(),
                original_file_name: format!("{title}.json"),
                content_type: content_type.to_string(),
                content_length: body.len().to_string(),
                chunk_size_bytes: chunk_size.to_string(),
                space_id: Some(drive_space_id.to_string()),
                parent_node_id: parent_node_id.map(str::to_string),
                retention: None,
                now_epoch_ms: Some(epoch_ms.to_string()),
                scene: Some(NOTES_CONTENT_SCENE.to_string()),
                source: Some(NOTES_CONTENT_SOURCE.to_string()),
                share_token: None,
            })
            .await
            .map_err(map_drive_error("prepare Drive uploader upload"))?;
        let upload_session_id = prepared.upload_session.id.clone();
        let node_id = prepared.upload_item.node_id.clone();
        let space_id = prepared.upload_item.space_id.clone();
        self.upload_single_part(
            tenant_id,
            operator_id,
            &upload_session_id,
            Some(&prepared.upload_session.storage_upload_id),
            Some(&prepared.upload_item.id),
            content_type,
            body,
            &checksum,
        )
        .await?;
        let version = self.latest_file_version(&node_id, tenant_id).await?;
        Ok((node_id, space_id, version))
    }

    async fn upload_new_version(
        &self,
        tenant_id: &str,
        operator_id: &str,
        drive_space_id: &str,
        drive_node_id: &str,
        page_id: &str,
        content_type: &str,
        body: &[u8],
    ) -> Result<FileVersion, NotesProductError> {
        let epoch_ms = current_epoch_ms();
        let session_id = format!("notes-session-{page_id}-{epoch_ms}");
        let checksum = sha256_prefixed(body);
        let session = self
            .client
            .drive()
            .upload_sessions_create(&CreateUploadSessionRequest {
                session_id,
                space_id: drive_space_id.to_string(),
                node_id: drive_node_id.to_string(),
                // Notes uploads keep the Drive binding resolution order
                // (bucket -> space -> space-type -> tenant); the contract field is
                // optional and an omitted preference is the previous behaviour.
                storage_provider_id: None,
                bucket: None,
                object_key: None,
                idempotency_key: format!("notes-content-{page_id}-{epoch_ms}"),
                expires_at_epoch_ms: (epoch_ms + 3_600_000).to_string(),
            })
            .await
            .map_err(map_drive_error("create Drive upload session"))?;
        self.upload_single_part(
            tenant_id,
            operator_id,
            &session.id,
            Some(&session.storage_upload_id),
            None,
            content_type,
            body,
            &checksum,
        )
        .await?;
        self.latest_file_version(drive_node_id, tenant_id).await
    }

    async fn upload_single_part(
        &self,
        _tenant_id: &str,
        _operator_id: &str,
        upload_session_id: &str,
        storage_upload_id: Option<&str>,
        upload_item_id: Option<&str>,
        content_type: &str,
        body: &[u8],
        checksum: &str,
    ) -> Result<(), NotesProductError> {
        let presigned = self
            .client
            .drive()
            .upload_sessions_parts_update(
                upload_session_id,
                1,
                &PresignUploadPartRequest {
                    upload_id: storage_upload_id.map(str::to_string),
                    requested_ttl_seconds: Some(900),
                },
            )
            .await
            .map_err(map_drive_error("presign Drive upload part"))?;
        let mut request = self
            .http
            .request(
                reqwest::Method::from_bytes(presigned.method.as_bytes())
                    .unwrap_or(reqwest::Method::PUT),
                &presigned.upload_url,
            )
            .body(body.to_vec());
        for (key, value) in presigned.headers {
            request = request.header(key, value);
        }
        let response = request.send().await.map_err(|error| {
            NotesProductError::Internal(format!("upload Drive object bytes failed: {error}"))
        })?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(NotesProductError::Internal(format!(
                "upload Drive object bytes failed with status {status}: {body}"
            )));
        }
        let etag = response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(|value| value.trim_matches('"').to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "\"uploaded\"".to_string());
        if let Some(upload_item_id) = upload_item_id {
            self.client
                .drive()
                .uploader_uploads_parts_update(
                    upload_item_id,
                    1,
                    &MarkUploaderPartUploadedRequest {
                        upload_session_id: upload_session_id.to_string(),
                        offset_bytes: "0".to_string(),
                        size_bytes: body.len().to_string(),
                        etag: etag.clone(),
                        checksum_sha256_hex: Some(
                            checksum.strip_prefix("sha256:").unwrap_or(checksum).to_string(),
                        ),
                        uploaded_at_epoch_ms: Some(current_epoch_ms().to_string()),
                    },
                )
                .await
                .map_err(map_drive_error("mark Drive upload part uploaded"))?;
        }
        self.client
            .drive()
            .upload_sessions_complete(
                upload_session_id,
                &CompleteUploadSessionRequest {
                    upload_id: storage_upload_id.map(str::to_string),
                    content_type: content_type.to_string(),
                    content_length: body.len().to_string(),
                    checksum_sha256_hex: checksum.to_string(),
                    parts: vec![CompletedUploadPart { part_no: 1, etag }],
                },
            )
            .await
            .map_err(map_drive_error("complete Drive upload session"))?;
        Ok(())
    }

    async fn latest_file_version(
        &self,
        node_id: &str,
        _tenant_id: &str,
    ) -> Result<FileVersion, NotesProductError> {
        let versions = self.list_all_versions(node_id, _tenant_id).await?;
        versions
            .into_iter()
            .max_by_key(|version| version.version_no.parse::<i64>().unwrap_or(0))
            .ok_or_else(|| NotesProductError::NotFound("Drive file version not found".to_string()))
    }

    async fn resolve_version(
        &self,
        node_id: &str,
        _tenant_id: &str,
        drive_version_id: &str,
    ) -> Result<FileVersion, NotesProductError> {
        let response = self
            .client
            .drive()
            .versions_list(node_id, Some("100"), None)
            .await
            .map_err(map_drive_error("list Drive file versions"))?;
        response
            .items
            .into_iter()
            .find(|version| version.id == drive_version_id)
            .ok_or_else(|| {
                NotesProductError::NotFound("Drive file version not found".to_string())
            })
    }

    async fn list_all_versions(
        &self,
        node_id: &str,
        _tenant_id: &str,
    ) -> Result<Vec<FileVersion>, NotesProductError> {
        let mut items = Vec::new();
        let mut page_token: Option<String> = None;
        loop {
            let response = self
                .client
                .drive()
                .versions_list(node_id, Some("100"), page_token.as_deref())
                .await
                .map_err(map_drive_error("list Drive file versions"))?;
            items.extend(response.items);
            page_token = response
                .page_info
                .next_cursor
                .filter(|token| !is_blank(Some(token.as_str())));
            if page_token.is_none() {
                break;
            }
        }
        Ok(items)
    }

    async fn download_json_content(
        &self,
        _tenant_id: &str,
        node_id: &str,
    ) -> Result<Value, NotesProductError> {
        let download = self
            .client
            .drive()
            .download_urls_create(&CreateDownloadUrlRequest {
                node_id: node_id.to_string(),
                requested_ttl_seconds: Some(900),
            })
            .await
            .map_err(map_drive_error("create Drive download url"))?;
        let response = self
            .http
            .get(&download.download_url)
            .send()
            .await
            .map_err(|error| {
                NotesProductError::Internal(format!("download Drive content failed: {error}"))
            })?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(NotesProductError::Internal(format!(
                "download Drive content failed with status {status}: {body}"
            )));
        }
        let bytes = response.bytes().await.map_err(|error| {
            NotesProductError::Internal(format!("read Drive content bytes failed: {error}"))
        })?;
        serde_json::from_slice(&bytes).map_err(|error| {
            NotesProductError::Internal(format!("parse Drive page content json failed: {error}"))
        })
    }
}

fn serialize_page_content(content: &Value) -> Result<Vec<u8>, NotesProductError> {
    serde_json::to_vec(content).map_err(|error| {
        NotesProductError::Internal(format!("serialize page content failed: {error}"))
    })
}

/// Build the snapshot the Notes page row stores from a Drive file version and
/// the page body that version holds.
fn snapshot_from_version(
    drive_space_id: &str,
    drive_node_id: &str,
    version: &FileVersion,
    content_type: &str,
    content_schema_version: &str,
    content: Value,
) -> DrivePageContentSnapshot {
    build_snapshot(PageSnapshotInput {
        drive_space_id,
        drive_node_id,
        drive_version_id: &version.id,
        drive_version_no: version.version_no.parse::<i64>().unwrap_or(0),
        content_type,
        content_schema_version,
        content_hash: Some(version.checksum_sha256_hex.clone()),
        content,
    })
}

fn sha256_prefixed(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    format!("sha256:{digest:x}")
}

fn current_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn map_drive_error(context: &'static str) -> impl Fn(SdkworkError) -> NotesProductError {
    move |error| NotesProductError::Internal(format!("{context}: {error}"))
}
