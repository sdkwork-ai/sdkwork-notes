use sqlx::any::AnyPoolOptions;
use sdkwork_database_config::DatabaseConfig;

use sdkwork_notes_pages_repository_sqlx::install_sqlite_schema;
use sdkwork_notes_pages_repository_sqlx::notes_store::SqlNotesStore;
use sdkwork_notes_pages_service::service::NotesService;

use super::drive_port::NotesApiDrivePort;

pub async fn assemble_notes_service_from_env(
) -> Result<NotesService<SqlNotesStore, NotesApiDrivePort>, String> {
    sqlx::any::install_default_drivers();
    // Notes pages persist to client-local SQLite (ENVIRONMENT_SPEC §7.2): the
    // SqlNotesStore and schema installer are SQLite-only. When the deployment
    // declares SDKWORK_DATABASE_SQLITE_URL it takes precedence over the
    // workspace PostgreSQL profile so embedded gateway deployments can run
    // notes against the client-local store while every other module uses
    // PostgreSQL; the legacy workspace resolution keeps standalone behavior.
    let config = if sdkwork_database_config::client_local_sqlite_url_configured() {
        DatabaseConfig::load_client_local_from_env("notes")
    } else {
        DatabaseConfig::from_env("notes")
    }
    .map_err(|error| format!("resolve notes database config failed: {error}"))?;
    let pool = AnyPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(config.acquire_timeout())
        .idle_timeout(config.idle_timeout())
        .max_lifetime(config.max_lifetime())
        .connect(&config.url)
        .await
        .map_err(|error| format!("connect notes database failed: {error}"))?;
    install_sqlite_schema(&pool)
        .await
        .map_err(|error| format!("install notes schema failed: {error}"))?;

    // MODULE_SPEC.md §4.2: the Drive port is resolved here, at startup, so a
    // misconfigured split-deployment opt-in or a packaged deployment without
    // the host PostgreSQL pool fails the boot instead of the first page save.
    let drive_port = NotesApiDrivePort::from_env()
        .map_err(|error| format!("initialize notes Drive port failed: {error}"))?;

    Ok(NotesService::new(SqlNotesStore::new(pool), drive_port))
}
