use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Board {
    pub id: u64,
    pub name: String,
    #[serde(rename = "type")]
    pub board_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BoardConfig {
    pub id: u64,
    pub name: String,
    #[serde(rename = "type", default)]
    pub board_type: String,
}
