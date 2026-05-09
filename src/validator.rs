use serde_json::Value;

#[derive(Copy, Clone)]
pub enum Context<'a> {
    Input { positional: Option<&'a str> },
    Output,
}

pub fn validate_input(
    value: &Value,
    schema: &Value,
    positional: Option<&str>,
) -> Result<(), String> {
    validate_object(value, schema, Context::Input { positional }, "")
}

pub fn validate_output(value: &Value, schema: &Value) -> Result<(), String> {
    let ty = schema.get("type").and_then(|t| t.as_str()).unwrap_or("object");
    if ty == "object" {
        validate_object(value, schema, Context::Output, "")
    } else {
        validate_field(value, schema, Context::Output, "")
    }
}

fn validate_object(
    value: &Value,
    schema: &Value,
    ctx: Context<'_>,
    path: &str,
) -> Result<(), String> {
    let obj = match value.as_object() {
        Some(o) => o,
        None => {
            return Err(format_error(
                ctx,
                path,
                ErrorKind::TypeMismatch {
                    expected: "object".into(),
                    got: value.clone(),
                },
            ));
        }
    };

    let properties = schema.get("properties").and_then(|p| p.as_object());
    let required: Vec<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let additional_properties = schema
        .get("additionalProperties")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    for req in &required {
        if !obj.contains_key(*req) {
            return Err(format_error(ctx, &join_path(path, req), ErrorKind::Required));
        }
    }

    for (key, val) in obj {
        let prop_schema = properties.and_then(|p| p.get(key));
        match prop_schema {
            Some(s) => validate_field(val, s, ctx, &join_path(path, key))?,
            None => {
                if !additional_properties {
                    return Err(format_error(ctx, &join_path(path, key), ErrorKind::Unknown));
                }
            }
        }
    }

    Ok(())
}

fn validate_field(
    value: &Value,
    schema: &Value,
    ctx: Context<'_>,
    path: &str,
) -> Result<(), String> {
    let ty = schema.get("type").and_then(|t| t.as_str()).unwrap_or("string");

    if !matches_type(value, ty) {
        return Err(format_error(
            ctx,
            path,
            ErrorKind::TypeMismatch {
                expected: ty.into(),
                got: value.clone(),
            },
        ));
    }

    if let Some(en) = schema.get("enum").and_then(|e| e.as_array())
        && !en.iter().any(|v| v == value)
    {
        return Err(format_error(
            ctx,
            path,
            ErrorKind::EnumViolation {
                allowed: en.clone(),
                got: value.clone(),
            },
        ));
    }

    if ty == "array"
        && let Some(items_schema) = schema.get("items")
    {
        let arr = value.as_array().expect("type already checked");
        let item_ty = items_schema
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("string");
        let items_enum = items_schema.get("enum").and_then(|e| e.as_array());
        for item in arr {
            if !matches_type(item, item_ty) {
                return Err(format_error(
                    ctx,
                    path,
                    ErrorKind::TypeMismatch {
                        expected: item_ty.into(),
                        got: item.clone(),
                    },
                ));
            }
            if let Some(en) = items_enum
                && !en.iter().any(|v| v == item)
            {
                return Err(format_error(
                    ctx,
                    path,
                    ErrorKind::EnumViolation {
                        allowed: en.clone(),
                        got: item.clone(),
                    },
                ));
            }
        }
    }

    if ty == "object" {
        validate_object(value, schema, ctx, path)?;
    }

    Ok(())
}

fn matches_type(value: &Value, ty: &str) -> bool {
    match ty {
        "string" => value.is_string(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "null" => value.is_null(),
        _ => true,
    }
}

enum ErrorKind {
    Required,
    Unknown,
    TypeMismatch { expected: String, got: Value },
    EnumViolation { allowed: Vec<Value>, got: Value },
}

fn format_error(ctx: Context<'_>, path: &str, kind: ErrorKind) -> String {
    match ctx {
        Context::Input { positional } => format_input_error(path, kind, positional),
        Context::Output => format_output_error(path, kind),
    }
}

fn format_input_error(path: &str, kind: ErrorKind, positional: Option<&str>) -> String {
    let is_positional = positional == Some(path);
    let kebab = path_to_kebab(path);
    match kind {
        ErrorKind::Required if is_positional => {
            format!("Error: <{path}>: positional argument required")
        }
        ErrorKind::Required => format!("Error: --{kebab}: required"),
        ErrorKind::Unknown => format!("Error: --{kebab}: unknown flag"),
        ErrorKind::TypeMismatch { expected, got } => format!(
            "Error: --{kebab}: expected {expected}, got {}",
            format_value(&got)
        ),
        ErrorKind::EnumViolation { allowed, got } => format!(
            "Error: --{kebab}: must be one of {}, got {}",
            format_enum(&allowed),
            format_value(&got)
        ),
    }
}

fn format_output_error(path: &str, kind: ErrorKind) -> String {
    let p = if path.is_empty() {
        "<root>".to_string()
    } else {
        format!(".{path}")
    };
    match kind {
        ErrorKind::Required => format!("output validation: {p}: required"),
        ErrorKind::Unknown => format!("output validation: {p}: unknown property"),
        ErrorKind::TypeMismatch { expected, got } => format!(
            "output validation: {p}: expected {expected}, got {}",
            format_value(&got)
        ),
        ErrorKind::EnumViolation { allowed, got } => format!(
            "output validation: {p}: must be one of {}, got {}",
            format_enum(&allowed),
            format_value(&got)
        ),
    }
}

fn join_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

fn path_to_kebab(path: &str) -> String {
    path.split('.')
        .map(camel_to_kebab)
        .collect::<Vec<_>>()
        .join(".")
}

pub fn camel_to_kebab(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

pub fn kebab_to_camel(s: &str) -> String {
    let mut out = String::new();
    let mut upper_next = false;
    for c in s.chars() {
        if c == '-' {
            upper_next = true;
        } else if upper_next {
            out.push(c.to_ascii_uppercase());
            upper_next = false;
        } else {
            out.push(c);
        }
    }
    out
}

pub fn format_value(v: &Value) -> String {
    match v {
        Value::String(s) => format!("\"{s}\""),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => format!(
            "[{}]",
            arr.iter().map(format_value).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(_) => "<object>".to_string(),
    }
}

pub fn format_enum(values: &[Value]) -> String {
    format!(
        "[{}]",
        values.iter().map(format_value).collect::<Vec<_>>().join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn input_required_missing_uses_kebab() {
        let schema = json!({
            "type": "object",
            "properties": { "userName": { "type": "string" } },
            "required": ["userName"],
            "additionalProperties": false
        });
        let err = validate_input(&json!({}), &schema, None).unwrap_err();
        assert_eq!(err, "Error: --user-name: required");
    }

    #[test]
    fn input_type_mismatch() {
        let schema = json!({
            "type": "object",
            "properties": { "count": { "type": "integer" } },
            "required": ["count"]
        });
        let err = validate_input(&json!({ "count": "abc" }), &schema, None).unwrap_err();
        assert_eq!(err, "Error: --count: expected integer, got \"abc\"");
    }

    #[test]
    fn input_integer_rejects_float() {
        let schema = json!({
            "type": "object",
            "properties": { "count": { "type": "integer" } },
            "required": ["count"]
        });
        let err = validate_input(&json!({ "count": 1.5 }), &schema, None).unwrap_err();
        assert_eq!(err, "Error: --count: expected integer, got 1.5");
    }

    #[test]
    fn input_enum_violation() {
        let schema = json!({
            "type": "object",
            "properties": {
                "color": { "type": "string", "enum": ["red", "green", "blue"] }
            },
            "required": ["color"]
        });
        let err = validate_input(&json!({ "color": "purple" }), &schema, None).unwrap_err();
        assert_eq!(
            err,
            "Error: --color: must be one of [\"red\", \"green\", \"blue\"], got \"purple\""
        );
    }

    #[test]
    fn input_unknown_flag_blocked_by_additional_properties_false() {
        let schema = json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        });
        let err = validate_input(&json!({ "mystery": "x" }), &schema, None).unwrap_err();
        assert_eq!(err, "Error: --mystery: unknown flag");
    }

    #[test]
    fn input_array_items_type_mismatch() {
        let schema = json!({
            "type": "object",
            "properties": {
                "tags": { "type": "array", "items": { "type": "string" } }
            }
        });
        let err = validate_input(&json!({ "tags": ["a", 42] }), &schema, None).unwrap_err();
        assert_eq!(err, "Error: --tags: expected string, got 42");
    }

    #[test]
    fn input_array_items_enum_violation() {
        let schema = json!({
            "type": "object",
            "properties": {
                "colors": {
                    "type": "array",
                    "items": { "type": "string", "enum": ["red", "blue"] }
                }
            }
        });
        let err = validate_input(&json!({ "colors": ["red", "purple"] }), &schema, None).unwrap_err();
        assert_eq!(
            err,
            "Error: --colors: must be one of [\"red\", \"blue\"], got \"purple\""
        );
    }

    #[test]
    fn input_happy_path() {
        let schema = json!({
            "type": "object",
            "properties": {
                "userName": { "type": "string" },
                "count": { "type": "integer" },
                "ratio": { "type": "number" },
                "enabled": { "type": "boolean" },
                "tags": { "type": "array", "items": { "type": "string" } }
            },
            "required": ["userName", "count"],
            "additionalProperties": false
        });
        validate_input(
            &json!({
                "userName": "alice",
                "count": 3,
                "ratio": 1.5,
                "enabled": true,
                "tags": ["a", "b"]
            }),
            &schema,
            None,
        )
        .unwrap();
    }

    #[test]
    fn output_required_missing() {
        let schema = json!({
            "type": "object",
            "properties": { "y": { "type": "string" } },
            "required": ["y"]
        });
        let err = validate_output(&json!({}), &schema).unwrap_err();
        assert_eq!(err, "output validation: .y: required");
    }

    #[test]
    fn output_type_mismatch_at_root() {
        let schema = json!({
            "type": "object",
            "properties": { "y": { "type": "string" } }
        });
        let err = validate_output(&json!({ "y": 42 }), &schema).unwrap_err();
        assert_eq!(err, "output validation: .y: expected string, got 42");
    }

    #[test]
    fn output_unknown_property_blocked() {
        let schema = json!({
            "type": "object",
            "properties": { "y": { "type": "string" } },
            "additionalProperties": false
        });
        let err = validate_output(&json!({ "y": "ok", "extra": 1 }), &schema).unwrap_err();
        assert_eq!(err, "output validation: .extra: unknown property");
    }

    #[test]
    fn output_non_object_root_when_object_expected() {
        let schema = json!({ "type": "object" });
        let err = validate_output(&json!("not-an-object"), &schema).unwrap_err();
        assert_eq!(
            err,
            "output validation: <root>: expected object, got \"not-an-object\""
        );
    }

    #[test]
    fn camel_kebab_roundtrip() {
        assert_eq!(camel_to_kebab("userName"), "user-name");
        assert_eq!(kebab_to_camel("user-name"), "userName");
        assert_eq!(camel_to_kebab("apiKey"), "api-key");
        assert_eq!(kebab_to_camel("api-key"), "apiKey");
        assert_eq!(camel_to_kebab("count"), "count");
    }
}
