use std::path::Path;

use flowforge_core::{AgentDef, AgentSource, Error, Result};
use tracing::warn;

include!(concat!(env!("OUT_DIR"), "/builtin_agents.rs"));

/// Parse a single agent definition from markdown content with YAML frontmatter.
///
/// The expected format is:
/// ```text
/// ---
/// name: my-agent
/// description: Does things
/// capabilities: [rust, testing]
/// patterns: ["test.*"]
/// priority: normal
/// ---
/// # Agent body in markdown
/// ```
pub fn parse_agent_def(content: &str) -> Result<AgentDef> {
    let content = content.trim_start_matches('\u{feff}'); // strip BOM

    if !content.starts_with("---") {
        return Err(Error::Agent(
            "Agent definition must start with YAML frontmatter (---)".to_string(),
        ));
    }

    // Find the closing `---` delimiter (skip the opening one)
    let after_open = &content[3..];
    let close_pos = after_open
        .find("\n---")
        .ok_or_else(|| Error::Agent("Missing closing --- for YAML frontmatter".to_string()))?;

    let yaml_str = &after_open[..close_pos];
    // Body starts after the closing `---` and its newline
    let body_start = 3 + close_pos + 4; // opening "---" + yaml + "\n---"
    let body = if body_start < content.len() {
        content[body_start..].trim().to_string()
    } else {
        String::new()
    };

    let mut agent_def: AgentDef = serde_yaml::from_str(yaml_str)
        .map_err(|e| Error::Agent(format!("Invalid YAML frontmatter: {e}")))?;

    agent_def.body = body;

    Ok(agent_def)
}

/// Load all `.md` agent definitions from a directory, recursively walking subdirectories.
/// Non-parseable files are warned about and skipped.
pub fn load_from_dir(path: &Path, source: AgentSource) -> Result<Vec<AgentDef>> {
    if !path.is_dir() {
        return Ok(Vec::new());
    }

    let mut agents = Vec::new();
    load_from_dir_recursive(path, &source, &mut agents)?;
    Ok(agents)
}

fn load_from_dir_recursive(
    path: &Path,
    source: &AgentSource,
    agents: &mut Vec<AgentDef>,
) -> Result<()> {
    let entries = std::fs::read_dir(path).map_err(|e| {
        Error::Agent(format!(
            "Failed to read agent directory {}: {e}",
            path.display()
        ))
    })?;

    for entry in entries {
        let entry = entry.map_err(Error::Io)?;
        let file_path = entry.path();

        if file_path.is_dir() {
            load_from_dir_recursive(&file_path, source, agents)?;
            continue;
        }

        if file_path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to read agent file {}: {e}", file_path.display());
                continue;
            }
        };

        match parse_agent_def(&content) {
            Ok(mut agent) => {
                agent.source = source.clone();
                agents.push(agent);
            }
            Err(e) => {
                warn!("Failed to parse agent file {}: {e}", file_path.display());
            }
        }
    }

    Ok(())
}

/// Load built-in agent definitions compiled into the binary.
///
/// These agents are embedded at build time from the `agents/` directory
/// via `include_str!` in the build script.
pub fn load_builtin() -> Vec<AgentDef> {
    let mut agents = Vec::new();
    for (name, content) in builtin_agents() {
        match parse_agent_def(content) {
            Ok(mut agent) => {
                agent.source = AgentSource::BuiltIn;
                agents.push(agent);
            }
            Err(e) => {
                warn!("Failed to parse built-in agent '{}': {e}", name);
            }
        }
    }
    agents
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agent_def_basic() {
        let content = concat!(
            "---\n",
            "name: test-agent\n",
            "description: A test agent\n",
            "capabilities:\n",
            "  - rust\n",
            "  - testing\n",
            "patterns:\n",
            "  - \"test.*\"\n",
            "priority: high\n",
            "color: \"#ff0000\"\n",
            "---\n",
            "# Test Agent\n",
            "\n",
            "This agent does testing things.\n",
        );
        let agent = parse_agent_def(content).unwrap();
        assert_eq!(agent.name, "test-agent");
        assert_eq!(agent.description, "A test agent");
        assert_eq!(agent.capabilities, vec!["rust", "testing"]);
        assert_eq!(agent.patterns, vec!["test.*"]);
        assert_eq!(agent.priority, flowforge_core::Priority::High);
        assert_eq!(agent.color.as_deref(), Some("#ff0000"));
        assert!(agent.body.contains("# Test Agent"));
    }

    #[test]
    fn test_parse_agent_def_minimal() {
        let content = "---\nname: minimal\ndescription: Minimal agent\n---\n";
        let agent = parse_agent_def(content).unwrap();
        assert_eq!(agent.name, "minimal");
        assert!(agent.capabilities.is_empty());
        assert!(agent.patterns.is_empty());
        assert_eq!(agent.priority, flowforge_core::Priority::Normal);
        assert!(agent.body.is_empty());
    }

    #[test]
    fn test_parse_agent_def_no_frontmatter() {
        let content = "# Just markdown\nNo frontmatter here.";
        assert!(parse_agent_def(content).is_err());
    }

    #[test]
    fn test_parse_agent_def_unclosed_frontmatter() {
        let content = "---\nname: broken\n";
        assert!(parse_agent_def(content).is_err());
    }
}
