use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Board {
    pub id: u64,
    pub name: String,
    #[serde(rename = "type")]
    pub board_type: String,
    #[serde(default)]
    pub location: Option<BoardLocation>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct BoardLocation {
    #[serde(default, rename = "projectKey")]
    pub project_key: Option<String>,
    #[serde(default, rename = "projectName")]
    pub project_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BoardConfig {
    pub id: u64,
    pub name: String,
    #[serde(rename = "type", default)]
    pub board_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_deserializes_with_location() {
        let json = r#"{
            "id": 42,
            "name": "My Board",
            "type": "scrum",
            "location": {
                "projectKey": "PROJ",
                "projectName": "My Project"
            }
        }"#;
        let board: Board = serde_json::from_str(json).unwrap();
        assert_eq!(board.id, 42);
        assert_eq!(board.board_type, "scrum");
        let loc = board.location.unwrap();
        assert_eq!(loc.project_key.as_deref(), Some("PROJ"));
        assert_eq!(loc.project_name.as_deref(), Some("My Project"));
    }

    #[test]
    fn board_deserializes_without_location() {
        let json = r#"{
            "id": 99,
            "name": "No Location Board",
            "type": "kanban"
        }"#;
        let board: Board = serde_json::from_str(json).unwrap();
        assert_eq!(board.id, 99);
        assert!(board.location.is_none());
    }
}
