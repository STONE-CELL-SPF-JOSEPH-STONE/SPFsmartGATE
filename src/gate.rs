// SPF Smart Gateway - Gate (Primary Enforcement Point)
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// Every tool call passes through here. Calculate -> Validate -> Allow/Warn.
// Max mode: violations warn + force CRITICAL tier. Never blocks — escalates.
// Enforcement: compiled validation rules, write whitelist, path blocking,
// Build Anchor protocol, content inspection. No runtime config bypass.

use chrono::Utc;
use crate::calculate::{self, ComplexityResult, ToolParams};
use crate::config::{EnforceMode, SpfConfig};
use crate::inspect;
use crate::session::Session;
use crate::validate;
use serde::{Deserialize, Serialize};

/// Gate decision — the final word on whether a tool call proceeds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDecision {
    pub allowed: bool,
    pub tool: String,
    pub complexity: ComplexityResult,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub message: String,
}

/// Human-readable summary of what the action will do.
/// Used for logging and audit output.
fn format_params(tool: &str, params: &ToolParams) -> String {
    match tool {
        "Bash" | "spf_bash" => {
            format!("Command: {}", params.command.as_deref().unwrap_or("(none)"))
        }
        "Read" | "spf_read" => {
            format!("File: {}", params.file_path.as_deref().unwrap_or("(none)"))
        }
        "Write" | "spf_write" => {
            let len = params.content.as_ref().map(|c| c.len()).unwrap_or(0);
            format!("File: {} | Content: {} bytes",
                params.file_path.as_deref().unwrap_or("(none)"), len)
        }
        "Edit" | "spf_edit" => {
            let old_preview: String = params.old_string.as_deref()
                .unwrap_or("").chars().take(60).collect();
            let new_preview: String = params.new_string.as_deref()
                .unwrap_or("").chars().take(60).collect();
            format!("File: {} | Replace: \"{}...\" -> \"{}...\"",
                params.file_path.as_deref().unwrap_or("(none)"),
                old_preview, new_preview)
        }
        "Glob" | "spf_glob" => {
            format!("Pattern: {} | Path: {}",
                params.command.as_deref().unwrap_or("*"),
                params.file_path.as_deref().unwrap_or("."))
        }
        "Grep" | "spf_grep" => {
            format!("Pattern: {} | Path: {}",
                params.command.as_deref().unwrap_or(""),
                params.file_path.as_deref().unwrap_or("."))
        }
        _ => {
            let mut parts = Vec::new();
            if let Some(ref cmd) = params.command {
                parts.push(format!("arg: {}", cmd));
            }
            if let Some(ref fp) = params.file_path {
                parts.push(format!("path: {}", fp));
            }
            if parts.is_empty() {
                "(no params)".to_string()
            } else {
                parts.join(" | ")
            }
        }
    }
}

// ========================================================================
// GATE PROCESS — primary enforcement
// ========================================================================

/// Process a tool call through the gate
///
/// Pipeline:
/// 1. Calculate complexity (C, tier, allocation)
/// 2. Validate against rules (blocked paths, Build Anchor, write whitelist, dangerous cmds)
/// 3. Content inspection on Write/Edit
/// 4. Max mode: if warnings present, escalate to CRITICAL tier (warn, don't block)
/// 5. Return allow/block decision
pub fn process(
    tool: &str,
    params: &ToolParams,
    config: &SpfConfig,
    session: &Session,
) -> GateDecision {
    // Rate limiting — max operations per minute by category
    let now = Utc::now();
    let one_minute_ago = now - chrono::Duration::seconds(60);
    let recent_count = session.rate_window.iter()
        .filter(|ts| **ts > one_minute_ago)
        .count();

    let max_per_minute = match tool {
        "Write" | "spf_write" | "Edit" | "spf_edit" |
        "Bash" | "spf_bash" | "spf_web_download" | "spf_notebook_edit" => 60,
        "spf_web_fetch" | "spf_web_search" | "spf_web_api" => 30,
        _ => 120,  // reads, search, status — more lenient
    };

    if recent_count >= max_per_minute {
        let msg = format!("RATE LIMITED: {} calls in last minute (max {})", recent_count, max_per_minute);
        return GateDecision {
            allowed: false,
            tool: tool.to_string(),
            complexity: ComplexityResult {
                tool: tool.to_string(),
                c: 0,
                tier: "RATE_LIMITED".to_string(),
                analyze_percent: 100,
                build_percent: 0,
                a_optimal_tokens: 0,
                requires_approval: true,
            },
            warnings: vec![],
            errors: vec![msg.clone()],
            message: format!("BLOCKED | {} | {}", tool, msg),
        };
    }

    // Step 1: Calculate complexity
    let mut complexity = calculate::calculate(tool, params, config);

    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    // Step 2: Validate against rules
    let validation = match tool {
        "Edit" | "spf_edit" => {
            let file_path = params.file_path.as_deref().unwrap_or("unknown");
            validate::validate_edit(file_path, config, session)
        }
        "Write" | "spf_write" => {
            let file_path = params.file_path.as_deref().unwrap_or("unknown");
            let content_len = params.content.as_ref().map(|c| c.len()).unwrap_or(0);
            validate::validate_write(file_path, content_len, config, session)
        }
        "Bash" | "spf_bash" => {
            let command = params.command.as_deref().unwrap_or("");
            validate::validate_bash(command, config)
        }
        "Read" | "spf_read" => {
            let file_path = params.file_path.as_deref().unwrap_or("unknown");
            validate::validate_read(file_path, config)
        }
        "spf_web_download" => {
            let file_path = params.file_path.as_deref().unwrap_or("unknown");
            // content_len unknown pre-download — pass 0, path checks still enforce
            validate::validate_write(file_path, 0, config, session)
        }
        "spf_notebook_edit" => {
            let file_path = params.file_path.as_deref().unwrap_or("unknown");
            let content_len = params.content.as_ref().map(|c| c.len()).unwrap_or(0);
            validate::validate_write(file_path, content_len, config, session)
        }
        // HARD BLOCK — spf_fs_* tools are USER/SYSTEM-ONLY, never allow via MCP
        "spf_fs_import" | "spf_fs_export" |
        "spf_fs_exists" | "spf_fs_stat" | "spf_fs_ls" | "spf_fs_read" |
        "spf_fs_write" | "spf_fs_mkdir" | "spf_fs_rm" | "spf_fs_rename" => {
            validate::ValidationResult {
                valid: false,
                warnings: vec![],
                errors: vec![format!("BLOCKED: {} is a user/system-only command — not available to AI agents", tool)],
            }
        }
        // Known tools that don't need path/write validation — explicitly allowed
        "spf_calculate" | "spf_status" | "spf_session" |
        "spf_glob" | "spf_grep" |
        "spf_web_search" | "spf_web_fetch" | "spf_web_api" |
        "spf_brain_search" | "spf_brain_store" | "spf_brain_context" |
        "spf_brain_index" | "spf_brain_list" | "spf_brain_status" |
        "spf_brain_recall" | "spf_brain_list_docs" | "spf_brain_get_doc" |
        "spf_rag_collect_web" | "spf_rag_collect_file" | "spf_rag_collect_folder" |
        "spf_rag_collect_drop" | "spf_rag_index_gathered" | "spf_rag_dedupe" |
        "spf_rag_status" | "spf_rag_list_gathered" | "spf_rag_bandwidth_status" |
        "spf_rag_fetch_url" | "spf_rag_collect_rss" | "spf_rag_list_feeds" |
        "spf_rag_pending_searches" | "spf_rag_fulfill_search" |
        "spf_rag_smart_search" | "spf_rag_auto_fetch_gaps" |
        "spf_config_paths" | "spf_config_stats" |
        "spf_projects_list" | "spf_projects_get" | "spf_projects_set" |
        "spf_projects_delete" | "spf_projects_stats" |
        "spf_tmp_list" | "spf_tmp_stats" | "spf_tmp_get" | "spf_tmp_active" |
        "spf_agent_stats" | "spf_agent_memory_search" | "spf_agent_memory_by_tag" |
        "spf_agent_session_info" | "spf_agent_context"
            => validate::ValidationResult::ok(),
        // DEFAULT DENY — unknown tools blocked until explicitly added to allowlist
        _ => {
            validate::ValidationResult {
                valid: false,
                warnings: vec![],
                errors: vec![format!("BLOCKED: unknown tool '{}' — not in gate allowlist", tool)],
            }
        }
    };

    warnings.extend(validation.warnings);
    errors.extend(validation.errors);

    // Step 3: Content inspection on Write/Edit operations
    let inspection = match tool {
        "Write" | "spf_write" => {
            let file_path = params.file_path.as_deref().unwrap_or("unknown");
            let content = params.content.as_deref().unwrap_or("");
            inspect::inspect_content(content, file_path, config)
        }
        "Edit" | "spf_edit" => {
            let file_path = params.file_path.as_deref().unwrap_or("unknown");
            let new_string = params.new_string.as_deref().unwrap_or("");
            inspect::inspect_content(new_string, file_path, config)
        }
        "spf_notebook_edit" => {
            let file_path = params.file_path.as_deref().unwrap_or("unknown");
            let content = params.content.as_deref().unwrap_or("");
            inspect::inspect_content(content, file_path, config)
        }
        // Safe: unknown tools already blocked by validation above (allowed = valid && valid)
        _ => validate::ValidationResult::ok(),
    };

    warnings.extend(inspection.warnings);
    errors.extend(inspection.errors);

    // Step 4: Max mode escalation — if any "MAX TIER:" warnings present,
    // force complexity to CRITICAL tier instead of blocking
    if config.enforce_mode == EnforceMode::Max {
        let has_max_warnings = warnings.iter().any(|w| w.starts_with("MAX TIER:"));
        if has_max_warnings {
            complexity.tier = "CRITICAL".to_string();
            complexity.analyze_percent = config.tiers.critical.analyze_percent;
            complexity.build_percent = config.tiers.critical.build_percent;
            complexity.requires_approval = true;
            warnings.push("ESCALATED TO CRITICAL TIER — Max mode enforcement".to_string());
        }
    }

    let allowed = validation.valid && inspection.valid;

    // Build message with action details
    let details = format_params(tool, params);
    let message = if allowed {
        format!(
            "ALLOWED | {} | C={} | {} | {}%/{}% | {}",
            tool, complexity.c, complexity.tier,
            complexity.analyze_percent, complexity.build_percent,
            details
        )
    } else {
        format!(
            "BLOCKED | {} | C={} | {} errors | {}",
            tool, complexity.c, errors.len(),
            details
        )
    };

    GateDecision {
        allowed,
        tool: tool.to_string(),
        complexity,
        warnings,
        errors,
        message,
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SpfConfig;
    use crate::session::Session;

    fn default_config() -> SpfConfig {
        SpfConfig::default()
    }

    #[test]
    fn allowed_tool_passes_gate() {
        let config = default_config();
        let session = Session::new();
        let params = ToolParams::default();
        let decision = process("spf_status", &params, &config, &session);
        assert!(decision.allowed, "spf_status should be allowed: {}", decision.message);
    }

    #[test]
    fn blocked_fs_tool_denied() {
        let config = default_config();
        let session = Session::new();
        let params = ToolParams::default();
        let decision = process("spf_fs_write", &params, &config, &session);
        assert!(!decision.allowed, "spf_fs_write should be BLOCKED");
        assert!(decision.errors.iter().any(|e| e.contains("BLOCKED")));
    }

    #[test]
    fn unknown_tool_denied_default_deny() {
        let config = default_config();
        let session = Session::new();
        let params = ToolParams::default();
        let decision = process("evil_new_tool", &params, &config, &session);
        assert!(!decision.allowed, "Unknown tool should be blocked by default-deny");
        assert!(decision.errors.iter().any(|e| e.contains("not in gate allowlist")));
    }

    #[test]
    fn all_fs_tools_blocked() {
        let config = default_config();
        let session = Session::new();
        let params = ToolParams::default();
        let fs_tools = [
            "spf_fs_exists", "spf_fs_stat", "spf_fs_ls", "spf_fs_read",
            "spf_fs_write", "spf_fs_mkdir", "spf_fs_rm", "spf_fs_rename",
        ];
        for tool in &fs_tools {
            let decision = process(tool, &params, &config, &session);
            assert!(!decision.allowed, "{} should be BLOCKED", tool);
        }
    }
}
