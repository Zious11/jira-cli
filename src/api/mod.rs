pub mod assets;
pub mod auth;
pub mod auth_embedded;
/// Windows DPAPI-encrypted-file OAuth-token fallback (ADR-0021, issue #759).
/// Thin sibling module to `auth.rs`, internal to the crate — see
/// `auth_windows_store`'s module doc for the design this implements.
pub(crate) mod auth_windows_store;
pub mod client;
pub mod jira;
pub mod jsm;
pub mod pagination;
pub mod rate_limit;
pub(crate) mod refresh_coordinator;
