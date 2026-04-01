use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::pagination::ServiceDeskPage;
use crate::types::jsm::Queue;
use crate::types::jsm::QueueIssueKey;

impl JiraClient {
    /// List all queues for a service desk, auto-paginating.
    pub async fn list_queues(&self, service_desk_id: &str) -> Result<Vec<Queue>> {
        let base = format!(
            "/rest/servicedeskapi/servicedesk/{}/queue",
            urlencoding::encode(service_desk_id)
        );
        let mut all = Vec::new();
        let mut start = 0u32;
        let page_size = 50u32;

        loop {
            let path = format!(
                "{}?includeCount=true&start={}&limit={}",
                base, start, page_size
            );
            let page: ServiceDeskPage<Queue> = self.get_from_instance(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all.extend(page.values);
            if !has_more {
                break;
            }
            start = next;
        }
        Ok(all)
    }

    /// Get issue keys from a queue, with optional limit and auto-pagination.
    ///
    /// Returns keys in queue order. Only extracts the `key` field from each
    /// issue — the caller batch-fetches full issue data via `search_issues`.
    pub async fn get_queue_issue_keys(
        &self,
        service_desk_id: &str,
        queue_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<String>> {
        let base = format!(
            "/rest/servicedeskapi/servicedesk/{}/queue/{}/issue",
            urlencoding::encode(service_desk_id),
            urlencoding::encode(queue_id)
        );
        let mut all = Vec::new();
        let mut start = 0u32;
        let max_page_size = 50u32;

        loop {
            let page_size = match limit {
                Some(cap) => {
                    let remaining = cap.saturating_sub(all.len() as u32);
                    if remaining == 0 {
                        break;
                    }
                    remaining.min(max_page_size)
                }
                None => max_page_size,
            };
            let path = format!("{}?start={}&limit={}", base, start, page_size);
            let page: ServiceDeskPage<QueueIssueKey> = self.get_from_instance(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all.extend(page.values.into_iter().map(|ik| ik.key));

            if let Some(cap) = limit {
                if all.len() >= cap as usize {
                    all.truncate(cap as usize);
                    break;
                }
            }
            if !has_more {
                break;
            }
            start = next;
        }
        Ok(all)
    }
}
