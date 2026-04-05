use serde::{Deserialize, Serialize};

/// Object schema from GET /objectschema/list.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ObjectSchema {
    pub id: String,
    pub name: String,
    #[serde(rename = "objectSchemaKey")]
    pub object_schema_key: String,
    pub description: Option<String>,
    #[serde(rename = "objectCount", default)]
    pub object_count: i64,
    #[serde(rename = "objectTypeCount", default)]
    pub object_type_count: i64,
}

/// Object type entry from GET /objectschema/{id}/objecttypes/flat.
#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectTypeEntry {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub position: i32,
    #[serde(rename = "objectCount", default)]
    pub object_count: i64,
    #[serde(rename = "objectSchemaId")]
    pub object_schema_id: String,
    #[serde(default)]
    pub inherited: bool,
    #[serde(rename = "abstractObjectType", default)]
    pub abstract_object_type: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_object_schema_full() {
        let json = r#"{
            "id": "6",
            "name": "ITSM",
            "objectSchemaKey": "ITSM",
            "status": "Ok",
            "description": "IT assets schema",
            "objectCount": 95,
            "objectTypeCount": 34
        }"#;
        let schema: ObjectSchema = serde_json::from_str(json).unwrap();
        assert_eq!(schema.id, "6");
        assert_eq!(schema.name, "ITSM");
        assert_eq!(schema.object_schema_key, "ITSM");
        assert_eq!(schema.description.as_deref(), Some("IT assets schema"));
        assert_eq!(schema.object_count, 95);
        assert_eq!(schema.object_type_count, 34);
    }

    #[test]
    fn deserialize_object_schema_minimal() {
        let json = r#"{
            "id": "1",
            "name": "HR",
            "objectSchemaKey": "HR"
        }"#;
        let schema: ObjectSchema = serde_json::from_str(json).unwrap();
        assert_eq!(schema.id, "1");
        assert_eq!(schema.name, "HR");
        assert!(schema.description.is_none());
        assert_eq!(schema.object_count, 0);
        assert_eq!(schema.object_type_count, 0);
    }

    #[test]
    fn deserialize_object_type_entry() {
        let json = r#"{
            "id": "19",
            "name": "Employee",
            "position": 0,
            "objectCount": 42,
            "objectSchemaId": "1",
            "inherited": false,
            "abstractObjectType": false,
            "parentObjectTypeInherited": false
        }"#;
        let entry: ObjectTypeEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.id, "19");
        assert_eq!(entry.name, "Employee");
        assert_eq!(entry.position, 0);
        assert_eq!(entry.object_count, 42);
        assert_eq!(entry.object_schema_id, "1");
        assert!(!entry.inherited);
        assert!(!entry.abstract_object_type);
        assert!(entry.description.is_none());
    }

    #[test]
    fn deserialize_object_type_entry_with_description() {
        let json = r#"{
            "id": "23",
            "name": "Office",
            "description": "Physical office or site.",
            "position": 2,
            "objectCount": 0,
            "objectSchemaId": "6",
            "inherited": false,
            "abstractObjectType": false
        }"#;
        let entry: ObjectTypeEntry = serde_json::from_str(json).unwrap();
        assert_eq!(
            entry.description.as_deref(),
            Some("Physical office or site.")
        );
        assert_eq!(entry.position, 2);
    }
}
