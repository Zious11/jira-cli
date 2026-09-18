use crate::api::client::JiraClient;
use crate::api::pagination::OffsetPage;
use crate::types::jira::{Board, BoardConfig};
use anyhow::Result;

impl JiraClient {
    /// List all boards accessible to the authenticated user.
    ///
    /// Optionally filter by `project_key` (`projectKeyOrId` query param) and/or
    /// `board_type` (`type` query param, e.g. `"scrum"` or `"kanban"`).
    ///
    /// BC-X.15.001 (FIX-F5-001 F3): the CLI scope-hint rewrite
    /// (`cli::board::rewrite_agile_scope_error`) now scans the whole anyhow
    /// error chain, not only the top-level error — adding `.context()` here
    /// is safe, but keep `JrError::InsufficientScope` reachable somewhere in
    /// the returned chain.
    pub async fn list_boards(
        &self,
        project_key: Option<&str>,
        board_type: Option<&str>,
    ) -> Result<Vec<Board>> {
        let mut all_boards: Vec<Board> = Vec::new();
        let mut start_at: u32 = 0;
        let max_results: u32 = 50;

        loop {
            let mut path = format!(
                "/rest/agile/1.0/board?startAt={}&maxResults={}",
                start_at, max_results
            );
            if let Some(pk) = project_key {
                path.push_str(&format!("&projectKeyOrId={}", urlencoding::encode(pk)));
            }
            if let Some(bt) = board_type {
                path.push_str(&format!("&type={}", urlencoding::encode(bt)));
            }
            let page: OffsetPage<Board> = self.get(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all_boards.extend(page.values.unwrap_or_default());

            if !has_more {
                break;
            }
            start_at = next;
        }

        Ok(all_boards)
    }

    /// Get the configuration for a specific board.
    ///
    /// BC-X.15.001 (FIX-F5-001 F3): the CLI scope-hint rewrite
    /// (`cli::board::rewrite_agile_scope_error`) now scans the whole anyhow
    /// error chain, not only the top-level error — adding `.context()` here
    /// is safe, but keep `JrError::InsufficientScope` reachable somewhere in
    /// the returned chain.
    pub async fn get_board_config(&self, board_id: u64) -> Result<BoardConfig> {
        let path = format!("/rest/agile/1.0/board/{}/configuration", board_id);
        self.get(&path).await
    }
}
