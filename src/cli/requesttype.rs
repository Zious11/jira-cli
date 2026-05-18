//! `jr requesttype list/fields` discovery commands for JSM request types.
//!
//! Anchors BC-X.12.001..008. Uses `require_service_desk` from
//! `src/api/jsm/servicedesks.rs` (extended in this PR to take a call-site label
//! per BC-X.8.004), `partial_match` for name resolution, and `cache::*` for the
//! per-(profile, serviceDeskId) request-type cache (7d TTL).

use anyhow::Result;

use crate::api::client::JiraClient;
use crate::cli::{OutputFormat, RequestTypeCommand};
use crate::config::Config;

pub async fn handle(
    command: RequestTypeCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
) -> Result<()> {
    let _ = (command, output_format, config, client, project_override);
    unimplemented!("BC-X.12.001..008: jr requesttype list/fields handler")
}
