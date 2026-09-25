//! Conservative source-project admission for development extensions.

use std::io::{Read, sink};

use anyhow::{Context, Result, ensure};
use serde_json::Value;

const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

#[derive(Default)]
pub(super) struct SourceProject {
    package_json: Option<Vec<u8>>,
    cargo_toml: Option<Vec<u8>>,
    has_app_source: bool,
    has_discovery_config: bool,
}

impl SourceProject {
    /// Consumes a verified regular-file entry and records only root manifests.
    pub(super) fn read_file(&mut self, path: &str, entry: &mut impl Read) -> Result<u64> {
        self.has_app_source |= path == "app" || path.starts_with("app/");
        self.has_discovery_config |= path == "lenso.toml";
        let slot = match path {
            "package.json" => Some(&mut self.package_json),
            "Cargo.toml" => Some(&mut self.cargo_toml),
            _ => None,
        };
        if let Some(slot) = slot {
            let mut bytes = Vec::new();
            let count = entry.take(MAX_MANIFEST_BYTES + 1).read_to_end(&mut bytes)?;
            ensure!(
                count as u64 <= MAX_MANIFEST_BYTES,
                "development extension root manifest exceeds size limit"
            );
            *slot = Some(bytes);
            Ok(count as u64)
        } else {
            Ok(std::io::copy(entry, &mut sink())?)
        }
    }

    pub(super) fn validate(&self, plugin_id: &str, version: &str) -> Result<()> {
        // The Engine also discovers app/ and paths named in lenso.toml. Reject
        // them here to keep this archive-only check a sound subset of discovery.
        ensure!(
            !self.has_app_source && !self.has_discovery_config,
            "development extension must contain one root Plugin source project"
        );
        match (&self.package_json, &self.cargo_toml) {
            (Some(bytes), None) => validate_bun(bytes, plugin_id, version),
            (None, Some(bytes)) => validate_cargo(bytes, plugin_id, version),
            _ => anyhow::bail!(
                "development extension needs exactly one root package.json or Cargo.toml"
            ),
        }
    }
}

fn nonempty_conventions(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
}

fn validate_bun(bytes: &[u8], plugin_id: &str, version: &str) -> Result<()> {
    let manifest: Value = serde_json::from_slice(bytes).context("parse extension package.json")?;
    let lenso = manifest
        .get("lenso")
        .context("extension needs lenso metadata")?;
    ensure!(
        manifest.get("version").and_then(Value::as_str) == Some(version)
            && lenso.get("pluginId").and_then(Value::as_str) == Some(plugin_id)
            && lenso.get("runtime").and_then(Value::as_str) == Some("bun")
            && nonempty_conventions(lenso.get("conventions")),
        "development extension package.json differs from signed Plugin ID/version or lacks conventions"
    );
    ensure!(
        lenso
            .get("published_resources")
            .is_none_or(|resources| { resources.as_array().is_some_and(Vec::is_empty) }),
        "development extension cannot declare published resources"
    );
    Ok(())
}

fn validate_cargo(bytes: &[u8], plugin_id: &str, version: &str) -> Result<()> {
    let manifest: toml::Value =
        toml::from_str(std::str::from_utf8(bytes).context("extension Cargo.toml must be UTF-8")?)
            .context("parse extension Cargo.toml")?;
    let manifest = serde_json::to_value(manifest)?;
    let lenso = manifest
        .pointer("/package/metadata/lenso")
        .context("extension needs package.metadata.lenso")?;
    ensure!(
        manifest.pointer("/package/version").and_then(Value::as_str) == Some(version)
            && lenso.get("plugin-id").and_then(Value::as_str) == Some(plugin_id)
            && nonempty_conventions(lenso.get("conventions")),
        "development extension Cargo.toml differs from signed Plugin ID/version or lacks conventions"
    );
    ensure!(
        manifest.pointer("/package/metadata/lenso-cli").is_none(),
        "development extension Cargo source needs simple root metadata"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::SourceProject;

    const ID: &str = "lenso.auth.api-token";
    const VERSION: &str = "0.1.1";
    const BUN: &str = r#"{"name":"lenso-auth-api-token-dev-extension","version":"0.1.1","type":"module","lenso":{"pluginId":"lenso.auth.api-token","runtime":"bun","rootSlot":"tools","source":"index.ts","conventions":[{"id":"lenso.auth.api-token.compiler","entries":["page.tsx"],"compiler":{"program":"bun","args":["compiler.mjs"]}}]}}"#;

    fn source(path: &str, contents: &str) -> SourceProject {
        let mut source = SourceProject::default();
        source
            .read_file(path, &mut Cursor::new(contents.as_bytes()))
            .unwrap();
        source
    }

    #[test]
    fn accepts_root_bun_plugin_with_matching_identity_and_conventions() {
        assert!(source("package.json", BUN).validate(ID, VERSION).is_ok());
    }

    #[test]
    fn rejects_bun_plugin_with_wrong_version() {
        assert!(source("package.json", BUN).validate(ID, "0.1.2").is_err());
    }

    #[test]
    fn rejects_bun_plugin_with_wrong_identity() {
        assert!(
            source("package.json", BUN)
                .validate("lenso.auth.other", VERSION)
                .is_err()
        );
    }

    #[test]
    fn rejects_bun_plugin_without_conventions() {
        let mut manifest: serde_json::Value = serde_json::from_str(BUN).unwrap();
        manifest["lenso"]["conventions"] = serde_json::json!([]);
        let manifest = serde_json::to_string(&manifest).unwrap();
        assert!(
            source("package.json", &manifest)
                .validate(ID, VERSION)
                .is_err()
        );
    }

    #[test]
    fn accepts_simple_root_cargo_plugin_with_conventions() {
        let manifest = r#"
[package]
name = "auth-extension"
version = "0.1.1"
[package.metadata.lenso]
plugin-id = "lenso.auth.api-token"
conventions = [{ id = "compiler", entries = ["page.rs"] }]
"#;
        source("Cargo.toml", manifest)
            .validate(ID, VERSION)
            .unwrap();
    }

    #[test]
    fn rejects_additional_discovery_roots() {
        let mut source = source("package.json", BUN);
        source
            .read_file("app/nested/package.json", &mut Cursor::new(b"{}"))
            .unwrap();
        assert!(source.validate(ID, VERSION).is_err());
    }

    #[test]
    fn rejects_explicit_workspace_source_configuration() {
        let mut source = source("package.json", BUN);
        source
            .read_file("lenso.toml", &mut Cursor::new(b"plugin_sources = []"))
            .unwrap();
        assert!(source.validate(ID, VERSION).is_err());
    }

    #[test]
    fn rejects_ambiguous_root_manifests() {
        let mut source = source("package.json", BUN);
        source
            .read_file("Cargo.toml", &mut Cursor::new(b"[package]\n"))
            .unwrap();
        assert!(source.validate(ID, VERSION).is_err());
    }
}
