use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::User;

#[derive(Debug, Deserialize, Serialize)]
pub struct Worklog {
    pub id: Option<String>,
    pub author: Option<User>,
    #[serde(rename = "timeSpentSeconds")]
    pub time_spent_seconds: Option<u64>,
    #[serde(rename = "timeSpent")]
    pub time_spent: Option<String>,
    pub comment: Option<Value>,
    pub started: Option<String>,
}
