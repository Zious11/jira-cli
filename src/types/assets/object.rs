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
    #[serde(rename = "defaultType")]
    pub default_type: Option<DefaultType>,
    #[serde(rename = "referenceType")]
    pub reference_type: Option<ReferenceType>,
    #[serde(rename = "referenceObjectType")]
    pub reference_object_type: Option<ReferenceObjectType>,
    #[serde(rename = "minimumCardinality", default)]
    pub minimum_cardinality: i32,
    #[serde(rename = "maximumCardinality", default)]
    pub maximum_cardinality: i32,
    #[serde(default)]
    pub editable: bool,
    pub description: Option<String>,
    pub options: Option<String>,
}

/// Attribute data type (e.g., Text, DateTime, Select).
#[derive(Debug, Deserialize, Serialize)]
pub struct DefaultType {
    pub id: i32,
    pub name: String,
}

/// Reference link type (e.g., "Depends on", "References").
#[derive(Debug, Deserialize, Serialize)]
pub struct ReferenceType {
    pub id: String,
    pub name: String,
}

/// Target object type for a reference attribute (e.g., "Service", "Employee").
#[derive(Debug, Deserialize, Serialize)]
pub struct ReferenceObjectType {
    pub id: String,
    pub name: String,
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

    #[test]
    fn deserialize_attribute_def_with_default_type() {
        let json = r#"{
            "id": "135",
            "name": "Name",
            "system": false,
            "hidden": false,
            "label": true,
            "position": 1,
            "defaultType": { "id": 0, "name": "Text" },
            "minimumCardinality": 1,
            "maximumCardinality": 1,
            "editable": true,
            "description": "The name of the object"
        }"#;
        let def: ObjectTypeAttributeDef = serde_json::from_str(json).unwrap();
        assert_eq!(def.name, "Name");
        assert!(def.label);
        let dt = def.default_type.unwrap();
        assert_eq!(dt.id, 0);
        assert_eq!(dt.name, "Text");
        assert_eq!(def.minimum_cardinality, 1);
        assert!(def.editable);
        assert_eq!(def.description.as_deref(), Some("The name of the object"));
        assert!(def.reference_type.is_none());
        assert!(def.reference_object_type.is_none());
    }

    #[test]
    fn deserialize_attribute_def_with_reference() {
        let json = r#"{
            "id": "869",
            "name": "Service relationships",
            "system": false,
            "hidden": false,
            "label": false,
            "position": 6,
            "referenceType": { "id": "36", "name": "Depends on" },
            "referenceObjectTypeId": "122",
            "referenceObjectType": { "id": "122", "name": "Service" },
            "minimumCardinality": 0,
            "maximumCardinality": -1,
            "editable": true
        }"#;
        let def: ObjectTypeAttributeDef = serde_json::from_str(json).unwrap();
        assert_eq!(def.name, "Service relationships");
        assert!(def.default_type.is_none());
        let rt = def.reference_type.unwrap();
        assert_eq!(rt.name, "Depends on");
        let rot = def.reference_object_type.unwrap();
        assert_eq!(rot.name, "Service");
        assert_eq!(def.minimum_cardinality, 0);
        assert_eq!(def.maximum_cardinality, -1);
    }

    #[test]
    fn deserialize_attribute_def_select_with_options() {
        let json = r#"{
            "id": "868",
            "name": "Tier",
            "system": false,
            "hidden": false,
            "label": false,
            "position": 5,
            "defaultType": { "id": 10, "name": "Select" },
            "minimumCardinality": 1,
            "maximumCardinality": 1,
            "editable": true,
            "options": "Tier 1,Tier 2,Tier 3"
        }"#;
        let def: ObjectTypeAttributeDef = serde_json::from_str(json).unwrap();
        let dt = def.default_type.unwrap();
        assert_eq!(dt.name, "Select");
        assert_eq!(def.options.as_deref(), Some("Tier 1,Tier 2,Tier 3"));
        assert_eq!(def.minimum_cardinality, 1);
    }

    #[test]
    fn deserialize_attribute_def_backward_compat() {
        // Existing JSON without the new fields — must still deserialize
        let json = r#"{
            "id": "134",
            "name": "Key",
            "system": true,
            "hidden": false,
            "label": false,
            "position": 0
        }"#;
        let def: ObjectTypeAttributeDef = serde_json::from_str(json).unwrap();
        assert_eq!(def.id, "134");
        assert!(def.system);
        assert!(def.default_type.is_none());
        assert!(def.reference_type.is_none());
        assert!(def.reference_object_type.is_none());
        assert_eq!(def.minimum_cardinality, 0);
        assert_eq!(def.maximum_cardinality, 0);
        assert!(!def.editable);
        assert!(def.description.is_none());
        assert!(def.options.is_none());
    }
}
