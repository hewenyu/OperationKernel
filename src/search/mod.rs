pub mod providers;

use anyhow::Result;

/// Search provider abstraction - different providers can be plugged in
#[async_trait::async_trait]
pub trait SearchProvider: Send + Sync {
    /// Perform a search query with given options
    async fn search(&self, query: &str, options: &SearchOptions) -> Result<SearchResults, SearchError>;
}

/// Search options for filtering and controlling results
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// Maximum number of results to return (default: 10)
    pub max_results: usize,
    /// Only include results from these domains (empty = no filter)
    pub allowed_domains: Vec<String>,
    /// Exclude results from these domains
    pub blocked_domains: Vec<String>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_results: 10,
            allowed_domains: Vec::new(),
            blocked_domains: Vec::new(),
        }
    }
}

/// Search results container
#[derive(Debug, Clone)]
pub struct SearchResults {
    /// Individual search result items
    pub items: Vec<SearchResult>,
    /// Total number of results available (if provided by the search engine)
    pub total_results: Option<u64>,
}

/// Individual search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Page title
    pub title: String,
    /// Page URL
    pub url: String,
    /// Snippet/description of the page content
    pub snippet: String,
}

/// Search-related errors
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("API error: {0}")]
    ApiError(String),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Invalid API key")]
    InvalidApiKey,

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}
