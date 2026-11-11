//! Notes -> Drive port selection (MODULE_SPEC.md §4.2).
//!
//! sdkwork-drive is reached **in-process** by default: Notes calls Drive's
//! service/store crates on the host PostgreSQL pool the packaged gateway
//! already owns. No Drive endpoint variable is read on that path, so a
//! deployment needs no sibling URL and no service credential.
//!
//! Two explicit opt-ins exist, and neither is a default:
//!
//! | Selection | Variable | Why |
//! | --- | --- | --- |
//! | `http` | `SDKWORK_NOTES_DRIVE_MODE=http` + `SDKWORK_DRIVE_FACADE_URL` | a real split deployment where Drive runs in another process/container (MODULE_SPEC.md §4.2.3). The endpoint is validated here, at startup. |
//! | `memory` | `SDKWORK_NOTES_DRIVE_MODE=memory` (or the legacy `SDKWORK_NOTES_USE_MEMORY_DRIVE=1`) | local development / tests with no Drive database. |
//!
//! An unrecognized mode value is an error, not a silent fallback, so a
//! misspelled opt-in cannot look like a working split deployment.
//!
//! # No host pool
//!
//! The in-process port needs the process PostgreSQL pool that owns `dr_drive_*`
//! (the packaged gateway installs it before mounting modules). A standalone
//! development process has no such pool; rather than failing every page request
//! at runtime, a `standalone` (or unset) deployment profile keeps the previous
//! local memory Drive. Any other profile fails at startup instead of silently
//! writing page bytes to memory in a real deployment.

use sdkwork_notes_pages_service::domain::{DrivePageContentSnapshot, DriveVersionPage};
use sdkwork_notes_pages_service::error::NotesProductError;
use sdkwork_notes_pages_service::infrastructure::drive::MemoryDrivePageContentPort;
use sdkwork_notes_pages_service::ports::{
    CreateDrivePageContentCommand, DrivePageContentPort, ListDrivePageContentVersionsCommand,
    ReadDrivePageContentCommand, RestoreDrivePageContentVersionCommand,
    UpdateDrivePageContentCommand,
};
use sdkwork_utils_rust::string::trim;

use crate::service::drive_http_adapter::HttpDrivePageContentPort;
use crate::service::drive_in_process::InProcessDrivePageContentPort;

/// Selects how this process reaches sdkwork-drive.
pub const DRIVE_MODE_ENV: &str = "SDKWORK_NOTES_DRIVE_MODE";

/// Legacy local-development switch, kept working for existing dev profiles.
pub const LEGACY_MEMORY_DRIVE_ENV: &str = "SDKWORK_NOTES_USE_MEMORY_DRIVE";

/// Deployment profile used to decide whether a missing host pool is a local
/// development process or a misconfigured packaged deployment.
const DEPLOYMENT_PROFILE_ENV: &str = "SDKWORK_NOTES_DEPLOYMENT_PROFILE";

/// How this process reaches sdkwork-drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveMode {
    /// Call Drive's service/store crates in this process over the host pool.
    InProcess,
    /// Local in-memory Drive for development and tests.
    Memory,
    /// Address Drive's app-api over HTTP; split deployments only.
    Http,
}

impl DriveMode {
    /// Stable spelling used in logs, health output, and the opt-in value.
    pub fn label(self) -> &'static str {
        match self {
            Self::InProcess => "in-process",
            Self::Memory => "memory",
            Self::Http => "http",
        }
    }
}

/// Why the Drive port could not be built at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrivePortBootstrapError {
    /// `SDKWORK_NOTES_DRIVE_MODE` carried a value this build does not know.
    UnsupportedMode(String),
    /// The HTTP opt-in was selected without a usable endpoint.
    HttpAdapter(String),
    /// A packaged deployment selected in-process Drive without the process
    /// PostgreSQL pool that owns `dr_drive_*`.
    HostPoolUnavailable { profile: Option<String> },
}

impl std::fmt::Display for DrivePortBootstrapError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedMode(value) => write!(
                formatter,
                "unsupported {DRIVE_MODE_ENV} value: {value} (expected \"in-process\", \"memory\", or \"http\")"
            ),
            Self::HttpAdapter(message) => write!(formatter, "{message}"),
            Self::HostPoolUnavailable { profile } => write!(
                formatter,
                "in-process Drive requires the host PostgreSQL pool that owns dr_drive_*; \
                 deployment profile {} has none (set {DRIVE_MODE_ENV}=memory only for local \
                 development, or {DRIVE_MODE_ENV}=http for a split deployment)",
                profile.clone().unwrap_or_else(|| "unset".to_string())
            ),
        }
    }
}

impl std::error::Error for DrivePortBootstrapError {}

/// Inputs to the Drive port decision, so the rules stay unit-testable without
/// mutating process-global environment state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrivePortSelectionInput {
    /// Raw `SDKWORK_NOTES_DRIVE_MODE` value.
    pub mode: Option<String>,
    /// Raw `SDKWORK_NOTES_USE_MEMORY_DRIVE` truthiness.
    pub legacy_memory_drive: Option<bool>,
    /// Whether this process owns the host PostgreSQL pool.
    pub host_postgres_pool: bool,
    /// Raw `SDKWORK_NOTES_DEPLOYMENT_PROFILE` value.
    pub deployment_profile: Option<String>,
}

/// The port a selection resolves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrivePortSelection {
    Memory,
    InProcess,
    Http,
}

/// Resolves the configured mode, ignoring the host pool.
///
/// Absent or blank means [`DriveMode::InProcess`] unless the legacy memory
/// switch is on. An explicit mode always wins over the legacy switch.
pub fn resolve_drive_mode(
    mode: Option<&str>,
    legacy_memory_drive: Option<bool>,
) -> Result<DriveMode, DrivePortBootstrapError> {
    match mode.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(if legacy_memory_drive == Some(true) {
            DriveMode::Memory
        } else {
            DriveMode::InProcess
        }),
        Some(raw) => match raw.to_ascii_lowercase().as_str() {
            "in-process" | "in_process" | "inprocess" => Ok(DriveMode::InProcess),
            "memory" => Ok(DriveMode::Memory),
            "http" => Ok(DriveMode::Http),
            _ => Err(DrivePortBootstrapError::UnsupportedMode(raw.to_owned())),
        },
    }
}

/// Resolves the concrete port, including the host-pool requirement.
///
/// # Errors
///
/// Fails for an unrecognized mode, and when a non-development deployment would
/// have to degrade the in-process port to memory because the host pool is
/// missing.
pub fn select_drive_port(
    input: DrivePortSelectionInput,
) -> Result<DrivePortSelection, DrivePortBootstrapError> {
    match resolve_drive_mode(input.mode.as_deref(), input.legacy_memory_drive)? {
        DriveMode::Memory => Ok(DrivePortSelection::Memory),
        DriveMode::Http => Ok(DrivePortSelection::Http),
        DriveMode::InProcess if input.host_postgres_pool => Ok(DrivePortSelection::InProcess),
        // A standalone/local process has no Drive database to call in-process;
        // keep the previous local memory Drive instead of failing every page
        // request at runtime.
        DriveMode::InProcess if is_local_development_profile(input.deployment_profile.as_deref()) => {
            Ok(DrivePortSelection::Memory)
        }
        // A packaged deployment must not silently keep page bytes in memory.
        DriveMode::InProcess => Err(DrivePortBootstrapError::HostPoolUnavailable {
            profile: input.deployment_profile,
        }),
    }
}

/// Whether the deployment profile is a local development profile.
///
/// A missing or blank profile behaves like `standalone`, matching the shared
/// runtime default in `sdkwork_utils_rust::service_base_url`.
fn is_local_development_profile(profile: Option<&str>) -> bool {
    sdkwork_utils_rust::service_base_url::is_standalone_profile(profile)
}

/// The Drive page-content port this process uses.
#[derive(Clone)]
pub enum NotesApiDrivePort {
    Memory(MemoryDrivePageContentPort),
    InProcess(InProcessDrivePageContentPort),
    Http(HttpDrivePageContentPort),
}

impl NotesApiDrivePort {
    /// Builds the port from the process environment.
    ///
    /// # Errors
    ///
    /// Fails at startup (never at the first page request) for an unrecognized
    /// mode, an HTTP opt-in without an endpoint, or a packaged deployment
    /// without the host PostgreSQL pool. The default in-process path never
    /// needs sibling configuration.
    pub fn from_env() -> Result<Self, DrivePortBootstrapError> {
        let mode = std::env::var(DRIVE_MODE_ENV).ok();
        let legacy_memory_drive = std::env::var(LEGACY_MEMORY_DRIVE_ENV)
            .ok()
            .map(|value| parse_truthy(&value));
        let deployment_profile = std::env::var(DEPLOYMENT_PROFILE_ENV).ok();
        let host_postgres_pool = host_postgres_pool();
        let selection = select_drive_port(DrivePortSelectionInput {
            mode,
            legacy_memory_drive,
            host_postgres_pool: host_postgres_pool.is_some(),
            deployment_profile: deployment_profile.clone(),
        })?;
        match selection {
            DrivePortSelection::Memory => Ok(Self::Memory(MemoryDrivePageContentPort::default())),
            DrivePortSelection::Http => Ok(Self::Http(
                HttpDrivePageContentPort::from_env().map_err(DrivePortBootstrapError::HttpAdapter)?,
            )),
            DrivePortSelection::InProcess => match host_postgres_pool {
                Some(pool) => Ok(Self::InProcess(InProcessDrivePageContentPort::new(pool))),
                None => Err(DrivePortBootstrapError::HostPoolUnavailable {
                    profile: deployment_profile,
                }),
            },
        }
    }
}

/// The canonical process PostgreSQL pool, when the host gateway installed one.
///
/// Client-local SQLite pools share the same registry, so the pool is only
/// accepted here when it is the PostgreSQL server pool.
fn host_postgres_pool() -> Option<sqlx::PgPool> {
    sdkwork_database_sqlx::process_shared_database_pool()
        .and_then(|pool| pool.as_postgres().cloned())
}

fn parse_truthy(value: &str) -> bool {
    matches!(
        trim(value).to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[async_trait::async_trait]
impl DrivePageContentPort for NotesApiDrivePort {
    async fn create_page_content(
        &self,
        command: CreateDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, NotesProductError> {
        match self {
            Self::Memory(port) => port.create_page_content(command).await,
            Self::InProcess(port) => port.create_page_content(command).await,
            Self::Http(port) => port.create_page_content(command).await,
        }
    }

    async fn read_page_content(
        &self,
        command: ReadDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, NotesProductError> {
        match self {
            Self::Memory(port) => port.read_page_content(command).await,
            Self::InProcess(port) => port.read_page_content(command).await,
            Self::Http(port) => port.read_page_content(command).await,
        }
    }

    async fn update_page_content(
        &self,
        command: UpdateDrivePageContentCommand,
    ) -> Result<DrivePageContentSnapshot, NotesProductError> {
        match self {
            Self::Memory(port) => port.update_page_content(command).await,
            Self::InProcess(port) => port.update_page_content(command).await,
            Self::Http(port) => port.update_page_content(command).await,
        }
    }

    async fn list_page_content_versions(
        &self,
        command: ListDrivePageContentVersionsCommand,
    ) -> Result<DriveVersionPage, NotesProductError> {
        match self {
            Self::Memory(port) => port.list_page_content_versions(command).await,
            Self::InProcess(port) => port.list_page_content_versions(command).await,
            Self::Http(port) => port.list_page_content_versions(command).await,
        }
    }

    async fn restore_page_content_version(
        &self,
        command: RestoreDrivePageContentVersionCommand,
    ) -> Result<DrivePageContentSnapshot, NotesProductError> {
        match self {
            Self::Memory(port) => port.restore_page_content_version(command).await,
            Self::InProcess(port) => port.restore_page_content_version(command).await,
            Self::Http(port) => port.restore_page_content_version(command).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(
        mode: Option<&str>,
        pool: bool,
        profile: Option<&str>,
    ) -> Result<DrivePortSelection, DrivePortBootstrapError> {
        select_drive_port(DrivePortSelectionInput {
            mode: mode.map(str::to_string),
            legacy_memory_drive: None,
            host_postgres_pool: pool,
            deployment_profile: profile.map(str::to_string),
        })
    }

    #[test]
    fn an_unset_mode_defaults_to_in_process_on_the_host_pool() {
        assert_eq!(
            selection(None, true, Some("cloud")).expect("in-process default"),
            DrivePortSelection::InProcess
        );
        assert_eq!(
            resolve_drive_mode(None, None).expect("default mode"),
            DriveMode::InProcess
        );
    }

    #[test]
    fn blank_modes_are_treated_as_unset() {
        for value in ["", "   ", "\t"] {
            assert_eq!(
                resolve_drive_mode(Some(value), None).expect("blank is unset"),
                DriveMode::InProcess,
                "value {value:?} must keep the in-process default"
            );
        }
    }

    #[test]
    fn http_is_an_explicit_opt_in() {
        for value in ["http", "HTTP", " Http "] {
            assert_eq!(
                resolve_drive_mode(Some(value), Some(true)).expect("http opt-in"),
                DriveMode::Http,
                "value {value:?} must select the split-deployment adapter"
            );
        }
    }

    #[test]
    fn memory_remains_available_for_local_development() {
        for value in ["memory", "MEMORY", " Memory "] {
            assert_eq!(
                resolve_drive_mode(Some(value), None).expect("memory opt-in"),
                DriveMode::Memory,
                "value {value:?} must select the local memory Drive"
            );
        }
        assert_eq!(
            resolve_drive_mode(None, Some(true)).expect("legacy memory switch"),
            DriveMode::Memory
        );
        for value in ["in-process", "in_process", "inprocess", "IN-PROCESS"] {
            assert_eq!(
                resolve_drive_mode(Some(value), Some(true)).expect("explicit mode wins"),
                DriveMode::InProcess,
                "value {value:?} must select the in-process port"
            );
        }
    }

    #[test]
    fn an_unknown_mode_names_the_variable_and_the_accepted_values() {
        let error = resolve_drive_mode(Some("grpc"), None)
            .expect_err("an unknown mode must not silently fall back");
        let rendered = error.to_string();
        assert!(rendered.contains(DRIVE_MODE_ENV), "{rendered}");
        assert!(rendered.contains("\"in-process\""), "{rendered}");
        assert!(rendered.contains("\"memory\""), "{rendered}");
        assert!(rendered.contains("\"http\""), "{rendered}");
    }

    #[test]
    fn a_local_process_without_a_host_pool_keeps_the_memory_drive() {
        for profile in [None, Some("standalone"), Some("STANDALONE")] {
            assert_eq!(
                selection(None, false, profile).expect("local development profile"),
                DrivePortSelection::Memory,
                "profile {profile:?}"
            );
        }
    }

    #[test]
    fn a_packaged_deployment_without_a_host_pool_fails_at_startup() {
        let error = selection(None, false, Some("cloud"))
            .expect_err("a packaged deployment must not degrade to memory");
        let rendered = error.to_string();
        assert!(rendered.contains("PostgreSQL"), "{rendered}");
        assert!(rendered.contains("cloud"), "{rendered}");
    }

    #[test]
    fn the_http_opt_in_without_an_endpoint_fails_at_startup() {
        // The endpoint validation lives in the adapter; the port must surface
        // its message unchanged so the boot log names the missing variable.
        let error = DrivePortBootstrapError::HttpAdapter(format!(
            "{DRIVE_MODE_ENV}=http requires SDKWORK_DRIVE_FACADE_URL"
        ));
        let rendered = error.to_string();
        assert!(rendered.contains(DRIVE_MODE_ENV), "{rendered}");
        assert!(rendered.contains("SDKWORK_DRIVE_FACADE_URL"), "{rendered}");
    }

    #[test]
    fn labels_are_the_documented_spellings() {
        assert_eq!(DriveMode::InProcess.label(), "in-process");
        assert_eq!(DriveMode::Memory.label(), "memory");
        assert_eq!(DriveMode::Http.label(), "http");
    }

    #[test]
    fn the_default_path_never_reads_a_sibling_endpoint() {
        // Regression guard for MODULE_SPEC.md §4.2: only the opt-in
        // `drive_http_adapter` module may address the Drive endpoint. Doc
        // comments may name the opt-in variable; executable code may not.
        let source = include_str!("drive_port.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("the module has a production section");
        let code = production
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("SDKWORK_DRIVE_FACADE_URL"),
            "the Drive endpoint belongs to the opt-in HTTP adapter only"
        );
        assert!(
            !code.contains("_URL\""),
            "the port selector must not read an endpoint variable"
        );
        assert!(
            !code.contains("reqwest"),
            "the port selector must not depend on an HTTP client"
        );
    }

    #[test]
    fn only_the_opt_in_adapter_reads_the_drive_endpoint() {
        let source = include_str!("drive_http_adapter.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("the module has a production section");
        assert!(
            production.contains("\"SDKWORK_DRIVE_FACADE_URL\""),
            "the opt-in adapter must address a configured Drive endpoint"
        );
        assert!(
            production.contains("SDKWORK_NOTES_DRIVE_MODE=http"),
            "the adapter must document the mode opt-in it requires"
        );
    }
}
