// SPF Smart Gateway - Web Browser Module
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// AI-friendly web access: search, read pages, download, API calls.
// All access gated through SPF gate::process() in mcp.rs handlers.
// Nothing bypasses SPF.

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::time::Duration;

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub description: String,
}

/// Validate URL is safe for external access (blocks SSRF targets)
fn validate_url(url: &str) -> Result<(), String> {
    // Enforce http/https scheme
    let without_scheme = if let Some(rest) = url.strip_prefix("https://") {
        rest
    } else if let Some(rest) = url.strip_prefix("http://") {
        rest
    } else {
        return Err(format!("BLOCKED: Only http/https URLs allowed: {}", url));
    };

    // Extract hostname — handle bracketed IPv6 [::1] before port split
    let host_port = without_scheme
        .split('/')
        .next().unwrap_or("")
        .split('?')
        .next().unwrap_or("")
        .to_lowercase();

    let host = if host_port.starts_with('[') {
        // Bracketed IPv6: [::1]:8080 or [::ffff:127.0.0.1]
        host_port.split(']').next().unwrap_or("").trim_start_matches('[')
    } else {
        // IPv4 or hostname: 127.0.0.1:8080 or example.com
        host_port.split(':').next().unwrap_or("")
    };

    // Named loopback/special hosts
    if host == "localhost" || host == "::1" || host == "0.0.0.0" {
        return Err(format!("SSRF BLOCKED: loopback address: {}", host));
    }

    // IPv4 classification
    if let Ok(addr) = host.parse::<Ipv4Addr>() {
        if addr.is_loopback() {
            return Err(format!("SSRF BLOCKED: loopback IP: {}", host));
        }
        if addr.is_private() {
            return Err(format!("SSRF BLOCKED: private network IP: {}", host));
        }
        if addr.is_link_local() {
            return Err(format!("SSRF BLOCKED: link-local IP: {}", host));
        }
        // Cloud metadata (169.254.x.x range)
        let octets = addr.octets();
        if octets[0] == 169 && octets[1] == 254 {
            return Err(format!("SSRF BLOCKED: metadata endpoint: {}", host));
        }
        // Additional cloud metadata IPs
        if host == "100.100.100.200" {
            return Err(format!("SSRF BLOCKED: cloud metadata endpoint: {}", host));
        }
    }

    // IPv6 classification — catches [::1], [::ffff:127.0.0.1], [fe80::1], etc.
    if let Ok(addr) = host.parse::<Ipv6Addr>() {
        if addr.is_loopback() {
            return Err(format!("SSRF BLOCKED: IPv6 loopback: {}", host));
        }
        // IPv4-mapped IPv6 (::ffff:127.0.0.1, ::ffff:10.0.0.1, etc.)
        if let Some(mapped) = addr.to_ipv4_mapped() {
            if mapped.is_loopback() {
                return Err(format!("SSRF BLOCKED: IPv4-mapped loopback: {}", host));
            }
            if mapped.is_private() {
                return Err(format!("SSRF BLOCKED: IPv4-mapped private IP: {}", host));
            }
            if mapped.is_link_local() {
                return Err(format!("SSRF BLOCKED: IPv4-mapped link-local: {}", host));
            }
            let octets = mapped.octets();
            if octets[0] == 169 && octets[1] == 254 {
                return Err(format!("SSRF BLOCKED: IPv4-mapped metadata endpoint: {}", host));
            }
        }
        // IPv6 link-local (fe80::/10)
        let segments = addr.segments();
        if segments[0] & 0xffc0 == 0xfe80 {
            return Err(format!("SSRF BLOCKED: IPv6 link-local: {}", host));
        }
        // IPv6 unique local (fc00::/7)
        if segments[0] & 0xfe00 == 0xfc00 {
            return Err(format!("SSRF BLOCKED: IPv6 unique-local (private): {}", host));
        }
    }

    Ok(())
}

/// Web client for SPF
pub struct WebClient {
    client: Client,
}

impl WebClient {
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .user_agent("SPF-SmartGate/1.0 (AI-Browser)")
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        Ok(Self { client })
    }

    /// Search via Brave Search API (requires BRAVE_API_KEY env var)
    pub fn search_brave(&self, query: &str, api_key: &str, count: u32) -> Result<Vec<SearchResult>, String> {
        let resp = self.client
            .get("https://api.search.brave.com/res/v1/web/search")
            .header("X-Subscription-Token", api_key)
            .header("Accept", "application/json")
            .query(&[("q", query), ("count", &count.to_string())])
            .send()
            .map_err(|e| format!("Brave search failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Brave API error: HTTP {}", resp.status().as_u16()));
        }

        let body: serde_json::Value = resp.json()
            .map_err(|e| format!("Parse failed: {}", e))?;

        let mut results = Vec::new();
        if let Some(web) = body.get("web").and_then(|w| w.get("results")).and_then(|r| r.as_array()) {
            for item in web {
                results.push(SearchResult {
                    title: item["title"].as_str().unwrap_or("").to_string(),
                    url: item["url"].as_str().unwrap_or("").to_string(),
                    description: item["description"].as_str().unwrap_or("").to_string(),
                });
            }
        }
        Ok(results)
    }

    /// Search via DuckDuckGo HTML (no API key needed, fallback)
    pub fn search_ddg(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        let resp = self.client
            .post("https://html.duckduckgo.com/html/")
            .form(&[("q", query)])
            .send()
            .map_err(|e| format!("DDG search failed: {}", e))?;

        let html = resp.text().map_err(|e| format!("Read failed: {}", e))?;

        let mut results = Vec::new();
        let mut current_title = String::new();
        let mut current_url = String::new();

        for line in html.lines() {
            let trimmed = line.trim();

            // DDG result links have class "result__a"
            if trimmed.contains("result__a") && trimmed.contains("href=") {
                if let Some(url) = extract_attr(trimmed, "href") {
                    current_url = url;
                }
                if let Some(text) = extract_tag_text(trimmed) {
                    current_title = html_decode(&text);
                }
            }

            // DDG snippets have class "result__snippet"
            if trimmed.contains("result__snippet") {
                let desc = if let Some(text) = extract_tag_text(trimmed) {
                    html_decode(&text)
                } else {
                    String::new()
                };

                if !current_url.is_empty() {
                    results.push(SearchResult {
                        title: std::mem::take(&mut current_title),
                        url: std::mem::take(&mut current_url),
                        description: desc,
                    });
                }
            }
        }

        if results.is_empty() && !current_url.is_empty() {
            results.push(SearchResult {
                title: current_title,
                url: current_url,
                description: String::new(),
            });
        }

        Ok(results)
    }

    /// Auto-search: Brave if key available, otherwise DDG
    pub fn search(&self, query: &str, count: u32) -> Result<(String, Vec<SearchResult>), String> {
        if let Ok(key) = std::env::var("BRAVE_API_KEY") {
            if !key.is_empty() {
                let results = self.search_brave(query, &key, count)?;
                return Ok(("brave".to_string(), results));
            }
        }
        let results = self.search_ddg(query)?;
        Ok(("duckduckgo".to_string(), results))
    }

    /// Fetch URL and convert to clean readable text
    pub fn read_page(&self, url: &str) -> Result<(String, usize, String), String> {
        validate_url(url)?;

        let resp = self.client
            .get(url)
            .send()
            .map_err(|e| format!("Fetch failed: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(format!("HTTP {}: {}", status.as_u16(), url));
        }

        let content_type = resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let body = resp.text().map_err(|e| format!("Read failed: {}", e))?;
        let raw_len = body.len();

        // JSON: pretty print
        if content_type.contains("json") {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body) {
                let pretty = serde_json::to_string_pretty(&parsed).unwrap_or(body);
                return Ok((pretty, raw_len, content_type));
            }
            return Ok((body, raw_len, content_type));
        }

        // HTML: convert to readable text
        if content_type.contains("html") || body.trim_start().starts_with('<') {
            let text = html2text::from_read(body.as_bytes(), 120);
            return Ok((text, raw_len, content_type));
        }

        // Plain text or other
        Ok((body, raw_len, content_type))
    }

    /// Download file to disk
    pub fn download(&self, url: &str, save_path: &str) -> Result<(usize, String), String> {
        validate_url(url)?;

        let resp = self.client
            .get(url)
            .send()
            .map_err(|e| format!("Download failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("HTTP {}: {}", resp.status().as_u16(), url));
        }

        let content_type = resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        let bytes = resp.bytes().map_err(|e| format!("Read failed: {}", e))?;
        let size = bytes.len();

        if let Some(parent) = std::path::Path::new(save_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        std::fs::write(save_path, &bytes)
            .map_err(|e| format!("Write failed: {}", e))?;

        Ok((size, content_type))
    }

    /// Generic API request (GET/POST/PUT/DELETE/PATCH)
    pub fn api_request(
        &self,
        method: &str,
        url: &str,
        headers_json: &str,
        body: &str,
    ) -> Result<(u16, String, String), String> {
        validate_url(url)?;

        let mut req = match method.to_uppercase().as_str() {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            "PATCH" => self.client.patch(url),
            "HEAD" => self.client.head(url),
            _ => return Err(format!("Unsupported method: {}", method)),
        };

        // Parse custom headers from JSON object
        if !headers_json.is_empty() {
            if let Ok(headers) = serde_json::from_str::<serde_json::Value>(headers_json) {
                if let Some(obj) = headers.as_object() {
                    for (key, value) in obj {
                        if let Some(val) = value.as_str() {
                            req = req.header(key.as_str(), val);
                        }
                    }
                }
            }
        }

        // Add body for methods that support it
        if !body.is_empty() {
            match method.to_uppercase().as_str() {
                "POST" | "PUT" | "PATCH" => {
                    req = req.header("Content-Type", "application/json")
                        .body(body.to_string());
                }
                _ => {}
            }
        }

        let resp = req.send().map_err(|e| format!("Request failed: {}", e))?;
        let status = resp.status().as_u16();
        let resp_headers = format!("{:?}", resp.headers().clone());
        let resp_body = resp.text().map_err(|e| format!("Read body failed: {}", e))?;

        Ok((status, resp_headers, resp_body))
    }
}

/// Extract attribute value from HTML tag
fn extract_attr(html: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    if let Some(start) = html.find(&pattern) {
        let rest = &html[start + pattern.len()..];
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

/// Extract text content between > and </
fn extract_tag_text(html: &str) -> Option<String> {
    if let Some(start) = html.rfind('>') {
        let rest = &html[start + 1..];
        if let Some(end) = rest.find("</") {
            let text = rest[..end].trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

/// Decode common HTML entities
fn html_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // === PUBLIC IPS MUST BE ALLOWED ===

    #[test]
    fn allows_public_ipv4() {
        assert!(validate_url("https://8.8.8.8/path").is_ok());
        assert!(validate_url("https://1.1.1.1/").is_ok());
        assert!(validate_url("http://93.184.216.34/").is_ok());
    }

    #[test]
    fn allows_public_hostname() {
        assert!(validate_url("https://example.com/").is_ok());
        assert!(validate_url("https://api.github.com/repos").is_ok());
    }

    // === LOOPBACK MUST BE BLOCKED ===

    #[test]
    fn blocks_loopback_ipv4() {
        assert!(validate_url("https://127.0.0.1/").is_err());
        assert!(validate_url("https://127.0.0.99/admin").is_err());
    }

    #[test]
    fn blocks_localhost() {
        assert!(validate_url("https://localhost/").is_err());
        assert!(validate_url("http://localhost:8080/api").is_err());
    }

    // === PRIVATE NETWORKS MUST BE BLOCKED ===

    #[test]
    fn blocks_private_rfc1918() {
        assert!(validate_url("https://10.0.0.1/").is_err());         // 10.0.0.0/8
        assert!(validate_url("https://172.16.0.1/").is_err());       // 172.16.0.0/12
        assert!(validate_url("https://192.168.1.1/").is_err());      // 192.168.0.0/16
    }

    // === CLOUD METADATA MUST BE BLOCKED ===

    #[test]
    fn blocks_metadata_endpoints() {
        assert!(validate_url("http://169.254.169.254/latest/meta-data/").is_err());
        assert!(validate_url("http://100.100.100.200/").is_err());
    }

    // === IPV6 MUST BE BLOCKED ===

    #[test]
    fn blocks_ipv6_loopback() {
        assert!(validate_url("https://[::1]/").is_err());
    }

    #[test]
    fn blocks_ipv4_mapped_ipv6() {
        assert!(validate_url("https://[::ffff:127.0.0.1]/").is_err());
        assert!(validate_url("https://[::ffff:10.0.0.1]/").is_err());
        assert!(validate_url("https://[::ffff:192.168.1.1]/").is_err());
    }

    #[test]
    fn blocks_ipv6_private() {
        assert!(validate_url("https://[fe80::1]/").is_err());  // link-local
        assert!(validate_url("https://[fd00::1]/").is_err());  // unique-local
    }

    // === SCHEME ENFORCEMENT ===

    #[test]
    fn blocks_non_http_schemes() {
        assert!(validate_url("ftp://example.com/file").is_err());
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("gopher://evil.com/").is_err());
    }
}
