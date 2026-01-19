use super::base::{Tool, ToolContext, ToolError, ToolResult};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// WebFetch tool - Fetch and analyze web content
pub struct WebFetchTool {
    cache: Arc<Mutex<ResponseCache>>,
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(ResponseCache::new())),
        }
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple cache for web responses (15-minute TTL)
struct ResponseCache {
    entries: HashMap<String, CacheEntry>,
    max_entries: usize,
}

struct CacheEntry {
    content: String,
    timestamp: SystemTime,
}

impl ResponseCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            max_entries: 100,
        }
    }

    fn get(&mut self, url: &str) -> Option<String> {
        // Clean expired entries first
        self.clean_expired();

        if let Some(entry) = self.entries.get(url) {
            // Check if entry is still valid (15 minutes)
            if entry.timestamp.elapsed().ok()? < Duration::from_secs(15 * 60) {
                return Some(entry.content.clone());
            }
        }
        None
    }

    fn set(&mut self, url: String, content: String) {
        // Enforce max entries
        if self.entries.len() >= self.max_entries {
            self.clean_expired();
            if self.entries.len() >= self.max_entries {
                // Remove oldest entry
                if let Some(oldest_key) = self
                    .entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.timestamp)
                    .map(|(k, _)| k.clone())
                {
                    self.entries.remove(&oldest_key);
                }
            }
        }

        self.entries.insert(
            url,
            CacheEntry {
                content,
                timestamp: SystemTime::now(),
            },
        );
    }

    fn clean_expired(&mut self) {
        self.entries.retain(|_, entry| {
            entry
                .timestamp
                .elapsed()
                .map(|d| d < Duration::from_secs(15 * 60))
                .unwrap_or(false)
        });
    }
}

#[derive(Debug, Deserialize)]
struct WebFetchParams {
    url: String,
    prompt: String,
}

#[async_trait::async_trait]
impl Tool for WebFetchTool {
    fn id(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch content from a URL and convert HTML to markdown. \
         Processes the content with a prompt. \
         Includes 15-minute response cache. \
         Automatically upgrades HTTP to HTTPS."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Fully-formed URL to fetch (HTTP URLs upgraded to HTTPS)",
                    "format": "uri"
                },
                "prompt": {
                    "type": "string",
                    "description": "Prompt describing what information to extract from the page"
                }
            },
            "required": ["url", "prompt"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: WebFetchParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        // Upgrade HTTP to HTTPS
        let url = if params.url.starts_with("http://") {
            params.url.replace("http://", "https://")
        } else {
            params.url.clone()
        };

        tracing::debug!(url = %url, prompt = %params.prompt, "web_fetch start");

        // Check cache first
        let cached_content = self.cache.lock().unwrap().get(&url);
        let was_cached = cached_content.is_some();

        let markdown_content = if let Some(content) = cached_content {
            tracing::debug!(url = %url, "using cached content");
            content
        } else {
            // Fetch the URL
            tracing::debug!(url = %url, "fetching URL");
            let response = reqwest::get(&url)
                .await
                .map_err(|e| ToolError::Other(e.into()))?;

            // Check for redirects to different host
            let final_url = response.url().clone();
            let orig_host = url.parse::<reqwest::Url>().ok().and_then(|u| u.host_str().map(|s| s.to_string()));
            let final_host = final_url.host_str().map(|s| s.to_string());

            if orig_host.is_some() && final_host.is_some() && orig_host != final_host {
                tracing::debug!(
                    original_url = %url,
                    redirect_url = %final_url,
                    "URL redirected to different host"
                );
                return Ok(ToolResult::new(
                    "Redirect Detected",
                    format!(
                        "The URL redirected to a different host:\n\
                         Original: {}\n\
                         Redirect: {}\n\n\
                         Please make a new request with the redirect URL if you want to fetch its content.",
                        url, final_url
                    ),
                )
                .with_metadata("redirect_url", json!(final_url.to_string()))
                .with_metadata("original_url", json!(url)));
            }

            // Get HTML content
            let html = response
                .text()
                .await
                .map_err(|e| ToolError::Other(e.into()))?;

            // Convert HTML to markdown
            let markdown = html2text::from_read(html.as_bytes(), 100);

            // Cache the result
            self.cache.lock().unwrap().set(url.clone(), markdown.clone());

            markdown
        };

        // Return the content with the prompt context
        let output = format!(
            "# Content from {}\n\n\
             Prompt: {}\n\n\
             ---\n\n\
             {}",
            url, params.prompt, markdown_content
        );

        tracing::debug!(
            url = %url,
            content_len = markdown_content.len(),
            "web_fetch complete"
        );

        Ok(ToolResult::new(format!("Fetched {}", url), output)
            .with_metadata("url", json!(url))
            .with_metadata("content_length", json!(markdown_content.len()))
            .with_metadata("cached", json!(was_cached)))
    }
}
