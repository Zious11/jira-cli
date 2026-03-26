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

/// A single attribute entry from `GET /object/{id}/attributes`.
/// Includes the full attribute definition with name, unlike `AssetAttribute`
/// which only has the numeric `objectTypeAttributeId`.
#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectAttribute {
    pub id: String,
    #[serde(rename = "objectTypeAttributeId")]
    pub object_type_attribute_id: String,
    #[serde(rename = "objectTypeAttribute")]
    pub object_type_attribute: ObjectTypeAttributeDef,
    #[serde(rename = "objectAttributeValues", default)]
    pub values: Vec<ObjectAttributeValue>,
}

/// Attribute definition from the object type schema.
#[derive(Debug, Deserialize, Serialize)]
pub struct ObjectTypeAttributeDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub system: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub label: bool,
    #[serde(default)]
    pub position: i32,
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

    #[test]
    fn deserialize_object_attribute_with_name() {
        let json = r#"{
            "id": "637",
            "objectTypeAttributeId": "134",
            "objectTypeAttribute": {
                "id": "134",
                "name": "Location",
                "system": false,
                "hidden": false,
                "label": false,
                "position": 4
            },
            "objectAttributeValues": [
                { "value": "New York, NY", "displayValue": "New York, NY" }
            ]
        }"#;
        let attr: ObjectAttribute = serde_json::from_str(json).unwrap();
        assert_eq!(attr.id, "637");
        assert_eq!(attr.object_type_attribute_id, "134");
        assert_eq!(attr.object_type_attribute.name, "Location");
        assert!(!attr.object_type_attribute.system);
        assert!(!attr.object_type_attribute.hidden);
        assert!(!attr.object_type_attribute.label);
        assert_eq!(attr.object_type_attribute.position, 4);
        assert_eq!(attr.values.len(), 1);
        assert_eq!(
            attr.values[0].display_value.as_deref(),
            Some("New York, NY")
        );
    }

    #[test]
    fn deserialize_object_attribute_defaults() {
        let json = r#"{
            "id": "640",
            "objectTypeAttributeId": "135",
            "objectTypeAttribute": {
                "id": "135",
                "name": "Name"
            },
            "objectAttributeValues": []
        }"#;
        let attr: ObjectAttribute = serde_json::from_str(json).unwrap();
        assert_eq!(attr.object_type_attribute.name, "Name");
        assert!(!attr.object_type_attribute.system);
        assert!(!attr.object_type_attribute.hidden);
        assert!(!attr.object_type_attribute.label);
        assert_eq!(attr.object_type_attribute.position, 0);
        assert!(attr.values.is_empty());
    }

    #[test]
    fn deserialize_object_attribute_system() {
        let json = r#"{
            "id": "638",
            "objectTypeAttributeId": "136",
            "objectTypeAttribute": {
                "id": "136",
                "name": "Created",
                "system": true,
                "hidden": false,
                "label": false,
                "position": 2
            },
            "objectAttributeValues": [
                { "value": "2021-02-16T20:04:41.527Z", "displayValue": "16/Feb/21 8:04 PM" }
            ]
        }"#;
        let attr: ObjectAttribute = serde_json::from_str(json).unwrap();
        assert!(attr.object_type_attribute.system);
        assert_eq!(
            attr.values[0].display_value.as_deref(),
            Some("16/Feb/21 8:04 PM")
        );
    }
}
