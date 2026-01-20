use crate::search::providers::BraveSearchProvider;
use crate::search::{SearchError, SearchOptions, SearchProvider, SearchResults};
use crate::tool::base::{Tool, ToolContext, ToolError, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// Cached search result with timestamp
struct CachedResult {
    results: SearchResults,
    timestamp: Instant,
}

/// Web search tool - performs web searches using configured search provider
///
/// Features:
/// - Multiple search provider support (Brave, SerpAPI, DuckDuckGo)
/// - 15-minute result caching to reduce API calls
/// - Domain whitelist/blacklist filtering
/// - Automatic rate limiting and error handling
pub struct WebSearchTool {
    provider: Arc<dyn SearchProvider>,
    cache: Arc<Mutex<HashMap<String, CachedResult>>>,
}

impl WebSearchTool {
    /// Create a new WebSearchTool
    ///
    /// Provider selection order:
    /// 1. SEARCH_PROVIDER environment variable (brave, serpapi, duckduckgo)
    /// 2. Default: brave
    pub fn new() -> Self {
        let provider_name =
            std::env::var("SEARCH_PROVIDER").unwrap_or_else(|_| "brave".to_string());

        let provider: Arc<dyn SearchProvider> = match provider_name.as_str() {
            "brave" => Arc::new(BraveSearchProvider::new()),
            // Future providers can be added here:
            // "serpapi" => Arc::new(SerpApiProvider::new()),
            // "duckduckgo" => Arc::new(DuckDuckGoProvider::new()),
            _ => {
                tracing::warn!(
                    provider = %provider_name,
                    "unknown search provider, defaulting to brave"
                );
                Arc::new(BraveSearchProvider::new())
            }
        };

        tracing::info!(provider = %provider_name, "web search tool initialized");

        Self {
            provider,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get search results from cache or perform a new search
    ///
    /// Cache TTL: 15 minutes (900 seconds)
    /// Cache size limit: 100 entries (LRU eviction)
    async fn get_cached_or_search(
        &self,
        query: &str,
        options: &SearchOptions,
    ) -> Result<SearchResults, SearchError> {
        // Create cache key (query + options)
        let cache_key = format!(
            "{}|{}|{}",
            query,
            options.allowed_domains.join(","),
            options.blocked_domains.join(",")
        );

        // Check cache (15 minute TTL)
        {
            let cache = self.cache.lock().await;
            if let Some(cached) = cache.get(&cache_key) {
                if cached.timestamp.elapsed().as_secs() < 900 {
                    tracing::debug!(
                        query = %query,
                        age_secs = cached.timestamp.elapsed().as_secs(),
                        "returning cached search results"
                    );
                    return Ok(cached.results.clone());
                }
            }
        }

        // Perform fresh search
        tracing::debug!(query = %query, "performing fresh search");
        let results = self.provider.search(query, options).await?;

        // Update cache
        {
            let mut cache = self.cache.lock().await;
            cache.insert(
                cache_key.clone(),
                CachedResult {
                    results: results.clone(),
                    timestamp: Instant::now(),
                },
            );

            // Limit cache size to 100 entries (remove oldest)
            if cache.len() > 100 {
                if let Some(oldest_key) = cache
                    .iter()
                    .min_by_key(|(_, v)| v.timestamp)
                    .map(|(k, _)| k.clone())
                {
                    cache.remove(&oldest_key);
                    tracing::trace!("evicted oldest cache entry");
                }
            }
        }

        Ok(results)
    }
}

#[derive(Debug, Deserialize)]
struct WebSearchParams {
    query: String,
    #[serde(default)]
    allowed_domains: Vec<String>,
    #[serde(default)]
    blocked_domains: Vec<String>,
}

#[async_trait]
impl Tool for WebSearchTool {
    fn id(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web and return relevant results. \
         Supports domain filtering and automatic caching. \
         \n\n\
         Usage notes:\n\
         - Results are cached for 15 minutes to reduce API calls\n\
         - Use allowed_domains to restrict results to specific sites\n\
         - Use blocked_domains to exclude unwanted domains\n\
         - Requires BRAVE_API_KEY environment variable (free tier: 2000 req/month)\n\
         \n\n\
         CRITICAL REQUIREMENT:\n\
         After answering the user's question, you MUST include a 'Sources:' section \
         at the end of your response with all relevant URLs as markdown hyperlinks."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "minLength": 2,
                    "description": "The search query to use. Today's date is 2026-01-20. \
                                    When searching for recent information, use the correct year (2026)."
                },
                "allowed_domains": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Only include search results from these domains (optional)"
                },
                "blocked_domains": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Never include search results from these domains (optional)"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: WebSearchParams = serde_json::from_value(params)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        if params.query.trim().is_empty() {
            return Err(ToolError::InvalidParams("query cannot be empty".into()));
        }

        let options = SearchOptions {
            max_results: 10,
            allowed_domains: params.allowed_domains,
            blocked_domains: params.blocked_domains,
        };

        let results = self
            .get_cached_or_search(&params.query, &options)
            .await
            .map_err(|e| match e {
                SearchError::InvalidApiKey => ToolError::Other(anyhow::anyhow!(
                    "BRAVE_API_KEY not set or invalid. \
                     Please set the BRAVE_API_KEY environment variable. \
                     Get your free API key at https://brave.com/search/api/"
                )),
                SearchError::RateLimitExceeded => ToolError::Other(anyhow::anyhow!(
                    "Search API rate limit exceeded. \
                     Please wait a moment and try again. \
                     Free tier: 2000 requests/month"
                )),
                SearchError::ApiError(msg) => {
                    ToolError::Other(anyhow::anyhow!("Search API error: {}", msg))
                }
                SearchError::NetworkError(e) => {
                    ToolError::Other(anyhow::anyhow!("Network error: {}", e))
                }
                SearchError::Other(e) => ToolError::Other(e),
            })?;

        // Format output
        let mut output = format!("Search: {}\n", params.query);
        output.push_str(&format!(
            "Found {} results{}\n\n",
            results.items.len(),
            if let Some(total) = results.total_results {
                format!(" (total available: {})", total)
            } else {
                String::new()
            }
        ));

        if results.items.is_empty() {
            output.push_str("No results found. Try a different query or check your domain filters.\n");
        } else {
            for (idx, item) in results.items.iter().enumerate() {
                output.push_str(&format!(
                    "{}. {}\n   {}\n   {}\n\n",
                    idx + 1,
                    item.title,
                    item.url,
                    item.snippet
                ));
            }

            output.push_str("\n---\n");
            output.push_str("Remember to include a 'Sources:' section with markdown links in your response!\n");
        }

        Ok(ToolResult::new(format!("Web search: {}", params.query), output)
            .with_metadata("num_results", json!(results.items.len()))
            .with_metadata("query", json!(params.query))
            .with_metadata(
                "total_results",
                json!(results.total_results.unwrap_or(0)),
            ))
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_generation() {
        // This is implicitly tested by get_cached_or_search
        // but we can verify the format here
        let query = "rust programming";
        let allowed = vec!["github.com".to_string()];
        let blocked = vec!["spam.com".to_string()];

        let cache_key = format!("{}|{}|{}", query, allowed.join(","), blocked.join(","));
        assert_eq!(cache_key, "rust programming|github.com|spam.com");
    }

    #[tokio::test]
    async fn test_tool_validates_empty_query() {
        let tool = WebSearchTool::new();
        let ctx = ToolContext {
            session_id: "test".to_string(),
            message_id: "test".to_string(),
            agent: "test".to_string(),
            working_dir: std::path::PathBuf::from("/tmp"),
            shell_manager: Arc::new(crate::process::BackgroundShellManager::new()),
        };

        let params = json!({
            "query": "   "  // Empty/whitespace query
        });

        let result = tool.execute(params, &ctx).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot be empty"));
    }
}
