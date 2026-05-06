use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::{Value, json};
use wasmtime::Engine;

use super::server::ToolHandler;

pub struct SkillsHandler {
    engine: Engine,
    skills: Vec<SkillEntry>,
}

struct SkillEntry {
    name: String,
    description: String,
    input_schema: Value,
    skill_source: String,
    schema_source: String,
}

impl SkillsHandler {
    pub fn new(engine: &Engine) -> anyhow::Result<Self> {
        let dir = user_skills_dir()?;
        let entries = scan_skills_dir(engine, &dir);
        Ok(Self {
            engine: engine.clone(),
            skills: entries,
        })
    }
}

impl ToolHandler for SkillsHandler {
    fn tool_specs(&self) -> Value {
        let specs: Vec<Value> = self
            .skills
            .iter()
            .map(|s| {
                json!({
                    "name": s.name,
                    "description": s.description,
                    "inputSchema": s.input_schema,
                })
            })
            .collect();
        Value::Array(specs)
    }

    fn call(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        let skill = self
            .skills
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| format!("unknown skill: {name}"))?;

        let args_json = args.to_string();
        match crate::run_skill_for_mcp(
            &self.engine,
            &skill.skill_source,
            &skill.schema_source,
            &args_json,
        ) {
            Ok(stdout) => Ok(text_result(&stdout, false)),
            Err(msg) => Ok(text_result(&msg, true)),
        }
    }
}

fn user_skills_dir() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().context("failed to determine home directory")?;
    Ok(home.join(".skill-forge").join("skills"))
}

fn scan_skills_dir(engine: &Engine, dir: &Path) -> Vec<SkillEntry> {
    let read_dir = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            eprintln!(
                "[skill-forge] skills mode: failed to read {}: {e}",
                dir.display()
            );
            return Vec::new();
        }
    };

    let mut entries = Vec::new();
    for ent in read_dir.flatten() {
        let path = ent.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        match load_skill_entry(engine, &path, &name) {
            Ok(entry) => entries.push(entry),
            Err(reason) => {
                eprintln!("[skill-forge] skipping skill '{name}': {reason}");
            }
        }
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn load_skill_entry(engine: &Engine, dir: &Path, name: &str) -> Result<SkillEntry, String> {
    let skill_path = dir.join("skill.js");
    let schema_path = dir.join("schema.js");
    let description_path = dir.join("DESCRIPTION.md");

    let skill_source = fs::read_to_string(&skill_path)
        .map_err(|e| format!("missing or unreadable skill.js: {e}"))?;
    let schema_source = fs::read_to_string(&schema_path)
        .map_err(|e| format!("missing or unreadable schema.js: {e}"))?;
    let description = fs::read_to_string(&description_path)
        .map_err(|e| format!("missing or unreadable DESCRIPTION.md: {e}"))?;

    let schema_envelope = crate::evaluate_schema_for_mcp(engine, &skill_source, &schema_source)
        .map_err(|e| format!("schema.js evaluation failed: {e}"))?;

    let input_schema = schema_envelope
        .get("input")
        .cloned()
        .ok_or_else(|| "schema envelope missing 'input'".to_string())?;

    Ok(SkillEntry {
        name: name.to_string(),
        description,
        input_schema,
        skill_source,
        schema_source,
    })
}

fn text_result(text: &str, is_error: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_entry(name: &str) -> SkillEntry {
        SkillEntry {
            name: name.to_string(),
            description: format!("desc for {name}\n"),
            input_schema: json!({ "type": "object" }),
            skill_source: String::new(),
            schema_source: String::new(),
        }
    }

    #[test]
    fn text_result_shape_is_mcp_compatible() {
        let v = text_result("hello", false);
        assert_eq!(v["isError"], json!(false));
        assert_eq!(v["content"][0]["type"], json!("text"));
        assert_eq!(v["content"][0]["text"], json!("hello"));
    }

    #[test]
    fn text_result_marks_errors() {
        let v = text_result("boom", true);
        assert_eq!(v["isError"], json!(true));
    }

    #[test]
    fn tool_specs_maps_skill_entry_fields() {
        let handler = SkillsHandler {
            engine: wasmtime::Engine::default(),
            skills: vec![make_entry("alpha"), make_entry("bravo")],
        };
        let specs = handler.tool_specs();
        let arr = specs.as_array().expect("array");
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "alpha");
        assert_eq!(arr[0]["description"], "desc for alpha\n");
        assert_eq!(arr[0]["inputSchema"]["type"], "object");
        assert_eq!(arr[1]["name"], "bravo");
    }

    #[test]
    fn scan_skips_dir_with_missing_description() {
        let tmp = std::env::temp_dir().join(format!(
            "skill-forge-skills-scan-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dir = tmp.join("missing-desc");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("skill.js"), "defineSkill(async () => ({}));\n").unwrap();
        fs::write(dir.join("schema.js"), "defineSchema({ type: 'object' });\n").unwrap();
        // intentionally no DESCRIPTION.md

        let engine = wasmtime::Engine::default();
        let entries = scan_skills_dir(&engine, &tmp);
        assert!(entries.is_empty(), "expected skill to be skipped");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn scan_returns_empty_when_root_missing() {
        let tmp = std::env::temp_dir().join(format!(
            "skill-forge-skills-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let engine = wasmtime::Engine::default();
        let entries = scan_skills_dir(&engine, &tmp);
        assert!(entries.is_empty());
    }
}
