use serde::Deserialize;

/// Offset-based pagination used by most Jira REST API endpoints.
///
/// Different endpoints return items under different keys (`values`, `issues`, `worklogs`,
/// `comments`), so all four are optional — callers use `items()` to get whichever is populated.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OffsetPage<T> {
    /// Items returned under the "values" key (boards, sprints, etc.).
    #[serde(default)]
    pub values: Option<Vec<T>>,
    /// Items returned under the "issues" key (search endpoints).
    #[serde(default)]
    pub issues: Option<Vec<T>>,
    /// Items returned under the "worklogs" key (worklog endpoints).
    #[serde(default)]
    pub worklogs: Option<Vec<T>>,
    /// Items returned under the "comments" key (comment endpoints).
    #[serde(default)]
    pub comments: Option<Vec<T>>,
    /// The index of the first item returned in this page.
    #[serde(default)]
    pub start_at: u32,
    /// The maximum number of items that could be returned.
    #[serde(default)]
    pub max_results: u32,
    /// The total number of items matching the query.
    #[serde(default)]
    pub total: u32,
}

impl<T> OffsetPage<T> {
    /// Returns a reference to whichever item list is populated, preferring
    /// `values` > `issues` > `worklogs` > `comments`. Returns an empty slice if none are set.
    pub fn items(&self) -> &[T] {
        if let Some(ref v) = self.values {
            return v;
        }
        if let Some(ref v) = self.issues {
            return v;
        }
        if let Some(ref v) = self.worklogs {
            return v;
        }
        if let Some(ref v) = self.comments {
            return v;
        }
        &[]
    }

    /// Returns true if there are more pages after this one.
    pub fn has_more(&self) -> bool {
        self.start_at + self.max_results < self.total
    }

    /// Returns the `startAt` value for the next page.
    pub fn next_start(&self) -> u32 {
        self.start_at + self.max_results
    }
}

/// Cursor-based pagination used by the JQL search endpoint (v3).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPage<T> {
    /// The issues returned in this page.
    #[serde(default)]
    pub issues: Vec<T>,
    /// Opaque token for fetching the next page. `None` when this is the last page.
    pub next_page_token: Option<String>,
}

impl<T> CursorPage<T> {
    /// Returns true when there are more pages to fetch.
    pub fn has_more(&self) -> bool {
        self.next_page_token.is_some()
    }
}

/// Offset-based pagination used by Jira Service Management `/rest/servicedeskapi/` endpoints.
///
/// Uses different field names than `OffsetPage`: `size` (items in page) instead of `total`,
/// `isLastPage` boolean instead of computed from startAt+maxResults, and `start`/`limit`
/// instead of `startAt`/`maxResults`.
#[derive(Debug, Deserialize)]
pub struct ServiceDeskPage<T> {
    /// Count of items in the current page.
    pub size: u32,
    /// Zero-based starting index.
    pub start: u32,
    /// Maximum items per page.
    pub limit: u32,
    /// Whether this is the last page of results.
    #[serde(rename = "isLastPage")]
    pub is_last_page: bool,
    /// The items in this page.
    #[serde(default)]
    pub values: Vec<T>,
}

impl<T> ServiceDeskPage<T> {
    /// Returns true if there are more pages after this one.
    pub fn has_more(&self) -> bool {
        !self.is_last_page
    }

    /// Returns the `start` value for the next page.
    pub fn next_start(&self) -> u32 {
        self.start + self.size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_page_has_more() {
        let page: OffsetPage<String> = OffsetPage {
            values: Some(vec!["a".into(), "b".into()]),
            issues: None,
            worklogs: None,
            comments: None,
            start_at: 0,
            max_results: 2,
            total: 5,
        };
        assert!(page.has_more());
        assert_eq!(page.next_start(), 2);
    }

    #[test]
    fn test_offset_page_last_page() {
        let page: OffsetPage<String> = OffsetPage {
            values: Some(vec!["a".into()]),
            issues: None,
            worklogs: None,
            comments: None,
            start_at: 4,
            max_results: 2,
            total: 5,
        };
        assert!(!page.has_more());
    }

    #[test]
    fn test_offset_page_items_from_issues() {
        let page: OffsetPage<String> = OffsetPage {
            values: None,
            issues: Some(vec!["issue-1".into()]),
            worklogs: None,
            comments: None,
            start_at: 0,
            max_results: 50,
            total: 1,
        };
        assert_eq!(page.items(), &["issue-1".to_string()]);
    }

    #[test]
    fn test_offset_page_items_from_comments() {
        let page: OffsetPage<String> = OffsetPage {
            values: None,
            issues: None,
            worklogs: None,
            comments: None,
            start_at: 0,
            max_results: 50,
            total: 1,
        };
        assert!(page.items().is_empty());

        let page_with_comments: OffsetPage<String> = OffsetPage {
            values: None,
            issues: None,
            worklogs: None,
            comments: Some(vec!["comment-1".into()]),
            start_at: 0,
            max_results: 50,
            total: 1,
        };
        assert_eq!(page_with_comments.items(), &["comment-1".to_string()]);
    }

    #[test]
    fn test_cursor_page_has_more() {
        let with_token: CursorPage<String> = CursorPage {
            issues: vec!["a".into()],
            next_page_token: Some("abc123".into()),
        };
        assert!(with_token.has_more());

        let last_page: CursorPage<String> = CursorPage {
            issues: vec!["b".into()],
            next_page_token: None,
        };
        assert!(!last_page.has_more());
    }

    #[test]
    fn test_service_desk_page_has_more() {
        let page: ServiceDeskPage<String> = ServiceDeskPage {
            size: 5,
            start: 0,
            limit: 50,
            is_last_page: false,
            values: vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()],
        };
        assert!(page.has_more());
        assert_eq!(page.next_start(), 5);
    }

    #[test]
    fn test_service_desk_page_last_page() {
        let page: ServiceDeskPage<String> = ServiceDeskPage {
            size: 3,
            start: 10,
            limit: 50,
            is_last_page: true,
            values: vec!["a".into(), "b".into(), "c".into()],
        };
        assert!(!page.has_more());
        assert_eq!(page.next_start(), 13);
    }

    #[test]
    fn test_service_desk_page_empty() {
        let page: ServiceDeskPage<String> = ServiceDeskPage {
            size: 0,
            start: 0,
            limit: 50,
            is_last_page: true,
            values: vec![],
        };
        assert!(!page.has_more());
        assert_eq!(page.next_start(), 0);
        assert!(page.values.is_empty());
    }

    #[test]
    fn test_service_desk_page_deserialize() {
        let json = r#"{
            "size": 2,
            "start": 0,
            "limit": 50,
            "isLastPage": false,
            "values": ["item1", "item2"]
        }"#;
        let page: ServiceDeskPage<String> = serde_json::from_str(json).unwrap();
        assert_eq!(page.size, 2);
        assert_eq!(page.values.len(), 2);
        assert!(!page.is_last_page);
    }
}
