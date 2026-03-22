use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Project {
    pub key: String,
    pub name: String,
}
