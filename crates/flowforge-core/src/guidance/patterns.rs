//! Compiled regex patterns for destructive ops and secrets detection.
//! Shared between the guidance engine and hook::check_dangerous_command().

use std::sync::LazyLock;

use regex::Regex;

use crate::types::RiskLevel;

/// Built-in destructive-ops patterns, compiled once via LazyLock.
pub static DESTRUCTIVE_PATTERNS: LazyLock<Vec<(Regex, &'static str, RiskLevel)>> =
    LazyLock::new(|| {
        let patterns: Vec<(&str, &str, RiskLevel)> = vec![
            (
                r"rm\s+-rf\s+[/~]",
                "Recursive delete of root/home",
                RiskLevel::Critical,
            ),
            (
                r"rm\s+-rf\s+/\*",
                "Recursive delete of all root contents",
                RiskLevel::Critical,
            ),
            (r":\(\)\{:\|:&\};:", "Fork bomb", RiskLevel::Critical),
            (r"mkfs\.", "Filesystem formatting", RiskLevel::Critical),
            (
                r"dd\s+if=/dev/(zero|random|urandom)",
                "Disk overwrite",
                RiskLevel::Critical,
            ),
            (
                r">\s*/dev/sd[a-z]",
                "Direct disk overwrite",
                RiskLevel::Critical,
            ),
            (
                r"chmod\s+-R\s+777\s+/",
                "Remove permissions from root",
                RiskLevel::Critical,
            ),
            (
                r"--no-preserve-root",
                "Root protection bypass",
                RiskLevel::Critical,
            ),
            (
                r"sudo\s+rm\s+-rf",
                "Sudo recursive force delete",
                RiskLevel::Critical,
            ),
            (r"git\s+reset\s+--hard", "Git hard reset", RiskLevel::High),
            (r"git\s+push\s+--force", "Git force push", RiskLevel::High),
            (r"git\s+push\s+-f\b", "Git force push", RiskLevel::High),
            (r"git\s+clean\s+-fd", "Git clean force", RiskLevel::High),
            (
                r"(wget|curl)\s.*\|\s*(ba)?sh",
                "Pipe download to shell",
                RiskLevel::High,
            ),
        ];
        patterns
            .into_iter()
            .filter_map(|(pat, desc, risk)| Regex::new(pat).ok().map(|r| (r, desc, risk)))
            .collect()
    });

/// Built-in secrets patterns, compiled once via LazyLock.
pub static SECRET_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    let patterns = [
        r"AKIA[0-9A-Z]{16}",                         // AWS access key
        r"(?i)bearer\s+[a-z0-9\-._~+/]+=*",          // Bearer token
        r"-----BEGIN\s+(RSA\s+)?PRIVATE\s+KEY-----", // Private key
        r#"(?i)["']?(api[_-]?key|api[_-]?secret|access[_-]?token|auth[_-]?token|secret[_-]?key)["']?\s*[:=]\s*["'][a-z0-9]{20,}"#, // Generic API keys
    ];
    patterns.iter().filter_map(|p| Regex::new(p).ok()).collect()
});

/// Check if a command matches any destructive pattern using compiled regexes.
/// Used by both the guidance engine and `hook::check_dangerous_command()`.
pub fn check_dangerous_command(command: &str) -> Option<&'static str> {
    let cmd_lower = command.to_lowercase();
    let cmd_normalized = cmd_lower.replace('\\', "").replace('\n', " ");

    for (regex, desc, _risk) in DESTRUCTIVE_PATTERNS.iter() {
        if regex.is_match(&cmd_normalized) {
            return Some(desc);
        }
    }
    None
}
