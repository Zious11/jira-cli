use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct AssetObject {
    pub id: String,
    pub label: String,
    #[serde(rename = "objectKey")]
    pub object_key: String,
    #[serde(rename = "objectType")]
    pub object_type: ObjectType,
    pub created: Option<String>,
    pub updated: Option<String>,
    #[serde(default)]
    pub attributes: Vec<AssetAttribute>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ObjectType {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AssetAttribute {
    pub id: String,
    #[serde(rename = "objectTypeAttributeId")]
    pub object_type_attribute_id: String,
    #[serde(rename = "objectAttributeValues", default)]
    pub values: Vec<ObjectAttributeValue>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectAttributeValue {
    pub value: Option<String>,
    #[serde(rename = "displayValue")]
    pub display_value: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_asset_object_minimal() {
        let json = r#"{
            "id": "88",
            "label": "Acme Corp",
            "objectKey": "OBJ-88",
            "objectType": { "id": "23", "name": "Client" }
        }"#;
        let obj: AssetObject = serde_json::from_str(json).unwrap();
        assert_eq!(obj.id, "88");
        assert_eq!(obj.label, "Acme Corp");
        assert_eq!(obj.object_key, "OBJ-88");
        assert_eq!(obj.object_type.name, "Client");
        assert!(obj.attributes.is_empty());
        assert!(obj.created.is_none());
    }

    #[test]
    fn deserialize_asset_object_with_attributes() {
        let json = r#"{
            "id": "88",
            "label": "Acme Corp",
            "objectKey": "OBJ-88",
            "objectType": { "id": "23", "name": "Client" },
            "created": "2025-12-17T14:58:00.000Z",
            "updated": "2026-01-29T19:52:00.000Z",
            "attributes": [
                {
                    "id": "637",
                    "objectTypeAttributeId": "134",
                    "objectAttributeValues": [
                        { "value": "contact@acme.com", "displayValue": "contact@acme.com" }
                    ]
                }
            ]
        }"#;
        let obj: AssetObject = serde_json::from_str(json).unwrap();
        assert_eq!(obj.attributes.len(), 1);
        assert_eq!(
            obj.attributes[0].values[0].display_value.as_deref(),
            Some("contact@acme.com")
        );
    }
}
