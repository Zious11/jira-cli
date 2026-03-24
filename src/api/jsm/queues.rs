use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::pagination::ServiceDeskPage;
use crate::types::jsm::Queue;
use crate::types::jsm::QueueIssue;

impl JiraClient {
    /// List all queues for a service desk, auto-paginating.
    pub async fn list_queues(&self, service_desk_id: &str) -> Result<Vec<Queue>> {
        let base = format!("/rest/servicedeskapi/servicedesk/{}/queue", service_desk_id);
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

    /// Get issues in a queue, with optional limit and auto-pagination.
    pub async fn get_queue_issues(
        &self,
        service_desk_id: &str,
        queue_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<QueueIssue>> {
        let base = format!(
            "/rest/servicedeskapi/servicedesk/{}/queue/{}/issue",
            service_desk_id, queue_id
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
            let page: ServiceDeskPage<QueueIssue> = self.get_from_instance(&path).await?;
            let has_more = page.has_more();
            let next = page.next_start();
            all.extend(page.values);

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
