//! Notes product service construction (API_ASSEMBLY_SPEC §3/§6.1).
//! The assembly owns the notes pages service bootstrap — repository, drive
//! port, and database lifecycle — so consuming gateways never import
//! `sdkwork-notes-pages-service` or `sdkwork-notes-pages-repository-sqlx`
//! directly.

pub mod database;
pub mod drive_http_adapter;
pub mod drive_in_process;
pub mod drive_page_content_mapping;
pub mod drive_port;
