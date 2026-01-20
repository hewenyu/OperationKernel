use crate::search::{SearchError, SearchOptions, SearchProvider, SearchResult, SearchResults};
use std::time::Duration;

/// Brave Search API provider
///
/// Requires BRAVE_API_KEY environment variable to be set.
/// Free tier: 2000 requests/month
/// Documentation: https://brave.com/search/api/
pub struct BraveSearchProvider {
    client: reqwest::Client,
    api_key: String,
}

impl BraveSearchProvider {
    /// Create a new Brave Search provider
    ///
    /// API key is read from BRAVE_API_KEY environment variable
    pub fn new() -> Self {
        let api_key = std::env::var("BRAVE_API_KEY").unwrap_or_else(|_| {
            tracing::warn!("BRAVE_API_KEY not set, web searches will fail");
            String::new()
        });

        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            api_key,
        }
    }

    /// Check if URL matches domain filtering rules
    fn matches_domain_filters(url: &str, options: &SearchOptions) -> bool {
        // White list filtering (if specified, only these domains are allowed)
        if !options.allowed_domains.is_empty() {
            let matches_allowed = options
                .allowed_domains
                .iter()
                .any(|domain| url.contains(domain));
            if !matches_allowed {
                return false;
            }
        }

        // Blacklist filtering (exclude these domains)
        if options
            .blocked_domains
            .iter()
            .any(|domain| url.contains(domain))
        {
            return false;
        }

        true
    }
}

#[async_trait::async_trait]
impl SearchProvider for BraveSearchProvider {
    async fn search(
        &self,
        query: &str,
        options: &SearchOptions,
    ) -> Result<SearchResults, SearchError> {
        if self.api_key.is_empty() {
            return Err(SearchError::InvalidApiKey);
        }

        let url = "https://api.search.brave.com/res/v1/web/search";

        tracing::debug!(
            query = %query,
            max_results = options.max_results,
            "performing brave search"
        );

        let response = self
            .client
            .get(url)
            .header("X-Subscription-Token", &self.api_key)
            .header("Accept", "application/json")
            .query(&[
                ("q", query),
                ("count", &options.max_results.to_string()),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            tracing::warn!(
                status = %status,
                error = %error_text,
                "brave search api error"
            );

            return match status.as_u16() {
                401 | 403 => Err(SearchError::InvalidApiKey),
                429 => Err(SearchError::RateLimitExceeded),
                _ => Err(SearchError::ApiError(format!(
                    "HTTP {}: {}",
                    status, error_text
                ))),
            };
        }

        let json: serde_json::Value = response.json().await?;

        let mut items = Vec::new();
        if let Some(web_results) = json["web"]["results"].as_array() {
            for result in web_results {
                let url_str = result["url"].as_str().unwrap_or("").to_string();

                // Apply domain filtering
                if !Self::matches_domain_filters(&url_str, options) {
                    tracing::trace!(url = %url_str, "filtered out by domain rules");
                    continue;
                }

                items.push(SearchResult {
                    title: result["title"].as_str().unwrap_or("").to_string(),
                    url: url_str,
                    snippet: result["description"].as_str().unwrap_or("").to_string(),
                });

                // Stop once we have enough results
                if items.len() >= options.max_results {
                    break;
                }
            }
        }

        tracing::debug!(
            query = %query,
            result_count = items.len(),
            "brave search completed"
        );

        Ok(SearchResults {
            items,
            total_results: json["web"]["total"].as_u64(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_filtering_whitelist() {
        let options = SearchOptions {
            max_results: 10,
            allowed_domains: vec!["github.com".to_string(), "rust-lang.org".to_string()],
            blocked_domains: vec![],
        };

        assert!(BraveSearchProvider::matches_domain_filters(
            "https://github.com/rust-lang/rust",
            &options
        ));
        assert!(BraveSearchProvider::matches_domain_filters(
            "https://www.rust-lang.org/",
            &options
        ));
        assert!(!BraveSearchProvider::matches_domain_filters(
            "https://stackoverflow.com/questions",
            &options
        ));
    }

    #[test]
    fn test_domain_filtering_blacklist() {
        let options = SearchOptions {
            max_results: 10,
            allowed_domains: vec![],
            blocked_domains: vec!["example.com".to_string(), "spam.org".to_string()],
        };

        assert!(BraveSearchProvider::matches_domain_filters(
            "https://github.com/",
            &options
        ));
        assert!(!BraveSearchProvider::matches_domain_filters(
            "https://example.com/page",
            &options
        ));
        assert!(!BraveSearchProvider::matches_domain_filters(
            "https://spam.org/",
            &options
        ));
    }

    #[test]
    fn test_domain_filtering_whitelist_and_blacklist() {
        let options = SearchOptions {
            max_results: 10,
            allowed_domains: vec!["github.com".to_string()],
            blocked_domains: vec!["gist.github.com".to_string()],
        };

        assert!(BraveSearchProvider::matches_domain_filters(
            "https://github.com/rust-lang/rust",
            &options
        ));
        assert!(!BraveSearchProvider::matches_domain_filters(
            "https://gist.github.com/user/12345",
            &options
        ));
        assert!(!BraveSearchProvider::matches_domain_filters(
            "https://gitlab.com/",
            &options
        ));
    }
}
