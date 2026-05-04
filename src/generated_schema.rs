use serde_json::Value;

const ROOT_TYPE: &str = "object";
const ALLOWED_PROPERTY_TYPES: &[&str] = &["string", "number", "integer", "boolean", "array"];
const ALLOWED_ITEMS_TYPES: &[&str] = &["string", "number", "integer", "boolean"];
const ALLOWED_ROOT_KEYS: &[&str] = &[
    "type",
    "properties",
    "required",
    "additionalProperties",
    "description",
];
const ALLOWED_PROPERTY_KEYS: &[&str] = &["type", "description", "default", "enum", "items"];
const ALLOWED_ITEMS_KEYS: &[&str] = &["type", "enum"];
const FORBIDDEN_KEYWORDS: &[&str] = &[
    "oneOf", "allOf", "anyOf", "$ref", "pattern", "minimum", "maximum",
];

pub fn validate(schema: &Value) -> Result<(), String> {
    let root = schema
        .as_object()
        .ok_or_else(|| "schema root must be a JSON object".to_string())?;

    reject_forbidden(root, "schema")?;

    let ty = root
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or_else(|| "schema.type is required".to_string())?;
    if ty != ROOT_TYPE {
        return Err(format!("schema.type must be \"object\", got \"{ty}\""));
    }

    let additional = root
        .get("additionalProperties")
        .ok_or_else(|| "schema.additionalProperties must be set to false".to_string())?;
    if additional.as_bool() != Some(false) {
        return Err("schema.additionalProperties must be false".to_string());
    }

    for key in root.keys() {
        if !ALLOWED_ROOT_KEYS.contains(&key.as_str()) {
            return Err(format!("schema has disallowed key \"{key}\" at root"));
        }
    }

    let empty_map = serde_json::Map::new();
    let properties = match root.get("properties") {
        Some(Value::Object(map)) => map,
        Some(_) => return Err("schema.properties must be an object".to_string()),
        None => &empty_map,
    };

    for (name, prop_schema) in properties {
        validate_property(name, prop_schema)?;
    }

    if let Some(required) = root.get("required") {
        let arr = required
            .as_array()
            .ok_or_else(|| "schema.required must be an array".to_string())?;
        for v in arr {
            let key = v
                .as_str()
                .ok_or_else(|| "schema.required entries must be strings".to_string())?;
            if !properties.contains_key(key) {
                return Err(format!(
                    "schema.required contains \"{key}\" which is not in properties"
                ));
            }
        }
    }

    Ok(())
}

fn validate_property(name: &str, prop: &Value) -> Result<(), String> {
    let obj = prop
        .as_object()
        .ok_or_else(|| format!("schema.properties.{name} must be an object"))?;

    reject_forbidden(obj, &format!("schema.properties.{name}"))?;

    for key in obj.keys() {
        if !ALLOWED_PROPERTY_KEYS.contains(&key.as_str()) {
            return Err(format!(
                "schema.properties.{name} has disallowed key \"{key}\""
            ));
        }
    }

    let ty = obj
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or_else(|| format!("schema.properties.{name}.type is required"))?;
    if !ALLOWED_PROPERTY_TYPES.contains(&ty) {
        return Err(format!(
            "schema.properties.{name}.type \"{ty}\" is not allowed (allowed: {})",
            ALLOWED_PROPERTY_TYPES.join(", ")
        ));
    }

    if ty == "array" {
        let items = obj
            .get("items")
            .ok_or_else(|| format!("schema.properties.{name}.items is required for array"))?;
        validate_items(name, items)?;
    } else if obj.contains_key("items") {
        return Err(format!(
            "schema.properties.{name}.items only allowed when type is \"array\""
        ));
    }

    Ok(())
}

fn validate_items(prop_name: &str, items: &Value) -> Result<(), String> {
    let obj = items
        .as_object()
        .ok_or_else(|| format!("schema.properties.{prop_name}.items must be an object"))?;

    reject_forbidden(obj, &format!("schema.properties.{prop_name}.items"))?;

    for key in obj.keys() {
        if !ALLOWED_ITEMS_KEYS.contains(&key.as_str()) {
            return Err(format!(
                "schema.properties.{prop_name}.items has disallowed key \"{key}\""
            ));
        }
    }

    let ty = obj
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or_else(|| format!("schema.properties.{prop_name}.items.type is required"))?;
    if !ALLOWED_ITEMS_TYPES.contains(&ty) {
        return Err(format!(
            "schema.properties.{prop_name}.items.type \"{ty}\" is not allowed (allowed: {})",
            ALLOWED_ITEMS_TYPES.join(", ")
        ));
    }

    Ok(())
}

fn reject_forbidden(obj: &serde_json::Map<String, Value>, path: &str) -> Result<(), String> {
    for kw in FORBIDDEN_KEYWORDS {
        if obj.contains_key(*kw) {
            return Err(format!("{path} uses forbidden keyword \"{kw}\""));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_minimal_empty_schema() {
        let s = json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        });
        validate(&s).unwrap();
    }

    #[test]
    fn accepts_full_property_set() {
        let s = json!({
            "type": "object",
            "properties": {
                "userName": { "type": "string", "description": "user name" },
                "count": { "type": "integer", "default": 0 },
                "ratio": { "type": "number" },
                "enabled": { "type": "boolean" },
                "color": { "type": "string", "enum": ["red", "green"] },
                "tags": { "type": "array", "items": { "type": "string" } }
            },
            "required": ["userName"],
            "additionalProperties": false
        });
        validate(&s).unwrap();
    }

    #[test]
    fn rejects_non_object_root() {
        let s = json!("string");
        assert_eq!(
            validate(&s).unwrap_err(),
            "schema root must be a JSON object"
        );
    }

    #[test]
    fn rejects_missing_type() {
        let s = json!({ "properties": {}, "additionalProperties": false });
        assert_eq!(validate(&s).unwrap_err(), "schema.type is required");
    }

    #[test]
    fn rejects_non_object_root_type() {
        let s = json!({
            "type": "string",
            "additionalProperties": false
        });
        assert_eq!(
            validate(&s).unwrap_err(),
            "schema.type must be \"object\", got \"string\""
        );
    }

    #[test]
    fn rejects_missing_additional_properties() {
        let s = json!({ "type": "object", "properties": {} });
        assert_eq!(
            validate(&s).unwrap_err(),
            "schema.additionalProperties must be set to false"
        );
    }

    #[test]
    fn rejects_additional_properties_true() {
        let s = json!({
            "type": "object",
            "properties": {},
            "additionalProperties": true
        });
        assert_eq!(
            validate(&s).unwrap_err(),
            "schema.additionalProperties must be false"
        );
    }

    #[test]
    fn rejects_required_not_in_properties() {
        let s = json!({
            "type": "object",
            "properties": { "a": { "type": "string" } },
            "required": ["a", "b"],
            "additionalProperties": false
        });
        assert_eq!(
            validate(&s).unwrap_err(),
            "schema.required contains \"b\" which is not in properties"
        );
    }

    #[test]
    fn rejects_forbidden_keyword_at_root() {
        let s = json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false,
            "oneOf": []
        });
        assert_eq!(
            validate(&s).unwrap_err(),
            "schema uses forbidden keyword \"oneOf\""
        );
    }

    #[test]
    fn rejects_forbidden_keyword_in_property() {
        let s = json!({
            "type": "object",
            "properties": {
                "x": { "type": "string", "pattern": "^a$" }
            },
            "additionalProperties": false
        });
        assert_eq!(
            validate(&s).unwrap_err(),
            "schema.properties.x uses forbidden keyword \"pattern\""
        );
    }

    #[test]
    fn rejects_nested_object_property() {
        let s = json!({
            "type": "object",
            "properties": {
                "inner": { "type": "object" }
            },
            "additionalProperties": false
        });
        let err = validate(&s).unwrap_err();
        assert!(err.contains("schema.properties.inner.type \"object\" is not allowed"));
    }

    #[test]
    fn rejects_array_without_items() {
        let s = json!({
            "type": "object",
            "properties": { "tags": { "type": "array" } },
            "additionalProperties": false
        });
        assert_eq!(
            validate(&s).unwrap_err(),
            "schema.properties.tags.items is required for array"
        );
    }

    #[test]
    fn rejects_array_items_object() {
        let s = json!({
            "type": "object",
            "properties": {
                "tags": { "type": "array", "items": { "type": "object" } }
            },
            "additionalProperties": false
        });
        let err = validate(&s).unwrap_err();
        assert!(err.contains("items.type \"object\" is not allowed"));
    }

    #[test]
    fn rejects_disallowed_property_key() {
        let s = json!({
            "type": "object",
            "properties": {
                "x": { "type": "string", "minimum": 0 }
            },
            "additionalProperties": false
        });
        let err = validate(&s).unwrap_err();
        assert_eq!(
            err,
            "schema.properties.x uses forbidden keyword \"minimum\""
        );
    }

    #[test]
    fn rejects_disallowed_root_key() {
        let s = json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false,
            "title": "foo"
        });
        assert_eq!(
            validate(&s).unwrap_err(),
            "schema has disallowed key \"title\" at root"
        );
    }

    #[test]
    fn allows_property_without_required_when_optional() {
        let s = json!({
            "type": "object",
            "properties": {
                "a": { "type": "string" },
                "b": { "type": "integer" }
            },
            "required": ["a"],
            "additionalProperties": false
        });
        validate(&s).unwrap();
    }
}
