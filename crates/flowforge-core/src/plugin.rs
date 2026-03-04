//! Plugin SDK: TOML-based extensions for custom tools, hooks, and agents.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::config::{FlowForgeConfig, PluginsConfig};
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginMeta,
    #[serde(default)]
    pub tools: Vec<PluginToolDef>,
    #[serde(default)]
    pub hooks: Vec<PluginHookDef>,
    #[serde(default)]
    pub agents: Vec<PluginAgentRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub description: String,
}

fn default_version() -> String {
    "0.1.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginToolDef {
    pub name: String,
    pub description: String,
    pub command: String,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default)]
    pub input_schema: Option<String>,
}

fn default_timeout() -> u64 {
    5000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHookDef {
    pub event: String,
    pub command: String,
    #[serde(default = "default_priority")]
    pub priority: i32,
}

fn default_priority() -> i32 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAgentRef {
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub dir: PathBuf,
}

/// Check if a plugin is enabled given the config.
pub fn is_plugin_enabled(name: &str, config: &PluginsConfig) -> bool {
    // Disabled list takes precedence
    if config.disabled.iter().any(|d| d == name) {
        return false;
    }
    // If enabled list is empty, all are enabled
    if config.enabled.is_empty() {
        return true;
    }
    config.enabled.iter().any(|e| e == name)
}

/// Load a single plugin from a directory.
pub fn load_plugin(dir: &Path) -> Result<LoadedPlugin> {
    let manifest_path = dir.join("plugin.toml");
    if !manifest_path.exists() {
        return Err(crate::Error::Plugin(format!(
            "No plugin.toml found in {}",
            dir.display()
        )));
    }

    let content = std::fs::read_to_string(&manifest_path)?;
    let manifest: PluginManifest = toml::from_str(&content)
        .map_err(|e| crate::Error::Plugin(format!("Invalid plugin.toml: {e}")))?;

    Ok(LoadedPlugin {
        manifest,
        dir: dir.to_path_buf(),
    })
}

/// Load all plugins from the plugins directory.
pub fn load_all_plugins(config: &PluginsConfig) -> Result<Vec<LoadedPlugin>> {
    let plugins_dir = FlowForgeConfig::plugins_dir();
    if !plugins_dir.exists() {
        return Ok(vec![]);
    }

    let mut plugins = Vec::new();
    let entries = std::fs::read_dir(&plugins_dir)?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        match load_plugin(&path) {
            Ok(plugin) => {
                if is_plugin_enabled(&plugin.manifest.plugin.name, config) {
                    plugins.push(plugin);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to load plugin from {}: {e}", path.display());
            }
        }
    }

    Ok(plugins)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_load_plugin_valid_manifest() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("plugin.toml"),
            r#"
[plugin]
name = "test-plugin"
description = "A test plugin"

[[tools]]
name = "hello"
description = "Says hello"
command = "echo hello"
"#,
        )
        .unwrap();

        let plugin = load_plugin(dir.path()).unwrap();
        assert_eq!(plugin.manifest.plugin.name, "test-plugin");
        assert_eq!(plugin.manifest.tools.len(), 1);
        assert_eq!(plugin.manifest.tools[0].name, "hello");
        assert_eq!(plugin.manifest.tools[0].timeout, 5000); // default
    }

    #[test]
    fn test_load_plugin_missing_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let result = load_plugin(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_is_plugin_enabled_default() {
        let config = PluginsConfig::default();
        assert!(is_plugin_enabled("any-plugin", &config));
    }

    #[test]
    fn test_is_plugin_enabled_disabled() {
        let config = PluginsConfig {
            enabled: vec![],
            disabled: vec!["blocked".to_string()],
        };
        assert!(!is_plugin_enabled("blocked", &config));
        assert!(is_plugin_enabled("other", &config));
    }

    #[test]
    fn test_is_plugin_enabled_explicit_list() {
        let config = PluginsConfig {
            enabled: vec!["allowed".to_string()],
            disabled: vec![],
        };
        assert!(is_plugin_enabled("allowed", &config));
        assert!(!is_plugin_enabled("other", &config));
    }
}
