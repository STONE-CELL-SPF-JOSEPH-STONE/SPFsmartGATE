// SPF Smart Gateway - Complexity Calculator
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// Implements: C = (basic ^ 1) + (dependencies ^ 7) + (complex ^ 10) + (files × 10)
// Master formula: a_optimal(C) = W_eff × (1 - 1/ln(C + e))

use crate::config::SpfConfig;
use serde::{Deserialize, Serialize};

/// Result of complexity calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityResult {
    pub tool: String,
    pub c: u64,
    pub tier: String,
    pub analyze_percent: u8,
    pub build_percent: u8,
    pub a_optimal_tokens: u64,
    pub requires_approval: bool,
}

/// Input parameters for complexity calculation
/// EXTENDED: Supports ALL tool types — brain, rag, glob, grep, web
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ToolParams {
    // Common
    pub file_path: Option<String>,
    // Edit
    pub old_string: Option<String>,
    pub new_string: Option<String>,
    pub replace_all: Option<bool>,
    // Write
    pub content: Option<String>,
    // Bash
    pub command: Option<String>,
    // Search (glob/grep)
    pub query: Option<String>,
    pub pattern: Option<String>,
    pub path: Option<String>,
    // Brain operations
    pub collection: Option<String>,
    pub limit: Option<u64>,
    pub text: Option<String>,
    pub title: Option<String>,
    // RAG/Web operations
    pub url: Option<String>,
    pub topic: Option<String>,
    pub category: Option<String>,
}

// ============================================================================
// DYNAMIC COMPLEXITY HELPERS
// complex^10: 1→1, 2→1024, 3→59049, 4→1048576
// files×10: scales linearly with affected file count
// ============================================================================

/// Calculate dynamic complexity factor (0-4 scale)
/// This is the primary lever for tier escalation via ^10 exponent
fn calc_complex_factor(content_len: u64, has_risk: bool, is_architectural: bool) -> u64 {
    let mut complex: u64 = 0;
    
    // Size-based complexity
    if content_len > 200 { complex += 1; }      // Moderate size
    if content_len > 1000 { complex += 1; }     // Large change
    if content_len > 5000 { complex += 1; }     // Very large change
    
    // Risk indicators add complexity
    if has_risk { complex += 1; }
    
    // Architectural changes are highest complexity
    if is_architectural { complex = complex.max(3); }
    
    complex.min(4)  // Cap at 4 (4^10 = 1,048,576)
}

/// Calculate dynamic files factor based on scope
fn calc_files_factor(path: &str, pattern: &str, cmd: &str) -> u64 {
    // Codebase-wide operations
    if cmd.contains("find") || cmd.contains("xargs") || cmd.contains("-r ") {
        return 100;  // 100×10 = 1000
    }
    
    // Recursive glob
    if pattern.contains("**") || path.contains("**") || cmd.contains("**") {
        return 50;   // 50×10 = 500
    }
    
    // Simple glob
    if pattern.contains("*") || path.contains("*") || cmd.contains("*") {
        return 20;   // 20×10 = 200
    }
    
    // Root directory = potentially many files
    if path == "." || path == "/" || path.ends_with("src") || path.ends_with("lib") {
        return 20;
    }
    
    // Default single file
    1
}

/// Check if file is architectural (config, main, lib, mod)
fn is_architectural_file(path: &str) -> bool {
    let p = path.to_lowercase();
    p.contains("config") || p.contains("main.") || p.contains("lib.") 
        || p.contains("mod.") || p.contains("cargo.toml") || p.contains("package.json")
        || p.contains(".env") || p.contains("settings") || p.contains("schema")
        || p.ends_with("rc") || p.ends_with(".yaml") || p.ends_with(".yml")
}

/// Check if content has risk indicators
fn has_risk_indicators(content: &str) -> bool {
    content.contains("delete") || content.contains("drop") || content.contains("remove")
        || content.contains("truncate") || content.contains("override") 
        || content.contains("force") || content.contains("unsafe")
        || content.contains("rm ") || content.contains("sudo")
}

/// Calculate complexity value C for a tool call
pub fn calculate_c(tool: &str, params: &ToolParams, config: &SpfConfig) -> u64 {
    let (basic, dependencies, complex_factor, files) = match tool {
        "Edit" | "spf_edit" => {
            let old_str = params.old_string.as_deref().unwrap_or("");
            let new_str = params.new_string.as_deref().unwrap_or("");
            let old_len = old_str.len() as u64;
            let new_len = new_str.len() as u64;
            let total_len = old_len + new_len;
            let file_path = params.file_path.as_deref().unwrap_or("");
            
            let basic = config.complexity_weights.edit.basic + total_len / 20;
            
            // Dependencies: replace_all affects more, large diffs have cascading effects
            let mut deps = if params.replace_all.unwrap_or(false) { 3u64 } else { 1 };
            if total_len > 500 { deps += 1; }
            
            // Complex factor: dynamic based on size, risk, architecture
            let has_risk = has_risk_indicators(new_str);
            let is_arch = is_architectural_file(file_path);
            let complex = calc_complex_factor(total_len, has_risk, is_arch);
            
            // Files: edits affect 1 file but replace_all could have wide impact
            let files = if params.replace_all.unwrap_or(false) { 5u64 } else { 1 };
            
            (basic, deps, complex, files)
        }

        "Write" | "spf_write" => {
            let content = params.content.as_deref().unwrap_or("");
            let content_len = content.len() as u64;
            let file_path = params.file_path.as_deref().unwrap_or("");

            let basic = config.complexity_weights.write.basic + content_len / 50;
            
            // Dependencies: imports/requires in content indicate deps
            let mut deps = config.complexity_weights.write.dependencies;
            if content.contains("import ") || content.contains("require(") 
                || content.contains("use ") || content.contains("mod ") {
                deps += 2;
            }
            
            // Complex factor: dynamic
            let has_risk = has_risk_indicators(content);
            let is_arch = is_architectural_file(file_path);
            let complex = calc_complex_factor(content_len, has_risk, is_arch);
            
            (basic, deps, complex, 1u64)
        }

        "Bash" | "spf_bash" => {
            let cmd = params.command.as_deref().unwrap_or("");

            // Check dangerous commands
            let is_dangerous = config.dangerous_commands.iter().any(|d| cmd.contains(d.as_str()));
            // Check git operations
            let is_git = cmd.contains("git push") || cmd.contains("git reset")
                || cmd.contains("git rebase") || cmd.contains("git merge");
            // Check piped/chained
            let is_piped = cmd.contains("&&") || cmd.contains("|");
            
            // Dynamic files calculation
            let files = calc_files_factor("", "", cmd);
            
            // Count pipe stages as dependencies
            let pipe_count = cmd.matches("|").count() as u64;
            let chain_count = cmd.matches("&&").count() as u64;
            
            if is_dangerous {
                let w = &config.complexity_weights.bash_dangerous;
                // Dangerous = high complex factor
                (w.basic, w.dependencies + pipe_count + chain_count, 3u64.max(w.complex), files)
            } else if is_git {
                let w = &config.complexity_weights.bash_git;
                // Git operations: complex=2 minimum (1024 added to C)
                (w.basic, w.dependencies + pipe_count, 2u64.max(w.complex), files)
            } else if is_piped {
                let w = &config.complexity_weights.bash_piped;
                // Piped: complexity scales with pipe count
                let complex = (1 + pipe_count).min(3);
                (w.basic, w.dependencies + pipe_count + chain_count, complex, files)
            } else {
                let w = &config.complexity_weights.bash_simple;
                (w.basic, w.dependencies, w.complex, files)
            }
        }

        "Read" | "spf_read" => {
            // Reads are safe - encourage information gathering
            let w = &config.complexity_weights.read;
            (w.basic, w.dependencies, w.complex, w.files)
        }

        "Glob" | "spf_glob" | "Grep" | "spf_grep" => {
            let w = &config.complexity_weights.search;
            let path = params.path.as_deref().unwrap_or(".");
            let pattern = params.pattern.as_deref().unwrap_or("");
            
            // Dynamic files based on pattern scope
            let files = calc_files_factor(path, pattern, "");
            
            // Search complexity based on pattern
            let complex = if pattern.len() > 50 { 1u64 } else { w.complex };
            
            (w.basic, w.dependencies, complex, files)
        }

        // === BRAIN OPERATIONS — MUST BE GATED ===
        "brain_search" | "spf_brain_search" => {
            let limit = params.limit.unwrap_or(5);
            (10, limit, 0, 1)
        }
        "brain_store" | "spf_brain_store" => {
            let text_len = params.text.as_ref().map(|s| s.len()).unwrap_or(0) as u64;
            (20 + text_len / 50, 2, if text_len > 5000 { 1 } else { 0 }, 1)
        }
        "brain_index" | "spf_brain_index" => (50, 5, 1, 10),
        "brain_recall" | "spf_brain_recall" |
        "brain_context" | "spf_brain_context" |
        "brain_list" | "spf_brain_list" |
        "brain_status" | "spf_brain_status" |
        "brain_list_docs" | "spf_brain_list_docs" |
        "brain_get_doc" | "spf_brain_get_doc" => (10, 1, 0, 1),

        // === RAG OPERATIONS — MUST BE GATED ===
        "rag_collect_web" | "spf_rag_collect_web" => (50, 10, 1, 5),
        "rag_fetch_url" | "spf_rag_fetch_url" => (30, 5, 1, 1),
        "rag_collect_file" | "spf_rag_collect_file" => (15, 2, 0, 1),
        "rag_collect_folder" | "spf_rag_collect_folder" => (30, 5, 0, 10),
        "rag_index_gathered" | "spf_rag_index_gathered" => (40, 5, 1, 10),
        "rag_collect_drop" | "spf_rag_collect_drop" => (25, 3, 0, 5),
        "rag_collect_rss" | "spf_rag_collect_rss" => (25, 5, 0, 5),
        "rag_dedupe" | "spf_rag_dedupe" => (20, 3, 0, 1),
        "rag_smart_search" | "spf_rag_smart_search" |
        "rag_auto_fetch_gaps" | "spf_rag_auto_fetch_gaps" => (40, 8, 1, 5),
        "rag_fulfill_search" | "spf_rag_fulfill_search" => (20, 3, 0, 1),
        "rag_status" | "spf_rag_status" |
        "rag_list_gathered" | "spf_rag_list_gathered" |
        "rag_bandwidth_status" | "spf_rag_bandwidth_status" |
        "rag_list_feeds" | "spf_rag_list_feeds" |
        "rag_pending_searches" | "spf_rag_pending_searches" => (8, 1, 0, 1),

        // === WEB OPERATIONS ===
        "web_fetch" | "spf_web_fetch" => (30, 5, 1, 1),
        "web_search" | "spf_web_search" => (25, 3, 0, 1),

        // === NOTEBOOK ===
        "notebook_edit" | "spf_notebook_edit" => (15, 2, 0, 1),

        // === STATUS (low complexity) ===
        "status" | "spf_status" | "session" | "spf_session" |
        "calculate" | "spf_calculate" => (5, 0, 0, 1),

        // === UNKNOWN — default high for safety ===
        _ => {
            let w = &config.complexity_weights.unknown;
            (w.basic, w.dependencies, w.complex, w.files)
        }
    };

    // Apply formula: C = (basic ^ power) + (deps ^ power) + (complex ^ power) + (files × mult)
    // HARDCODE: Saturating math prevents overflow — system never breaks
    let c = basic.saturating_pow(config.formula.basic_power)
        .saturating_add(dependencies.saturating_pow(config.formula.deps_power))
        .saturating_add(complex_factor.saturating_pow(config.formula.complex_power))
        .saturating_add(files.saturating_mul(config.formula.files_multiplier));

    c
}

/// Apply master formula: a_optimal(C) = W_eff × (1 - 1/ln(C + e))
pub fn a_optimal(c: u64, config: &SpfConfig) -> u64 {
    let c_f = if c == 0 { 1.0 } else { c as f64 };
    let result = config.formula.w_eff * (1.0 - 1.0 / (c_f + config.formula.e).ln());
    result.max(0.0) as u64
}

/// Full complexity calculation — returns everything needed for enforcement
pub fn calculate(tool: &str, params: &ToolParams, config: &SpfConfig) -> ComplexityResult {
    let c = calculate_c(tool, params, config);
    let (tier, analyze, build, requires_approval) = config.get_tier(c);
    let tokens = a_optimal(c, config);

    ComplexityResult {
        tool: tool.to_string(),
        c,
        tier: tier.to_string(),
        analyze_percent: analyze,
        build_percent: build,
        a_optimal_tokens: tokens,
        requires_approval,
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
    fn read_produces_simple_tier() {
        let config = default_config();
        let params = ToolParams::default();
        let result = calculate("spf_read", &params, &config);
        assert_eq!(result.tier, "SIMPLE");
        assert!(result.c < 500, "Read C={} should be < 500", result.c);
    }

    #[test]
    fn simple_bash_is_simple_tier() {
        let config = default_config();
        let params = ToolParams { command: Some("ls -la".to_string()), ..Default::default() };
        let result = calculate("spf_bash", &params, &config);
        assert_eq!(result.tier, "SIMPLE", "Simple bash C={} tier={}", result.c, result.tier);
    }

    #[test]
    fn dangerous_bash_is_critical_tier() {
        let config = default_config();
        let params = ToolParams { command: Some("rm -rf / --no-preserve-root".to_string()), ..Default::default() };
        let result = calculate("spf_bash", &params, &config);
        assert_eq!(result.tier, "CRITICAL", "Dangerous bash C={} should be CRITICAL", result.c);
        assert!(result.c >= 10000);
    }

    #[test]
    fn status_tool_is_minimal_complexity() {
        let config = default_config();
        let params = ToolParams::default();
        let result = calculate("spf_status", &params, &config);
        assert!(result.c < 100, "Status C={} should be minimal", result.c);
        assert_eq!(result.tier, "SIMPLE");
    }

    #[test]
    fn unknown_tool_uses_default_weights() {
        let config = default_config();
        let params = ToolParams::default();
        let c = calculate_c("totally_unknown_tool", &params, &config);
        // unknown: basic=20, deps=3, complex=1, files=1
        // C = 20 + 3^7 + 1^10 + 1*10 = 20 + 2187 + 1 + 10 = 2218
        assert!(c >= 2000, "Unknown tool C={} should be >= 2000 (LIGHT+)", c);
    }

    #[test]
    fn a_optimal_within_bounds() {
        let config = default_config();
        let tokens = a_optimal(100, &config);
        assert!(tokens > 0, "a_optimal(100) should be > 0");
        assert!(tokens < 40000, "a_optimal(100)={} should be < W_eff(40000)", tokens);
    }

    #[test]
    fn a_optimal_zero_input() {
        let config = default_config();
        let tokens = a_optimal(0, &config);
        // C=0 → uses c_f=1.0, ln(1+e) ≈ 1.31, result should be positive
        assert!(tokens > 0, "a_optimal(0)={} should still be > 0", tokens);
    }

    #[test]
    fn risk_indicators_detected() {
        assert!(has_risk_indicators("please delete this file"));
        assert!(has_risk_indicators("sudo make install"));
        assert!(has_risk_indicators("rm -rf everything"));
        assert!(!has_risk_indicators("create a new file"));
        assert!(!has_risk_indicators("read the documentation"));
    }
}
