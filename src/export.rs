//! State management for `forge export`.
//!
//! The on-disk manifest is owned here. The actual SKILL.md / DESCRIPTION.md
//! rendering lives in the `render-skill-md` builtin tool (TypeScript) so the
//! templating logic stays close to the agent runtime and can be reused by
//! future per-target tools without going through Rust.

use std::fmt;
use std::path::PathBuf;

use serde_json::{Value, json};

/// Bumped any time the on-disk manifest layout changes incompatibly.
/// Older readers reject unknown versions rather than guess.
pub const MANIFEST_VERSION: u64 = 1;

#[derive(Debug)]
#[allow(dead_code)] // ManifestVersionUnsupported / ManifestMalformed surface in Phase 6
pub enum ExportError {
    ManifestVersionUnsupported { found: u64, expected: u64 },
    ManifestMalformed { reason: String },
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::ManifestVersionUnsupported { found, expected } => write!(
                f,
                "manifest version {found} is not supported (expected {expected}). \
                 Delete ~/.skill-forge/exports/.manifest.json to regenerate."
            ),
            ExportError::ManifestMalformed { reason } => {
                write!(f, "manifest is malformed: {reason}")
            }
        }
    }
}

impl std::error::Error for ExportError {}

/// One placement we have created in a previous `forge export` run. Persisted
/// in the manifest so the next run can reconcile orphans without rescanning
/// every potential target directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestPlacement {
    pub target: String,
    pub name: String,
    pub dest: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct ExportManifest {
    pub skills: Vec<String>,
    pub placements: Vec<ManifestPlacement>,
}

impl ExportManifest {
    pub fn to_json(&self) -> Value {
        let placements: Vec<Value> = self
            .placements
            .iter()
            .map(|p| {
                json!({
                    "target": p.target,
                    "name": p.name,
                    "dest": p.dest.to_string_lossy(),
                })
            })
            .collect();
        json!({
            "version": MANIFEST_VERSION,
            "skills": self.skills,
            "placements": placements,
        })
    }

    /// Parse the on-disk JSON. Unknown versions or malformed shapes return
    /// `ExportError` so the host can surface a clean message to the user.
    #[allow(dead_code)] // surfaced in Phase 6 reconciliation
    pub fn from_json_str(s: &str) -> Result<Self, ExportError> {
        let v: Value = serde_json::from_str(s).map_err(|e| ExportError::ManifestMalformed {
            reason: e.to_string(),
        })?;
        let version = v
            .get("version")
            .and_then(|x| x.as_u64())
            .ok_or_else(|| ExportError::ManifestMalformed {
                reason: "missing 'version' field".into(),
            })?;
        if version != MANIFEST_VERSION {
            return Err(ExportError::ManifestVersionUnsupported {
                found: version,
                expected: MANIFEST_VERSION,
            });
        }
        let skills: Vec<String> = v
            .get("skills")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let placements: Vec<ManifestPlacement> = v
            .get("placements")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| {
                        let target = p.get("target")?.as_str()?.to_string();
                        let name = p.get("name")?.as_str()?.to_string();
                        let dest = PathBuf::from(p.get("dest")?.as_str()?);
                        Some(ManifestPlacement { target, name, dest })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(ExportManifest { skills, placements })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_roundtrips_through_json() {
        let m = ExportManifest {
            skills: vec!["a".into(), "b".into()],
            placements: vec![ManifestPlacement {
                target: "claude-code".into(),
                name: "a".into(),
                dest: PathBuf::from("/home/x/.claude/skills/a"),
            }],
        };
        let json_str = serde_json::to_string(&m.to_json()).unwrap();
        let parsed = ExportManifest::from_json_str(&json_str).unwrap();
        assert_eq!(parsed, m);
    }

    #[test]
    fn manifest_rejects_unknown_version() {
        let body = r#"{"version": 99, "skills": [], "placements": []}"#;
        let err = ExportManifest::from_json_str(body).unwrap_err();
        match err {
            ExportError::ManifestVersionUnsupported { found, expected } => {
                assert_eq!(found, 99);
                assert_eq!(expected, MANIFEST_VERSION);
            }
            other => panic!("expected ManifestVersionUnsupported, got {other:?}"),
        }
    }

    #[test]
    fn manifest_rejects_malformed_json() {
        let err = ExportManifest::from_json_str("not json").unwrap_err();
        assert!(matches!(err, ExportError::ManifestMalformed { .. }));
    }
}
