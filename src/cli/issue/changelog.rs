use anyhow::Result;

use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};

pub(super) async fn handle(
    _command: IssueCommand,
    _output_format: &OutputFormat,
    _client: &JiraClient,
) -> Result<()> {
    // Implemented in Task 4.
    unimplemented!("changelog handler — see Task 4")
}
