use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::validator::{self, camel_to_kebab, format_enum, format_value, kebab_to_camel};

pub fn build_args_json(schema: &Value, argv: &[String]) -> Result<String, String> {
    let mut map = parse_argv_to_partial(schema, argv)?;
    apply_boolean_defaults(&mut map, schema);
    let value = Value::Object(map);
    validator::validate_input(&value, schema)?;
    Ok(value.to_string())
}

pub fn build_args_json_with_stdin(
    schema: &Value,
    argv: &[String],
    stdin: &Value,
) -> Result<String, String> {
    let argv_overrides = parse_argv_to_partial(schema, argv)?;
    let mut map = match stdin {
        Value::Object(o) => o.clone(),
        Value::Null => Map::new(),
        _ => {
            return Err(format!(
                "Error: stdin: expected JSON object, got {}",
                format_value(stdin)
            ));
        }
    };
    for (k, v) in argv_overrides {
        map.insert(k, v);
    }
    apply_boolean_defaults(&mut map, schema);
    let value = Value::Object(map);
    validator::validate_input(&value, schema)?;
    Ok(value.to_string())
}

fn parse_argv_to_partial(
    schema: &Value,
    argv: &[String],
) -> Result<Map<String, Value>, String> {
    let properties = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .cloned()
        .unwrap_or_default();
    let additional_properties = schema
        .get("additionalProperties")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let mut flag_to_prop: HashMap<String, String> = HashMap::new();
    for prop_name in properties.keys() {
        flag_to_prop.insert(camel_to_kebab(prop_name), prop_name.clone());
    }

    let mut result: Map<String, Value> = Map::new();

    let mut i = 0;
    while i < argv.len() {
        let token = &argv[i];
        if !token.starts_with("--") || token.len() == 2 {
            return Err(format!("Error: {token}: unexpected positional argument"));
        }
        let flag = &token[2..];

        let (prop_name, prop_schema) = match flag_to_prop.get(flag) {
            Some(name) => (
                name.clone(),
                properties.get(name).cloned().unwrap_or(Value::Null),
            ),
            None => {
                if !additional_properties {
                    return Err(format!("Error: --{flag}: unknown flag"));
                }
                if i + 1 >= argv.len() {
                    return Err(format!("Error: --{flag}: missing value"));
                }
                let value = argv[i + 1].clone();
                let camel = kebab_to_camel(flag);
                result.insert(camel, Value::String(value));
                i += 2;
                continue;
            }
        };

        let ty = prop_schema
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("string");

        if ty == "boolean" {
            result.insert(prop_name, Value::Bool(true));
            i += 1;
            continue;
        }

        if i + 1 >= argv.len() {
            return Err(format!("Error: --{flag}: missing value"));
        }
        let raw_value = argv[i + 1].clone();
        i += 2;

        if ty == "array" {
            let item_ty = prop_schema
                .get("items")
                .and_then(|x| x.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("string");
            let item = parse_typed_value(&raw_value, item_ty, flag)?;
            if let Some(items_enum) = prop_schema
                .get("items")
                .and_then(|x| x.get("enum"))
                .and_then(|e| e.as_array())
                && !items_enum.iter().any(|v| v == &item)
            {
                return Err(format!(
                    "Error: --{flag}: must be one of {}, got {}",
                    format_enum(items_enum),
                    format_value(&item)
                ));
            }
            let arr = result
                .entry(prop_name)
                .or_insert_with(|| Value::Array(vec![]));
            if let Some(a) = arr.as_array_mut() {
                a.push(item);
            }
        } else {
            let parsed = parse_typed_value(&raw_value, ty, flag)?;
            if let Some(enum_values) = prop_schema.get("enum").and_then(|e| e.as_array())
                && !enum_values.iter().any(|v| v == &parsed)
            {
                return Err(format!(
                    "Error: --{flag}: must be one of {}, got {}",
                    format_enum(enum_values),
                    format_value(&parsed)
                ));
            }
            result.insert(prop_name, parsed);
        }
    }

    Ok(result)
}

fn apply_boolean_defaults(map: &mut Map<String, Value>, schema: &Value) {
    let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) else {
        return;
    };
    for (prop_name, prop_schema) in properties {
        let ty = prop_schema
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("string");
        if ty == "boolean" && !map.contains_key(prop_name) {
            map.insert(prop_name.clone(), Value::Bool(false));
        }
    }
}

fn parse_typed_value(raw: &str, ty: &str, flag: &str) -> Result<Value, String> {
    match ty {
        "string" => Ok(Value::String(raw.to_string())),
        "integer" => {
            let valid = !raw.is_empty()
                && raw != "-"
                && raw.chars().enumerate().all(|(i, c)| {
                    if i == 0 && c == '-' {
                        true
                    } else {
                        c.is_ascii_digit()
                    }
                });
            if !valid {
                return Err(format!(
                    "Error: --{flag}: expected integer, got {}",
                    format_raw(raw)
                ));
            }
            let n: i64 = raw.parse().map_err(|_| {
                format!(
                    "Error: --{flag}: expected integer, got {}",
                    format_raw(raw)
                )
            })?;
            Ok(Value::Number(n.into()))
        }
        "number" => {
            let n: f64 = raw.parse().map_err(|_| {
                format!("Error: --{flag}: expected number, got {}", format_raw(raw))
            })?;
            if n.is_nan() || n.is_infinite() {
                return Err(format!(
                    "Error: --{flag}: expected number, got {}",
                    format_raw(raw)
                ));
            }
            let num = serde_json::Number::from_f64(n).ok_or_else(|| {
                format!("Error: --{flag}: expected number, got {}", format_raw(raw))
            })?;
            Ok(Value::Number(num))
        }
        "boolean" => Err(format!(
            "Error: --{flag}: boolean flag does not take a value"
        )),
        _ => Ok(Value::String(raw.to_string())),
    }
}

fn format_raw(s: &str) -> String {
    format!("\"{s}\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn s(v: &str) -> String {
        v.to_string()
    }

    #[test]
    fn happy_path_all_types() {
        let schema = json!({
            "type": "object",
            "properties": {
                "userName": { "type": "string" },
                "count": { "type": "integer" },
                "ratio": { "type": "number" },
                "enabled": { "type": "boolean" },
                "tags": { "type": "array", "items": { "type": "string" } },
                "color": { "type": "string", "enum": ["red", "green", "blue"] },
                "optionalNote": { "type": "string", "default": "unused" }
            },
            "required": ["userName", "count", "ratio"],
            "additionalProperties": false
        });
        let argv = vec![
            s("--user-name"), s("alice"),
            s("--count"), s("3"),
            s("--ratio"), s("1.5"),
            s("--enabled"),
            s("--tags"), s("a"), s("--tags"), s("b"),
            s("--color"), s("red"),
        ];
        let json_str = build_args_json(&schema, &argv).expect("should succeed");
        let v: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(
            v,
            json!({
                "userName": "alice",
                "count": 3,
                "ratio": 1.5,
                "enabled": true,
                "tags": ["a", "b"],
                "color": "red"
            })
        );
    }

    #[test]
    fn boolean_unspecified_is_false() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "verbose": { "type": "boolean" }
            },
            "required": ["name"],
            "additionalProperties": false
        });
        let argv = vec![s("--name"), s("alice")];
        let json_str = build_args_json(&schema, &argv).unwrap();
        let v: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(v, json!({ "name": "alice", "verbose": false }));
    }

    #[test]
    fn default_is_omitted() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "default": "world" }
            },
            "additionalProperties": false
        });
        let argv: Vec<String> = vec![];
        let json_str = build_args_json(&schema, &argv).unwrap();
        let v: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(v, json!({}));
    }

    #[test]
    fn type_mismatch_integer() {
        let schema = json!({
            "type": "object",
            "properties": { "count": { "type": "integer" } },
            "required": ["count"],
            "additionalProperties": false
        });
        let err = build_args_json(&schema, &[s("--count"), s("abc")]).unwrap_err();
        assert_eq!(err, "Error: --count: expected integer, got \"abc\"");
    }

    #[test]
    fn type_mismatch_integer_floating() {
        let schema = json!({
            "type": "object",
            "properties": { "count": { "type": "integer" } },
            "required": ["count"],
            "additionalProperties": false
        });
        let err = build_args_json(&schema, &[s("--count"), s("1.5")]).unwrap_err();
        assert_eq!(err, "Error: --count: expected integer, got \"1.5\"");
    }

    #[test]
    fn type_mismatch_number_nan() {
        let schema = json!({
            "type": "object",
            "properties": { "ratio": { "type": "number" } },
            "required": ["ratio"],
            "additionalProperties": false
        });
        let err = build_args_json(&schema, &[s("--ratio"), s("NaN")]).unwrap_err();
        assert_eq!(err, "Error: --ratio: expected number, got \"NaN\"");
    }

    #[test]
    fn required_missing() {
        let schema = json!({
            "type": "object",
            "properties": { "userName": { "type": "string" } },
            "required": ["userName"],
            "additionalProperties": false
        });
        let err = build_args_json(&schema, &[]).unwrap_err();
        assert_eq!(err, "Error: --user-name: required");
    }

    #[test]
    fn enum_violation() {
        let schema = json!({
            "type": "object",
            "properties": {
                "color": { "type": "string", "enum": ["red", "green", "blue"] }
            },
            "required": ["color"],
            "additionalProperties": false
        });
        let err = build_args_json(&schema, &[s("--color"), s("purple")]).unwrap_err();
        assert_eq!(
            err,
            "Error: --color: must be one of [\"red\", \"green\", \"blue\"], got \"purple\""
        );
    }

    #[test]
    fn unknown_flag() {
        let schema = json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        });
        let err = build_args_json(&schema, &[s("--mystery"), s("foo")]).unwrap_err();
        assert_eq!(err, "Error: --mystery: unknown flag");
    }

    #[test]
    fn negative_integer_accepted() {
        let schema = json!({
            "type": "object",
            "properties": { "count": { "type": "integer" } },
            "required": ["count"],
            "additionalProperties": false
        });
        let json_str = build_args_json(&schema, &[s("--count"), s("-3")]).unwrap();
        let v: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(v, json!({ "count": -3 }));
    }

    #[test]
    fn empty_argv_with_no_required() {
        let schema = json!({
            "type": "object",
            "properties": {},
            "additionalProperties": true
        });
        let json_str = build_args_json(&schema, &[]).unwrap();
        let v: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(v, json!({}));
    }

    #[test]
    fn fail_fast_first_violation() {
        let schema = json!({
            "type": "object",
            "properties": {
                "count": { "type": "integer" },
                "ratio": { "type": "number" }
            },
            "required": ["count", "ratio"],
            "additionalProperties": false
        });
        let err = build_args_json(
            &schema,
            &[s("--count"), s("abc"), s("--ratio"), s("xyz")],
        )
        .unwrap_err();
        assert_eq!(err, "Error: --count: expected integer, got \"abc\"");
    }

    #[test]
    fn stdin_provides_required_field_no_cli_flag() {
        let schema = json!({
            "type": "object",
            "properties": {
                "userName": { "type": "string" },
                "count": { "type": "integer" }
            },
            "required": ["userName", "count"],
            "additionalProperties": false
        });
        let stdin = json!({ "userName": "alice", "count": 7 });
        let json_str = build_args_json_with_stdin(&schema, &[], &stdin).unwrap();
        let v: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(v, json!({ "userName": "alice", "count": 7 }));
    }

    #[test]
    fn stdin_overridden_by_cli_flag() {
        let schema = json!({
            "type": "object",
            "properties": {
                "userName": { "type": "string" },
                "count": { "type": "integer" }
            },
            "required": ["userName", "count"]
        });
        let stdin = json!({ "userName": "alice", "count": 7 });
        let argv = vec![s("--user-name"), s("bob")];
        let json_str = build_args_json_with_stdin(&schema, &argv, &stdin).unwrap();
        let v: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(v, json!({ "userName": "bob", "count": 7 }));
    }

    #[test]
    fn stdin_violates_input_schema_type() {
        let schema = json!({
            "type": "object",
            "properties": {
                "count": { "type": "integer" }
            },
            "required": ["count"]
        });
        let stdin = json!({ "count": "not-a-number" });
        let err = build_args_json_with_stdin(&schema, &[], &stdin).unwrap_err();
        assert_eq!(
            err,
            "Error: --count: expected integer, got \"not-a-number\""
        );
    }

    #[test]
    fn stdin_missing_required_after_merge() {
        let schema = json!({
            "type": "object",
            "properties": {
                "userName": { "type": "string" },
                "count": { "type": "integer" }
            },
            "required": ["userName", "count"]
        });
        let stdin = json!({ "userName": "alice" });
        let err = build_args_json_with_stdin(&schema, &[], &stdin).unwrap_err();
        assert_eq!(err, "Error: --count: required");
    }

    #[test]
    fn stdin_empty_object_uses_cli_flags_only() {
        let schema = json!({
            "type": "object",
            "properties": {
                "userName": { "type": "string" }
            },
            "required": ["userName"]
        });
        let stdin = json!({});
        let argv = vec![s("--user-name"), s("alice")];
        let json_str = build_args_json_with_stdin(&schema, &argv, &stdin).unwrap();
        let v: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(v, json!({ "userName": "alice" }));
    }

    #[test]
    fn stdin_non_object_rejected() {
        let schema = json!({ "type": "object", "properties": {} });
        let stdin = json!([1, 2, 3]);
        let err = build_args_json_with_stdin(&schema, &[], &stdin).unwrap_err();
        assert_eq!(err, "Error: stdin: expected JSON object, got [1, 2, 3]");
    }
}
