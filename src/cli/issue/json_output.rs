use serde_json::{Value, json};

/// JSON response for `issue move` — both changed and idempotent cases.
pub(crate) fn move_response(key: &str, status: &str, changed: bool) -> Value {
    json!({
        "key": key,
        "status": status,
        "changed": changed
    })
}

/// JSON response for `issue assign` when the assignment changed.
pub(crate) fn assign_changed_response(key: &str, display_name: &str, account_id: &str) -> Value {
    json!({
        "key": key,
        "assignee": display_name,
        "assignee_account_id": account_id,
        "changed": true
    })
}

/// JSON response for `issue assign` when already assigned to the target user.
pub(crate) fn assign_unchanged_response(key: &str, display_name: &str, account_id: &str) -> Value {
    json!({
        "key": key,
        "assignee": display_name,
        "assignee_account_id": account_id,
        "changed": false
    })
}

/// JSON response for `issue assign --unassign`.
pub(crate) fn unassign_response(key: &str, changed: bool) -> Value {
    json!({
        "key": key,
        "assignee": null,
        "changed": changed
    })
}

/// JSON response for `issue edit`.
pub(crate) fn edit_response(key: &str) -> Value {
    json!({
        "key": key,
        "updated": true
    })
}

/// JSON response for `issue link`.
pub(crate) fn link_response(key1: &str, key2: &str, link_type: &str) -> Value {
    json!({
        "key1": key1,
        "key2": key2,
        "type": link_type,
        "linked": true
    })
}

/// JSON response for `issue unlink` — covers both success and no-match cases.
pub(crate) fn unlink_response(unlinked: bool, count: usize) -> Value {
    json!({
        "unlinked": unlinked,
        "count": count
    })
}

/// JSON response for `issue remote-link`.
pub(crate) fn remote_link_response(
    key: &str,
    id: u64,
    url: &str,
    title: &str,
    self_url: &str,
) -> Value {
    json!({
        "key": key,
        "id": id,
        "url": url,
        "title": title,
        "self": self_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_json_snapshot;

    #[test]
    fn test_move_response_changed() {
        assert_json_snapshot!(move_response("TEST-1", "In Progress", true));
    }

    #[test]
    fn test_move_response_unchanged() {
        assert_json_snapshot!(move_response("TEST-1", "Done", false));
    }

    #[test]
    fn test_assign_changed() {
        assert_json_snapshot!(assign_changed_response("TEST-1", "Jane Doe", "abc123"));
    }

    #[test]
    fn test_assign_unchanged() {
        assert_json_snapshot!(assign_unchanged_response("TEST-1", "Jane Doe", "abc123"));
    }

    #[test]
    fn test_unassign() {
        assert_json_snapshot!(unassign_response("TEST-1", true));
    }

    #[test]
    fn test_unassign_unchanged() {
        assert_json_snapshot!(unassign_response("TEST-1", false));
    }

    #[test]
    fn test_edit() {
        assert_json_snapshot!(edit_response("TEST-1"));
    }

    #[test]
    fn test_link() {
        assert_json_snapshot!(link_response("TEST-1", "TEST-2", "Blocks"));
    }

    #[test]
    fn test_unlink_success() {
        assert_json_snapshot!(unlink_response(true, 2));
    }

    #[test]
    fn test_unlink_no_match() {
        assert_json_snapshot!(unlink_response(false, 0));
    }

    #[test]
    fn test_remote_link() {
        assert_json_snapshot!(remote_link_response(
            "TEST-1",
            10000,
            "https://example.com",
            "Example",
            "https://example.atlassian.net/rest/api/3/issue/TEST-1/remotelink/10000",
        ));
    }
}
