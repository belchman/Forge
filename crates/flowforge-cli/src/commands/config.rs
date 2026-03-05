use colored::Colorize;
use flowforge_core::config::FlowForgeConfig;
use flowforge_core::Result;

/// Pretty-print the resolved config with defaults highlighted.
pub fn show() -> Result<()> {
    let config_path = FlowForgeConfig::config_path();
    let config = FlowForgeConfig::load(&config_path)?;

    let has_file = config_path.exists();
    println!("{}", "FlowForge Configuration".bold());
    println!("{}", "-".repeat(50));
    if has_file {
        println!("Source: {}", config_path.display().to_string().cyan());
    } else {
        println!("Source: {} (using defaults)", "no config file".yellow());
    }
    println!();

    let toml_str = toml::to_string_pretty(&config)
        .map_err(|e| flowforge_core::Error::Config(format!("Failed to serialize config: {e}")))?;

    // If we have a file on disk, compare against defaults to highlight overrides
    let default_toml = toml::to_string_pretty(&FlowForgeConfig::default())
        .map_err(|e| flowforge_core::Error::Config(format!("Failed to serialize defaults: {e}")))?;

    let default_lines: std::collections::HashSet<&str> = default_toml.lines().collect();

    for line in toml_str.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            println!();
        } else if trimmed.starts_with('[') {
            println!("{}", line.bold());
        } else if has_file && !default_lines.contains(line) {
            // This value differs from the default
            println!("{} {}", line, "(custom)".green());
        } else {
            println!("{}", line);
        }
    }

    Ok(())
}

/// Look up a config value by dot-notation key (e.g., `patterns.short_term_max`).
pub fn get(key: &str) -> Result<()> {
    let config_path = FlowForgeConfig::config_path();
    let config = FlowForgeConfig::load(&config_path)?;

    let value = lookup_value(&config, key)?;
    println!("{value}");
    Ok(())
}

/// Validate and persist a config value by dot-notation key.
pub fn set(key: &str, value: &str) -> Result<()> {
    let config_path = FlowForgeConfig::config_path();
    let mut config = FlowForgeConfig::load(&config_path)?;

    apply_value(&mut config, key, value)?;
    config.validate()?;
    config.save(&config_path)?;

    println!("Set {} = {}", key.cyan(), value.green());
    Ok(())
}

/// Look up a config field by dot-notation key, returning its display string.
fn lookup_value(config: &FlowForgeConfig, key: &str) -> Result<String> {
    // Serialize to a TOML Value tree, then navigate with dot notation
    let toml_val: toml::Value = toml::Value::try_from(config)
        .map_err(|e| flowforge_core::Error::Config(format!("serialize error: {e}")))?;

    let parts: Vec<&str> = key.split('.').collect();
    let mut current = &toml_val;

    for part in &parts {
        match current.get(part) {
            Some(v) => current = v,
            None => {
                return Err(flowforge_core::Error::Config(format!(
                    "Unknown config key: {key}"
                )));
            }
        }
    }

    Ok(format_toml_value(current))
}

/// Apply a string value to the config at the given dot-notation key.
fn apply_value(config: &mut FlowForgeConfig, key: &str, value: &str) -> Result<()> {
    // Serialize config to TOML Value, modify the tree, then deserialize back
    let mut toml_val: toml::Value = toml::Value::try_from(&*config)
        .map_err(|e| flowforge_core::Error::Config(format!("serialize error: {e}")))?;

    let parts: Vec<&str> = key.split('.').collect();
    if parts.is_empty() {
        return Err(flowforge_core::Error::Config("Empty key".to_string()));
    }

    // Navigate to the parent table
    let mut current = &mut toml_val;
    for part in &parts[..parts.len() - 1] {
        current = current.get_mut(part).ok_or_else(|| {
            flowforge_core::Error::Config(format!("Unknown config section: {part}"))
        })?;
    }

    let field = parts[parts.len() - 1];

    // Get the existing value to determine expected type
    let existing = current
        .get(field)
        .ok_or_else(|| flowforge_core::Error::Config(format!("Unknown config key: {key}")))?;

    let new_val = parse_value_matching_type(existing, value)
        .map_err(|msg| flowforge_core::Error::Config(format!("Invalid value for {key}: {msg}")))?;

    current
        .as_table_mut()
        .ok_or_else(|| flowforge_core::Error::Config(format!("{key} parent is not a table")))?
        .insert(field.to_string(), new_val);

    // Deserialize back
    let updated: FlowForgeConfig = toml_val.try_into().map_err(|e: toml::de::Error| {
        flowforge_core::Error::Config(format!("Config update failed: {e}"))
    })?;

    *config = updated;
    Ok(())
}

/// Parse a string value to match the type of an existing TOML value.
fn parse_value_matching_type(
    existing: &toml::Value,
    input: &str,
) -> std::result::Result<toml::Value, String> {
    match existing {
        toml::Value::Boolean(_) => match input {
            "true" => Ok(toml::Value::Boolean(true)),
            "false" => Ok(toml::Value::Boolean(false)),
            _ => Err(format!("expected 'true' or 'false', got '{input}'")),
        },
        toml::Value::Integer(_) => input
            .parse::<i64>()
            .map(toml::Value::Integer)
            .map_err(|_| format!("expected integer, got '{input}'")),
        toml::Value::Float(_) => input
            .parse::<f64>()
            .map(toml::Value::Float)
            .map_err(|_| format!("expected number, got '{input}'")),
        toml::Value::String(_) => Ok(toml::Value::String(input.to_string())),
        toml::Value::Array(_) => {
            // Accept comma-separated values as a string array
            let items: Vec<toml::Value> = input
                .split(',')
                .map(|s| toml::Value::String(s.trim().to_string()))
                .collect();
            Ok(toml::Value::Array(items))
        }
        _ => Err(format!(
            "cannot set values of type {:?}",
            existing.type_str()
        )),
    }
}

/// Format a TOML value for display.
fn format_toml_value(val: &toml::Value) -> String {
    match val {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(n) => n.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_toml_value).collect();
            items.join(", ")
        }
        toml::Value::Table(_) => toml::to_string_pretty(val).unwrap_or_else(|_| format!("{val:?}")),
        toml::Value::Datetime(dt) => dt.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_valid_key() {
        let config = FlowForgeConfig::default();
        let val = lookup_value(&config, "patterns.short_term_max").unwrap();
        assert_eq!(val, "500");
    }

    #[test]
    fn test_lookup_invalid_key() {
        let config = FlowForgeConfig::default();
        let result = lookup_value(&config, "patterns.nonexistent_field");
        assert!(result.is_err());
    }

    #[test]
    fn test_lookup_nested_key() {
        let config = FlowForgeConfig::default();
        let val =
            lookup_value(&config, "work_tracking.work_stealing.stale_threshold_mins").unwrap();
        assert_eq!(val, "30");
    }

    #[test]
    fn test_apply_integer_value() {
        let mut config = FlowForgeConfig::default();
        apply_value(&mut config, "patterns.short_term_max", "1000").unwrap();
        assert_eq!(config.patterns.short_term_max, 1000);
    }

    #[test]
    fn test_apply_float_value() {
        let mut config = FlowForgeConfig::default();
        apply_value(&mut config, "patterns.min_injection_similarity", "0.75").unwrap();
        assert!((config.patterns.min_injection_similarity - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_apply_bool_value() {
        let mut config = FlowForgeConfig::default();
        apply_value(&mut config, "guidance.enabled", "false").unwrap();
        assert!(!config.guidance.enabled);
    }

    #[test]
    fn test_apply_string_value() {
        let mut config = FlowForgeConfig::default();
        apply_value(&mut config, "general.log_level", "debug").unwrap();
        assert_eq!(config.general.log_level, "debug");
    }

    #[test]
    fn test_apply_invalid_bool() {
        let mut config = FlowForgeConfig::default();
        let result = apply_value(&mut config, "guidance.enabled", "maybe");
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_invalid_integer() {
        let mut config = FlowForgeConfig::default();
        let result = apply_value(&mut config, "patterns.short_term_max", "not_a_number");
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_unknown_key() {
        let mut config = FlowForgeConfig::default();
        let result = apply_value(&mut config, "patterns.no_such_field", "42");
        assert!(result.is_err());
    }

    #[test]
    fn test_roundtrip_set_then_get() {
        let mut config = FlowForgeConfig::default();
        apply_value(&mut config, "patterns.short_term_max", "999").unwrap();
        let val = lookup_value(&config, "patterns.short_term_max").unwrap();
        assert_eq!(val, "999");
    }

    #[test]
    fn test_apply_validates_config() {
        let mut config = FlowForgeConfig::default();
        // Setting trust score out of range should fail validation in `set()`,
        // but apply_value itself doesn't validate — that's done by the caller.
        // So we test the apply + validate flow:
        apply_value(&mut config, "guidance.trust_initial_score", "1.5").unwrap();
        assert!(config.validate().is_err());
    }
}
