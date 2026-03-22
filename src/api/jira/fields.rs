use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::api::client::JiraClient;

#[derive(Debug, Deserialize, Serialize)]
pub struct Field {
    pub id: String,
    pub name: String,
    pub custom: Option<bool>,
}

impl JiraClient {
    pub async fn list_fields(&self) -> Result<Vec<Field>> {
        self.get("/rest/api/3/field").await
    }

    pub async fn find_team_field_id(&self) -> Result<Option<String>> {
        let fields = self.list_fields().await?;
        Ok(fields
            .iter()
            .find(|f| f.name.to_lowercase() == "team" && f.custom == Some(true))
            .map(|f| f.id.clone()))
    }
}
