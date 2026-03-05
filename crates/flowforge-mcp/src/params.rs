use serde_json::Value;

/// Extension trait for extracting typed parameters from `serde_json::Value`.
pub trait ParamExt {
    fn str_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str;
    fn opt_str(&self, key: &str) -> Option<&str>;
    fn u64_or(&self, key: &str, default: u64) -> u64;
    fn i64_or(&self, key: &str, default: i64) -> i64;
    fn bool_or(&self, key: &str, default: bool) -> bool;
    fn opt_i64(&self, key: &str) -> Option<i64>;
    fn opt_u32(&self, key: &str) -> Option<u32>;
    /// Require a string parameter. Returns Err with a descriptive error if missing or wrong type.
    fn require_str(&self, key: &str) -> Result<&str, flowforge_core::Error>;
}

impl ParamExt for Value {
    fn str_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.get(key).and_then(|v| v.as_str()).unwrap_or(default)
    }

    fn opt_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_str())
    }

    fn u64_or(&self, key: &str, default: u64) -> u64 {
        self.get(key).and_then(|v| v.as_u64()).unwrap_or(default)
    }

    fn i64_or(&self, key: &str, default: i64) -> i64 {
        self.get(key).and_then(|v| v.as_i64()).unwrap_or(default)
    }

    fn bool_or(&self, key: &str, default: bool) -> bool {
        self.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
    }

    fn opt_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_i64())
    }

    fn opt_u32(&self, key: &str) -> Option<u32> {
        self.get(key).and_then(|v| v.as_u64()).map(|n| n as u32)
    }

    fn require_str(&self, key: &str) -> Result<&str, flowforge_core::Error> {
        self.get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                flowforge_core::Error::Config(format!("missing required parameter: {key}"))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_require_str_rejects_empty() {
        let params = json!({"key": ""});
        assert!(params.require_str("key").is_err());

        let params = json!({});
        assert!(params.require_str("key").is_err());
    }

    #[test]
    fn test_require_str_accepts_valid() {
        let params = json!({"key": "value"});
        assert_eq!(params.require_str("key").unwrap(), "value");
    }
}
