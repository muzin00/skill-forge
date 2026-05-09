use serde_json::Value;

const ALLOWED_KEYS: &[&str] = &["positional"];

pub fn validate(args: &Value, input_schema: &Value) -> Result<(), String> {
    let obj = args
        .as_object()
        .ok_or_else(|| "args must be a JSON object".to_string())?;

    for key in obj.keys() {
        if !ALLOWED_KEYS.contains(&key.as_str()) {
            return Err(format!("args has disallowed key \"{key}\""));
        }
    }

    if let Some(positional) = obj.get("positional") {
        validate_positional(positional, input_schema)?;
    }

    Ok(())
}

fn validate_positional(positional: &Value, input_schema: &Value) -> Result<(), String> {
    let name = positional
        .as_str()
        .ok_or_else(|| "args.positional must be a string".to_string())?;

    let prop = input_schema
        .get("properties")
        .and_then(|p| p.as_object())
        .and_then(|p| p.get(name))
        .ok_or_else(|| format!("args.positional \"{name}\" not in schema.properties"))?;

    let ty = prop.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match ty {
        "array" => {
            let items_ty = prop
                .get("items")
                .and_then(|i| i.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("");
            if items_ty == "boolean" {
                return Err(format!(
                    "args.positional \"{name}\" refers to array of boolean — not supported for positional"
                ));
            }
        }
        "string" => {}
        _ => {
            return Err(format!(
                "args.positional \"{name}\" must reference type=array or type=string, got \"{ty}\""
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema_with(prop_name: &str, prop: Value) -> Value {
        json!({
            "type": "object",
            "properties": { prop_name: prop },
            "required": [prop_name],
            "additionalProperties": false
        })
    }

    #[test]
    fn accepts_valid_positional_string_array() {
        let schema = schema_with(
            "files",
            json!({ "type": "array", "items": { "type": "string" } }),
        );
        let args = json!({ "positional": "files" });
        validate(&args, &schema).unwrap();
    }

    #[test]
    fn accepts_valid_positional_integer_array() {
        let schema = schema_with(
            "ports",
            json!({ "type": "array", "items": { "type": "integer" } }),
        );
        let args = json!({ "positional": "ports" });
        validate(&args, &schema).unwrap();
    }

    #[test]
    fn accepts_empty_args_object() {
        let schema = schema_with("x", json!({ "type": "string" }));
        let args = json!({});
        validate(&args, &schema).unwrap();
    }

    #[test]
    fn rejects_non_object_args() {
        let schema = schema_with("x", json!({ "type": "string" }));
        let args = json!("string");
        assert_eq!(validate(&args, &schema).unwrap_err(), "args must be a JSON object");
    }

    #[test]
    fn rejects_unknown_key() {
        let schema = schema_with("x", json!({ "type": "string" }));
        let args = json!({ "bogus": "y" });
        assert_eq!(
            validate(&args, &schema).unwrap_err(),
            "args has disallowed key \"bogus\""
        );
    }

    #[test]
    fn rejects_positional_not_string() {
        let schema = schema_with("x", json!({ "type": "string" }));
        let args = json!({ "positional": 42 });
        assert_eq!(
            validate(&args, &schema).unwrap_err(),
            "args.positional must be a string"
        );
    }

    #[test]
    fn rejects_positional_property_not_in_schema() {
        let schema = schema_with(
            "files",
            json!({ "type": "array", "items": { "type": "string" } }),
        );
        let args = json!({ "positional": "missing" });
        assert_eq!(
            validate(&args, &schema).unwrap_err(),
            "args.positional \"missing\" not in schema.properties"
        );
    }

    #[test]
    fn accepts_valid_positional_string_scalar() {
        let schema = schema_with("name", json!({ "type": "string" }));
        let args = json!({ "positional": "name" });
        validate(&args, &schema).unwrap();
    }

    #[test]
    fn rejects_positional_integer_scalar() {
        let schema = schema_with("count", json!({ "type": "integer" }));
        let args = json!({ "positional": "count" });
        assert_eq!(
            validate(&args, &schema).unwrap_err(),
            "args.positional \"count\" must reference type=array or type=string, got \"integer\""
        );
    }

    #[test]
    fn rejects_positional_boolean_scalar() {
        let schema = schema_with("enabled", json!({ "type": "boolean" }));
        let args = json!({ "positional": "enabled" });
        assert_eq!(
            validate(&args, &schema).unwrap_err(),
            "args.positional \"enabled\" must reference type=array or type=string, got \"boolean\""
        );
    }

    #[test]
    fn rejects_positional_array_of_boolean() {
        let schema = schema_with(
            "flags",
            json!({ "type": "array", "items": { "type": "boolean" } }),
        );
        let args = json!({ "positional": "flags" });
        assert_eq!(
            validate(&args, &schema).unwrap_err(),
            "args.positional \"flags\" refers to array of boolean — not supported for positional"
        );
    }
}
