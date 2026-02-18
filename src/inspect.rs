// SPF Smart Gateway - Content Inspection
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// Inspects content being written/edited/executed for:
// - Credential patterns (API keys, tokens, private keys)
// - Path traversal attempts (../ sequences)
// - Shell injection in written content (backticks, $(), eval)
// - References to paths outside allowed boundaries

use crate::config::{EnforceMode, SpfConfig};
use crate::validate::ValidationResult;

/// Credential patterns to detect
const CREDENTIAL_PATTERNS: &[(&str, &str)] = &[
    ("sk-", "Possible API secret key"),
    ("AKIA", "Possible AWS access key"),
    ("ghp_", "Possible GitHub personal access token"),
    ("gho_", "Possible GitHub OAuth token"),
    ("ghs_", "Possible GitHub server token"),
    ("github_pat_", "Possible GitHub PAT"),
    ("glpat-", "Possible GitLab PAT"),
    ("xoxb-", "Possible Slack bot token"),
    ("xoxp-", "Possible Slack user token"),
    ("-----BEGIN RSA PRIVATE KEY", "RSA private key detected"),
    ("-----BEGIN OPENSSH PRIVATE KEY", "SSH private key detected"),
    ("-----BEGIN EC PRIVATE KEY", "EC private key detected"),
    ("-----BEGIN PRIVATE KEY", "Private key detected"),
    ("password=", "Possible hardcoded password"),
    ("passwd=", "Possible hardcoded password"),
    ("secret=", "Possible hardcoded secret"),
    ("api_key=", "Possible hardcoded API key"),
    ("apikey=", "Possible hardcoded API key"),
    ("access_token=", "Possible hardcoded access token"),
];

/// Shell injection patterns in written content
const SHELL_INJECTION_PATTERNS: &[(&str, &str)] = &[
    ("$(", "Command substitution in content"),
    ("eval ", "Eval statement in content"),
    ("exec ", "Exec statement in content"),
    ("`", "Backtick command substitution in content"),
];

/// Inspect content being written or edited
pub fn inspect_content(
    content: &str,
    file_path: &str,
    config: &SpfConfig,
) -> ValidationResult {
    let mut result = ValidationResult::ok();

    // Skip inspection for shell scripts and config files where these patterns are expected
    if file_path.ends_with(".sh") || file_path.ends_with(".bash")
        || file_path.ends_with(".zsh") || file_path.ends_with(".rs")
        || file_path.ends_with(".py") || file_path.ends_with(".js")
        || file_path.ends_with(".ts") || file_path.ends_with(".toml")
        || file_path.ends_with(".json") || file_path.ends_with(".md")
    {
        // For code files, only check credentials — shell patterns are normal
        check_credentials(content, config, &mut result);
        check_path_traversal(content, config, &mut result);
        check_blocked_path_references(content, config, &mut result);
        return result;
    }

    // Full inspection for non-code files
    check_credentials(content, config, &mut result);
    check_path_traversal(content, config, &mut result);
    check_shell_injection(content, config, &mut result);
    check_blocked_path_references(content, config, &mut result);

    result
}

/// Check for credential patterns
fn check_credentials(
    content: &str,
    config: &SpfConfig,
    result: &mut ValidationResult,
) {
    for (pattern, description) in CREDENTIAL_PATTERNS {
        if content.contains(pattern) {
            match config.enforce_mode {
                EnforceMode::Max => {
                    result.warn(format!("MAX TIER: CREDENTIAL DETECTED — {}", description));
                }
                EnforceMode::Soft => {
                    result.warn(format!("Credential warning: {}", description));
                }
            }
        }
    }
}

/// Check for path traversal attempts
fn check_path_traversal(
    content: &str,
    config: &SpfConfig,
    result: &mut ValidationResult,
) {
    if content.contains("../") || content.contains("..\\") {
        match config.enforce_mode {
            EnforceMode::Max => {
                result.warn("MAX TIER: PATH TRAVERSAL — content contains ../ sequences".to_string());
            }
            EnforceMode::Soft => {
                result.warn("Path traversal pattern detected in content".to_string());
            }
        }
    }
}

/// Check for shell injection patterns (non-code files only)
fn check_shell_injection(
    content: &str,
    config: &SpfConfig,
    result: &mut ValidationResult,
) {
    for (pattern, description) in SHELL_INJECTION_PATTERNS {
        if content.contains(pattern) {
            match config.enforce_mode {
                EnforceMode::Max => {
                    result.warn(format!("MAX TIER: SHELL INJECTION — {}", description));
                }
                EnforceMode::Soft => {
                    result.warn(format!("Shell pattern warning: {}", description));
                }
            }
        }
    }
}

/// Check for references to blocked paths in content
fn check_blocked_path_references(
    content: &str,
    config: &SpfConfig,
    result: &mut ValidationResult,
) {
    for blocked in &config.blocked_paths {
        if content.contains(blocked.as_str()) {
            result.warn(format!("Content references blocked path: {}", blocked));
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SpfConfig;

    fn default_config() -> SpfConfig {
        SpfConfig::default()
    }

    #[test]
    fn detects_aws_access_key() {
        let config = default_config();
        let result = inspect_content("my key is AKIAIOSFODNN7EXAMPLE", "data.txt", &config);
        assert!(!result.warnings.is_empty(), "Should detect AKIA pattern");
    }

    #[test]
    fn detects_private_key() {
        let config = default_config();
        let result = inspect_content("-----BEGIN RSA PRIVATE KEY-----\nblah", "key.txt", &config);
        assert!(!result.warnings.is_empty(), "Should detect RSA private key");
    }

    #[test]
    fn detects_github_pat() {
        let config = default_config();
        let result = inspect_content("token: ghp_abc123def456ghi789", "notes.txt", &config);
        assert!(!result.warnings.is_empty(), "Should detect GitHub PAT");
    }

    #[test]
    fn detects_path_traversal() {
        let config = default_config();
        let result = inspect_content("read from ../../../etc/passwd", "data.txt", &config);
        assert!(!result.warnings.is_empty(), "Should detect path traversal");
    }

    #[test]
    fn detects_shell_injection_in_non_code() {
        let config = default_config();
        let result = inspect_content("run $(whoami) now", "data.txt", &config);
        assert!(!result.warnings.is_empty(), "Should detect command substitution");
    }

    #[test]
    fn skips_shell_patterns_in_code_files() {
        let config = default_config();
        // Shell patterns are normal in .sh files — should NOT flag shell injection
        let result = inspect_content("echo $(date)", "script.sh", &config);
        // Should have zero warnings about shell injection (only creds/traversal checked for code)
        let shell_warnings: Vec<_> = result.warnings.iter()
            .filter(|w| w.contains("SHELL") || w.contains("Command substitution"))
            .collect();
        assert!(shell_warnings.is_empty(), "Should skip shell patterns in .sh files: {:?}", shell_warnings);
    }

    #[test]
    fn clean_content_passes() {
        let config = default_config();
        let result = inspect_content("Hello, this is normal content.", "readme.txt", &config);
        assert!(result.warnings.is_empty(), "Clean content should have no warnings: {:?}", result.warnings);
        assert!(result.valid, "Clean content should be valid");
    }
}
