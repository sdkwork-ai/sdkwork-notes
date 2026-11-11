//! Shared Notes <-> Drive page-content mapping.
//!
//! Both Drive transports (the in-process port and the opt-in HTTP adapter) must
//! produce byte-identical [`DrivePageContentSnapshot`] and
//! [`DriveVersionSummary`] values, so the mapping lives in exactly one place
//! (`MODULE_SPEC.md` §4.2.7: no duplicated DTO mapping between the outbound and
//! the in-process transport).

use serde_json::Value;

use sdkwork_notes_pages_service::domain::{
    DrivePageContentSnapshot, DriveVersionSummary, PageInfo, DriveVersionPage,
};

/// Everything one page-content snapshot is assembled from.
pub(crate) struct PageSnapshotInput<'a> {
    pub drive_space_id: &'a str,
    pub drive_node_id: &'a str,
    pub drive_version_id: &'a str,
    pub drive_version_no: i64,
    pub content_type: &'a str,
    pub content_schema_version: &'a str,
    pub content_hash: Option<String>,
    pub content: Value,
}

/// Build the snapshot Notes stores on its page row.
pub(crate) fn build_snapshot(input: PageSnapshotInput<'_>) -> DrivePageContentSnapshot {
    let (snippet, word_count, task_count) = extract_content_metrics(&input.content);
    DrivePageContentSnapshot {
        drive_space_id: input.drive_space_id.to_string(),
        drive_node_id: input.drive_node_id.to_string(),
        drive_uri: drive_uri(input.drive_space_id, input.drive_node_id),
        drive_version_id: input.drive_version_id.to_string(),
        drive_version_no: input.drive_version_no,
        content_type: input.content_type.to_string(),
        content_schema_version: input.content_schema_version.to_string(),
        content_hash: input.content_hash,
        snippet,
        word_count,
        task_count,
        content: input.content,
    }
}

/// Map one Drive version row to the version summary the Notes API returns.
///
/// `version_kind` is derived from the version number exactly as the previous
/// Drive app-SDK adapter did, so the wire response does not change.
pub(crate) fn build_version_summary(
    drive_version_id: &str,
    drive_version_no: i64,
    created_at: &str,
) -> DriveVersionSummary {
    DriveVersionSummary {
        drive_version_id: drive_version_id.to_string(),
        drive_version_no,
        version_kind: if drive_version_no <= 1 {
            "initial".to_string()
        } else {
            "auto".to_string()
        },
        version_label: None,
        change_summary: None,
        created_at: created_at.to_string(),
    }
}

/// The page-content version page Notes returns, paginated in memory.
///
/// The replaced adapter listed every version and paginated the sorted list, so
/// the page shape (including `page`, `pageSize`, `hasMore` and `nextCursor`) is
/// preserved.
pub(crate) fn build_version_page(
    versions: &[sdkwork_drive_workspace_service::domain::node_version::DriveNodeVersion],
    page: i64,
    page_size: i64,
) -> DriveVersionPage {
    let start = ((page.max(1) - 1) * page_size.max(1)) as usize;
    let take = page_size.max(1) as usize;
    let items = versions
        .iter()
        .skip(start)
        .take(take)
        .map(|version| {
            build_version_summary(&version.id, version.version_no, &version.created_at)
        })
        .collect();
    let has_more = start.saturating_add(take) < versions.len();
    DriveVersionPage {
        items,
        page_info: PageInfo {
            page,
            page_size,
            has_more,
            next_cursor: if has_more {
                Some((page + 1).to_string())
            } else {
                None
            },
        },
    }
}

/// The page body's derived fields. Identical to the replaced adapter so the
/// Notes page summary keeps its previous values.
fn extract_content_metrics(content: &Value) -> (Option<String>, i64, i64) {
    let snippet = content
        .get("title")
        .or_else(|| content.get("snippet"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let word_count = content
        .get("wordCount")
        .or_else(|| content.get("word_count"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let task_count = content
        .get("taskCount")
        .or_else(|| content.get("task_count"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    (snippet, word_count, task_count)
}

pub(crate) fn drive_uri(drive_space_id: &str, drive_node_id: &str) -> String {
    format!("drive://spaces/{drive_space_id}/nodes/{drive_node_id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn snapshot_metrics_match_the_replaced_adapter() {
        let snapshot = build_snapshot(PageSnapshotInput {
            drive_space_id: "space-1",
            drive_node_id: "node-1",
            drive_version_id: "version-1",
            drive_version_no: 1,
            content_type: "application/json",
            content_schema_version: "notes/v1",
            content_hash: Some("sha256:abc".to_string()),
            content: json!({ "title": "Plan", "wordCount": 7, "task_count": 2 }),
        });
        assert_eq!(snapshot.drive_uri, "drive://spaces/space-1/nodes/node-1");
        assert_eq!(snapshot.snippet.as_deref(), Some("Plan"));
        assert_eq!(snapshot.word_count, 7);
        assert_eq!(snapshot.task_count, 2);
    }

    #[test]
    fn version_kind_stays_derived_from_the_version_number() {
        assert_eq!(build_version_summary("v1", 1, "t").version_kind, "initial");
        assert_eq!(build_version_summary("v2", 2, "t").version_kind, "auto");
    }

    #[test]
    fn empty_version_list_reports_an_empty_first_page() {
        let page = build_version_page(&[], 1, 10);
        assert!(page.items.is_empty());
        assert!(!page.page_info.has_more);
        assert_eq!(page.page_info.page, 1);
        assert_eq!(page.page_info.page_size, 10);
    }
}
